//! Curve constraints through the same inspector and input dispatch as the UI.
use super::*;
use crate::ui::{DropdownKind, PathAction as P};
use poseforge_core::{CameraLens, CurvePreset, Node};

fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    let mut curve = Node::new(1, "Path"); curve.curve = Some(CurvePreset::Line.curve());
    let mut mesh = Node::new(2, "Mesh"); mesh.mesh = Some(10);
    let mut camera = Node::new(3, "Camera"); camera.camera = Some(CameraLens::default());
    let mut p = Project::default();
    p.nodes = vec![curve,mesh,camera];
    p.meshes.push(poseforge_core::box_mesh(10, "Cube", Vec3::ONE, None));
    p.lights.truncate(1); p.lights[0].kind = poseforge_core::LightKind::Spot;
    a.editor = Editor::new(p); a.editor.mode = EditorMode::Scene;
    a.editor.selected = Some(2); a.reset_workflow_selection();
    a.surface.tab = 0; a.width = 1280.; a.height = 900.;
    a
}
fn bottom(a: &mut App) {
    a.inspector_scroll = a.inspector_max_scroll();
    a.ui = a.build_ui();
}
fn click(a: &mut App, action: Action) {
    a.ui = a.build_ui();
    let point = a.ui.action_point(&action).unwrap_or_else(|| panic!("Missing visible control {action:?}"));
    a.mouse_move(point); a.click_ui_control();
}

#[test]
fn three_object_types_bind_from_scrolled_dropdown_and_detach_with_undo() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1.,2.] {
            for target in [2,3,0] {
                let mut a = app(); a.preferences.theme = theme;
                a.scale = scale; a.width *= scale; a.height *= scale;
                if target == 0 { a.select_light(0); } else { a.editor.selected = Some(target); }
                bottom(&mut a); click(&mut a, Action::DropdownOpen(DropdownKind::CurvePath));
                assert!(a.dropdown_is_open(DropdownKind::CurvePath));
                a.choose_dropdown(0);
                assert_eq!(a.path_binding().unwrap().curve, 1);
                a.apply_field_value(Action::Path(P::Field(0)), "37,5").unwrap();
                assert_eq!(a.path_binding().unwrap().position, 0.375);
                let before = a.editor.evaluated().unwrap();
                let undo = a.editor.undo_len();
                bottom(&mut a); click(&mut a, Action::Path(P::Clear));
                assert!(a.path_binding().is_none());
                let after = a.editor.evaluated().unwrap();
                let (first,second) = if target == 0 { (before.lights[0].position, after.lights[0].position) }
                    else { (before.global[&target].transform_point3(Vec3::ZERO), after.global[&target].transform_point3(Vec3::ZERO)) };
                assert!(first.distance(second) < 0.0001);
                assert_eq!(a.editor.undo_len(), undo + 1);
                a.action(Action::Undo);
                // History deliberately clears index-based light selection.
                if target == 0 { a.select_light(0); }
                assert_eq!(a.path_binding().unwrap().position, 0.375);
            }
        }
    }
}

#[test]
fn slider_capture_cancel_and_reset_use_single_undo_transactions() {
    let mut a = app(); a.action(Action::Path(P::Bind(1)));
    let original = a.editor.project.clone(); let undo = a.editor.undo_len();
    bottom(&mut a);
    let (rect,range,value) = a.ui.hits.iter().find_map(|h| match &h.action {
        Action::Slide(action,range,value) if **action == Action::Path(P::Field(0)) => Some((h.rect,*range,*value)),
        _ => None,
    }).unwrap();
    a.mouse_move([rect[0]+rect[2]*0.35,rect[1]+rect[3]*0.5]);
    a.begin_slider(Action::Path(P::Field(0)), range, value);
    assert!(matches!(a.drag, Some(Drag::Slider(_))));
    a.mouse_move([rect[0]+rect[2]*0.6,rect[1]+rect[3]*0.5]);
    assert!(a.path_binding().unwrap().position > 0.);
    a.action(Action::Cancel); assert_eq!(a.editor.project, original); assert_eq!(a.editor.undo_len(), undo);
    a.apply_field_value(Action::Path(P::Field(0)), "60").unwrap();
    bottom(&mut a); click(&mut a, Action::ResetNumber(Box::new(Action::Path(P::Field(0)))));
    assert_eq!(a.path_binding().unwrap().position, 0.);
    assert_eq!(a.editor.undo_len(), undo + 2);
    a.action(Action::Undo); assert_eq!(a.path_binding().unwrap().position, 0.6);
}

#[test]
fn stale_dropdown_locks_and_missing_curves_do_not_edit_a_different_target() {
    let mut a = app(); bottom(&mut a);
    click(&mut a, Action::DropdownOpen(DropdownKind::CurvePath));
    a.editor.selected = Some(3); a.choose_dropdown(0);
    assert!(a.editor.project.nodes.iter().all(|n| n.curve_binding.is_none()));
    a.editor.project.node_mut(3).unwrap().locked = true;
    a.action(Action::Path(P::Bind(1))); assert!(a.path_binding().is_none());
    a.editor.project.node_mut(3).unwrap().locked = false;
    a.editor.delete_node(1).unwrap(); bottom(&mut a);
    assert!(a.ui.action_point(&Action::DropdownOpen(DropdownKind::CurvePath)).is_none());
    assert!(!a.apply_field_value(Action::Path(P::Field(0)), "50").is_ok());
}

#[test]
fn bound_light_world_fields_and_render_snapshot_use_current_timeline_sample() {
    let mut a = app(); a.select_light(0); a.action(Action::Path(P::Bind(1)));
    a.apply_field_value(Action::Path(P::Field(1)), "25").unwrap(); a.editor.time = 2.;
    let eval = a.editor.evaluated().unwrap();
    assert!(eval.lights[0].position.distance(Vec3::ZERO) < 0.0001);
    let frame = frame_preparation::render_project(&a.editor.project, &eval, false);
    assert!(frame.lights[0].curve_binding.is_none());
    assert_eq!(frame.lights[0].position, eval.lights[0].position);
    assert!(a.editor.project.lights[0].curve_binding.is_some());
    drop(frame);
    a.apply_field_value(Action::SurfaceLightNumber(1), "3").unwrap();
    a.apply_field_value(Action::SurfaceLightNumber(2), "2").unwrap();
    let position = a.evaluated_light(0).unwrap().position;
    assert!(position.distance(Vec3::new(3.,2.,0.)) < 0.0001);
    a.action(Action::Path(P::Clear));
    assert!(a.editor.project.lights[0].position.distance(position) < 0.0001);
}

#[test]
fn source_selection_and_invalid_numeric_values_keep_binding_intact() {
    let mut a = app(); a.action(Action::Path(P::Bind(1)));
    let before = a.editor.project.clone(); let undo = a.editor.undo_len();
    for value in ["nan", "inf", "-1", "101", "abc"] {
        assert!(a.apply_field_value(Action::Path(P::Field(0)), value).is_err());
    }
    assert_eq!(a.editor.project, before); assert_eq!(a.editor.undo_len(), undo);
    a.action(Action::Path(P::SelectCurve)); assert_eq!(a.editor.selected, Some(1));
    assert!(a.path_target().is_none()); assert!(a.path_panel_start().is_none());
}
