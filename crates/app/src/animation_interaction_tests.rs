//! App-level animation gestures: real hit routing, transactional edits and preview state.
//! These checks use the same logical pointer coordinates and shortcut resolver as
//! the window event handler, without constructing platform-private KeyEvents.
use super::*;
use crate::animation_timeline::{TimelineControl as Control, TimelineHit};
use poseforge_core::{Channel, Clip, Interpolation, Keyframe, Node, Property};

fn animation_app() -> App {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.preferences.left_panel = true;
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app.scale = 1.;
    let mut project = Project {
        fps: 24,
        duration: 4.,
        nodes: vec![Node::new(1, "Таз"), Node::new(2, "Кисть")],
        ..Project::default()
    };
    let mut first = Clip::new("Исходный", 4.);
    for property in [Property::Translation, Property::Rotation] {
        first.channels.push(Channel {
            node: 1,
            property,
            interpolation: Interpolation::Linear,
            keys: [6., 18., 36.25]
                .into_iter()
                .map(|frame| {
                    let value = if property == Property::Rotation {
                        vec![0., 0., 0., 1.]
                    } else {
                        vec![frame as f32 * 0.01, 0., 0.]
                    };
                    Keyframe::new(frame / 24., value)
                })
                .collect(),
        });
    }
    first.channels.push(Channel {
        node: 2,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        keys: [12., 30.]
            .into_iter()
            .map(|frame| Keyframe::new(frame / 24., vec![0., frame as f32 * 0.01, 0.]))
            .collect(),
    });
    project.clips = vec![first, Clip::new("Вторая сцена движения", 4.)];
    app.editor = Editor::new(project);
    app.editor.mode = EditorMode::Animation;
    app.editor.selected = Some(1);
    app.reset_workflow_selection();
    app.sync_animation_context();
    app.saved_snapshot = app.editor.project.clone();
    app.dirty = false;
    app
}

fn rebuild(app: &mut App) {
    app.ui = app.build_ui();
}

fn center(rect: [f32; 4]) -> [f32; 2] {
    [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5]
}

fn action_rect(app: &App, action: &Action) -> [f32; 4] {
    app.ui
        .hits
        .iter()
        .rev()
        .find(|hit| &hit.action == action)
        .unwrap_or_else(|| panic!("Expected a reachable animation control: {action:?}"))
        .rect
}

fn click_action(app: &mut App, action: Action) {
    rebuild(app);
    let point = center(action_rect(app, &action));
    assert_eq!(app.ui.control_at(point), Some(action));
    app.mouse_move(point);
    app.click_ui_control();
}

fn marker_point(app: &App, node: Option<u64>, frame: f64) -> [f32; 2] {
    let layout = app.timeline_layout();
    let fps = app.editor.project.fps as f64;
    center(
        layout
            .markers
            .iter()
            .find(|marker| {
                marker.node == node
                    && marker
                        .keys
                        .iter()
                        .any(|key| (key.time * fps - frame).abs() < 1e-7)
            })
            .unwrap_or_else(|| panic!("Expected a marker on track {node:?} at frame {frame}"))
            .rect,
    )
}

fn click_marker(app: &mut App, node: Option<u64>, frame: f64) {
    rebuild(app);
    let point = marker_point(app, node, frame);
    assert!(app.ui.hit(point).is_none(), "A panel covers the key marker");
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    app.mouse_up(MouseButton::Left);
}

fn begin_marker_drag(app: &mut App, node: Option<u64>, frame: f64) -> ([f32; 2], f32) {
    rebuild(app);
    let point = marker_point(app, node, frame);
    let layout = app.timeline_layout();
    let pixels_per_frame = layout.grid[2] / layout.view_span as f32;
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Keys(_))));
    (point, pixels_per_frame)
}

fn drag_frames(app: &mut App, origin: [f32; 2], pixels_per_frame: f32, frames: f32) {
    app.mouse_move([origin[0] + pixels_per_frame * frames, origin[1]]);
}

fn shortcut(app: &mut App, key: Key, modifiers: ModifiersState) -> Action {
    app.modifiers = modifiers;
    let action = app
        .timeline_shortcut(
            &key,
            modifiers.control_key() || modifiers.super_key(),
            modifiers.shift_key(),
        )
        .expect("The focused timeline must handle its keyboard shortcut");
    app.action(action.clone());
    app.modifiers = ModifiersState::empty();
    action
}

