use poseforge_core::*;

fn scene()->(Editor,u64) {
    let mut p=Project::default();let mut mesh=Node::new(10,"Emitter");mesh.mesh=Some(11);
    mesh.transform.translation=Vec3::new(3.,2.,1.);
    let mut curve=Node::new(12,"Curve");curve.curve=Some(CurveData{points:vec![[0.,0.,0.],[2.,0.,0.]],interpolation:CurveInterpolation::Polyline,..Default::default()});
    p.meshes.push(box_mesh(11,"Emitter",Vec3::ONE,None));p.nodes.extend([mesh,curve]);
    let mut e=Editor::new(p);let fx=e.create_fx(FxKind::Smoke).unwrap();(e,fx)
}
fn same(a:Mat4,b:Mat4) {assert!(a.to_cols_array().iter().zip(b.to_cols_array()).all(|(a,b)|(*a-b).abs()<0.0002));}

#[test]
fn sources_follow_transform_and_cannot_form_a_dependency_cycle() {
    let (mut e,id)=scene();let before=e.project.clone();let undo=e.undo_len();e.set_fx_source(id,Some(10)).unwrap();
    let globals=e.project.global_transforms().unwrap();
    same(globals[&id],globals[&10]*e.project.node(id).unwrap().transform.matrix());
    same(e.project.transform_frame(id,&globals,0.).unwrap(),globals[&10]);
    e.set_fx_source(id,Some(10)).unwrap();assert_eq!(e.undo_len(),undo+1);
    e.undo();assert_eq!(e.project,before);e.set_fx_source(id,Some(12)).unwrap();
    let bound=e.project.clone();
    assert!(e.transact("Cycle",|p|p.node_mut(12).unwrap().parent=Some(id)).is_err());assert_eq!(e.project,bound);
    assert!(e.set_fx_source(id,Some(id)).is_err());assert!(e.set_fx_source(id,Some(999)).is_err());assert_eq!(e.project,bound);
}

#[test]
fn detachment_preserves_animated_world_pose_and_is_one_undo() {
    let (mut e,id)=scene();e.set_fx_source(id,Some(10)).unwrap();
    let mut clip=Clip::new("Offset",2.);
    clip.channels.push(Channel{node:id,property:Property::Translation,interpolation:Interpolation::Linear,
        keys:vec![Keyframe::new(0.,vec![0.,0.,0.]),Keyframe::new(2.,vec![1.,0.,0.])]});
    e.project.clips.push(clip);e.active_clip=Some(0);e.time=0.75;
    let before=e.project.clone();let poses:Vec<_>=[0.,0.75,2.].into_iter().map(|t|before.evaluated(Some(0),t).unwrap().global[&id]).collect();
    let undo=e.undo_len();e.set_fx_source(id,None).unwrap();assert_eq!(e.undo_len(),undo+1);
    for (t,pose) in [0.,0.75,2.].into_iter().zip(poses) {same(e.project.evaluated(Some(0),t).unwrap().global[&id],pose);}
    e.undo();assert_eq!(e.project,before);
}

#[test]
fn deleting_source_disables_emission_and_clears_obstacles_without_moving_fx() {
    let (mut e,id)=scene();e.set_fx_source(id,Some(10)).unwrap();
    let mut obstacle=Node::new(30,"Obstacle");obstacle.mesh=Some(11);e.project.nodes.push(obstacle);
    e.project.node_mut(id).unwrap().collision_targets.fx=Some(vec![CollisionTarget{node:30,enabled:true}]);
    let before=e.project.clone();let world=e.project.global_transforms().unwrap()[&id];
    e.delete_node(30).unwrap();assert!(e.project.resolved_collision_targets(id,CollisionKind::Fx).is_empty());e.undo();assert_eq!(e.project,before);
    e.delete_node(10).unwrap();same(e.project.global_transforms().unwrap()[&id],world);
    let settings=e.project.node(id).unwrap().fx.unwrap();assert!(settings.source.is_none() && !settings.enabled);
    e.project.validate().unwrap();e.undo();assert_eq!(e.project,before);
}

#[test]
fn copied_group_remaps_source_and_obstacle_and_binding_removes_self_collision() {
    let (mut e,id)=scene();let mut root=Node::new(50,"Group");root.empty=true;e.project.nodes.push(root);
    for node in &mut e.project.nodes {if node.id!=50 {node.parent=Some(50);}}
    e.project.node_mut(id).unwrap().collision_targets.fx=Some(vec![CollisionTarget{node:10,enabled:true}]);
    e.set_fx_source(id,Some(10)).unwrap();assert!(e.project.resolved_collision_targets(id,CollisionKind::Fx).is_empty());
    let mut obstacle=Node::new(60,"Obstacle");obstacle.parent=Some(50);obstacle.mesh=Some(11);e.project.nodes.push(obstacle);
    e.project.node_mut(id).unwrap().collision_targets.fx=Some(vec![CollisionTarget{node:60,enabled:false}]);
    let copy=e.project.duplicate_subtree(50).unwrap();
    let fx=e.project.nodes.iter().find(|n|n.parent==Some(copy) && n.fx.is_some()).unwrap();
    let source=fx.fx.unwrap().source.unwrap();assert_ne!(source,10);assert_eq!(e.project.node(source).unwrap().parent,Some(copy));
    let obstacle=e.project.resolved_collision_targets(fx.id,CollisionKind::Fx)[0];assert_ne!(obstacle.node,60);assert!(!obstacle.enabled);
    assert_eq!(e.project.node(obstacle.node).unwrap().parent,Some(copy));e.project.validate().unwrap();
}
