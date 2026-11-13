use poseforge_core::*;
use std::collections::BTreeMap;

fn near(a: Vec3, b: Vec3) {
    assert!((a - b).length() < 1e-4, "{a:?} != {b:?}");
}

#[test]
fn demo_is_valid_and_bind_skin_is_identity_in_world() {
    let p = Project::demo();
    p.validate().unwrap();
    let globals = p.global_transforms().unwrap();
    let n = p.node(20).unwrap();
    let skin = n.skin.as_ref().unwrap();
    for (j, ibm) in skin.joints.iter().zip(&skin.inverse_bind_matrices) {
        let m = globals[j] * *ibm;
        near(
            m.transform_point3(Vec3::new(0.2, 0.8, 0.5)),
            Vec3::new(0.2, 0.8, 0.5),
        );
    }
}

#[test]
fn starter_geometry_is_shared_but_editing_one_scene_cannot_change_another() {
    let mut first = Project::demo();
    let second = Project::demo();
    assert!(first.meshes[0].vertices.shares_storage(&second.meshes[0].vertices));
    assert!(first.meshes.iter().map(|m| m.vertices.len()).sum::<usize>() < 16_000);
    let untouched = second.meshes[0].vertices[0].position;
    first.meshes[0].vertices[0].position[0] += 0.5;
    assert_eq!(second.meshes[0].vertices[0].position, untouched);
    assert_eq!(Project::demo().meshes[0].vertices[0].position, untouched);
    second.validate().unwrap();
}