fn replace_draft(app: &mut App, value: &str) {
    let input = app.input.as_mut().expect("Expected an inline number field");
    input.select_all();
    input.insert(value);
}

fn edit_number(app: &mut App, control: Control, value: &str) {
    click_action(app, Action::Animation(control));
    assert!(app.dialog.is_none() && !app.animation_panel);
    replace_draft(app, value);
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none(), "Rejected {control:?}: {}", app.status);
}

fn selected_at(app: &App, node: u64, frame: f64) -> usize {
    app.timeline
        .selection
        .iter()
        .filter(|key| key.node == node && (key.time * 24. - frame).abs() < 1e-7)
        .count()
}

#[test]
fn shift_selected_tracks_drag_as_one_edit_and_escape_restores_preview() {
    let mut app = animation_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, Some(1), 6.);
    assert_eq!(app.timeline.selection.len(), 2);
    app.modifiers = ModifiersState::SHIFT;
    click_marker(&mut app, Some(2), 12.);
    app.modifiers = ModifiersState::empty();
    assert_eq!(app.timeline.selection.len(), 3);
    assert_eq!(app.editor.undo_len(), 0);

    let (origin, pixels_per_frame) = begin_marker_drag(&mut app, Some(1), 6.);
    drag_frames(&mut app, origin, pixels_per_frame, 1.);
    drag_frames(&mut app, origin, pixels_per_frame, 4.);
    assert_eq!(selected_at(&app, 1, 10.), 2);
    assert_eq!(selected_at(&app, 2, 16.), 1);
    assert_eq!(
        app.editor.undo_len(),
        0,
        "Pointer previews must not create history"
    );
    app.mouse_up(MouseButton::Right);
    assert!(matches!(app.drag, Some(Drag::Keys(_))));
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);
    assert!(app.dirty);
    assert_eq!(app.editor.project.nodes, original.nodes);
    assert_eq!(
        app.editor.project.clips[0].channels[0].keys[2].time,
        36.25 / 24.
    );

    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
    assert!(!app.dirty);
    click_marker(&mut app, Some(1), 6.);
    let selection = app.timeline.selection.clone();
    let history = app.editor.undo_len();
    let (origin, pixels_per_frame) = begin_marker_drag(&mut app, Some(1), 6.);
    drag_frames(&mut app, origin, pixels_per_frame, 3.);
    assert_ne!(app.editor.project, original);
    app.action(Action::Cancel);
    assert!(app.drag.is_none());
    assert!(app.drag_button.is_none());
    assert_eq!(app.timeline.selection, selection);
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), history);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, original);
}

#[test]
fn retime_collision_rolls_back_previous_valid_drag_preview_without_history() {
    let mut app = animation_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, Some(1), 6.);
    let selection = app.timeline.selection.clone();
    let (origin, pixels_per_frame) = begin_marker_drag(&mut app, Some(1), 6.);
    drag_frames(&mut app, origin, pixels_per_frame, 3.);
    assert_ne!(app.editor.project, original);
    // Frame 18 already has unselected keys in both affected channels.
    drag_frames(&mut app, origin, pixels_per_frame, 12.);
    assert!(matches!(&app.drag, Some(Drag::Keys(drag)) if !drag.valid));
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, original);
    assert_eq!(app.timeline.selection, selection);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
    assert!(app.status.contains("отменён"));
}

