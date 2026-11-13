use super::*;
use crate::ui::PathAction as P;
use poseforge_core::{CameraLens, CurveBinding, CurveData, CurveInterpolation, LightKind, LightTarget, Node};

fn app() -> App {
    let mut a=App::new(None);a.preferences=Preferences::default();
    let mut path=Node::new(1,"Path");path.curve=Some(CurveData{points:vec![[-2.,0.,0.],[2.,0.,0.]],interpolation:CurveInterpolation::Polyline,..Default::default()});
    let mut camera=Node::new(2,"Camera");camera.camera=Some(CameraLens::default());
    camera.curve_binding=Some(CurveBinding{curve:1,position:0.5,..Default::default()});
    camera.transform.translation=Vec3::Y*0.7;
    camera.transform.rotation=Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let mut mesh=Node::new(3,"Cube");mesh.mesh=Some(10);mesh.transform.translation=Vec3::new(0.,-1.,0.);
    let mut p=Project{nodes:vec![path,camera,mesh],meshes:vec![poseforge_core::box_mesh(10,"Cube",Vec3::splat(0.2),None)],..Default::default()};
    p.lights.truncate(1);p.lights[0].kind=LightKind::Spot;p.lights[0].curve_binding=p.nodes[1].curve_binding;
    p.lights[0].position=Vec3::Y*0.2;p.lights[0].direction=Vec3::NEG_Z;
    p.lights[0].target=Some(LightTarget{node:3,local_center:Vec3::ZERO});
    a.editor=Editor::new(p);a.editor.mode=EditorMode::Scene;a.reset_workflow_selection();
    a.editor.selected=Some(2);a.surface.tab=0;a.width=1280.;a.height=900.;
    a.distance=8.;a.yaw=0.5;a.pitch=0.3;a.target=Vec3::ZERO;a.tool=Tool::Move;a
}
fn begin_camera(a:&mut App,handle:crate::gizmo::Handle)->[f32;2] {
    let world=a.selected_gizmo().unwrap().origin;
    a.mouse=a.project_point(world).unwrap();let start=a.mouse;
    a.begin_gizmo_drag(handle);assert!(matches!(a.drag,Some(Drag::Gizmo{..}|Drag::Group(_))));start
}

#[test]
fn bound_camera_center_and_axis_drags_slide_without_changing_local_transform() {
    for handle in [crate::gizmo::Handle::Center,crate::gizmo::Handle::Axis(0)] {
        let mut a=app();let before=a.editor.project.clone();let undo=a.editor.undo_len();
        let start=begin_camera(&mut a,handle);a.mouse_move([start[0]+65.,start[1]-13.]);
        let camera=a.editor.project.node(2).unwrap();assert_ne!(camera.curve_binding,before.node(2).unwrap().curve_binding);
        assert_eq!(camera.transform,before.node(2).unwrap().transform);assert_eq!(camera.rest_transform,before.node(2).unwrap().rest_transform);
        let position=a.editor.evaluated().unwrap().global[&2].w_axis.truncate();
        assert!(position.z.abs()<0.0001 && (position.y-0.7).abs()<0.0001);
        a.action(Action::Cancel);assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
        let start=begin_camera(&mut a,handle);a.mouse_move([start[0]+65.,start[1]-13.]);a.mouse_move(start);a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
        let start=begin_camera(&mut a,handle);a.mouse_move([start[0]+65.,start[1]-13.]);a.mouse_up(MouseButton::Left);
        let moved=a.editor.project.clone();assert_eq!(a.editor.undo_len(),undo+1);
        a.action(Action::Undo);assert_eq!(a.editor.project,before);a.action(Action::Redo);assert_eq!(a.editor.project,moved);
    }
}

#[test]
fn bound_light_drag_preserves_offset_target_and_direction_and_supports_undo() {
    let mut a=app();a.select_light(0);let before=a.editor.project.clone();let undo=a.editor.undo_len();
    for cancel in [true,false] {
        let point=a.project_point(a.evaluated_light(0).unwrap().position).unwrap();a.mouse=point;
        assert!(a.light_pointer_down());assert!(matches!(a.drag,Some(Drag::Light(_))));
        a.mouse_move([point[0]+65.,point[1]-13.]);let light=&a.editor.project.lights[0];
        assert_ne!(light.curve_binding,before.lights[0].curve_binding);assert_eq!(light.position,before.lights[0].position);
        assert_eq!(light.direction,before.lights[0].direction);assert_eq!(light.target,before.lights[0].target);
        let position=a.evaluated_light(0).unwrap().position;assert!(position.z.abs()<0.0001 && (position.y-0.2).abs()<0.0001);
        if cancel { a.action(Action::Cancel);assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo); }
        else { a.mouse_up(MouseButton::Left);assert_eq!(a.editor.undo_len(),undo+1);a.action(Action::Undo);assert_eq!(a.editor.project,before); }
    }
}

