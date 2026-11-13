//! Regression coverage for source curves, resource browsing and camera effects.
use super::*;
use crate::ui::{CurveAction as C, CameraTool};
use poseforge_core::{Material, Node, CurvePreset};
fn app() -> App {
    let mut a = App::new(None); a.preferences = Preferences::default();
    a.preferences.right_panel = true; a.preferences.animation_visible = false;
    a.width = 1200.; a.height = 820.; a.editor = Editor::new(Project::default());
    a.editor.mode = EditorMode::Scene; a.saved_snapshot = a.editor.project.clone(); a
}
#[test]
fn curve_creation_and_geometry_keep_source_points_and_hide_uv_until_conversion() {
    let mut a = app(); a.action(Action::Curve(C::Create(0)));
    let id = a.editor.selected.unwrap();
    assert!(a.editor.project.node(id).unwrap().curve.is_some()); assert!(a.editor.project.meshes.is_empty());
    assert!(!a.selected_mesh_for_uv());
    a.action(Action::Curve(C::AddGeometry)); let with_geometry = a.editor.project.clone();
    assert!(a.editor.project.node(id).unwrap().mesh.is_some()); assert!(!a.selected_mesh_for_uv());
    a.apply_field_value(Action::Curve(C::Field(4)), "0,125").unwrap();
    assert_eq!(a.selected_curve().unwrap().1.geometry.unwrap().radius, 0.125);
    assert!(a.editor.undo()); assert_eq!(a.editor.project, with_geometry);
    a.action(Action::Curve(C::Convert)); assert!(a.selected_mesh_for_uv());
}
#[test]
fn freehand_release_creates_one_undo_step_and_escape_discards_the_stroke() {
    let mut a = app(); let v = a.viewport(); let start = [v[0]+v[2]*0.5, v[1]+v[3]*0.5];
    a.action(Action::Curve(C::Draw)); a.mouse_move(start); a.mouse_down(MouseButton::Left);
    a.mouse_move([start[0]+30.,start[1]+12.]); a.mouse_move([start[0]+60.,start[1]-12.]);
    a.mouse_up(MouseButton::Left); assert_eq!(a.editor.undo_len(), 1);
    assert_eq!(a.primitive_panel, a.editor.selected);
    assert!(a.primitive_panel_rect().is_some());
    let curve = a.selected_curve().unwrap().1; assert!(curve.points.len() >= 3); assert!(curve.geometry.is_none());
    let before = a.editor.project.clone();
    a.action(Action::Curve(C::Draw)); a.mouse_move(start); a.mouse_down(MouseButton::Left);
    a.mouse_move([start[0]+30.,start[1]+10.]); a.action(Action::Cancel);
    assert_eq!(a.editor.project, before); assert!(!a.curves.drawing); assert!(a.drag.is_none());
}
#[test]
fn point_drag_is_one_transaction_and_cancel_restores_original_points() {
    let mut a = app(); a.action(Action::Curve(C::Create(2))); a.frame_curve();
    let original = a.editor.project.clone(); let first = a.selected_curve().unwrap().1.points[0];
    a.mouse = a.project_point(Vec3::from_array(first)).unwrap();
    assert!(a.curve_pointer_down()); assert!(matches!(a.drag, Some(Drag::Curve(_))));
    a.mouse_move([a.mouse[0]+30., a.mouse[1]+20.]); assert_ne!(a.editor.project, original);
    a.action(Action::Cancel); assert_eq!(a.editor.project, original);
    assert_eq!(a.editor.undo_len(), 1);
    assert!(a.drag.is_none());
    assert!(a.primitive_panel_rect().is_some());
    a.action(Action::Cancel);
    assert!(a.primitive_panel.is_none());
}
#[test]
fn material_expansion_selects_polygons_and_cross_detaches_only_that_instance() {
    let mut a = app(); let mut one = Node::new(1, "One"); one.mesh = Some(2);
    let mut two = Node::new(4, "Two"); two.mesh = Some(2);
    a.editor = Editor::new(Project { nodes: vec![one,two], meshes: vec![poseforge_core::box_mesh(2,"Box",Vec3::ONE,Some(3))],
        materials: vec![Material {id:3,in_library:true,..Default::default()}], ..Default::default() });
    a.editor.mode = EditorMode::Scene; a.action(Action::Select(1)); a.set_properties_tab(1);
    a.action(Action::MaterialUsers(3)); assert_eq!(a.material_users(3), vec![1,4]);
    let mut u = Ui::new(1.); a.draw_material_panel(&mut u, 0., 300.);
    assert!(u.hits.iter().any(|h| h.action == Action::MaterialUserSelect(4,3)));
    a.action(Action::MaterialUserSelect(4,3)); assert_eq!(a.editor.selected,Some(4));
    assert_eq!(a.authoring.selection_mode, ui::MeshSelection::Face);
    assert_eq!(a.authoring.faces.len(), a.editor.project.mesh(2).unwrap().face_count());
    a.action(Action::MaterialUserDetach(4,3)); assert_eq!(a.material_users(3),vec![1]);
    assert!(a.editor.undo()); assert_eq!(a.material_users(3),vec![1,4]);
}
#[test]
fn resource_library_categories_search_and_scene_import_do_not_replace_the_scene() {
    let mut a = app(); a.action(Action::Curve(C::Create(2))); let original = a.editor.project.clone();
    a.ui = a.build_ui();
    assert!(a.ui.hits.iter().any(|h| h.action == Action::DropdownOpen(ui::DropdownKind::AssetLibrary)));
    a.open_dropdown(ui::DropdownKind::AssetLibrary); let u = a.build_ui();
    for category in 0..7 { assert!(u.hits.iter().any(|h| h.action == Action::AssetCategory(category))); }
    a.close_asset_popup();
    a.action(Action::AssetScene(0)); a.wait_for_background_jobs(); assert!(a.editor.project.nodes.len() > original.nodes.len());
    assert_eq!(a.editor.project.node(original.nodes[0].id).unwrap().curve, Some(CurvePreset::Line.curve()));
    a.editor.project.validate().unwrap(); assert!(a.editor.undo()); assert_eq!(a.editor.project,original);
}
#[test]
fn camera_effect_fields_reach_the_view_and_numeric_reset() {
    let mut a = app(); a.action(Action::CreateCamera); a.action(Action::CameraTool(CameraTool::Effects));
    a.apply_field_value(Action::CameraNumber(8), "0.7").unwrap(); a.action(Action::CameraView);
    assert!(a.view().effects.enabled); assert_eq!(a.view().effects.vignette, 0.7);
    a.action(Action::ResetNumber(Box::new(Action::CameraNumber(8)))); assert_eq!(a.view().effects.vignette, 0.);
}
