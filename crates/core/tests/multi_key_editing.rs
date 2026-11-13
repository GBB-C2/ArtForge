use poseforge_core::*;

fn editor() -> Editor {
    let mut project = Project {
        duration: 2.,
        ..Project::default()
    };
    let mut hand = Node::new(2, "Hand");
    hand.parent = Some(1);
    let mut foot = Node::new(3, "Foot");
    foot.parent = Some(1);
    project.nodes = vec![Node::new(1, "Character"), hand, foot];
    let mut clip = Clip::new("Motion", 2.);
    for node in [2, 3] {
        clip.key_transform(node, 0., Transform::default());
        clip.key_transform(node, 1., Transform::from_translation(Vec3::Y));
    }
    project.clips.push(clip);
    let mut editor = Editor::new(project);
    editor.mode = EditorMode::Animation;
    editor
}

fn key(node: NodeId, property: Property, time: f64) -> KeyRef {
    KeyRef {
        node,
        property,
        time,
    }
}

fn node_keys(clip: &Clip, node: NodeId) -> Vec<KeyRef> {
    clip.channels
        .iter()
        .filter(|channel| channel.node == node)
        .flat_map(|channel| {
            channel
                .keys
                .iter()
                .map(move |frame| key(node, channel.property, frame.time))
        })
        .collect()
}

fn make_cubic(editor: &mut Editor) {
    for channel in &mut editor.project.clips[0].channels {
        if channel.node == 2 {
            channel.interpolation = Interpolation::CubicSpline;
            for key in &mut channel.keys {
                key.in_tangent = Some(vec![0.25; key.value.len()]);
                key.out_tangent = Some(vec![-0.125; key.value.len()]);
            }
        }
    }
    editor.project.validate().unwrap();
}

#[test]
fn multi_key_move_is_independent_of_channel_order_and_preserves_imported_curve_data() {
    let mut editor = editor();
    make_cubic(&mut editor);
    editor.project.clips[0].channels.reverse();
    // Imported, off-frame time must not be silently snapped to the display grid.
    for channel in &mut editor.project.clips[0].channels {
        if channel.node == 2 {
            channel.keys[1].time += 0.00037;
        }
    }
    let before = editor.project.clone();
    let selection = node_keys(&before.clips[0], 2);
    editor.time = 0.3;
    let moved = editor.move_keys(&selection, 48).unwrap();
    assert_eq!(moved.len(), 6);
    assert_eq!(editor.time, 0.3);
    assert_eq!(editor.undo_len(), 1);
    assert!((editor.project.duration - 3.00037).abs() < 1e-12);
    for (source, target) in before.clips[0]
        .channels
        .iter()
        .zip(&editor.project.clips[0].channels)
    {
        if source.node == 2 {
            assert_eq!(target.interpolation, source.interpolation);
            for (a, b) in source.keys.iter().zip(&target.keys) {
                assert_eq!(b.time, a.time + 2.);
                assert_eq!(b.value, a.value);
                assert_eq!(b.in_tangent, a.in_tangent);
                assert_eq!(b.out_tangent, a.out_tangent);
            }
        } else {
            assert_eq!(target, source);
        }
    }
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    assert_eq!(editor.time, 0.3);
    assert!(editor.redo());
    assert_eq!(
        editor.project.clips[0].key_times(Some(2)),
        vec![2., 3.00037]
    );
}

#[test]
fn moving_a_chain_into_its_selected_source_times_is_valid_but_external_collisions_are_atomic() {
    let mut editor = editor();
    let selection = node_keys(&editor.project.clips[0], 2);
    editor.move_keys(&selection, 24).unwrap();
    assert_eq!(editor.project.clips[0].key_times(Some(2)), vec![1., 2.]);
    let before = editor.project.clone();
    let history = editor.undo_len();
    let invalid = [
        key(2, Property::Translation, 1.),
        key(3, Property::Translation, 0.),
    ];
    assert!(editor.move_keys(&invalid, 24).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), history);
}

