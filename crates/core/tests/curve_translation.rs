use poseforge_core::*;

fn fixture() -> (Project, CurveBinding) {
    let mut curve = Node::new(1, "Path");
    curve.curve = Some(CurveData { points: vec![[0.,0.,0.],[4.,0.,0.]], interpolation: CurveInterpolation::Polyline, ..Default::default() });
    (Project { nodes: vec![curve], ..Default::default() }, CurveBinding { curve:1, position:0.5, ..Default::default() })
}
fn close(a:f32,b:f32) { assert!((a-b).abs()<0.0002, "{a} != {b}"); }

#[test]
fn pointer_projection_keeps_offset_and_clamps_to_world_space_path() {
    let (mut p,b)=fixture();
    let n=p.node_mut(1).unwrap();n.transform.scale=Vec3::new(2.,3.,1.);n.transform.translation=Vec3::new(3.,2.,1.);
    let global=p.global_transforms().unwrap();let offset=Vec3::new(0.,0.7,0.);
    let drag=p.curve_translation(b,&global,0.,offset,true).unwrap();
    close(drag.position(Vec3::new(9.,8.,1.)).unwrap(),0.75);
    close(drag.position(Vec3::new(-100.,8.,1.)).unwrap(),0.);
    close(drag.position(Vec3::new(100.,8.,1.)).unwrap(),1.);
    let pos=drag.position(Vec3::new(9.,8.,1.)).unwrap();
    let world=p.curve_binding_frame(CurveBinding{position:pos,..b},&global,0.,true).unwrap().transform_point3(offset);
    assert!(world.distance(Vec3::new(9.,2.7,1.))<0.0003);
}

#[test]
fn curved_offset_trajectory_uses_the_same_frame_as_the_evaluator() {
    let (mut p,b)=fixture();p.node_mut(1).unwrap().curve=Some(CurvePreset::Circle.curve());
    let global=p.global_transforms().unwrap();
    for follow in [false,true] { for negative in [false,true] {
        let binding=CurveBinding{follow,position:0.2,..b};let offset=Vec3::new(0.3,0.7,0.15);
        let target=p.curve_binding_frame(CurveBinding{position:0.73,..binding},&global,0.,negative).unwrap().transform_point3(offset);
        let drag=p.curve_translation(binding,&global,0.,offset,negative).unwrap();
        let position=drag.position(target).unwrap();close(position,0.73);
        let world=p.curve_binding_frame(CurveBinding{position,..binding},&global,0.,negative).unwrap().transform_point3(offset);
        assert!(world.distance(target)<0.001);
    }}
}

#[test]
fn animated_repeat_preserves_speed_and_offsets_the_base_position() {
    let (p,b)=fixture();let b=CurveBinding{position:0.6,speed:0.25,repeat:true,..b};let global=p.global_transforms().unwrap();
    let drag=p.curve_translation(b,&global,2.,Vec3::ZERO,true).unwrap();
    close(drag.position(Vec3::new(3.8,0.,0.)).unwrap(),0.45);
    close(drag.advance(1.).unwrap(),0.85);
    assert_eq!(drag.binding(),b);
    let initial=p.curve_binding_frame(b,&global,2.,true).unwrap().transform_point3(Vec3::ZERO);
    assert_eq!(drag.position(initial).unwrap(),b.position);
    assert_eq!(drag.advance(0.).unwrap(),b.position);
    let end=drag.position(Vec3::X*20.).unwrap();
    let world=p.curve_binding_frame(CurveBinding{position:end,..b},&global,2.,true).unwrap().w_axis.truncate();
    assert!(world.x>3.999 && world.x<=4., "open repeating endpoint must not teleport to the beginning");
    for time in [1.,3.,7.] {
        let b=CurveBinding{speed:0.1,..b};let drag=p.curve_translation(b,&global,time,Vec3::ZERO,true).unwrap();
        let start=drag.position(Vec3::NEG_X*20.).unwrap();
        let world=p.curve_binding_frame(CurveBinding{position:start,..b},&global,time,true).unwrap().w_axis.truncate();
        assert!(world.x<0.001, "rounding at the loop seam must preserve the first endpoint");
    }
}

#[test]
fn nonrepeating_animation_only_uses_reachable_phases() {
    let (p,b)=fixture();let b=CurveBinding{position:0.4,speed:0.1,..b};let global=p.global_transforms().unwrap();
    let drag=p.curve_translation(b,&global,2.,Vec3::ZERO,true).unwrap();
    close(drag.position(Vec3::NEG_X).unwrap(),0.);
    close(drag.position(Vec3::X*100.).unwrap(),0.8);
    close(drag.advance(100.).unwrap(),0.8);
    let finished=p.curve_translation(b,&global,20.,Vec3::ZERO,true).unwrap();
    assert_eq!(finished.position(Vec3::ZERO).unwrap(),b.position);
    assert_eq!(finished.advance(-100.).unwrap(),b.position);
}

#[test]
fn closed_seam_and_vertical_duplicate_points_stay_finite() {
    let (mut p,b)=fixture();p.node_mut(1).unwrap().curve=Some(CurvePreset::Circle.curve());
    let global=p.global_transforms().unwrap();let b=CurveBinding{position:0.99,repeat:true,..b};
    let target=p.curve_binding_frame(CurveBinding{position:0.01,..b},&global,0.,true).unwrap().w_axis.truncate();
    let drag=p.curve_translation(b,&global,0.,Vec3::ZERO,true).unwrap();close(drag.position(target).unwrap(),0.01);
    let (mut p,b)=fixture();p.node_mut(1).unwrap().curve.as_mut().unwrap().points=vec![[0.,0.,0.],[0.,0.,0.],[0.,4.,0.]];
    let global=p.global_transforms().unwrap();let drag=p.curve_translation(b,&global,0.,Vec3::ZERO,true).unwrap();
    close(drag.position(Vec3::new(2.,3.,1.)).unwrap(),0.75);
}

#[test]
fn invalid_inputs_are_rejected_without_modifying_the_document() {
    let (p,b)=fixture();let before=p.clone();let global=p.global_transforms().unwrap();
    assert!(p.curve_translation(b,&global,f64::NAN,Vec3::ZERO,true).is_err());
    assert!(p.curve_translation(b,&global,0.,Vec3::splat(f32::NAN),true).is_err());
    assert!(p.curve_translation(CurveBinding{curve:999,..b},&global,0.,Vec3::ZERO,true).is_err());
    let drag=p.curve_translation(b,&global,0.,Vec3::ZERO,true).unwrap();
    assert!(drag.position(Vec3::splat(f32::NAN)).is_err());assert!(drag.advance(f32::INFINITY).is_err());assert_eq!(p,before);
    let repeated=p.curve_translation(CurveBinding{repeat:true,..b},&global,0.,Vec3::ZERO,true).unwrap();
    assert!(repeated.advance(f32::MAX).unwrap().is_finite());
}
