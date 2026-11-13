use poseforge_core::*;

fn fixture() -> Project {
    let mut p = Project::default();
    let mut curve = Node::new(1, "Path");
    curve.curve = Some(CurveData {
        points: vec![[0., 0., 0.], [4., 0., 0.]],
        interpolation: CurveInterpolation::Polyline,
        ..Default::default()
    });
    let mesh = PrimitiveSettings::new(PrimitiveKind::Cube).mesh(10, None).unwrap();
    let mut object = Node::new(2, "Mesh");
    object.mesh = Some(10);
    object.curve_binding = Some(CurveBinding { curve: 1, ..Default::default() });
    let mut camera = Node::new(3, "Camera");
    camera.camera = Some(CameraLens::default());
    camera.curve_binding = object.curve_binding;
    p.nodes = vec![curve, object, camera]; p.meshes = vec![mesh];
    p.lights.truncate(1);
    p.lights[0].curve_binding = p.nodes[1].curve_binding;
    p.lights[0].position = Vec3::ZERO; p.lights[0].direction = Vec3::NEG_Z;
    p.validate().unwrap(); p
}
fn close(a: Vec3, b: Vec3) { assert!(a.distance(b) < 0.0002, "{a:?} != {b:?}"); }
fn same_matrix(a: Mat4, b: Mat4) {
    for (x,y) in a.to_cols_array().into_iter().zip(b.to_cols_array()) { assert!((x-y).abs() < 0.0002); }
}

#[test]
fn mesh_camera_light_use_world_arc_length_and_distinct_forward_axes() {
    let mut p = fixture();
    let curve = p.node_mut(1).unwrap();
    curve.curve.as_mut().unwrap().points = vec![[0.,0.,0.],[1.,0.,0.],[1.,2.,0.]];
    curve.transform.scale = Vec3::new(2.,1.,1.);
    curve.transform.translation = Vec3::new(3.,0.,0.);
    for id in [2,3] { p.node_mut(id).unwrap().curve_binding.as_mut().unwrap().position = 0.25; }
    p.lights[0].curve_binding.as_mut().unwrap().position = 0.25;
    let scene = p.evaluated(None, 0.).unwrap();
    for id in [2,3] { close(scene.global[&id].transform_point3(Vec3::ZERO), Vec3::new(4.,0.,0.)); }
    close(scene.lights[0].position, Vec3::new(4.,0.,0.));
    close(scene.global[&2].transform_vector3(Vec3::Z), scene.global[&3].transform_vector3(Vec3::NEG_Z));
    close(scene.lights[0].direction, scene.global[&3].transform_vector3(Vec3::NEG_Z));
    assert!(scene.lights[0].curve_binding.is_none());
    assert!(p.lights[0].curve_binding.is_some());
}

#[test]
fn forward_reverse_repeat_clamp_and_offsets_follow_timeline() {
    let mut p = fixture();
    p.node_mut(2).unwrap().curve_binding = Some(CurveBinding { curve:1, position:0.25, speed:0.25, follow:false, ..Default::default() });
    p.node_mut(2).unwrap().transform.translation = Vec3::Y;
    close(p.evaluated(None, 1.).unwrap().global[&2].transform_point3(Vec3::ZERO), Vec3::new(2.,1.,0.));
    close(p.evaluated(None, 8.).unwrap().global[&2].transform_point3(Vec3::ZERO), Vec3::new(4.,1.,0.));
    p.node_mut(2).unwrap().curve_binding.as_mut().unwrap().repeat = true;
    close(p.evaluated(None, 4.).unwrap().global[&2].transform_point3(Vec3::ZERO), Vec3::new(1.,1.,0.));
    p.node_mut(2).unwrap().curve_binding.as_mut().unwrap().speed = -0.25;
    close(p.evaluated(None, 2.).unwrap().global[&2].transform_point3(Vec3::ZERO), Vec3::new(3.,1.,0.));
    p.node_mut(2).unwrap().curve_binding.as_mut().unwrap().repeat = false;
    close(p.evaluated(None, 2.).unwrap().global[&2].transform_point3(Vec3::ZERO), Vec3::Y);
    assert!(p.evaluated(None, f64::NAN).is_err());
}

