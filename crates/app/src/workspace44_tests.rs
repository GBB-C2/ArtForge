//! Library choices add layers; explicit drops retain their polygon scope.
use super::*;
use poseforge_core::{Material,Node,PrimitiveKind,PrimitiveSettings};
fn app() -> App {
    let mut mesh=PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap();
    let other=mesh.clone();let offset=mesh.vertices.len() as u32;
    for v in &mut mesh.vertices {v.position[0]-=2.;}
    mesh.vertices.extend(other.vertices.iter().map(|v|{let mut v=*v;v.position[0]+=2.;v}));
    mesh.indices.extend(other.indices.iter().map(|i|*i+offset));
    mesh.topology.as_mut().unwrap().faces.extend(other.topology.unwrap().faces.iter().map(|f|f.iter().map(|i|*i+offset).collect()));
    mesh.topology.as_mut().unwrap().primitive=None;
    let mut node=Node::new(1,"Two islands");node.mesh=Some(2);
    let mut a=App::new(None);a.preferences=Preferences::default();a.preferences.animation_visible=false;a.preferences.right_panel=true;
    a.width=1440.;a.height=900.;a.editor=Editor::new(Project{nodes:vec![node],meshes:vec![mesh],materials:vec![Material{id:3,..Default::default()},Material{id:4,name:"Paint".into(),in_library:true,..Default::default()}],..Default::default()});
    a.editor.mode=EditorMode::Scene;a.editor.selected=Some(1);a.surface.tab=1;a.frame_all();a.ui=a.build_ui();a
}
#[test]
fn library_pointer_click_adds_distinct_layers_and_escape_cancels_before_insertion() {
    let mut a=app();let original=a.editor.project.clone();let mut inserted=BTreeSet::new();
    for count in 1..=4 {
        a.ui=a.build_ui();a.open_dropdown(ui::DropdownKind::MaterialLibrary);
        a.input.as_mut().unwrap().insert("Paint");a.ui=a.build_ui();
        let row=a.ui.hits.iter().find(|h|h.action==Action::DropdownChoose(0)).unwrap().rect;
        a.mouse_move([row[0]+row[2]*0.5,row[1]+row[3]*0.5]);
        a.mouse_down(MouseButton::Left);
        assert_eq!(a.editor.undo_len(),count-1);
        a.mouse_up(MouseButton::Left);
        let id=a.selected_material_id().unwrap();assert_ne!(id,4);assert!(inserted.insert(id));
        assert_eq!(a.editor.project.node(1).unwrap().material_layers.len(),count);
        assert!(a.visible_material_ids().contains(&id));
        assert_eq!(a.editor.project.meshes,original.meshes);
        assert_eq!(a.editor.project.material(4),original.material(4));
        assert!(!a.editor.project.material(id).unwrap().in_library);
        assert_eq!(a.editor.undo_len(),count);
    }
    for _ in 0..4 {a.action(Action::Undo);}
    assert_eq!(a.editor.project,original);
    a.ui=a.build_ui();a.open_dropdown(ui::DropdownKind::MaterialLibrary);
    a.input.as_mut().unwrap().insert("Paint");a.ui=a.build_ui();
    let row=a.ui.hits.iter().find(|h|h.action==Action::DropdownChoose(0)).unwrap().rect;
    a.mouse_move([row[0]+row[2]*0.5,row[1]+row[3]*0.5]);a.mouse_down(MouseButton::Left);
    a.action(Action::Cancel);assert!(a.drag.is_none());assert_eq!(a.editor.project,original);assert_eq!(a.editor.undo_len(),0);
}
#[test]
fn library_slot_keeps_face_selection_until_explicit_assignment() {
    let mut a=app();let original=a.editor.project.clone();
    a.action(Action::Author(ui::AuthorAction::SelectMode(ui::MeshSelection::Face)));
    a.authoring.faces=BTreeSet::from([0,2]);
    a.add_from_material_library(Action::AssetMaterial(4));
    let id=a.selected_material_id().unwrap();
    assert_eq!(a.editor.project.meshes,original.meshes);
    assert_eq!(a.authoring.faces,BTreeSet::from([0,2]));
    assert!(a.material_list_ids().contains(&id));
    assert!(!a.editor.project.node(1).unwrap().material_layers[0].visible);
    assert_eq!(a.editor.undo_len(),1);
    a.action(Action::MaterialApplyFaces);
    assert_eq!(a.material_user_faces(1,id),BTreeSet::from([0,2]));
    assert!(a.editor.project.node(1).unwrap().material_layers.is_empty());
    a.action(Action::Undo);a.action(Action::Undo);assert_eq!(a.editor.project,original);
}

