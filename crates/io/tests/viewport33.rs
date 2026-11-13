#![allow(clippy::field_reassign_with_default)]
use poseforge_core::{Light, Material, Project, VolumeLight, SCHEMA_VERSION};
use poseforge_io::{load_project, save_project};

#[test]
fn light_volumes_and_material_visibility_roundtrip_independently() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = dir.path().join("volume.artforge");
    let mut p = Project::default();
    p.lights = vec![
        Light { volume: VolumeLight { enabled: true, strength: 2., tint: [1., 0., 0.] }, ..Default::default() },
        Light { volume: VolumeLight { enabled: true, strength: 0.5, tint: [0., 0., 1.] }, ..Default::default() },
    ];
    p.materials = vec![Material { id: 1, visible: false, ..Default::default() }];
    p.render_settings.atmosphere.density = 0.05;
    save_project(&p, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), p);
}

#[test]
fn old_fog_projects_keep_all_contributing_lights_without_rewriting_input() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = dir.path().join("old.artforge");
    let mut p = Project::default();
    p.materials = vec![Material { id: 1, ..Default::default() }];
    p.render_settings.atmosphere.density = 0.08;
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 21.into();
    for light in json["lights"].as_array_mut().unwrap() { light.as_object_mut().unwrap().remove("volume"); }
    json["materials"][0].as_object_mut().unwrap().remove("visible");
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert_eq!(loaded.render_settings.atmosphere.density, 0.08);
    assert!(loaded.lights.iter().all(|l| l.volume.enabled && l.volume.strength == 1.));
    assert!(loaded.materials[0].visible);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
