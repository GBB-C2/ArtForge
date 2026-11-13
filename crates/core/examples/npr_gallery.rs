//! Reproducible, editable NPR comparison scene built from the bundled gallery.
use poseforge_core::*;
use std::{fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("examples/npr-studio.artforge"));
    let mut p: Project =
        serde_json::from_slice(&fs::read(root.join("examples/material-fibers.artforge")).unwrap())
            .unwrap();
    p.version = SCHEMA_VERSION;
    p.title = "NPR · Toon, Anime и Тушь".into();
    p.textures.clear();
    let presets = [
        MaterialPreset::Toon,
        MaterialPreset::Anime,
        MaterialPreset::Ink,
        MaterialPreset::Toon,
        MaterialPreset::Anime,
        MaterialPreset::Ink,
    ];
    for (i, preset) in presets.into_iter().enumerate() {
        let id = p.materials[i + 1].id;
        p.materials[i + 1] = Material {
            id,
            ..preset.create()
        };
        if i >= 3 {
            p.materials[i + 1].npr.bands = 5;
            p.materials[i + 1].npr.softness = 0.04;
            p.materials[i + 1].name.push_str(" · 5 ступеней");
        }
        p.nodes[i + 1].name = p.materials[i + 1].name.clone();
        p.meshes[i + 1].name = p.materials[i + 1].name.clone();
    }
    p.lights = vec![Light {
        name: "NPR · ключевой свет".into(),
        kind: LightKind::Directional,
        direction: Vec3::new(-0.7, -0.8, -0.5).normalize(),
        color: [1.; 3],
        intensity: 0.85,
        ..Light::default()
    }];
    p.render_settings.ambient_intensity = 0.25;
    p.render_settings.ray_reflections = false;
    p.render_settings.ray_shadows = false;
    p.render_settings.tone_mapping = ToneMapping::Aces;
    p.render_settings.output_width = 1200;
    p.render_settings.output_height = 800;
    p.validate().unwrap();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(output, serde_json::to_vec(&p).unwrap()).unwrap();
}
