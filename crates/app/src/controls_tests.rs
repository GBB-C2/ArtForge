//! Integration checks for inline fields and captured slider gestures.
use super::*;
use crate::chrome::Menu;
use poseforge_core::Node;

// Exercise the user gesture when an existing test asks to edit a control.
// Raw pointer tests below still send individual presses to check click timing.
impl App {
    pub(super) fn click_ui_control(&mut self) {
        let numeric = matches!(self.ui.hit(self.mouse), Some(Action::Slide(..)));
        let editing = self
            .active_field_rect()
            .is_some_and(|r| ui::contains(r, self.mouse));
        let clicks = if numeric && !editing { 2 } else { 1 };
        self.slider_click = None;
        for click in 0..clicks {
            // CPU-only UI construction can exceed the double-click window.
            // Keep synthetic input timing independent of rendering cost;
            // raw pointer and timeout tests continue to exercise real timing.
            if click > 0 {
                if let Some(previous) = &mut self.slider_click { previous.refresh_time(); }
            }
            self.mouse_down(MouseButton::Left);
            self.mouse_up(MouseButton::Left);
            self.ui = self.build_ui();
        }
    }
}

fn control_app() -> App {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.preferences.left_panel = true;
    app.preferences.right_panel = true;
    let mut project = Project {
        duration: 6.,
        ..Project::default()
    };
    let mut first = Node::new(1, "Первый объект");
    first.transform.translation.x = 0.1234567;
    let mut second = Node::new(2, "Второй объект");
    second.transform.translation.x = -2.;
    project.nodes.extend([first, second]);
    app.editor = Editor::new(project);
    app.editor.selected = Some(1);
    app.editor.mode = EditorMode::Scene;
    app.saved_snapshot = app.editor.project.clone();
    app.dirty = false;
    app
}

#[test]
fn adjacent_reset_icon_discards_invalid_draft_and_uses_one_undo() {
    let mut app = control_app();
    app.height = 1000.;
    let before = app.editor.project.clone();
    app.action(Action::EditTransform(0));
    app.input.as_mut().unwrap().select_all();
    app.input.as_mut().unwrap().insert("invalid");
    click_action(&mut app, Action::ResetNumber(Box::new(Action::EditTransform(0))));
    assert!(app.input.is_none());
    assert_eq!(app.editor.project.node(1).unwrap().transform.translation.x, 0.);
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
}

#[test]
fn every_visible_numeric_bar_has_a_default_and_a_separate_reset_target() {
    let mut app = control_app();
    app.height = 1600.;
    app.create_primitive(0);
    app.primitive_panel = None;
    app.fluid_object_action(ui::FluidObjectAction::Attach);
    app.wait_for_background_jobs();
    for tab in [0, 1, 3, 4, 6, 10, 11] {
        app.set_properties_tab(tab);
        for bottom in [false, true] {
            app.inspector_scroll = if bottom { app.inspector_max_scroll() } else { 0. };
            app.ui = app.build_ui();
            for hit in &app.ui.hits {
                if let Action::Slide(field, _, _) = &hit.action {
                    assert!(app.number_default(field).is_some(), "Missing default: {field:?}");
                    assert!(app.ui.hits.iter().any(|h| h.action == Action::ResetNumber(field.clone())));
                }
            }
        }
    }
}

#[test]
fn primitive_parameters_show_quad_wires_without_entering_mesh_editing() {
    let mut app = control_app();
    app.create_primitive(0);
    app.frame_all();
    let id = app.editor.selected.unwrap();
    let original = app.editor.project.clone();
    // A remembered component X-ray setting must not expose back edges here.
    app.authoring.xray = true;
    let mut overlay = Ui::with_theme(1., ui::Theme::Dark, [0., 0.]);
    app.draw_mesh_overlay(&mut overlay);
    assert_eq!(
        overlay.rects.iter().filter(|r| r.depth.is_some()).count(),
        12
    );
    assert!(overlay.hits.is_empty());
    assert!(app.authoring.editing.is_none());
    assert_eq!(app.editor.project, original);
    app.primitive_confirm(&Action::Primitive(ui::PrimitiveAction::Field(3)), "3")
        .unwrap();
    let mesh = app.editor.editable_mesh(id).unwrap();
    assert!((0..mesh.face_count()).all(|f| mesh.face_vertices(f).unwrap().len() == 4));
    let mut overlay = Ui::with_theme(1., ui::Theme::Dark, [0., 0.]);
    app.draw_mesh_overlay(&mut overlay);
    assert!(overlay.rects.iter().filter(|r| r.depth.is_some()).count() > 12);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
    app.action(Action::Primitive(ui::PrimitiveAction::Close));
    let mut overlay = Ui::with_theme(1., ui::Theme::Dark, [0., 0.]);
    app.draw_mesh_overlay(&mut overlay);
    assert!(overlay.rects.is_empty());
}

fn rebuild_ui(app: &mut App) {
    app.ui = app.build_ui();
}

fn click_action(app: &mut App, action: Action) {
    rebuild_ui(app);
    let rect = app
        .ui
        .hits
        .iter()
        .rev()
        .find(|hit| hit.action == action)
        .unwrap_or_else(|| panic!("Expected a visible control for {action:?}"))
        .rect;
    app.mouse_move([rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5]);
    app.click_ui_control();
}

fn replace_draft(app: &mut App, value: &str) {
    let input = app.input.as_mut().expect("The field should own text input");
    input.select_all();
    input.insert(value);
}

fn begin_visible_slider(app: &mut App, field: Action) -> ([f32; 4], SliderRange, f64) {
    rebuild_ui(app);
    let (rect, range, value) = app
        .ui
        .hits
        .iter()
        .rev()
        .find_map(|hit| match &hit.action {
            Action::Slide(action, range, value) if **action == field => {
                Some((hit.rect, *range, *value))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Expected a visible slider for {field:?}"));
    app.mouse_move([
        rect[0] + rect[2] * range.ratio(value),
        rect[1] + rect[3] * 0.5,
    ]);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Slider(_))));
    (rect, range, value)
}

fn move_on_track(app: &mut App, rect: [f32; 4], fraction: f32) {
    app.mouse_move([rect[0] + rect[2] * fraction, rect[1] + rect[3] * 0.5]);
}

#[test]
fn double_click_anywhere_on_a_bar_edits_without_jumping_or_losing_precision() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            for fraction in [0.1, 0.9] {
                let mut app = control_app();
                app.preferences.theme = theme;
                app.scale = scale;
                rebuild_ui(&mut app);
                let field = Action::EditTransform(0);
                let rect = app
                    .ui
                    .hits
                    .iter()
                    .find(|hit| hit.action == field)
                    .unwrap()
                    .rect;
                let before = app.editor.project.clone();
                app.mouse_move([rect[0] + rect[2] * fraction, rect[1] + rect[3] * 0.5]);
                app.mouse_down(MouseButton::Left);
                assert!(matches!(app.drag, Some(Drag::Slider(_))));
                assert_eq!(app.editor.project, before);
                app.mouse_up(MouseButton::Left);
                assert!(app.input.is_none());
                assert_eq!(app.editor.undo_len(), 0);
                rebuild_ui(&mut app);
                // The gesture is two immediate clicks; CPU raster preparation
                // is not user delay. Timeout behavior has its own clock test.
                app.slider_click.as_mut().unwrap().refresh_time();
                app.mouse_down(MouseButton::Left);
                app.mouse_up(MouseButton::Left);
                let input = app.input.as_ref().expect("Double click opens exact entry");
                assert_eq!(input.purpose, field);
                assert_eq!(input.selection(), 0..input.value.chars().count());
                assert_eq!(app.editor.project, before);
                assert!(app.drag.is_none());
                assert_eq!(app.editor.undo_len(), 0);
                app.action(Action::ConfirmInput);
                assert_eq!(app.editor.project, before);
                assert_eq!(app.editor.undo_len(), 0);
            }
        }
    }
}

#[test]
fn double_click_tolerates_pointer_jitter_without_changing_the_number() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    begin_visible_slider(&mut app, Action::EditTransform(0));
    app.mouse_move([app.mouse[0] + 1., app.mouse[1] + 0.5]);
    assert_eq!(app.editor.project, before);
    app.mouse_up(MouseButton::Left);
    rebuild_ui(&mut app);
    app.slider_click.as_mut().unwrap().refresh_time();
    app.mouse_down(MouseButton::Left);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.input.as_ref().unwrap().purpose, Action::EditTransform(0));
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn bar_entry_validates_commits_one_undo_and_cancels_the_draft() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    click_action(&mut app, Action::EditTransform(0));
    replace_draft(&mut app, "NaN");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_some());
    assert!(app.surface.input_error.is_some());
    assert_eq!(app.editor.project, before);
    replace_draft(&mut app, "2,75");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none());
    assert_eq!(app.editor.project.node(1).unwrap().transform.translation.x, 2.75);
    assert_eq!(app.editor.undo_len(), 1);
    let applied = app.editor.project.clone();
    click_action(&mut app, Action::EditTransform(0));
    replace_draft(&mut app, "9");
    app.action(Action::Cancel);
    assert_eq!(app.editor.project, applied);
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
}

#[test]
fn bar_drag_is_relative_from_the_value_area_and_does_not_arm_a_double_click() {
    let mut app = control_app();
    rebuild_ui(&mut app);
    let (rect, range, start) = app
        .ui
        .hits
        .iter()
        .find_map(|hit| match &hit.action {
            Action::Slide(field, range, value) if **field == Action::EditTransform(0) => {
                Some((hit.rect, *range, *value))
            }
            _ => None,
        })
        .unwrap();
    let point = [rect[0] + rect[2] * 0.8, rect[1] + rect[3] * 0.5];
    let before = app.editor.project.clone();
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    assert_eq!(app.editor.project, before);
    app.mouse_move([point[0] + rect[2] * 0.1, point[1]]);
    let expected = range.clamp_and_snap(start + (range.max - range.min) * 0.1);
    let actual = app.editor.project.node(1).unwrap().transform.translation.x as f64;
    assert!((actual - expected).abs() < 0.00001);
    assert!(app.slider_click.is_none());
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);
    rebuild_ui(&mut app);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Slider(_))));
    assert!(app.input.is_none());
    app.cancel_drag();
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
}

#[test]
fn cancelled_capture_does_not_turn_the_next_press_into_text_entry() {
    let mut app = control_app();
    begin_visible_slider(&mut app, Action::EditTransform(0));
    app.cancel_drag();
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Slider(_))));
    assert!(app.input.is_none());
    app.mouse_up(MouseButton::Left);
    // Moving away and back also breaks the stationary click sequence.
    let point = app.mouse;
    app.mouse_move([point[0], point[1] + 20.]);
    app.mouse_move(point);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Slider(_))));
    assert!(app.input.is_none());
    app.cancel_drag();
}

