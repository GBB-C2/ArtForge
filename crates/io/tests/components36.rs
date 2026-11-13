use poseforge_core::{box_mesh, DestructionSettings, Node, PhysicsSettings, Project, Vec3, SCHEMA_VERSION};
use poseforge_io::{load_project, merge_project, save_project};

fn scene() -> Project {
    let mut source = Node::new(1, "Fracture source");
    source.mesh = Some(2); source.visible = false; source.render_visible = false;
    let mut settings = DestructionSettings { result: Some(3), ..Default::default() };
    settings.values[0] = 17.; source.destruction = Some(settings);
    let mut cloth = Node::new(4, "Cloth"); cloth.mesh = Some(2);
    cloth.physics = Some(PhysicsSettings { visible: false, ..Default::default() });
    Project {
        nodes: vec![source, Node::new(3, "Fragments"), cloth],
        meshes: vec![box_mesh(2, "Cube", Vec3::ONE, None)],
        lights: vec![],
        ..Default::default()
    }
}

#[test]
fn components_roundtrip_and_repeated_import_remaps_their_result_links() {
    let p = scene();
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("components.artforge");
    save_project(&p, &path).unwrap();
    let loaded = load_project(&path).unwrap(); assert_eq!(loaded, p);
    let mut merged = p.clone();
    for _ in 0..2 { merge_project(&mut merged, loaded.clone()).unwrap(); }
    merged.validate().unwrap();
    let roots: std::collections::BTreeSet<_> = merged.nodes.iter().filter_map(|n| n.destruction.and_then(|s| s.result)).collect();
    assert_eq!(roots.len(), 3);
    for node in merged.nodes.iter().filter(|n| n.destruction.is_some()) {
        let settings = node.destruction.unwrap();
        assert_eq!(settings.values[0], 17.);
        assert_eq!(merged.node(settings.result.unwrap()).unwrap().name, "Fragments");
    }
}

#[test]
fn previous_schema_defaults_cloth_visibility_without_modifying_input_file() {
    let mut p = scene(); p.node_mut(1).unwrap().destruction = None;
    let mut json = serde_json::to_value(&p).unwrap(); json["version"] = 23.into();
    json["nodes"][2]["physics"].as_object_mut().unwrap().remove("visible");
    let bytes = serde_json::to_vec(&json).unwrap();
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("old.artforge");
    std::fs::write(&path, &bytes).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert!(loaded.node(4).unwrap().physics.unwrap().visible);
    assert!(loaded.node(1).unwrap().destruction.is_none());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
