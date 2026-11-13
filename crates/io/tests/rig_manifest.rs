use poseforge_core::{Node, Project};
use poseforge_io::parse_rig_manifest;

const SIMPLE: &str = "Armature: Test\nBones: 2\n\nroot\tparent: -\tpath: root\nhand.L\tparent: root\tpath: root > hand.L\n";

#[test]
fn parses_actual_grace_export_without_changing_names() {
    let text = include_str!("../../../examples/grace-rig.txt");
    let rig = parse_rig_manifest(text).unwrap();
    assert_eq!(rig.armature, "Grace_rig");
    assert_eq!(rig.declared_count, 623);
    assert_eq!(rig.bones.len(), 623);
    assert_eq!(rig.bones[0].name, "c_pos");
    assert_eq!(rig.bones[0].parent, None);
    let master = rig
        .bones
        .iter()
        .find(|bone| bone.name == "c_root_master.x")
        .unwrap();
    assert_eq!(master.parent.as_deref(), Some("c_traj"));
    assert_eq!(master.path, vec!["c_pos", "c_traj", "c_root_master.x"]);
    assert!(rig.bones.iter().any(|bone| bone.name == "Hair.003.L.002"));
}

#[test]
fn handles_windows_lines_bom_unicode_and_child_before_parent() {
    let text = "\u{feff}Armature: Персонаж\r\nBones: 2\r\n\r\nКисть\tparent: Таз\tpath: Таз > Кисть\r\nТаз\tparent: -\tpath: Таз\r\n";
    let rig = parse_rig_manifest(text).unwrap();
    assert_eq!(rig.armature, "Персонаж");
    assert_eq!(rig.bones[0].parent.as_deref(), Some("Таз"));
}

#[test]
fn rejects_missing_and_malformed_headers() {
    for text in [
        "",
        "Armature: Test",
        "Armature:\nBones: 0",
        "Armature: Test\nBones: -1",
        "Armature: Test\nBones: 100001",
    ] {
        assert!(parse_rig_manifest(text).is_err(), "accepted {text:?}");
    }
}

#[test]
fn rejects_count_mismatch_duplicates_and_missing_parents() {
    assert!(parse_rig_manifest(&SIMPLE.replace("Bones: 2", "Bones: 3"))
        .unwrap_err()
        .to_string()
        .contains("прочитано 2"));
    let duplicate =
        "Armature: X\nBones: 2\nroot\tparent: -\tpath: root\nroot\tparent: -\tpath: root";
    assert!(parse_rig_manifest(duplicate)
        .unwrap_err()
        .to_string()
        .contains("повтор имени"));
    assert!(
        parse_rig_manifest(&SIMPLE.replace("parent: root", "parent: missing"))
            .unwrap_err()
            .to_string()
            .contains("не найден")
    );
}

#[test]
fn rejects_cycles_without_following_path_recursively() {
    let cycle = "Armature: X\nBones: 2\na\tparent: b\tpath: b > a\nb\tparent: a\tpath: a > b";
    assert!(parse_rig_manifest(cycle)
        .unwrap_err()
        .to_string()
        .contains("цикл"));
    let self_cycle = "Armature: X\nBones: 1\na\tparent: a\tpath: a";
    assert!(parse_rig_manifest(self_cycle)
        .unwrap_err()
        .to_string()
        .contains("цикл"));
}

#[test]
fn rejects_paths_that_disagree_with_parent_chain() {
    for text in [
        SIMPLE.replace("root > hand.L", "hand.L"),
        SIMPLE.replace("root > hand.L", "root > unknown > hand.L"),
        SIMPLE.replace("root > hand.L", "root"),
    ] {
        assert!(parse_rig_manifest(&text)
            .unwrap_err()
            .to_string()
            .contains("path"));
    }
}

#[test]
fn comparison_is_read_only_and_keeps_root_container_compatible() {
    let rig = parse_rig_manifest(SIMPLE).unwrap();
    let container = Node::new(1, "Test");
    let mut root = Node::new(2, "root");
    root.parent = Some(1);
    let mut hand = Node::new(3, "hand.L");
    hand.parent = Some(2);
    let project = Project {
        nodes: vec![container, root, hand],
        ..Project::default()
    };
    let before = project.clone();
    let report = rig.compare(&project);
    assert_eq!(project, before);
    assert_eq!(
        report.matched,
        vec![("root".to_owned(), 2), ("hand.L".to_owned(), 3)]
    );
    assert!(report.missing.is_empty());
    assert!(report.ambiguous.is_empty());
    assert!(report.hierarchy_mismatches.is_empty());
}

#[test]
fn comparison_reports_missing_ambiguous_and_changed_hierarchy() {
    let rig = parse_rig_manifest(SIMPLE).unwrap();
    let project = Project {
        nodes: vec![Node::new(1, "hand.L")],
        ..Project::default()
    };
    let report = rig.compare(&project);
    assert_eq!(report.missing, vec!["root"]);
    assert_eq!(report.matched.len(), 1);
    assert_eq!(report.hierarchy_mismatches.len(), 1);
    let duplicate_project = Project {
        nodes: vec![Node::new(1, "root"), Node::new(2, "root")],
        ..Project::default()
    };
    let report = rig.compare(&duplicate_project);
    assert!(report.matched.is_empty());
    assert_eq!(report.ambiguous, vec!["root"]);
    assert_eq!(report.missing, vec!["hand.L"]);
}

#[test]
fn comparison_uses_exact_names_including_side_suffix() {
    let rig = parse_rig_manifest(SIMPLE).unwrap();
    let project = Project {
        nodes: vec![Node::new(1, "Root"), Node::new(2, "hand.R")],
        ..Project::default()
    };
    let report = rig.compare(&project);
    assert!(report.matched.is_empty());
    assert_eq!(report.missing.len(), 2);
}