#[test]
fn primitive_dropdown_opens_translucent_panel_with_working_parameters_and_pointer_ownership() {
    use ui::PrimitiveAction as P;
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            for kind in [0, 1, 2, 4] {
                let menu_index = poseforge_core::PrimitiveKind::MESH_MENU.iter().position(|(code,_)| *code == kind).unwrap();
                let mut app = control_app();
                app.preferences.theme = theme;
                app.scale = scale;
                app.width = 1100.;
                app.height = 620.;
                click_action(&mut app, Action::DropdownOpen(ui::DropdownKind::MeshCreate));
                app.dropdown_key(&Key::Named(NamedKey::Home), false, false, false);
                for _ in 0..menu_index {
                    app.dropdown_key(&Key::Named(NamedKey::ArrowDown), false, false, false);
                }
                click_action(&mut app, Action::DropdownChoose(menu_index));
                assert!(app.dropdown.is_none());
                let (id, settings) = app.selected_primitive().unwrap();
                assert_eq!(app.primitive_panel, Some(id));
                let rect = app.primitive_panel_rect().unwrap();
                assert!(rect[0] >= app.left_panel_rect()[0] + app.left_panel_rect()[2]);
                assert!(rect[0] + rect[2] < app.right_panel_rect()[0]);
                assert!(rect[1] >= app.viewport()[1]);
                assert!(rect[1] + rect[3] <= app.viewport()[1] + app.viewport()[3]);
                rebuild_ui(&mut app);
                assert!(app
                    .ui
                    .rects
                    .iter()
                    .any(|r| (r.color[3] - 0.90).abs() < 0.001));
                assert!(!app
                    .ui
                    .texts
                    .iter()
                    .any(|t| t.text.contains("Редактировать меш")));
                let gap = [rect[0] + 5., rect[1] + 42.];
                assert!(!app.canvas_input_at(gap));
                assert_eq!(app.ui.hit(gap), Some(Action::Primitive(P::Backdrop)));
                let field = if kind == 2 { 6 } else { 0 };
                click_action(&mut app, Action::Primitive(P::Field(field)));
                replace_draft(&mut app, "3,5");
                app.action(Action::ConfirmInput);
                let changed = app.selected_primitive().unwrap().1;
                assert_eq!(
                    if kind == 2 {
                        changed.radius
                    } else {
                        changed.size[0]
                    },
                    3.5
                );
                assert_ne!(settings, changed);
                let m = app.editor.editable_mesh(id).unwrap();
                assert!((0..m.face_count()).all(|f| m.face_vertices(f).unwrap().len() == 4));
                app.editor.project.validate().unwrap();
                click_action(&mut app, Action::Primitive(P::Collapse));
                assert_eq!(app.primitive_panel_rect().unwrap()[3], 38.);
                click_action(&mut app, Action::Primitive(P::Collapse));
                click_action(&mut app, Action::Primitive(P::Reset));
                assert_eq!(app.selected_primitive().unwrap().1, settings);
                click_action(&mut app, Action::Primitive(P::Close));
                assert!(app.primitive_panel_rect().is_none());
                assert!(app.editor.project.node(id).is_some());
            }
        }
    }
}

#[test]
fn primitive_slider_cancel_commit_and_invalid_integer_use_document_transactions() {
    use ui::PrimitiveAction as P;
    let mut app = control_app();
    app.action(Action::Studio(ui::StudioAction::Mesh(0)));
    let before = app.editor.project.clone();
    let history = app.editor.undo_len();
    let (rect, _, _) = begin_visible_slider(&mut app, Action::Primitive(P::Field(0)));
    move_on_track(&mut app, rect, 0.65);
    assert_ne!(app.editor.project, before);
    app.action(Action::Cancel);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), history);
    assert!(app.primitive_panel_rect().is_some());
    let (rect, _, _) = begin_visible_slider(&mut app, Action::Primitive(P::Field(3)));
    move_on_track(&mut app, rect, 0.2);
    app.modifiers = ModifiersState::SHIFT;
    move_on_track(&mut app, rect, 0.4);
    move_on_track(&mut app, rect, 0.5);
    app.mouse_up(MouseButton::Left);
    app.modifiers = ModifiersState::empty();
    assert_eq!(app.editor.undo_len(), history + 1);
    assert!(app.selected_primitive().unwrap().1.segments[0] > 1);
    let after = app.editor.project.clone();
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
    app.action(Action::Redo);
    assert_eq!(app.editor.project, after);
    click_action(&mut app, Action::Primitive(P::Field(3)));
    replace_draft(&mut app, "2.5");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_some());
    assert!(app.surface.input_error.is_some());
    assert_eq!(app.editor.project, after);
    app.action(Action::Cancel);
    assert!(app.primitive_panel_rect().is_some());
    app.action(Action::Cancel);
    assert!(app.primitive_panel_rect().is_none());
}

#[test]
fn curve_creation_popup_owns_input_and_edits_the_source_in_both_themes() {
    use ui::{CurveAction as C, PrimitiveAction as P};
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut app = control_app();
            app.preferences.theme = theme;
            app.preferences.animation_visible = false;
            app.scale = scale; app.width = 1100.; app.height = 620.;
            click_action(&mut app, Action::DropdownOpen(ui::DropdownKind::CurveCreate));
            click_action(&mut app, Action::DropdownChoose(0));
            let id = app.editor.selected.unwrap();
            assert_eq!(app.primitive_panel, Some(id));
            let rect = app.primitive_panel_rect().unwrap();
            rebuild_ui(&mut app);
            assert!(app.ui.rects.iter().any(|r| (r.color[3] - 0.90).abs() < 0.001));
            let gap = [rect[0] + 5., rect[1] + 42.];
            assert!(!app.canvas_input_at(gap));
            assert_eq!(app.ui.hit(gap), Some(Action::Primitive(P::Backdrop)));
            app.mouse_move(gap);
            let distance = app.distance;
            app.scroll(-1.);
            assert_eq!(app.distance, distance);
            let original = app.editor.project.clone();
            let history = app.editor.undo_len();
            click_action(&mut app, Action::Curve(C::Field(0)));
            replace_draft(&mut app, "3,5");
            app.action(Action::ConfirmInput);
            assert_eq!(app.selected_curve().unwrap().1.points[0][0], 3.5);
            assert_eq!(app.editor.undo_len(), history + 1);
            app.action(Action::Undo);
            assert_eq!(app.editor.project, original);
            click_action(&mut app, Action::Curve(C::AddGeometry));
            assert!(app.editor.project.node(id).unwrap().mesh.is_some());
            let original = app.editor.project.clone();
            let (track, _, _) = begin_visible_slider(&mut app, Action::Curve(C::Field(4)));
            move_on_track(&mut app, track, 0.75);
            assert_ne!(app.editor.project, original);
            app.action(Action::Cancel);
            assert_eq!(app.editor.project, original);
            assert_eq!(app.primitive_panel, Some(id));
            click_action(&mut app, Action::Primitive(P::Close));
            assert!(app.primitive_panel.is_none());
            app.action(Action::Primitive(P::Open));
            assert_eq!(app.primitive_panel, Some(id));
            app.action(Action::Select(1));
            rebuild_ui(&mut app);
            assert!(app.primitive_panel.is_none());
            app.editor.project.validate().unwrap();
        }
    }
}

#[test]
fn curve_popup_scrolls_on_short_windows_and_prunes_after_conversion() {
    use ui::{CurveAction as C, PrimitiveAction as P};
    let mut app = control_app();
    app.preferences.animation_visible = false;
    app.width = 1100.; app.height = 420.;
    app.action(Action::Curve(C::Create(0)));
    app.action(Action::Curve(C::AddGeometry));
    let rect = app.primitive_panel_rect().unwrap();
    assert!(rect[3] < app.curve_creation_height());
    assert!(rect[1] >= app.viewport()[1]);
    assert!(rect[1] + rect[3] <= app.viewport()[1] + app.viewport()[3]);
    app.mouse_move([rect[0] + 5., rect[1] + 42.]);
    let distance = app.distance;
    app.scroll(-100.);
    assert!(app.curves.popup_scroll > 0.);
    assert_eq!(app.distance, distance);
    rebuild_ui(&mut app);
    assert!(app.ui.action_point(&Action::Curve(C::RemoveGeometry)).is_some());
    assert!(app.ui.action_point(&Action::Primitive(P::Close)).is_some());
    click_action(&mut app, Action::Primitive(P::Collapse));
    assert_eq!(app.primitive_panel_rect().unwrap()[3], 38.);
    click_action(&mut app, Action::Primitive(P::Collapse));
    app.action(Action::Curve(C::Convert));
    rebuild_ui(&mut app);
    assert!(app.primitive_panel.is_none());
    assert!(app.selected_mesh_for_uv());
}

#[test]
fn primitive_parameters_detach_instances_and_stop_after_component_edits_or_selection_change() {
    use ui::{AuthorAction as A, PrimitiveAction as P};
    let mut app = control_app();
    app.action(Action::Studio(ui::StudioAction::Mesh(0)));
    let node = app.editor.selected.unwrap();
    let old_mesh = app.editor.project.node(node).unwrap().mesh.unwrap();
    let mut instance = app.editor.project.node(node).unwrap().clone();
    instance.id = app.editor.project.next_id();
    app.editor.project.nodes.push(instance);
    let original = app.editor.project.mesh(old_mesh).unwrap().clone();
    app.primitive_confirm(&Action::Primitive(P::Field(0)), "4")
        .unwrap();
    assert_ne!(app.editor.project.node(node).unwrap().mesh, Some(old_mesh));
    assert_eq!(app.editor.project.mesh(old_mesh), Some(&original));
    app.action(Action::Author(A::SelectMode(ui::MeshSelection::Face)));
    assert!(app.primitive_panel_rect().is_none());
    let before = app.editor.project.clone();
    assert!(app
        .primitive_confirm(&Action::Primitive(P::Field(0)), "5")
        .is_err());
    assert_eq!(app.editor.project, before);
    app.authoring.faces.insert(0);
    app.action(Action::Author(A::DeleteFaces));
    app.action(Action::Mode(0));
    assert!(app.selected_primitive().is_none());
    app.action(Action::Primitive(P::Open));
    assert!(app.primitive_panel_rect().is_none());
    app.action(Action::Undo);
    app.action(Action::Primitive(P::Open));
    assert!(app.primitive_panel_rect().is_some());
    app.action(Action::Select(1));
    rebuild_ui(&mut app);
    assert!(app.primitive_panel.is_none());
    app.action(Action::Select(node));
    app.editor.project.node_mut(node).unwrap().locked = true;
    assert!(app.selected_primitive().is_none());
    assert!(app
        .primitive_confirm(&Action::Primitive(P::Field(0)), "6")
        .is_err());
}

#[test]
fn invalid_numeric_draft_keeps_focus_and_blocks_selection_without_mutation() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    click_action(&mut app, Action::EditTransform(0));
    replace_draft(&mut app, "NaN");
    app.action(Action::ConfirmInput);
    assert_eq!(app.input.as_ref().unwrap().value, "NaN");
    assert!(app.surface.input_error.is_some());
    click_action(&mut app, Action::Select(2));
    assert_eq!(app.editor.selected, Some(1));
    assert_eq!(
        app.input.as_ref().unwrap().purpose,
        Action::EditTransform(0)
    );
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn outside_selection_commits_the_field_to_its_original_object() {
    let mut app = control_app();
    let other = app.editor.project.node(2).unwrap().clone();
    click_action(&mut app, Action::EditTransform(0));
    replace_draft(&mut app, "2,75");
    click_action(&mut app, Action::Select(2));
    assert!(app.input.is_none());
    assert_eq!(app.editor.selected, Some(2));
    assert_eq!(
        app.editor.project.node(1).unwrap().transform.translation.x,
        2.75
    );
    assert_eq!(app.editor.project.node(2).unwrap(), &other);
    assert_eq!(app.editor.undo_len(), 1);
}

#[test]
fn accepting_an_unchanged_field_preserves_full_precision_and_history() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    click_action(&mut app, Action::EditTransform(0));
    // The display intentionally rounds; simply visiting it must not round data.
    assert_ne!(
        app.input.as_ref().unwrap().value,
        before.node(1).unwrap().transform.translation.x.to_string()
    );
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none());
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn a_slider_gesture_with_multiple_updates_commits_one_undo_step() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    let (rect, _, _) = begin_visible_slider(&mut app, Action::EditTransform(0));
    for fraction in [0.6, 0.7, 0.8] {
        move_on_track(&mut app, rect, fraction);
        assert_eq!(app.editor.undo_len(), 0);
    }
    assert!(app.editor.project.node(1).unwrap().transform.translation.x > 5.);
    app.mouse_up(MouseButton::Left);
    let after = app.editor.project.clone();
    assert!(app.drag.is_none());
    assert_eq!(app.editor.undo_len(), 1);
    assert!(app.editor.undo());
    assert_eq!(app.editor.project, before);
    assert!(app.editor.redo());
    assert_eq!(app.editor.project, after);
}

