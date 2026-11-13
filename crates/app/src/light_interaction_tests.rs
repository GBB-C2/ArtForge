//! Real pointer/action routing for scene lights, without a native window or GPU.
use super::*;
use poseforge_core::{Light, LightKind, Node};

fn light_app() -> App {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.preferences.left_panel = true;
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app.scale = 1.;
    app.target = Vec3::ZERO;
    app.distance = 6.;
    let project = Project {
        nodes: vec![Node::new(1, "Объект, который нужно сохранить")],
        lights: vec![
            Light {
                name: "Точечный".into(),
                kind: LightKind::Point,
                intensity: 40.,
                direction: Vec3::new(0.25, -1., -0.5),
                ..Light::default()
            },
            Light {
                name: "Прожектор".into(),
                kind: LightKind::Spot,
                intensity: 60.,
                direction: Vec3::new(-0.3, -1., -0.5).normalize(),
                ..Light::default()
            },
        ],
        ..Project::default()
    };
    app.editor = Editor::new(project);
    app.editor.mode = EditorMode::Scene;
    app.editor.selected = Some(1);
    app.reset_workflow_selection();
    let view = app.view();
    let forward = (view.target - view.eye).normalize();
    let right = forward.cross(view.up).normalize();
    let up = right.cross(forward).normalize();
    // Keep source markers outside the selected object's 96 px view ring.
    // A visible object handle intentionally owns a coincident pointer hit.
    app.editor.project.lights[0].position = right * -0.9 + up * 0.35;
    app.editor.project.lights[1].position = right * 0.9 + up * 0.35;
    app.saved_snapshot = app.editor.project.clone();
    app.dirty = false;
    app
}

fn spot_grip_app(ortho: bool, scale: f32) -> App {
    let mut app = light_app();
    app.orthographic = ortho;
    app.scale = scale;
    app.select_light(1);
    let view = app.view();
    let forward = (view.target - view.eye).normalize();
    let right = forward.cross(view.up).normalize();
    let up = right.cross(forward).normalize();
    let light = &mut app.editor.project.lights[1];
    light.position = Vec3::ZERO;
    light.direction = (right * 0.8 - up * 0.25 + forward * 0.4).normalize();
    light.inner_cone_angle = 0.2;
    light.outer_cone_angle = 0.65;
    app.ui = app.build_ui();
    app
}

#[test]
fn spotlight_grips_are_pickable_cancel_exactly_and_commit_once_at_each_projection_and_dpi() {
    use super::light_shape_controls::LightGrip;
    for ortho in [false, true] {
        for scale in [1., 2.] {
            for grip in [LightGrip::Direction, LightGrip::Inner, LightGrip::Outer] {
                let mut app = spot_grip_app(ortho, scale);
                let point = app
                    .light_shape_handles()
                    .into_iter()
                    .find(|(g, _)| *g == grip)
                    .unwrap()
                    .1;
                let original = app.editor.project.clone();
                app.mouse_move(point);
                app.mouse_down(MouseButton::Left);
                assert!(matches!(app.drag, Some(Drag::LightShape(_))), "{grip:?}");
                app.mouse_move(point);
                app.mouse_up(MouseButton::Left);
                assert_eq!(
                    app.editor.project, original,
                    "grabbing without motion is exact"
                );
                assert_eq!(app.editor.undo_len(), 0);
                app.mouse_down(MouseButton::Left);
                app.mouse_move([point[0] + 28., point[1] + 18.]);
                assert_ne!(app.editor.project.lights[1], original.lights[1], "{grip:?}");
                app.action(Action::Cancel);
                assert_eq!(app.editor.project, original);
                assert_eq!(app.editor.undo_len(), 0);
                app.mouse_move(point);
                app.mouse_down(MouseButton::Left);
                app.mouse_move([point[0] + 28., point[1] + 18.]);
                app.mouse_up(MouseButton::Left);
                let changed = app.editor.project.clone();
                changed.validate().unwrap();
                assert_eq!(app.editor.undo_len(), 1);
                assert_eq!(changed.lights[1].position, original.lights[1].position);
                assert_eq!(changed.lights[1].color, original.lights[1].color);
                assert_eq!(changed.lights[0], original.lights[0]);
                if grip == LightGrip::Direction {
                    assert!((changed.lights[1].direction.length() - 1.).abs() < 1e-5);
                    assert_eq!(
                        changed.lights[1].outer_cone_angle,
                        original.lights[1].outer_cone_angle
                    );
                } else {
                    assert_eq!(changed.lights[1].direction, original.lights[1].direction);
                }
                app.action(Action::Undo);
                assert_eq!(app.editor.project, original);
                app.action(Action::Redo);
                assert_eq!(app.editor.project, changed);
            }
        }
    }
}

