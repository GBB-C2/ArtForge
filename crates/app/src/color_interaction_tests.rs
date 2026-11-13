//! Color editing through the application's real focus, pointer and history paths.
use super::*;
use crate::color_picker::{ColorControl as Color, ColorDraft};
use poseforge_core::{box_mesh, AlphaMode, Light, Material, Node};

fn color_app() -> App {
    let mut project = Project::default();
    project.materials.push(Material {
        id: 1,
        base_color: [0.1234567, 0.3000123, 0.7189234, 0.7312345],
        emissive: [100., 200., 50.],
        ..Material::default()
    });
    project.meshes.push(box_mesh(2, "Box", Vec3::ONE, Some(1)));
    for id in [3, 4] {
        let mut node = Node::new(id, format!("Object {id}"));
        node.mesh = Some(2);
        node.transform.translation.x = (id - 3) as f32 * 2.;
        project.nodes.push(node);
    }
    project.lights[0].color = [2.5, 0.675123, 0.200098];
    project.lights.push(Light::default());
    project.render_settings.sky_color = [1.75, 0.3000123, 0.1222334];
    let mut app = App::new(None);
    app.editor = Editor::new(project);
    app.editor.mode = EditorMode::Scene;
    app.editor.selected = Some(3);
    app.saved_snapshot = app.editor.project.clone();
    app.preferences = Preferences::default();
    app.preferences.left_panel = true;
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app.dirty = false;
    app
}

fn targets() -> [Action; 8] {
    [
        Action::SurfaceMaterialColor(false),
        Action::SurfaceMaterialColor(true),
        Action::SurfaceAttenuationColor,
        Action::SurfaceLightColor,
        Action::RenderColor(0),
        Action::RenderColor(1),
        Action::RenderColor(2),
        Action::RenderColor(4),
    ]
}

fn rebuild(app: &mut App) {
    app.ui = app.build_ui();
}

fn center(rect: [f32; 4]) -> [f32; 2] {
    [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5]
}

fn hit_rect(app: &mut App, action: &Action) -> [f32; 4] {
    rebuild(app);
    if !app
        .ui
        .hits
        .iter()
        .any(|h| h.action == *action && h.rect[3] >= 22.)
        && !app.has_modal()
        && app.input.is_none()
    {
        let max = app.inspector_max_scroll();
        for i in 0..=((max / 80.).ceil() as usize) {
            app.inspector_scroll = (i as f32 * 80.).min(max);
            rebuild(app);
            if app
                .ui
                .hits
                .iter()
                .any(|h| h.action == *action && h.rect[3] >= 22.)
            {
                break;
            }
        }
    }
    app.ui
        .hits
        .iter()
        .rev()
        .find(|hit| hit.action == *action)
        .unwrap_or_else(|| panic!("Missing visible color control: {action:?}"))
        .rect
}

fn click_at(app: &mut App, point: [f32; 2]) {
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    app.mouse_up(MouseButton::Left);
}

fn click(app: &mut App, action: Action) {
    let rect = hit_rect(app, &action);
    app.mouse_move(center(rect));
    app.click_ui_control();
    // The filled bar opens its picker on click and switches to HEX on a second click.
    if matches!(action, Action::SurfaceMaterialColor(_) | Action::SurfaceAttenuationColor
        | Action::SurfaceLightColor | Action::RenderColor(_)) {
        click_at(app, center(rect));
    }
}

fn show_target(app: &mut App, source: &Action) {
    app.surface.tab = match source {
        Action::SurfaceMaterialColor(_) | Action::SurfaceAttenuationColor => 1,
        Action::SurfaceLightColor => {
            app.select_light(0);
            0
        }
        Action::RenderColor(4) => { app.select_light(0); 0 }
        Action::RenderColor(_) => 3,
        _ => panic!("Not a color target"),
    };
    app.inspector_scroll = if matches!(source, Action::SurfaceAttenuationColor) {
        820_f32.min(app.inspector_max_scroll())
    } else if matches!(source, Action::RenderColor(3..=4)) {
        app.inspector_max_scroll()
    } else if matches!(source, Action::RenderColor(_)) {
        980_f32.min(app.inspector_max_scroll())
    } else {
        0.
    };
}

fn open(app: &mut App, source: Action) {
    show_target(app, &source);
    click(app, Action::OpenColorPicker(Box::new(source)));
    assert!(app.color_picker.is_some());
    assert!(app.input.is_none());
    rebuild(app);
}

fn draft(app: &App) -> &ColorDraft {
    &app.color_picker.as_ref().unwrap().picker.draft
}