#[test]
fn alt_drag_duplicates_keys_and_alt_arrow_retimes_the_copy_only() {
    let mut app = animation_app();
    let original = app.editor.project.clone();
    app.modifiers = ModifiersState::ALT;
    let (origin, pixels_per_frame) = begin_marker_drag(&mut app, Some(1), 6.);
    drag_frames(&mut app, origin, pixels_per_frame, 3.);
    app.mouse_up(MouseButton::Left);
    app.modifiers = ModifiersState::empty();
    assert_eq!(app.editor.undo_len(), 1);
    assert_eq!(selected_at(&app, 1, 9.), 2);
    for channel in &app.editor.project.clips[0].channels[..2] {
        assert_eq!(channel.keys.len(), 4);
        assert_eq!(channel.keys[0].time, 6. / 24.);
        assert_eq!(channel.keys[1].time, 9. / 24.);
        assert_eq!(channel.keys[0].value, channel.keys[1].value);
    }
    assert_eq!(
        shortcut(
            &mut app,
            Key::Named(NamedKey::ArrowRight),
            ModifiersState::ALT
        ),
        Action::Animation(Control::ShiftKeys(1))
    );
    assert_eq!(selected_at(&app, 1, 10.), 2);
    assert_eq!(app.editor.undo_len(), 2);
    assert_eq!(
        app.editor.project.clips[0].channels[0].keys[0].time,
        6. / 24.
    );
    app.action(Action::Undo);
    assert_eq!(
        app.editor.project.clips[0].channels[0].keys[1].time,
        9. / 24.
    );
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
}

#[test]
fn reverse_box_and_shift_toggle_select_keys_across_tracks_without_editing_scene() {
    let mut app = animation_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, Some(2), 30.);
    let layout = app.timeline_layout();
    let first = layout.rows.iter().find(|row| row.node == Some(1)).unwrap();
    let second = layout.rows.iter().find(|row| row.node == Some(2)).unwrap();
    let from = [layout.x_at_frame(20.), second.rect[1] + second.rect[3] - 1.];
    let to = [layout.x_at_frame(4.), first.rect[1] + 1.];
    assert!(matches!(layout.hit(from), Some(TimelineHit::Empty(_))));
    app.modifiers = ModifiersState::SHIFT;
    app.mouse_move(from);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::KeyBox(_))));
    app.mouse_move(to);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.timeline.selection.len(), 6);
    assert_eq!(selected_at(&app, 1, 6.), 2);
    assert_eq!(selected_at(&app, 1, 18.), 2);
    assert_eq!(selected_at(&app, 2, 12.), 1);
    assert_eq!(selected_at(&app, 2, 30.), 1);
    // Shift-clicking an already selected grouped marker toggles its channels off.
    click_marker(&mut app, Some(1), 18.);
    app.modifiers = ModifiersState::empty();
    assert_eq!(app.timeline.selection.len(), 4);
    assert_eq!(selected_at(&app, 1, 18.), 0);
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn focused_delete_removes_only_keys_and_shortcuts_respect_track_filter() {
    let mut app = animation_app();
    let nodes = app.editor.project.nodes.clone();
    click_marker(&mut app, Some(1), 6.);
    assert_eq!(
        shortcut(
            &mut app,
            Key::Named(NamedKey::Delete),
            ModifiersState::empty()
        ),
        Action::Animation(Control::Delete)
    );
    assert!(app.timeline.selection.is_empty());
    assert_eq!(app.editor.project.nodes, nodes);
    assert_eq!(app.editor.project.clips[0].channels[0].keys.len(), 2);
    let history = app.editor.undo_len();
    shortcut(
        &mut app,
        Key::Named(NamedKey::Delete),
        ModifiersState::empty(),
    );
    assert_eq!(app.editor.project.nodes, nodes);
    assert_eq!(app.editor.undo_len(), history);

    app.scene_scroll = app.scene_node_offset(2);
    rebuild(&mut app);
    click_action(&mut app, Action::Select(2));
    assert!(!app.animation.focused);
    assert!(app
        .timeline_shortcut(&Key::Named(NamedKey::Delete), false, false)
        .is_none());
    click_action(&mut app, Action::Animation(Control::OnlySelected));
    app.timeline.zoom(30., 8., 96.);
    shortcut(
        &mut app,
        Key::Character("a".into()),
        ModifiersState::CONTROL,
    );
    assert_eq!(
        app.timeline.selection.len(),
        2,
        "Select all includes keys outside current zoom"
    );
    assert!(app.timeline.selection.iter().all(|key| key.node == 2));
    app.set_time(20. / 24.);
    assert_eq!(
        shortcut(
            &mut app,
            Key::Named(NamedKey::ArrowLeft),
            ModifiersState::SHIFT
        ),
        Action::PreviousKey
    );
    assert_eq!(app.editor.time, 12. / 24.);
    assert_eq!(
        shortcut(
            &mut app,
            Key::Named(NamedKey::ArrowRight),
            ModifiersState::SHIFT
        ),
        Action::NextKey
    );
    assert_eq!(app.editor.time, 30. / 24.);
}

