use super::*;

#[test]
fn camera_view_creates_from_current_view_once_and_undo_removes_it() {
    for orthographic in [false, true] {
        let mut app = App::new(None);
        app.editor = Editor::new(Project::default());
        app.editor.mode = EditorMode::Scene;
        app.orthographic = orthographic;
        app.yaw = 0.73;
        app.pitch = 0.29;
        app.target = Vec3::new(1., 2., -3.);
        let original = app.editor.project.clone();
        let view = app.view();
        let ui = app.build_ui();
        assert!(ui.hits.iter().any(|h| h.action == Action::CameraView));
        app.action(Action::CameraView);
        let id = app.camera_view.expect("one-click camera view");
        let camera = app
            .editor
            .project
            .camera_from_node(id, &app.editor.evaluated().unwrap().global)
            .unwrap();
        assert!(camera.eye.distance(view.eye) < 1e-5);
        assert!(
            (camera.target - camera.eye)
                .normalize()
                .distance((view.target - view.eye).normalize())
                < 1e-5
        );
        assert_eq!(camera.orthographic, orthographic);
        if let Some(size) = view.orthographic {
            assert!((camera.ortho_size - size).abs() < 1e-5);
        }
        assert_eq!(app.editor.project.active_camera, Some(id));
        app.action(Action::CameraView);
        assert!(app.camera_view.is_none());
        app.action(Action::CameraView);
        assert_eq!(app.camera_view, Some(id));
        assert_eq!(
            app.editor
                .project
                .nodes
                .iter()
                .filter(|n| n.camera.is_some())
                .count(),
            1
        );
        app.action(Action::Undo);
        app.build_ui();
        assert_eq!(app.editor.project, original);
        assert!(app.camera_view.is_none());
        app.action(Action::Redo);
        assert!(app.editor.project.node(id).unwrap().camera.is_some());
    }
}

#[test]
fn camera_view_reuses_an_existing_camera_without_an_active_camera() {
    let mut app = App::new(None);
    app.action(Action::CreateCamera);
    let id = app.editor.selected.unwrap();
    app.editor.selected = None;
    app.editor.project.active_camera = None;
    let original = app.editor.project.clone();
    app.action(Action::CameraView);
    assert_eq!(app.camera_view, Some(id));
    assert_eq!(app.editor.project, original);
}

#[test]
fn creating_camera_is_visible_selectable_and_undoable() {
    let mut app = App::new(None);
    let count = app.editor.project.nodes.len();
    app.action(Action::CreateCamera);
    let id = app.selected_camera_id().expect("created camera selected");
    assert_eq!(app.editor.project.nodes.len(), count + 1);
    assert_eq!(app.editor.project.active_camera, Some(id));
    assert!(app.camera_view.is_none());
    let camera = app
        .editor
        .project
        .camera_from_node(id, &app.editor.evaluated().unwrap().global)
        .unwrap();
    let position = app
        .project_point(camera.eye)
        .expect("new camera visible in editor");
    assert!(app.canvas_input_at(position));
    let mut ui = Ui::with_theme(1., ui::Theme::Dark, [0., 0.]);
    app.draw_camera_overlay(&mut ui);
    assert!(ui.rects.len() > 20);
    app.editor.selected = None;
    app.mouse = position;
    assert!(app.camera_pointer_down());
    assert_eq!(app.editor.selected, Some(id));
    app.action(Action::Undo);
    assert!(app.editor.project.node(id).is_none());
    assert!(app.editor.project.active_camera.is_none());
    app.action(Action::Redo);
    assert!(app.editor.project.node(id).unwrap().camera.is_some());
}
#[test]
fn camera_lens_fields_view_and_free_navigation_preserve_history() {
    let mut app = App::new(None);
    app.action(Action::CreateCamera);
    let id = app.selected_camera_id().unwrap();
    let before = app.editor.project.node(id).unwrap().camera.unwrap();
    assert!(app
        .camera_confirm(&Action::CameraNumber(0), "62,5")
        .unwrap());
    assert!(
        (app.editor
            .project
            .node(id)
            .unwrap()
            .camera
            .unwrap()
            .fov_y_radians
            .to_degrees()
            - 62.5)
            .abs()
            < 1e-4
    );
    assert!(app.camera_confirm(&Action::CameraNumber(0), "nan").is_err());
    app.action(Action::Undo);
    assert_eq!(app.editor.project.node(id).unwrap().camera.unwrap(), before);
    app.action(Action::CameraView);
    let view = app.view();
    let snapshot = app.editor.project.clone();
    assert!(app.camera_view.is_some());
    app.leave_camera_view();
    assert!(app.camera_view.is_none());
    assert_eq!(snapshot, app.editor.project);
    assert!(app.view().eye.distance(view.eye) < 1e-4);
}
#[test]
fn camera_can_move_with_existing_gizmo_then_cancel_or_commit_once() {
    let mut app = App::new(None);
    app.action(Action::CreateCamera);
    let id = app.selected_camera_id().unwrap();
    let original = app.editor.project.node(id).unwrap().transform;
    app.tool = Tool::Move;
    let gizmo = app.selected_gizmo().unwrap();
    app.mouse = app.project_point(gizmo.origin).unwrap();
    app.begin_gizmo_drag(crate::gizmo::Handle::Center);
    let Some(Drag::Gizmo { gesture, .. }) = &app.drag else {
        panic!("camera uses ordinary gizmo");
    };
    let _ = gesture;
    app.mouse = [app.mouse[0] + 30., app.mouse[1] + 8.];
    app.mouse_move(app.mouse);
    assert_ne!(app.editor.project.node(id).unwrap().transform, original);
    app.cancel_drag();
    assert_eq!(app.editor.project.node(id).unwrap().transform, original);
}
#[test]
fn camera_export_uses_evaluated_world_position_and_preserves_free_view_fallback() {
    let mut app = App::new(None);
    app.action(Action::CreateCamera);
    let id = app.selected_camera_id().unwrap();
    let fallback = app.view();
    let mut globals = app.editor.evaluated().unwrap().global;
    globals.insert(id, Mat4::from_translation(Vec3::new(4., 5., 6.)));
    let render = camera_controls::export_camera_view(&app.editor.project, &globals, &fallback);
    assert_eq!(render.eye, Vec3::new(4., 5., 6.));
    app.editor.project.active_camera = None;
    let render = camera_controls::export_camera_view(&app.editor.project, &globals, &fallback);
    assert_eq!(render.eye, fallback.eye);
}

