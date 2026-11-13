use poseforge_core::{
    camera_transform, Camera, CameraLens, Editor, Mat4, Node, Project, Quat, Vec3,
};

fn camera_scene() -> Project {
    let mut project = Project::default();
    let mut camera = Node::new(10, "Camera");
    camera.camera = Some(CameraLens::default());
    project.nodes.push(camera);
    project.active_camera = Some(10);
    project
}

#[test]
fn old_projects_default_to_saved_editor_camera() {
    let project = Project::demo();
    let mut json = serde_json::to_value(&project).unwrap();
    json.as_object_mut().unwrap().remove("active_camera");
    for node in json["nodes"].as_array_mut().unwrap() {
        node.as_object_mut().unwrap().remove("camera");
    }
    let loaded: Project = serde_json::from_value(json).unwrap();
    loaded.validate().unwrap();
    assert!(loaded.active_camera.is_none());
    assert!(loaded.nodes.iter().all(|n| n.camera.is_none()));
    assert_eq!(
        loaded.render_camera(&loaded.global_transforms().unwrap()),
        loaded.camera
    );
}
#[test]
fn scene_camera_uses_evaluated_parent_and_orthonormalizes_scaled_basis() {
    let mut project = camera_scene();
    let mut parent = Node::new(2, "Rig");
    parent.transform.translation = Vec3::new(1., 2., 3.);
    parent.transform.rotation = Quat::from_rotation_y(0.8);
    parent.transform.scale = Vec3::new(2., 3., 4.);
    project.nodes.push(parent);
    project.node_mut(10).unwrap().parent = Some(2);
    project.node_mut(10).unwrap().transform.translation = Vec3::new(0.2, 1., -0.5);
    project.validate().unwrap();
    let mut globals = project.global_transforms().unwrap();
    let a = project.render_camera(&globals);
    assert!(a.eye.distance(globals[&10].transform_point3(Vec3::ZERO)) < 1e-5);
    assert!((a.target - a.eye).dot(a.up).abs() < 1e-6);
    assert!((a.up.length() - 1.).abs() < 1e-6);
    globals.insert(
        10,
        Mat4::from_translation(Vec3::new(12., 5., 7.)) * globals[&10],
    );
    let b = project.render_camera(&globals);
    assert!(b.eye.distance(a.eye + Vec3::new(12., 5., 7.)) < 1e-5);
    assert_eq!(a.fov_y_radians, b.fov_y_radians);
}
#[test]
fn camera_transform_preserves_view_and_roll() {
    let source = Camera {
        eye: Vec3::new(2., 3., 7.),
        target: Vec3::ZERO,
        up: Vec3::new(0.4, 1., 0.),
        ..Camera::default()
    };
    let mut project = camera_scene();
    project.node_mut(10).unwrap().transform = camera_transform(&source);
    let actual = project.render_camera(&project.global_transforms().unwrap());
    assert!(actual.eye.distance(source.eye) < 1e-5);
    assert!(
        (actual.target - actual.eye)
            .normalize()
            .distance((source.target - source.eye).normalize())
            < 1e-5
    );
    let expected = source.view_projection(1.5);
    assert!(expected.abs_diff_eq(actual.view_projection(1.5), 1e-4));
}
#[test]
fn camera_lens_reference_and_delete_undo_are_validated() {
    let mut editor = Editor::new(camera_scene());
    editor.selected = Some(10);
    editor.delete_selected().unwrap();
    assert!(editor.project.active_camera.is_none());
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project.active_camera, Some(10));
    assert!(editor.project.node(10).unwrap().camera.is_some());
    let duplicate = editor.project.duplicate_subtree(10).unwrap();
    assert_ne!(duplicate, 10);
    assert!(editor.project.node(duplicate).unwrap().camera.is_some());
    assert_eq!(editor.project.active_camera, Some(10));
    editor.project.active_camera = Some(999);
    assert!(editor.project.validate().is_err());
    editor.project.active_camera = Some(10);
    editor
        .project
        .node_mut(10)
        .unwrap()
        .camera
        .as_mut()
        .unwrap()
        .fov_y_radians = f32::NAN;
    assert!(editor.project.validate().is_err());
}
#[test]
fn camera_node_and_active_selection_roundtrip() {
    let mut project = camera_scene();
    project.node_mut(10).unwrap().camera = Some(CameraLens {
        orthographic: true,
        ortho_size: 1.25,
        ..CameraLens::default()
    });
    let loaded: Project = serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
    assert_eq!(loaded, project);
    loaded.validate().unwrap();
    assert!(
        loaded
            .render_camera(&loaded.global_transforms().unwrap())
            .orthographic
    );
}
