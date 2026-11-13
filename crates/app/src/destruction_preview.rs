//! Transient rigid-body preview. Workers own simulation; the document never stores frames.
use super::{scene_colliders, scene_fields, App};
use glam::{Mat4, Quat, Vec3, Vec4};
use poseforge_core::{DestructionSettings, EvaluatedScene, Mesh, Project};
use poseforge_simulation::{fields::Fields, mesh_contact::{MeshCollider, Triangle},
    shard_contact::{solve_shard_contacts, ShardContact, ShardForces, ShardMotion, ShardShape}};
use std::{collections::{BTreeMap, BTreeSet}, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}};

#[derive(Clone, PartialEq)]
struct ShardInput { id: u64, mesh: Mesh, model: Mat4 }

#[derive(Clone, PartialEq)]
struct Input {
    source: u64,
    root: u64,
    center: Vec3,
    settings: DestructionSettings,
    shards: Vec<ShardInput>,
    fields: Fields,
    colliders: Vec<scene_colliders::Input>,
}
impl Input {
    fn collect(project: &Project, scene: &EvaluatedScene, source: u64) -> Result<Self, String> {
        let node = project.node(source).ok_or("Исходный меш удалён")?;
        let mut settings = node.destruction.ok_or("Добавьте разрушение мешу")?;
        settings.validate().map_err(|e| e.to_string())?;
        // Display and generation options must not restart an active physics preview.
        settings.randomize=false;settings.show_floor=false;settings.dust_enabled=false;
        let defaults=DestructionSettings::default();settings.dust_density=defaults.dust_density;
        settings.dust_size=defaults.dust_size;settings.dust_lifetime=defaults.dust_lifetime;
        let root = settings.result.ok_or("Сначала создайте осколки")?;
        if !settings.visible || !project.node_visible(root, false) || project.node_locked(source)
            || project.node_locked(root) { return Err("Включите осколки и снимите блокировку для запуска".into()); }
        let center = scene.global.get(&root).ok_or("Группа осколков удалена")?.w_axis.truncate();
        let mut shards = Vec::new();
        for node in project.nodes.iter().filter(|n| n.parent == Some(root) && project.node_visible(n.id, false)) {
            let Some(mesh) = node.mesh.and_then(|id| project.mesh(id)) else { continue; };
            let model = *scene.global.get(&node.id).ok_or("Осколок недоступен")?;
            if !model.is_finite() || model.determinant().abs() < 1e-8 { return Err("Восстановите ненулевой масштаб осколков".into()); }
            shards.push(ShardInput { id: node.id, mesh: mesh.clone(), model });
        }
        if shards.is_empty() || shards.len() > 64 || shards.iter().map(|s| s.mesh.vertices.len()).sum::<usize>() > 100_000
            || shards.iter().map(|s| s.mesh.indices.len() / 3).sum::<usize>() > 200_000 {
            return Err("Предпросмотр: до 64 осколков, 100000 вершин и 200000 треугольников суммарно".into());
        }
        let colliders = if settings.mesh_collision {
            scene_colliders::inputs(project, &scene.global, false, Some(source), Mat4::IDENTITY,
                Some(scene), node.collision_targets.destruction.as_deref())
        } else { Vec::new() };
        Ok(Self { source, root, center, settings, shards, colliders,
            fields: scene_fields::collect(project, &scene.global, false, 2, Mat4::IDENTITY) })
    }
}