#[test]
fn clipboard_crosses_clips_but_new_document_clears_all_key_references() {
    let mut app = animation_app();
    let original = app.editor.project.clone();
    click_marker(&mut app, Some(1), 6.);
    shortcut(
        &mut app,
        Key::Character("c".into()),
        ModifiersState::CONTROL,
    );
    edit_number(&mut app, Control::PreviewStart, "6");
    app.action(Action::SetClip(1));
    assert!(app.timeline.selection.is_empty());
    assert!(!app.animation.preview_enabled);
    assert_eq!(app.animation_preview_range(), [0, 96]);
    app.set_time(2.);
    click_action(&mut app, Action::Animation(Control::Paste));
    assert_eq!(app.timeline.selection.len(), 2);
    assert_eq!(selected_at(&app, 1, 48.), 2);
    assert_eq!(app.editor.project.clips[0], original.clips[0]);
    assert_eq!(app.editor.project.clips[1].channels.len(), 2);
    assert!(app.editor.project.clips[1]
        .channels
        .iter()
        .all(|c| c.keys.len() == 1 && c.keys[0].time == 2.));
    assert_eq!(app.editor.undo_len(), 1);

    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
    assert!(!app.dirty);
    app.action(Action::New);
    assert!(!app.editor.project.nodes.is_empty());
    app.action(Action::ConfirmNew);
    assert!(app.editor.project.nodes.is_empty());
    assert!(app.timeline.selection.is_empty());
    assert!(!app.animation.focused);
    assert_eq!(app.animation.speed, 1.);
    assert!(!app.animation.preview_enabled);
    app.editor.mode = EditorMode::Animation;
    rebuild(&mut app);
    assert!(!app
        .ui
        .hits
        .iter()
        .any(|hit| hit.action == Action::Animation(Control::Paste)));
}

