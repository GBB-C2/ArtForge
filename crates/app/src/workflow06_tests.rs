//! Cross-feature regressions for camera nodes, light rows and popup input ownership.
use super::*;

fn app_with_camera() -> (App, u64) {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.preferences.left_panel = true;
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app.action(Action::CreateCamera);
    let camera = app.selected_camera_id().expect("new camera selected");
    (app, camera)
}

#[test]
fn switching_camera_light_and_node_keeps_destructive_commands_on_the_selected_type() {
    let (mut app, camera) = app_with_camera();
    let nodes = app.editor.project.nodes.clone();
    let light_count = app.editor.project.lights.len();
    let expected = Box::new(app.editor.project.lights[0].clone());
    app.action(Action::SceneSelectLight(0, expected));
    assert_eq!(app.editor.selected, None);
    app.action(Action::Duplicate);
    assert_eq!(app.editor.project.nodes, nodes);
    assert_eq!(app.editor.project.lights.len(), light_count + 1);
    app.action(Action::Select(camera));
    assert!(app.active_light_index().is_none());
    app.action(Action::Duplicate);
    let duplicate = app.editor.selected.unwrap();
    assert_ne!(duplicate, camera);
    assert!(app.editor.project.node(duplicate).unwrap().camera.is_some());
    let lights = app.editor.project.lights.clone();
    app.action(Action::Delete);
    assert!(app.editor.project.node(duplicate).is_none());
    assert!(app.editor.project.node(camera).is_some());
    assert_eq!(app.editor.project.active_camera, Some(camera));
    assert_eq!(app.editor.project.lights, lights);
    app.action(Action::Undo);
    assert!(app.editor.project.node(duplicate).is_some());
    app.editor.project.validate().unwrap();
}

#[test]
fn dismissing_light_dropdown_over_camera_row_does_not_select_or_move_camera() {
    let (mut app, camera) = app_with_camera();
    let expected = Box::new(app.editor.project.lights[0].clone());
    app.action(Action::SceneSelectLight(0, expected));
    app.ui = app.build_ui();
    let camera_row = app
        .ui
        .hits
        .iter()
        .find(|hit| hit.action == Action::Select(camera))
        .unwrap()
        .rect;
    app.action(Action::DropdownOpen(ui::DropdownKind::LightKind));
    assert!(app.dropdown.is_some());
    let original = app.editor.project.clone();
    app.ui = app.build_ui();
    app.mouse_move([
        camera_row[0] + camera_row[2] * 0.5,
        camera_row[1] + camera_row[3] * 0.5,
    ]);
    app.mouse_down(MouseButton::Left);
    app.mouse_up(MouseButton::Left);
    assert!(app.dropdown.is_none());
    assert_eq!(app.editor.selected, None);
    assert_eq!(app.active_light_index(), Some(0));
    assert_eq!(app.editor.project, original);
}

#[test]
fn camera_lens_history_and_light_visibility_are_independent() {
    let (mut app, camera) = app_with_camera();
    let original_lens = app.editor.project.node(camera).unwrap().camera.unwrap();
    assert!(app
        .camera_confirm(&Action::CameraNumber(0), "63,5")
        .unwrap());
    let edited_lens = app.editor.project.node(camera).unwrap().camera.unwrap();
    assert!((edited_lens.fov_y_radians.to_degrees() - 63.5).abs() < 0.001);
    let original_enabled = app.editor.project.lights[0].enabled;
    let expected = Box::new(app.editor.project.lights[0].clone());
    app.action(Action::SceneToggleLight(0, expected));
    assert_eq!(app.editor.selected, Some(camera));
    assert_ne!(app.editor.project.lights[0].enabled, original_enabled);
    app.action(Action::Undo);
    assert_eq!(app.editor.project.lights[0].enabled, original_enabled);
    assert_eq!(
        app.editor.project.node(camera).unwrap().camera,
        Some(edited_lens)
    );
    app.action(Action::Undo);
    assert_eq!(
        app.editor.project.node(camera).unwrap().camera,
        Some(original_lens)
    );
    app.editor.project.validate().unwrap();
}

#[test]
fn returning_export_to_free_view_exits_camera_preview_without_moving_camera() {
    let (mut app, camera) = app_with_camera();
    app.action(Action::CameraView);
    let original = app.editor.project.node(camera).unwrap().clone();
    let view = app.view();
    app.action(Action::CameraClearActive);
    assert!(app.editor.project.active_camera.is_none());
    assert!(app.camera_view.is_none());
    assert_eq!(app.editor.project.node(camera), Some(&original));
    assert!(app.view().eye.distance(view.eye) < 0.0001);
    app.action(Action::Undo);
    assert_eq!(app.editor.project.active_camera, Some(camera));
    assert_eq!(app.editor.project.node(camera), Some(&original));
}