#[test]
fn animated_curve_parent_and_follower_children_evaluate_in_dependency_order() {
    let mut p = fixture();
    let parent = Node::new(20, "Curve parent");
    let mut child = Node::new(21, "Follower child"); child.parent = Some(2); child.transform.translation = Vec3::Y;
    p.node_mut(1).unwrap().parent = Some(20);
    p.nodes.extend([child, parent]); p.nodes.reverse();
    let mut clip = Clip::new("Move path", 2.);
    clip.key_transform(20, 0., Transform::default());
    clip.key_transform(20, 1., Transform::from_translation(Vec3::new(0.,3.,0.)));
    p.clips.push(clip);
    let scene = p.evaluated(Some(0), 1.).unwrap();
    close(scene.global[&2].transform_point3(Vec3::ZERO), Vec3::new(0.,3.,0.));
    close(scene.global[&21].transform_point3(Vec3::ZERO), Vec3::new(0.,4.,0.));
    close(scene.lights[0].position, Vec3::new(0.,3.,0.));
}

#[test]
fn vertical_and_closed_paths_have_finite_continuous_frames() {
    let mut p = fixture();
    p.node_mut(1).unwrap().curve.as_mut().unwrap().points = vec![[0.,0.,0.],[0.,4.,0.]];
    let scene = p.evaluated(None, 0.).unwrap();
    close(scene.global[&3].transform_vector3(Vec3::NEG_Z), Vec3::Y);
    p.node_mut(1).unwrap().curve = Some(CurvePreset::Circle.curve());
    let globals = p.global_transforms().unwrap();
    let mut binding = CurveBinding { curve:1, ..Default::default() };
    let first = p.curve_binding_frame(binding, &globals, 0., false).unwrap();
    binding.position = 1.;
    same_matrix(first, p.curve_binding_frame(binding, &globals, 0., false).unwrap());
    for i in 0..=200 {
        binding.position = i as f32 / 200.;
        let m = p.curve_binding_frame(binding, &globals, 0., false).unwrap();
        assert!(m.is_finite()); assert!((m.determinant()-1.).abs() < 0.0001);
    }
}

#[test]
fn deletion_detaches_all_followers_at_current_time_and_undo_restores_connection() {
    let mut p = fixture();
    for id in [2,3] { p.node_mut(id).unwrap().curve_binding.as_mut().unwrap().speed = 0.25; }
    p.lights[0].curve_binding.as_mut().unwrap().speed = 0.25;
    let mut e = Editor::new(p); e.time = 2.;
    let original = e.project.clone(); let before = e.evaluated().unwrap();
    e.delete_node(1).unwrap();
    let after = e.evaluated().unwrap();
    for id in [2,3] {
        assert!(e.project.node(id).unwrap().curve_binding.is_none());
        same_matrix(before.global[&id], after.global[&id]);
    }
    assert!(e.project.lights[0].curve_binding.is_none());
    close(before.lights[0].position, after.lights[0].position);
    close(before.lights[0].direction, after.lights[0].direction);
    assert_eq!(e.undo_len(), 1); assert!(e.undo()); assert_eq!(e.project, original);
}