fn replace_text(app: &mut App, text: &str) {
    let input = app.input.as_mut().expect("Color field must own text input");
    input.select_all();
    input.insert(text);
}

fn edit(app: &mut App, control: Color, text: &str) {
    click(app, Action::ColorPicker(control));
    replace_text(app, text);
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none(), "Color input failed: {}", app.status);
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}

fn track(app: &mut App, control: Color) -> ([f32; 4], SliderRange, f64) {
    rebuild(app);
    app.ui
        .hits
        .iter()
        .find_map(|hit| match &hit.action {
            Action::Slide(action, range, value) if **action == Action::ColorPicker(control) => {
                Some((hit.rect, *range, *value))
            }
            _ => None,
        })
        .expect("Color numeric control must have a slider")
}

#[test]
fn color_bar_double_click_preserves_preview_then_accepts_exact_percent() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    open(&mut app, Action::SurfaceMaterialColor(false));
    let (rect, _, _) = track(&mut app, Color::Alpha);
    let point = [rect[0] + rect[2] * 0.9, center(rect)[1]];
    click_at(&mut app, point);
    assert_eq!(app.editor.project, before);
    assert!(app.input.is_none());
    rebuild(&mut app);
    click_at(&mut app, point);
    assert_eq!(app.input.as_ref().unwrap().purpose, Action::ColorPicker(Color::Alpha));
    assert!(app.drag.is_none());
    assert_eq!(app.editor.project, before);
    replace_text(&mut app, "25,5");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none());
    near(app.editor.project.material(1).unwrap().base_color[3], 0.255);
    assert_eq!(app.editor.undo_len(), 0);
    click(&mut app, Action::ColorPicker(Color::Apply));
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
}

#[test]
fn color_channel_reset_keeps_popup_open_and_restores_opening_channel() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    open(&mut app, Action::SurfaceMaterialColor(false));
    app.color_confirm(&Action::ColorPicker(Color::Alpha), "25").unwrap();
    click(&mut app, Action::ResetNumber(Box::new(Action::ColorPicker(Color::Alpha))));
    assert!(app.color_picker.is_some());
    near(draft(&app).rgba()[3], before.material(1).unwrap().base_color[3]);
    assert_eq!(app.editor.undo_len(), 0);
    app.action(Action::Cancel);
    assert_eq!(app.editor.project, before);
}

#[test]
fn wheel_sv_and_numeric_track_preview_live_then_commit_one_undo() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    open(&mut app, Action::SurfaceMaterialColor(false));
    let layout = app.color_layout().unwrap();
    app.mouse_move(layout.hue_position(120.));
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Color(_))));
    near(draft(&app).hsv().hue, 120.);
    assert_ne!(app.editor.project, before);
    app.mouse_move(layout.hue_position(240.));
    app.mouse_up(MouseButton::Right);
    assert!(matches!(app.drag, Some(Drag::Color(_))));
    assert_eq!(app.drag_button, Some(MouseButton::Left));
    near(draft(&app).hsv().hue, 240.);
    app.mouse_up(MouseButton::Left);
    assert!(app.drag.is_none());

    let [x, y, w, h] = layout.sv;
    rebuild(&mut app);
    app.mouse_move([x + w * 0.3, y + h * 0.35]);
    app.mouse_down(MouseButton::Left);
    app.mouse_move([x + w * 0.75, y + h * 0.2]);
    app.mouse_up(MouseButton::Left);
    near(draft(&app).hsv().saturation, 0.75);
    near(draft(&app).hsv().value, 0.8);

    let (rect, range, value) = track(&mut app, Color::Alpha);
    app.mouse_move([rect[0] + range.ratio(value) * rect[2], center(rect)[1]]);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Color(_))));
    let point = [rect[0] + rect[2] * 0.2, center(rect)[1]];
    app.mouse_move(point);
    near(app.editor.project.material(1).unwrap().base_color[3], 0.2);
    app.modifiers = ModifiersState::SHIFT;
    app.mouse_move(point);
    near(draft(&app).rgba()[3], 0.2);
    app.mouse_move([point[0] + rect[2] * 0.1, point[1]]);
    near(draft(&app).rgba()[3], 0.21);
    app.modifiers = ModifiersState::empty();
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(app.dirty);
    let after = app.editor.project.clone();

    click(&mut app, Action::ColorPicker(Color::Apply));
    assert!(app.color_picker.is_none());
    assert_eq!(app.editor.undo_len(), 1);
    assert_eq!(app.editor.project, after);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
    assert!(!app.dirty);
    app.action(Action::Redo);
    assert_eq!(app.editor.project, after);
    assert!(app.dirty);
}