struct Body { id: u64, basis: Mat4, shape: ShardShape, motion: ShardMotion, previous: ShardMotion }
struct World { bodies: Vec<Body>, collider: Option<Arc<MeshCollider>>, fields: Fields, forces: ShardForces, shard_collision: bool, time: f64, backlog: f64 }
struct Snapshot { matrices: BTreeMap<u64, Mat4>, time: f64 }
impl World {
    fn new(input: &Input, cancel: &AtomicBool, cache: &mut Vec<(Vec<scene_colliders::Input>, Arc<MeshCollider>)>) -> Result<Self, String> {
        let collider = if input.colliders.is_empty() { None }
            else if let Some((_, collider)) = cache.iter().find(|(inputs, _)| *inputs == input.colliders) { Some(collider.clone()) }
            else {
                let collider = Arc::new(scene_colliders::surface(&input.colliders, cancel)?);
                cache.push((input.colliders.clone(), collider.clone())); Some(collider)
            };
        let mut bodies = Vec::new();
        for (index, shard) in input.shards.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) { return Err("Предпросмотр отменён".into()); }
            let mut triangles = Vec::new();
            for (i, face) in shard.mesh.indices.chunks_exact(3).enumerate() {
                if i % 256 == 0 && cancel.load(Ordering::Relaxed) { return Err("Предпросмотр отменён".into()); }
                let point = |index: u32| shard.mesh.vertices.get(index as usize)
                    .map(|v| shard.model.transform_vector3(Vec3::from_array(v.position)))
                    .ok_or("Повреждена геометрия осколка");
                triangles.push(Triangle([point(face[0])?, point(face[1])?, point(face[2])?]));
            }
            let position = shard.model.w_axis.truncate();
            let direction = (position - input.center).try_normalize().unwrap_or(Vec3::Y);
            let seed_angle = (index as f32 * 2.399963 + input.settings.values[1] as f32 * 0.1).sin();
            let axis = Vec3::new(direction.z + 0.3, seed_angle, -direction.x + 0.2).try_normalize().unwrap_or(Vec3::Y);
            let mut basis = shard.model; basis.w_axis = Vec4::W;
            let motion = ShardMotion { position, rotation: Quat::IDENTITY,
                    velocity: (direction + Vec3::Y * 0.35) * input.settings.values[3] as f32,
                    angular_axis: axis, angular_speed: input.settings.values[3] as f32 * 0.5 };
            bodies.push(Body { id: shard.id, basis, shape: ShardShape::new(triangles)?, motion, previous: motion });
        }
        Ok(Self { bodies, collider, fields: input.fields.clone(), shard_collision: input.settings.shard_collision, time: 0., backlog: 0.,
            forces: ShardForces { gravity: input.settings.values[4] as f32, floor: input.settings.collision_floor(),
                restitution: input.settings.values[6] as f32 } })
    }
    #[cfg(test)]
    fn advance(&mut self, elapsed: f64, cancel: &AtomicBool) -> Result<Snapshot, String> {
        advance_worlds(std::slice::from_mut(self),elapsed,cancel)
    }
}
fn advance_worlds(worlds:&mut[World], elapsed:f64,cancel:&AtomicBool)->Result<Snapshot,String> {
        let Some(first)=worlds.first() else{return Err("Нет групп для предпросмотра".into());};
        let backlog=(first.backlog+elapsed).min(0.25);
        let ticks = (backlog * 120.).floor().min(8.) as usize;
        for _ in 0..ticks {
            for world in worlds.iter_mut() {for body in &mut world.bodies {
                if cancel.load(Ordering::Relaxed) { return Err("Предпросмотр отменён".into()); }
                body.previous=body.motion;
                let acceleration = world.fields.acceleration(body.motion.position, body.motion.velocity);
                body.motion.velocity = (body.motion.velocity + acceleration / 120.).clamp_length_max(100.);
                body.shape.step(&mut body.motion, world.forces, world.collider.as_deref(), 1. / 120., cancel)?;
            }}
            if worlds.iter().any(|w|w.shard_collision) {
                let mut contacts=Vec::new();
                for world in worlds.iter_mut() {for body in &mut world.bodies {
                    contacts.push(ShardContact {shape:&body.shape,motion:&mut body.motion,previous:body.previous,
                        enabled:world.shard_collision,restitution:world.forces.restitution,floor:world.forces.floor});
                }}
                solve_shard_contacts(&mut contacts,cancel)?;
                drop(contacts);
                for world in worlds.iter_mut().filter(|w|w.shard_collision) {for body in &mut world.bodies {
                    body.shape.resolve_environment(&mut body.motion,body.previous,world.forces,world.collider.as_deref(),cancel)?;
                }}
            }
            for world in worlds.iter_mut() {world.time+=1./120.;}
        }
        for world in worlds.iter_mut() {world.backlog=backlog-ticks as f64/120.;}
        Ok(Snapshot { time: worlds[0].time, matrices: worlds.iter().flat_map(|w|&w.bodies).map(|body|
            (body.id, Mat4::from_rotation_translation(body.motion.rotation, body.motion.position) * body.basis)).collect() })
}

