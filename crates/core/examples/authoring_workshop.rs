//! Reproducible editable examples for geometry, cloth and an authored skeleton.
use poseforge_core::*;
use std::{collections::BTreeSet, fs, path::PathBuf};

fn object(p: &mut Project, id: u64, mesh: Mesh, translation: Vec3) {
    let mut node = Node::new(id, mesh.name.clone());
    node.mesh = Some(mesh.id);
    node.transform.translation = translation;
    p.meshes.push(mesh);
    p.nodes.push(node);
}
fn setup(title: &str) -> Project {
    let mut p = Project {
        title: title.into(),
        ..Project::default()
    };
    p.render_settings.background_mode = BackgroundMode::Solid;
    p.render_settings.background_color = [0.025, 0.035, 0.05];
    p.render_settings.ambient_intensity = 0.5;
    p.render_settings.output_width = 1200;
    p.render_settings.output_height = 800;
    p.materials = vec![
        Material {
            id: 10,
            name: "Графит".into(),
            base_color: [0.08, 0.11, 0.15, 1.],
            roughness: 0.6,
            ..Material::default()
        },
        Material {
            id: 11,
            name: "Синий сатин".into(),
            base_color: [0.08, 0.3, 0.65, 1.],
            double_sided: true,
            ..MaterialPreset::Satin.create()
        },
        Material {
            id: 12,
            name: "Тёплая керамика".into(),
            base_color: [0.75, 0.24, 0.07, 1.],
            double_sided: true,
            ..MaterialPreset::Ceramic.create()
        },
    ];
    p.camera.eye = Vec3::new(6., 4.6, 10.);
    p.camera.target = Vec3::new(0., 1.35, 0.);
    p.camera.fov_y_radians = 0.65;
    p.lights = vec![
        Light {
            name: "Большой софтбокс".into(),
            kind: LightKind::Area,
            position: Vec3::new(-3., 6., 4.),
            direction: Vec3::new(3., -5., -4.).normalize(),
            intensity: 10.,
            area_size: [4., 3.],
            ..Light::default()
        },
        Light {
            name: "Контур".into(),
            kind: LightKind::Point,
            position: Vec3::new(3., 4., -3.),
            color: [0.5, 0.7, 1.],
            intensity: 70.,
            ..Light::default()
        },
    ];
    object(
        &mut p,
        100,
        box_mesh(101, "Основание", Vec3::new(8., 0.15, 5.), Some(10)),
        Vec3::ZERO,
    );
    p.node_mut(100).unwrap().physics = Some(PhysicsSettings {
        kind: PhysicsKind::Collider,
        ..PhysicsSettings::default()
    });
    p
}
fn save(p: &mut Project, out: &std::path::Path, name: &str) {
    let id = p.next_id();
    let mut node = Node::new(id, "Камера мастерской");
    node.camera = Some(CameraLens::from_camera(&p.camera));
    node.transform = camera_transform(&p.camera);
    p.active_camera = Some(id);
    p.nodes.push(node);
    p.validate().unwrap();
    fs::write(out.join(name), serde_json::to_vec(p).unwrap()).unwrap();
}
fn main() {
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples"));
    fs::create_dir_all(&out).unwrap();
    let mut p = setup("Мастерская 0.11 · геометрия, ткань и скелет");
    object(
        &mut p,
        200,
        physics_mesh(201, 11, false),
        Vec3::new(-2.3, 0.6, 0.),
    );
    p.node_mut(200).unwrap().name = "Ткань · запустите физику".into();
    p.node_mut(200).unwrap().physics = Some(PhysicsSettings {
        bending: 24.,
        friction: 0.65,
        iterations: 12,
        wind: 1.8,
        ..PhysicsSettings::default()
    });
    object(
        &mut p,
        300,
        box_mesh(301, "Геометрия · F2", Vec3::new(1., 1., 1.), Some(12)),
        Vec3::new(2.2, 0.65, 0.),
    );
    let mut strip = physics_mesh(401, 12, false);
    strip.name = "Лента · привязана к цепи".into();
    for v in &mut strip.vertices {
        v.position[0] *= 0.25;
        v.position[1] *= 1.4;
    }
    object(&mut p, 400, strip, Vec3::new(0., 0.15, 0.));
    let mut e = Editor::new(p);
    e.edit_mesh(
        300,
        MeshEdit::Extrude {
            faces: BTreeSet::from([0, 1]),
            distance: 0.35,
        },
    )
    .unwrap();
    let root = e
        .create_skeleton(SkeletonTemplate::Chain, Some(400))
        .unwrap();
    e.bind_mesh(400, root).unwrap();
    e.project.node_mut(root).unwrap().name = "Риг ленты · 3 кости".into();
    let joint = root + 2;
    let rest = e.project.node(joint).unwrap().transform;
    let mut bend = rest;
    bend.rotation = Quat::from_rotation_z(-0.65);
    let mut clip = Clip::new("Лента · сгибание", 2.);
    clip.key_transform(joint, 0., rest);
    clip.key_transform(joint, 1., bend);
    clip.key_transform(joint, 2., rest);
    e.project.clips.push(clip);
    save(&mut e.project, &out, "authoring-workshop.artforge");

    let mut p = setup("Риг 0.11 · гуманоид, автовеса и IK");
    p.camera.eye = Vec3::new(3.8, 2.8, 7.);
    p.camera.target = Vec3::new(0., 1.2, 0.);
    let mut body = Mesh {
        id: 501,
        name: "Манекен · единый меш".into(),
        vertices: Vec::new().into(),
        indices: Vec::new().into(),
        material: Some(12),
        topology: None,
        morph_targets: Vec::new(),
    };
    for (size, center) in [
        (Vec3::new(0.44, 0.55, 0.25), Vec3::new(0., 1.15, 0.)),
        (Vec3::splat(0.32), Vec3::new(0., 1.85, 0.)),
        (Vec3::new(0.17, 0.8, 0.19), Vec3::new(0.18, 0.5, 0.)),
        (Vec3::new(0.17, 0.8, 0.19), Vec3::new(-0.18, 0.5, 0.)),
        (Vec3::new(0.65, 0.17, 0.18), Vec3::new(0.56, 1.53, 0.)),
        (Vec3::new(0.65, 0.17, 0.18), Vec3::new(-0.56, 1.53, 0.)),
        (Vec3::new(0.18, 0.15, 0.15), Vec3::new(0., 1.58, 0.)),
    ] {
        let mut part = box_mesh(0, "", size, None);
        let offset = body.vertices.len() as u32;
        for v in &mut part.vertices {
            v.position = (Vec3::from_array(v.position) + center).to_array();
        }
        body.vertices.extend(part.vertices);
        body.indices
            .extend(part.indices.into_iter().map(|i| i + offset));
    }
    object(&mut p, 500, body, Vec3::ZERO);
    let mut e = Editor::new(p);
    e.edit_mesh(500, MeshEdit::Subdivide).unwrap();
    e.edit_mesh(500, MeshEdit::Subdivide).unwrap();
    let root = e
        .create_skeleton(SkeletonTemplate::Humanoid, Some(500))
        .unwrap();
    e.bind_mesh(500, root).unwrap();
    let joint = e.project.rig_profiles[0].assignments[&BoneRole::LeftForearm];
    let rest = e.project.node(joint).unwrap().transform;
    let mut raised = rest;
    raised.rotation = Quat::from_rotation_z(0.8);
    let mut clip = Clip::new("Рука · проба рига", 2.);
    clip.key_transform(joint, 0., rest);
    clip.key_transform(joint, 1., raised);
    clip.key_transform(joint, 2., rest);
    e.project.clips.push(clip);
    save(&mut e.project, &out, "rig-workshop.artforge");
}