#[test]
fn surface_hit_uses_selected_faces_or_one_island_empty_view_uses_the_whole_mesh() {
    let mut a=app();a.mouse=a.project_point(Vec3::new(-2.,0.,1.)).unwrap();
    let (node,faces)=a.material_drop_scope(Some(1)).unwrap();assert_eq!(node,1);assert_eq!(faces.as_ref().unwrap().len(),6);
    assert!(faces.as_ref().unwrap().iter().all(|f|*f<6));
    let eval=a.editor.evaluated().unwrap();let hit=crate::picking::pick_material_surface(&a.picking_project(),&eval,a.view().view_projection(),a.viewport(),a.mouse).unwrap();
    a.authoring.editing=Some(1);a.authoring.selection_mode=ui::MeshSelection::Face;a.authoring.faces=BTreeSet::from([hit.face]);
    assert_eq!(a.material_drop_scope(Some(1)).unwrap().1,Some(a.authoring.faces.clone()));
    let r=a.viewport();let empty=[[r[0]+30.,r[1]+r[3]-40.],[r[0]+r[2]*0.5,r[1]+r[3]-40.]].into_iter().find(|p|{
        a.canvas_input_at(*p) && crate::picking::pick_material_surface(&a.picking_project(),&eval,a.view().view_projection(),r,*p).is_none()
    }).unwrap();
    a.mouse=empty;assert_eq!(a.material_drop_scope(Some(1)),Some((1,None)));
    a.mouse=[a.width-70.,350.];assert!(a.begin_material_drag(&Action::AssetMaterial(4)));
    a.mouse=empty;a.mouse_up(MouseButton::Left);
    assert!(a.material_list_ids().contains(&3) && a.material_list_ids().contains(&4));
    assert_eq!(a.selected_material_id(),Some(4));assert!(a.visible_material_ids().contains(&4));
    a.mouse=[-10.,-10.];assert!(a.material_drop_scope(Some(1)).is_none());
}
#[test]
fn preset_drag_commits_one_step_and_keeps_unpainted_islands() {
    let mut a=app();let original=a.editor.project.clone();let target=a.project_point(Vec3::new(-2.,0.,1.)).unwrap();
    a.mouse=[a.width-70.,350.];assert!(a.begin_material_drag(&Action::AssetPreset(2)));a.mouse=target;a.mouse_up(MouseButton::Left);
    let mesh=a.editor.project.mesh(a.editor.project.node(1).unwrap().mesh.unwrap()).unwrap();
    assert!(mesh.face_material(0)!=Some(3));assert!((6..12).all(|f|mesh.face_material(f)==Some(3)));
    assert_eq!(a.editor.undo_len(),1);assert!(a.editor.undo());assert_eq!(a.editor.project,original);
}
#[test]
fn four_new_tools_expose_text_parameters_for_all_component_modes() {
    for mode in [ui::MeshSelection::Vertex,ui::MeshSelection::Edge,ui::MeshSelection::Face] {
        let mut a=app();a.action(Action::Author(ui::AuthorAction::SelectMode(mode)));
        for tool in [ui::ModelingTool::Sphere,ui::ModelingTool::Circle,ui::ModelingTool::Inflate,ui::ModelingTool::Twist] {
            a.choose_modeling_tool(tool);let mut u=Ui::new(1.);a.draw_modeling_palette(&mut u,0.,290.);
            assert!(u.hits.iter().any(|h|matches!(&h.action,Action::Slide(field,_,_) if matches!(**field,Action::Normal(ui::NormalAction::Field(5..=7))))));
        }
    }
}
