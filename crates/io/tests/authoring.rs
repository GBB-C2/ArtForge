use poseforge_core::*;
use poseforge_io::{load_project, save_project};
#[test]
fn schema_six_physics_defaults_migrate_without_rewriting_and_authored_rig_roundtrips() {
    let mut p = Project::default();
    p.meshes.push(physics_mesh(2, 3, false));
    p.materials.push(Material {
        id: 3,
        ..Material::default()
    });
    let mut n = Node::new(1, "Cloth");
    n.mesh = Some(2);
    n.physics = Some(PhysicsSettings::default());
    p.nodes.push(n);
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 6.into();
    for key in ["bending", "friction", "iterations"] {
        json["nodes"][0]["physics"]
            .as_object_mut()
            .unwrap()
            .remove(key);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.artforge");
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let mut loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert_eq!(loaded.nodes[0].physics, Some(PhysicsSettings::default()));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    loaded.nodes[0].physics = None;
    let mut e = Editor::new(loaded);
    let root = e.create_skeleton(SkeletonTemplate::Chain, Some(1)).unwrap();
    e.bind_mesh(1, root).unwrap();
    save_project(&e.project, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), e.project);
    let mut settings = PhysicsSettings {
        iterations: 0,
        ..PhysicsSettings::default()
    };
    assert!(settings.validate().is_err());
    settings.iterations = 25;
    assert!(settings.validate().is_err());
    settings.iterations = 24;
    settings.friction = 1.01;
    assert!(settings.validate().is_err());
    settings.friction = 1.;
    settings.bending = 100.;
    assert!(settings.validate().is_ok());
}
