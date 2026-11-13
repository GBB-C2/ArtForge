//! Portable cloth cache stored as ordinary morph targets and a linear animation clip.
use super::*;
use poseforge_core::{Channel, Clip, Interpolation, Keyframe, MorphTarget, Property};
pub(super) fn bake(
    project: &Project,
    id: u64,
    frames: usize,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize),
) -> Result<Project, String> {
    if !(2..=240).contains(&frames) {
        return Err("Запись ткани: 2–240 кадров при 24 кадр/с".into());
    }
    let input = physics_preview::inputs(project, false)
        .into_iter()
        .find(|b| b.id == id)
        .ok_or("Выберите доступную ткань")?;
    if input
        .mesh
        .vertices
        .len()
        .saturating_mul(frames)
        .saturating_mul(24)
        > 256 * 1024 * 1024
    {
        return Err("Кэш ткани превышает 256 МиБ: уменьшите число кадров или сетку".into());
    }
    let collider = scene_colliders::surface(&input.colliders, cancel)?;
    let mut world = input.world()?;
    let mut mesh = input.mesh.clone();
    mesh.id = project.next_id();
    mesh.name = format!("{} · кэш", mesh.name);
    let mut keys = Vec::with_capacity(frames);
    for frame in 0..frames {
        if cancel.load(Ordering::Relaxed) {
            return Err("Запись ткани отменена".into());
        }
        if frame > 0 {
            for _ in 0..5 {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Запись ткани отменена".into());
                }
                world.step_with_mesh(input.config(), &[], &collider);
            }
        }
        let shape = input.snapshot(&world);
        mesh.morph_targets.push(MorphTarget {
            name: format!("Кадр {frame}"),
            positions: shape
                .vertices
                .iter()
                .zip(&input.mesh.vertices)
                .map(|(a, b)| {
                    (Vec3::from_array(a.position) - Vec3::from_array(b.position)).to_array()
                })
                .collect(),
            normals: shape
                .vertices
                .iter()
                .zip(&input.mesh.vertices)
                .map(|(a, b)| (Vec3::from_array(a.normal) - Vec3::from_array(b.normal)).to_array())
                .collect(),
        });
        let mut weights = vec![0.; frames];
        weights[frame] = 1.;
        keys.push(Keyframe::new(frame as f64 / 24., weights));
        progress(frame + 1);
    }
    let mut result = project.clone();
    let node = result.node_mut(id).ok_or("Объект удалён")?;
    node.physics = None;
    node.mesh = Some(mesh.id);
    node.morph_weights = vec![0.; frames];
    node.morph_weights[0] = 1.;
    result.duration = result.duration.max((frames - 1) as f64 / 24.);
    result.meshes.push(mesh);
    result.clips.push(Clip {
        name: "Ткань · 24 кадр/с".into(),
        duration: (frames - 1) as f64 / 24.,
        channels: vec![Channel {
            node: id,
            property: Property::Weights,
            interpolation: Interpolation::Linear,
            keys,
        }],
    });
    result.validate().map_err(|e| e.to_string())?;
    Ok(result)
}
impl App {
    pub(super) fn start_physics_bake(&mut self) {
        let Some(id) = self.editor.selected else {
            return;
        };
        if self.job.is_some() || !self.node_selectable(id) {
            return;
        }
        self.cancel_drag();
        self.editor.playing = false;
        self.studio.physics_running = false;
        let before = self.editor.project.clone();
        let frames = self.studio.bake_frames();
        let (tx, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancellation = cancel.clone();
        std::thread::spawn(move || {
            let result = bake(&before, id, frames, &cancellation, |frame| {
                let _ = tx.send(JobMessage::Progress(format!(
                    "Запись ткани: {frame}/{frames} · Esc — отмена"
                )));
            });
            let msg = match result {
                Ok(after) => JobMessage::PhysicsBaked(Box::new(before), Box::new(after)),
                Err(e) => JobMessage::Error(e),
            };
            let _ = tx.send(msg);
        });
        self.job = Some(Job {
            receiver,
            cancel,
            label: "Запись анимации ткани".into(),
        });
        self.status = "Расчёт ткани с исходного кадра…".into();
    }
    pub(super) fn finish_physics_bake(&mut self, before: Project, after: Project) {
        self.job = None;
        if self.editor.project != before {
            self.status = "Сцена изменилась во время записи; повторите расчёт.".into();
            return;
        }
        let clip = after.clips.len() - 1;
        let r = self.editor.transact("Записать анимацию ткани", |p| {
            *p = after
        });
        if self.result(r).is_some() {
            self.editor.active_clip = Some(clip);
            self.editor.time = 0.;
            self.studio.physics.reset();
            self.invalidate();
            self.status =
                "Анимация ткани записана. Её можно воспроизводить, сохранять и экспортировать."
                    .into();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baked_cloth_uses_its_own_enabled_plain_mesh_targets() {
        let mut p=Project::default();let settings=poseforge_core::PrimitiveSettings::new(poseforge_core::PrimitiveKind::Plane);
        p.meshes.push(settings.mesh(2,None).unwrap());p.meshes.push(poseforge_core::box_mesh(4,"Platform",Vec3::new(3.,0.1,3.),None));
        let mut cloth=poseforge_core::Node::new(1,"Cloth");cloth.mesh=Some(2);cloth.transform.translation.y=2.;cloth.physics=Some(poseforge_core::PhysicsSettings {pinned:false,..Default::default()});
        let mut obstacle=poseforge_core::Node::new(3,"Platform");obstacle.mesh=Some(4);obstacle.transform.translation.y=1.;p.nodes=vec![cloth,obstacle];
        for enabled in [true,false] {
            p.node_mut(1).unwrap().collision_targets.cloth=Some(vec![poseforge_core::CollisionTarget {node:3,enabled}]);
            let before=p.clone();let after=bake(&p,1,48,&AtomicBool::new(false),|_|{}).unwrap();
            let mesh=after.mesh(after.node(1).unwrap().mesh.unwrap()).unwrap();let last=mesh.morph_targets.last().unwrap();
            let bottom=mesh.vertices.iter().zip(&last.positions).map(|(v,d)|v.position[1]+d[1]+2.).fold(f32::INFINITY,f32::min);
            if enabled {assert!(bottom>1.08,"{bottom}");} else {assert!(bottom<0.2,"{bottom}");}
            assert_eq!(p,before);assert_eq!(after.node(3),p.node(3));
        }
    }
    #[test]
    fn bake_replays_vertices_between_frames_and_cancel_preserves_document() {
        let mut p = Project::default();
        let mut mesh = poseforge_core::box_mesh(2, "cloth", Vec3::splat(0.2), None);
        for v in &mut mesh.vertices {
            v.position[1] += 2.;
        }
        p.meshes.push(mesh);
        let mut n = poseforge_core::Node::new(1, "cloth");
        n.mesh = Some(2);
        n.physics = Some(poseforge_core::PhysicsSettings {
            pinned: false,
            ..Default::default()
        });
        p.nodes.push(n);
        let before = p.clone();
        let after = bake(&p, 1, 5, &AtomicBool::new(false), |_| {}).unwrap();
        after.validate().unwrap();
        let mesh = after.mesh(after.node(1).unwrap().mesh.unwrap()).unwrap();
        assert_eq!(mesh.morph_targets.len(), 5);
        assert!(mesh.morph_targets[4].positions[0][1] < -0.05);
        assert_eq!(after.clips.last().unwrap().channels[0].keys[2].value[2], 1.);
        assert!(bake(&p, 1, 5, &AtomicBool::new(true), |_| {}).is_err());
        assert_eq!(p, before);
    }
}