#[test]
fn spotlight_grips_preserve_limits_and_fine_drag_does_not_jump_on_modifier_change() {
    use super::light_shape_controls::LightGrip;
    let mut app = spot_grip_app(false, 1.);
    let point = app
        .light_shape_handles()
        .into_iter()
        .find(|(g, _)| *g == LightGrip::Outer)
        .unwrap()
        .1;
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    app.mouse_move([point[0] + 10., point[1]]);
    let intermediate = app.editor.project.clone();
    app.modifiers = ModifiersState::SHIFT;
    app.mouse_move([point[0] + 10., point[1]]);
    assert_eq!(app.editor.project, intermediate);
    app.modifiers = ModifiersState::CONTROL;
    app.mouse_move([point[0] + 10000., point[1] + 10000.]);
    app.editor.project.validate().unwrap();
    let l = &app.editor.project.lights[1];
    assert!(l.outer_cone_angle > l.inner_cone_angle);
    assert!(l.outer_cone_angle <= std::f32::consts::FRAC_PI_2);
    app.action(Action::SurfaceTab(1));
    assert!(app.drag.is_none());
    assert_eq!(app.editor.undo_len(), 0, "tool switches cancel the gesture");
    app.select_light(0);
    assert!(
        app.light_shape_handles().is_empty(),
        "point lights have no aiming handle"
    );
}

#[test]
fn light_direction_sliders_are_visible_draggable_and_share_numeric_undo() {
    for field in [14, 15] {
        let mut app = spot_grip_app(false, 2.);
        app.preferences.collapsed_sections.insert(900);
        let action = Action::SurfaceLightNumber(field);
        let mut hit = None;
        let max = app.inspector_max_scroll();
        for i in 0..=30 {
            app.inspector_scroll = max * i as f32 / 30.;
            app.ui = app.build_ui();
            hit = app
                .ui
                .hits
                .iter()
                .find(|h| {
                    matches!(&h.action, Action::Slide(a, _, _) if **a == action) && h.rect[3] >= 14.
                })
                .cloned();
            if hit.is_some() {
                break;
            }
        }
        let hit = hit.unwrap();
        let Action::Slide(_, range, value) = hit.action else {
            unreachable!()
        };
        let point = [
            hit.rect[0] + hit.rect[2] * range.ratio(value),
            hit.rect[1] + hit.rect[3] * 0.5,
        ];
        let before = app.editor.project.clone();
        app.mouse_move(point);
        app.mouse_down(MouseButton::Left);
        assert!(matches!(app.drag, Some(Drag::Slider(_))));
        app.mouse_move([point[0] + 15., point[1]]);
        app.mouse_up(MouseButton::Left);
        assert_ne!(
            app.editor.project.lights[1].direction,
            before.lights[1].direction
        );
        assert_eq!(app.editor.undo_len(), 1);
        app.editor.project.validate().unwrap();
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        assert!(app.surface_confirm(action.clone(), "nan").is_err());
        assert_eq!(app.editor.project, before);
        app.surface_confirm(action, "45").unwrap();
        assert_eq!(app.editor.undo_len(), 1);
        assert!((app.editor.project.lights[1].direction.length() - 1.).abs() < 1e-5);
    }
}

fn rebuild(app: &mut App) {
    app.ui = app.build_ui();
}

fn marker(app: &App, index: usize) -> [f32; 2] {
    let point = app
        .project_point(app.editor.project.lights[index].position)
        .expect("Light marker must project in front of the camera");
    assert!(ui::contains(app.viewport(), point));
    point
}

