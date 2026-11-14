//! Material polygons persist independently of selection; the library opens as a popup.
use super::*;
use poseforge_core::{Material,Node,PrimitiveKind,PrimitiveSettings};
fn app() -> App {
    let mut node=Node::new(1,"Cube");node.mesh=Some(2);
    let mut a=App::new(None);a.preferences=Preferences::default();a.preferences.animation_visible=false;
    a.width=1440.;a.height=1100.;a.editor=Editor::new(Project {nodes:vec![node],
        meshes:vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap()],
        materials:vec![Material{id:3,..Default::default()},Material{id:4,in_library:true,name:"Library".into(),..Default::default()}],..Default::default()});
    a.editor.mode=EditorMode::Scene;a.editor.selected=Some(1);a.set_properties_tab(1);a
}
#[test]
fn library_popup_remains_available_and_footer_assigns_only_selected_polygons() {
    let mut a=app();let original=a.editor.project.clone();
    a.action(Action::Author(ui::AuthorAction::SelectMode(ui::MeshSelection::Face)));
    a.authoring.faces=BTreeSet::from([0,2]);
    let mut u=Ui::new(1.);a.draw_material_panel(&mut u,0.,300.);
    let assigned=u.hits.iter().find(|h|h.action==Action::MaterialSelect(3)).unwrap().rect;
    assert!(!u.hits.iter().any(|h|matches!(h.action,Action::AssetMaterial(_))));
    let library_button=u.hits.iter().find(|h|h.action==Action::DropdownOpen(ui::DropdownKind::MaterialLibrary)).unwrap().rect;
    let actions_button=u.hits.iter().find(|h|h.action==Action::DropdownOpen(ui::DropdownKind::MaterialActions)).unwrap().rect;
    assert_eq!(library_button[0],actions_button[0]);
    assert!(library_button[1]>=actions_button[1]+actions_button[3]);
    assert!(!u.hits.iter().any(|h|h.action==Action::SurfaceUnassignMaterial));
    assert!(a.editor.project.material(4).unwrap().in_library);
    a.action(Action::AssetMaterial(4));assert_eq!(a.editor.project,original);assert_eq!(a.authoring.faces,BTreeSet::from([0,2]));
    u=Ui::new(1.);a.draw_material_panel(&mut u,0.,300.);
    let assign=u.hits.iter().find(|h|h.action==Action::MaterialApplyFaces).unwrap().rect;
    assert!(assign[1]>assigned[1]+assigned[3]);
    assert!(assign[1]+assign[3]<material_controls::MATERIAL_LIST_Y+a.material_list_height());
    assert!(!u.texts.iter().any(|t|t.text=="Назначить" || t.text=="Удалить назначение"));
    a.action(Action::MaterialApplyFaces);assert_eq!(a.material_user_faces(1,4),BTreeSet::from([0,2]));assert_eq!(a.editor.undo_len(),1);
    a.authoring.faces=BTreeSet::from([5]);a.action(Action::MaterialUserSelect(1,4));
    assert_eq!(a.authoring.faces,BTreeSet::from([0,2]));assert_eq!(a.editor.undo_len(),1);assert_eq!(a.surface.tab,1);
    a.authoring.faces=BTreeSet::from([0]);a.action(Action::MaterialUnassignFaces);
    assert_eq!(a.material_user_faces(1,4),BTreeSet::from([2]));assert!(a.editor.project.material(4).unwrap().in_library);
    a.action(Action::Undo);assert_eq!(a.material_user_faces(1,4),BTreeSet::from([0,2]));
}
#[test]
fn opening_a_saved_material_restores_its_polygon_group_after_selection_changes() {
    let mut a=app();a.editor.assign_material_region(1,4,Some(&BTreeSet::from([1,3]))).unwrap();
    let saved:Project=serde_json::from_str(&serde_json::to_string(&a.editor.project).unwrap()).unwrap();
    a.editor=Editor::new(saved);a.editor.mode=EditorMode::Scene;a.editor.selected=None;
    a.action(Action::MaterialUserSelect(1,4));
    assert_eq!(a.authoring.selection_mode,ui::MeshSelection::Face);assert_eq!(a.authoring.faces,BTreeSet::from([1,3]));assert_eq!(a.editor.undo_len(),0);
    a.action(Action::MaterialUserDetach(1,4));assert!(a.material_user_faces(1,4).is_empty());assert!(a.editor.project.material(4).is_some());
    assert_eq!(a.editor.undo_len(),1);a.action(Action::Undo);assert_eq!(a.material_user_faces(1,4),BTreeSet::from([1,3]));
}
#[test]
fn library_entries_do_not_expand_mesh_properties_or_request_hidden_previews() {
    let mut a=app();let shift=a.material_controls_shift();
    for id in 5..20 {a.editor.project.materials.push(Material{id,in_library:true,..Default::default()});}
    let mut u=Ui::new(1.);a.draw_material_panel(&mut u,0.,300.);
    assert_eq!(a.material_controls_shift(),shift);
    assert!(!u.hits.iter().any(|h|matches!(h.action,Action::AssetMaterial(_))));
    assert!(!u.hits.iter().any(|h|matches!(h.action,Action::MaterialApplyFaces|Action::MaterialUnassignFaces)));
    assert_eq!(a.visible_material_ids(),vec![3]);
    a.action(Action::AssetMaterial(4));a.action(Action::SurfaceMaterialName);
    u=Ui::new(1.);a.draw_material_panel(&mut u,0.,300.);
    assert!(u.hits.iter().any(|h|h.action==Action::SurfaceMaterialName));
    assert_eq!(a.selected_material_id(),Some(4));
    a.editor.selected=None;u=Ui::new(1.);a.draw_material_panel(&mut u,0.,300.);
    assert!(!u.hits.iter().any(|h|matches!(h.action,Action::AssetMaterial(_))));
    assert!(a.visible_material_ids().is_empty());
}

