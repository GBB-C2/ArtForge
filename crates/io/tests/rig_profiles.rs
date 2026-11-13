use poseforge_core::{BoneRole, Editor, Node, Project, RigProfile, Vec3, SCHEMA_VERSION};
use poseforge_io::{load_project, merge_project, save_project};
use std::collections::{BTreeMap, BTreeSet};

fn character() -> Project {
    let mut project = Project {
        title: "Персонаж с сохранённым ригом".into(),
        ..Project::default()
    };
    project.nodes.push(Node::new(1, "Персонаж"));
    for (id, name, parent) in [
        (2, "Hips", 1),
        (3, "Spine", 2),
        (4, "Head", 3),
        (5, "LeftArm", 3),
        (6, "LeftForeArm", 5),
        (7, "LeftHand", 6),
    ] {
        let mut joint = Node::new(id, name);
        joint.parent = Some(parent);
        joint.is_joint = true;
        joint.transform.translation = Vec3::Y;
        project.nodes.push(joint);
    }
    project.rig_profiles.push(RigProfile {
        root: Some(1),
        assignments: BTreeMap::from([
            (BoneRole::Pelvis, 2),
            (BoneRole::Head, 4),
            (BoneRole::LeftHand, 7),
        ]),
    });
    project.validate().unwrap();
    project
}

#[test]
fn rig_profile_roundtrip_keeps_stable_ids_and_unicode_project() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("персонаж.poseforge");
    let project = character();
    save_project(&project, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), project);
}

#[test]
fn old_schema_one_projects_without_profiles_still_open() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("legacy.poseforge");
    let mut data = serde_json::to_value(character()).unwrap();
    data.as_object_mut().unwrap().remove("rig_profiles");
    data["version"] = serde_json::json!(1);
    std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
    let project = load_project(&path).unwrap();
    assert_eq!(project.version, SCHEMA_VERSION);
    assert!(project.rig_profiles.is_empty());
    assert_eq!(project.nodes.len(), 7);
}

#[test]
fn actual_version_one_fixture_migrates_without_changing_its_author_data() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project_v1.poseforge");
    let original_bytes = std::fs::read(&source).unwrap();
    let mut expected: Project = serde_json::from_slice(&original_bytes).unwrap();
    assert_eq!(expected.version, 1, "Keep this fixture in the old format");
    assert!(expected.rig_profiles.is_empty());
    expected.version = SCHEMA_VERSION;

    let migrated = load_project(&source).unwrap();
    assert_eq!(migrated, expected);
    assert_eq!(std::fs::read(&source).unwrap(), original_bytes);
    let saved = directory.path().join("migrated.poseforge");
    save_project(&migrated, &saved).unwrap();
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&saved).unwrap()).unwrap();
    assert_eq!(data["version"], SCHEMA_VERSION);
    assert_eq!(load_project(saved).unwrap(), migrated);
}

#[test]
fn unknown_and_invalid_legacy_versions_are_rejected_without_rewriting() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("unsupported.poseforge");
    for version in [0, SCHEMA_VERSION + 1, 999] {
        let mut data = serde_json::to_value(character()).unwrap();
        data["version"] = serde_json::json!(version);
        let bytes = serde_json::to_vec(&data).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(load_project(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    // Migration must not bypass the ordinary validation of legacy content.
    let mut data = serde_json::to_value(character()).unwrap();
    data["version"] = serde_json::json!(1);
    data["fps"] = serde_json::json!(0);
    let bytes = serde_json::to_vec(&data).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(load_project(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn importing_the_same_rig_twice_remaps_every_profile_reference() {
    let incoming = character();
    let mut destination = Project::default();
    let first_roots = merge_project(&mut destination, incoming.clone()).unwrap();
    let second_roots = merge_project(&mut destination, incoming).unwrap();
    assert_eq!(destination.rig_profiles.len(), 2);
    let first = &destination.rig_profiles[0];
    let second = &destination.rig_profiles[1];
    assert_eq!(first.root, first_roots.first().copied());
    assert_eq!(second.root, second_roots.first().copied());
    assert_ne!(first.root, second.root);
    let first_ids: BTreeSet<_> = first.assignments.values().copied().collect();
    let second_ids: BTreeSet<_> = second.assignments.values().copied().collect();
    assert!(first_ids.is_disjoint(&second_ids));
    for role in first.assignments.keys() {
        let a = destination.node(first.assignments[role]).unwrap();
        let b = destination.node(second.assignments[role]).unwrap();
        assert_eq!(a.name, b.name);
        assert_ne!(a.id, b.id);
    }
    destination.validate().unwrap();
}

#[test]
fn duplicating_a_character_copies_its_profile_but_not_external_references() {
    let mut project = character();
    let original = project.rig_profiles[0].clone();
    let copied_root = project.duplicate_subtree(1).unwrap();
    assert_eq!(project.rig_profiles.len(), 2);
    assert_eq!(project.rig_profiles[0], original);
    let copied = &project.rig_profiles[1];
    assert_eq!(copied.root, Some(copied_root));
    assert!(copied
        .assignments
        .values()
        .all(|id| !original.assignments.values().any(|old| old == id)));
    project.validate().unwrap();

    // Copying only a hand must not create a second profile whose pelvis/head
    // still belong to the original character.
    project.duplicate_subtree(7).unwrap();
    assert_eq!(project.rig_profiles.len(), 2);
    project.validate().unwrap();
}

#[test]
fn invalid_profile_cannot_replace_saved_file_or_merge_destination() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("character.poseforge");
    let good = character();
    save_project(&good, &path).unwrap();
    let before_bytes = std::fs::read(&path).unwrap();
    let mut invalid = good.clone();
    invalid.rig_profiles[0]
        .assignments
        .insert(BoneRole::Head, 999);
    assert!(save_project(&invalid, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before_bytes);

    let mut destination = good.clone();
    assert!(merge_project(&mut destination, invalid).is_err());
    assert_eq!(destination, good);
}

#[test]
fn deleting_joints_prunes_profiles_and_undo_restores_them() {
    let mut editor = Editor::new(character());
    let original = editor.project.clone();
    editor.selected = Some(7);
    editor.delete_selected().unwrap();
    assert_eq!(editor.project.rig_profiles.len(), 1);
    let profile = &editor.project.rig_profiles[0];
    assert!(!profile.assignments.contains_key(&BoneRole::LeftHand));
    assert_eq!(profile.assignments[&BoneRole::Pelvis], 2);
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, original);

    editor.selected = Some(1);
    editor.delete_selected().unwrap();
    assert!(editor.project.rig_profiles.is_empty());
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, original);
}

#[test]
fn duplicate_profile_roots_are_rejected() {
    let mut project = poseforge_core::Project::default();
    project
        .nodes
        .push(poseforge_core::Node::new(1, "Character"));
    let profile = poseforge_core::RigProfile {
        root: Some(1),
        assignments: Default::default(),
    };
    project.rig_profiles = vec![profile.clone(), profile];
    assert!(project.validate().is_err());
}

#[test]
fn unscoped_draft_profile_cannot_be_saved_or_merged() {
    let mut project = character();
    project.rig_profiles[0].root = None;
    // Detection may hold an unscoped draft, but persisted mappings must name a
    // character root so copying/importing cannot mix independent characters.
    project.rig_profiles[0].validate(&project).unwrap();
    assert!(project.validate().is_err());

    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("unscoped.poseforge");
    assert!(save_project(&project, &path).is_err());
    assert!(!path.exists());

    let mut destination = character();
    let before = destination.clone();
    assert!(merge_project(&mut destination, project).is_err());
    assert_eq!(destination, before);
}
