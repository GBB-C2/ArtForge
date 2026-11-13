//! Fracture tools produce ordinary editable meshes and optional portable animation.
use super::*;
use crate::ui::DestructionAction as D;
use poseforge_core::{Channel, Clip, Interpolation, Keyframe, Node, Property};
use poseforge_simulation::{mesh_contact::Triangle, shard_contact::{solve_shard_contacts, ShardContact, ShardForces, ShardMotion, ShardShape}};

pub(super) const PANEL_BOTTOM: f32 = 1026.;

#[derive(Clone, Copy)]
pub(super) struct Settings {
    values: [f64; 8],
    seam_percent: f64,
    cut_contrast: f64,
    mesh_collision: bool,
    shard_collision: bool,
}
impl Default for Settings {
    fn default() -> Self { Self::from(poseforge_core::DestructionSettings::default()) }
}
impl From<poseforge_core::DestructionSettings> for Settings {
    fn from(s: poseforge_core::DestructionSettings) -> Self {
        Self { values: s.values, seam_percent: s.seam_percent, cut_contrast: s.cut_contrast,
            mesh_collision: s.mesh_collision, shard_collision: s.shard_collision }
    }
}
const FIELDS: [(&str, f64, f64, f64); 10] = [
    ("Осколки", 2., 64., 1.), ("Случайность", 0., 999999., 1.),
    ("Раздвинуть, м", 0., 10., 0.01), ("Импульс, м/с", 0., 30., 0.1),
    ("Гравитация, м/с²", 0., 30., 0.01), ("Пол · Y, м", -100., 100., 0.01),
    ("Отскок", 0., 0.95, 0.01), ("Кадры записи · 24 кадр/с", 2., 240., 1.),
    ("Ширина швов, %", 0., 5., 0.05), ("Контраст разрезов", 0., 1., 0.01),
];
impl Settings {
    pub(super) fn value(self, field: u8) -> Option<f64> {
        match field { 8 => Some(self.seam_percent), 9 => Some(self.cut_contrast), _ => self.values.get(field as usize).copied() }
    }
    fn set(&mut self, field: u8, value: f64) -> Result<(), String> {
        let Some((_, min, max, _)) = FIELDS.get(field as usize) else { return Err("Неизвестный параметр разрушения".into()); };
        if !value.is_finite() || value < *min || value > *max || (matches!(field, 0 | 1 | 7) && value.fract() != 0.) {
            return Err(format!("Допустимо {min}–{max}{}", if matches!(field, 0 | 1 | 7) { ", целое число" } else { "" }));
        }
        match field { 8 => self.seam_percent = value, 9 => self.cut_contrast = value, _ => self.values[field as usize] = value };
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct State { pub settings: Settings, pub preview: super::destruction_preview::Preview }

fn remove_result(project: &Project, root: Option<u64>) -> Result<Project, String> {
    let mut editor = Editor::new(project.clone());
    if let Some(id) = root.filter(|id| project.node(*id).is_some()) {
        editor.delete_node(id).map_err(|e| e.to_string())?;
        let remaining: std::collections::BTreeSet<_> = editor.project.nodes.iter().map(|n| n.id).collect();
        let removed_meshes: std::collections::BTreeSet<_> = project.nodes.iter()
            .filter(|n| !remaining.contains(&n.id)).filter_map(|n| n.mesh).collect();
        let used: std::collections::BTreeSet<_> = editor.project.nodes.iter().filter_map(|n| n.mesh).collect();
        // Rebuilding used to accumulate every old shard mesh and empty animation.
        // Only prune assets from the removed result; shared/user assets survive.
        editor.project.meshes.retain(|m| !removed_meshes.contains(&m.id) || used.contains(&m.id));
        let mut index = 0;
        editor.project.clips.retain(|clip| {
            let keep = !clip.channels.is_empty() || project.clips[index].channels.is_empty();
            index += 1; keep
        });
    }
    Ok(editor.project)
}

/// Detach or clear a generated result on a clone so callers commit one undo step.
pub(super) fn without_destruction(project: &Project, source: u64, remove: bool) -> Result<Project, String> {
    let Some(settings) = project.node(source).and_then(|n| n.destruction) else { return Ok(project.clone()); };
    let mut result = remove_result(project, settings.result)?;
    let node = result.node_mut(source).ok_or("Исходный объект удалён")?;
    node.visible = settings.source_visible; node.render_visible = settings.source_render_visible;
    node.destruction = if remove { None } else { Some(poseforge_core::DestructionSettings { result: None, ..settings }) };
    Ok(result)
}

#[cfg(test)]
pub(super) fn build(project: &Project, source: u64, previous: Option<u64>, settings: Settings, bake: bool, cancel: &AtomicBool) -> Result<(Project, u64), String> {
    build_evaluated(project, source, previous, settings, bake, cancel, None)
}

fn build_evaluated(project: &Project, source: u64, previous: Option<u64>, settings: Settings, bake: bool,
    cancel: &AtomicBool, evaluated: Option<&poseforge_core::EvaluatedScene>) -> Result<(Project, u64), String> {
    for (i, v) in settings.values.iter().enumerate() { let mut check = settings; check.set(i as u8, *v)?; }
    poseforge_core::DestructionSettings { values: settings.values, seam_percent: settings.seam_percent,
        cut_contrast: settings.cut_contrast, ..Default::default() }.validate().map_err(|e| e.to_string())?;
    let node = project.node(source).ok_or("Выберите меш")?;
    if project.node_locked(source) || node.fluid.is_some() || node.physics.is_some()
        || node.camera.is_some() || node.is_joint || project.nodes.iter().any(|n| n.parent == Some(source)) {
        return Err("Выберите отдельный меш без жидкости, ткани и дочерних объектов".into());
    }
    let mesh = node.mesh.and_then(|id| project.mesh(id)).ok_or("Выберите объёмный меш")?;
    let globals = match evaluated { Some(scene) => scene.global.clone(), None => project.global_transforms().map_err(|e| e.to_string())? };
    let fields=scene_fields::collect(project,&globals,false,2,Mat4::IDENTITY);
    let colliders = if bake && settings.mesh_collision {
        let inputs = scene_colliders::inputs(project, &globals, false, Some(source), Mat4::IDENTITY, evaluated, node.collision_targets.destruction.as_deref());
        if inputs.is_empty() { None } else { Some(scene_colliders::surface(&inputs, cancel)?) }
    } else { None };
    let global = globals[&source];
    if !global.is_finite() || global.determinant().abs() < 1e-8 { return Err("Нулевой масштаб меша".into()); }
    // Fracture the current evaluated surface while retaining the rigged source for Undo/restore.
    let mut snapshot = mesh.clone();
    let weights = evaluated.and_then(|scene| scene.morph_weights.get(&source)).unwrap_or(&node.morph_weights);
    let palette: Vec<_> = node.skin.iter().flat_map(|skin| skin.joints.iter().zip(&skin.inverse_bind_matrices)
        .map(|(joint, bind)| global.inverse() * globals.get(joint).copied().unwrap_or(Mat4::IDENTITY) * *bind)).collect();
    for (index, vertex) in snapshot.vertices.iter_mut().enumerate() {
        if index % 256 == 0 && cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
        let mut p = Vec3::from_array(vertex.position); let mut n = Vec3::from_array(vertex.normal);
        for (target, weight) in mesh.morph_targets.iter().zip(weights) {
            if let Some(delta) = target.positions.get(index) { p += Vec3::from_array(*delta) * *weight; }
            if let Some(delta) = target.normals.get(index) { n += Vec3::from_array(*delta) * *weight; }
        }
        let mut skin = Mat4::IDENTITY;
        if !palette.is_empty() && vertex.weights.iter().sum::<f32>() > 0.0001 {
            skin = Mat4::ZERO;
            for (&joint, &weight) in vertex.joints.iter().zip(&vertex.weights) {
                skin += palette.get(joint as usize).copied().unwrap_or(Mat4::IDENTITY) * weight;
            }
        }
        vertex.position = skin.transform_point3(p).to_array();
        vertex.normal = if skin.determinant().abs() > 1e-8 { skin.inverse().transpose().transform_vector3(n).normalize_or_zero().to_array() } else { n.to_array() };
        vertex.weights = [0.; 4]; vertex.joints = [0; 4];
    }
    snapshot.morph_targets.clear();
    if let Some(t) = &mut snapshot.topology { t.primitive = None; }
    let mesh = &snapshot;
    let mut result = remove_result(project, previous)?;
    let interior = super::destruction_materials::prepare(&mut result, source)?;
    let exterior_layers = result.node(source).unwrap().material_layers.clone();
    let fragments = poseforge_core::fracture_mesh_with_material(mesh, settings.values[0] as usize,
        settings.values[1] as u32, settings.cut_contrast as f32, Some(interior), cancel)?;
    let (lo, hi) = fluid_domains::mesh_bounds(mesh);
    let center = global.transform_point3((lo + hi) * 0.5);
    let normals = global.inverse().transpose();
    let root_id = result.next_id().max(project.next_id());
    let mut group = Node::new(root_id, format!("{} · разрушение", node.name));
    group.transform.translation = center;
    result.nodes.push(group);
    let mut bodies = Vec::new();
    for (index, mut fragment) in fragments.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
        let origin = global.transform_point3(fragment.center);
        let direction = (origin - center).try_normalize().unwrap_or(Vec3::Y);
        for vertex in &mut fragment.mesh.vertices {
            vertex.position = global.transform_vector3(Vec3::from_array(vertex.position) * (1. - settings.seam_percent as f32 * 0.01)).to_array();
            vertex.normal = normals.transform_vector3(Vec3::from_array(vertex.normal)).normalize_or_zero().to_array();
        }
        if global.determinant() < 0. {
            if let Some(topology) = &mut fragment.mesh.topology { for face in &mut topology.faces { face.reverse(); } }
            fragment.mesh.rebuild_polygon_indices().map_err(|e| e.to_string())?;
        }
        let mesh_id = result.next_id();
        fragment.mesh.id = mesh_id;
        let mut shard = Node::new(mesh_id + 1, format!("Осколок {:02}", index + 1));
        shard.parent = Some(root_id); shard.mesh = Some(mesh_id);
        shard.material_layers = exterior_layers.clone();
        let position = origin + direction * settings.values[2] as f32;
        shard.transform.translation = position - center;
        if bake {
            let seed_angle = (index as f32 * 2.399963 + settings.values[1] as f32 * 0.1).sin();
            let axis = Vec3::new(direction.z + 0.3, seed_angle, -direction.x + 0.2).try_normalize().unwrap_or(Vec3::Y);
            let triangles = fragment.mesh.indices.chunks_exact(3).map(|t| Triangle([
                Vec3::from_array(fragment.mesh.vertices[t[0] as usize].position),
                Vec3::from_array(fragment.mesh.vertices[t[1] as usize].position),
                Vec3::from_array(fragment.mesh.vertices[t[2] as usize].position),
            ])).collect();
            let shape = ShardShape::new(triangles)?;
            let motion = ShardMotion { position, rotation: Quat::IDENTITY,
                velocity: (direction + Vec3::Y * 0.35) * settings.values[3] as f32,
                angular_axis: axis, angular_speed: settings.values[3] as f32 * 0.5 };
            bodies.push((shard.id,shape,motion));
        }
        result.meshes.push(fragment.mesh); result.nodes.push(shard);
    }
    let component = node.destruction.unwrap_or(poseforge_core::DestructionSettings {
        source_visible: node.visible, source_render_visible: node.render_visible, ..Default::default()
    });
    let original = result.node_mut(source).ok_or("Исходник удалён")?;
    original.destruction = Some(poseforge_core::DestructionSettings { values: settings.values, seam_percent: settings.seam_percent, cut_contrast: settings.cut_contrast,
        mesh_collision: settings.mesh_collision, shard_collision: settings.shard_collision,
        interior_material: Some(interior), result: Some(root_id), visible: true, ..component });
    original.visible = false; original.render_visible = false;
    if bake {
        let channels=bake_motion(&mut bodies,settings,&fields,colliders.as_ref(),center,cancel)?;
        let duration = (settings.values[7] - 1.) / 24.;
        result.duration = result.duration.max(duration);
        result.clips.push(Clip { name: format!("{} · падение осколков", node.name), duration, channels });
    }
    result.clean_outliner();
    result.validate().map_err(|e| e.to_string())?;
    Ok((result, root_id))
}

fn bake_motion(bodies:&mut[(u64,ShardShape,ShardMotion)],settings:Settings,
    fields:&poseforge_simulation::fields::Fields,collider:Option<&poseforge_simulation::mesh_contact::MeshCollider>,
    center:Vec3,cancel:&AtomicBool)->Result<Vec<Channel>,String> {
    let forces=ShardForces {gravity:settings.values[4] as f32,floor:settings.values[5] as f32,restitution:settings.values[6] as f32};
    let mut keys=vec![(Vec::new(),Vec::new());bodies.len()];
    for frame in 0..settings.values[7] as usize {
        if cancel.load(Ordering::Relaxed) {return Err("Разрушение отменено".into());}
        for _ in 0..(if frame==0 {0}else{5}) {
            let previous:Vec<_>=bodies.iter().map(|b|b.2).collect();
            for (_,shape,motion) in bodies.iter_mut() {
                motion.velocity=(motion.velocity+fields.acceleration(motion.position,motion.velocity)/120.).clamp_length_max(100.);
                shape.step(motion,forces,collider,1./120.,cancel)?;
            }
            if settings.shard_collision {
                let mut contacts:Vec<_>=bodies.iter_mut().zip(previous.iter().copied()).map(|((_,shape,motion),previous)|
                    ShardContact {shape,motion,previous,enabled:true,restitution:forces.restitution,floor:forces.floor}).collect();
                solve_shard_contacts(&mut contacts,cancel)?;
                drop(contacts);
                for ((_,shape,motion),previous) in bodies.iter_mut().zip(previous) {
                    shape.resolve_environment(motion,previous,forces,collider,cancel)?;
                }
            }
        }
        for ((_,_,motion),(translation,rotation)) in bodies.iter().zip(&mut keys) {
            translation.push(Keyframe::new(frame as f64/24.,(motion.position-center).to_array().to_vec()));
            rotation.push(Keyframe::new(frame as f64/24.,motion.rotation.to_array().to_vec()));
        }
    }
    let mut channels=Vec::new();
    for ((node,_,_),(translation,rotation)) in bodies.iter().zip(keys) {
        for (property,keys) in [(Property::Translation,translation),(Property::Rotation,rotation)] {
            channels.push(Channel {node:*node,property,interpolation:Interpolation::Linear,keys});
        }
    }
    Ok(channels)
}

impl App {
    pub(super) fn destruction_source(&self) -> Option<u64> {
        let selected = self.editor.selected?;
        let mut cursor = Some(selected); let mut visited = BTreeSet::new();
        while let Some(id) = cursor {
            if !visited.insert(id) { break; }
            if let Some(owner) = self.editor.project.nodes.iter().find(|n| n.destruction.is_some_and(|d| d.result == Some(id))) { return Some(owner.id); }
            cursor = self.editor.project.node(id).and_then(|n| n.parent);
        }
        self.editor.project.node(selected).filter(|n| n.mesh.is_some()).map(|n| n.id)
    }
    pub(super) fn destruction_settings(&self) -> Settings {
        self.destruction_source().and_then(|id| self.editor.project.node(id)).and_then(|n| n.destruction)
            .map_or(self.destruction.settings, Settings::from)
    }
    pub(super) fn destruction_confirm(&mut self, action: &Action, text: &str) -> Result<bool, String> {
        let Action::Destruction(D::Field(field)) = action else { return Ok(false); };
        let source = self.destruction_source().ok_or("Выберите меш с разрушением")?;
        if self.editor.project.node_locked(source) { return Err("Исходный объект заблокирован".into()); }
        let mut component = self.editor.project.node(source).and_then(|n| n.destruction).ok_or("Добавьте разрушение мешу")?;
        let mut settings = Settings::from(component);
        let value = text.trim().replace(',', ".").parse::<f64>().map_err(|_| "Введите число")?;
        settings.set(*field, value)?; component.values = settings.values;
        component.seam_percent = settings.seam_percent; component.cut_contrast = settings.cut_contrast;
        self.editor.transact("Параметр разрушения", |p| p.node_mut(source).unwrap().destruction = Some(component)).map_err(|e| e.to_string())?;
        self.update_dirty(); Ok(true)
    }
    pub(super) fn destruction_action(&mut self, action: D) {
        if self.job.is_some() || self.native_picker.is_some() || self.has_modal() || self.editor.has_staged() { return; }
        let Some(source) = self.destruction_source() else { self.status = "Выберите объёмный меш.".into(); return; };
        if action == D::Reset {
            if self.destruction.preview.time(source).is_some() { self.destruction.preview.reset(); }
            self.status = "Предпросмотр осколков сброшен · ключи анимации сохранены.".into(); self.redraw(); return;
        }
        if self.editor.project.node_locked(source) { self.status = "Исходный объект заблокирован.".into(); return; }
        if action == D::Play && self.editor.project.node(source).and_then(|n| n.destruction).and_then(|d| d.result).is_some() {
            self.toggle_destruction_preview(source); self.redraw(); return;
        }
        match action {
            D::Reset => {}
            D::MeshCollision | D::ShardCollision => {
                let result = self.editor.transact("Столкновения осколков", |p| {
                    if let Some(component) = p.node_mut(source).and_then(|n| n.destruction.as_mut()) {
                        if action == D::ShardCollision { component.shard_collision = !component.shard_collision; }
                        else { component.mesh_collision = !component.mesh_collision; }
                    }
                });
                if self.result(result).is_some() {
                    self.status = "Столкновения изменены. Запустите предпросмотр или перезапишите анимацию.".into();
                }
            }
            D::Field(field) => {
                if let Some(value) = self.destruction_settings().value(field) { self.input(Action::Destruction(action), value.to_string()); }
            }
            D::Material => { self.action(Action::Select(source)); self.surface.material_library = false; self.action(Action::SurfaceTab(1)); }
            D::Reseed => {
                let next = (self.destruction_settings().values[1] + 1.) % 1000000.;
                if let Err(error) = self.destruction_confirm(&Action::Destruction(D::Field(1)), &next.to_string()) { self.status = error; }
            }
            D::Restore | D::Remove => {
                let after = match without_destruction(&self.editor.project, source, action == D::Remove) {
                    Ok(after) => after, Err(error) => { self.status = error; return; }
                };
                let result = self.editor.transact("Вернуть исходный объект", |p| *p = after);
                if self.result(result).is_some() {
                    self.editor.active_clip = None; self.editor.playing = false;
                    self.action(Action::Select(source)); self.set_properties_tab(13); self.invalidate_geometry();
                }
            }
            D::Attach | D::Create | D::Bake | D::Play => {
                self.cancel_drag(); self.cancel_modeling_live();
                self.destruction.preview.reset();
                let evaluated = match self.evaluated_interactive() { Ok(scene) => scene, Err(error) => { self.status = error.to_string(); return; } };
                self.editor.playing = false;
                let previous = self.editor.project.node(source).and_then(|n| n.destruction).and_then(|d| d.result);
                let before = self.editor.project.clone(); let settings = self.destruction_settings();
                let bake = action == D::Bake; let preview = action == D::Play;
                let cancel = Arc::new(AtomicBool::new(false)); let c = cancel.clone();
                let (tx, receiver) = mpsc::channel();
                let spawned = std::thread::Builder::new().name("fracture-build".into()).spawn(move || {
                    let mut input = before.clone();
                    if action == D::Attach {
                        let node = input.node_mut(source).unwrap(); node.fluid = None; node.physics = None;
                        node.collision_targets.destruction = Some(Vec::new());
                    }
                    let result = build_evaluated(&input, source, previous, settings, bake, &c, Some(&evaluated));
                    let message = match result {
                        Ok((after, root)) => JobMessage::DestructionReady(Box::new(before), Box::new(after), source, root, bake, preview),
                        Err(error) => JobMessage::Error(error),
                    };
                    let _ = tx.send(message);
                });
                match spawned {
                    Ok(_) => {
                        self.job = Some(Job { receiver, cancel, label: "Разрушение".into() });
                        self.status = "Создание закрытых осколков…".into();
                    }
                    Err(error) => { self.status = format!("Не удалось запустить разрушение: {error}"); }
                }
            }
        }
        self.redraw();
    }
    pub(super) fn finish_destruction(&mut self, before: Project, after: Project, source: u64, root: u64, bake: bool, preview: bool) {
        self.job = None;
        if self.editor.project != before { self.status = "Сцена изменилась; повторите разрушение.".into(); return; }
        let clip = if bake { Some(after.clips.len().saturating_sub(1)) } else {
            self.editor.active_clip.and_then(|i| before.clips.get(i)).and_then(|old| after.clips.iter().position(|c| c == old))
        };
        let result = self.editor.transact("Разрушение примитива", |p| *p = after);
        if self.result(result).is_some() {
            self.authoring.editing = None; self.editor.active_clip = clip;
            self.action(Action::Select(source)); self.set_properties_tab(13);
            self.scene_tools.material_expanded.insert(source); self.collapsed.remove(&root);
            self.studio.physics_running = false; self.studio.physics.reset(); self.fluid_domains.reset(source);
            if bake { self.editor.time = 0.; self.editor.playing = false; self.toggle_animation_playback(); }
            self.invalidate_geometry();
            self.status = if bake { "Разрушение назначено примитиву; воспроизводится падение осколков." }
                else { "Разрушение рядом с материалом. Глаз — исходная форма / осколки; крестик — снять." }.into();
            if preview { self.toggle_destruction_preview(source); }
        }
    }
    pub(super) fn draw_destruction(&self, u: &mut Ui, rx: f32, rw: f32) {
        let x = rx + 16.; let w = rw - 32.; let p = u.palette();
        let source = self.destruction_source();
        let component = source.and_then(|id| self.editor.project.node(id)).and_then(|n| n.destruction);
        let Some(component) = component else {
            u.fitted_label([x, 174., w, 20.], "Разрушение назначается мешу", 12., p.text);
            u.fitted_label([x, 204., w, 20.], "Форма меша задаёт объём осколков.", 11., p.muted);
            if source.is_some() { u.icon_button([x, 238., w, 32.], "Добавить разрушение", Action::Destruction(D::Attach), false, Icon::Fracture); }
            else { u.disabled_icon_button([x, 238., w, 32.], "Добавить разрушение", Icon::Fracture); }
            u.fitted_label([x, 288., w, 20.], "Замкнутый меш · в том числе вогнутый.", 11., p.muted);
            return;
        };
        let caption = source.and_then(|id|self.destruction.preview.time(id)).map_or_else(||"Разрушение примитива".into(),|t|format!("Предпросмотр · {t:.2} с · без ключей"));
        u.fitted_label([x, 174., w, 20.], &caption, 12., p.text);
        let shift = self.collision_layout_shift(poseforge_core::CollisionKind::Destruction) + 34.;
        u.icon_button([x, 202., w, 28.], "Материалы осколков", Action::Destruction(D::Material), false, Icon::Material);
        let playing = source.is_some_and(|id| self.destruction.preview.running_for(id));
        let half = (w - 8.) * 0.5;
        if self.job.is_none() {
            u.icon_button([x,244.,half,32.],if playing {"Пауза"}else{"Запустить"},Action::Destruction(D::Play),playing,if playing {Icon::Pause}else{Icon::Play});
            u.icon_button([x+half+8.,244.,half,32.],"В начало",Action::Destruction(D::Reset),false,Icon::Start);
        } else {
            u.disabled_icon_button([x,244.,half,32.],"Запустить",Icon::Play);
            u.disabled_icon_button([x+half+8.,244.,half,32.],"В начало",Icon::Start);
        }
        for (i, (label, min, max, step)) in FIELDS.iter().enumerate() {
            u.number_field([x, 288. + i as f32 * 36., w, 30.], label, Settings::from(component).value(i as u8).unwrap(),
                Action::Destruction(D::Field(i as u8)), SliderRange::new(*min, *max, *step));
        }
        u.checkbox([x, 654., w, 30.], "Столкновения с мешами", Action::Destruction(D::MeshCollision), component.mesh_collision);
        u.checkbox([x, 688., w, 30.], "Столкновения осколков между собой", Action::Destruction(D::ShardCollision), component.shard_collision);
        self.draw_collision_list(u, poseforge_core::CollisionKind::Destruction, x, 722., w);
        u.button([x, 720. + shift, w, 28.], "Новый вариант случайности", Action::Destruction(D::Reseed), false);
        for (i, (label, action, icon)) in [("Пересоздать осколки", D::Create, Icon::Fracture), ("Записать ключи анимации", D::Bake, Icon::Sequence)].into_iter().enumerate() {
            let r = [x, 760. + shift + i as f32 * 40., w, 32.];
            if self.job.is_none() { u.icon_button(r, label, Action::Destruction(action), false, icon); }
            else { u.disabled_icon_button(r, label, icon); }
        }
        if component.result.is_some() { u.button([x, 842. + shift, w, 28.], "Очистить осколки · оставить настройки", Action::Destruction(D::Restore), false); }
        u.button([x, 882. + shift, w, 28.], "Убрать разрушение с примитива", Action::Destruction(D::Remove), false);
        u.fitted_label([x, 926. + shift, w, 18.], "Запуск и пауза не создают ключей.", 10., p.muted);
        u.fitted_label([x, 946. + shift, w, 18.], "Внешний материал и материал срезов — отдельно.", 10., p.muted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> Project {
        let mut p = Project::default();
        let mut n = Node::new(1, "Source"); n.mesh = Some(2); n.transform.translation.y = 2.;
        p.nodes.push(n); p.meshes.push(poseforge_core::box_mesh(2, "Cube", Vec3::splat(0.5), None)); Editor::new(p).project
    }
    #[test]
    fn fracture_and_bake_keep_source_and_serialize_as_normal_scene_data() {
        let before = scene();
        let settings = Settings { values: [4., 9., 0., 1.5, 9.81, 0., 0.2, 48.], ..Default::default() };
        let (after, root) = build(&before, 1, None, settings, true, &AtomicBool::new(false)).unwrap();
        after.validate().unwrap();
        assert_eq!(after.mesh(2), before.mesh(2));
        assert!(!after.node(1).unwrap().visible && !after.node(1).unwrap().render_visible);
        let fragments: Vec<_> = after.nodes.iter().filter(|n| n.parent == Some(root)).collect();
        assert_eq!(fragments.len(), 4);
        assert_eq!(after.clips.last().unwrap().channels.len(), 8);
        let restored: Project = serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap();
        assert_eq!(restored, after);
        for frame in 1..48 {
            let eval = after.evaluated(Some(after.clips.len() - 1), frame as f64 / 24.).unwrap();
            for n in &fragments {
                let mesh = after.mesh(n.mesh.unwrap()).unwrap();
                assert!(mesh.vertices.iter().all(|v| eval.global[&n.id].transform_point3(Vec3::from_array(v.position)).y >= -0.0001));
            }
        }
        assert!(build(&before, 1, None, settings, true, &AtomicBool::new(true)).is_err());
        assert!(before.node(1).unwrap().visible);
    }
    #[test]
    fn rebuilding_replaces_the_previous_result_and_one_undo_restores_it() {
        let before = scene();
        let settings = Settings { values: [4., 2., 0., 1.5, 9.81, 0., 0.2, 12.], ..Default::default() };
        let (first, root) = build(&before, 1, None, settings, false, &AtomicBool::new(false)).unwrap();
        let (second, next) = build(&first, 1, Some(root), settings, false, &AtomicBool::new(false)).unwrap();
        assert!(second.node(root).is_none());
        assert_eq!(second.nodes.iter().filter(|n| n.parent == Some(next)).count(), 4);
        let mut editor = Editor::new(first);
        let first = editor.project.clone();
        editor.transact("Разрушение", |p| *p = second).unwrap();
        editor.undo(); assert_eq!(editor.project, first);
    }

    #[test]
    fn fracture_bake_uses_scene_meshes_and_keeps_collision_switch_in_component() {
        let mut before = scene();
        let mut obstacle = Node::new(10, "Platform"); obstacle.mesh = Some(11);
        obstacle.transform.translation.y = 0.9; obstacle.rest_transform=Some(obstacle.transform);
        obstacle.physics = Some(poseforge_core::PhysicsSettings { kind: poseforge_core::PhysicsKind::Collider, ..Default::default() });
        before.nodes.push(obstacle);
        before.meshes.push(poseforge_core::box_mesh(11, "Platform", Vec3::new(3.,0.1,3.), None));
        let mut settings = Settings { values: [2.,1.,0.,0.,9.81,-2.,0.,72.], ..Default::default() };
        for enabled in [true, false] {
            settings.mesh_collision = enabled;
            let (after, root) = build(&before, 1, None, settings, true, &AtomicBool::new(false)).unwrap();
            assert_eq!(after.node(1).unwrap().destruction.unwrap().mesh_collision, enabled);
            let evaluated = after.evaluated(Some(after.clips.len()-1), 71./24.).unwrap();
            let bottom = after.nodes.iter().filter(|node| node.parent == Some(root)).flat_map(|node| {
                let model = evaluated.global[&node.id];
                after.mesh(node.mesh.unwrap()).unwrap().vertices.iter().map(move |v| model.transform_point3(Vec3::from_array(v.position)).y)
            }).fold(f32::INFINITY, f32::min);
            if enabled { assert!(bottom >= 0.999 && bottom < 1.01, "{bottom}"); }
            else { assert!(bottom < -1.9, "{bottom}"); }
            assert_eq!(after.node(10), before.node(10));
        }
    }
    #[test]
    fn fracture_bake_obeys_enabled_entries_for_plain_meshes() {
        let mut before=scene();let mut platform=Node::new(10,"Plain platform");platform.mesh=Some(11);platform.transform.translation.y=0.9;platform.rest_transform=Some(platform.transform);
        before.nodes.push(platform);before.meshes.push(poseforge_core::box_mesh(11,"Platform",Vec3::new(3.,0.1,3.),None));
        let settings=Settings {values:[2.,1.,0.,0.,9.81,-2.,0.,72.],..Default::default()};
        for enabled in [true,false] {
            before.node_mut(1).unwrap().collision_targets.destruction=Some(vec![poseforge_core::CollisionTarget {node:10,enabled}]);
            let (after,root)=build(&before,1,None,settings,true,&AtomicBool::new(false)).unwrap();
            let evaluated=after.evaluated(Some(after.clips.len()-1),71./24.).unwrap();
            let bottom=after.nodes.iter().filter(|n|n.parent==Some(root)).flat_map(|node| {
                let model=evaluated.global[&node.id];after.mesh(node.mesh.unwrap()).unwrap().vertices.iter()
                    .map(move |v|model.transform_point3(Vec3::from_array(v.position)).y)
            }).fold(f32::INFINITY,f32::min);
            if enabled {assert!(bottom>=0.999&&bottom<1.01,"{bottom}");} else {assert!(bottom< -1.9,"{bottom}");}
            assert_eq!(after.node(10),before.node(10));
            assert_eq!(after.node(1).unwrap().collision_targets,before.node(1).unwrap().collision_targets);
        }
    }

    #[test]
    fn collision_toggle_preserves_generated_meshes_and_is_one_undo() {
        let mut app = App::new(None);
        let (project, _) = build(&scene(), 1, None, Settings { values: [2.,1.,0.,0.,9.81,0.,0.,12.], ..Default::default() }, false, &AtomicBool::new(false)).unwrap();
        app.editor = Editor::new(project); app.reset_workflow_selection(); app.editor.selected = Some(1);
        let before = app.editor.project.clone(); let history = app.editor.undo_len();
        app.destruction_action(D::MeshCollision);
        assert!(!app.editor.project.node(1).unwrap().destruction.unwrap().mesh_collision);
        assert_eq!(app.editor.project.meshes, before.meshes);
        assert_eq!(app.editor.undo_len(), history + 1);
        app.editor.undo(); assert_eq!(app.editor.project, before);
    }

    #[test]
    fn rebuilding_does_not_accumulate_old_meshes_or_empty_bake_clips() {
        let before = scene(); let mut settings = Settings::default(); settings.values[0] = 4.; settings.values[7] = 4.;
        let (mut project, mut root) = build(&before, 1, None, settings, true, &AtomicBool::new(false)).unwrap();
        let meshes = project.meshes.len(); let clips = project.clips.len();
        for _ in 0..4 {
            let old = root;
            (project, root) = build(&project, 1, Some(root), settings, true, &AtomicBool::new(false)).unwrap();
            assert!(project.node(old).is_none());
            assert_eq!(project.meshes.len(), meshes); assert_eq!(project.clips.len(), clips);
            project.validate().unwrap();
        }
        let restored = without_destruction(&project, 1, true).unwrap();
        assert_eq!(restored.meshes, before.meshes); assert_eq!(restored.clips, before.clips);
    }
    #[test]
    fn deleting_generated_result_keeps_a_mesh_used_by_an_external_instance() {
        let before = scene(); let mut settings = Settings::default(); settings.values[0] = 2.;
        let (mut p, root) = build(&before, 1, None, settings, false, &AtomicBool::new(false)).unwrap();
        let mesh = p.nodes.iter().find(|n| n.parent == Some(root)).unwrap().mesh.unwrap();
        let mut instance = Node::new(p.next_id(), "Keep instance"); instance.mesh = Some(mesh); p.nodes.push(instance);
        let restored = without_destruction(&p, 1, true).unwrap();
        assert!(restored.mesh(mesh).is_some()); restored.validate().unwrap();
    }
    #[test]
    fn seam_width_scales_shards_without_moving_their_origins_or_source() {
        let before = scene(); let mut settings = Settings::default(); settings.values[0] = 2.; settings.seam_percent = 0.;
        let (closed, root) = build(&before, 1, None, settings, false, &AtomicBool::new(false)).unwrap();
        settings.seam_percent = 2.;
        let (open, root2) = build(&before, 1, None, settings, false, &AtomicBool::new(false)).unwrap();
        for (a,b) in closed.nodes.iter().filter(|n| n.parent == Some(root)).zip(open.nodes.iter().filter(|n| n.parent == Some(root2))) {
            assert_eq!(a.transform, b.transform);
            for (x,y) in closed.mesh(a.mesh.unwrap()).unwrap().vertices.iter().zip(&open.mesh(b.mesh.unwrap()).unwrap().vertices) {
                assert!((Vec3::from_array(x.position) * 0.98).distance(Vec3::from_array(y.position)) < 1e-6);
            }
        }
        assert_eq!(open.mesh(2), before.mesh(2));
        let saved: Project = serde_json::from_str(&serde_json::to_string(&open).unwrap()).unwrap();
        assert_eq!(saved.node(1).unwrap().destruction.unwrap().seam_percent, 2.);
    }

    #[test]
    fn wind_changes_baked_shard_motion_and_disabled_target_restores_it() {
        let mut p=scene();let settings=Settings{values:[4.,17.,0.04,0.,0.,-100.,0.25,12.],mesh_collision:false,..Default::default()};
        let id=p.next_id();let mut field=Node::new(id,"Wind");field.empty=true;field.force_field=Some(poseforge_core::ForceFieldSettings{kind:poseforge_core::ForceFieldKind::Wind,radius:100.,falloff:0.,..Default::default()});p.nodes.push(field);
        let enabled=build(&p,1,None,settings,true,&AtomicBool::new(false)).unwrap().0;
        p.node_mut(id).unwrap().force_field.as_mut().unwrap().targets[2]=false;
        let disabled=build(&p,1,None,settings,true,&AtomicBool::new(false)).unwrap().0;
        let last_z=|p:&Project|p.clips.last().unwrap().channels.iter().find(|c|c.property==Property::Translation).unwrap().keys.last().unwrap().value[2];
        assert!(last_z(&enabled)>last_z(&disabled)+0.1);
    }


    #[test]
    fn fracture_adds_two_slots_without_changing_shared_source_and_reuses_interior() {
        let mut before=scene();let mut instance=Node::new(20,"Other instance");instance.mesh=Some(2);instance.rest_transform=Some(instance.transform);before.nodes.push(instance);
        let settings=Settings {values:[4.,2.,0.,0.,9.81,0.,0.,4.],..Default::default()};
        let (first,root)=build(&before,1,None,settings,false,&AtomicBool::new(false)).unwrap();
        let source=first.node(1).unwrap();let interior=source.destruction.unwrap().interior_material.unwrap();
        let exterior=source.material_layers[0].material;assert_ne!(interior,exterior);
        assert_eq!(first.mesh(2),before.mesh(2));assert_eq!(first.node(20),before.node(20));
        for node in first.nodes.iter().filter(|n|n.parent==Some(root)) {
            assert_eq!(node.material_layers,source.material_layers);
            assert!(first.mesh(node.mesh.unwrap()).unwrap().assigned_materials().contains(&interior));
        }
        let (second,_)=build(&first,1,Some(root),settings,false,&AtomicBool::new(false)).unwrap();
        assert_eq!(second.materials,first.materials);assert_eq!(second.node(1).unwrap().destruction.unwrap().interior_material,Some(interior));
        let mut app=App::new(None);app.editor=Editor::new(first);app.reset_workflow_selection();app.editor.selected=Some(1);
        assert_eq!(app.scene_material_ids(1),vec![exterior,interior]);
        let snapshot=app.editor.project.clone();
        app.action(Action::MaterialLayerVisible(1,exterior));
        assert!(app.editor.project.nodes.iter().filter(|n|n.parent==Some(root)).all(|n|!n.material_layers[0].visible));
        app.editor.undo();assert_eq!(app.editor.project,snapshot);
        assert!(app.detach_destruction_material(1,interior));
        assert!(app.editor.project.node(1).unwrap().destruction.unwrap().interior_material.is_none());
        app.editor.undo();assert_eq!(app.editor.project,snapshot);
    }
    #[test]
    fn mutual_collision_checkbox_is_persistent_and_undoable_without_rebuilding_meshes() {
        let (project,_)=build(&scene(),1,None,Settings {values:[2.,1.,0.,0.,9.81,0.,0.,4.],..Default::default()},false,&AtomicBool::new(false)).unwrap();
        let mut app=App::new(None);app.editor=Editor::new(project);app.reset_workflow_selection();app.editor.selected=Some(1);
        let before=app.editor.project.clone();app.destruction_action(D::ShardCollision);
        assert!(app.editor.project.node(1).unwrap().destruction.unwrap().shard_collision);
        assert_eq!(app.editor.project.meshes,before.meshes);
        app.editor.undo();assert_eq!(app.editor.project,before);
    }
    #[test]
    fn mutual_contacts_are_used_in_animation_bake() {
        let settings=Settings {values:[2.,1.,0.,0.,0.,-10.,0.,4.],shard_collision:true,..Default::default()};
        let shape=|| {
            let mesh=poseforge_core::box_mesh(1,"Body",Vec3::splat(0.2),None);
            ShardShape::new(mesh.indices.chunks_exact(3).map(|t|Triangle([
                Vec3::from_array(mesh.vertices[t[0] as usize].position),
                Vec3::from_array(mesh.vertices[t[1] as usize].position),
                Vec3::from_array(mesh.vertices[t[2] as usize].position),
            ])).collect()).unwrap()
        };
        let motion=|x,v|ShardMotion {position:Vec3::X*x,rotation:Quat::IDENTITY,velocity:Vec3::X*v,angular_axis:Vec3::Y,angular_speed:0.};
        let mut bodies=vec![(1,shape(),motion(-0.3,3.)),(2,shape(),motion(0.3,-3.))];
        let channels=bake_motion(&mut bodies,settings,&Default::default(),None,Vec3::ZERO,&AtomicBool::new(false)).unwrap();
        assert_eq!(channels.len(),4);assert!(bodies[0].2.position.x<bodies[1].2.position.x);
        for (_,_,state) in bodies {assert!(state.position.is_finite() && state.velocity.is_finite());}
    }
}