#[test]
fn duplicate_retains_source_and_rejects_any_occupied_destination_without_partial_copies() {
    let mut editor = editor();
    make_cubic(&mut editor);
    let selection = node_keys(&editor.project.clips[0], 2);
    let original = editor.project.clone();
    assert!(editor.duplicate_keys(&selection, 0).is_err());
    assert!(editor.duplicate_keys(&selection, 24).is_err());
    assert_eq!(editor.project, original);
    assert_eq!(editor.undo_len(), 0);
    let inserted = editor.duplicate_keys(&selection, 72).unwrap();
    assert_eq!(inserted.len(), 6);
    assert_eq!(editor.undo_len(), 1);
    for channel in &editor.project.clips[0].channels {
        let old = original.clips[0]
            .channels
            .iter()
            .find(|c| c.node == channel.node && c.property == channel.property)
            .unwrap();
        if channel.node == 2 {
            assert_eq!(channel.keys[..2], old.keys);
            assert_eq!(channel.keys[2].value, old.keys[0].value);
            assert_eq!(channel.keys[2].in_tangent, old.keys[0].in_tangent);
        } else {
            assert_eq!(channel, old);
        }
    }
    assert!(editor.undo());
    assert_eq!(editor.project, original);
}

#[test]
fn deletion_deduplicates_references_and_removes_only_empty_target_channels() {
    let mut editor = editor();
    let before = editor.project.clone();
    let mut selection = node_keys(&before.clips[0], 2);
    selection.push(selection[0]);
    assert_eq!(editor.delete_keys(&selection).unwrap(), 6);
    assert_eq!(editor.project.clips[0].channels.len(), 3);
    assert!(editor.project.clips[0]
        .channels
        .iter()
        .all(|channel| channel.node == 3));
    assert_eq!(editor.undo_len(), 1);
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, before);
}

#[test]
fn copied_relative_times_and_cubic_tangents_paste_into_a_new_clip_with_one_undo() {
    let mut editor = editor();
    make_cubic(&mut editor);
    for channel in &mut editor.project.clips[0].channels {
        if channel.node == 2 {
            channel.keys[0].time = 0.125;
            channel.keys[1].time = 1.00037;
        }
    }
    let source = editor.project.clips[0].clone();
    let selection = node_keys(&source, 2);
    let clipboard = editor.copy_keys(&selection).unwrap();
    assert_eq!(clipboard.len(), 6);
    assert!(!clipboard.is_empty());
    assert!((clipboard.duration() - 0.87537).abs() < 1e-12);
    assert_eq!(editor.undo_len(), 0);
    let target = editor.create_clip("Second").unwrap();
    // Copy offsets are seconds; changing FPS does not retime the copied motion.
    editor.project.fps = 30;
    let before_paste = editor.project.clone();
    let history = editor.undo_len();
    let inserted = editor.paste_keys(&clipboard, 90).unwrap();
    assert_eq!(inserted.len(), 6);
    assert_eq!(editor.undo_len(), history + 1);
    assert!((editor.project.duration - 3.87537).abs() < 1e-12);
    assert_eq!(editor.project.clips[0], source);
    for channel in &editor.project.clips[target].channels {
        let old = source
            .channels
            .iter()
            .find(|c| c.node == channel.node && c.property == channel.property)
            .unwrap();
        assert_eq!(channel.interpolation, old.interpolation);
        for (a, b) in old.keys.iter().zip(&channel.keys) {
            assert!((b.time - (a.time - 0.125 + 3.)).abs() < 1e-12);
            assert_eq!(a.value, b.value);
            assert_eq!(a.in_tangent, b.in_tangent);
            assert_eq!(a.out_tangent, b.out_tangent);
        }
    }
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, before_paste);
}

#[test]
fn paste_never_silently_changes_destination_interpolation_or_overwrites_existing_keys() {
    let mut editor = editor();
    let selection = node_keys(&editor.project.clips[0], 2);
    let linear = editor.copy_keys(&selection).unwrap();
    let original = editor.project.clone();
    assert!(editor.paste_keys(&linear, 0).is_err());
    assert_eq!(editor.project, original);
    make_cubic(&mut editor);
    let before = editor.project.clone();
    assert!(editor.paste_keys(&linear, 72).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), 0);
}

