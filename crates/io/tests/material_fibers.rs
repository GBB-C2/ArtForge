use poseforge_core::{Material, MaterialPreset, Project, SCHEMA_VERSION};
use poseforge_io::{load_project, save_project};

#[test]
fn schema_five_migration_preserves_legacy_absorption_and_does_not_rewrite_source() {
    let p = Project {
        materials: vec![Material {
            base_color: [0.2, 0.7, 1., 1.],
            transmission: 1.,
            ..Material::default()
        }],
        ..Project::default()
    };
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 5.into();
    for key in [
        "anisotropy",
        "anisotropy_rotation",
        "sheen_weight",
        "sheen_roughness",
        "attenuation_color",
    ] {
        json["materials"][0].as_object_mut().unwrap().remove(key);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.artforge");
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert_eq!(loaded.materials, p.materials);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn fiber_and_independent_absorption_roundtrip_and_invalid_save_is_atomic() {
    let mut p = Project {
        materials: vec![Material {
            anisotropy_rotation: -37.5,
            attenuation_color: Some([0.19, 0.71, 0.93]),
            ..MaterialPreset::Satin.create()
        }],
        ..Project::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Материалы.artforge");
    save_project(&p, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), p);
    let bytes = std::fs::read(&path).unwrap();
    p.materials[0].sheen_roughness = -0.1;
    assert!(save_project(&p, &path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
