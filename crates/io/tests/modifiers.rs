use poseforge_core::*;
#[test]
fn stacks_roundtrip_and_legacy_projects_default_to_empty_without_rewriting_geometry() {
    let mut n = Node::new(1, "Куб");
    n.mesh = Some(2);
    let source = Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    };
    let mut e = Editor::new(source.clone());
    e.add_modifier(1, ModifierKind::Array).unwrap();
    e.add_modifier(1, ModifierKind::Subdivision).unwrap();
    let bytes = serde_json::to_vec(&e.project).unwrap();
    let loaded = poseforge_io::load_project_bytes(&bytes).unwrap();
    assert_eq!(loaded, e.project);
    assert_eq!(loaded.meshes, source.meshes);
    assert_eq!(
        loaded.evaluated(None, 0.).unwrap().modifier_meshes[&1].face_count(),
        48
    );
    let mut old = serde_json::to_value(&source).unwrap();
    old["version"] = serde_json::json!(43);
    old["nodes"][0].as_object_mut().unwrap().remove("modifiers");
    let legacy = poseforge_io::load_project_bytes(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(legacy, source);
    let mut merged = source.clone();
    let ids = poseforge_io::merge_project(&mut merged, loaded).unwrap();
    let n = merged.node(ids[0]).unwrap();
    assert_eq!(n.modifiers.len(), 2);
    merged.validate().unwrap();
}
