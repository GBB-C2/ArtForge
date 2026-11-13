use super::*;
use poseforge_core::{CameraLens, CurveBinding, CurvePreset, Material, Node};

fn app() -> App {
    let mut a=App::new(None);a.preferences=Preferences::default();
    a.editor=Editor::new(Project::default());a.editor.mode=EditorMode::Scene;
    a.reset_workflow_selection();a.width=1280.;a.height=900.;a
}

#[test]
fn camera_navigation_on_a_curve_preserves_eye_on_rotation_and_cancels_atomically() {
    let mut a=app();let mut path=Node::new(1,"Path");path.curve=Some(CurvePreset::Line.curve());
    path.transform.translation=Vec3::new(4.,1.,-2.);
    let mut camera=Node::new(2,"Camera");camera.camera=Some(CameraLens::default());
    camera.curve_binding=Some(CurveBinding{curve:1,position:0.5,..Default::default()});
    a.editor=Editor::new(Project{nodes:vec![path,camera],..Default::default()});
    a.editor.mode=EditorMode::Scene;a.camera_view=Some(2);a.camera_navigation_lock=true;
    a.editor.selected=Some(2);a.mouse=[600.,300.];
    let original=a.editor.project.clone();let before=a.view().eye;
    assert!(a.begin_camera_navigation(false,MouseButton::Right));
    a.mouse_move([650.,325.]);
    assert!(a.view().eye.distance(before)<0.0001,"looking around must not add the path frame twice");
    assert_ne!(a.editor.project,original);
    a.action(Action::Cancel);assert_eq!(a.editor.project,original);assert!(!a.editor.has_active_edit());
    let history=a.editor.undo_len();assert!(a.camera_scroll(1.));
    assert_eq!(a.editor.undo_len(),history+1);
    assert!(a.editor.project.node(2).unwrap().curve_binding.is_some());
    a.action(Action::Undo);assert_eq!(a.editor.project,original);
}

#[test]
fn render_view_reuses_static_document_when_no_override_is_needed() {
    let mut a=app();let mut node=Node::new(1,"Cube");node.mesh=Some(2);
    a.editor=Editor::new(Project{nodes:vec![node],meshes:vec![poseforge_core::box_mesh(2,"Cube",Vec3::ONE,Some(3))],
        materials:vec![Material{id:3,double_sided:false,..Default::default()}],..Default::default()});
    a.preferences.viewport_shading=3;a.preferences.show_backfaces=false;
    let eval=a.editor.evaluated().unwrap();
    let mut frame=frame_preparation::render_project(&a.editor.project,&eval,false);
    if a.viewport_shading_needs_override(&frame) { a.apply_viewport_shading(frame.to_mut()); }
    assert!(matches!(frame,std::borrow::Cow::Borrowed(_)));
    drop(frame);a.preferences.show_backfaces=true;
    assert!(a.viewport_shading_needs_override(&a.editor.project));
    a.preferences.show_backfaces=false;a.preferences.viewport_shading=1;
    assert!(a.viewport_shading_needs_override(&a.editor.project));
}

#[test]
fn atelier_scene_validates_evaluates_and_imports_as_one_background_undo_step() {
    let scene=asset_library::library_scene(2).unwrap();scene.validate().unwrap();
    assert_eq!(scene.nodes.iter().filter(|n|n.is_joint).count(),17);
    assert_eq!(scene.nodes.iter().filter(|n|n.curve_binding.is_some()).count(),2);
    for time in [0.,3.,6.,9.,12.] { scene.evaluated(None,time).unwrap(); }
    let mut a=app();let before=a.editor.project.clone();let undo=a.editor.undo_len();
    a.action(Action::AssetScene(2));assert!(a.job.is_some());assert!(a.scene_write_pending());
    assert_eq!(a.editor.project,before);
    a.wait_for_background_jobs();a.editor.project.validate().unwrap();
    assert!(a.editor.project.nodes.len()>30);assert_eq!(a.editor.undo_len(),undo+1);
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
}

#[test]
fn cancelled_and_stale_library_jobs_never_replace_current_scene() {
    let mut a=app();let before=a.editor.project.clone();
    a.action(Action::AssetScene(2));a.action(Action::Cancel);a.wait_for_background_jobs();
    assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),0);
    a.action(Action::AssetScene(0));a.editor.project.title="Newer document edit".into();
    let edited=a.editor.project.clone();a.wait_for_background_jobs();
    assert_eq!(a.editor.project,edited);assert_eq!(a.editor.undo_len(),0);
}