#[test]
fn detaching_rebases_animated_offsets_and_cubic_tangents() {
    let mut p = fixture();
    p.node_mut(2).unwrap().curve_binding.as_mut().unwrap().position = 0.5;
    let mut clip = Clip::new("Offset", 2.);
    clip.key_transform(2, 0., Transform::default());
    clip.key_transform(2, 2., Transform::from_translation(Vec3::new(0.,1.,1.)));
    for c in &mut clip.channels {
        c.interpolation = Interpolation::CubicSpline;
        for k in &mut c.keys { k.in_tangent = Some(vec![0.;k.value.len()]); k.out_tangent = Some(vec![0.;k.value.len()]); }
    }
    p.clips.push(clip);
    let frames: Vec<_> = [0.,0.5,1.,2.].into_iter().map(|t| p.evaluated(Some(0), t).unwrap().global[&2]).collect();
    let sampled = p.evaluated(Some(0), 1.).unwrap();
    p.detach_curve_node(2, &sampled.global, sampled.local[&2]).unwrap();
    p.validate().unwrap();
    for (i,t) in [0.,0.5,1.,2.].into_iter().enumerate() { same_matrix(frames[i], p.evaluated(Some(0), t).unwrap().global[&2]); }
}

#[test]
fn cycles_missing_sources_and_nonfinite_parameters_reject_without_history() {
    let mut e = Editor::new(fixture()); let before = e.project.clone();
    assert!(e.transact("Cycle", |p| p.node_mut(1).unwrap().parent = Some(2)).is_err());
    assert!(e.transact("Missing", |p| p.node_mut(2).unwrap().curve_binding.as_mut().unwrap().curve = 99).is_err());
    for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(e.transact("Invalid position", |p| p.node_mut(2).unwrap().curve_binding.as_mut().unwrap().position = value).is_err());
    }
    assert_eq!(e.project, before); assert_eq!(e.undo_len(), 0);
}

#[test]
fn subtree_duplicates_remap_internal_paths_and_retain_external_paths() {
    let mut p = fixture();
    p.nodes.push(Node::new(20, "Group"));
    for id in [1,2] { p.node_mut(id).unwrap().parent = Some(20); }
    let copy = p.duplicate_subtree(20).unwrap();
    let path = p.nodes.iter().find(|n| n.parent == Some(copy) && n.curve.is_some()).unwrap().id;
    let follower = p.nodes.iter().find(|n| n.parent == Some(copy) && n.mesh.is_some()).unwrap();
    assert_eq!(follower.curve_binding.unwrap().curve, path);
    let camera = p.duplicate_subtree(3).unwrap();
    assert_eq!(p.node(camera).unwrap().curve_binding.unwrap().curve, 1);
    p.validate().unwrap();
}

#[test]
fn removing_only_aim_target_keeps_path_and_world_direction() {
    let mut p = fixture();
    let mut target = Node::new(20, "Aim target"); target.transform.translation = Vec3::new(2.,3.,-1.);
    p.nodes.push(target); p.lights[0].curve_binding.as_mut().unwrap().position = 0.5;
    p.lights[0].target = Some(LightTarget { node:20, local_center:Vec3::ZERO });
    let mut e = Editor::new(p); let direction = e.evaluated().unwrap().lights[0].direction;
    e.delete_node(20).unwrap();
    assert!(e.project.lights[0].target.is_none()); assert!(e.project.lights[0].curve_binding.is_some());
    close(direction, e.evaluated().unwrap().lights[0].direction);
}

#[test]
fn converting_source_to_mesh_freezes_followers_and_keeps_them_in_scene() {
    let mut e = Editor::new(fixture());
    let mut curve = e.project.node(1).unwrap().curve.clone().unwrap();
    curve.geometry = Some(CurveGeometry::default());
    e.set_curve(1, curve).unwrap();
    e.project.node_mut(2).unwrap().curve_binding.as_mut().unwrap().position = 0.75;
    let before = e.evaluated().unwrap();
    e.convert_curve_to_mesh(1).unwrap();
    assert!(e.project.node(1).unwrap().mesh.is_some());
    assert!(e.project.node(2).unwrap().curve_binding.is_none());
    same_matrix(before.global[&2], e.evaluated().unwrap().global[&2]);
    assert!(e.undo()); assert!(e.project.node(2).unwrap().curve_binding.is_some());
}

