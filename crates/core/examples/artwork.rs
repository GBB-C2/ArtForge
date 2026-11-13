//! Rebuild the editable 0.8 examples using the public document API.
use poseforge_core::*;
use std::{fs, path::PathBuf};
fn add(
    p: &mut Project,
    name: &str,
    mesh: Mesh,
    position: Vec3,
    physics: Option<PhysicsSettings>,
) -> u64 {
    let id = p.next_id().max(mesh.id + 1);
    let mut n = Node::new(id, name);
    n.mesh = Some(mesh.id);
    n.transform.translation = position;
    n.physics = physics;
    p.meshes.push(mesh);
    p.nodes.push(n);
    id
}
fn save(p: &Project, dir: &std::path::Path, name: &str) {
    p.validate().unwrap();
    fs::write(dir.join(name), serde_json::to_vec(p).unwrap()).unwrap();
}
fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("examples"));
    fs::create_dir_all(&dir).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut glass: Project =
        serde_json::from_slice(&fs::read(root.join("examples/area-studio.artforge")).unwrap())
            .unwrap();
    glass.version = SCHEMA_VERSION;
    glass.title = "Стекло · свет и атмосфера".into();
    glass.nodes.truncate(4);
    glass.meshes.truncate(4);
    glass.materials.truncate(4);
    glass.textures.clear();
    glass.active_camera = None;
    glass.camera.eye = Vec3::new(5.8, 3.5, 8.5);
    glass.camera.target = Vec3::new(0., 1., 0.);
    glass.camera.fov_y_radians = 0.75;
    for (i, n) in glass.nodes.iter_mut().skip(1).enumerate() {
        n.transform.translation = Vec3::new(i as f32 * 2.5 - 2.5, 1.01, 0.);
    }
    glass.materials[0].base_color = [1.; 4];
    glass.materials[0].base_color_texture = Some(3000);
    let mut rgba8 = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            rgba8.extend(if (x / 2 + y / 2) % 2 == 0 {
                [185, 194, 210, 255]
            } else {
                [38, 48, 70, 255]
            });
        }
    }
    glass.textures.push(Texture {
        id: 3000,
        name: "Контрастная подложка · рефракция".into(),
        width: 16,
        height: 16,
        rgba8: rgba8.into(),
    });
    MaterialPreset::Glass.apply(&mut glass.materials[1]);
    glass.materials[1].name = "Бирюзовое стекло".into();
    glass.materials[1].base_color = [0.7, 0.94, 1., 1.];
    glass.materials[1].thickness = 1.2;
    glass.materials[1].attenuation_distance = 2.;
    MaterialPreset::Glass.apply(&mut glass.materials[2]);
    glass.materials[2].name = "Прозрачное стекло".into();
    glass.materials[2].base_color = [1.; 4];
    glass.materials[2].thickness = 1.6;
    glass.materials[3].base_color = [0.8, 0.14, 0.04, 1.];
    glass.materials[3].metallic = 0.35;
    glass.render_settings.background_mode = BackgroundMode::Solid;
    glass.render_settings.background_color = [0.025, 0.04, 0.08];
    glass.render_settings.atmosphere.density = 0.008;
    glass.render_settings.atmosphere.distance = 22.;
    glass.render_settings.atmosphere.anisotropy = 0.35;
    glass.render_settings.shadow_filter_radius = 3.;
    glass.render_settings.output_width = 1200;
    glass.render_settings.output_height = 800;
    save(&glass, &dir, "glass-atmosphere.artforge");
    let mut physics = Project {
        title: "Ткань и столкновения".into(),
        ..Project::default()
    };
    physics.camera = glass.camera.clone();
    physics.camera.eye = Vec3::new(5., 3., 7.);
    physics.camera.target = Vec3::new(0., 1.1, 0.);
    physics.render_settings = glass.render_settings;
    physics.render_settings.atmosphere.density = 0.;
    physics.lights = glass.lights.clone();
    for (id, name, color) in [
        (1, "Ткань", [0.08, 0.4, 0.7, 1.]),
        (3, "Коллайдер", [0.12, 0.15, 0.21, 1.]),
    ] {
        physics.materials.push(Material {
            id,
            name: name.into(),
            base_color: color,
            double_sided: id == 1,
            ..Material::default()
        });
    }
    add(
        &mut physics,
        "Ткань · закреплён верх",
        physics_mesh(10, 1, false),
        Vec3::new(-1.4, 0.3, 0.),
        Some(PhysicsSettings {
            wind: 2.,
            ..Default::default()
        }),
    );
    add(
        &mut physics,
        "Пол · коллайдер",
        box_mesh(14, "Пол", Vec3::new(8., 0.2, 6.), Some(3)),
        Vec3::new(0., -0.1, 0.),
        Some(PhysicsSettings {
            kind: PhysicsKind::Collider,
            ..Default::default()
        }),
    );
    add(
        &mut physics,
        "Препятствие · коллайдер",
        box_mesh(16, "Препятствие", Vec3::new(1.2, 0.6, 0.5), Some(3)),
        Vec3::new(-1.4, 0.3, 0.2),
        Some(PhysicsSettings {
            kind: PhysicsKind::Collider,
            ..Default::default()
        }),
    );
    physics.fluid_settings.source_center = [0., 2., 0.];
    physics.fluid_settings.source_kind = 1;
    physics.fluid_settings.source_size = [0.7; 3];
    save(&physics, &dir, "cloth-collisions.artforge");
    let mut layers = Project {
        title: "Слои · движение и добавочное вращение".into(),
        duration: 2.,
        ..Project::default()
    };
    layers.camera = physics.camera;
    layers.render_settings = physics.render_settings;
    layers.materials.push(Material {
        id: 1,
        base_color: [0.12, 0.4, 0.8, 1.],
        ..Default::default()
    });
    let id = add(
        &mut layers,
        "Анимированный куб",
        box_mesh(2, "Куб", Vec3::ONE, Some(1)),
        Vec3::Y,
        None,
    );
    let mut base = Clip::new("Перемещение", 2.);
    for (t, x) in [(0., -1.5), (1., 1.5), (2., -1.5)] {
        base.key_transform(id, t, Transform::from_translation(Vec3::new(x, 1., 0.)));
    }
    let mut spin = Clip::new("Вращение", 2.);
    for (t, a) in [
        (0., 0.),
        (1., std::f32::consts::PI),
        (2., std::f32::consts::TAU),
    ] {
        let tr = Transform {
            rotation: Quat::from_rotation_y(a),
            ..Transform::default()
        };
        spin.key_transform(id, t, tr);
    }
    layers.clips = vec![base, spin];
    layers.animation_layers.push(AnimationLayer {
        name: "Добавочное вращение".into(),
        clip: 1,
        weight: 1.,
        additive: true,
        enabled: true,
        solo: false,
    });
    save(&layers, &dir, "animation-layers.artforge");
}