fn click_marker(app: &mut App, index: usize) {
    rebuild(app);
    let point = marker(app, index);
    assert!(
        app.ui.hit(point).is_none(),
        "A UI control covers the light marker"
    );
    assert!(
        app.selected_gizmo()
            .and_then(|gizmo| gizmo.hit(point))
            .is_none(),
        "Fixture marker overlaps the selected object's gizmo"
    );
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    app.mouse_up(MouseButton::Left);
}

fn click_action(app: &mut App, action: Action) {
    rebuild(app);
    if !app.ui.hits.iter().any(|h| h.action == action) {
        let max = app.inspector_max_scroll();
        for step in 0..=((max / 100.).ceil() as usize) {
            app.inspector_scroll = (step as f32 * 100.).min(max);
            rebuild(app);
            if app
                .ui
                .hits
                .iter()
                .any(|h| h.action == action && h.rect[3] >= 28.)
            {
                break;
            }
        }
    }
    let rect = app
        .ui
        .hits
        .iter()
        .rev()
        .find(|hit| hit.action == action)
        .unwrap_or_else(|| panic!("Expected a visible control for {action:?}"))
        .rect;
    let point = [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5];
    assert_eq!(app.ui.control_at(point), Some(action));
    app.mouse_move(point);
    app.click_ui_control();
}

fn begin_center_drag(app: &mut App, index: usize) -> [f32; 2] {
    assert_eq!(app.active_light_index(), Some(index));
    rebuild(app);
    let point = marker(app, index);
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Light(_))));
    point
}

#[test]
fn first_marker_click_selects_light_without_moving_it_when_inspector_opens() {
    let mut app = light_app();
    app.preferences.right_panel = false;
    let original = app.editor.project.clone();
    let before_viewport = app.viewport();
    rebuild(&mut app);
    let point = marker(&app, 0);
    assert!(app
        .selected_gizmo()
        .and_then(|gizmo| gizmo.hit(point))
        .is_none());
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    assert_eq!(app.active_light_index(), Some(0));
    assert_eq!(app.editor.selected, None);
    assert_eq!(app.surface.tab, 0);
    assert!(app.preferences.right_panel);
    assert!(!app.animation.focused);
    assert!(app.tool == Tool::Move);
    assert!(
        app.drag.is_none(),
        "First selection must not capture a changing viewport"
    );
    assert_eq!(app.viewport(), before_viewport);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
    // The second press uses the selected light and owns the gesture.
    begin_center_drag(&mut app, 0);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn selected_object_ring_keeps_pointer_priority_when_a_light_marker_overlaps_it() {
    let mut app = light_app();
    let view = app.view();
    let forward = (view.target - view.eye).normalize();
    let right = forward.cross(view.up).normalize();
    let radius = 2. * app.distance * (view.fov_y * 0.5).tan() * 96. / app.viewport()[3];
    app.editor.project.lights[0].position = right * radius;
    app.saved_snapshot = app.editor.project.clone();
    let original = app.editor.project.clone();
    rebuild(&mut app);
    let point = marker(&app, 0);
    assert!(app.ui.hit(point).is_none());
    assert!(app
        .selected_gizmo()
        .and_then(|gizmo| gizmo.hit(point))
        .is_some());
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Gizmo { .. })));
    assert_eq!(app.editor.selected, Some(1));
    assert_eq!(app.active_light_index(), None);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn light_drag_previews_preserve_direction_and_return_to_start_before_one_undo() {
    let mut app = light_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, 0);
    let from = begin_center_drag(&mut app, 0);
    app.mouse_move([from[0] + 20., from[1] + 12.]);
    assert_ne!(
        app.editor.project.lights[0].position,
        original.lights[0].position
    );
    assert_eq!(
        app.editor.project.lights[0].direction,
        original.lights[0].direction
    );
    assert_eq!(app.editor.undo_len(), 0);
    app.mouse_move(from);
    assert_eq!(
        app.editor.project, original,
        "A return to the exact start must be a no-op"
    );
    assert!(!app.dirty);
    app.mouse_move([from[0] + 35., from[1] - 16.]);
    app.mouse_up(MouseButton::Right);
    assert!(matches!(app.drag, Some(Drag::Light(_))));
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);
    assert!(app.dirty);
    assert_eq!(app.editor.project.nodes, original.nodes);
    assert_eq!(app.editor.project.lights[1], original.lights[1]);
    assert_eq!(
        app.editor.project.lights[0].direction,
        original.lights[0].direction
    );
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
    assert!(!app.dirty);
}