#[test]
fn enlarged_modeling_controls_fit_the_panel_and_keep_properties_below() {
    for width in [280.,336.,440.] {
        let mut a=app();a.preferences.right_width=Some(width);
        a.action(Action::Author(ui::AuthorAction::SelectMode(ui::MeshSelection::Vertex)));
        let mut u=Ui::new(1.);let w=width-properties::RAIL_WIDTH-32.;a.draw_modeling_palette(&mut u,0.,w);
        let buttons:Vec<_>=u.hits.iter().filter(|h|matches!(h.action,Action::ModelingTool(_))).collect();
        assert_eq!(buttons.len(),29);
        for (i,b) in buttons.iter().enumerate() {
            assert_eq!(b.rect[2],44.);assert_eq!(b.rect[3],44.);
            assert!(b.rect[0]>=0. && b.rect[0]+b.rect[2]<=w+0.01);
            assert!(b.rect[1]+b.rect[3]<a.modeling_properties_start());
            for other in &buttons[i+1..] {assert!(b.rect[0]+44.<=other.rect[0] || other.rect[0]+44.<=b.rect[0] || b.rect[1]+44.<=other.rect[1] || other.rect[1]+44.<=b.rect[1]);}
        }
    }
}

#[test]
fn emissive_volume_is_one_undo_step_and_footer_actions_share_a_row() {
    let mut a=app();let before=a.editor.project.clone();
    a.action(Action::SurfaceEmissionVolume);
    assert!(a.editor.project.material(3).unwrap().emission_volume);
    assert!(a.editor.project.render_settings.atmosphere.density>0.);
    assert_eq!(a.editor.undo_len(),1);
    assert!(a.editor.undo());assert_eq!(a.editor.project,before);
    a.action(Action::Author(ui::AuthorAction::SelectMode(ui::MeshSelection::Face)));
    a.authoring.faces=BTreeSet::from([0,2]);
    a.action(Action::MaterialApplyFaces);
    let mut u=Ui::new(1.);a.draw_material_panel(&mut u,0.,300.);
    let apply=u.hits.iter().find(|h|h.action==Action::MaterialApplyFaces).unwrap().rect;
    let remove=u.hits.iter().find(|h|h.action==Action::MaterialUnassignFaces).unwrap().rect;
    assert_eq!(apply[1],remove[1]);assert!(apply[0]+apply[2]<=remove[0]);
    let volume=u.hits.iter().find(|h|h.action==Action::SurfaceEmissionVolume).unwrap().rect;
    let alpha=u.hits.iter().find(|h|h.action==Action::SurfaceMaterialNumber(2)).unwrap().rect;
    assert!(volume[1]+volume[3]<=alpha[1]);
}