#[test]
fn changing_shift_precision_during_a_slider_drag_preserves_the_current_value() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    let (rect, range, _) = begin_visible_slider(&mut app, Action::EditTransform(0));
    move_on_track(&mut app, rect, 0.7);
    let coarse_value = app.editor.project.node(1).unwrap().transform.translation.x;
    assert_ne!(app.editor.project, before);
    let mut pointer = app.mouse;

    app.modifiers = ModifiersState::SHIFT;
    app.mouse_move(pointer);
    assert_eq!(
        app.editor.project.node(1).unwrap().transform.translation.x,
        coarse_value
    );
    pointer[0] += 10.;
    app.mouse_move(pointer);
    let fine_value = app.editor.project.node(1).unwrap().transform.translation.x;
    let expected_delta = 10. / rect[2] as f64 * (range.max - range.min) * 0.1;
    assert!(
        ((fine_value - coarse_value) as f64 - expected_delta).abs() <= range.step * 0.51 + 1e-5
    );
    assert!(fine_value > coarse_value);

    app.modifiers = ModifiersState::empty();
    app.mouse_move(pointer);
    assert_eq!(
        app.editor.project.node(1).unwrap().transform.translation.x,
        fine_value
    );
    app.action(Action::Cancel);
    assert!(app.drag.is_none());
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn thumb_capture_and_vertical_motion_do_not_round_the_exact_start_value() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    rebuild_ui(&mut app);
    let (rect, range, value) = app
        .ui
        .hits
        .iter()
        .find_map(|hit| match &hit.action {
            Action::Slide(field, range, value) if **field == Action::EditTransform(0) => {
                Some((hit.rect, *range, *value))
            }
            _ => None,
        })
        .unwrap();
    let pointer = [
        rect[0] + rect[2] * range.ratio(value) + 5.,
        rect[1] + rect[3] * 0.5,
    ];
    app.mouse_move(pointer);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Slider(_))));
    app.mouse_move([pointer[0], pointer[1] + 200.]);
    app.modifiers = ModifiersState::SHIFT;
    app.mouse_move([pointer[0], pointer[1] - 100.]);
    app.modifiers = ModifiersState::empty();
    app.mouse_move(pointer);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn fine_slider_adjustment_reaches_values_between_coarse_decimal_steps() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    let (rect, range, start) = begin_visible_slider(&mut app, Action::EditTransform(0));
    app.modifiers = ModifiersState::SHIFT;
    app.mouse_move(app.mouse);
    let desired_delta = range.step * 0.2;
    let pixels = desired_delta / (range.max - range.min) / 0.1 * rect[2] as f64;
    app.mouse_move([app.mouse[0] + pixels as f32, app.mouse[1]]);
    let value = app.editor.project.node(1).unwrap().transform.translation.x as f64;
    assert!(value > start);
    assert!(value - start < range.step * 0.5);
    app.action(Action::Cancel);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn shift_keeps_integer_slider_values_integral() {
    let mut app = control_app();
    app.action(Action::AnimationPanel);
    let (rect, range, start) = begin_visible_slider(&mut app, Action::EditFps);
    app.modifiers = ModifiersState::SHIFT;
    app.mouse_move(app.mouse);
    let pixels = 1.2 / (range.max - range.min) / 0.1 * rect[2] as f64;
    app.mouse_move([app.mouse[0] + pixels as f32, app.mouse[1]]);
    assert_eq!(app.editor.project.fps, (start + 1.) as u32);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);
}

#[test]
fn reversing_after_dragging_beyond_a_limit_responds_without_dead_travel() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    let (rect, range, _) = begin_visible_slider(&mut app, Action::EditTransform(0));
    let outside = [rect[0] + rect[2] * 3., rect[1] + rect[3] * 0.5];
    app.mouse_move(outside);
    assert_eq!(
        app.editor.project.node(1).unwrap().transform.translation.x as f64,
        range.max
    );
    app.mouse_move([outside[0] - 2., outside[1]]);
    assert!(app.editor.project.node(1).unwrap().transform.translation.x < range.max as f32);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);
    app.editor.undo();
    assert_eq!(app.editor.project, before);
}