#[test]
fn camera_safe_frame_matches_export_crop_for_wide_and_portrait_outputs() {
    let mut app = App::new(None);
    app.action(Action::CreateCamera);
    let id = app.selected_camera_id().unwrap();
    for scale in [1., 2.] {
        app.scale = scale;
        for (width, height) in [(1920, 1080), (800, 1200), (2400, 400)] {
            app.editor.project.render_settings.output_width = width;
            app.editor.project.render_settings.output_height = height;
            for ortho in [false, true] {
                app.editor
                    .project
                    .node_mut(id)
                    .unwrap()
                    .camera
                    .as_mut()
                    .unwrap()
                    .orthographic = ortho;
                app.camera_view = Some(id);
                let camera = app
                    .editor
                    .project
                    .camera_from_node(id, &app.editor.evaluated().unwrap().global)
                    .unwrap();
                let view = app.camera_object_view().unwrap();
                let vp = view.viewport;
                let aspect = width as f32 / height as f32;
                let fw = vp[2].min(vp[3] * aspect);
                let fh = vp[3].min(vp[2] / aspect);
                let left = vp[0] + (vp[2] - fw) / 2.;
                let top = vp[1] + (vp[3] - fh) / 2.;
                let expected = View {
                    effects: Default::default(),
                    depth_of_field: Default::default(),
                    eye: camera.eye,
                    target: camera.target,
                    up: camera.up,
                    fov_y: camera.fov_y_radians,
                    viewport: [0., 0., width as f32, height as f32],
                    orthographic: ortho.then_some(camera.ortho_size),
                };
                let forward = (camera.target - camera.eye).normalize();
                let right = forward.cross(camera.up).normalize();
                let point = camera.eye + forward * 3. + right * 0.15 + camera.up * 0.1;
                let actual = view.project(point).unwrap();
                let reference = expected.project(point).unwrap();
                assert!(((actual[0] - left) / fw - reference[0] / width as f32).abs() < 1e-4);
                assert!(((actual[1] - top) / fh - reference[1] / height as f32).abs() < 1e-4);
            }
        }
    }
}

#[test]
fn camera_frame_stays_between_panels_and_above_the_floating_timeline() {
    for (width, height) in [(1100., 620.), (1440., 900.)] {
        for scale in [1., 2.] {
            for right_panel in [false, true] {
                for expanded in [false, true] {
                    let mut app = App::new(None);
                    app.preferences = Preferences::default();
                    app.preferences.right_panel = right_panel;
                    app.width = width;
                    app.height = height;
                    app.scale = scale;
                    app.action(Action::CreateCamera);
                    app.action(Action::CameraView);
                    app.editor.mode = EditorMode::Animation;
                    app.timeline.expanded = expanded;
                    let v = app
                        .camera_object_view()
                        .unwrap()
                        .viewport
                        .map(|v| v / scale);
                    let left = app.left_panel_rect();
                    assert!(v[0] >= left[0] + left[2] + 10.);
                    assert!(v[1] + v[3] < app.animation_dock_rect()[1]);
                    if right_panel {
                        assert!(v[0] + v[2] < app.right_panel_rect()[0]);
                    }
                    let u = app.build_ui();
                    assert!(!u.hits.iter().any(|h| h.action == Action::CameraExitView));
                    assert!(!u.texts.iter().any(|t| t.text.contains("Свободный вид")));
                    let lock = Action::Studio(ui::StudioAction::CameraLock);
                    let r = u.hits.iter().find(|h| h.action == lock).unwrap().rect;
                    assert_eq!(r[2], r[3]);
                    assert_eq!(
                        u.hit([r[0] + r[2] / 2., r[1] + r[3] / 2.]),
                        Some(lock.clone())
                    );
                    assert_ne!(u.hit([r[0], r[1]]), Some(lock));
                    let view = u
                        .hits
                        .iter()
                        .find(|h| h.action == Action::CameraView)
                        .unwrap()
                        .rect;
                    assert_eq!(view[2..], [32., 32.]);
                }
            }
        }
    }
}