#[test]
fn stale_invalid_ambiguous_or_locked_selection_never_partially_edits_the_document() {
    let mut editor = editor();
    let valid = key(2, Property::Translation, 0.);
    for invalid in [
        key(999, Property::Translation, 0.),
        key(2, Property::Translation, 0.4),
        key(2, Property::Weights, 0.),
        key(2, Property::Translation, f64::NAN),
    ] {
        let before = editor.project.clone();
        assert!(editor.move_keys(&[valid, invalid], 2).is_err());
        assert!(editor.delete_keys(&[valid, invalid]).is_err());
        assert_eq!(editor.project, before);
        assert_eq!(editor.undo_len(), 0);
    }
    editor.project.node_mut(1).unwrap().locked = true;
    let before = editor.project.clone();
    assert!(editor.copy_keys(&[valid]).is_err());
    assert!(editor.move_keys(&[valid], 2).is_err());
    assert!(editor
        .set_key_interpolation(&[valid], KeyInterpolationPreset::Step)
        .is_err());
    assert_eq!(editor.project, before);
    editor.project.node_mut(1).unwrap().locked = false;
    let channel = &mut editor.project.clips[0].channels[0];
    let mut near = channel.keys[0].clone();
    near.time = 0.5e-9;
    channel.keys.insert(1, near);
    editor.project.validate().unwrap();
    assert!(editor.copy_keys(&[valid]).is_err());
    assert_eq!(editor.undo_len(), 0);
}

#[test]
fn paste_rechecks_target_existence_and_ancestor_locks() {
    let mut editor = editor();
    let clipboard = editor
        .copy_keys(&node_keys(&editor.project.clips[0], 2))
        .unwrap();
    editor.project.node_mut(1).unwrap().locked = true;
    let before = editor.project.clone();
    assert!(editor.paste_keys(&clipboard, 72).is_err());
    assert_eq!(editor.project, before);
    editor.project.node_mut(1).unwrap().locked = false;
    editor.project.nodes.retain(|node| node.id != 2);
    editor.project.clips[0]
        .channels
        .retain(|channel| channel.node != 2);
    let before = editor.project.clone();
    assert!(editor.paste_keys(&clipboard, 72).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), 0);
}

#[test]
fn interpolation_presets_affect_only_referenced_channels_and_undo_restores_exact_tangents() {
    let mut editor = editor();
    make_cubic(&mut editor);
    let original = editor.project.clone();
    let selection = [key(2, Property::Translation, 0.)];
    assert_eq!(
        editor
            .set_key_interpolation(&selection, KeyInterpolationPreset::EaseInOut)
            .unwrap(),
        1
    );
    let eased = &editor.project.clips[0].channels[0];
    assert_eq!(eased.interpolation, Interpolation::CubicSpline);
    assert_eq!(eased.sample(0.25).unwrap(), vec![0., 0.15625, 0.]);
    assert_eq!(
        editor.project.clips[0].channels[1..],
        original.clips[0].channels[1..]
    );
    assert_eq!(editor.undo_len(), 1);
    assert!(editor.undo());
    assert_eq!(editor.project, original);
    editor
        .set_key_interpolation(&selection, KeyInterpolationPreset::Step)
        .unwrap();
    assert_eq!(
        editor.project.clips[0].channels[0].sample(0.75).unwrap(),
        vec![0., 0., 0.]
    );
    editor
        .set_key_interpolation(&selection, KeyInterpolationPreset::Linear)
        .unwrap();
    assert_eq!(
        editor.project.clips[0].channels[0].sample(0.25).unwrap(),
        vec![0., 0.25, 0.]
    );
}

#[test]
fn explicit_ease_keeps_quaternion_motion_on_the_shortest_hemisphere() {
    let mut editor = editor();
    let channel = editor.project.clips[0]
        .channels
        .iter_mut()
        .find(|channel| channel.node == 2 && channel.property == Property::Rotation)
        .unwrap();
    channel.keys[1].value = (-Quat::from_rotation_y(1.3)).to_array().to_vec();
    let before = editor.project.clone();
    editor
        .set_key_interpolation(
            &[key(2, Property::Rotation, 0.)],
            KeyInterpolationPreset::EaseInOut,
        )
        .unwrap();
    let channel = editor.project.clips[0]
        .channels
        .iter()
        .find(|channel| channel.node == 2 && channel.property == Property::Rotation)
        .unwrap();
    let midpoint = Quat::from_array(channel.sample(0.5).unwrap().try_into().unwrap());
    assert!(midpoint.dot(Quat::from_rotation_y(0.65)).abs() > 0.99999);
    for time in [0., 0.1, 0.4, 0.5, 0.6, 0.9, 1.] {
        let value = Quat::from_array(channel.sample(time).unwrap().try_into().unwrap());
        assert!(value.is_finite());
        assert!((value.length() - 1.).abs() < 1e-6);
    }
    assert!(editor.undo());
    assert_eq!(editor.project, before);
}