#[test]
fn cancel_restores_a_slider_without_closing_its_animation_panel() {
    let mut app = control_app();
    app.action(Action::AnimationPanel);
    let before = app.editor.project.clone();
    let (rect, _, _) = begin_visible_slider(&mut app, Action::EditFps);
    move_on_track(&mut app, rect, 0.8);
    assert_ne!(app.editor.project.fps, before.fps);
    app.action(Action::Cancel);
    assert!(app.animation_panel);
    assert!(app.drag.is_none());
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn cancel_discards_title_draft_and_keeps_the_project_menu_open() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    app.action(Action::ToggleProjectMenu);
    click_action(&mut app, Action::EditTitle);
    replace_draft(&mut app, "Несохранённое название");
    app.action(Action::Cancel);
    assert!(app.input.is_none());
    assert_eq!(app.menu, Some(Menu::Project));
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn returning_a_slider_to_its_original_thumb_preserves_exact_value_and_no_undo() {
    let mut app = control_app();
    let before = app.editor.project.clone();
    let (rect, range, value) = begin_visible_slider(&mut app, Action::EditTransform(0));
    move_on_track(&mut app, rect, 0.8);
    assert_ne!(app.editor.project, before);
    move_on_track(&mut app, rect, range.ratio(value));
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn cancel_restores_export_dimensions_and_frame_time_without_document_history() {
    let mut app = control_app();
    app.action(Action::Mode(3));
    app.inspector_scroll = 1250.;
    let export_size = [
        app.editor.project.render_settings.output_width,
        app.editor.project.render_settings.output_height,
    ];
    let (rect, _, _) = begin_visible_slider(&mut app, Action::EditWidth);
    move_on_track(&mut app, rect, 0.85);
    assert_ne!(
        app.editor.project.render_settings.output_width,
        export_size[0]
    );
    app.action(Action::Cancel);
    assert_eq!(
        [
            app.editor.project.render_settings.output_width,
            app.editor.project.render_settings.output_height
        ],
        export_size
    );
    assert_eq!(app.editor.undo_len(), 0);

    app.action(Action::AnimationPanel);
    app.editor.set_time(1.25).unwrap();
    let (rect, _, _) = begin_visible_slider(&mut app, Action::EditFrame);
    move_on_track(&mut app, rect, 0.8);
    assert!(app.editor.time > 4.);
    app.action(Action::Cancel);
    assert_eq!(app.editor.time, 1.25);
    assert!(app.animation_panel);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn quoted_unicode_path_is_applied_as_literal_text_with_internal_spaces() {
    let mut app = control_app();
    app.dialog = Some(FileDialog {
        purpose: FilePurpose::Save,
        path: "/__poseforge_test_only__/before.poseforge".into(),
        entries: Vec::new(),
        offset: 0,
    });
    click_action(&mut app, Action::EditPath);
    replace_draft(
        &mut app,
        "  \"/__poseforge_test_only__/Персонаж А/новая поза.poseforge\"  ",
    );
    app.action(Action::ConfirmInput);
    assert!(app.input.is_none());
    assert_eq!(
        app.dialog.as_ref().unwrap().path,
        "/__poseforge_test_only__/Персонаж А/новая поза.poseforge"
    );
    assert!(app.native_picker.is_none());
    assert!(app.job.is_none());
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn tab_uses_only_visible_owner_fields_and_invalid_draft_blocks_navigation() {
    let mut app = control_app();
    app.action(Action::AnimationPanel);
    click_action(&mut app, Action::EditFps);
    assert!(!app
        .ui
        .hits
        .iter()
        .any(|hit| matches!(hit.action, Action::EditTransform(_) | Action::SearchScene)));
    app.focus_next_field(false);
    assert_eq!(app.input.as_ref().unwrap().purpose, Action::EditDuration);
    app.focus_next_field(true);
    assert_eq!(app.input.as_ref().unwrap().purpose, Action::EditFps);
    let before = app.editor.project.clone();
    replace_draft(&mut app, "241");
    app.focus_next_field(false);
    assert_eq!(app.input.as_ref().unwrap().purpose, Action::EditFps);
    assert_eq!(app.input.as_ref().unwrap().value, "241");
    assert!(app.surface.input_error.is_some());
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);

    app.action(Action::Cancel);
    let clips = app.editor.project.clips.len();
    click_action(&mut app, Action::CreateClip);
    app.focus_next_field(false);
    assert_eq!(app.input.as_ref().unwrap().purpose, Action::EditFrame);
    assert_eq!(app.editor.project.clips.len(), clips);
    assert_eq!(app.editor.undo_len(), 0);
}

/// Dumps the real layout primitives without requiring a GPU or native window.
/// A separate CPU compositor is used only to inspect layout, not native behavior.
#[test]
fn emit_ui_previews_when_requested() {
    let Some(directory) = std::env::var_os("POSEFORGE_QA_UI_OUTPUT") else {
        return;
    };
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let cases = std::env::var("POSEFORGE_QA_UI_CASES").ok();
    for (theme_name, theme) in [("dark", ui::Theme::Dark), ("light", ui::Theme::Light)] {
        for scale in [1., 2.] {
            let mut qa_renderer: Option<Renderer> = None;
            for case in [
                "rig-build31",
                "rig-bind31",
                "rig-pose31",
                "normals31",
                "normals-faces311",
                "normals-direction311",
                "handles-face313",
                "handles-cut313",
                "handles-extrude312",
                "handles-input312",
                "handles-uv312",
                "handles311",
                "handles-compact311",
                "shape31",
                "uv30",
                "uv-compact30",
                "rig30",
                "primitive30",
                "camera30",
                "workspace29",
                "settings29",
                "accent29",
                "rig29",
                "fox28",
                "fox-edit28",
                "selection27",
                "light27",
                "camera27",
                "edit27",
                "materials26",
                "layers26",
                "hidden26",
                "camera26",
                "scene25",
                "scene-filter25",
                "materials24",
                "library24",
                "scene24",
                "uv23",
                "knife23",
                "proportional23",
                "fluid-cache23",
                "object-properties222",
                "modeling22-edge",
                "modeling22-topology",
                "npr22-palette",
                "npr22-contours",
                "modeling21-tools",
                "modeling21-bevel",
                "modeling21-loop",
                "modeling21-xray",
                "primitive-cube20",
                "primitive-sphere20",
                "primitive-cloth20",
                "quad-edit20",
                "mesh19",
                "mesh-menu19",
                "light-menu19",
                "spot19",
                "npr18",
                "npr-palette18",
                "npr-create18",
                "library17",
                "nodes17",
                "camera17",
                "collections17",
                "environment17",
                "physics17",
                "weights17",
                "view-menu16",
                "library16",
                "material-menu16",
                "material-add16",
                "camera16",
                "viewport16",
                "materials15",
                "sections15",
                "search15",
                "render15",
                "gizmo15",
                "collapsed15",
                "workspace14",
                "collapsed14",
                "materials14",
                "camera14",
                "animation14",
                "nodes14",
                "hdri13",
                "rays13",
                "nodes13",
                "node-color13",
                "node-menu13",
                "material-create12",
                "material-uv12",
                "light-temperature12",
                "light-shape12",
                "contact12",
                "mesh-edit11",
                "mesh-faces11",
                "physics11",
                "rig11",
                "rig-bones11",
                "properties-material10",
                "properties-object10",
                "properties-render10",
                "properties-fluid10",
                "properties-camera10",
                "properties-menu10",
                "fiber09",
                "absorption09",
                "presets09",
                "physics08",
                "background08",
                "scene-mesh081",
                "properties081",
                "layers08",
                "glass08",
                "pose",
                "slider-active",
                "slider-fine",
                "dropdown-render",
                "dropdown-material",
                "dropdown-light",
                "area-light",
                "camera-object",
                "camera-view",
                "camera-transparent",
                "material",
                "materials-new",
                "materials-layered",
                "light-shadow",
                "settings-shadow-filter",
                "lights",
                "light-move",
                "light-rotate",
                "light-render",
                "render-quality",
                "render-environment",
                "render-output",
                "fluid",
                "fluid-parameters",
                "fluid-status",
                "animation",
                "animation-dopesheet",
                "animation-selection",
                "animation-ghosts",
                "animation-range",
                "animation-compact",
                "animation-invalid",
                "import-path",
                "invalid-number",
                "project-menu",
                "view-menu",
                "view-menu-off",
                "import-models",
                "import-models-animated",
                "import-blender-progress",
                "color-material",
                "color-alpha",
                "color-emission",
                "color-light",
                "color-environment",
                "color-invalid",
                "report-import",
                "report-error",
                "report-rig",
            ] {
                if cases
                    .as_ref()
                    .is_some_and(|cases| !cases.split(',').any(|wanted| wanted.trim() == case))
                {
                    continue;
                }
                if (case.ends_with("14")
                    || case.ends_with("15")
                    || case.ends_with("16")
                    || case.ends_with("17"))
                    && theme != ui::Theme::Dark
                {
                    continue;
                }
                let mut app = App::new(None);
                app.preferences = Preferences::default();
                app.preferences.theme = theme;
                app.preferences.left_panel = true;
                app.preferences.right_panel = true;
                app.width = 1100.;
                app.height = 760.;
                app.scale = scale;
                app.editor.mode = EditorMode::Pose;
                match case {
                    "rig-build31" | "rig-bind31" | "rig-pose31" => {
                        app.width = 1280.;
                        app.height = 900.;
                        app.preferences.accent = ui::Accent::Violet;
                        app.action(Action::Select(9));
                        if case == "rig-bind31" {
                            app.authoring.bind_target = app
                                .editor
                                .project
                                .nodes
                                .iter()
                                .find(|n| n.skin.is_some())
                                .map(|n| n.id);
                        }
                        app.action(Action::RigStage(match case {
                            "rig-build31" => 0,
                            "rig-bind31" => 1,
                            _ => 2,
                        }));
                        if case == "rig-pose31" {
                            let index = BoneRole::ALL
                                .iter()
                                .position(|r| *r == BoneRole::LeftHand)
                                .unwrap();
                            app.select_role(index, true);
                        }
                        app.frame_all();
                    }
                    "handles311" | "handles-compact311" | "handles-extrude312"
                    | "handles-input312" | "handles-uv312" | "handles-face313"
                    | "handles-cut313" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.preferences.accent = ui::Accent::Violet;
                        app.target = Vec3::ZERO;
                        app.width = if case == "handles-compact311" {
                            900.
                        } else {
                            1440.
                        };
                        app.height = if case == "handles-compact311" {
                            600.
                        } else {
                            900.
                        };
                        app.create_primitive(0);
                        app.primitive_panel = None;
                        app.frame_all();
                        if case == "handles-uv312" {
                            app.action(Action::Uv(ui::UvAction::Open));
                        }
                        let extrude = case == "handles-extrude312";
                        app.action(Action::Author(ui::AuthorAction::SelectMode(
                            if extrude || case == "handles-face313" {
                                ui::MeshSelection::Face
                            } else {
                                ui::MeshSelection::Edge
                            },
                        )));
                        app.authoring.edges = BTreeSet::from([(0, 1)]);
                        app.authoring.faces = BTreeSet::from([0]);
                        app.action(Action::Author(if extrude {
                            ui::AuthorAction::Extrude
                        } else if case == "handles-face313" {
                            ui::AuthorAction::Bevel
                        } else if case == "handles-cut313" {
                            ui::AuthorAction::LoopCut
                        } else {
                            ui::AuthorAction::EdgeBevel
                        }));
                        app.ui = app.build_ui();
                        if let Some(point) = app
                            .ui
                            .action_point(&Action::Author(ui::AuthorAction::LiveDrag(0)))
                        {
                            app.mouse_move(point);
                        }
                        if case == "handles311" {
                            app.mouse_down(MouseButton::Left);
                            app.mouse_move([app.mouse[0] + 12., app.mouse[1]]);
                        }
                        if case == "handles-input312" {
                            app.action(Action::Author(ui::AuthorAction::LiveField(0)));
                            app.input.as_mut().unwrap().select_all();
                            app.input.as_mut().unwrap().insert("0.125");
                        }
                    }
                    "normals31" | "normals-faces311" | "normals-direction311" | "shape31" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.width = 1280.;
                        app.height = 900.;
                        app.preferences.accent = ui::Accent::Violet;
                        app.target = Vec3::ZERO;
                        app.create_primitive(0);
                        app.primitive_confirm(
                            &Action::Primitive(ui::PrimitiveAction::Field(3)),
                            "3",
                        )
                        .unwrap();
                        app.primitive_panel = None;
                        app.action(Action::Author(ui::AuthorAction::SelectMode(
                            ui::MeshSelection::Face,
                        )));
                        if case.starts_with("normals") {
                            app.action(Action::SurfaceTab(10));
                            app.action(Action::Normal(ui::NormalAction::Show));
                            if case == "normals-faces311" {
                                app.action(Action::Normal(ui::NormalAction::Display(1)));
                                app.action(Action::Normal(ui::NormalAction::AutoLength));
                            } else if case == "normals-direction311" {
                                app.preferences.collapsed_sections.extend([1500, 1501]);
                            }
                        } else {
                            app.action(Action::Author(ui::AuthorAction::SelectMode(
                                ui::MeshSelection::Edge,
                            )));
                            app.authoring.edges = app
                                .mesh_cache(
                                    app.editor
                                        .editable_mesh(app.editor.selected.unwrap())
                                        .unwrap(),
                                )
                                .graph
                                .edges
                                .iter()
                                .take(3)
                                .copied()
                                .collect();
                            app.inspector_scroll = app.inspector_max_scroll();
                        }
                        app.frame_all();
                    }
                    "uv30" | "uv-compact30" | "primitive30" | "camera30" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.preferences.accent = ui::Accent::Violet;
                        app.target = Vec3::ZERO;
                        app.width = 1440.;
                        app.height = 900.;
                        app.create_primitive(if case == "primitive30" { 4 } else { 0 });
                        app.frame_all();
                        if case.starts_with("uv") {
                            if case == "uv-compact30" {
                                app.width = 1000.;
                                app.height = 680.;
                            }
                            let id = app.editor.selected.unwrap();
                            app.editor
                                .edit_mesh(
                                    id,
                                    poseforge_core::MeshEdit::Uv(poseforge_core::UvEdit::Unwrap),
                                )
                                .unwrap();
                            app.action(Action::Uv(ui::UvAction::Open));
                            app.authoring.faces = BTreeSet::from([0]);
                            app.frame_all();
                        } else if case == "camera30" {
                            app.primitive_panel = None;
                            app.action(Action::CameraView);
                        }
                    }
                    "rig30" => {
                        app.width = 1440.;
                        app.height = 960.;
                        app.preferences.accent = ui::Accent::Violet;
                        app.set_properties_tab(8);
                        app.sync_rig_pose();
                        let index = BoneRole::ALL
                            .iter()
                            .position(|r| *r == BoneRole::LeftHand)
                            .unwrap();
                        app.select_role(index, true);
                        let chain = app.pose_chain(app.editor.selected.unwrap()).unwrap();
                        let eval = app.editor.evaluated().unwrap();
                        let end = eval.global[&chain[2]].w_axis.truncate();
                        app.editor
                            .apply_ik(
                                chain[0],
                                chain[1],
                                chain[2],
                                end + Vec3::new(0.15, -0.15, 0.05),
                                Vec3::Z,
                            )
                            .unwrap();
                        app.frame_all();
                    }
                    "workspace29" | "settings29" | "accent29" | "rig29" => {
                        app.preferences.accent = ui::Accent::Violet;
                        app.editor.mode = EditorMode::Scene;
                        if case == "rig29" {
                            let id = app
                                .editor
                                .project
                                .nodes
                                .iter()
                                .find(|n| n.is_joint && n.parent.is_some())
                                .unwrap()
                                .id;
                            app.select_object(id);
                            app.frame_all();
                        } else {
                            app.editor = Editor::new(Project::default());
                            app.editor.mode = EditorMode::Scene;
                            app.action(Action::Studio(ui::StudioAction::Mesh(4)));
                            app.frame_all();
                            app.timeline.expanded = true;
                            app.editor.mode = EditorMode::Animation;
                            app.width = 1280.;
                            app.height = 940.;
                            if case == "workspace29" {
                                app.width = 1000.;
                                app.height = 680.;
                            } else {
                                app.primitive_panel = None;
                                app.action(Action::SurfaceTab(9));
                                if case == "accent29" {
                                    app.ui = app.build_ui();
                                    app.action(Action::DropdownOpen(ui::DropdownKind::Accent));
                                }
                            }
                        }
                    }
                    "fox28" | "fox-edit28" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.target = Vec3::ZERO;
                        app.action(Action::Studio(ui::StudioAction::Mesh(4)));
                        app.scene_tools
                            .material_expanded
                            .insert(app.editor.selected.unwrap());
                        app.frame_all();
                        if case == "fox-edit28" {
                            app.action(Action::Author(ui::AuthorAction::SelectMode(
                                ui::MeshSelection::Face,
                            )));
                            app.authoring.faces = BTreeSet::from([128, 129, 130, 131]);
                        }
                    }
                    "selection27" | "light27" | "camera27" | "edit27" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        let node = app.editor.selected.unwrap();
                        app.primitive_panel = None;
                        app.frame_all();
                        if case == "light27" {
                            app.editor.project.lights[0].kind = poseforge_core::LightKind::Spot;
                            app.aim_light_at(0, node, true);
                            app.select_light(0);
                            app.inspector_scroll = 430.;
                        } else if case == "camera27" {
                            app.action(Action::CreateCamera);
                        } else if case == "edit27" {
                            app.action(Action::Author(ui::AuthorAction::SelectMode(
                                ui::MeshSelection::Face,
                            )));
                            app.authoring.faces = BTreeSet::from([0]);
                        }
                    }
                    "materials26" | "layers26" | "hidden26" | "camera26" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.width = 1100.;
                        app.height = 760.;
                        app.target = Vec3::ZERO;
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        app.editor.project.title = "ArtForge · рабочая сцена".into();
                        app.frame_all();
                        let node = app.editor.selected.unwrap();
                        app.scene_tools.material_expanded.insert(node);
                        app.set_properties_tab(1);
                        app.action(Action::Author(ui::AuthorAction::SelectMode(
                            ui::MeshSelection::Face,
                        )));
                        app.authoring.faces = BTreeSet::from([0, 2, 4]);
                        app.action(Action::MaterialBrowse(true));
                        app.action(Action::SurfaceCreateMaterialPreset(2));
                        app.action(Action::MaterialSelect(app.selected_material_id().unwrap()));
                        app.action(Action::MaterialBrowse(false));
                        app.primitive_panel = None;
                        if case == "layers26" {
                            for _ in 0..5 {
                                app.action(Action::Studio(ui::StudioAction::LayerAdd));
                            }
                            app.timeline.expanded = true;
                            app.preferences.tools_expanded = false;
                        } else if case == "hidden26" {
                            app.action(Action::ToggleLeftPanel);
                            app.action(Action::ToggleAnimationPanel);
                            app.action(Action::ToggleAnimationPanel);
                        } else if case == "camera26" {
                            app.action(Action::Mode(0));
                            app.action(Action::CreateCamera);
                            app.action(Action::CameraView);
                            app.camera_navigation_lock = true;
                            app.action(Action::SurfaceTab(0));
                        }
                        app.prepare_material_previews();
                        let deadline = Instant::now() + std::time::Duration::from_secs(60);
                        while app.material_previews.busy() && Instant::now() < deadline {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            app.prepare_material_previews();
                        }
                    }
                    "scene25" | "scene-filter25" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.width = 1280.;
                        app.height = 800.;
                        app.target = Vec3::ZERO;
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        let cube = app.editor.selected.unwrap();
                        let mesh = app.editor.project.node(cube).unwrap().mesh.unwrap();
                        app.editor.project.node_mut(cube).unwrap().name = "Куб".into();
                        let accent = app
                            .editor
                            .create_library_material(poseforge_core::MaterialPreset::Toon)
                            .unwrap();
                        app.editor
                            .assign_face_material(mesh, &[cube], &BTreeSet::from([0, 2]), accent)
                            .unwrap();
                        app.action(Action::Studio(ui::StudioAction::Mesh(2)));
                        let sphere = app.editor.selected.unwrap();
                        app.editor
                            .project
                            .node_mut(sphere)
                            .unwrap()
                            .transform
                            .translation
                            .x = 2.;
                        app.action(Action::CreateCamera);
                        app.action(Action::SurfaceAddLight(2));
                        let folder = app.editor.project.next_id();
                        app.editor
                            .transact("Folder", |p| {
                                p.collections.push(poseforge_core::Collection {
                                    id: folder,
                                    name: "Модели и свет".into(),
                                    members: vec![sphere],
                                    light_members: vec![1],
                                    ..Default::default()
                                })
                            })
                            .unwrap();
                        app.action(Action::Select(cube));
                        app.set_properties_tab(1);
                        app.surface.material_library = false;
                        app.scene_tools.material_expanded.insert(cube);
                        app.primitive_panel = None;
                        app.frame_all();
                        app.prepare_material_previews();
                        let deadline = Instant::now() + std::time::Duration::from_secs(60);
                        while app.material_previews.busy() && Instant::now() < deadline {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            app.prepare_material_previews();
                        }
                        if case == "scene-filter25" {
                            app.ui = app.build_ui();
                            app.action(Action::DropdownOpen(ui::DropdownKind::SceneFilter));
                        }
                    }

                    "materials24" | "library24" | "scene24" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.width = 1280.;
                        app.height = 800.;
                        app.target = Vec3::ZERO;
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        let node = app.editor.selected.unwrap();
                        app.editor.project.node_mut(node).unwrap().name =
                            "Куб · несколько материалов".into();
                        app.action(Action::Author(ui::AuthorAction::SelectMode(
                            ui::MeshSelection::Face,
                        )));
                        app.authoring.faces = BTreeSet::from([0, 2]);
                        app.set_properties_tab(1);
                        app.action(Action::MaterialBrowse(true));
                        app.action(Action::SurfaceCreateMaterialPreset(10));
                        app.action(Action::MaterialSelect(app.selected_material_id().unwrap()));
                        for preset in [2, 3, 5, 11] {
                            app.action(Action::SurfaceCreateMaterialPreset(preset));
                        }
                        app.action(Action::MaterialBrowse(case == "library24"));
                        app.frame_all();
                        if case == "scene24" {
                            app.action(Action::Author(ui::AuthorAction::ToggleEdit));
                            app.action(Action::CreateCamera);
                            app.action(Action::SurfaceAddLight(2));
                            app.set_properties_tab(3);
                        }
                        app.prepare_material_previews();
                        let deadline = Instant::now() + std::time::Duration::from_secs(60);
                        while app.material_previews.busy() && Instant::now() < deadline {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            app.prepare_material_previews();
                        }
                    }

                    "uv23" | "knife23" | "proportional23" | "fluid-cache23" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.target = Vec3::ZERO;
                        app.width = 1280.;
                        app.height = 800.;
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        app.frame_all();
                        app.action(Action::Author(ui::AuthorAction::SelectMode(
                            ui::MeshSelection::Vertex,
                        )));
                        app.primitive_panel = None;
                        match case {
                            "uv23" => {
                                let node = app.authoring.editing.unwrap();
                                app.editor
                                    .edit_mesh(
                                        node,
                                        poseforge_core::MeshEdit::Uv(
                                            poseforge_core::UvEdit::Unwrap,
                                        ),
                                    )
                                    .unwrap();
                                app.action(Action::Uv(ui::UvAction::Open));
                                app.action(Action::Uv(ui::UvAction::Stretch));
                            }
                            "knife23" => app.action(Action::Author(ui::AuthorAction::Knife)),
                            "proportional23" => {
                                app.authoring.vertices.insert(0);
                                app.authoring.proportional = true;
                                app.authoring.proportional_radius = 2.;
                            }
                            _ => {
                                app.authoring.editing = None;
                                app.set_properties_tab(4);
                                app.fluid_on = true;
                                app.editor.mode = EditorMode::Animation;
                                app.fluid = FluidWorld::new(
                                    Default::default(),
                                    vec![Vec3::new(0., 0.8, 0.)],
                                )
                                .unwrap();
                                app.fluid_preview.cache = fluid_cache::bake(
                                    &app.fluid,
                                    0.5,
                                    24.,
                                    1.,
                                    &AtomicBool::new(false),
                                    |_, _| {},
                                )
                                .unwrap();
                            }
                        }
                    }

                    "modeling22-edge" | "modeling22-topology" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.target = Vec3::ZERO;
                        app.width = 1440.;
                        app.height = 900.;
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        app.frame_all();
                        app.action(Action::Author(ui::AuthorAction::SelectMode(
                            ui::MeshSelection::Edge,
                        )));
                        app.authoring.edges = BTreeSet::from([(0, 1)]);
                        if case == "modeling22-edge" {
                            app.action(Action::Author(ui::AuthorAction::EdgeBevel));
                            app.inspector_scroll = 600.;
                        } else {
                            app.action(Action::Author(ui::AuthorAction::Inspect));
                            app.inspector_scroll = app.inspector_max_scroll();
                        }
                    }
                    "npr22-palette" | "npr22-contours" => {
                        app.width = 1440.;
                        app.height = 900.;
                        app.editor = Editor::new(
                            poseforge_io::load_project(
                                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                    .join("../../examples/npr-studio.artforge"),
                            )
                            .unwrap(),
                        );
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.editor.selected = Some(1012);
                        app.set_properties_tab(if case == "npr22-palette" { 1 } else { 3 });
                        if case == "npr22-palette" {
                            for id in 100..=108 {
                                app.preferences.collapsed_sections.insert(id);
                            }
                            app.inspector_scroll = app.inspector_max_scroll();
                        } else {
                            app.editor.project.render_settings.npr_contours.width = 2.;
                            app.inspector_scroll = app.inspector_max_scroll();
                        }
                    }
                    "modeling21-tools" | "modeling21-bevel" | "modeling21-loop"
                    | "modeling21-xray" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.target = Vec3::ZERO;
                        app.width = 1440.;
                        app.height = 900.;
                        app.action(Action::Studio(ui::StudioAction::Mesh(
                            if case == "modeling21-xray" { 2 } else { 0 },
                        )));
                        app.frame_all();
                        app.action(Action::Author(ui::AuthorAction::SelectMode(
                            ui::MeshSelection::Face,
                        )));
                        app.authoring.faces.insert(0);
                        if case == "modeling21-bevel" {
                            app.action(Action::Author(ui::AuthorAction::Bevel));
                            app.inspector_scroll = 420.;
                        } else if case == "modeling21-loop" {
                            app.action(Action::Author(ui::AuthorAction::SelectMode(
                                ui::MeshSelection::Edge,
                            )));
                            app.authoring.edges = BTreeSet::from([(0, 1)]);
                            app.action(Action::Author(ui::AuthorAction::LoopCut));
                            app.inspector_scroll = 420.;
                        } else if case == "modeling21-xray" {
                            app.action(Action::Author(ui::AuthorAction::SelectMode(
                                ui::MeshSelection::Vertex,
                            )));
                            app.authoring.xray = true;
                        }
                    }
                    "primitive-cube20" | "primitive-sphere20" | "primitive-cloth20"
                    | "quad-edit20" => {
                        app.editor = Editor::new(Project::default());
                        app.editor.mode = EditorMode::Scene;
                        app.target = Vec3::ZERO;
                        app.width = 1440.;
                        app.height = 900.;
                        let kind = match case {
                            "primitive-sphere20" => 2,
                            "primitive-cloth20" => 3,
                            _ => 0,
                        };
                        app.action(Action::Studio(ui::StudioAction::Mesh(kind)));
                        app.frame_all();
                        if case == "quad-edit20" {
                            app.action(Action::Author(ui::AuthorAction::SelectMode(
                                ui::MeshSelection::Face,
                            )));
                            app.authoring.faces.insert(0);
                        }
                    }
                    "mesh19" | "mesh-menu19" | "light-menu19" | "spot19" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/npr-studio.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.editor.selected = Some(1012);
                        if case == "spot19" {
                            app.action(Action::SurfaceAddLight(2));
                            app.preferences.collapsed_sections.insert(900);
                            let view = app.view();
                            let forward = (view.target - view.eye).normalize();
                            let right = forward.cross(view.up).normalize();
                            let light = &mut app.editor.project.lights[app.surface.light];
                            light.position = app.target;
                            light.direction =
                                (forward * 0.5 + right * 0.8 - view.up * 0.2).normalize();
                        } else {
                            app.action(Action::Author(ui::AuthorAction::SelectMode(
                                ui::MeshSelection::Edge,
                            )));
                            app.action(Action::Author(ui::AuthorAction::SelectAll));
                            if case != "mesh19" {
                                app.ui = app.build_ui();
                                app.action(Action::DropdownOpen(if case == "mesh-menu19" {
                                    ui::DropdownKind::MeshCreate
                                } else {
                                    ui::DropdownKind::LightCreate
                                }));
                            }
                        }
                    }
                    "npr18" | "npr-palette18" | "npr-create18" => {
                        app.width = 1440.;
                        app.height = 900.;
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/npr-studio.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.tool = Tool::Orbit;
                        app.editor.selected = Some(1012);
                        app.set_properties_tab(1);
                        app.preferences.collapsed_sections.extend([100, 101]);
                        if case == "npr-palette18" {
                            app.preferences
                                .collapsed_sections
                                .extend([102, 103, 104, 105]);
                        }
                        if case == "npr-create18" {
                            app.preferences.collapsed_sections.remove(&101);
                            app.ui = app.build_ui();
                            app.open_dropdown(ui::DropdownKind::MaterialCreate);
                            app.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
                        }
                    }
                    name if name.ends_with("17") => {
                        app.width = 1440.;
                        app.height = 900.;
                        let file = if case == "physics17" {
                            "cloth-collisions.artforge"
                        } else if case == "weights17" {
                            "rig-workshop.artforge"
                        } else {
                            "artwork-workspace.artforge"
                        };
                        app.editor = Editor::new(
                            poseforge_io::load_project(
                                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                    .join("../../examples")
                                    .join(file),
                            )
                            .unwrap(),
                        );
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.tool = Tool::Orbit;
                        app.editor.project.render_settings.reflection_samples = 1;
                        app.editor.project.render_settings.area_shadow_samples = 4;
                        app.preferences.performance_overlay = true;
                        app.editor.selected = app
                            .editor
                            .project
                            .nodes
                            .iter()
                            .find(|n| n.mesh.is_some())
                            .map(|n| n.id);
                        app.set_properties_tab(1);
                        app.preferences.collapsed_sections.insert(100);
                        match case {
                            "uv23" | "knife23" | "proportional23" | "fluid-cache23" => {
                                app.editor = Editor::new(Project::default());
                                app.editor.mode = EditorMode::Scene;
                                app.target = Vec3::ZERO;
                                app.width = 1280.;
                                app.height = 800.;
                                app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                                app.frame_all();
                                app.action(Action::Author(ui::AuthorAction::SelectMode(
                                    ui::MeshSelection::Vertex,
                                )));
                                app.primitive_panel = None;
                                match case {
                                    "uv23" => {
                                        let node = app.authoring.editing.unwrap();
                                        app.editor
                                            .edit_mesh(
                                                node,
                                                poseforge_core::MeshEdit::Uv(
                                                    poseforge_core::UvEdit::Unwrap,
                                                ),
                                            )
                                            .unwrap();
                                        app.action(Action::Uv(ui::UvAction::Open));
                                        app.action(Action::Uv(ui::UvAction::Stretch));
                                    }
                                    "knife23" => {
                                        app.action(Action::Author(ui::AuthorAction::Knife))
                                    }
                                    "proportional23" => {
                                        app.authoring.vertices.insert(0);
                                        app.authoring.proportional = true;
                                        app.authoring.proportional_radius = 2.;
                                    }
                                    _ => {
                                        app.authoring.editing = None;
                                        app.set_properties_tab(4);
                                        app.fluid_on = true;
                                        app.editor.mode = EditorMode::Animation;
                                        app.fluid = FluidWorld::new(
                                            Default::default(),
                                            vec![Vec3::new(0., 0.8, 0.)],
                                        )
                                        .unwrap();
                                        app.fluid_preview.cache = fluid_cache::bake(
                                            &app.fluid,
                                            0.5,
                                            24.,
                                            1.,
                                            &AtomicBool::new(false),
                                            |_, _| {},
                                        )
                                        .unwrap();
                                    }
                                }
                            }

                            "library17" => {
                                app.preferences.material_grid = true;
                                app.preferences.material_thumbnail = 52.;
                                for (i, preset) in
                                    poseforge_core::MaterialPreset::ALL.iter().enumerate()
                                {
                                    let mut m = preset.create();
                                    m.id = 20000 + i as u64;
                                    m.favorite = i % 3 == 0;
                                    m.category = "Студия".into();
                                    app.editor.project.materials.push(m);
                                }
                            }
                            "nodes17" => {
                                app.editor.selected = Some(1012);
                                app.action(Action::Node(ui::NodeAction::Open));
                            }
                            "camera17" => {
                                app.editor.selected = Some(9000);
                                app.action(Action::CameraView);
                                app.set_properties_tab(0);
                                app.inspector_scroll = 640.;
                            }
                            "collections17" => {
                                let members = app.editor.project.collections[0].members.clone();
                                app.scene_tools.selected = members.iter().copied().collect();
                                app.editor.selected = members.first().copied();
                                app.set_properties_tab(0);
                                app.tool = Tool::Move;
                            }
                            "environment17" => {
                                app.set_properties_tab(3);
                                app.action(Action::World(ui::WorldAction::Library));
                            }
                            "physics17" => {
                                app.editor.selected = app
                                    .editor
                                    .project
                                    .nodes
                                    .iter()
                                    .find(|n| {
                                        n.physics.is_some_and(|p| {
                                            p.kind == poseforge_core::PhysicsKind::Cloth
                                        })
                                    })
                                    .map(|n| n.id);
                                app.set_properties_tab(6);
                                app.inspector_scroll = 175.;
                            }
                            "weights17" => {
                                app.editor.selected = Some(500);
                                app.authoring.bind_target = Some(500);
                                app.authoring.skeleton =
                                    app.editor.project.rig_profiles.first().and_then(|p| p.root);
                                app.set_properties_tab(8);
                                app.action(Action::Author(ui::AuthorAction::Paint));
                                app.inspector_scroll = 280.;
                            }
                            _ => {}
                        }
                    }

                    "view-menu16" | "library16" | "material-menu16" | "material-add16"
                    | "camera16" | "viewport16" | "materials15" | "sections15" | "search15"
                    | "render15" | "gizmo15" | "collapsed15" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/lighting-lab.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.editor.selected = Some(1012);
                        app.tool = Tool::Orbit;
                        app.editor.project.render_settings.reflection_samples = 1;
                        app.editor.project.render_settings.area_shadow_samples = 4;
                        app.set_properties_tab(1);
                        for (i, preset) in poseforge_core::MaterialPreset::ALL.iter().enumerate() {
                            let mut material = preset.create();
                            material.id = 2000 + i as u64;
                            app.editor.project.materials.push(material);
                        }
                        app.surface.material_offset = 4;
                        // Keep the library high enough to show all four sphere rows at 760 px.
                        app.preferences.collapsed_sections.insert(100);
                        match case {
                            "uv23" | "knife23" | "proportional23" | "fluid-cache23" => {
                                app.editor = Editor::new(Project::default());
                                app.editor.mode = EditorMode::Scene;
                                app.target = Vec3::ZERO;
                                app.width = 1280.;
                                app.height = 800.;
                                app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                                app.frame_all();
                                app.action(Action::Author(ui::AuthorAction::SelectMode(
                                    ui::MeshSelection::Vertex,
                                )));
                                app.primitive_panel = None;
                                match case {
                                    "uv23" => {
                                        let node = app.authoring.editing.unwrap();
                                        app.editor
                                            .edit_mesh(
                                                node,
                                                poseforge_core::MeshEdit::Uv(
                                                    poseforge_core::UvEdit::Unwrap,
                                                ),
                                            )
                                            .unwrap();
                                        app.action(Action::Uv(ui::UvAction::Open));
                                        app.action(Action::Uv(ui::UvAction::Stretch));
                                    }
                                    "knife23" => {
                                        app.action(Action::Author(ui::AuthorAction::Knife))
                                    }
                                    "proportional23" => {
                                        app.authoring.vertices.insert(0);
                                        app.authoring.proportional = true;
                                        app.authoring.proportional_radius = 2.;
                                    }
                                    _ => {
                                        app.authoring.editing = None;
                                        app.set_properties_tab(4);
                                        app.fluid_on = true;
                                        app.editor.mode = EditorMode::Animation;
                                        app.fluid = FluidWorld::new(
                                            Default::default(),
                                            vec![Vec3::new(0., 0.8, 0.)],
                                        )
                                        .unwrap();
                                        app.fluid_preview.cache = fluid_cache::bake(
                                            &app.fluid,
                                            0.5,
                                            24.,
                                            1.,
                                            &AtomicBool::new(false),
                                            |_, _| {},
                                        )
                                        .unwrap();
                                    }
                                }
                            }

                            "view-menu16" => {
                                app.preferences.right_panel = false;
                                app.menu = Some(crate::chrome::Menu::View);
                            }
                            "camera16" => {
                                app.action(Action::CreateCamera);
                                app.action(Action::CameraAlignView);
                                app.action(Action::CameraView);
                                app.action(Action::Studio(ui::StudioAction::CameraLock));
                            }
                            "viewport16" => {
                                app.action(Action::CreateCamera);
                                app.preferences.right_panel = false;
                                app.preferences.left_panel = false;
                                app.preferences.tools_expanded = false;
                            }
                            "material-menu16" | "material-add16" => {
                                app.ui = app.build_ui();
                                app.open_dropdown(if case == "material-menu16" {
                                    ui::DropdownKind::MaterialActions
                                } else {
                                    ui::DropdownKind::MaterialCreate
                                });
                            }
                            "render15" => {
                                app.editor.mode = EditorMode::Render;
                                app.set_properties_tab(3);
                            }
                            "sections15" => {
                                app.preferences
                                    .collapsed_sections
                                    .extend([101, 102, 103, 104, 105, 106, 107, 108]);
                            }
                            "search15" => {
                                app.ui = app.build_ui(); app.open_dropdown(ui::DropdownKind::MaterialLibrary);
                                app.input(Action::MaterialSearch, "Стекло".into());
                            }
                            "gizmo15" => {
                                let r = app.view_widget_rect();
                                app.mouse = [r[0] + r[2] / 2., r[1] + r[3] / 2.];
                            }
                            "collapsed15" => {
                                app.preferences.right_panel = false;
                                app.preferences.left_panel = false;
                                app.preferences.tools_expanded = false;
                            }
                            _ => {}
                        }
                        if std::env::var_os("ARTFORGE_QA_UI_GPU").is_some() {
                            app.prepare_material_previews();
                            let deadline = Instant::now() + Duration::from_secs(45);
                            while app.material_previews.busy() && Instant::now() < deadline {
                                std::thread::sleep(Duration::from_millis(20));
                                app.prepare_material_previews();
                            }
                            assert!(
                                !app.material_previews.busy(),
                                "material preview worker timed out"
                            );
                        }
                    }
                    "workspace14" | "collapsed14" | "materials14" | "camera14" | "animation14"
                    | "nodes14" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/lighting-lab.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.tool = Tool::Move;
                        app.editor.selected = Some(1012);
                        app.editor.project.render_settings.reflection_samples = 1;
                        app.editor.project.render_settings.area_shadow_samples = 4;
                        app.action(Action::SurfaceTab(1));
                        match case {
                            "uv23" | "knife23" | "proportional23" | "fluid-cache23" => {
                                app.editor = Editor::new(Project::default());
                                app.editor.mode = EditorMode::Scene;
                                app.target = Vec3::ZERO;
                                app.width = 1280.;
                                app.height = 800.;
                                app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                                app.frame_all();
                                app.action(Action::Author(ui::AuthorAction::SelectMode(
                                    ui::MeshSelection::Vertex,
                                )));
                                app.primitive_panel = None;
                                match case {
                                    "uv23" => {
                                        let node = app.authoring.editing.unwrap();
                                        app.editor
                                            .edit_mesh(
                                                node,
                                                poseforge_core::MeshEdit::Uv(
                                                    poseforge_core::UvEdit::Unwrap,
                                                ),
                                            )
                                            .unwrap();
                                        app.action(Action::Uv(ui::UvAction::Open));
                                        app.action(Action::Uv(ui::UvAction::Stretch));
                                    }
                                    "knife23" => {
                                        app.action(Action::Author(ui::AuthorAction::Knife))
                                    }
                                    "proportional23" => {
                                        app.authoring.vertices.insert(0);
                                        app.authoring.proportional = true;
                                        app.authoring.proportional_radius = 2.;
                                    }
                                    _ => {
                                        app.authoring.editing = None;
                                        app.set_properties_tab(4);
                                        app.fluid_on = true;
                                        app.editor.mode = EditorMode::Animation;
                                        app.fluid = FluidWorld::new(
                                            Default::default(),
                                            vec![Vec3::new(0., 0.8, 0.)],
                                        )
                                        .unwrap();
                                        app.fluid_preview.cache = fluid_cache::bake(
                                            &app.fluid,
                                            0.5,
                                            24.,
                                            1.,
                                            &AtomicBool::new(false),
                                            |_, _| {},
                                        )
                                        .unwrap();
                                    }
                                }
                            }

                            "collapsed14" => {
                                app.preferences.left_panel = false;
                                app.preferences.tools_expanded = false;
                                app.preferences.right_panel = false;
                            }
                            "materials14" => {
                                for (i, preset) in
                                    poseforge_core::MaterialPreset::ALL.iter().enumerate()
                                {
                                    let mut m = preset.create();
                                    m.id = 2000 + i as u64;
                                    m.name = format!("Образец · {}", m.name);
                                    app.editor.project.materials.push(m);
                                }
                                app.surface.material_query = "Образец".into();
                            }
                            "camera14" => {
                                app.action(Action::CreateCamera);
                                app.action(Action::CameraAlignView);
                                app.action(Action::CameraView);
                                app.action(Action::Studio(ui::StudioAction::CameraLock));
                                app.preferences.left_panel = false;
                                app.preferences.tools_expanded = false;
                            }
                            "animation14" => {
                                app.editor.mode = EditorMode::Animation;
                                app.timeline.expanded = true;
                            }
                            "nodes14" => {
                                app.action(Action::Node(ui::NodeAction::Open));
                            }
                            _ => {}
                        }
                    }
                    "hdri13" | "rays13" | "nodes13" | "node-color13" | "node-menu13" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/lighting-lab.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.tool = Tool::Orbit;
                        app.editor.selected = Some(1012);
                        // Captures keep the authored effects, using the lowest ray budgets for software Vulkan.
                        app.editor.project.render_settings.reflection_samples = 1;
                        app.editor.project.render_settings.area_shadow_samples = 4;
                        if matches!(case, "hdri13" | "rays13") {
                            app.action(Action::SurfaceTab(3));
                            app.inspector_scroll = if case == "hdri13" { 600. } else { 900. };
                        } else {
                            app.action(Action::SurfaceTab(1));
                            app.action(Action::Node(ui::NodeAction::Open));
                            if case == "node-color13" {
                                app.action(Action::Node(ui::NodeAction::Select(1)));
                            }
                            if case == "node-menu13" {
                                app.action(Action::Node(ui::NodeAction::AddMenu));
                            }
                        }
                    }

                    "material-create12"
                    | "material-uv12"
                    | "light-temperature12"
                    | "light-shape12"
                    | "contact12" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/lookdev-studio.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        app.tool = Tool::Orbit;
                        app.editor.selected = Some(1012);
                        if case.starts_with("light-") {
                            app.select_light(0);
                            app.action(Action::SurfaceAimLight);
                            app.editor.project.lights[0].area_size = [1.5, 0.35];
                            app.inspector_scroll =
                                if case == "light-shape12" { 650. } else { 180. };
                        } else if case == "contact12" {
                            app.action(Action::SurfaceTab(3));
                            app.inspector_scroll = 350.;
                        } else {
                            app.action(Action::SurfaceTab(1));
                            if case == "material-uv12" {
                                app.inspector_scroll = app.inspector_max_scroll();
                            } else {
                                app.ui = app.build_ui();
                                app.open_dropdown(ui::DropdownKind::MaterialCreate);
                                for _ in 0..5 {
                                    app.dropdown_key(
                                        &Key::Named(NamedKey::ArrowDown),
                                        false,
                                        false,
                                        false,
                                    );
                                }
                            }
                        }
                    }
                    "mesh-edit11" | "mesh-faces11" | "physics11" | "rig11" | "rig-bones11" => {
                        let file = if matches!(case, "rig11" | "rig-bones11") {
                            "rig-workshop.artforge"
                        } else {
                            "authoring-workshop.artforge"
                        };
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples")
                            .join(file);
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.editor.mode = EditorMode::Scene;
                        if case == "physics11" {
                            app.editor.selected = Some(200);
                            app.action(Action::SurfaceTab(6));
                        } else if matches!(case, "rig11" | "rig-bones11") {
                            app.editor.selected = Some(500);
                            app.authoring.bind_target = Some(500);
                            app.authoring.skeleton =
                                app.editor.project.rig_profiles.first().and_then(|p| p.root);
                            app.active_rig_root = app.authoring.skeleton;
                            app.preferences.skeleton = true;
                            app.action(Action::SurfaceTab(8));
                            if case == "rig-bones11" {
                                app.editor.selected = app.editor.project.rig_profiles[0]
                                    .assignments
                                    .get(&BoneRole::LeftForearm)
                                    .copied();
                            }
                        } else {
                            app.editor.selected = Some(300);
                            app.action(Action::Author(ui::AuthorAction::ToggleEdit));
                            app.action(Action::FrameSelected);
                            if case == "mesh-faces11" {
                                app.authoring.selection_mode = ui::MeshSelection::Face;
                                app.authoring.faces.extend([0, 1]);
                            } else {
                                app.authoring.vertices.insert(0);
                            }
                        }
                    }
                    "properties-material10"
                    | "properties-object10"
                    | "properties-render10"
                    | "properties-fluid10"
                    | "properties-camera10"
                    | "properties-menu10" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/material-fibers.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.action(Action::Select(if case == "properties-camera10" {
                            9000
                        } else {
                            1014
                        }));
                        app.editor.mode = EditorMode::Scene;
                        app.tool = Tool::Orbit;
                        let tab = match case {
                            "properties-object10" => 0,
                            "properties-render10" => 3,
                            "properties-fluid10" => 4,
                            _ => 1,
                        };
                        app.action(Action::SurfaceTab(tab));
                        if case == "properties-menu10" {
                            rebuild_ui(&mut app);
                            app.action(Action::DropdownOpen(ui::DropdownKind::MaterialLibrary));
                            app.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
                        }
                    }
                    "fiber09" | "absorption09" | "presets09" => {
                        let file = if case == "absorption09" {
                            "absorption-glass.artforge"
                        } else {
                            "material-fibers.artforge"
                        };
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples")
                            .join(file);
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.restore_camera();
                        app.action(Action::Select(if case == "absorption09" {
                            1012
                        } else {
                            1014
                        }));
                        app.editor.mode = EditorMode::Scene;
                        app.surface.tab = 1;
                        if case == "presets09" {
                            rebuild_ui(&mut app);
                            app.action(Action::DropdownOpen(ui::DropdownKind::MaterialLibrary));
                            app.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
                        } else if case == "absorption09" {
                            app.inspector_scroll = 820.;
                            rebuild_ui(&mut app);
                            app.action(Action::OpenColorPicker(Box::new(
                                Action::SurfaceAttenuationColor,
                            )));
                        } else {
                            app.inspector_scroll = app.inspector_max_scroll();
                        }
                    }
                    "physics08" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/cloth-collisions.artforge");
                        app.editor = Editor::new(poseforge_io::load_project(path).unwrap());
                        app.action(Action::Select(11));
                        app.editor.mode = EditorMode::Scene;
                        app.inspector_scroll = 700.;
                    }
                    "background08" => {
                        app.surface.tab = 3;
                        app.action(Action::Studio(ui::StudioAction::Background(1)));
                        app.inspector_scroll = 1000.;
                    }
                    "layers08" => {
                        app.action(Action::Studio(ui::StudioAction::LayerAdd));
                        app.action(Action::Studio(ui::StudioAction::LayerAdd));
                    }
                    "scene-mesh081" => {
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        app.editor.mode = EditorMode::Scene;
                        rebuild_ui(&mut app);
                        app.action(Action::DropdownOpen(ui::DropdownKind::MeshCreate));
                    }
                    "properties081" | "object-properties222" => {
                        app.action(Action::Studio(ui::StudioAction::Mesh(0)));
                        app.editor.mode = EditorMode::Scene;
                        if case == "object-properties222" {
                            app.height = 620.;
                            app.set_properties_tab(0);
                            app.inspector_scroll = app.inspector_max_scroll();
                        }
                    }
                    "glass08" => {
                        app.action(Action::Studio(ui::StudioAction::Mesh(2)));
                        app.surface.tab = 1;
                        app.action(Action::SurfacePreset(5));
                        app.inspector_scroll = 650.;
                    }

                    "camera-object" | "camera-view" | "camera-transparent" => {
                        app.action(Action::CreateCamera);
                        if case == "camera-transparent" {
                            app.action(Action::Transparent);
                        }
                        if case == "camera-view" {
                            app.action(Action::CameraView);
                            app.action(Action::Studio(ui::StudioAction::CameraLock));
                        }
                    }
                    "dropdown-render" | "dropdown-material" | "dropdown-light" | "area-light" => {
                        use crate::ui::DropdownKind;
                        let kind = match case {
                            "dropdown-render" => {
                                app.surface.tab = 3;
                                DropdownKind::RenderQuality
                            }
                            "dropdown-material" => {
                                app.surface.tab = 1;
                                DropdownKind::MaterialLibrary
                            }
                            _ => {
                                app.action(Action::SurfaceAddLight(3));
                                DropdownKind::LightKind
                            }
                        };
                        if case == "area-light" {
                            app.inspector_scroll = 200.;
                        } else {
                            rebuild_ui(&mut app);
                            app.action(Action::DropdownOpen(kind));
                            assert!(app.dropdown.is_some());
                        }
                    }
                    "slider-active" | "slider-fine" => {
                        let (rect, _, _) = begin_visible_slider(&mut app, Action::EditTransform(0));
                        move_on_track(&mut app, rect, 0.65);
                        if case == "slider-fine" {
                            app.modifiers = ModifiersState::SHIFT;
                            app.mouse_move(app.mouse);
                            app.mouse_move([app.mouse[0] + 3., app.mouse[1]]);
                        }
                    }
                    "materials-new" => {
                        app.surface.tab = 1;
                        for preset in poseforge_core::MaterialPreset::ALL {
                            let mut material = preset.create();
                            material.id = app.editor.project.next_id();
                            app.editor.project.materials.push(material);
                        }
                        app.action(Action::SurfaceMaterialList);
                    }
                    "materials-layered" => {
                        app.surface.tab = 1;
                        let id = app.selected_material_id().unwrap();
                        let material = app
                            .editor
                            .project
                            .materials
                            .iter_mut()
                            .find(|material| material.id == id)
                            .unwrap();
                        material.name = "Тёплая керамика · лак".into();
                        material.base_color = [0.42, 0.16, 0.055, 1.];
                        material.ior = 1.7;
                        material.coat_weight = 0.8;
                        material.coat_roughness = 0.16;
                        material.roughness = 0.32;
                        app.inspector_scroll = 250.;
                    }
                    "light-shadow" => {
                        app.editor.project.lights[0].kind = poseforge_core::LightKind::Point;
                        app.editor.project.lights[0].name = "Точечный · ключевой".into();
                        app.editor.project.lights[0].casts_shadow = true;
                        app.editor.project.lights[0].intensity = 120.;
                        app.select_light(0);
                    }
                    "settings-shadow-filter" => {
                        app.surface.tab = 3;
                        app.editor.project.render_settings.shadows = true;
                        app.editor.project.render_settings.shadow_filter_radius = 2.5;
                        app.inspector_scroll = 224.;
                    }
                    "color-material" | "color-alpha" | "color-emission" | "color-light"
                    | "color-environment" | "color-invalid" => {
                        use crate::color_picker::ColorControl;
                        let source = if case == "color-light" {
                            app.select_light(0);
                            Action::SurfaceLightColor
                        } else if case == "color-environment" {
                            app.surface.tab = 3;
                            app.inspector_scroll = 460.;
                            app.editor.project.render_settings.sky_color = [1.5, 0.5, 0.25];
                            Action::RenderColor(0)
                        } else {
                            app.surface.tab = 1;
                            if case == "color-emission" {
                                let id = app.selected_material_id().unwrap();
                                app.editor
                                    .project
                                    .materials
                                    .iter_mut()
                                    .find(|m| m.id == id)
                                    .unwrap()
                                    .emissive = [4., 1., 0.2];
                                app.inspector_scroll = 180.;
                            }
                            Action::SurfaceMaterialColor(case == "color-emission")
                        };
                        click_action(&mut app, Action::OpenColorPicker(Box::new(source)));
                        assert!(app.color_picker.is_some());
                        if case != "color-environment" {
                            app.color_confirm(
                                &Action::ColorPicker(ColorControl::Hex),
                                if case == "color-alpha" {
                                    "#D8783A80"
                                } else if case == "color-light" {
                                    "#FFF1C2"
                                } else {
                                    "#409FD8"
                                },
                            )
                            .unwrap();
                        }
                        if case == "color-invalid" {
                            app.action(Action::ColorPicker(ColorControl::Hex));
                            replace_draft(&mut app, "#12oops");
                            app.action(Action::ConfirmInput);
                            assert!(app.surface.input_error.is_some());
                        }
                    }
                    "report-import" | "report-error" => {
                        app.last_import_report = format!(
                            "Импорт: 4 меша, 63 сустава, 6 материалов, 8 текстур, 2 клипа.\n{}",
                            (1..=26).map(|i| format!("Предупреждение {i}: ресурс C:\\Проекты\\Персонаж\\Текстуры\\карта_{i}.png проверен; обратите внимание на UV0.")).collect::<Vec<_>>().join("\n")
                        );
                        app.action(Action::ImportReport);
                        app.action(Action::DetailsPage(1));
                        if case == "report-error" {
                            app.clipboard = None;
                            app.action(Action::CopyReport);
                        }
                    }
                    "report-rig" => {
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../examples/grace-rig.txt");
                        app.load_rig_manifest(&path);
                        app.rig_scroll = 40;
                    }
                    "project-menu" => app.menu = Some(crate::chrome::Menu::Project),
                    "view-menu" | "view-menu-off" => {
                        if case == "view-menu-off" {
                            app.preferences.grid = false;
                            app.preferences.skeleton = false;
                            app.orthographic = true;
                        }
                        app.menu = Some(crate::chrome::Menu::View);
                    }
                    "import-models" | "import-models-animated" => {
                        let animated = case == "import-models-animated";
                        app.preferences.blend_import_animations = animated;
                        if animated {
                            app.import_focus = Some(Action::ToggleBlendImportAnimations);
                        }
                        app.dialog = Some(FileDialog {
                            purpose: FilePurpose::Import,
                            path: if animated {
                                "/__poseforge_qa__/Модели/Персонаж.blend"
                            } else {
                                "/__poseforge_qa__/Модели"
                            }
                            .into(),
                            entries: vec![
                                (PathBuf::from("/__poseforge_qa__/Модели/Текстуры"), true),
                                (
                                    PathBuf::from("/__poseforge_qa__/Модели/Персонаж.blend"),
                                    false,
                                ),
                                (
                                    PathBuf::from("/__poseforge_qa__/Модели/Окружение.glb"),
                                    false,
                                ),
                            ],
                            offset: 0,
                        });
                    }
                    "import-blender-progress" => {
                        let (_sender, receiver) = mpsc::channel();
                        app.job = Some(Job {
                            receiver,
                            cancel: Arc::new(AtomicBool::new(false)),
                            label: "Импорт".into(),
                        });
                        app.status =
                            import_progress_label(-1., "Blender: экспорт модели · прошло 37 с");
                    }
                    "material" => app.surface.tab = 1,
                    "lights" => {
                        app.select_light(0);
                        app.inspector_scroll = 120.;
                    }
                    "light-move" | "light-rotate" | "light-render" => {
                        if case == "light-render" {
                            app.editor.mode = EditorMode::Render;
                        }
                        app.action(Action::SurfaceAddLight(if case == "light-rotate" {
                            2
                        } else {
                            1
                        }));
                        if case == "light-rotate" {
                            app.action(Action::Rotate);
                        }
                    }
                    "render-quality" => app.surface.tab = 3,
                    "render-environment" => {
                        app.surface.tab = 3;
                        app.inspector_scroll = 460.;
                    }
                    "render-output" => {
                        app.surface.tab = 3;
                        app.inspector_scroll = app.inspector_max_scroll();
                    }
                    "fluid" | "fluid-parameters" | "fluid-status" => {
                        app.create_primitive(0);
                        app.fluid_object_action(ui::FluidObjectAction::Attach);
                        app.wait_for_background_jobs();
                        app.primitive_panel = None;
                        app.inspector_scroll = match case {
                            "fluid-parameters" => 200.,
                            "fluid-status" => app.inspector_max_scroll(),
                            _ => 0.,
                        };
                    }
                    "animation" => app.action(Action::AnimationPanel),
                    "animation-dopesheet"
                    | "animation-selection"
                    | "animation-ghosts"
                    | "animation-range"
                    | "animation-compact"
                    | "animation-invalid" => {
                        use crate::animation_timeline::TimelineControl as Control;
                        app.editor.mode = EditorMode::Animation;
                        app.editor.selected = Some(12);
                        app.active_rig_root = Some(1);
                        app.sync_animation_context();
                        app.editor.set_time(1.2).unwrap();
                        app.animation.focused = true;
                        if case == "animation-selection" {
                            app.timeline.selection = app.editor.project.clips[0]
                                .channels
                                .iter()
                                .filter(|channel| channel.node == 10 || channel.node == 11)
                                .flat_map(|channel| {
                                    channel
                                        .keys
                                        .iter()
                                        .filter(|key| key.time >= 0.7 && key.time <= 1.7)
                                        .map(move |key| poseforge_core::KeyRef {
                                            node: channel.node,
                                            property: channel.property,
                                            time: key.time,
                                        })
                                })
                                .collect();
                            app.action(Action::Animation(Control::Copy));
                        }
                        if case == "animation-ghosts" {
                            app.animation_overlay.ghosts = true;
                            app.animation_overlay.path = true;
                            app.animation_overlay.ghost_step_frames = 8;
                        }
                        if case == "animation-range" {
                            app.animation.preview_enabled = true;
                            app.animation.preview_start = 12;
                            app.animation.preview_end = Some(60);
                            app.animation.speed = 0.5;
                        }
                        if case == "animation-compact" {
                            app.timeline.expanded = false;
                        }
                        if case == "animation-invalid" {
                            click_action(&mut app, Action::Animation(Control::Speed));
                            replace_draft(&mut app, "не число");
                            app.action(Action::ConfirmInput);
                            assert!(app.surface.input_error.is_some());
                        }
                    }
                    "import-path" => {
                        app.dialog = Some(FileDialog {
                            purpose: FilePurpose::Import,
                            path: "/__poseforge_qa__/Персонажи/герой.blend".into(),
                            entries: Vec::new(),
                            offset: 0,
                        });
                        click_action(&mut app, Action::EditPath);
                        replace_draft(&mut app, "\"C:\\Проекты\\Персонажи и окружение\\Герой — финальная версия\\Модель с анимацией и материалами.blend\"");
                    }
                    "invalid-number" => {
                        click_action(&mut app, Action::EditTransform(0));
                        replace_draft(&mut app, "не число");
                        app.action(Action::ConfirmInput);
                        assert!(app.surface.input_error.is_some());
                    }
                    _ => {}
                }
                if case.ends_with("17") && std::env::var_os("ARTFORGE_QA_UI_GPU").is_some() {
                    app.prepare_material_previews();
                    let deadline = Instant::now() + Duration::from_secs(180);
                    while app.material_previews.busy() && Instant::now() < deadline {
                        std::thread::sleep(Duration::from_millis(20));
                        app.prepare_material_previews();
                    }
                    assert!(
                        !app.material_previews.busy(),
                        "material preview worker timed out"
                    );
                }
                let rendered = app.build_ui();
                if std::env::var_os("ARTFORGE_QA_UI_GPU").is_some() {
                    let width = (app.width * scale) as u32;
                    let height = (app.height * scale) as u32;
                    let renderer = qa_renderer.get_or_insert_with(|| {
                        pollster::block_on(Renderer::headless(width, height)).unwrap()
                    });
                    renderer.invalidate_assets();
                    renderer.set_editor_appearance(
                        app.preferences.palette().viewport_linear(),
                        app.preferences.grid,
                    );
                    renderer.set_editor_scale(scale);
                    let selection: Vec<_> = if app.editor.mode == EditorMode::Scene
                        && app.authoring.editing.is_none()
                    {
                        app.object_selection().into_iter().collect()
                    } else {
                        Vec::new()
                    };
                    renderer.set_editor_selection(&selection, app.preferences.palette().accent);
                    let eval = app.editor.evaluated().unwrap();
                    renderer
                        .capture_viewport_png(
                            &app.editor.project,
                            &eval.global,
                            &app.view(),
                            &rendered.rects,
                            &rendered.texts,
                            &[],
                            directory.join(format!("{case}-{theme_name}-{}.png", scale as u32)),
                            width,
                            height,
                        )
                        .unwrap();
                }
                let output = serde_json::json!({
                    "width": app.width * scale,
                    "height": app.height * scale,
                    "background": theme.palette().viewport,
                    "rectangles": rendered.rects.iter().map(|rect| serde_json::json!({
                        "rect": rect.rect, "color": rect.color, "segment": rect.segment,
                    })).collect::<Vec<_>>(),
                    "texts": rendered.texts.iter().map(|text| serde_json::json!({
                        "x": text.x, "y": text.y, "text": text.text,
                        "size": text.size, "color": text.color,
                    })).collect::<Vec<_>>(),
                    "source": "Actual App::build_ui primitives; optional GPU PNG uses the editor render path, not native OS capture",
                });
                let path = directory.join(format!("{case}-{theme_name}-{}.json", scale as u32));
                std::fs::write(path, serde_json::to_vec(&output).unwrap()).unwrap();
            }
        }
    }
}

