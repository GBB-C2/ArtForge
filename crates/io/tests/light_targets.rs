use poseforge_core::{LightTarget, Node, Project, Vec3};
use poseforge_io::{load_project, merge_project, save_project};

#[test]
fn tracked_lights_roundtrip_migrate_legacy_and_remap_on_merge() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("tracking.afscene");
    let mut project = Project::default();
    project.nodes.push(Node::new(1, "Target"));
    project.lights[0].target = Some(LightTarget {
        node: 1,
        local_center: Vec3::new(0., 1., 0.),
    });
    save_project(&project, &file).unwrap();
    assert_eq!(load_project(&file).unwrap(), project);
    let mut destination = Project::default();
    destination.nodes.push(Node::new(1, "Existing"));
    merge_project(&mut destination, project.clone()).unwrap();
    let target = destination.lights.last().unwrap().target.unwrap();
    assert_ne!(target.node, 1);
    assert_eq!(destination.node(target.node).unwrap().name, "Target");
    let mut json = serde_json::to_value(&project).unwrap();
    json["version"] = 17.into();
    json["lights"][0].as_object_mut().unwrap().remove("target");
    std::fs::write(&file, serde_json::to_vec(&json).unwrap()).unwrap();
    let loaded = load_project(&file).unwrap();
    assert_eq!(loaded.version, poseforge_core::SCHEMA_VERSION);
    assert!(loaded.lights[0].target.is_none());
}

#[test]
fn tracked_light_evaluates_animated_parent_transforms_without_mutating_fallback() {
    use poseforge_core::{Clip, Transform};
    let mut project = Project::default();
    let mut target = Node::new(2, "Animated target");
    target.parent = Some(1);
    project.nodes = vec![Node::new(1, "Animated parent"), target];
    project.lights[0].position = Vec3::new(0., 3., 4.);
    project.lights[0].target = Some(LightTarget {
        node: 2,
        local_center: Vec3::X,
    });
    let mut clip = Clip::new("Move", 2.);
    clip.key_transform(1, 0., Transform::default());
    clip.key_transform(1, 1., Transform::from_translation(Vec3::new(3., 1., 0.)));
    project.clips.push(clip);
    let fallback = project.lights[0].direction;
    let scene = project.evaluated(Some(0), 1.).unwrap();
    let direction = project.lights[0].evaluated_direction(&scene.global);
    assert!(
        direction.distance((Vec3::new(4., 1., 0.) - project.lights[0].position).normalize()) < 1e-5
    );
    assert_eq!(project.lights[0].direction, fallback);
}
