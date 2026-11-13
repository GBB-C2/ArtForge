use poseforge_core::*;

fn animated_editor() -> Editor {
    let mut project = Project::default();
    let root = Node::new(1, "Character");
    let mut left = Node::new(2, "Hand.L");
    left.parent = Some(root.id);
    left.is_joint = true;
    let mut right = Node::new(3, "Hand.R");
    right.parent = Some(root.id);
    right.is_joint = true;
    project.nodes = vec![root, left, right];
    let mut clip = Clip::new("Base", 4.);
    for node in [2, 3] {
        clip.key_transform(node, 0., Transform::default());
        clip.key_transform(node, 1., Transform::from_translation(Vec3::Y));
    }
    project.clips.push(clip);
    project.validate().unwrap();
    let mut editor = Editor::new(project);
    editor.mode = EditorMode::Animation;
    editor
}

#[test]
fn creating_clip_undo_redo_restores_active_clip_and_cursor() {
    let mut editor = animated_editor();
    editor.set_time(1.25).unwrap();
    let original = editor.project.clone();
    let index = editor.create_clip("  Walk  ").unwrap();
    assert_eq!(index, 1);
    assert_eq!(editor.active_clip, Some(index));
    assert_eq!(editor.time, 0.);
    assert_eq!(editor.project.clips[index].name, "Walk");
    assert!(editor.project.clips[index].channels.is_empty());
    assert_eq!(editor.undo_len(), 1);
    assert!(editor.undo());
    assert_eq!(editor.project, original);
    assert_eq!(editor.active_clip, Some(0));
    assert_eq!(editor.time, 1.25);
    assert!(editor.redo());
    assert_eq!(editor.active_clip, Some(1));
    assert_eq!(editor.time, 0.);
    editor.project.validate().unwrap();
}

#[test]
fn duplicate_preserves_cubic_tangents_and_rename_is_reversible() {
    let mut editor = animated_editor();
    let channel = &mut editor.project.clips[0].channels[0];
    channel.interpolation = Interpolation::CubicSpline;
    for key in &mut channel.keys {
        key.in_tangent = Some(vec![0.25; key.value.len()]);
        key.out_tangent = Some(vec![-0.75; key.value.len()]);
    }
    let source = editor.project.clips[0].clone();
    editor.set_time(0.5).unwrap();
    let index = editor.duplicate_active_clip("Variation").unwrap();
    assert_eq!(editor.project.clips[index].channels, source.channels);
    assert_eq!(editor.project.clips[index].duration, source.duration);
    assert_eq!(editor.time, 0.5);
    editor.rename_active_clip("Wave").unwrap();
    assert_eq!(editor.project.clips[index].name, "Wave");
    assert!(editor.undo());
    assert_eq!(editor.project.clips[index].name, "Variation");
    assert!(editor.undo());
    assert_eq!(editor.active_clip, Some(0));
    assert_eq!(editor.project.clips.len(), 1);
    assert_eq!(editor.project.clips[0], source);
}

#[test]
fn clip_commands_reject_staged_edits_and_ambiguous_names_without_data_loss() {
    let mut editor = animated_editor();
    let pose = Transform::from_translation(Vec3::new(0.2, 0.3, 0.4));
    editor.set_transform(2, pose).unwrap();
    let project = editor.project.clone();
    let undo_len = editor.undo_len();
    assert!(matches!(
        editor.create_clip("Next"),
        Err(CoreError::UncommittedPose)
    ));
    assert!(matches!(
        editor.duplicate_active_clip("Copy"),
        Err(CoreError::UncommittedPose)
    ));
    assert!(matches!(
        editor.rename_active_clip("Rename"),
        Err(CoreError::UncommittedPose)
    ));
    assert_eq!(editor.project, project);
    assert_eq!(editor.evaluated().unwrap().local[&2], pose);
    assert_eq!(editor.undo_len(), undo_len);
    editor.resolve_staged(StagedResolution::Discard).unwrap();
    let project = editor.project.clone();
    let undo_len = editor.undo_len();
    for name in ["", "  ", "BASE", "Line\nbreak"] {
        assert!(editor.create_clip(name).is_err());
    }
    assert_eq!(editor.project, project);
    assert_eq!(editor.undo_len(), undo_len);
}

