use poseforge_core::{Material, MaterialPreset, Project, SCHEMA_VERSION};
use poseforge_io::{load_project, save_project};

#[test]
fn npr_roundtrip_and_invalid_save_preserve_previous_file() {
    let mut p = Project::default();
    for (i, preset) in [
        MaterialPreset::Toon,
        MaterialPreset::Anime,
        MaterialPreset::Ink,
    ]
    .into_iter()
    .enumerate()
    {
        let mut m = preset.create();
        m.id = i as u64 + 1;
        m.npr.shadow_color = [0.07, 0.21, 0.42];
        m.npr.palette_strength = 0.8;
        m.npr.palette_mid = [0.2, 0.4, 0.3];
        m.npr.palette_position = 0.4;
        m.npr.hatch_strength = 0.6;
        m.npr.hatch_density = 33.;
        m.npr.hatch_angle = -70.;
        m.npr.hatch_cross = 0.5;
        p.materials.push(m);
    }
    p.render_settings.npr_contours.width = 2.5;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("NPR.artforge");
    save_project(&p, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), p);
    let bytes = std::fs::read(&path).unwrap();
    p.materials[0].npr.bands = 0;
    assert!(save_project(&p, &path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn legacy_projects_load_as_pbr_without_rewriting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.artforge");
    let p = Project {
        materials: vec![MaterialPreset::Glass.create()],
        ..Project::default()
    };
    for version in 1..=13 {
        let mut json = serde_json::to_value(&p).unwrap();
        json["version"] = version.into();
        json["materials"][0].as_object_mut().unwrap().remove("npr");
        let bytes = serde_json::to_vec(&json).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.materials, p.materials);
        assert!(!loaded.materials[0].npr.enabled);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let partial: Material = serde_json::from_value(serde_json::json!({
        "id": 1, "name": "Partial NPR", "base_color": [1,1,1,1], "metallic": 0,
        "roughness": 0.8, "emissive": [0,0,0], "base_color_texture": null,
        "npr": { "enabled": true }
    }))
    .unwrap();
    assert!(partial.npr.enabled && partial.npr.valid());
}
