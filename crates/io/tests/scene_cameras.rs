use poseforge_core::{CameraLens, Node, Project, SCHEMA_VERSION};
use poseforge_io::{load_project, merge_project, save_project};

fn cameras() -> Project {
    let mut project = Project::default();
    for id in [1, 2] {
        let mut node = Node::new(id, format!("Camera {id}"));
        node.camera = Some(CameraLens::default());
        project.nodes.push(node);
    }
    project.active_camera = Some(2);
    project
}
#[test]
fn schema_three_loads_without_rewriting_and_saves_current_schema() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.poseforge");
    let new = dir.path().join("new.poseforge");
    let mut json = serde_json::to_value(Project::demo()).unwrap();
    json["version"] = serde_json::json!(3);
    json.as_object_mut().unwrap().remove("active_camera");
    for node in json["nodes"].as_array_mut().unwrap() {
        node.as_object_mut().unwrap().remove("camera");
    }
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&old, &bytes).unwrap();
    let project = load_project(&old).unwrap();
    assert_eq!(project.version, poseforge_core::SCHEMA_VERSION);
    assert!(project.active_camera.is_none());
    assert_eq!(std::fs::read(&old).unwrap(), bytes);
    save_project(&project, &new).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(new).unwrap()).unwrap();
    assert_eq!(saved["version"], SCHEMA_VERSION);
}
#[test]
fn camera_nodes_save_roundtrip_merge_ids_and_keep_destination_view() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cameras.poseforge");
    let project = cameras();
    save_project(&project, &path).unwrap();
    assert_eq!(project, load_project(path).unwrap());
    let mut destination = cameras();
    let original = destination.active_camera;
    let roots = merge_project(&mut destination, project).unwrap();
    assert_eq!(destination.active_camera, original);
    assert_eq!(roots.len(), 2);
    assert!(roots.iter().all(|id| *id > 2));
    assert!(roots
        .iter()
        .all(|id| destination.node(*id).unwrap().camera.is_some()));
    destination.validate().unwrap();
}
#[test]
fn gltf_preserves_all_camera_nodes_and_chooses_first_for_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cameras.gltf");
    let json = serde_json::json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0,1]}],
        "nodes":[{"name":"Wide camera","camera":0,"translation":[1,2,3]},{"name":"Ortho camera","camera":1}],
        "cameras":[{"type":"perspective","perspective":{"yfov":0.7,"znear":0.1}},
            {"type":"orthographic","orthographic":{"xmag":2,"ymag":1,"znear":0.1,"zfar":100}}]});
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let imported = poseforge_io::import_gltf(path).unwrap();
    let project = imported.project;
    assert_eq!(
        project.nodes.iter().filter(|n| n.camera.is_some()).count(),
        2
    );
    assert_eq!(project.active_camera, Some(1));
    assert!(!project.node(1).unwrap().camera.unwrap().orthographic);
    assert!(project.node(2).unwrap().camera.unwrap().orthographic);
    assert_eq!(project.camera.eye, poseforge_core::Vec3::new(1., 2., 3.));
    project.validate().unwrap();
}