#[test]
fn scoped_keying_uses_visible_pose_and_preserves_unrelated_staging() {
    let mut editor = animated_editor();
    editor.set_time(0.5).unwrap();
    let left = Transform::from_translation(Vec3::new(1., 2., 3.));
    let right = Transform::from_translation(Vec3::new(-1., 3., 2.));
    editor.set_transform(2, left).unwrap();
    editor.set_transform(3, right).unwrap();
    let before = editor.project.clone();
    let history = editor.undo_len();
    editor.key_nodes(&[2, 2]).unwrap();
    assert_eq!(editor.undo_len(), history + 1);
    assert!(editor.has_staged());
    let clip_pose = editor.project.evaluated(Some(0), 0.5).unwrap();
    assert_eq!(clip_pose.local[&2], left);
    assert_ne!(clip_pose.local[&3], right);
    assert_eq!(editor.evaluated().unwrap().local[&3], right);
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    assert_eq!(editor.evaluated().unwrap().local[&2], left);
    assert_eq!(editor.evaluated().unwrap().local[&3], right);
    editor.key_nodes(&[2, 3]).unwrap();
    assert!(!editor.has_staged());
    assert_eq!(
        editor.project.evaluated(Some(0), 0.5).unwrap().local[&3],
        right
    );
    editor.project.validate().unwrap();
}

#[test]
fn batch_keying_rejects_invalid_or_locked_nodes_before_mutation() {
    let mut editor = animated_editor();
    let before = editor.project.clone();
    assert!(editor.key_nodes(&[]).is_err());
    assert!(editor.key_nodes(&[2, 999]).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), 0);
    editor.project.node_mut(3).unwrap().locked = true;
    let before = editor.project.clone();
    assert!(editor.key_nodes(&[2, 3]).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), 0);
}

#[test]
fn conflicting_key_shift_is_atomic_across_nodes_and_channels() {
    let mut editor = animated_editor();
    // Only one channel collides; no earlier channel is allowed to move first.
    editor.project.clips[0].channels.retain(|c| c.node == 2);
    let transform = Transform::from_translation(Vec3::X);
    editor.project.clips[0].key_transform(3, 0., transform);
    let before = editor.project.clone();
    assert!(editor.shift_current_keys(24, &[3, 2]).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.time, 0.);
    assert_eq!(editor.undo_len(), 0);
}

#[test]
fn key_shift_uses_fps_preserves_values_and_restores_cursor_with_undo() {
    let mut editor = animated_editor();
    let before = editor.project.clone();
    let moved = editor.shift_current_keys(6, &[2, 3]).unwrap();
    assert_eq!(moved, 6);
    assert_eq!(editor.time, 0.25);
    assert_eq!(editor.undo_len(), 1);
    for (old, new) in before.clips[0]
        .channels
        .iter()
        .zip(&editor.project.clips[0].channels)
    {
        assert_eq!(new.keys[0].time, 0.25);
        assert_eq!(new.keys[0].value, old.keys[0].value);
        assert_eq!(new.keys[1], old.keys[1]);
    }
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    assert_eq!(editor.time, 0.);
    assert!(editor.redo());
    assert_eq!(editor.time, 0.25);
    editor.project.validate().unwrap();
}

#[test]
fn key_shift_extends_duration_and_rejects_negative_destinations() {
    let mut editor = animated_editor();
    editor.set_time(1.).unwrap();
    assert_eq!(editor.shift_current_keys(120, &[2]).unwrap(), 3);
    assert_eq!(editor.time, 6.);
    assert_eq!(editor.project.duration, 6.);
    assert_eq!(editor.project.clips[0].duration, 6.);
    let before = editor.project.clone();
    let undo_len = editor.undo_len();
    assert!(editor.shift_current_keys(-145, &[2]).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), undo_len);
    editor.project.validate().unwrap();
}

#[test]
fn key_deletion_is_scoped_removes_empty_channels_and_is_undoable() {
    let mut editor = animated_editor();
    editor.set_time(0.).unwrap();
    assert_eq!(editor.delete_current_keys(&[2]).unwrap(), 3);
    editor.set_time(1.).unwrap();
    let before = editor.project.clone();
    assert_eq!(editor.delete_current_keys(&[2]).unwrap(), 3);
    assert!(editor.project.clips[0].channels.iter().all(|c| c.node == 3));
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    let history = editor.undo_len();
    editor.set_time(0.5).unwrap();
    assert_eq!(editor.delete_current_keys(&[2]).unwrap(), 0);
    assert_eq!(editor.undo_len(), history);
}

#[test]
fn frame_navigation_keeps_seconds_and_rejects_staged_time_changes() {
    let mut editor = animated_editor();
    editor.set_frame(12).unwrap();
    assert_eq!(editor.time, 0.5);
    let pose = editor.evaluated().unwrap().local[&2];
    editor.set_transform(2, pose).unwrap();
    assert!(matches!(
        editor.set_frame(13),
        Err(CoreError::UncommittedPose)
    ));
    assert_eq!(editor.time, 0.5);
}
