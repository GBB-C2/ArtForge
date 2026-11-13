//! Small, isolated sample scenes rendered by the shared preview worker.
//! Geometry generation and GPU readback never run on the UI thread.
use super::*;
use poseforge_core::{CameraLens, CurveData, CurveGeometry, Light, LightKind, Material, Mesh, Node, PrimitiveKind, PrimitiveSettings, SkeletonTemplate};
use std::hash::{Hash, Hasher};

#[derive(Clone)]
pub(super) enum AssetImage {
    Primitive(u8), Curve(CurveData), Light(Light), Scene(u8), Camera(CameraLens),
    Skeleton(bool), Object(u64), Rig(u64),
}
pub(super) struct AssetRequest { source: AssetImage, snapshot: Option<Project> }
fn object_material_ids(node: &Node, mesh: &Mesh) -> BTreeSet<u64> {
    let mut ids: BTreeSet<_> = mesh.material.into_iter().chain(node.material_layers.iter().map(|layer| layer.material)).collect();
    if let Some(t) = &mesh.topology { ids.extend(t.face_materials.iter().flatten().copied()); }
    ids
}
fn hash_value<T: serde::Serialize>(value: &T, hash: &mut impl Hasher) { serde_json::to_vec(value).unwrap_or_default().hash(hash); }
// Thumbnail lights must not depend on scene objects or animation placement.
fn preview_light(mut light: Light) -> Light {
    light.name.clear(); light.position = Vec3::new(1.3,2.7,1.5);
    light.direction = Vec3::new(-1.3,-2.2,-1.5).normalize();
    light.target = None; light.curve_binding = None;
    light.enabled = true; light.casts_shadow = false; light.range = 0.; light
}
impl AssetImage {
    pub(super) fn key(&self, project: &Project) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        "asset-thumbnail-v2".hash(&mut hash); std::mem::discriminant(self).hash(&mut hash);
        match self {
            Self::Primitive(i) | Self::Scene(i) => i.hash(&mut hash),
            Self::Curve(curve) => hash_value(curve,&mut hash),
            Self::Light(light) => hash_value(&preview_light(light.clone()),&mut hash),
            Self::Camera(lens) => hash_value(lens,&mut hash),
            Self::Skeleton(humanoid) => humanoid.hash(&mut hash),
            Self::Object(id) => {
                id.hash(&mut hash);
                if let Some(node) = project.node(*id) {
                    hash_value(&node.material_layers,&mut hash);
                    if let Some(mesh) = node.mesh.and_then(|m| project.mesh(m)) {
                        (mesh.id,mesh.vertices.revision(),mesh.indices.revision(),mesh.material).hash(&mut hash);
                        if let Some(t) = &mesh.topology { t.face_materials.revision().hash(&mut hash); }
                        let ids = object_material_ids(node,mesh);
                        let materials: Vec<_> = project.materials.iter().filter(|m|ids.contains(&m.id)).collect();
                        for material in &materials { hash_value(material,&mut hash); }
                        for t in project.textures.iter().filter(|t|materials.iter().any(|m|super::material_preview::uses_texture(m,t.id))) {
                            (t.id,t.width,t.height,t.rgba8.revision()).hash(&mut hash);
                        }
                    }
                }
            }
            Self::Rig(root) => {
                root.hash(&mut hash);
                for node in &project.nodes { hash_value(&(node.id,node.parent,node.is_joint,node.transform),&mut hash); }
            }
        }
        hash.finish()
    }
    pub(super) fn request(&self, project: &Project) -> AssetRequest {
        let snapshot = match self {
            Self::Object(id) => project.node(*id).and_then(|node| node.mesh.and_then(|id| project.mesh(id)).map(|mesh| {
                let mut owner = Node::new(node.id,&node.name); owner.mesh = Some(mesh.id); owner.material_layers = node.material_layers.clone();
                let ids = object_material_ids(node,mesh);
                let materials: Vec<_> = project.materials.iter().filter(|m|ids.contains(&m.id)).cloned().collect();
                let textures = project.textures.iter().filter(|t|materials.iter().any(|m|super::material_preview::uses_texture(m,t.id))).cloned().collect();
                // Buffer clones share storage; image reduction runs on the worker.
                Project { nodes: vec![owner], meshes: vec![mesh.clone()], materials, textures, ..Project::default() }
            })),
            Self::Rig(_) => Some(Project { nodes: project.nodes.iter().map(|node| {
                let mut copy = Node::new(node.id,&node.name); copy.parent = node.parent; copy.transform = node.transform; copy.is_joint = node.is_joint; copy
            }).collect(), ..Project::default() }),
            _ => None,
        };
        AssetRequest { source: self.clone(), snapshot }
    }
}
pub(super) fn example_light(index: usize) -> Light {
    Light { kind: [LightKind::Directional,LightKind::Point,LightKind::Spot,LightKind::Area][index.min(3)],
        position: Vec3::new(1.3,2.7,1.5), direction: Vec3::new(-1.3,-2.2,-1.5).normalize(),
        intensity: if index==0 { 2.5 } else { 20. }, range: 0., area_size: [1.8,1.2],
        casts_shadow: false, ..Default::default() }
}
fn studio() -> Project {
    let mut p = Project::default();
    p.materials = vec![Material { id: 1, name: "Образец".into(),base_color: [0.24,0.38,0.58,1.],roughness: 0.38,..Default::default() }];
    p.lights = vec![Light { intensity: 3.,direction: Vec3::new(-0.4,-0.6,-0.8).normalize(),casts_shadow:false,..Default::default() },
        Light { intensity:0.7,direction:Vec3::new(0.6,0.2,-0.5).normalize(),color:[0.7,0.8,1.],casts_shadow:false,..Default::default() }];
    p
}
fn add_mesh(p: &mut Project, mut mesh: Mesh, transform: Transform) {
    mesh.id = p.next_id(); let mut node = Node::new(mesh.id+1,"Образец"); node.mesh = Some(mesh.id); node.transform = transform;
    p.meshes.push(mesh); p.nodes.push(node);
}
fn primitive(p: &mut Project, kind: PrimitiveKind, position: Vec3, size: Vec3) -> Result<(),String> {
    let mesh = PrimitiveSettings::new(kind).mesh(0,Some(1)).map_err(|e| e.to_string())?;
    add_mesh(p,mesh,Transform { translation: position,scale:size,..Default::default() }); Ok(())
}
fn rod(p: &mut Project, a: Vec3, b: Vec3, radius: f32) {
    let delta = b-a; if delta.length_squared()<1e-10 { return; }
    let mesh = poseforge_core::box_mesh(0,"Линия",Vec3::new(radius,delta.length(),radius)*0.5,Some(1));
    add_mesh(p,mesh,Transform { translation:(a+b)*0.5,rotation:Quat::from_rotation_arc(Vec3::Y,delta.normalize()),..Default::default() });
}
fn skeleton(source: &Project, root: Option<u64>) -> Result<Project,String> {
    let globals = source.global_transforms().map_err(|e| e.to_string())?; let mut p = studio();
    let mut ids: BTreeSet<_> = if let Some(root) = root { BTreeSet::from([root]) } else { source.nodes.iter().filter(|n| n.is_joint).map(|n| n.id).collect() };
    if root.is_some() { loop {
        let before = ids.len(); for n in &source.nodes { if n.is_joint && n.parent.is_some_and(|id| ids.contains(&id)) { ids.insert(n.id); } }
        if ids.len()==before { break; }
    } }
    // Bound thumbnail complexity even for imported production rigs.
    for n in source.nodes.iter().filter(|n| n.is_joint && ids.contains(&n.id)).take(256) {
        let Some(global) = globals.get(&n.id) else { continue; }; let b = global.w_axis.truncate();
        primitive(&mut p,PrimitiveKind::Sphere,b,Vec3::splat(0.09))?;
        if let Some(a) = n.parent.filter(|id| ids.contains(id)).and_then(|id| globals.get(&id)) { rod(&mut p,a.w_axis.truncate(),b,0.018); }
    }
    Ok(p)
}
fn camera(lens: CameraLens) -> Result<Project,String> {
    let mut p = studio(); primitive(&mut p,PrimitiveKind::Cube,Vec3::new(0.,0.,0.25),Vec3::new(0.28,0.20,0.16))?;
    let h = if lens.orthographic { 0.7 } else { (lens.fov_y_radians*0.5).tan().clamp(0.16,1.1) };
    let frame = [Vec3::new(-h*1.4,-h,-1.4),Vec3::new(h*1.4,-h,-1.4),Vec3::new(h*1.4,h,-1.4),Vec3::new(-h*1.4,h,-1.4)];
    for i in 0..4 { rod(&mut p,frame[i],frame[(i+1)%4],0.018);
        let start = if lens.orthographic { Vec3::new(frame[i].x,frame[i].y,0.) } else { Vec3::ZERO };
        rod(&mut p,start,frame[i],0.014);
    }
    Ok(p)
}
fn build(request: AssetRequest) -> Result<(Project,Vec3,bool),String> {
    let mut direction = Vec3::new(3.,2.,4.).normalize(); let mut light_sample = false;
    let p = match request.source {
        AssetImage::Primitive(index) => {
            let mut p = studio(); let kind = PrimitiveKind::from_code(index).ok_or("Неизвестный примитив")?;
            primitive(&mut p,kind,Vec3::ZERO,Vec3::ONE)?; p
        }
        AssetImage::Curve(mut curve) => {
            direction = if curve.points.iter().any(|p| p[2].abs()>0.01) { Vec3::new(0.3,0.15,1.).normalize() } else { Vec3::Z };
            curve.geometry.get_or_insert(CurveGeometry { radius:0.025,sides:8,..Default::default() });
            let mut p = studio(); let mesh = curve.mesh(0,Some(1)).map_err(|e| e.to_string())?; add_mesh(&mut p,mesh,Default::default()); p
        }
        AssetImage::Light(light) => {
            let mut p = studio(); light_sample = true;
            primitive(&mut p,PrimitiveKind::Sphere,Vec3::new(0.,0.65,0.),Vec3::splat(1.3))?;
            primitive(&mut p,PrimitiveKind::Plane,Vec3::ZERO,Vec3::splat(2.5))?;
            p.materials[0].base_color = [0.65,0.65,0.65,1.];
            p.lights = vec![preview_light(light)]; p.render_settings.ambient_intensity = 0.06; p
        }
        AssetImage::Scene(index) => {
            if index==1 { direction = Vec3::new(0.15,0.06,1.).normalize(); }
            super::asset_library::library_scene(index)?
        }
        AssetImage::Camera(lens) => camera(lens)?,
        AssetImage::Skeleton(humanoid) => {
            direction = Vec3::Z; let mut editor = Editor::new(Project::default());
            editor.create_skeleton(if humanoid { SkeletonTemplate::Humanoid } else { SkeletonTemplate::Chain },None).map_err(|e|e.to_string())?;
            skeleton(&editor.project,None)?
        }
        AssetImage::Object(_) => request.snapshot.ok_or("Объект больше не существует")?,
        AssetImage::Rig(root) => { direction = Vec3::Z; skeleton(&request.snapshot.ok_or("Скелет больше не существует")?,Some(root))? }
    };
    Ok((p,direction,light_sample))
}
pub(super) fn render(renderer: &mut Renderer, request: AssetRequest) -> Result<Vec<poseforge_render::UiRect>,String> {
    const SIZE: u32 = 104;
    let (mut p,direction,light_sample) = build(request)?;
    p.textures = p.textures.iter().map(super::material_preview::small_texture).collect();
    p.render_settings.shadows = false; p.render_settings.antialiasing = 1;
    p.render_settings.hdri_enabled = false; p.render_settings.hdri_background = false;
    p.render_settings.background_mode = poseforge_core::BackgroundMode::Solid;
    p.render_settings.background_color = [0.035,0.045,0.065];
    let globals = p.global_transforms().map_err(|e|e.to_string())?;
    let mut min = Vec3::splat(f32::INFINITY); let mut max = Vec3::splat(f32::NEG_INFINITY);
    for node in &p.nodes { if let (Some(mesh),Some(model)) = (node.mesh.and_then(|id|p.mesh(id)),globals.get(&node.id)) {
        for v in &mesh.vertices { let point = model.transform_point3(Vec3::from_array(v.position)); min = min.min(point); max = max.max(point); }
    } }
    if !min.is_finite() || !max.is_finite() { return Err("Нет геометрии для миниатюры".into()); }
    let target = if light_sample { Vec3::new(0.,0.65,0.) } else { (min+max)*0.5 };
    let radius = (max-min).length().max(0.05)*0.5;
    let side = Vec3::Y.cross(direction).normalize(); let up = direction.cross(side);
    let extent = (max-min)*0.5;
    let half_height = extent.dot(side.abs()).max(extent.dot(up.abs())).max(0.04)*1.16;
    let view = View { eye:target+direction*(radius*3.+1.),target,up:Vec3::Y,orthographic:Some(if light_sample { 1.7 } else { half_height }),
        viewport:[0.,0.,SIZE as f32,SIZE as f32],..Default::default() };
    renderer.invalidate_assets(); let rgba = renderer.render_rgba(&p,&globals,&view,SIZE,SIZE)?;
    Ok(super::material_preview::image_runs(&rgba,SIZE,false))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_preview_drops_scene_dependencies_and_ignores_motion_in_cache_key() {
        let p = Project::default();let light = example_light(2);
        let original_key = AssetImage::Light(light.clone()).key(&p);
        let mut moved = light.clone();moved.position = Vec3::splat(40.);
        moved.curve_binding = Some(poseforge_core::CurveBinding { curve:999, ..Default::default() });
        moved.target = Some(poseforge_core::LightTarget { node:888,local_center:Vec3::ZERO });
        assert_eq!(AssetImage::Light(moved.clone()).key(&p), original_key);
        let (preview,_,_) = build(AssetImage::Light(moved.clone()).request(&p)).unwrap();
        preview.validate().unwrap();
        assert!(preview.lights[0].curve_binding.is_none() && preview.lights[0].target.is_none());
        moved.intensity += 1.;assert_ne!(AssetImage::Light(moved).key(&p), original_key);
    }
    #[test]
    fn preview_projects_are_isolated_and_have_real_geometry() {
        let p = Project::default();
        for image in [AssetImage::Primitive(0),AssetImage::Curve(poseforge_core::CurvePreset::Helix.curve()),
            AssetImage::Camera(Default::default()),AssetImage::Skeleton(true),AssetImage::Light(example_light(2))] {
            let (preview,_,_) = build(image.request(&p)).unwrap(); preview.validate().unwrap();
            assert!(!preview.meshes.is_empty()); assert!(p.nodes.is_empty());
        }
    }
    #[test]
    fn object_key_changes_for_geometry_edits_and_undo_snapshots() {
        let mut p = studio(); primitive(&mut p,PrimitiveKind::Cube,Vec3::ZERO,Vec3::ONE).unwrap();
        let source = AssetImage::Object(p.nodes[0].id); let original = p.clone(); let before = source.key(&p);
        p.meshes[0].vertices[0].position[0] += 1.; assert_ne!(source.key(&p),before);
        assert_eq!(source.key(&original),before);
        let (isolated,_,_) = build(source.request(&p)).unwrap(); assert_eq!(isolated.meshes.len(),1);
    }
}
