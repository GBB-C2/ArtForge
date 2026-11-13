//! Editable comparisons for the 0.9 material renderer; no external textures needed.
use poseforge_core::*;
use std::{fs, path::PathBuf};

fn save(p: &Project, dir: &std::path::Path, name: &str) {
    p.validate().unwrap();
    fs::write(dir.join(name), serde_json::to_vec(p).unwrap()).unwrap();
}
fn camera(p: &mut Project) {
    p.active_camera = Some(9000);
    let mut node = Node::new(9000, "Камера · материалы");
    node.camera = Some(CameraLens::from_camera(&p.camera));
    node.transform = camera_transform(&p.camera);
    p.nodes.push(node);
}
fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("examples"));
    fs::create_dir_all(&out).unwrap();
    let mut p: Project =
        serde_json::from_slice(&fs::read(root.join("examples/area-studio.artforge")).unwrap())
            .unwrap();
    p.version = SCHEMA_VERSION;
    p.title = "Материалы 0.9 · металл, сатин и бархат".into();
    p.nodes.truncate(7);
    p.textures.clear();
    p.camera.eye = Vec3::new(7., 7.5, 13.5);
    p.camera.target = Vec3::new(0., 0.8, 0.);
    p.camera.fov_y_radians = 0.7;
    p.render_settings.output_width = 1200;
    p.render_settings.output_height = 800;
    p.render_settings.background_mode = BackgroundMode::Solid;
    p.render_settings.background_color = [0.025, 0.033, 0.048];
    p.render_settings.ambient_intensity = 0.45;
    p.materials[0].base_color = [0.09, 0.11, 0.14, 1.];
    let specs = [
        (MaterialPreset::BrushedMetal, "01 · Металл · вдоль UV", 0.),
        (
            MaterialPreset::BrushedMetal,
            "02 · Металл · поворот 90°",
            90.,
        ),
        (
            MaterialPreset::BrushedMetal,
            "03 · Металл · круглый блик",
            0.,
        ),
        (MaterialPreset::Satin, "04 · Сатин · синий", 20.),
        (MaterialPreset::Velvet, "05 · Бархат · бордовый", 0.),
        (MaterialPreset::Velvet, "06 · Без ворса · сравнение", 0.),
    ];
    for (i, (preset, name, angle)) in specs.into_iter().enumerate() {
        let id = p.materials[i + 1].id;
        p.materials[i + 1] = Material {
            id,
            name: name.into(),
            anisotropy_rotation: angle,
            ..preset.create()
        };
        p.nodes[i + 1].name = name.into();
        p.meshes[i + 1].name = name.into();
    }
    p.materials[3].anisotropy = 0.;
    p.materials[6].sheen_weight = 0.;
    // A compact key gives a readable anisotropic lobe; the large rim shows cloth edges.
    p.lights[0].kind = LightKind::Point;
    p.lights[0].position = Vec3::new(-2.5, 5., 5.);
    p.lights[0].intensity = 65.;
    p.lights[0].name = "Ключ · форма блика".into();
    p.lights[1].intensity = 8.;
    p.lights[2].intensity = 1.;
    camera(&mut p);
    save(&p, &out, "material-fibers.artforge");

    let mut glass: Project =
        serde_json::from_slice(&fs::read(root.join("examples/glass-atmosphere.artforge")).unwrap())
            .unwrap();
    glass.version = SCHEMA_VERSION;
    glass.title = "Стекло 0.9 · независимое поглощение".into();
    glass.render_settings.atmosphere.density = 0.;
    glass.render_settings.output_width = 1200;
    glass.render_settings.output_height = 800;
    let specs = [
        ("01 · Бесцветное стекло", [1., 1., 1.]),
        ("02 · Бирюзовое поглощение", [0.12, 0.72, 0.91]),
        ("03 · Янтарное поглощение", [0.92, 0.32, 0.055]),
    ];
    for (i, (name, color)) in specs.into_iter().enumerate() {
        let id = glass.materials[i + 1].id;
        glass.materials[i + 1] = Material {
            id,
            name: name.into(),
            base_color: [1.; 4],
            attenuation_color: Some(color),
            thickness: 1.2,
            attenuation_distance: 1.5,
            ..MaterialPreset::Glass.create()
        };
        glass.nodes[i + 1].name = name.into();
        glass.meshes[i + 1].name = name.into();
    }
    camera(&mut glass);
    save(&glass, &out, "absorption-glass.artforge");
    save(&rough_glass(), &out, "rough-glass.artforge");
}