#[test]
fn capture_cancel_restores_drag_start_and_escape_restores_the_session_baseline() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    open(&mut app, Action::SurfaceMaterialColor(false));
    edit(&mut app, Color::Hex, "#0080C0");
    let start_project = app.editor.project.clone();
    let start_draft = draft(&app).clone();
    let layout = app.color_layout().unwrap();
    rebuild(&mut app);
    app.mouse_move(layout.hue_position(20.));
    app.mouse_down(MouseButton::Left);
    app.mouse_move(layout.hue_position(80.));
    assert_ne!(app.editor.project, start_project);
    app.cancel_drag();
    assert!(app.drag.is_none());
    assert!(app.drag_button.is_none());
    assert_eq!(app.editor.project, start_project);
    assert_eq!(draft(&app).rgba(), start_draft.rgba());
    assert_eq!(draft(&app).hsv(), start_draft.hsv());
    assert_eq!(app.editor.undo_len(), 0);
    assert!(app.dirty);

    app.mouse_move(layout.hue_position(300.));
    app.mouse_down(MouseButton::Left);
    assert!(app.color_key(&Key::Named(NamedKey::Escape), false, false));
    assert!(app.color_picker.is_none());
    assert!(app.drag.is_none());
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
    assert!(!app.editor.modified);
}

#[test]
fn invalid_color_text_blocks_apply_tab_and_outside_click_but_escape_discards_it() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    let other = center(hit_rect(&mut app, &Action::Select(4)));
    open(&mut app, Action::SurfaceMaterialColor(false));
    edit(&mut app, Color::Hex, "#00FF00");
    let preview = app.editor.project.clone();
    click(&mut app, Action::ColorPicker(Color::Hue));
    replace_text(&mut app, "NaN");
    app.action(Action::ConfirmInput);
    assert!(app.surface.input_error.is_some());
    app.action(Action::ColorPicker(Color::Apply));
    assert!(app.color_picker.is_some());
    assert!(app.color_key(&Key::Named(NamedKey::Tab), false, false));
    assert_eq!(
        app.input.as_ref().unwrap().purpose,
        Action::ColorPicker(Color::Hue)
    );
    click_at(&mut app, other);
    assert_eq!(app.editor.selected, Some(3));
    assert_eq!(app.input.as_ref().unwrap().value, "NaN");
    assert_eq!(app.editor.project, preview);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(app.color_key(&Key::Named(NamedKey::Escape), false, false));
    assert!(app.input.is_none());
    assert!(app.color_picker.is_none());
    assert!(app.surface.input_error.is_none());
    assert_eq!(app.editor.project, before);
    assert!(!app.dirty);
}

#[test]
fn outside_dismissal_applies_once_and_consumes_selection_and_delete_hits() {
    for underlying in [Action::Select(4), Action::Delete] {
        let mut app = color_app();
        let before_nodes = app.editor.project.nodes.clone();
        let rect = if underlying == Action::Delete {
            [12., 14., 48., 28.]
        } else {
            hit_rect(&mut app, &underlying)
        };
        open(&mut app, Action::SurfaceMaterialColor(false));
        edit(&mut app, Color::Hex, "#FF8000");
        let preview = app.editor.project.clone();
        rebuild(&mut app);
        assert!(!ui::contains(
            app.color_layout().unwrap().panel,
            center(rect)
        ));
        // A stale/destructive background hit must not receive the dismissal click.
        app.ui.hits.push(ui::Hit {
            rect,
            action: underlying.clone(),
        });
        assert_eq!(app.ui.hit(center(rect)), Some(underlying.clone()));
        click_at(&mut app, center(rect));
        assert!(app.color_picker.is_none());
        assert_eq!(app.editor.project, preview);
        assert_eq!(app.editor.project.nodes, before_nodes);
        assert_eq!(app.editor.selected, Some(3));
        assert_eq!(app.editor.undo_len(), 1);
        if underlying == Action::Select(4) {
            click(&mut app, underlying);
            assert_eq!(app.editor.selected, Some(4));
            assert_eq!(app.editor.project, preview);
        }
    }
}