#[test]
fn pure_drag_previews_commit_once_or_cancel_without_cumulative_drift() {
    let mut editor = editor();
    let before = editor.project.clone();
    let start_clip = before.clips[0].clone();
    let selection = node_keys(&start_clip, 2);
    editor.copy_keys(&selection).unwrap();
    for cancel in [true, false] {
        editor.begin_edit("Переместить ключи");
        for frame_delta in [1, 5, 3] {
            let (preview, _) = start_clip
                .retimed_keys(&selection, frame_delta as f64 / 24., false)
                .unwrap();
            editor
                .transact("Предпросмотр ключей", |project| {
                    project.clips[0] = preview
                })
                .unwrap();
            assert_eq!(editor.undo_len(), 0);
        }
        if cancel {
            editor.cancel_edit();
            assert_eq!(editor.project, before);
        } else {
            editor.commit_edit().unwrap();
            assert_eq!(editor.undo_len(), 1);
            assert_eq!(
                editor.project.clips[0].key_times(Some(2)),
                vec![0.125, 1.125]
            );
            assert!(editor.undo());
            assert_eq!(editor.project, before);
        }
    }
}

#[test]
fn no_op_commands_do_not_pollute_history_and_uncommitted_pose_blocks_key_changes() {
    let mut editor = editor();
    // A clip may have a longer declared duration than the scene's play range.
    // Merely clicking a selected marker must not extend that scene duration.
    editor.project.clips[0].duration = 20.;
    let original = editor.project.clone();
    let selection = node_keys(&editor.project.clips[0], 2);
    editor.move_keys(&selection, 0).unwrap();
    assert_eq!(
        editor
            .set_key_interpolation(&selection, KeyInterpolationPreset::Linear)
            .unwrap(),
        0
    );
    assert_eq!(editor.undo_len(), 0);
    assert_eq!(editor.project, original);
    editor
        .set_transform(2, Transform::from_translation(Vec3::X))
        .unwrap();
    let before = editor.project.clone();
    let history = editor.undo_len();
    assert!(matches!(
        editor.move_keys(&selection, 1),
        Err(CoreError::UncommittedPose)
    ));
    assert!(matches!(
        editor.copy_keys(&selection),
        Err(CoreError::UncommittedPose)
    ));
    assert_eq!(editor.project, before);
    assert_eq!(editor.evaluated().unwrap().local[&2].translation, Vec3::X);
    assert_eq!(editor.undo_len(), history);
}

#[test]
fn preview_rejects_nonfinite_negative_and_precision_collapsed_destinations() {
    let editor = editor();
    let clip = &editor.project.clips[0];
    let selection = node_keys(clip, 2);
    for delta in [f64::NAN, f64::INFINITY, -0.001, 1e30] {
        assert!(clip.retimed_keys(&selection, delta, false).is_err());
    }
    assert!(clip.retimed_keys(&[], 1., false).is_err());
}

