//! Transactions and interaction paths introduced with the layer-based inspector.
use super::*;
use poseforge_core::{box_mesh, Material, Node};

fn app() -> App {
    let mut node = Node::new(1, "Mesh"); node.mesh = Some(2);
    let mut a = App::new(None); a.preferences = Preferences::default();
    a.preferences.left_panel = true; a.preferences.right_panel = true;
    a.width = 1280.; a.height = 920.; a.surface.tab = 1;
    a.editor = Editor::new(Project {nodes: vec![node], meshes: vec![box_mesh(2, "Cube", Vec3::ONE, Some(3))],
        materials: vec![Material {id: 3, in_library: true, ..Default::default()}], ..Default::default()});
    a.editor.selected = Some(1); a.editor.mode = EditorMode::Scene; a.saved_snapshot = a.editor.project.clone(); a
}
fn click(a: &mut App, action: Action) {
    a.ui = a.build_ui();
    let r = a.ui.hits.iter().rev().find(|h|h.action == action).unwrap().rect;
    a.mouse_move([r[0]+r[2]*0.45,r[1]+r[3]*0.5]);
    a.mouse_down(MouseButton::Left); a.mouse_up(MouseButton::Left);
}
#[test]
fn default_add_is_a_local_layer_and_double_click_rename_can_cancel() {
    let mut a = app(); let original = a.editor.project.clone();
    click(&mut a, Action::SurfaceCreateMaterial);
    let material = a.selected_material_id().unwrap();
    assert_ne!(material,3); assert!(!a.editor.project.material(material).unwrap().in_library);
    assert_eq!(a.editor.project.mesh(2).unwrap(),original.mesh(2).unwrap());
    assert_eq!(a.editor.undo_len(),1);
    assert!(!a.build_ui().hits.iter().any(|h|h.action == Action::SurfaceMaterialName));
    click(&mut a,Action::MaterialSelect(material));
    assert!(a.input.is_none()); click(&mut a,Action::MaterialSelect(material));
    assert_eq!(a.input.as_ref().unwrap().purpose,Action::SurfaceMaterialName);
    a.input.as_mut().unwrap().select_all(); a.input.as_mut().unwrap().insert("Черновик");
    a.action(Action::Cancel); assert_eq!(a.editor.undo_len(),1);
    click(&mut a,Action::MaterialSelect(material)); click(&mut a,Action::MaterialSelect(material));
    a.input.as_mut().unwrap().select_all(); a.input.as_mut().unwrap().insert("Синий лак");
    a.action(Action::ConfirmInput); assert_eq!(a.editor.project.material(material).unwrap().name,"Синий лак");
    a.action(Action::Undo); a.action(Action::Undo); assert_eq!(a.editor.project,original);
}
#[test]
fn library_is_opt_in_categories_filter_and_removal_keeps_layers() {
    let mut a = app(); a.action(Action::SurfaceCreateMaterial);
    let id = a.selected_material_id().unwrap();
    a.editor.project.materials.iter_mut().find(|m|m.id==id).unwrap().category="Лаки".into();
    a.ui=a.build_ui(); a.open_dropdown(ui::DropdownKind::MaterialLibrary); a.ui=a.build_ui();
    assert!(!a.material_popup_visible_ids().contains(&id)); a.close_material_dropdown();
    a.action(Action::MaterialLibraryToggle);
    a.ui=a.build_ui(); a.open_dropdown(ui::DropdownKind::MaterialLibrary); a.ui=a.build_ui();
    let category = a.material_categories().iter().position(|c| c.as_deref() == Some("Лаки")).unwrap();
    click(&mut a,Action::MaterialLibraryCategory(category)); assert_eq!(a.material_popup_visible_ids(),vec![id]);
    click(&mut a,Action::MaterialLibraryRemove(id));
    assert!(a.editor.project.node(1).unwrap().material_layers.iter().any(|l|l.material==id));
    assert!(!a.editor.project.material(id).unwrap().in_library);
    a.close_material_dropdown(); a.action(Action::Undo); assert!(a.editor.project.material(id).unwrap().in_library);
}
#[test]
fn layer_order_visibility_and_delete_are_independent_undo_steps() {
    let mut a=app(); a.action(Action::SurfaceCreateMaterial); let bottom=a.selected_material_id().unwrap();
    a.action(Action::SurfaceCreateMaterial);let top=a.selected_material_id().unwrap();
    a.action(Action::MaterialLayerMove(-1));
    assert_eq!(a.editor.project.node(1).unwrap().material_layers[0].material,top);
    a.action(Action::MaterialLayerVisible(1,top));
    assert!(!a.editor.project.node(1).unwrap().material_layers[0].visible);
    a.action(Action::SceneMaterialDetach(1,top));
    assert_eq!(a.editor.project.node(1).unwrap().material_layers[0].material,bottom);
    a.action(Action::Undo); assert!(!a.editor.project.node(1).unwrap().material_layers[0].visible);
    a.action(Action::Undo); assert!(a.editor.project.node(1).unwrap().material_layers[0].visible);
    a.action(Action::Undo); assert_eq!(a.editor.project.node(1).unwrap().material_layers[0].material,bottom);
}
#[test]
fn tools_are_right_of_scene_curves_have_their_own_menu_and_uv_requires_selection() {
    let mut a=app(); let u=a.build_ui(); let scene=a.left_panel_rect(); let rail=a.rail_rect();
    assert!(rail[0]>=scene[0]+scene[2]+8.); assert_eq!(rail[1],scene[1]);
    let creation=|kind|u.hits.iter().find(|h|h.action==Action::DropdownOpen(kind)).unwrap().rect[0];
    assert!(creation(ui::DropdownKind::MeshCreate)<creation(ui::DropdownKind::CurveCreate));
    assert!(creation(ui::DropdownKind::CurveCreate)<creation(ui::DropdownKind::SkeletonCreate));
    assert!(creation(ui::DropdownKind::SkeletonCreate)<creation(ui::DropdownKind::LightCreate));
    assert!(u.hits.iter().any(|h|h.action==Action::Uv(ui::UvAction::Open)));
    a.editor.selected=None; let u=a.build_ui();
    assert!(!u.hits.iter().any(|h|matches!(h.action,Action::Uv(_))));
    a.action(Action::Uv(ui::UvAction::Open)); assert!(!a.uv.open);
}
