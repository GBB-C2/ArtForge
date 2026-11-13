use super::*;
use crate::ui::{HelperAction as H,PathAction as P,DropdownKind as D};
use poseforge_core::{Node,CurveData,CurveInterpolation,ForceFieldKind,EmptyShape};
fn app()->App {let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.preferences=Preferences::default();a.preferences.right_panel=true;a.width=1440.;a.height=900.;a}
fn path(a:&mut App)->u64 {let id=a.editor.project.next_id();let mut n=Node::new(id,"Path");n.curve=Some(CurveData{points:vec![[-1.,0.,0.],[1.,0.,0.]],interpolation:CurveInterpolation::Polyline,..Default::default()});a.editor.project.nodes.push(n);id}
#[test]
fn every_new_creation_menu_entry_builds_a_real_object_and_undo_restores_scene() {
    let mut a=app();
    for (kind,count) in [(D::EmptyCreate,6),(D::ForceCreate,4),(D::MeshCreate,8)] {for i in 0..count {
        let before=a.editor.project.clone();a.ui=a.build_ui();a.action(Action::DropdownOpen(kind));assert!(a.dropdown_is_open(kind));a.action(Action::DropdownChoose(i));let id=a.editor.selected.unwrap();assert_eq!(a.editor.project.node(id).unwrap().transform.translation,Vec3::ZERO);
        if kind==D::ForceCreate {assert!(a.editor.project.node(id).unwrap().force_field.is_some());}else if kind==D::EmptyCreate {assert_eq!(a.editor.project.node(id).unwrap().empty_shape,EmptyShape::ALL[i]);}else{assert!(a.editor.project.node(id).unwrap().mesh.is_some());}
        assert!(a.editor.undo());assert_eq!(a.editor.project,before);a.reset_workflow_selection();
    }}
}
#[test]
fn field_properties_dropdown_and_eyedropper_fit_themes_dpi_and_keep_undo() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
        let mut a=app();a.preferences.theme=theme;a.scale=scale;path(&mut a);a.helper_action(H::CreateField(3));
        let before=a.editor.project.clone();let mut found=std::collections::BTreeSet::new();let max=a.inspector_max_scroll();
        for step in 0..=(max/24.).ceil() as usize {a.inspector_scroll=(step as f32*24.).min(max);a.ui=a.build_ui();
            for i in 0..4 {if a.ui.action_point(&Action::Helper(H::Number(i))).is_some() {found.insert(i);}}
            if let (Some(open),Some(pick))=(a.ui.action_point(&Action::DropdownOpen(D::CurvePath)),a.ui.action_point(&Action::Path(P::Pick))) {assert!(pick[0]>open[0]);found.insert(4);}
            if let (Some(play),Some(reset))=(a.ui.action_point(&Action::Helper(H::Play)),a.ui.action_point(&Action::Helper(H::Reset))) {
                assert!(reset[0]>play[0]);assert!((reset[1]-play[1]).abs()<0.01);found.insert(5);
            }
        }assert_eq!(found.len(),6);
        assert!(a.helper_confirm(&Action::Helper(H::Number(1)),"-3").is_err());assert_eq!(a.editor.project,before);
        a.helper_confirm(&Action::Helper(H::Number(0)),"-12,5").unwrap();assert_eq!(a.selected_helper().unwrap().force_field.unwrap().strength,-12.5);assert!(a.editor.undo());assert_eq!(a.editor.project,before);
        a.helper_action(H::Target(0));assert!(!a.selected_helper().unwrap().force_field.unwrap().targets[0]);a.editor.undo();assert_eq!(a.editor.project,before);
    }}
}
#[test]
fn curve_eyedropper_keeps_receiver_and_undo_and_cancel_do_not_click_through() {
    for receiver in 0..3 {
        let mut a=app();let curve=path(&mut a);
        if receiver==0 {a.create_primitive(0);}else if receiver==1 {a.create_empty_object();let id=a.editor.selected.unwrap();let n=a.editor.project.node_mut(id).unwrap();n.empty=false;n.camera=Some(Default::default());}else{a.action(Action::SurfaceAddLight(1));}
        a.set_properties_tab(0);let target=a.path_target().unwrap();let selection=a.editor.selected;let before=a.editor.project.clone();
        a.path_action(P::Pick);assert_eq!(a.path_pick,Some(target));a.action(Action::Select(999_999));assert!(a.path_pick.is_some());assert_eq!(a.editor.project,before);
        a.action(Action::Select(curve));assert!(a.path_pick.is_none());assert_eq!(a.editor.selected,selection);assert_eq!(a.path_binding().unwrap().curve,curve);assert!(a.editor.undo());assert_eq!(a.editor.project,before);
        a.path_action(P::Pick);a.action(Action::Cancel);assert!(a.path_pick.is_none());assert_eq!(a.editor.project,before);
        a.path_action(P::Pick);a.ui=a.build_ui();a.mouse=a.ui.action_point(&Action::DropdownOpen(D::MeshCreate)).unwrap();a.mouse_down(MouseButton::Left);a.mouse_up(MouseButton::Left);assert!(a.path_pick.is_none()&&a.dropdown.is_none());assert_eq!(a.editor.project,before);
    }
}
#[test]
fn viewport_curve_pick_is_geometric_and_skips_hidden_and_locked_paths() {
    let mut a=app();let curve=path(&mut a);a.create_empty_object();let receiver=a.editor.selected;
    let point=a.project_point(Vec3::ZERO).unwrap();assert_eq!(a.pick_curve_at(point),Some(curve));
    a.editor.project.node_mut(curve).unwrap().locked=true;assert_eq!(a.pick_curve_at(point),None);a.editor.project.node_mut(curve).unwrap().locked=false;
    a.ui=a.build_ui();a.path_action(P::Pick);a.mouse=point;a.mouse_down(MouseButton::Left);a.mouse_up(MouseButton::Left);assert_eq!(a.path_binding().unwrap().curve,curve);assert_eq!(a.editor.selected,receiver);
}
#[test]
fn enabled_fields_respect_targets_parent_visibility_and_solver_space() {
    let mut e=Editor::new(Project::default());let id=e.create_force_field(ForceFieldKind::Wind).unwrap();let p=&mut e.project;
    p.node_mut(id).unwrap().transform.rotation=Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let globals=p.global_transforms().unwrap();let fields=scene_fields::collect(p,&globals,false,0,Mat4::IDENTITY);assert!(fields.acceleration(Vec3::ZERO,Vec3::ZERO).x>9.);
    p.node_mut(id).unwrap().force_field.as_mut().unwrap().targets[0]=false;assert!(scene_fields::collect(p,&globals,false,0,Mat4::IDENTITY).is_empty());assert!(!scene_fields::collect(p,&globals,false,1,Mat4::IDENTITY).is_empty());
    p.node_mut(id).unwrap().visible=false;assert!(scene_fields::collect(p,&globals,false,1,Mat4::IDENTITY).is_empty());
}