#[test]
fn escape_and_capture_cancellation_restore_light_without_touching_scene_or_history() {
    let mut app = light_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, 0);
    let from = begin_center_drag(&mut app, 0);
    app.mouse_move([from[0] + 28., from[1] + 10.]);
    assert_ne!(app.editor.project, original);
    app.action(Action::Cancel);
    assert!(app.drag.is_none());
    assert!(app.drag_button.is_none());
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
    assert_eq!(app.active_light_index(), Some(0));
    assert_eq!(app.surface.tab, 0);

    let from = begin_center_drag(&mut app, 0);
    app.mouse_move([from[0] - 22., from[1] + 16.]);
    // CursorLeft, focus loss, resize and DPI changes share this cancellation path.
    app.cancel_drag();
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(app.drag.is_none() && app.drag_button.is_none());
}

#[test]
fn delete_owns_selected_light_and_node_selection_releases_that_ownership() {
    let mut app = light_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, 0);
    app.action(Action::Delete);
    assert_eq!(app.editor.project.lights, vec![original.lights[1].clone()]);
    assert_eq!(app.editor.project.nodes, original.nodes);
    assert_eq!(app.editor.undo_len(), 1);
    assert_eq!(app.active_light_index(), None);
    // A second Delete cannot fall back to the object selected before the light.
    app.action(Action::Delete);
    assert_eq!(app.editor.project.nodes, original.nodes);
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);

    click_marker(&mut app, 0);
    app.action(Action::Select(1));
    assert_eq!(app.active_light_index(), None);
    assert_eq!(app.editor.selected, Some(1));
    app.action(Action::Delete);
    assert!(app.editor.project.nodes.is_empty());
    assert_eq!(app.editor.project.lights, original.lights);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
}

#[test]
fn added_and_repositioned_lights_remain_visible_and_aim_at_current_view() {
    let mut app = light_app();
    let original = app.editor.project.clone();
    click_action(
        &mut app,
        Action::DropdownOpen(ui::DropdownKind::LightCreate),
    );
    click_action(&mut app, Action::DropdownChoose(1));
    let index = original.lights.len();
    assert_eq!(app.active_light_index(), Some(index));
    assert_eq!(app.editor.selected, None);
    assert_eq!(app.editor.project.lights[index].kind, LightKind::Point);
    let added = app.editor.project.lights[index].clone();
    assert!(added.position.distance(app.view().eye) > 0.1);
    assert!(ui::contains(app.viewport(), marker(&app, index)));
    assert!((added.direction - (app.target - added.position).normalize()).length() < 1e-5);
    assert_eq!(app.editor.undo_len(), 1);
    // A source spawned by the toolbar is immediately selected and draggable.
    begin_center_drag(&mut app, index);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);

    app.yaw += 0.5;
    app.action(Action::SurfaceAimLight); // Internal compatibility command; inspector button was removed.
    let aimed = &app.editor.project.lights[index];
    assert_ne!(aimed.position, added.position);
    assert!((aimed.direction - (app.target - aimed.position).normalize()).length() < 1e-5);
    assert!(ui::contains(app.viewport(), marker(&app, index)));
    assert_eq!(app.editor.undo_len(), 2);
    app.action(Action::Undo);
    assert_eq!(app.editor.project.lights[index], added);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
}

#[test]
fn spot_rotation_uses_visible_ring_and_preserves_position_as_one_edit() {
    let mut app = light_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, 1);
    app.action(Action::Rotate);
    assert!(app.tool == Tool::Rotate);
    rebuild(&mut app);
    let pivot = marker(&app, 1);
    // The outer 96 px view ring is a real gizmo handle, separate from the axes.
    let from = [pivot[0] + 96., pivot[1]];
    let to = [pivot[0], pivot[1] + 96.];
    assert!(ui::contains(app.viewport(), from) && ui::contains(app.viewport(), to));
    assert!(app.ui.hit(from).is_none());
    app.mouse_move(from);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Light(_))));
    app.mouse_move(to);
    assert_eq!(
        app.editor.project.lights[1].position,
        original.lights[1].position
    );
    assert!((app.editor.project.lights[1].direction - original.lights[1].direction).length() > 0.1);
    assert!((app.editor.project.lights[1].direction.length() - 1.).abs() < 1e-5);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);
    assert_eq!(app.editor.project.nodes, original.nodes);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
}

