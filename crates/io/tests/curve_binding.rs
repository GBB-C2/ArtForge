use poseforge_core::*;
use poseforge_io::{load_project, merge_project, save_project};

#[test]
fn bindings_roundtrip_and_remap_for_all_three_target_types_on_merge() {
    let mut p = Project::default();
    let mut path = Node::new(1, "Path"); path.curve = Some(CurvePreset::Circle.curve());
    let binding = CurveBinding { curve:1, position:0.25, speed:0.1, repeat:true, follow:false };
    let mut mesh = Node::new(2, "Mesh"); mesh.mesh = Some(10); mesh.curve_binding = Some(binding);
    let mut camera = Node::new(3, "Camera"); camera.camera = Some(CameraLens::default()); camera.curve_binding = Some(binding);
    p.nodes = vec![path,mesh,camera]; p.meshes.push(box_mesh(10,"Cube",Vec3::ONE,None));
    p.lights.truncate(1); p.lights[0].curve_binding = Some(binding);
    let directory = tempfile::tempdir().unwrap(); let file = directory.path().join("path.artforge");
    save_project(&p, &file).unwrap(); assert_eq!(load_project(&file).unwrap(), p);
    let mut merged = p.clone(); let roots = merge_project(&mut merged, p).unwrap();
    let new_path = roots.iter().copied().find(|id| merged.node(*id).unwrap().curve.is_some()).unwrap();
    assert_ne!(new_path, 1);
    for id in roots {
        let n = merged.node(id).unwrap();
        if n.mesh.is_some() || n.camera.is_some() { assert_eq!(n.curve_binding.unwrap().curve, new_path); }
    }
    assert_eq!(merged.lights.last().unwrap().curve_binding.unwrap().curve, new_path);
    merged.validate().unwrap();
}

#[test]
fn schema_29_defaults_to_unbound_without_rewriting_source_file() {
    let mut p = Project::default(); p.nodes.push(Node::new(1, "Existing"));
    let mut json = serde_json::to_value(p).unwrap(); json["version"] = 29.into();
    for node in json["nodes"].as_array_mut().unwrap() { node.as_object_mut().unwrap().remove("curve_binding"); }
    for light in json["lights"].as_array_mut().unwrap() { light.as_object_mut().unwrap().remove("curve_binding"); }
    let bytes = serde_json::to_vec(&json).unwrap();
    let directory = tempfile::tempdir().unwrap(); let file = directory.path().join("legacy.artforge");
    std::fs::write(&file, &bytes).unwrap(); let p = load_project(&file).unwrap();
    assert_eq!(p.version, SCHEMA_VERSION);
    assert!(p.nodes.iter().all(|n| n.curve_binding.is_none()));
    assert!(p.lights.iter().all(|l| l.curve_binding.is_none()));
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
}