#[test]
fn object_bone_binding_preserves_world_follows_and_survives_target_deletion() {
    let mut p = fixture();
    p.node_mut(1).unwrap().curve = None;
    p.node_mut(1).unwrap().is_joint = true;
    p.node_mut(1).unwrap().transform.translation = Vec3::new(3., 2., 1.);
    for id in [2,3] { p.node_mut(id).unwrap().curve_binding = None; }
    p.lights[0].curve_binding = None;
    p.node_mut(3).unwrap().transform.translation = Vec3::new(7., 4., -2.);
    let scene = p.evaluated(None, 0.).unwrap();
    p.bind_node_preserving_world(3, CurveBinding { curve:1, ..Default::default() }, &scene.global, 0.).unwrap();
    same_matrix(p.evaluated(None, 0.).unwrap().global[&3], scene.global[&3]);
    p.node_mut(1).unwrap().transform.translation += Vec3::Y;
    let moved = p.evaluated(None, 0.).unwrap();
    close(moved.global[&3].w_axis.truncate(), scene.global[&3].w_axis.truncate() + Vec3::Y);
    let mut e = Editor::new(p); let before = e.project.clone();
    e.delete_node(1).unwrap();
    assert!(e.project.node(3).unwrap().curve_binding.is_none());
    same_matrix(e.evaluated().unwrap().global[&3], moved.global[&3]);
    e.project.validate().unwrap(); assert!(e.undo()); assert_eq!(e.project, before);
    let restored: Project = serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
    restored.validate().unwrap(); assert_eq!(restored, before);
}

#[test]
fn general_bindings_reject_cycles_and_freeze_light_on_deleted_empty() {
    let mut p = fixture();
    p.node_mut(1).unwrap().curve = None; p.node_mut(1).unwrap().empty = true;
    p.node_mut(1).unwrap().transform.translation = Vec3::new(2.,3.,4.);
    let mut e = Editor::new(p); let before = e.project.clone();
    assert!(e.transact("cycle", |p| p.node_mut(1).unwrap().curve_binding = Some(CurveBinding {curve:2,..Default::default()})).is_err());
    assert_eq!(e.project, before); assert_eq!(e.undo_len(), 0);
    let scene = e.evaluated().unwrap(); e.delete_node(1).unwrap();
    assert!(e.project.lights[0].curve_binding.is_none());
    close(e.project.lights[0].position, scene.lights[0].position);
    close(e.project.lights[0].direction, scene.lights[0].direction);
    e.project.validate().unwrap();
}

#[test]
fn attaching_an_animated_camera_to_an_object_rebases_all_keys_without_a_jump() {
    let mut p=fixture();p.node_mut(1).unwrap().curve=None;p.node_mut(1).unwrap().empty=true;
    p.node_mut(1).unwrap().transform.translation=Vec3::new(3.,2.,1.);
    p.node_mut(1).unwrap().transform.rotation=Quat::from_rotation_y(0.4);
    p.node_mut(3).unwrap().curve_binding=None;
    let mut clip=Clip::new("Camera",2.);clip.key_transform(3,0.,Transform::from_translation(Vec3::Y));
    clip.key_transform(3,2.,Transform::from_translation(Vec3::new(1.,2.,3.)));p.clips.push(clip);
    let frames:Vec<_>=[0.,0.5,1.,2.].into_iter().map(|t|p.evaluated(Some(0),t).unwrap().global[&3]).collect();
    let sampled=p.evaluated(Some(0),1.).unwrap();
    p.bind_node_preserving_world(3,CurveBinding{curve:1,..Default::default()},&sampled.global,1.).unwrap();p.validate().unwrap();
    for (i,t) in [0.,0.5,1.,2.].into_iter().enumerate(){same_matrix(frames[i],p.evaluated(Some(0),t).unwrap().global[&3]);}
}