#[test]
fn preview_range_speed_play_once_and_loop_hold_the_last_frame_without_changing_export() {
    let mut app = animation_app();
    let original = app.editor.project.clone();
    edit_number(&mut app, Control::PreviewStart, "6");
    edit_number(&mut app, Control::PreviewEnd, "18");
    edit_number(&mut app, Control::Speed, "2");
    click_action(&mut app, Action::Animation(Control::Loop));
    assert!(!app.animation.loop_enabled);
    click_action(&mut app, Action::Play);
    assert!(app.editor.playing);
    assert_eq!(app.editor.time, 6. / 24.);
    app.tick_animation(0.125);
    assert_eq!(app.editor.time, 12. / 24.);
    app.tick_animation(0.125);
    assert_eq!(app.editor.time, 18. / 24.);
    assert!(!app.editor.playing);

    click_action(&mut app, Action::Animation(Control::Loop));
    click_action(&mut app, Action::Play);
    assert_eq!(
        app.editor.time,
        6. / 24.,
        "Restart at preview beginning after reaching the end"
    );
    app.tick_animation(0.25);
    assert_eq!(app.editor.time, 18. / 24.);
    assert!(app.editor.playing);
    app.tick_animation(0.25 / 24. / 2.);
    assert_eq!(
        app.editor.time,
        18. / 24.,
        "The final frame gets its own display interval"
    );
    app.tick_animation(0.8 / 24. / 2.);
    assert!(app.editor.time >= 6. / 24. && app.editor.time < 7. / 24.);
    assert!(app.editor.playing);
    click_action(&mut app, Action::Animation(Control::PreviewRange));
    assert!(!app.editor.playing);
    assert_eq!(app.animation_preview_range(), [0, 96]);
    assert_eq!(
        app.editor.project, original,
        "Preview settings must not alter render/export or document timing"
    );
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn invalid_preview_numbers_remain_inline_and_block_timeline_edits_until_resolved() {
    let mut app = animation_app();
    let original = app.editor.project.clone();
    click_action(&mut app, Action::Animation(Control::PreviewStart));
    replace_draft(&mut app, "96");
    app.action(Action::ConfirmInput);
    assert_eq!(app.input.as_ref().unwrap().value, "96");
    assert!(app.surface.input_error.is_some());
    assert_eq!(app.animation_preview_range(), [0, 96]);
    click_marker(&mut app, Some(2), 12.);
    assert_eq!(app.editor.selected, Some(1));
    assert!(app.timeline.selection.is_empty());
    assert_eq!(
        app.input.as_ref().unwrap().purpose,
        Action::Animation(Control::PreviewStart)
    );
    assert!(app.dialog.is_none() && !app.animation_panel);
    replace_draft(&mut app, "6");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none());
    assert_eq!(app.animation_preview_range(), [6, 96]);

    click_action(&mut app, Action::Animation(Control::Speed));
    replace_draft(&mut app, "NaN");
    app.action(Action::ConfirmInput);
    assert_eq!(app.input.as_ref().unwrap().value, "NaN");
    assert_eq!(app.animation.speed, 1.);
    replace_draft(&mut app, "0,5");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none());
    assert_eq!(app.animation.speed, 0.5);
    click_action(&mut app, Action::Animation(Control::PreviewEnd));
    replace_draft(&mut app, "5");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_some());
    assert_eq!(app.animation_preview_range(), [6, 96]);
    app.action(Action::Cancel);
    assert!(app.input.is_none());
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn dock_controls_and_real_key_hits_are_reachable_at_minimum_size_in_both_themes_and_dpi() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 1.5, 2.] {
            for [width, height] in [[1100., 760.], [1280., 800.]] {
                let mut app = animation_app();
                app.preferences.theme = theme;
                app.scale = scale;
                app.width = width;
                app.height = height;
                click_marker(&mut app, Some(1), 6.);
                shortcut(
                    &mut app,
                    Key::Character("c".into()),
                    ModifiersState::CONTROL,
                );
                rebuild(&mut app);
                let dock = app.animation_dock_rect();
                // Floating dock gutters show the scene; only side panels stop above it.
                assert!(app.viewport()[1] + app.viewport()[3] > dock[1]);
                for rect in [app.left_panel_rect(), app.right_panel_rect()] {
                    assert!(rect[1] + rect[3] <= dock[1] - 11.9);
                }
                let controls = [
                    Control::ToggleExpanded,
                    Control::PreviewStart,
                    Control::PreviewEnd,
                    Control::PreviewRange,
                    Control::Loop,
                    Control::Speed,
                    Control::ZoomIn,
                    Control::ZoomOut,
                    Control::Fit,
                    Control::Copy,
                    Control::Paste,
                    Control::Duplicate,
                    Control::Delete,
                    Control::SelectAll,
                    Control::Interpolation(0),
                    Control::Interpolation(1),
                    Control::Interpolation(2),
                    Control::OnlySelected,
                    Control::ToggleGhosts,
                    Control::ToggleMotionPath,
                    Control::EditGhostStep,
                ];
                let mut actions: Vec<_> = controls.into_iter().map(Action::Animation).collect();
                actions.extend([
                    Action::Start,
                    Action::PreviousKey,
                    Action::Play,
                    Action::NextKey,
                    Action::EditFrame,
                    Action::Key,
                    Action::AutoKey,
                    Action::AnimationPanel,
                ]);
                for action in actions {
                    let rect = action_rect(&app, &action);
                    assert!(rect[0] >= dock[0] && rect[1] >= dock[1]);
                    assert!(rect[0] + rect[2] <= dock[0] + dock[2] + 0.1);
                    assert!(rect[1] + rect[3] <= dock[1] + dock[3]);
                    assert_eq!(
                        app.ui.control_at(center(rect)),
                        Some(action),
                        "Overlapping control at {width}×{height}, scale {scale}"
                    );
                }
                click_action(&mut app, Action::Animation(Control::ToggleExpanded));
                assert!(!app.timeline.expanded);
                assert_eq!(app.animation_dock_rect()[3], 116.);
                click_marker(&mut app, None, 6.);
                assert_eq!(app.timeline.selection.len(), 2);
                click_action(&mut app, Action::Animation(Control::ToggleExpanded));
                assert!(app.timeline.expanded);
                edit_number(&mut app, Control::Speed, "0,5");
                assert_eq!(app.animation.speed, 0.5);
            }
        }
    }
}