#[test]
fn camera_view_pan_and_scroll_follow_path_even_when_looking_across_it() {
    let mut a=app();a.camera_view=Some(2);a.camera_navigation_lock=true;a.mouse=[600.,300.];
    let before=a.editor.project.clone();let undo=a.editor.undo_len();
    assert!(a.begin_camera_navigation(true,MouseButton::Middle));a.mouse_move([540.,300.]);
    assert_ne!(a.editor.project.node(2).unwrap().curve_binding,before.node(2).unwrap().curve_binding);
    assert_eq!(a.editor.project.node(2).unwrap().transform,before.node(2).unwrap().transform);
    a.action(Action::Cancel);assert_eq!(a.editor.project,before);
    assert!(a.camera_scroll(1.));assert!(a.editor.project.node(2).unwrap().curve_binding.unwrap().position>0.5);
    assert_eq!(a.editor.project.node(2).unwrap().transform,before.node(2).unwrap().transform);
    assert_eq!(a.editor.undo_len(),undo+1);a.action(Action::Undo);assert_eq!(a.editor.project,before);
    a.mouse=[600.,300.];a.begin_view_navigation(2);a.mouse_move([600.,280.]);
    assert!(a.editor.project.node(2).unwrap().curve_binding.unwrap().position>0.5);
    assert_eq!(a.editor.project.node(2).unwrap().transform,before.node(2).unwrap().transform);
    a.mouse_up(MouseButton::Left);a.action(Action::Undo);assert_eq!(a.editor.project,before);
}

#[test]
fn unbound_camera_and_mesh_in_edit_keep_free_translation() {
    for id in [2,3] {
        let mut a=app();a.editor.project.node_mut(2).unwrap().curve_binding=None;
        a.editor.project.node_mut(3).unwrap().curve_binding=Some(CurveBinding{curve:1,position:0.5,..Default::default()});
        a.editor.selected=Some(id);
        if id==3 {a.action(Action::Path(P::Edit));}
        let before=a.editor.project.node(id).unwrap().clone();
        let start=begin_camera(&mut a,crate::gizmo::Handle::Center);a.mouse_move([start[0]+60.,start[1]-10.]);
        assert_ne!(a.editor.project.node(id).unwrap().transform,before.transform);
        assert_eq!(a.editor.project.node(id).unwrap().curve_binding,before.curve_binding);
    }
}

#[test]
fn bound_mesh_moves_along_path_until_edit_is_enabled() {
    let mut a=app();a.editor.selected=Some(3);
    a.editor.project.node_mut(3).unwrap().curve_binding=Some(CurveBinding{curve:1,position:0.5,..Default::default()});
    let before=a.editor.project.clone();let start=begin_camera(&mut a,crate::gizmo::Handle::Center);
    a.mouse_move([start[0]+60.,start[1]-30.]);a.mouse_up(MouseButton::Left);
    assert_ne!(a.editor.project.node(3).unwrap().curve_binding,before.node(3).unwrap().curve_binding);
    assert_eq!(a.editor.project.node(3).unwrap().transform,before.node(3).unwrap().transform);
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
}

