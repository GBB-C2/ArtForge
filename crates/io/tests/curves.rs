use poseforge_core::*;
use poseforge_io::{load_project, merge_project, save_project};

#[test]
fn curves_and_camera_effects_roundtrip_and_merge_with_remapped_geometry() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("curves.artforge");
    let mut editor = Editor::new(Project::default());
    let curve = editor.create_curve("Helix", CurvePreset::Helix.curve()).unwrap();
    let mut data = CurvePreset::Helix.curve(); data.geometry = Some(Default::default());
    editor.set_curve(curve, data.clone()).unwrap();
    let mut camera = Node::new(editor.project.next_id(), "Camera");
    camera.camera = Some(CameraLens { effects: CameraEffects { enabled: true, grain: 0.1, ..Default::default() }, ..Default::default() });
    editor.project.nodes.push(camera);
    save_project(&editor.project, &path).unwrap();
    let loaded = load_project(&path).unwrap(); assert_eq!(loaded, editor.project);
    let mut merged = loaded.clone(); let roots = merge_project(&mut merged, loaded).unwrap();
    let added = roots.iter().filter_map(|id| merged.node(*id)).find(|n| n.curve.is_some()).unwrap();
    assert_eq!(added.curve, Some(data));
    assert_ne!(added.mesh, editor.project.node(curve).unwrap().mesh); merged.validate().unwrap();
}
#[test]
fn legacy_curve_tubes_migrate_in_memory_without_rewriting_the_file() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("legacy.artforge");
    let mut node = Node::new(1, "Circle"); node.mesh = Some(2);
    let mesh = PrimitiveSettings::new(PrimitiveKind::CurveCircle).mesh(2, None).unwrap();
    let mut project = Project { nodes: vec![node], meshes: vec![mesh.clone()], ..Default::default() };
    project.version = 25;
    let bytes = serde_json::to_vec(&project).unwrap(); std::fs::write(&path, &bytes).unwrap();
    let loaded = load_project(&path).unwrap(); assert_eq!(loaded.version, SCHEMA_VERSION);
    assert!(loaded.node(1).unwrap().curve.as_ref().unwrap().closed);
    assert_eq!(loaded.mesh(2), Some(&mesh)); assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn shadow_only_roundtrips_merges_and_defaults_off_in_schema_28() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("shadow.artforge");
    let mut node = Node::new(1, "Caster"); node.mesh = Some(2); node.shadow_only = true;
    let project = Project { nodes: vec![node], meshes: vec![box_mesh(2, "Cube", Vec3::ONE, None)], ..Default::default() };
    save_project(&project, &path).unwrap();
    let loaded = load_project(&path).unwrap(); assert_eq!(loaded, project);
    let mut merged = project.clone(); let roots = merge_project(&mut merged, loaded).unwrap();
    assert_ne!(roots[0], 1); assert!(merged.node(roots[0]).unwrap().shadow_only);
    merged.validate().unwrap();
    let mut legacy = serde_json::to_value(&project).unwrap();
    legacy["version"] = 28.into();
    legacy["nodes"][0].as_object_mut().unwrap().remove("shadow_only");
    let bytes = serde_json::to_vec(&legacy).unwrap(); std::fs::write(&path, &bytes).unwrap();
    let migrated = load_project(&path).unwrap();
    assert_eq!(migrated.version, SCHEMA_VERSION);
    let node = migrated.node(1).unwrap();
    assert!(!node.shadow_only && node.visible && node.render_visible);
    assert_eq!(std::fs::read(path).unwrap(), bytes, "migration must not rewrite the input file");
}