#[test]
fn untouched_hex_and_restore_original_preserve_float_hdr_dirty_state_and_redo() {
    for (index, source) in targets().into_iter().enumerate() {
        let mut app = color_app();
        if index % 2 == 1 {
            app.editor.project.title.push_str(" unsaved");
            app.update_dirty();
        }
        let dirty = app.dirty;
        let before = app.editor.project.clone();
        app.editor
            .transact("Prepare redo", |project| project.title.push('!'))
            .unwrap();
        let redo = app.editor.project.clone();
        app.action(Action::Undo);
        let modified = app.editor.modified;
        assert!(app.editor.can_redo());

        open(&mut app, source.clone());
        assert_eq!(app.editor.project, before);
        assert_eq!(app.dirty, dirty);
        let original_hex = draft(&app).hex();
        // Different text spelling forces the parser path without changing bytes.
        edit(
            &mut app,
            Color::Hex,
            &format!(" {} ", original_hex.to_ascii_lowercase()),
        );
        assert_eq!(app.editor.project, before);
        click(&mut app, Action::ColorPicker(Color::Apply));
        assert_eq!(app.editor.undo_len(), 0);
        assert!(app.editor.can_redo());
        assert_eq!(app.editor.modified, modified);
        assert_eq!(app.dirty, dirty);

        open(&mut app, source);
        edit(&mut app, Color::Hex, "#F01080");
        assert_ne!(app.editor.project, before);
        click(&mut app, Action::ColorPicker(Color::RestoreOriginal));
        assert_eq!(app.editor.project, before);
        click(&mut app, Action::ColorPicker(Color::Apply));
        assert_eq!(app.editor.undo_len(), 0);
        assert_eq!(app.editor.modified, modified);
        assert_eq!(app.dirty, dirty);
        assert!(app.editor.can_redo());
        app.action(Action::Redo);
        assert_eq!(app.editor.project, redo);
    }
}

#[test]
fn alpha_only_edits_preserve_exact_rgb_and_respect_existing_alpha_modes() {
    for mode in [AlphaMode::Opaque, AlphaMode::Mask, AlphaMode::Blend] {
        let mut app = color_app();
        app.editor.project.materials[0].alpha_mode = mode;
        app.saved_snapshot = app.editor.project.clone();
        let before = app.editor.project.clone();
        let rgb = &before.materials[0].base_color[..3];
        open(&mut app, Action::SurfaceMaterialColor(false));
        edit(&mut app, Color::Alpha, "0");
        let material = &app.editor.project.materials[0];
        assert_eq!(&material.base_color[..3], rgb);
        assert_eq!(material.base_color[3], 0.);
        assert_eq!(
            material.alpha_mode,
            if mode == AlphaMode::Opaque {
                AlphaMode::Blend
            } else {
                mode
            }
        );
        edit(&mut app, Color::Alpha, "100");
        assert_eq!(app.editor.project.materials[0].alpha_mode, mode);
        assert_eq!(&app.editor.project.materials[0].base_color[..3], rgb);
        edit(&mut app, Color::Alpha, "62,5");
        let mut expected = before.clone();
        expected.materials[0].base_color[3] = 0.625;
        if mode == AlphaMode::Opaque {
            expected.materials[0].alpha_mode = AlphaMode::Blend;
        }
        assert_eq!(app.editor.project, expected);
        assert_eq!(app.editor.undo_len(), 0);
        click(&mut app, Action::ColorPicker(Color::Apply));
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
    }
}

#[test]
fn emissive_black_then_new_color_retains_the_opening_hdr_strength() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    open(&mut app, Action::SurfaceMaterialColor(true));
    assert!(!draft(&app).alpha_enabled());
    for (hex, expected) in [("#000000", [0.; 3]), ("#FF0000", [200., 0., 0.])] {
        app.color_picker
            .as_mut()
            .unwrap()
            .picker
            .draft
            .set_text(Color::Hex, hex)
            .unwrap();
        app.preview_color().unwrap();
        let mut expected_project = before.clone();
        expected_project.materials[0].emissive = expected;
        assert_eq!(app.editor.project, expected_project);
        assert_eq!(app.editor.undo_len(), 0);
    }
    click(&mut app, Action::ColorPicker(Color::Apply));
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
}