#[test]
fn converting_a_rotated_spot_to_point_restores_move_tool_and_prevents_misleading_rotation() {
    let mut app = light_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, 1);
    app.action(Action::Rotate);
    assert!(app.tool == Tool::Rotate);
    click_action(&mut app, Action::DropdownOpen(ui::DropdownKind::LightKind));
    click_action(&mut app, Action::DropdownChoose(0));
    assert_eq!(app.editor.project.lights[1].kind, LightKind::Directional);
    assert!(app.tool == Tool::Rotate);
    click_action(&mut app, Action::DropdownOpen(ui::DropdownKind::LightKind));
    click_action(&mut app, Action::DropdownChoose(1));
    assert_eq!(app.editor.project.lights[1].kind, LightKind::Point);
    assert!(app.tool == Tool::Move);
    assert_eq!(app.active_light_index(), Some(1));
    assert_eq!(app.editor.undo_len(), 2);
    let point_state = app.editor.project.clone();
    app.action(Action::Rotate);
    assert!(app.tool == Tool::Move);
    assert_eq!(app.editor.project, point_state);
    assert_eq!(app.editor.undo_len(), 2);
    assert_eq!(app.editor.project.nodes, original.nodes);
    assert_eq!(
        app.editor.project.lights[1].position,
        original.lights[1].position
    );
}

#[test]
fn render_mode_allows_light_pick_and_drag_in_both_themes_at_minimum_size_and_dpi() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 1.5, 2.] {
            let mut app = light_app();
            app.preferences.theme = theme;
            app.scale = scale;
            app.editor.mode = EditorMode::Render;
            let original = app.editor.project.clone();
            click_marker(&mut app, 1);
            assert_eq!(app.active_light_index(), Some(1));
            assert_eq!(app.editor.mode, EditorMode::Render);
            assert_eq!(app.editor.selected, None);
            assert_eq!(app.editor.project, original);
            let from = begin_center_drag(&mut app, 1);
            app.mouse_move([from[0] + 18., from[1] + 8.]);
            app.mouse_up(MouseButton::Left);
            assert_ne!(
                app.editor.project.lights[1].position,
                original.lights[1].position
            );
            assert_eq!(app.editor.project.nodes, original.nodes);
            assert_eq!(app.editor.undo_len(), 1);
            assert_eq!(app.editor.mode, EditorMode::Render);
            app.action(Action::Undo);
            assert_eq!(app.editor.project, original);
        }
    }
}

#[test]
fn temperature_name_and_area_rotation_commit_with_undo_and_aim_preserves_position() {
    let mut app = light_app();
    app.editor.project.lights[1].kind = LightKind::Area;
    app.select_light(1);
    let before = app.editor.project.clone();
    click_action(&mut app, Action::SurfaceLightTemperature);
    assert!(app.editor.project.lights[1].use_temperature);
    app.action(Action::SurfaceLightNumber(12));
    let input = app.input.as_mut().unwrap();
    input.select_all();
    input.insert("3200");
    app.action(Action::ConfirmInput);
    assert_eq!(app.editor.project.lights[1].temperature_kelvin, 3200.);
    assert!(app
        .surface_confirm(Action::SurfaceLightNumber(12), "999")
        .is_err());
    app.surface_confirm(Action::SurfaceLightNumber(13), "90")
        .unwrap();
    app.action(Action::SurfaceLightName);
    let input = app.input.as_mut().unwrap();
    input.select_all();
    input.insert("Тёплый ключевой");
    app.action(Action::ConfirmInput);
    assert_eq!(app.editor.project.lights[1].name, "Тёплый ключевой");
    let position = app.editor.project.lights[1].position;
    app.target = Vec3::new(-2., 1., 0.);
    app.action(Action::SurfacePointLightAtView);
    let light = &app.editor.project.lights[1];
    assert_eq!(light.position, position);
    assert!(
        light
            .direction
            .distance((app.target - position).normalize())
            < 1e-6
    );
    for _ in 0..5 {
        app.action(Action::Undo);
    }
    assert_eq!(app.editor.project, before);
}