fn rough_glass() -> Project {
    let mut p = Project {
        title: "Стекло 0.10 · толщина и мягкое преломление".into(),
        ..Project::default()
    };
    p.camera.eye = Vec3::new(3., 3., 10.5);
    p.camera.target = Vec3::new(0., 1.3, -0.4);
    p.camera.fov_y_radians = 0.68;
    p.render_settings.output_width = 1200;
    p.render_settings.output_height = 800;
    p.render_settings.background_mode = BackgroundMode::Solid;
    p.render_settings.background_color = [0.025, 0.035, 0.05];
    p.render_settings.ambient_intensity = 0.4;
    p.materials = vec![
        Material {
            id: 1,
            name: "Основание".into(),
            base_color: [0.065, 0.09, 0.13, 1.],
            roughness: 0.8,
            ..Material::default()
        },
        Material {
            id: 2,
            name: "Контрастная подложка".into(),
            base_color: [1.; 4],
            base_color_texture: Some(90),
            emissive: [0.12; 3],
            emissive_texture: Some(90),
            roughness: 1.,
            ..Material::default()
        },
        Material {
            id: 3,
            name: "Рамки · тёмный металл".into(),
            base_color: [0.12, 0.16, 0.2, 1.],
            ..MaterialPreset::Metal.create()
        },
        Material {
            id: 4,
            name: "Красный предмет перед стеклом".into(),
            base_color: [0.6, 0.012, 0.009, 1.],
            roughness: 0.45,
            ..Material::default()
        },
    ];
    let mut pixels = vec![];
    for y in 0..64 {
        for x in 0..128 {
            pixels.extend(if (x / 4 + y / 4) % 2 == 0 {
                [222, 226, 232, 255]
            } else {
                [32, 55, 88, 255]
            });
        }
    }
    p.textures.push(Texture {
        id: 90,
        name: "Частая клетка · за стеклом".into(),
        width: 128,
        height: 64,
        rgba8: pixels.into(),
    });
    fn add(p: &mut Project, mesh: Mesh, position: Vec3) {
        let mut node = Node::new(mesh.id + 1000, mesh.name.clone());
        node.mesh = Some(mesh.id);
        node.transform.translation = position;
        p.meshes.push(mesh);
        p.nodes.push(node);
    }
    fn pane(id: u64, name: &str, material: u64, width: f32, height: f32) -> Mesh {
        Mesh {
            id,
            name: name.into(),
            material: Some(material),
            indices: vec![0, 1, 2, 0, 2, 3].into(),
            topology: None,
            morph_targets: vec![],
            vertices: [
                ([-width, -height, 0.], [0., 1.]),
                ([width, -height, 0.], [1., 1.]),
                ([width, height, 0.], [1., 0.]),
                ([-width, height, 0.], [0., 0.]),
            ]
            .into_iter()
            .map(|(position, uv)| Vertex {
                position,
                uv,
                normal: [0., 0., 1.],
                ..Vertex::default()
            })
            .collect(),
        }
    }
    add(
        &mut p,
        box_mesh(100, "Подиум", Vec3::new(5., 0.1, 2.5), Some(1)),
        Vec3::new(0., -0.1, -0.5),
    );
    add(
        &mut p,
        pane(101, "Клетка за стеклом", 2, 5., 2.),
        Vec3::new(0., 2., -1.4),
    );
    for (i, (name, roughness, thickness)) in [
        ("01 · Прозрачное", 0.06, 0.12),
        ("02 · Матовое · тонкое", 0.8, 0.12),
        ("03 · Матовое · толстое", 0.8, 1.2),
    ]
    .into_iter()
    .enumerate()
    {
        let material = 10 + i as u64;
        p.materials.push(Material {
            id: material,
            name: name.into(),
            base_color: [1.; 4],
            attenuation_color: Some([1.; 3]),
            roughness,
            thickness,
            ..MaterialPreset::Glass.create()
        });
        let x = (i as f32 - 1.) * 2.45;
        add(
            &mut p,
            pane(200 + i as u64, name, material, 1., 1.25),
            Vec3::new(x, 1.4, 0.),
        );
        for (j, (offset, extent)) in [
            (Vec3::new(-1.03, 0., 0.), Vec3::new(0.035, 1.32, 0.045)),
            (Vec3::new(1.03, 0., 0.), Vec3::new(0.035, 1.32, 0.045)),
            (Vec3::new(0., -1.285, 0.), Vec3::new(1.06, 0.035, 0.045)),
            (Vec3::new(0., 1.285, 0.), Vec3::new(1.06, 0.035, 0.045)),
        ]
        .into_iter()
        .enumerate()
        {
            add(
                &mut p,
                box_mesh(300 + i as u64 * 4 + j as u64, "Рамка", extent, Some(3)),
                Vec3::new(x, 1.4, 0.) + offset,
            );
        }
    }
    add(
        &mut p,
        box_mesh(
            400,
            "Передний предмет · проверка ореолов",
            Vec3::new(0.1, 1.05, 0.08),
            Some(4),
        ),
        Vec3::new(0.2, 1.05, 0.6),
    );
    p.lights = vec![
        Light {
            name: "Большой мягкий ключ".into(),
            kind: LightKind::Area,
            position: Vec3::new(-3., 6., 4.),
            direction: Vec3::new(3., -4.5, -4.).normalize(),
            intensity: 9.,
            area_size: [3., 3.],
            ..Light::default()
        },
        Light {
            name: "Заполнение".into(),
            kind: LightKind::Area,
            position: Vec3::new(5., 4., 3.),
            direction: Vec3::new(-5., -2.5, -3.).normalize(),
            intensity: 3.,
            area_size: [3., 3.],
            casts_shadow: false,
            ..Light::default()
        },
    ];
    camera(&mut p);
    p
}