fn collect_inputs(project: &Project, scene: &EvaluatedScene, sources: &[u64]) -> Result<Vec<Input>, String> {
    if sources.is_empty() || sources.len() > 16 { return Err("Предпросмотр: от 1 до 16 групп осколков".into()); }
    let inputs = sources.iter().map(|&source| Input::collect(project, scene, source)).collect::<Result<Vec<_>,_>>()?;
    let shards = inputs.iter().flat_map(|i| &i.shards);
    if shards.clone().count() > 64 || shards.clone().map(|s|s.mesh.vertices.len()).sum::<usize>() > 100_000
        || shards.map(|s|s.mesh.indices.len()/3).sum::<usize>() > 200_000 {
        return Err("Предпросмотр всех групп: до 64 осколков, 100000 вершин и 200000 треугольников суммарно".into());
    }
    Ok(inputs)
}
struct Request { generation: u64, input: Vec<Input>, elapsed: f64, cancel: Arc<AtomicBool> }
struct Completed { generation: u64, result: Result<Snapshot, String> }
#[derive(Default)]
pub(super) struct Preview {
    sender: Option<mpsc::SyncSender<Request>>,
    receiver: Option<mpsc::Receiver<Completed>>,
    pending: Option<Arc<AtomicBool>>,
    generation: u64,
    input: Option<Vec<Input>>,
    snapshot: Option<Snapshot>,
    running: bool,
    elapsed: f64,
}
impl Preview {
    pub fn running(&self) -> bool { self.running }
    pub fn running_for(&self, source: u64) -> bool { self.running && self.input.as_ref().is_some_and(|inputs| inputs.iter().any(|i| i.source == source)) }
    pub fn is_pending(&self) -> bool { self.pending.is_some() }
    pub fn pause(&mut self) { self.running = false; self.elapsed = 0.; }
    pub fn reset(&mut self) {
        if let Some(cancel) = self.pending.take() { cancel.store(true, Ordering::Relaxed); }
        self.generation = self.generation.wrapping_add(1);
        self.running = false; self.elapsed = 0.; self.input = None; self.snapshot = None;
    }
    pub fn affects(&self, id: u64) -> bool { self.input.as_ref().is_some_and(|inputs| inputs.iter().any(|i| i.root == id || i.shards.iter().any(|s| s.id == id))) }
    pub fn time(&self, source: u64) -> Option<f64> {
        self.input.as_ref().filter(|inputs| inputs.iter().any(|i| i.source == source)).map(|_| self.snapshot.as_ref().map_or(0., |s| s.time))
    }
    pub fn times(&self) -> BTreeMap<u64,f64> {
        self.input.iter().flatten().map(|input|(input.source,self.snapshot.as_ref().map_or(0.,|s|s.time))).collect()
    }
    fn start(&mut self) -> Result<(), String> {
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (completed, receiver) = mpsc::channel();
        std::thread::Builder::new().name("shards-preview".into()).spawn(move || {
            let mut active: Option<(u64, Vec<World>)> = None;
            for request in requests {
                if request.cancel.load(Ordering::Relaxed) { continue; }
                let result = (|| {
                    if active.as_ref().is_none_or(|(generation, _)| *generation != request.generation) {
                        let mut cache = Vec::new();
                        let worlds = request.input.iter().map(|input| World::new(input, &request.cancel, &mut cache)).collect::<Result<Vec<_>, _>>()?;
                        active = Some((request.generation, worlds));
                    }
                    advance_worlds(&mut active.as_mut().unwrap().1,request.elapsed,&request.cancel)
                })();
                if completed.send(Completed { generation: request.generation, result }).is_err() { break; }
            }
        }).map_err(|e| format!("Не удалось запустить предпросмотр осколков: {e}"))?;
        self.sender = Some(sender); self.receiver = Some(receiver); Ok(())
    }
    fn advance(&mut self, dt: f64) -> Result<(), String> {
        if !dt.is_finite() || dt < 0. { return Err("Некорректное время предпросмотра".into()); }
        self.elapsed = (self.elapsed + dt).min(0.25);
        if self.pending.is_some() { return Ok(()); }
        let Some(input) = self.input.clone() else { return Ok(()); };
        if self.sender.is_none() { self.start()?; }
        let cancel = Arc::new(AtomicBool::new(false));
        let request = Request { input, generation: self.generation, elapsed: self.elapsed, cancel: cancel.clone() };
        match self.sender.as_ref().unwrap().try_send(request) {
            Ok(()) => { self.pending = Some(cancel); self.elapsed = 0.; }
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.sender = None; self.receiver = None; return Err("Предпросмотр осколков остановлен; повторите запуск".into());
            }
        } Ok(())
    }
    fn poll(&mut self) -> Result<(), String> {
        loop {
            let Some(receiver) = &self.receiver else { return Ok(()); };
            match receiver.try_recv() {
                Ok(completed) if completed.generation == self.generation => {
                    self.pending = None; self.snapshot = Some(completed.result?);
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.sender = None; self.receiver = None; return Err("Предпросмотр осколков остановлен; повторите запуск".into());
                }
            }
        }
    }
    fn apply(&self, project: &Project, scene: &mut EvaluatedScene) {
        let (Some(input), Some(snapshot)) = (&self.input, &self.snapshot) else { return; };
        let sources: Vec<_> = input.iter().map(|i|i.source).collect();
        if collect_inputs(project, scene, &sources).as_ref() != Ok(input) { return; }
        let original = scene.global.clone();
        for node in &project.nodes {
            let mut cursor = Some(node.id); let mut seen = BTreeSet::new();
            while let Some(id) = cursor {
                if !seen.insert(id) { break; }
                if let Some(matrix) = snapshot.matrices.get(&id) {
                    if let (Some(old), Some(child)) = (original.get(&id), original.get(&node.id)) {
                        scene.global.insert(node.id, *matrix * old.inverse() * *child);
                    }
                    break;
                }
                cursor = project.node(id).and_then(|n| n.parent);
            }
        }
    }
}
impl Drop for Preview { fn drop(&mut self) { if let Some(cancel) = &self.pending { cancel.store(true, Ordering::Relaxed); } } }