#[test]
fn floating_surfaces_have_no_opaque_backing_and_timeline_owns_its_input() {
    let alpha_at = |u: &Ui, point: [f32; 2]| {
        u.rects
            .iter()
            .filter(|r| ui::contains(r.rect, point))
            .fold(0_f32, |alpha, r| r.color[3] + alpha * (1. - r.color[3]))
    };
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut app = control_app();
            app.preferences.theme = theme;
            app.scale = scale;
            click_action(&mut app, Action::DropdownOpen(ui::DropdownKind::MeshCreate));
            click_action(&mut app, Action::DropdownChoose(0));
            let r = app.primitive_panel_rect().unwrap();
            let mut panel = Ui::with_theme(scale, theme, [0., 0.]);
            app.draw_primitive_panel(&mut panel);
            let gap = [(r[0] + 8.) * scale, (r[1] + 42.) * scale];
            assert!((alpha_at(&panel, gap) - 0.90).abs() < 0.001);
            let fill = panel
                .rects
                .iter()
                .find(|r| ui::contains(r.rect, gap))
                .unwrap();
            assert_ne!(fill.color[..3], theme.palette().panel[..3]);
            assert_eq!(alpha_at(&panel, [r[0] * scale, r[1] * scale]), 0.);
            app.editor.mode = EditorMode::Animation;
            for expanded in [false, true] {
                app.timeline.expanded = expanded;
                app.ui = app.build_ui();
                let dock = app.animation_dock_rect();
                let view = app.viewport();
                assert!(view[1] + view[3] > dock[1] + dock[3] - 8.);
                assert!(!app.canvas_input_at([dock[0] + 2., dock[1] + 2.]));
                assert!(app.canvas_input_at([3., dock[1] + 24.]));
            }
        }
    }
}