#[test]
fn mannequin_starts_in_its_t_bind_pose_before_waving() {
    let project = Project::demo();
    let rest = project.global_transforms().unwrap();
    let first = project.evaluated(Some(0), 0.).unwrap();
    for (id, matrix) in &rest {
        for (a, b) in matrix.to_cols_array().into_iter().zip(first.global[id].to_cols_array()) {
            assert!((a - b).abs() < 1e-6);
        }
    }
    for (shoulder, elbow, wrist) in [(7, 8, 9), (10, 11, 12)] {
        let upper = rest[&shoulder].transform_point3(Vec3::ZERO);
        let middle = rest[&elbow].transform_point3(Vec3::ZERO);
        let lower = rest[&wrist].transform_point3(Vec3::ZERO);
        assert!((upper.y - middle.y).abs() < 1e-6 && (middle.y - lower.y).abs() < 1e-6);
        assert!(middle.x.abs() > upper.x.abs() + 0.2 && lower.x.abs() > middle.x.abs() + 0.2);
    }
    assert_ne!(project.evaluated(Some(0), 0.7).unwrap().global[&12], rest[&12]);
}
#[test]
fn duplicate_skeleton_instances_have_independent_pose_and_animation() {
    let mut p = Project::demo();
    let original = p.global_transforms().unwrap();
    let new_root = p.duplicate_subtree(1).unwrap();
    assert_ne!(new_root, 1);
    p.validate().unwrap();
    let cloned_arm = p
        .nodes
        .iter()
        .find(|n| n.name == "Плечо.R — копия")
        .unwrap()
        .id;
    p.node_mut(cloned_arm).unwrap().transform.rotation = Quat::from_rotation_z(1.);
    let now = p.global_transforms().unwrap();
    assert_eq!(original[&10], now[&10]);
    let original_keys = p.clips[0]
        .channels
        .iter()
        .find(|c| c.node == 10 && c.property == Property::Rotation)
        .unwrap()
        .keys
        .clone();
    let mut changed = p.node(cloned_arm).unwrap().transform;
    changed.translation += Vec3::Z;
    p.clips[0].key_transform(cloned_arm, 0.5, changed);
    assert_eq!(
        original_keys,
        p.clips[0]
            .channels
            .iter()
            .find(|c| c.node == 10 && c.property == Property::Rotation)
            .unwrap()
            .keys
    );
}
#[test]
fn direct_sampling_equals_sequential_sampling_and_fps_does_not_retime() {
    let mut p = Project::demo();
    let direct = p.evaluated(Some(0), 1.25).unwrap();
    for i in 0..=125 {
        p.evaluated(Some(0), i as f64 / 100.).unwrap();
    }
    let sequential = p.evaluated(Some(0), 1.25).unwrap();
    assert_eq!(direct.global, sequential.global);
    p.fps = 60;
    assert_eq!(direct.global, p.evaluated(Some(0), 1.25).unwrap().global);
}
#[test]
fn cubic_tangents_use_seconds_and_quaternions_use_short_arc() {
    let c = Channel {
        node: 1,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        keys: vec![
            Keyframe {
                time: 0.,
                value: vec![0., 0., 0.],
                in_tangent: Some(vec![0.; 3]),
                out_tangent: Some(vec![2., 0., 0.]),
            },
            Keyframe {
                time: 2.,
                value: vec![0., 0., 0.],
                in_tangent: Some(vec![0.; 3]),
                out_tangent: Some(vec![0.; 3]),
            },
        ],
    };
    assert!((c.sample(1.).unwrap()[0] - 0.5).abs() < 1e-6);
    let qa = Quat::from_rotation_z(170f32.to_radians());
    let qb = Quat::from_rotation_z(-170f32.to_radians());
    let c = Channel {
        node: 1,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        keys: vec![
            Keyframe::new(0., qa.to_array().to_vec()),
            Keyframe::new(1., qb.to_array().to_vec()),
        ],
    };
    let q = Quat::from_array(c.sample(0.5).unwrap().try_into().unwrap());
    near(q * Vec3::X, -Vec3::X);
}
#[test]
fn two_bone_ik_preserves_lengths_reachable_unreachable_and_degenerate_pole() {
    for target in [Vec3::new(1.2, 0.8, 0.), Vec3::new(100., 0., 0.), Vec3::ZERO] {
        let solution = solve_two_bone(
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(2., 0., 0.),
            target,
            Vec3::new(1., 0., 0.),
        )
        .unwrap();
        assert!((solution.root.distance(solution.middle) - 1.).abs() < 1e-4);
        assert!((solution.middle.distance(solution.end) - 1.).abs() < 1e-4);
        assert!(solution.middle.is_finite() && solution.end.is_finite());
    }
    assert!(solve_two_bone(Vec3::ZERO, Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z).is_err());
}
#[test]
fn ik_rotations_actually_move_tip_to_target_and_undo_restores() {
    let p = Project::demo();
    let mut e = Editor::new(p);
    let before = e.project.clone();
    let pose = e.evaluated().unwrap();
    let shoulder = pose.global[&10].transform_point3(Vec3::ZERO);
    let target = shoulder + Vec3::new(0.35, 0.38, 0.1);
    e.apply_ik(10, 11, 12, target, shoulder + Vec3::Z).unwrap();
    near(
        e.evaluated().unwrap().global[&12].transform_point3(Vec3::ZERO),
        target,
    );
    assert_eq!(e.undo_len(), 1);
    assert!(e.undo());
    assert_eq!(e.project, before);
}
#[test]
fn gesture_cancel_and_commit_are_single_undo_transactions() {
    let mut e = Editor::new(Project::demo());
    let before = e.project.clone();
    e.begin_edit("Перетаскивание");
    for i in 0..30 {
        let mut t = e.project.node(10).unwrap().transform;
        t.rotation = Quat::from_rotation_z(i as f32 * 0.02);
        e.set_transform(10, t).unwrap();
    }
    e.cancel_edit();
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), 0);
    e.begin_edit("Перетаскивание");
    for i in 0..30 {
        let mut t = e.project.node(10).unwrap().transform;
        t.rotation = Quat::from_rotation_z(i as f32 * 0.02);
        e.set_transform(10, t).unwrap();
    }
    e.commit_edit().unwrap();
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_ne!(e.project, before);
}
#[test]
fn staged_pose_blocks_time_change_survives_evaluation_and_can_be_keyed() {
    let mut e = Editor::new(Project::demo());
    e.mode = EditorMode::Animation;
    e.selected = Some(10);
    e.set_time(0.5).unwrap();
    let mut transform = e.evaluated().unwrap().local[&10];
    transform.rotation = Quat::from_rotation_z(0.75);
    e.set_transform(10, transform).unwrap();
    assert!(e.has_staged());
    assert_eq!(e.evaluated().unwrap().local[&10], transform);
    assert!(matches!(e.set_time(1.), Err(CoreError::UncommittedPose)));
    assert_eq!(e.time, 0.5);
    e.key_selected().unwrap();
    assert!(!e.has_staged());
    e.set_time(1.).unwrap();
    e.set_time(0.5).unwrap();
    let q = e.evaluated().unwrap().local[&10].rotation;
    assert!(q.dot(transform.rotation).abs() > 0.99999);
}
#[test]
fn invalid_transaction_rolls_back_and_cycle_is_reported() {
    let mut e = Editor::new(Project::demo());
    let before = e.project.clone();
    assert!(e
        .transact("bad", |p| p.node_mut(1).unwrap().parent = Some(18))
        .is_err());
    assert_eq!(e.project, before);
    let mut p = Project::default();
    for i in 0..2000 {
        let mut n = Node::new(i, "deep");
        n.parent = if i == 0 { None } else { Some(i - 1) };
        p.nodes.push(n);
    }
    assert_eq!(p.global_transforms().unwrap().len(), 2000);
    p.node_mut(0).unwrap().parent = Some(1999);
    assert!(matches!(
        p.global_transforms(),
        Err(CoreError::HierarchyCycle(_))
    ));
}
#[test]
fn serializable_authoring_document_roundtrips_without_evaluated_state() {
    let p = Project::demo();
    let bytes = serde_json::to_vec(&p).unwrap();
    let restored: Project = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(p, restored);
    restored.validate().unwrap();
    assert_eq!(
        p.evaluated(Some(0), 1.5).unwrap().global,
        restored.evaluated(Some(0), 1.5).unwrap().global
    );
}
#[test]
fn hierarchy_uses_ids_and_skin_joint_order_is_preserved() {
    let mut p = Project::default();
    let mut a = Node::new(50, "same");
    a.transform.translation = Vec3::X;
    let mut b = Node::new(7, "same");
    b.parent = Some(50);
    b.transform.translation = Vec3::Y;
    p.nodes = vec![b, a];
    near(
        p.global_transforms().unwrap()[&7].transform_point3(Vec3::ZERO),
        Vec3::X + Vec3::Y,
    );
    let overrides = BTreeMap::from([(50, Transform::from_translation(Vec3::Z))]);
    near(
        p.globals_for(&overrides).unwrap()[&7].transform_point3(Vec3::ZERO),
        Vec3::Z + Vec3::Y,
    );
}
#[test]
fn rest_pose_survives_save_reload_and_editing() {
    let mut e = Editor::new(Project::demo());
    let rest = e.project.node(10).unwrap().transform;
    let mut changed = rest;
    changed.rotation = Quat::from_rotation_x(1.);
    e.set_transform(10, changed).unwrap();
    let encoded = serde_json::to_vec(&e.project).unwrap();
    let p = serde_json::from_slice(&encoded).unwrap();
    let mut reopened = Editor::new(p);
    reopened.selected = Some(10);
    reopened.reset_selected().unwrap();
    assert_eq!(reopened.project.node(10).unwrap().transform, rest);
}
#[test]
fn degenerate_camera_light_and_animation_are_rejected() {
    let mut p = Project::demo();
    p.camera.up = (p.camera.target - p.camera.eye).normalize();
    assert!(p.validate().is_err());
    let mut p = Project::demo();
    p.lights[0].direction = Vec3::ZERO;
    assert!(p.validate().is_err());
    let c = Channel {
        node: 1,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        keys: vec![
            Keyframe::new(0., vec![1.; 3]),
            Keyframe::new(1., vec![-1.; 3]),
        ],
    };
    let mut p = Project::demo();
    p.clips.push(Clip {
        name: "zero scale".into(),
        duration: 1.,
        channels: vec![c],
    });
    assert!(p.evaluated(Some(1), 0.5).is_err());
}
#[test]
fn undo_restores_rest_pose_map_before_a_node_id_is_reused() {
    let mut e = Editor::new(Project::default());
    e.transact("add", |p| {
        let mut n = Node::new(8, "first");
        n.transform.translation = Vec3::X;
        p.nodes.push(n);
    })
    .unwrap();
    e.undo();
    e.transact("add again", |p| {
        let mut n = Node::new(8, "second");
        n.transform.translation = Vec3::Y;
        p.nodes.push(n);
    })
    .unwrap();
    e.set_transform(8, Transform::from_translation(Vec3::Z))
        .unwrap();
    e.selected = Some(8);
    e.reset_selected().unwrap();
    assert_eq!(e.project.node(8).unwrap().transform.translation, Vec3::Y);
}
#[test]
fn matrix_overflow_and_id_exhaustion_fail_without_panicking() {
    let mut p = Project::default();
    for id in 0..4 {
        let mut n = Node::new(id, "huge");
        n.transform.scale = Vec3::splat(1e20);
        n.parent = if id == 0 { None } else { Some(id - 1) };
        p.nodes.push(n);
    }
    assert!(p.validate().is_err());
    let mut p = Project::default();
    p.nodes.push(Node::new(u64::MAX, "last"));
    assert_eq!(p.next_id(), u64::MAX);
    assert!(p.duplicate_subtree(u64::MAX).is_err());
}
#[test]
fn render_preserves_static_or_animated_preview_and_single_sequence_pose_agree() {
    let mut e = Editor::new(Project::demo());
    e.time = 1.25;
    let static_pose = e.evaluated().unwrap();
    e.mode = EditorMode::Render;
    e.render_animated = false;
    assert_eq!(static_pose.global, e.evaluated().unwrap().global);
    assert_eq!(e.evaluated_clip(), None);
    e.mode = EditorMode::Animation;
    let animated = e.evaluated().unwrap();
    e.mode = EditorMode::Render;
    e.render_animated = true;
    let render = e.evaluated().unwrap();
    assert_eq!(animated.global, render.global);
    let single = e.project.evaluated(e.evaluated_clip(), 1.25).unwrap();
    let sequence = e
        .project
        .evaluated(e.evaluated_clip(), 30f64 / 24.)
        .unwrap();
    assert_eq!(single.global, sequence.global);
    assert_eq!(single.global, render.global);
}