impl App {
    pub(super) fn preview_shard_or_child(&self, id: u64) -> bool {
        let mut cursor = Some(id); let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) { break; }
            if self.destruction.preview.affects(id) { return true; }
            cursor = self.editor.project.node(id).and_then(|node| node.parent);
        }
        false
    }
    pub(super) fn toggle_destruction_preview(&mut self, source: u64) {
        if self.destruction.preview.running_for(source) {
            self.destruction.preview.pause(); self.status = "Предпросмотр осколков на паузе.".into(); return;
        }
        match self.start_destruction_previews(&[source]) {
            Ok(()) => {
                self.status = "Предпросмотр осколков без ключей · запись анимации — отдельная команда.".into();
            }
            Err(error) => self.status = error,
        }
    }
    pub(super) fn start_destruction_previews(&mut self, sources: &[u64]) -> Result<(), String> {
        self.editor.playing = false;
        let scene = self.evaluated_interactive().map_err(|e|e.to_string())?;
        let input = collect_inputs(&self.editor.project, &scene, sources)?;
        if self.destruction.preview.input.as_ref() != Some(&input) { self.destruction.preview.reset(); }
        self.destruction.preview.input = Some(input); self.destruction.preview.running = true;
        self.editor.playing = false; self.last_tick = std::time::Instant::now(); Ok(())
    }
    pub(super) fn tick_destruction_preview(&mut self, dt: f64) {
        if let Err(error) = self.destruction.preview.poll() {
            self.destruction.preview.reset(); self.status = error; return;
        }
        let Some(sources) = self.destruction.preview.input.as_ref().map(|inputs| inputs.iter().map(|i|i.source).collect::<Vec<_>>()) else { return; };
        if self.editor.playing { self.destruction.preview.reset(); return; }
        let current = self.evaluated_interactive().map_err(|e| e.to_string())
            .and_then(|scene| collect_inputs(&self.editor.project, &scene, &sources));
        if current.as_ref().ok() != self.destruction.preview.input.as_ref() {
            self.destruction.preview.reset();
            self.status = "Сцена изменилась · запустите предпросмотр осколков заново.".into(); return;
        }
        if self.destruction.preview.running && self.job.is_none() && self.drag.is_none()
            && self.input.is_none() && !self.has_modal() {
            if let Err(error) = self.destruction.preview.advance(dt) { self.destruction.preview.reset(); self.status = error; }
        }
    }
    pub(super) fn evaluated_viewport(&self) -> Result<EvaluatedScene, poseforge_core::CoreError> {
        let mut scene = self.evaluated_interactive()?;
        self.destruction.preview.apply(&self.editor.project, &mut scene);
        Ok(scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::DestructionAction as D;
    use poseforge_core::{box_mesh, Channel, Clip, CollisionTarget, Editor, Interpolation, Keyframe, Node, Property};

    fn scene() -> Project {
        let mut p = Project::default();
        let mut source = Node::new(1, "Source");
        source.mesh = Some(2); source.visible = false; source.render_visible = false;
        source.transform.translation.y = 2.;
        source.destruction = Some(DestructionSettings {
            result: Some(3), values: [2., 1., 0., 0., 9.81, -10., 0., 24.], ..Default::default()
        });
        source.collision_targets.destruction = Some(vec![CollisionTarget { node: 7, enabled: true }]);
        let mut root = Node::new(3, "Fragments"); root.transform.translation.y = 2.;
        let mut shard = Node::new(4, "Shard"); shard.parent = Some(3); shard.mesh = Some(5);
        let mut child = Node::new(6, "Child"); child.parent = Some(4); child.empty = true;
        child.transform.translation.x = 0.1;
        let mut obstacle = Node::new(7, "Platform"); obstacle.mesh = Some(8);
        p.nodes = vec![source, root, shard, child, obstacle];
        p.meshes = vec![box_mesh(2, "Source", Vec3::splat(0.2), None),
            box_mesh(5, "Shard", Vec3::splat(0.2), None),
            box_mesh(8, "Platform", Vec3::new(2., 0.25, 2.), None)];
        let mut clip = Clip::new("Existing keys", 4.);
        clip.channels.push(Channel { node: 1, property: Property::Translation, interpolation: Interpolation::Linear,
            keys: vec![Keyframe::new(0., vec![0., 2., 0.]), Keyframe::new(4., vec![0., 2., 0.])] });
        p.clips.push(clip); p.validate().unwrap(); p
    }
    fn app(project: Project) -> App {
        let mut a = App::new(None); a.editor = Editor::new(project); a.reset_workflow_selection();
        a.preferences = Default::default(); a.width = 1440.; a.height = 900.;
        a.editor.selected = Some(1); a.editor.active_clip = Some(0); a.editor.time = 1.25;
        a
    }
    #[test]
    fn disabled_floor_allows_live_fall_and_dust_edits_keep_the_simulation_input() {
        let mut p=scene();let d=p.node_mut(1).unwrap().destruction.as_mut().unwrap();d.floor_enabled=false;d.mesh_collision=false;
        let eval=p.evaluated(None,0.).unwrap();let input=Input::collect(&p,&eval,1).unwrap();
        let cancel=AtomicBool::new(false);let mut world=World::new(&input,&cancel,&mut Vec::new()).unwrap();
        let mut snapshot=world.advance(0.,&cancel).unwrap();
        for _ in 0..180 {snapshot=world.advance(1./120.,&cancel).unwrap();}
        assert!(snapshot.matrices[&4].w_axis.y< -3.);assert!(snapshot.matrices[&4].is_finite());
        let d=p.node_mut(1).unwrap().destruction.as_mut().unwrap();d.dust_enabled=true;d.dust_density=4.;d.dust_size=2.;d.show_floor=true;d.randomize=true;
        assert!(input==Input::collect(&p,&eval,1).unwrap());
        let mut preview=Preview::default();preview.input=Some(vec![input]);preview.snapshot=Some(snapshot);preview.pause();
        assert!(preview.times()[&1]>1.);preview.reset();assert!(preview.times().is_empty());
    }
    #[test]
    fn live_shards_fall_onto_selected_mesh_and_move_children_without_keys() {
        let project = scene(); let before = project.clone();
        let mut evaluated = project.evaluated(Some(0), 1.25).unwrap();
        let input = Input::collect(&project, &evaluated, 1).unwrap();
        let cancel = AtomicBool::new(false); let mut world = World::new(&input, &cancel, &mut Vec::new()).unwrap();
        let mut snapshot = world.advance(0., &cancel).unwrap();
        for _ in 0..120 { snapshot = world.advance(1. / 120., &cancel).unwrap(); }
        let matrix = snapshot.matrices[&4];
        assert!(matrix.w_axis.y < evaluated.global[&4].w_axis.y - 0.5);
        assert!(project.mesh(5).unwrap().vertices.iter().all(|v|
            matrix.transform_point3(Vec3::from_array(v.position)).y >= 0.249));
        let mut preview = Preview::default(); preview.input = Some(vec![input]); preview.snapshot = Some(snapshot);
        preview.apply(&project, &mut evaluated);
        assert!(evaluated.global[&4].abs_diff_eq(matrix, 1e-6));
        assert!(evaluated.global[&6].w_axis.truncate().distance(matrix.transform_point3(Vec3::X * 0.1)) < 1e-5);
        assert_eq!(project, before);
    }
    #[test]
    fn groups_share_identical_collision_geometry_and_enforce_the_total_shard_budget() {
        let mut project=scene();
        let mut second=project.node(1).unwrap().clone();second.id=10;second.destruction.as_mut().unwrap().result=Some(11);
        let mut root=Node::new(11,"Second group");root.transform.translation=Vec3::new(1.,2.,0.);
        project.nodes.extend([second,root]);
        for id in 20..53 {let mut shard=Node::new(id,"Shard");shard.parent=Some(11);shard.mesh=Some(5);project.nodes.push(shard);}
        let scene=project.evaluated(None,0.).unwrap();let inputs=collect_inputs(&project,&scene,&[1,10]).unwrap();
        let cancel=AtomicBool::new(false);let mut cache=Vec::new();
        let first=World::new(&inputs[0],&cancel,&mut cache).unwrap();
        let second=World::new(&inputs[1],&cancel,&mut cache).unwrap();
        assert_eq!(cache.len(),1);assert!(Arc::ptr_eq(first.collider.as_ref().unwrap(),second.collider.as_ref().unwrap()));
        for id in 60..92 {let mut shard=Node::new(id,"Shard");shard.parent=Some(3);shard.mesh=Some(5);project.nodes.push(shard);}
        let scene=project.evaluated(None,0.).unwrap();
        assert!(Input::collect(&project,&scene,1).is_ok() && Input::collect(&project,&scene,10).is_ok());
        assert!(collect_inputs(&project,&scene,&[1,10]).is_err());
    }
    #[test]
    fn play_pause_reset_preserve_existing_keys_time_mode_and_undo() {
        let mut a = app(scene()); let before = a.editor.project.clone();
        let undo = a.editor.undo_len(); let mode = a.editor.mode;
        a.editor.playing = true; a.destruction_action(D::Play);
        assert!(a.destruction.preview.running_for(1)); assert!(!a.editor.playing);
        a.tick_destruction_preview(1. / 60.);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        // Pausing must still accept the one in-flight result without queueing another step.
        a.destruction_action(D::Play);
        while a.destruction.preview.is_pending() && std::time::Instant::now() < deadline {
            a.tick_destruction_preview(0.);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(!a.destruction.preview.is_pending());
        assert!(a.destruction.preview.time(1).unwrap() > 0.);
        let paused = a.evaluated_viewport().unwrap().global;
        a.tick_destruction_preview(1.); assert_eq!(a.evaluated_viewport().unwrap().global, paused);
        a.destruction_action(D::Play); assert!(a.destruction.preview.running_for(1));
        a.destruction_action(D::Reset); assert!(a.destruction.preview.time(1).is_none());
        assert_eq!(a.evaluated_viewport().unwrap().global, a.editor.evaluated().unwrap().global);
        assert_eq!(a.editor.project, before); assert_eq!(a.editor.undo_len(), undo);
        assert_eq!(a.editor.time, 1.25); assert_eq!(a.editor.active_clip, Some(0)); assert!(a.editor.mode == mode);
    }
    #[test]
    fn reset_rejects_late_worker_results_and_scene_edits_invalidate_preview() {
        let mut a = app(scene()); a.destruction_action(D::Play);
        let generation = a.destruction.preview.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        a.destruction.preview.pending = Some(cancel.clone()); a.destruction.preview.receiver = Some(receiver);
        a.destruction_action(D::Reset); assert!(cancel.load(Ordering::Relaxed));
        assert!(sender.send(Completed { generation, result: Ok(Snapshot { matrices: BTreeMap::new(), time: 99. }) }).is_ok());
        a.destruction.preview.poll().unwrap(); assert!(a.destruction.preview.snapshot.is_none());
        a.destruction_action(D::Play);
        a.editor.project.node_mut(7).unwrap().transform.translation.y = 0.5;
        a.tick_destruction_preview(0.);
        assert!(!a.destruction.preview.running()); assert!(a.destruction.preview.time(1).is_none());
    }
    #[test]
    fn play_builds_missing_fragments_as_one_undo_without_recording_animation() {
        let mut p = scene(); p.nodes.retain(|n| matches!(n.id, 1 | 7));
        p.meshes.retain(|m| m.id != 5);
        let source = p.node_mut(1).unwrap(); source.visible = true; source.render_visible = true;
        source.destruction.as_mut().unwrap().result = None;
        let mut a = app(p); let before = a.editor.project.clone(); let undo = a.editor.undo_len();
        a.destruction_action(D::Play); a.wait_for_background_jobs();
        assert!(a.destruction.preview.running_for(1), "{}", a.status);
        assert!(a.editor.project.node(1).unwrap().destruction.unwrap().result.is_some());
        assert_eq!(a.editor.project.clips, before.clips); assert_eq!(a.editor.active_clip, Some(0));
        assert_eq!(a.editor.time, 1.25); assert_eq!(a.editor.undo_len(), undo + 1);
        a.destruction_action(D::Reset); assert!(a.editor.undo()); assert_eq!(a.editor.project, before);
    }

    #[test]
    fn groups_advance_together_and_mutual_contacts_obey_each_groups_flag() {
        let make=|id,x,v,enabled| {
            let mesh=poseforge_core::box_mesh(1,"Body",Vec3::splat(0.2),None);
            let shape=ShardShape::new(mesh.indices.chunks_exact(3).map(|t|Triangle([
                Vec3::from_array(mesh.vertices[t[0] as usize].position),Vec3::from_array(mesh.vertices[t[1] as usize].position),
                Vec3::from_array(mesh.vertices[t[2] as usize].position)])).collect()).unwrap();
            let motion=ShardMotion {position:Vec3::X*x,rotation:Quat::IDENTITY,velocity:Vec3::X*v,angular_axis:Vec3::Y,angular_speed:0.};
            World {bodies:vec![Body {id,basis:Mat4::IDENTITY,shape,motion,previous:motion}],collider:None,fields:Default::default(),
                forces:ShardForces {gravity:0.,floor:-10.,restitution:0.},shard_collision:enabled,time:0.,backlog:0.}
        };
        for enabled in [false,true] {
            let mut worlds=[make(1,-0.3,3.,true),make(2,0.3,-3.,enabled)];
            let mut snapshot=advance_worlds(&mut worlds,0.05,&AtomicBool::new(false)).unwrap();
            for _ in 0..2 {snapshot=advance_worlds(&mut worlds,0.05,&AtomicBool::new(false)).unwrap();}
            assert_eq!(worlds[0].time,worlds[1].time);assert_eq!(snapshot.matrices.len(),2);
            let separation=worlds[1].bodies[0].motion.position.x-worlds[0].bodies[0].motion.position.x;
            if enabled {assert!(separation>0.);}else{assert!(separation<0.);}
        }
    }
}