#[test]
fn slider_repeated_pointer_wrap_keeps_direction_fine_mode_and_single_undo() {
    let mut a=App::new(None);a.preferences.right_panel=true;a.surface.tab=0;
    a.editor=Editor::new(Project::default());let mut node=Node::new(1,"Object");node.empty=true;
    a.editor.project.nodes.push(node);a.editor.selected=Some(1);a.editor.mode=EditorMode::Scene;
    a.ui=a.build_ui();
    let (rect,action,range,value)=a.ui.hits.iter().find_map(|hit|match &hit.action {
        Action::Slide(action,range,value) if **action==Action::EditTransform(0)=>Some((hit.rect,(**action).clone(),*range,*value)),_=>None
    }).expect("translation slider");
    a.mouse=[rect[0]+rect[2]*0.5,rect[1]+rect[3]*0.5];
    let before=a.editor.project.clone();a.begin_slider(action,range,value);
    a.mouse_move([a.mouse[0]+10.,a.mouse[1]]);let first=a.editor.project.node(1).unwrap().transform.translation.x;
    for x in [4.,a.width-4.,4.] {a.rebase_slider_pointer(x);a.mouse_move([x+10.,a.mouse[1]]);}
    let after=a.editor.project.node(1).unwrap().transform.translation.x;assert!(after>first);
    a.modifiers=ModifiersState::SHIFT;a.rebase_slider_pointer(4.);a.mouse_move([14.,a.mouse[1]]);
    assert!(a.editor.project.node(1).unwrap().transform.translation.x>=after);
    a.mouse_up(MouseButton::Left);assert!(a.drag.is_none());assert_eq!(a.editor.undo_len(),1);
    a.editor.undo();assert_eq!(a.editor.project,before);
}