#[test]
fn all_eight_color_triggers_keep_hex_entry_and_only_modify_their_own_target() {
    let rgb = surfaces::parse_color("#40A0E0").unwrap();
    for source in targets() {
        let mut app = color_app();
        let before = app.editor.project.clone();
        show_target(&mut app, &source);
        let field = hit_rect(&mut app, &source);
        let trigger = hit_rect(&mut app, &Action::OpenColorPicker(Box::new(source.clone())));
        assert_eq!(field, trigger);
        assert!(field[3] >= 22. && trigger[3] >= 22.);
        click(&mut app, source.clone());
        assert!(app.input.is_some());
        assert!(app.color_picker.is_none());
        app.action(Action::ConfirmInput);
        assert_eq!(app.editor.project, before);
        open(&mut app, source.clone());
        assert_eq!(
            draft(&app).alpha_enabled(),
            source == Action::SurfaceMaterialColor(false)
        );
        edit(&mut app, Color::Hex, "#40A0E0");
        let mut expected = before.clone();
        match source {
            Action::SurfaceMaterialColor(false) => {
                expected.materials[0].base_color[..3].copy_from_slice(&rgb)
            }
            Action::SurfaceMaterialColor(true) => {
                let maximum = rgb.into_iter().fold(0., f32::max);
                expected.materials[0].emissive = rgb.map(|channel| channel / maximum * 200.);
            }
            Action::SurfaceAttenuationColor => expected.materials[0].attenuation_color = Some(rgb),
            Action::SurfaceLightColor => expected.lights[0].color = rgb,
            Action::RenderColor(0) => expected.render_settings.sky_color = rgb,
            Action::RenderColor(1) => expected.render_settings.ground_color = rgb,
            Action::RenderColor(2) => expected.render_settings.background_color = rgb,
            Action::RenderColor(4) => expected.render_settings.atmosphere.color = rgb,
            _ => unreachable!(),
        }
        assert_eq!(app.editor.project, expected);
        assert_eq!(app.editor.undo_len(), 0);
        click(&mut app, Action::ColorPicker(Color::Apply));
        assert_eq!(app.editor.project, expected);
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
    }
}

#[test]
fn alt_down_tab_and_enter_keep_keyboard_focus_inside_the_color_transaction() {
    let mut app = color_app();
    let source = Action::SurfaceMaterialColor(false);
    show_target(&mut app, &source);
    click(&mut app, source);
    assert!(!app.color_open_key(&Key::Named(NamedKey::ArrowDown)));
    app.modifiers = ModifiersState::ALT;
    assert!(app.color_open_key(&Key::Named(NamedKey::ArrowDown)));
    app.modifiers = ModifiersState::empty();
    assert!(app.color_picker.is_some());
    assert!(app.input.is_none());
    // The opening action must replace background hits synchronously: Tab may
    // arrive before a redraw, so it must never focus an underlying scene field.
    for control in [
        Color::Hex,
        Color::Hue,
        Color::Saturation,
        Color::Value,
        Color::Alpha,
        Color::Hex,
    ] {
        assert!(app.color_key(&Key::Named(NamedKey::Tab), false, false));
        assert_eq!(
            app.input.as_ref().unwrap().purpose,
            Action::ColorPicker(control)
        );
    }
    assert!(app.color_key(&Key::Named(NamedKey::Tab), false, true));
    assert_eq!(
        app.input.as_ref().unwrap().purpose,
        Action::ColorPicker(Color::Alpha)
    );
    assert!(!app.color_key(&Key::Character("5".into()), false, false));
    replace_text(&mut app, "50");
    assert!(app.color_key(&Key::Named(NamedKey::Enter), false, false));
    assert_eq!(
        app.input.as_ref().unwrap().purpose,
        Action::SurfaceMaterialColor(false)
    );
    assert!(app.color_picker.is_none());
    assert_eq!(app.editor.project.materials[0].base_color[3], 0.5);
    assert_eq!(app.editor.undo_len(), 1);
}

