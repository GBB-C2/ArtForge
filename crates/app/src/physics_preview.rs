//! Worker-owned cloth preview. Snapshots share completed vertex buffers until the next tick.
use super::*;
use poseforge_core::{Mesh, PhysicsKind, PhysicsSettings};
use poseforge_simulation::{
    deform::{Collider, DeformConfig, DeformWorld},
    mesh_contact::MeshCollider,
};
#[derive(Clone, PartialEq)]
pub(super) struct BodyInput {
    pub fields: poseforge_simulation::fields::Fields,
    pub id: u64,
    pub mesh: Mesh,
    pub model: Mat4,
    pub settings: PhysicsSettings,
    pub colliders: Vec<scene_colliders::Input>,
}
impl BodyInput {
    pub fn world(&self) -> Result<DeformWorld, String> {
        if !self.model.is_finite() || self.model.determinant().abs() < 1e-8 {
            return Err("Ткань: восстановите ненулевой масштаб объекта".into());
        }
        let local: Vec<_> = self
            .mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.position))
            .collect();
        let min = local
            .iter()
            .copied()
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let max = local
            .iter()
            .copied()
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        let axis = if max.y - min.y > 1e-4 { 1 } else { 2 };
        let edge = if self.settings.kind == PhysicsKind::Muscle {
            min[axis]
        } else {
            max[axis]
        };
        let pinned: Vec<_> = local
            .iter()
            .map(|p| self.settings.pinned && (p[axis] - edge).abs() < 0.0001)
            .collect();
        let mut world = DeformWorld::new(
            local
                .iter()
                .map(|p| self.model.transform_point3(*p))
                .collect(),
            &self.mesh.indices,
            &pinned,
        )?;
        world.set_axis(self.model.transform_vector3(Vec3::Y));
        world.set_fields(self.fields.clone());
        Ok(world)
    }
    pub fn config(&self) -> DeformConfig {
        let s = self.settings;
        DeformConfig {
            self_collision: s.self_collision,
            bending: s.bending,
            friction: s.friction,
            iterations: s.iterations,
            compliance: s.compliance,
            damping: s.damping,
            gravity: s.gravity,
            wind: s.wind,
            radius: s.thickness,
            activation: s.activation,
            contraction: s.contraction,
            muscle: s.kind == PhysicsKind::Muscle,
        }
    }
    pub fn snapshot(&self, world: &DeformWorld) -> Mesh {
        let normals = world.normals(&self.mesh.indices);
        let inverse = self.model.inverse();
        let mut mesh = self.mesh.clone();
        if let Some(topology) = &mut mesh.topology {
            topology.primitive = None;
        }
        for ((v, p), n) in mesh.vertices.iter_mut().zip(&world.positions).zip(normals) {
            v.position = inverse.transform_point3(*p).to_array();
            v.normal = self
                .model
                .transpose()
                .transform_vector3(n)
                .normalize_or_zero()
                .to_array();
        }
        mesh
    }
}
struct Body {
    input: BodyInput,
    world: DeformWorld,
    collider: Arc<MeshCollider>,
}
pub(super) struct Snapshot {
    input: BodyInput,
    mesh: Mesh,
}
struct Request {
    generation: u64,
    inputs: Vec<BodyInput>,
    elapsed: f64,
    cancel: Arc<AtomicBool>,
}
struct Completed {
    generation: u64,
    result: Result<Vec<Snapshot>, String>,
}
#[derive(Default)]
pub(super) struct PhysicsPreview {
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: Option<Receiver<Completed>>,
    pending: Option<Arc<AtomicBool>>,
    generation: u64,
    elapsed: f64,
    pub snapshots: Vec<Snapshot>,
    collider_inputs: Vec<BodyInput>,
    collider_boxes: Vec<Collider>,
}
pub(super) fn inputs(project: &Project, colliders: bool) -> Vec<BodyInput> {
    let Ok(globals) = project.global_transforms() else {
        return vec![];
    };
    let visible = project.visible_node_ids(false);
    let fields=if colliders {Default::default()}else{scene_fields::collect(project,&globals,false,1,Mat4::IDENTITY)};
    project
        .nodes
        .iter()
        .filter_map(|n| {
            let settings = n.physics?;
            if !settings.visible || !visible.contains(&n.id)
                || (settings.kind == PhysicsKind::Collider) != colliders
                || (!colliders && project.node_locked(n.id))
            {
                return None;
            }
            Some(BodyInput {
                fields:fields.clone(),
                id: n.id,
                mesh: project.mesh(n.mesh?)?.clone(),
                model: *globals.get(&n.id)?,
                settings,
                colliders: if colliders {vec![]} else {scene_colliders::inputs(project, &globals, false, Some(n.id), Mat4::IDENTITY, None, n.collision_targets.cloth.as_deref())},
            })
        })
        .collect()
}
impl PhysicsPreview {
    pub fn is_pending(&self) -> bool { self.pending.is_some() }
    pub fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(cancel) = self.pending.take() { cancel.store(true, Ordering::Relaxed); }
        self.elapsed = 0.;
        self.snapshots.clear();
    }
    fn start(&mut self) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let (done, receiver) = mpsc::channel();
        std::thread::Builder::new().name("cloth-preview".into()).spawn(move || {
            let mut generation = u64::MAX;
            let mut bodies = BTreeMap::<u64, Body>::new();
            let mut backlog = 0.;
            for request in rx {
                if request.cancel.load(Ordering::Relaxed) { continue; }
                if generation != request.generation {
                    bodies.clear();
                    backlog = 0.;
                    generation = request.generation;
                }
                let result = (|| -> Result<Vec<Snapshot>, String> {
                    if request
                        .inputs
                        .iter()
                        .map(|i| i.mesh.vertices.len())
                        .sum::<usize>()
                        > 65536
                    {
                        return Err("Физика: лимит предпросмотра 65536 вершин".into());
                    }
                    bodies.retain(|id, _| request.inputs.iter().any(|i| i.id == *id));
                    for input in request.inputs {
                        if request.cancel.load(Ordering::Relaxed) { return Err("Расчёт ткани отменён".into()); }
                        let replace = bodies.get(&input.id).is_none_or(|b| {
                            b.input.mesh != input.mesh
                                || b.input.model != input.model
                                || b.input.settings.kind != input.settings.kind
                                || b.input.settings.pinned != input.settings.pinned
                                || b.input.colliders != input.colliders
                        });
                        if replace {
                            let world = input.world()?;
                            // Identical receiver lists share the expensive triangle tree.
                            let collider=if let Some(body)=bodies.values().find(|b|b.input.colliders==input.colliders) {
                                body.collider.clone()
                            } else {Arc::new(scene_colliders::surface(&input.colliders,&request.cancel)?)};
                            bodies.insert(input.id, Body { input, world, collider });
                        } else {
                            let id = input.id;
                            let body=bodies.get_mut(&id).unwrap();
                            body.world.set_fields(input.fields.clone());body.input = input;
                        }
                    }
                    backlog = (backlog + request.elapsed).min(0.25);
                    let ticks = (backlog * 120.).floor().min(8.) as usize;
                    backlog -= ticks as f64 / 120.;
                    for b in bodies.values_mut() {
                        for _ in 0..ticks {
                            if !b.world.step_with_mesh_cancellable(b.input.config(), &[], &b.collider, &request.cancel) {
                                return Err("Расчёт ткани отменён".into());
                            }
                        }
                    }
                    Ok(bodies
                        .values()
                        .map(|b| Snapshot {
                            input: b.input.clone(),
                            mesh: b.input.snapshot(&b.world),
                        })
                        .collect())
                })();
                if request.cancel.load(Ordering::Relaxed) { continue; }
                if done
                    .send(Completed {
                        generation: request.generation,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        }).map_err(|e| format!("Не удалось запустить расчёт ткани: {e}"))?;
        self.sender = Some(tx);
        self.receiver = Some(receiver);
        Ok(())
    }
    fn advance(&mut self, inputs: Vec<BodyInput>, dt: f64) -> Result<(), String> {
        if !dt.is_finite() || dt < 0. { return Err("Некорректное время расчёта ткани".into()); }
        self.elapsed = (self.elapsed + dt).min(0.25);
        if self.pending.is_some() { return Ok(()); }
        if self.sender.is_none() { self.start()?; }
        let cancel = Arc::new(AtomicBool::new(false));
        let request = Request {
            generation: self.generation, inputs,
            elapsed: self.elapsed, cancel: cancel.clone(),
        };
        match self.sender.as_ref().unwrap().try_send(request) {
            Ok(()) => { self.pending = Some(cancel); self.elapsed = 0.; }
            // A canceled request may still occupy the single waiting slot.
            // Keep elapsed time, retry next frame, and never wait on the UI thread.
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.reset(); self.sender = None; self.receiver = None;
                return Err("Расчёт ткани остановлен; нажмите запуск для повтора".into());
            }
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<(), String> {
        loop {
            let Some(receiver) = &self.receiver else { return Ok(()); };
            match receiver.try_recv() {
                Ok(completed) if completed.generation == self.generation => {
                    self.pending = None;
                    self.snapshots = completed.result?;
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.reset(); self.sender = None; self.receiver = None;
                    return Err("Расчёт ткани остановлен; нажмите запуск для повтора".into());
                }
            }
        }
    }
}
impl Drop for PhysicsPreview {
    fn drop(&mut self) {
        if let Some(cancel) = &self.pending { cancel.store(true, Ordering::Relaxed); }
    }
}
pub(super) fn node_bounds(project: &Project, id: u64) -> Option<(Vec3, Vec3)> {
    let globals = project.global_transforms().ok()?;
    let model = *globals.get(&id)?;
    let mesh = project.mesh(project.node(id)?.mesh?)?;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        let p = model.transform_point3(Vec3::from_array(v.position));
        min = min.min(p);
        max = max.max(p);
    }
    min.is_finite().then_some((min, max))
}
impl App {
    pub(super) fn tick_physics(&mut self, dt: f64) {
        if let Err(error) = self.studio.physics.poll() {
            self.studio.physics_running = false;
            self.status = error;
        }
        let project = &self.editor.project;
        let colliders = inputs(project, true);
        if self.studio.physics.collider_inputs != colliders {
            let boxes: Vec<_> = colliders
                .iter()
                .take(64)
                .filter_map(|i| {
                    let min = i
                        .mesh
                        .vertices
                        .iter()
                        .map(|v| i.model.transform_point3(Vec3::from_array(v.position)))
                        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
                    let max = i
                        .mesh
                        .vertices
                        .iter()
                        .map(|v| i.model.transform_point3(Vec3::from_array(v.position)))
                        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
                    min.is_finite().then_some(Collider {
                        min: min - Vec3::splat(0.001),
                        max: max + Vec3::splat(0.001),
                    })
                })
                .collect();
            self.studio.physics.collider_boxes = boxes;
            self.studio.physics.collider_inputs = colliders.clone();
        }
        if self.fluid.colliders() != self.studio.physics.collider_boxes {
            self.fluid_preview.cancel();
            if let Err(e) = self
                .fluid
                .set_colliders(self.studio.physics.collider_boxes.clone())
            {
                self.status = e.to_string();
            }
        }
        if self.studio.physics_running && self.job.is_none() && self.drag.is_none()
            && self.input.is_none() && !self.has_modal() && !self.editor.playing {
            if let Err(error) = self.studio.physics.advance(inputs(project, false), dt) {
                self.studio.physics_running = false;
                self.status = error;
            }
        }
    }
    pub(super) fn apply_physics_preview(&self, project: &mut Project) {
        if self.studio.physics.snapshots.is_empty() {
            return;
        }
        let Ok(globals) = project.global_transforms() else {
            return;
        };
        for snapshot in &self.studio.physics.snapshots {
            let input = &snapshot.input;
            if !self.node_selectable(input.id) {
                continue;
            }
            let matches = project.node(input.id).is_some_and(|n| {
                n.physics == Some(input.settings) && n.mesh == Some(input.mesh.id)
            }) && globals.get(&input.id) == Some(&input.model)
                && project.mesh(input.mesh.id) == Some(&input.mesh);
            if !matches || scene_fields::collect(&self.editor.project,&globals,false,1,Mat4::IDENTITY)!=input.fields || scene_colliders::inputs(&self.editor.project, &globals, false, Some(input.id), Mat4::IDENTITY, None,
                self.editor.project.node(input.id).and_then(|n| n.collision_targets.cloth.as_deref())) != input.colliders {
                continue;
            }
            let mut mesh = snapshot.mesh.clone();
            mesh.id = project.next_id();
            project.node_mut(input.id).unwrap().mesh = Some(mesh.id);
            project.meshes.push(mesh);
        }
    }
    pub(super) fn bake_physics(&mut self) {
        let mut preview = self.editor.project.clone();
        self.apply_physics_preview(&mut preview);
        if preview == self.editor.project {
            self.status = "Сначала запустите расчёт ткани.".into();
            return;
        }
        for snapshot in &self.studio.physics.snapshots {
            if let Some(n) = preview.node_mut(snapshot.input.id) {
                if n.mesh != Some(snapshot.input.mesh.id) {
                    n.physics = None;
                }
            }
        }
        let r = self
            .editor
            .transact("Записать результат физики в меш", |p| *p = preview);
        if self.result(r).is_some() {
            self.studio.physics_running = false;
            self.studio.physics.reset();
            self.invalidate();
        }
    }
    pub(super) fn rebuild_fluid_source(&mut self) {
        self.fluid_preview.cancel();
        match source_world(self.editor.project.fluid_settings) {
            Ok(world) => {
                self.fluid = world;
                self.fluid_preview.cache.reset_for(&self.fluid);
                self.fluid_applied = self.editor.project.fluid_settings;
                self.status = format!(
                    "Создано {} частиц. Запуск — кнопка ▶ на таймлайне.",
                    self.fluid.positions().len()
                );
            }
            Err(e) => self.status = e,
        }
    }
}

pub(super) fn source_spacing(s: poseforge_core::FluidSettings) -> f32 {
    let mut size = Vec3::from_array(s.source_size);
    if s.source_kind == 2 {
        size.x = size.x.min(0.24);
        size.z = size.z.min(0.24);
    }
    let mut spacing = (size.x * size.y * size.z / 3000.).cbrt().max(0.08);
    while size
        .to_array()
        .map(|v| ((v / spacing / 2.).ceil().max(1.) as usize) * 2 + 1)
        .into_iter()
        .product::<usize>()
        > 6000
    {
        spacing *= 1.1;
    }
    spacing
}
pub(super) fn source_world(settings: poseforge_core::FluidSettings) -> Result<FluidWorld, String> {
    let center = Vec3::from_array(settings.source_center);
    let mut size = Vec3::from_array(settings.source_size);
    if settings.source_kind == 2 {
        size.x = size.x.min(0.24);
        size.z = size.z.min(0.24);
    }
    let spacing = source_spacing(settings);
    let cells = size
        .to_array()
        .map(|v| ((v / spacing / 2.).ceil().max(1.) as usize) * 2);
    let mut positions = vec![];
    for y in 0..=cells[1] {
        for z in 0..=cells[2] {
            for x in 0..=cells[0] {
                let p = Vec3::new(
                    x as f32 / cells[0] as f32,
                    y as f32 / cells[1] as f32,
                    z as f32 / cells[2] as f32,
                ) - Vec3::splat(0.5);
                if settings.source_kind != 1 || p.length_squared() <= 0.25 {
                    positions.push(center + p * size);
                }
            }
        }
    }
    FluidWorld::new(super::fluid_preview::simulation_config(settings), positions)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    #[test]
    fn cloth_receivers_keep_independent_lists_and_legacy_membership() {
        let mut p=Project::default();p.meshes.push(poseforge_core::box_mesh(10,"Mesh",Vec3::splat(0.2),None));
        for id in 1..=3 {let mut n=poseforge_core::Node::new(id,"Mesh");n.mesh=Some(10);if id<3 {n.physics=Some(PhysicsSettings::default());}p.nodes.push(n);}
        p.node_mut(1).unwrap().collision_targets.cloth=Some(vec![poseforge_core::CollisionTarget {node:3,enabled:true}]);
        p.node_mut(2).unwrap().collision_targets.cloth=Some(vec![]);
        let bodies=inputs(&p,false);assert_eq!(bodies[0].colliders.len(),1);assert!(bodies[1].colliders.is_empty());
        p.node_mut(1).unwrap().collision_targets.cloth.as_mut().unwrap()[0].enabled=false;assert!(inputs(&p,false)[0].colliders.is_empty());
        p.node_mut(3).unwrap().physics=Some(PhysicsSettings {kind:PhysicsKind::Collider,..Default::default()});p.node_mut(1).unwrap().collision_targets.cloth=None;
        assert_eq!(inputs(&p,false)[0].colliders.len(),1);assert!(inputs(&p,false)[1].colliders.is_empty());
    }
    #[test]
    fn paused_snapshots_reuse_vertex_storage_and_undo_keeps_input_mesh() {
        let mut p = Project::default();
        let mesh = poseforge_core::box_mesh(2, "cloth", Vec3::splat(0.2), None);
        p.meshes.push(mesh);
        let mut node = poseforge_core::Node::new(1, "cloth");
        node.mesh = Some(2);
        node.transform.translation = Vec3::Y * 2.;
        node.physics = Some(PhysicsSettings {
            pinned: false,
            ..Default::default()
        });
        p.nodes.push(node);
        let mut app = App::new(None);
        app.editor = Editor::new(p);
        let input = inputs(&app.editor.project, false).remove(0);
        let mut world = input.world().unwrap();
        world.step(input.config(), &[]);
        let mesh = input.snapshot(&world);
        let original = input.mesh.clone();
        app.studio.physics.snapshots.push(Snapshot { input, mesh });
        let mut one = app.editor.project.clone();
        let mut two = one.clone();
        app.apply_physics_preview(&mut one);
        app.apply_physics_preview(&mut two);
        let a = one.mesh(one.node(1).unwrap().mesh.unwrap()).unwrap();
        let b = two.mesh(two.node(1).unwrap().mesh.unwrap()).unwrap();
        assert_eq!(a.vertices.revision(), b.vertices.revision());
        assert!(a.vertices.shares_storage(&b.vertices));
        assert_eq!(app.editor.project.mesh(2).unwrap(), &original);
    }

    #[test]
    fn dead_cloth_worker_releases_pending_state_and_handles() {
        let (tx, rx) = mpsc::channel::<Completed>(); drop(tx);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut preview = PhysicsPreview::default();
        preview.receiver = Some(rx); preview.pending = Some(cancel.clone());
        assert!(preview.poll().is_err());
        assert!(cancel.load(Ordering::Relaxed));
        assert!(preview.pending.is_none() && preview.receiver.is_none() && preview.sender.is_none());
    }
    #[test]
    fn cloth_queue_never_blocks_and_reset_invalidates_stale_results() {
        let (sender, requests) = mpsc::sync_channel(1);
        let (done, receiver) = mpsc::channel();
        let mut preview = PhysicsPreview::default();
        preview.sender = Some(sender); preview.receiver = Some(receiver);
        preview.advance(vec![], 0.01).unwrap();
        let cancel = preview.pending.clone().unwrap(); let generation = preview.generation;
        preview.reset();
        assert!(cancel.load(Ordering::Relaxed));
        preview.advance(vec![], 0.02).unwrap();
        assert!(preview.pending.is_none()); assert_eq!(preview.elapsed, 0.02);
        let _old = requests.try_recv().unwrap();
        done.send(Completed { generation, result: Err("stale error".into()) }).unwrap();
        preview.poll().unwrap();
        preview.advance(vec![], 0.).unwrap();
        assert_eq!(requests.try_recv().unwrap().elapsed, 0.02);
    }

}
