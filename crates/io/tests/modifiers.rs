use poseforge_core::*;
#[test]
fn advanced_stacks_roundtrip_migrate_and_remap_curve_targets() {
    let mut root = Node::new(1, "Group");
    root.transform.translation = Vec3::X;
    let mut node = Node::new(2, "Mesh");
    node.parent = Some(1);
    node.mesh = Some(4);
    let mut curve = Node::new(3, "Path");
    curve.parent = Some(1);
    curve.curve = Some(CurveData {
        points: vec![[0., 0., 0.], [8., 0., 0.]],
        interpolation: CurveInterpolation::Polyline,
        ..Default::default()
    });
    node.modifiers = vec![
        ModifierSettings::RadialArray {
            count: 2,
            axis: 2,
            radius: 2.,
            angle: 3.,
            rotate: true,
        },
        ModifierSettings::CurveArray {
            count: 2,
            curve: Some(3),
            spacing: 3.,
            start: 0.,
            roll: 0.2,
            follow: true,
        },
        ModifierKind::Bevel.settings(),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, settings)| MeshModifier {
        id: i as u64 + 1,
        name: "M".into(),
        enabled: true,
        settings,
    })
    .collect();
    let source = Project {
        nodes: vec![root, node, curve],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(4, None)
            .unwrap()],
        ..Default::default()
    };
    let loaded = poseforge_io::load_project_bytes(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert_eq!(loaded, source);
    let mut merged = source.clone();
    let ids = poseforge_io::merge_project(&mut merged, loaded).unwrap();
    assert_eq!(ids.len(), 1);
    let curve = merged
        .nodes
        .iter()
        .find(|n| n.parent == Some(ids[0]) && n.curve.is_some())
        .unwrap()
        .id;
    let object = merged
        .nodes
        .iter()
        .find(|n| n.parent == Some(ids[0]) && n.mesh.is_some())
        .unwrap()
        .id;
    assert!(
        matches!(merged.node(object).unwrap().modifiers[1].settings,ModifierSettings::CurveArray {curve:Some(c),..} if c==curve)
    );
    merged.validate().unwrap();
    let mut duplicate = source.clone();
    let copy = duplicate.duplicate_subtree(1).unwrap();
    let object = duplicate
        .nodes
        .iter()
        .find(|n| n.parent == Some(copy) && n.mesh.is_some())
        .unwrap();
    let curve = duplicate
        .nodes
        .iter()
        .find(|n| n.parent == Some(copy) && n.curve.is_some())
        .unwrap();
    assert!(
        matches!(object.modifiers[1].settings,ModifierSettings::CurveArray {curve:Some(c),..} if c==curve.id)
    );
    duplicate.validate().unwrap();
    let mut old = serde_json::to_value(&Project::default()).unwrap();
    old["version"] = serde_json::json!(44);
    let migrated = poseforge_io::load_project_bytes(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(migrated.version, SCHEMA_VERSION);
}
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