#[test]
fn popup_owns_scene_keys_scroll_and_nonleft_pointer_input() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    let camera = (app.yaw, app.pitch, app.distance, app.target);
    let selection = app.editor.selected;
    let tool = app.tool;
    let time = app.editor.time;
    open(&mut app, Action::SurfaceMaterialColor(false));
    let panel = app.color_layout().unwrap().panel;
    app.mouse_move([panel[0] + 6., panel[1] + 40.]);
    for button in [MouseButton::Middle, MouseButton::Right] {
        app.mouse_down(button);
        app.mouse_move([panel[0] + 10., panel[1] + 42.]);
        app.mouse_up(button);
        assert!(app.drag.is_none());
    }
    for (key, ctrl) in [
        (Key::Character("g".into()), false),
        (Key::Character("z".into()), true),
        (Key::Named(NamedKey::Delete), false),
        (Key::Named(NamedKey::Space), false),
    ] {
        assert!(app.color_key(&key, ctrl, false));
    }
    let scroll = (app.scene_scroll, app.inspector_scroll);
    for point in [
        center(app.viewport()),
        center(app.left_panel_rect()),
        center(app.right_panel_rect()),
    ] {
        app.mouse_move(point);
        app.scroll(-4.);
    }
    assert_eq!((app.scene_scroll, app.inspector_scroll), scroll);
    assert_eq!((app.yaw, app.pitch, app.distance, app.target), camera);
    assert_eq!(app.editor.selected, selection);
    assert!(app.tool == tool);
    assert_eq!(app.editor.time, time);
    assert_eq!(app.editor.project, before);
    assert!(app.ui.hits.iter().all(|hit| matches!(hit.action, Action::ColorPicker(_))
        || matches!(&hit.action, Action::Slide(action, _, _) | Action::ResetNumber(action) if matches!(**action, Action::ColorPicker(_)))));
    // An explicit command still resolves ownership by cancelling the old target.
    edit(&mut app, Color::Hex, "#FF0000");
    app.action(Action::Select(4));
    assert!(app.color_picker.is_none());
    assert_eq!(app.editor.selected, Some(4));
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn popup_geometry_and_draft_survive_theme_dpi_and_window_size_changes() {
    let mut app = color_app();
    open(&mut app, Action::SurfaceMaterialColor(false));
    edit(&mut app, Color::Hex, "#2080D0");
    let preview = app.editor.project.clone();
    let original_draft = draft(&app).clone();
    for (width, height, scale, theme) in [
        (1100., 760., 1., ui::Theme::Dark),
        (1440., 900., 2., ui::Theme::Light),
        (1100., 760., 1.5, ui::Theme::Light),
        (1280., 800., 2., ui::Theme::Dark),
    ] {
        // Native resize/DPI handling cancels capture before updating logical size.
        app.cancel_drag();
        app.width = width;
        app.height = height;
        app.scale = scale;
        app.preferences.theme = theme;
        rebuild(&mut app);
        let layout = app.color_layout().unwrap();
        assert!(layout.panel.iter().all(|value| value.is_finite()));
        assert!(layout.panel[0] >= 0. && layout.panel[1] >= 0.);
        assert!(layout.panel[0] + layout.panel[2] <= width);
        assert!(layout.panel[1] + layout.panel[3] <= height - 32.);
        assert!(!app.ui.hits.is_empty());
        for hit in &app.ui.hits {
            assert!(hit.rect.iter().all(|value| value.is_finite()));
            assert!(hit.rect[0] >= layout.panel[0] && hit.rect[1] >= layout.panel[1]);
            assert!(hit.rect[0] + hit.rect[2] <= layout.panel[0] + layout.panel[2] + 0.001);
            assert!(hit.rect[1] + hit.rect[3] <= layout.panel[1] + layout.panel[3] + 0.001);
        }
        for control in [
            Color::Hex,
            Color::Hue,
            Color::Saturation,
            Color::Value,
            Color::Alpha,
            Color::Apply,
            Color::Cancel,
        ] {
            assert!(app
                .ui
                .hits
                .iter()
                .any(|hit| hit.action == Action::ColorPicker(control)));
        }
        assert_eq!(draft(&app).rgba(), original_draft.rgba());
        assert_eq!(draft(&app).hsv(), original_draft.hsv());
        assert_eq!(app.editor.project, preview);
        assert_eq!(app.editor.undo_len(), 0);
    }
    click(&mut app, Action::ColorPicker(Color::Apply));
    assert_eq!(app.editor.undo_len(), 1);
    assert_eq!(app.editor.project, preview);
}

#[test]
fn marker_clicks_and_return_to_start_preserve_original_float_colors() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    open(&mut app, Action::SurfaceMaterialColor(false));
    let original = draft(&app).clone();
    let layout = app.color_layout().unwrap();
    for control in [Color::Wheel, Color::Sv] {
        let from = if control == Color::Wheel {
            layout.hue_position(original.hsv().hue)
        } else {
            layout.sv_position(original.hsv())
        };
        let to = if control == Color::Wheel {
            layout.hue_position(original.hsv().hue + 70.)
        } else {
            [
                layout.sv[0] + layout.sv[2] * 0.8,
                layout.sv[1] + layout.sv[3] * 0.2,
            ]
        };
        rebuild(&mut app);
        click_at(&mut app, from);
        assert_eq!(app.editor.project, before);
        assert_eq!(draft(&app).rgba(), original.rgba());
        app.mouse_move(from);
        app.mouse_down(MouseButton::Left);
        assert!(matches!(app.drag, Some(Drag::Color(_))));
        app.mouse_move(to);
        assert_ne!(app.editor.project, before);
        app.mouse_move(from);
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.editor.project, before);
        assert_eq!(draft(&app).rgba(), original.rgba());
        assert_eq!(draft(&app).hsv(), original.hsv());
        assert!(!app.dirty);
        assert_eq!(app.editor.undo_len(), 0);
    }
    click(&mut app, Action::ColorPicker(Color::Apply));
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.editor.modified);
    assert!(!app.dirty);
}