#[test]
fn edit_offsets_and_reset_preserve_binding_orientation_and_one_step_history() {
    for receiver in [2,3,0] {for scale in [1.,2.] {
        let mut a=app();a.scale=scale;a.width*=scale;a.height*=scale;
        a.editor.project.node_mut(1).unwrap().transform.translation=Vec3::new(0.4,0.,0.2);
        a.editor.project.node_mut(1).unwrap().transform.rotation=Quat::from_rotation_y(0.6);
        a.editor.project.node_mut(3).unwrap().curve_binding=Some(CurveBinding{curve:1,position:0.5,..Default::default()});
        if receiver==0 {a.select_light(0);}else{a.editor.selected=Some(receiver);}
        let before=a.editor.project.clone();let binding=a.path_binding().unwrap();let undo=a.editor.undo_len();
        a.action(Action::Path(P::Edit));assert!(a.editing_path_curve().is_some());
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
        for cancel in [true,false] {
            let start=if receiver==0 {
                let point=a.project_point(a.evaluated_light(0).unwrap().position).unwrap();a.mouse=point;
                assert!(a.light_pointer_down());point
            }else{begin_camera(&mut a,crate::gizmo::Handle::Center)};
            let pointer=[start[0]+50.,start[1]-35.];a.mouse_move(pointer);
            assert_eq!(a.path_binding(),Some(binding));
            let world=if receiver==0 {a.evaluated_light(0).unwrap().position}
                else{a.editor.evaluated().unwrap().global[&receiver].w_axis.truncate()};
            let screen=a.project_point(world).unwrap();
            assert!((screen[0]-pointer[0]).abs()<0.2 && (screen[1]-pointer[1]).abs()<0.2,"free movement must follow the pointer in curve space");
            if cancel {a.action(Action::Cancel);assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);}
            else{a.mouse_up(MouseButton::Left);assert_eq!(a.editor.undo_len(),undo+1);}
        }
        let moved=a.editor.project.clone();
        a.action(Action::Path(P::ResetOffset));assert_eq!(a.path_offset(),Some(Vec3::ZERO));
        assert_eq!(a.path_binding(),Some(binding));assert_eq!(a.editor.project.clips,before.clips);
        if receiver==0 {
            assert_eq!(a.editor.project.lights[0].direction,before.lights[0].direction);
            assert_eq!(a.editor.project.lights[0].target,before.lights[0].target);
        } else {
            let transform=a.editor.project.node(receiver).unwrap().transform;
            assert_eq!(transform.rotation,before.node(receiver).unwrap().transform.rotation);
            assert_eq!(transform.scale,before.node(receiver).unwrap().transform.scale);
        }
        let count=a.editor.undo_len();a.action(Action::Path(P::ResetOffset));assert_eq!(a.editor.undo_len(),count);
        a.action(Action::Undo);assert_eq!(a.editor.project,moved);
        a.action(Action::Path(P::Edit));assert!(a.editing_path_curve().is_none());
        assert_eq!(a.editor.project,moved);
    }}
}

#[test]
fn edit_uses_authored_offsets_without_changing_existing_animation_keys() {
    use poseforge_core::{Channel,Clip,Interpolation,Keyframe,Property};
    let mut a=app();let mut clip=Clip::new("Camera animation",4.);
    clip.channels.push(Channel{node:2,property:Property::Translation,interpolation:Interpolation::Linear,
        keys:vec![Keyframe::new(0.,vec![0.,0.7,0.]),Keyframe::new(4.,vec![0.,1.7,0.])]});
    a.editor.project.clips.push(clip);a.editor.active_clip=Some(0);a.editor.time=1.25;
    a.editor.mode=EditorMode::Animation;a.editor.playing=true;
    let before=a.editor.project.clone();a.action(Action::Path(P::Edit));
    assert!(a.editor.mode==EditorMode::Scene);assert!(!a.editor.playing);
    assert_eq!(a.editor.project,before);
    let start=begin_camera(&mut a,crate::gizmo::Handle::Center);a.mouse_move([start[0]+40.,start[1]-20.]);a.mouse_up(MouseButton::Left);
    assert_ne!(a.editor.project.node(2).unwrap().transform,before.node(2).unwrap().transform);
    assert_eq!(a.editor.project.clips,before.clips);assert_eq!(a.editor.active_clip,Some(0));assert_eq!(a.editor.time,1.25);
    a.action(Action::Cancel);assert!(a.editing_path_curve().is_none());
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
}

#[test]
fn group_move_respects_camera_constraint_and_does_not_move_with_source_twice() {
    for companion in [3,1] {
        let mut a=app();a.scene_tools.selected=BTreeSet::from([2,companion]);
        let before=a.editor.project.clone();let start=begin_camera(&mut a,crate::gizmo::Handle::Center);
        // begin_camera uses the group gizmo when more than one object is selected.
        a.mouse_move([start[0]+60.,start[1]-10.]);
        let camera=a.editor.project.node(2).unwrap();assert_eq!(camera.transform,before.node(2).unwrap().transform);
        if companion==1 { assert_eq!(camera.curve_binding,before.node(2).unwrap().curve_binding); }
        else { assert_ne!(camera.curve_binding,before.node(2).unwrap().curve_binding); }
        a.action(Action::Cancel);assert_eq!(a.editor.project,before);
    }
}