#[test]
fn cubic_morph_keys_preserve_all_six_components_and_reject_changed_target_arity_atomically() {
    let mut editor = editor();
    let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]]
        .into_iter()
        .map(|position| Vertex {
            position,
            ..Vertex::default()
        })
        .collect();
    editor.project.meshes.push(Mesh {
        id: 1000,
        name: "Six expression targets".into(),
        vertices,
        indices: vec![0, 1, 2].into(),
        material: None,
        topology: None,
        morph_targets: (0..6)
            .map(|index| MorphTarget {
                name: format!("Expression {index}"),
                positions: vec![[0., 0.01 * (index + 1) as f32, 0.]; 3].into(),
                normals: vec![].into(),
            })
            .collect(),
    });
    let node = editor.project.node_mut(2).unwrap();
    node.mesh = Some(1000);
    node.morph_weights = vec![0.; 6];
    let source_channel = Channel {
        node: 2,
        property: Property::Weights,
        interpolation: Interpolation::CubicSpline,
        keys: vec![
            Keyframe {
                time: 0.2,
                value: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
                in_tangent: Some(vec![-0.05, -0.1, -0.15, -0.2, -0.25, -0.3]),
                out_tangent: Some(vec![0.11, 0.22, 0.33, 0.44, 0.55, 0.66]),
            },
            Keyframe {
                time: 0.8,
                value: vec![0.7, 0.6, 0.5, 0.4, 0.3, 0.2],
                in_tangent: Some(vec![0.15, 0.25, 0.35, 0.45, 0.55, 0.65]),
                out_tangent: Some(vec![-0.12, -0.24, -0.36, -0.48, -0.6, -0.72]),
            },
        ],
    };
    editor.project.clips[0]
        .channels
        .push(source_channel.clone());
    editor.project.validate().unwrap();
    let references = [
        key(2, Property::Weights, 0.2),
        key(2, Property::Weights, 0.8),
    ];
    let moved = editor.move_keys(&references, 24).unwrap();
    let shifted_channel = editor.project.clips[0]
        .channels
        .iter()
        .find(|channel| channel.node == 2 && channel.property == Property::Weights)
        .unwrap();
    for (source, shifted) in source_channel.keys.iter().zip(&shifted_channel.keys) {
        assert_eq!(shifted.time, source.time + 1.);
        assert_eq!(shifted.value, source.value);
        assert_eq!(shifted.in_tangent, source.in_tangent);
        assert_eq!(shifted.out_tangent, source.out_tangent);
    }
    let clipboard = editor.copy_keys(&moved).unwrap();
    assert_eq!(clipboard.len(), 2);
    assert!((clipboard.duration() - 0.6).abs() < 1e-12);
    let target = editor.create_clip("Morph copy").unwrap();
    let before_paste = editor.project.clone();
    let history = editor.undo_len();
    let pasted = editor.paste_keys(&clipboard, 96).unwrap();
    assert_eq!(pasted.len(), 2);
    assert_eq!(editor.undo_len(), history + 1);
    let copied_channel = &editor.project.clips[target].channels[0];
    assert_eq!(copied_channel.property, Property::Weights);
    assert_eq!(copied_channel.interpolation, Interpolation::CubicSpline);
    for (source, copied) in source_channel.keys.iter().zip(&copied_channel.keys) {
        assert!((copied.time - (4. + source.time - 0.2)).abs() < 1e-12);
        assert_eq!(copied.value.len(), 6);
        assert_eq!(copied.value, source.value);
        assert_eq!(copied.in_tangent, source.in_tangent);
        assert_eq!(copied.out_tangent, source.out_tangent);
    }
    let original_midpoint = source_channel.sample(0.5).unwrap();
    let copied_midpoint = copied_channel.sample(4.3).unwrap();
    assert_eq!(copied_midpoint.len(), 6);
    for (source, copied) in original_midpoint.iter().zip(copied_midpoint) {
        assert!((source - copied).abs() < 1e-6);
    }
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, before_paste);

    // The destination is otherwise a valid project. Its mesh now has five
    // targets, while the detached clipboard still contains six-component keys.
    let mut changed_project = editor.project.clone();
    for clip in &mut changed_project.clips {
        clip.channels
            .retain(|channel| !(channel.node == 2 && channel.property == Property::Weights));
    }
    changed_project.meshes[0].morph_targets.truncate(5);
    changed_project
        .node_mut(2)
        .unwrap()
        .morph_weights
        .truncate(5);
    changed_project.validate().unwrap();
    let mut changed_editor = Editor::new(changed_project);
    changed_editor.active_clip = Some(target);
    let before_rejected_paste = changed_editor.project.clone();
    assert!(changed_editor.paste_keys(&clipboard, 96).is_err());
    assert_eq!(changed_editor.project, before_rejected_paste);
    assert_eq!(changed_editor.undo_len(), 0);
}