#[test]
fn rewriting_existing_inline_hex_before_alt_down_preserves_float_and_hdr_sources() {
    for source in [
        Action::SurfaceMaterialColor(false),
        Action::SurfaceMaterialColor(true),
        Action::SurfaceLightColor,
    ] {
        let mut app = color_app();
        let before = app.editor.project.clone();
        show_target(&mut app, &source);
        click(&mut app, source);
        let spelling = format!(
            " {} ",
            app.input.as_ref().unwrap().value.to_ascii_lowercase()
        );
        replace_text(&mut app, &spelling);
        assert!(app.input.as_ref().unwrap().changed());
        app.modifiers = ModifiersState::ALT;
        assert!(app.color_open_key(&Key::Named(NamedKey::ArrowDown)));
        app.modifiers = ModifiersState::empty();
        assert!(app.color_picker.is_some());
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
        assert!(!app.editor.modified);
        assert!(!app.dirty);
        assert!(app.color_key(&Key::Named(NamedKey::Escape), false, false));
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
        assert!(!app.dirty);
    }
}

#[test]
fn marker_grab_offsets_do_not_jump_and_outer_sv_marker_halves_remain_draggable() {
    let mut app = color_app();
    open(&mut app, Action::SurfaceMaterialColor(false));
    let layout = app.color_layout().unwrap();
    let original = draft(&app).clone();
    let marker = layout.sv_position(original.hsv());
    let from = [marker[0] + 5., marker[1]];
    app.mouse_move(from);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Color(_))));
    assert_eq!(draft(&app).rgba(), original.rgba());
    app.mouse_move([from[0] + 1., from[1]]);
    near(
        draft(&app).hsv().saturation,
        original.hsv().saturation + 1. / layout.sv[2],
    );
    near(draft(&app).hsv().value, original.hsv().value);
    app.cancel_drag();

    let marker = layout.hue_position(original.hsv().hue);
    let from = [marker[0] + 5., marker[1]];
    let to = [from[0], from[1] + 1.];
    let expected = (original.hsv().hue + layout.hue_at(to, original.hsv().hue)
        - layout.hue_at(from, original.hsv().hue))
    .rem_euclid(360.);
    rebuild(&mut app);
    app.mouse_move(from);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Color(_))));
    assert_eq!(draft(&app).rgba(), original.rgba());
    app.mouse_move(to);
    let angular_error = (draft(&app).hsv().hue - expected + 180.).rem_euclid(360.) - 180.;
    assert!(
        angular_error.abs() < 0.001,
        "Hue jumped by the grab offset: {angular_error}"
    );
    app.cancel_drag();

    for (saturation, value, offset) in [
        (0., 0.5, [-5_f32, 0.]),
        (1., 0.5, [5., 0.]),
        (0.5, 0., [0., 5.]),
        (0.5, 1., [0., -5.]),
    ] {
        let color_draft = &mut app.color_picker.as_mut().unwrap().picker.draft;
        color_draft
            .set_value(Color::Saturation, saturation * 100.)
            .unwrap();
        color_draft.set_value(Color::Value, value * 100.).unwrap();
        app.preview_color().unwrap();
        rebuild(&mut app);
        let before = draft(&app).clone();
        let marker = layout.sv_position(before.hsv());
        let from = [marker[0] + offset[0], marker[1] + offset[1]];
        assert!(!ui::contains(layout.sv, from));
        app.mouse_move(from);
        app.mouse_down(MouseButton::Left);
        assert!(
            matches!(app.drag, Some(Drag::Color(_))),
            "Outside half of SV marker lost its hit"
        );
        assert_eq!(draft(&app).rgba(), before.rgba());
        let step = [
            if offset[0] == 0. {
                0.
            } else {
                offset[0].signum()
            },
            if offset[1] == 0. {
                0.
            } else {
                offset[1].signum()
            },
        ];
        app.mouse_move([from[0] - step[0], from[1] - step[1]]);
        near(
            draft(&app).hsv().saturation,
            saturation - step[0] / layout.sv[2],
        );
        near(draft(&app).hsv().value, value + step[1] / layout.sv[3]);
        app.cancel_drag();
    }
    app.action(Action::Cancel);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn keyboard_open_returns_to_source_hex_after_finish_while_mouse_open_does_not() {
    for keyboard_open in [true, false] {
        for finish in [NamedKey::Escape, NamedKey::Enter] {
            let mut app = color_app();
            let before = app.editor.project.clone();
            let source = Action::SurfaceMaterialColor(false);
            if keyboard_open {
                show_target(&mut app, &source);
                click(&mut app, source.clone());
                app.modifiers = ModifiersState::ALT;
                assert!(app.color_open_key(&Key::Named(NamedKey::ArrowDown)));
                app.modifiers = ModifiersState::empty();
            } else {
                open(&mut app, source.clone());
            }
            edit(&mut app, Color::Hex, "#D03090");
            assert!(app.color_key(&Key::Named(finish), false, false));
            assert!(app.color_picker.is_none());
            if keyboard_open {
                let input = app
                    .input
                    .as_ref()
                    .expect("Return focus to the initiating HEX field");
                assert_eq!(input.purpose, source);
                assert!(!input.changed());
                let [r, g, b, _] = app.editor.project.materials[0].base_color;
                assert_eq!(input.value, surfaces::color_hex([r, g, b]));
                assert!(app.active_field_rect().is_some());
            } else {
                assert!(app.input.is_none());
            }
            if finish == NamedKey::Escape {
                assert_eq!(app.editor.project, before);
                assert_eq!(app.editor.undo_len(), 0);
            } else {
                assert_ne!(app.editor.project, before);
                assert_eq!(app.editor.undo_len(), 1);
            }
        }
    }
}

#[test]
fn absorption_cancel_reset_and_locked_material_keep_history_and_original_color() {
    let mut app = color_app();
    let before = app.editor.project.clone();
    open(&mut app, Action::SurfaceAttenuationColor);
    edit(&mut app, Color::Hex, "#40A0E0");
    assert!(app.editor.project.materials[0].attenuation_color.is_some());
    click(&mut app, Action::ColorPicker(Color::Cancel));
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    open(&mut app, Action::SurfaceAttenuationColor);
    edit(&mut app, Color::Hex, "#40A0E0");
    click(&mut app, Action::ColorPicker(Color::Apply));
    let authored = app.editor.project.clone();
    assert_eq!(
        authored.materials[0].base_color,
        before.materials[0].base_color
    );
    click(
        &mut app,
        Action::Studio(ui::StudioAction::AbsorptionFromBase),
    );
    assert!(app.editor.project.materials[0].attenuation_color.is_none());
    assert_eq!(app.editor.undo_len(), 2);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, authored);
    app.editor.project.node_mut(3).unwrap().locked = true;
    let locked = app.editor.project.clone();
    app.action(Action::Studio(ui::StudioAction::AbsorptionFromBase));
    app.action(Action::OpenColorPicker(Box::new(
        Action::SurfaceAttenuationColor,
    )));
    assert!(app.color_picker.is_none());
    assert_eq!(app.editor.project, locked);
}

#[test]
fn filled_color_bar_double_click_edits_hex_and_reset_preserves_alpha_with_undo() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {
        let mut app=color_app();app.preferences.theme=theme;
        let before=app.editor.project.clone(); let source=Action::SurfaceMaterialColor(false);
        show_target(&mut app, &source);
        rebuild(&mut app);
        let open=Action::OpenColorPicker(Box::new(source.clone()));
        let point=app.ui.action_point(&open).unwrap();
        app.mouse_move(point);app.mouse_down(MouseButton::Left);app.mouse_up(MouseButton::Left);
        assert!(app.color_picker.is_some());
        app.mouse_move(point);app.mouse_down(MouseButton::Left);app.mouse_up(MouseButton::Left);
        assert!(app.color_picker.is_none());assert_eq!(app.input.as_ref().unwrap().purpose,source);
        app.input.as_mut().unwrap().insert("#2080C0");
        assert!(app.confirm_input());assert_eq!(app.editor.undo_len(),1);
        assert_eq!(app.editor.project.material(1).unwrap().base_color[3],before.material(1).unwrap().base_color[3]);
        let edited=app.editor.project.clone();
        app.action(Action::ResetColor(Box::new(source)));
        assert_eq!(&app.editor.project.material(1).unwrap().base_color[..3],&Material::default().base_color[..3]);
        assert_eq!(app.editor.project.material(1).unwrap().base_color[3],before.material(1).unwrap().base_color[3]);
        assert_eq!(app.editor.undo_len(),2);app.action(Action::Undo);assert_eq!(app.editor.project,edited);
    }
}
