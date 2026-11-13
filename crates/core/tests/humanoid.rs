use poseforge_core::*;
use std::collections::BTreeMap;

fn joint(id: u64, name: &str, parent: u64) -> Node {
    let mut node = Node::new(id, name);
    node.is_joint = true;
    node.parent = Some(parent);
    node
}

#[allow(clippy::field_reassign_with_default)]
fn simple_character() -> Project {
    let mut project = Project::default();
    project.nodes = vec![
        Node::new(1, "Character"),
        joint(2, "mixamorig:Hips", 1),
        joint(3, "mixamorig:Spine", 2),
        joint(4, "mixamorig:Spine2", 3),
        joint(5, "mixamorig:Neck", 4),
        joint(6, "mixamorig:Head", 5),
        joint(7, "mixamorig:LeftArm", 4),
        joint(8, "mixamorig:LeftForeArm", 7),
        joint(9, "mixamorig:LeftHand", 8),
        joint(10, "mixamorig:RightArm", 4),
        joint(11, "mixamorig:RightForeArm", 10),
        joint(12, "mixamorig:RightHand", 11),
        joint(13, "mixamorig:LeftUpLeg", 2),
        joint(14, "mixamorig:LeftLeg", 13),
        joint(15, "mixamorig:LeftFoot", 14),
        joint(16, "mixamorig:RightUpLeg", 2),
        joint(17, "mixamorig:RightLeg", 16),
        joint(18, "mixamorig:RightFoot", 17),
    ];
    project
}

#[test]
fn mixamo_roles_use_stable_ids_and_do_not_change_author_data() {
    let project = simple_character();
    let before = project.clone();
    let report = detect_humanoid(&project, None);
    assert_eq!(report.root, Some(1));
    assert_eq!(report.resolved.assignments.len(), 17);
    assert_eq!(report.resolved.assignments[&BoneRole::Pelvis], 2);
    assert_eq!(
        report.resolved.chain(BoneRole::LeftHand, &project).unwrap(),
        [7, 8, 9]
    );
    assert_eq!(
        report
            .resolved
            .chain(BoneRole::RightFoot, &project)
            .unwrap(),
        [16, 17, 18]
    );
    assert_eq!(report.resolved.mirror_pairs().len(), 6);
    assert_eq!(project, before);
}

#[test]
fn russian_demo_is_recognized_as_one_character() {
    let project = Project::demo();
    let report = detect_humanoid(&project, None);
    assert_eq!(character_roots(&project), vec![1]);
    assert_eq!(report.resolved.assignments.len(), 17);
    assert_eq!(report.resolved.assignments[&BoneRole::Head], 6);
    assert_eq!(
        report.resolved.chain(BoneRole::LeftFoot, &project).unwrap(),
        [13, 14, 15]
    );
}

#[test]
fn two_characters_under_one_scene_are_never_cross_matched() {
    let first = simple_character();
    let mut project = first.clone();
    let mut scene = Node::new(1000, "Scene");
    scene.parent = None;
    project.nodes.push(scene);
    project.node_mut(1).unwrap().parent = Some(1000);
    for node in first.nodes {
        let mut node = node;
        node.id += 100;
        node.parent = node.parent.map(|id| id + 100).or(Some(1000));
        project.nodes.push(node);
    }
    assert_eq!(character_roots(&project), vec![1, 101]);
    assert!(detect_humanoid(&project, None)
        .resolved
        .assignments
        .is_empty());
    assert!(detect_humanoid(&project, Some(1000))
        .resolved
        .assignments
        .is_empty());
    let second = detect_humanoid(&project, Some(101));
    assert_eq!(second.resolved.assignments.len(), 17);
    assert!(second
        .resolved
        .assignments
        .values()
        .all(|id| *id > 100 && *id < 1000));
}

#[test]
fn ambiguous_aliases_are_listed_without_arbitrary_selection() {
    let mut project = simple_character();
    project.nodes.push(joint(50, "arm.l", 4));
    let report = detect_humanoid(&project, Some(1));
    assert!(!report
        .resolved
        .assignments
        .contains_key(&BoneRole::LeftUpperArm));
    assert_eq!(report.candidates[&BoneRole::LeftUpperArm].len(), 2);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("Несколько подходящих")));
    assert_eq!(report.resolved.assignments[&BoneRole::RightUpperArm], 10);
}

#[test]
fn helper_candidates_do_not_replace_deforming_bones() {
    let mut project = simple_character();
    project.nodes.extend([
        joint(50, "c_hand_ik.l", 1),
        joint(51, "hand_ref.l", 1),
        joint(52, "hand_twist.l", 1),
        joint(53, "MCH-hand.L", 1),
    ]);
    let report = detect_humanoid(&project, Some(1));
    assert_eq!(report.resolved.assignments[&BoneRole::LeftHand], 9);
    let candidates = &report.candidates[&BoneRole::LeftHand];
    assert_eq!(candidates.len(), 5);
    assert_eq!(
        candidates
            .iter()
            .filter(|c| c.confidence == BoneConfidence::Ignored)
            .count(),
        4
    );
    project.nodes.retain(|n| n.id != 9);
    let report = detect_humanoid(&project, Some(1));
    assert!(!report
        .resolved
        .assignments
        .contains_key(&BoneRole::LeftHand));
}

fn arp_character() -> Project {
    let mut p = simple_character();
    let names = [
        "root.x",
        "spine_01.x",
        "spine_02.x",
        "neck.x",
        "head.x",
        "arm_stretch.l",
        "forearm_stretch.l",
        "hand.l",
        "arm_stretch.r",
        "forearm_stretch.r",
        "hand.r",
        "thigh_stretch.l",
        "leg_stretch.l",
        "foot.l",
        "thigh_stretch.r",
        "leg_stretch.r",
        "foot.r",
    ];
    for (node, name) in p.nodes.iter_mut().skip(1).zip(names) {
        node.name = format!("Character:Rig|{name}");
    }
    p
}

#[test]
fn auto_rig_pro_deform_stretch_names_and_configurable_spine_are_recognized() {
    let mut project = arp_character();
    project.nodes.extend([
        joint(50, "c_arm_stretch.l", 1),
        joint(51, "arm_twist.l", 1),
        joint(52, "arm_stretch.l.001", 7),
        joint(53, "c_hand_ik.l", 1),
    ]);
    let before = project.clone();
    let report = detect_humanoid(&project, None);
    assert_eq!(report.resolved.assignments.len(), 17);
    assert_eq!(report.resolved.assignments[&BoneRole::Chest], 4);
    assert_eq!(
        report.resolved.chain(BoneRole::LeftHand, &project).unwrap(),
        [7, 8, 9]
    );
    assert_eq!(
        report
            .resolved
            .chain(BoneRole::RightFoot, &project)
            .unwrap(),
        [16, 17, 18]
    );
    assert!(report.candidates[&BoneRole::LeftUpperArm]
        .iter()
        .filter(|c| c.node >= 50)
        .all(|c| c.confidence != BoneConfidence::High));
    assert_eq!(
        before, project,
        "recognition must not rewrite transforms or rig data"
    );
    project.nodes.push(joint(60, "spine_06.x", 4));
    project.node_mut(5).unwrap().parent = Some(60);
    assert_eq!(
        detect_humanoid(&project, Some(1)).resolved.assignments[&BoneRole::Chest],
        60
    );
}

#[test]
fn actual_skin_weights_resolve_driver_deform_ties_but_not_two_weighted_bones() {
    let mut project = arp_character();
    project.nodes.push(joint(50, "arm.l", 4));
    let mut mesh = box_mesh(101, "Skin", Vec3::ONE, None);
    for v in &mut mesh.vertices {
        v.joints = [0; 4];
        v.weights = [1., 0., 0., 0.];
    }
    project.meshes.push(mesh);
    let mut node = Node::new(100, "Body");
    node.mesh = Some(101);
    node.skin = Some(Skin {
        joints: vec![7, 50],
        inverse_bind_matrices: vec![Mat4::IDENTITY; 2],
    });
    project.nodes.push(node);
    let report = detect_humanoid(&project, Some(1));
    assert_eq!(report.resolved.assignments[&BoneRole::LeftUpperArm], 7);
    project.meshes[0].vertices[0].joints[0] = 1;
    assert!(!detect_humanoid(&project, Some(1))
        .resolved
        .assignments
        .contains_key(&BoneRole::LeftUpperArm));
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn rigify_limb_aliases_work_but_numbered_spine_requires_review() {
    let mut project = Project::default();
    project.nodes = vec![
        Node::new(1, "Rig"),
        joint(2, "DEF-spine", 1),
        joint(3, "DEF-spine.001", 2),
        joint(4, "DEF-spine.003", 3),
        joint(5, "DEF-upper_arm.L", 4),
        joint(6, "DEF-forearm.L", 5),
        joint(7, "DEF-hand.L", 6),
        joint(8, "DEF-upper_arm.L.001", 5),
    ];
    let report = detect_humanoid(&project, Some(1));
    assert_eq!(
        report.resolved.chain(BoneRole::LeftHand, &project).unwrap(),
        [5, 6, 7]
    );
    assert!(!report.resolved.assignments.contains_key(&BoneRole::Pelvis));
    assert_eq!(
        report.candidates[&BoneRole::Pelvis][0].confidence,
        BoneConfidence::Review
    );
    assert_eq!(
        report.candidates[&BoneRole::LeftUpperArm][1].confidence,
        BoneConfidence::Review
    );
}

#[test]
fn manual_assignment_rejects_duplicates_non_joints_and_other_characters_atomically() {
    let mut project = simple_character();
    project.nodes.push(Node::new(100, "Other character"));
    project.nodes.push(joint(101, "Hand.R", 100));
    let mut profile = RigProfile {
        root: Some(1),
        ..RigProfile::default()
    };
    profile.assign(BoneRole::LeftHand, 9, &project).unwrap();
    let before = profile.clone();
    assert!(profile.assign(BoneRole::RightHand, 9, &project).is_err());
    assert_eq!(profile, before);
    assert!(profile.assign(BoneRole::RightHand, 101, &project).is_err());
    assert_eq!(profile, before);
    assert!(profile.assign(BoneRole::RightHand, 1, &project).is_err());
    assert_eq!(profile, before);
    profile.unassign(BoneRole::LeftHand);
    assert!(profile.assignments.is_empty());
}

#[test]
fn ik_does_not_skip_intermediate_helper_nodes() {
    let mut project = simple_character();
    project.nodes.push(joint(50, "forearm_twist.L", 8));
    project.node_mut(9).unwrap().parent = Some(50);
    let report = detect_humanoid(&project, Some(1));
    assert_eq!(report.resolved.assignments[&BoneRole::LeftHand], 9);
    assert!(report.resolved.chain(BoneRole::LeftHand, &project).is_err());
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("прямой цепи IK нет")));
}

#[test]
fn skin_components_keep_unweighted_helpers_inside_one_character() {
    let mut project = simple_character();
    let mut helper = Node::new(50, "Unweighted helper");
    helper.parent = Some(8);
    project.nodes.push(helper);
    project.node_mut(9).unwrap().parent = Some(50);
    let mut mesh_node = Node::new(100, "Body mesh");
    mesh_node.skin = Some(Skin {
        joints: (2..=18).collect(),
        inverse_bind_matrices: vec![Mat4::IDENTITY; 17],
    });
    project.nodes.push(mesh_node);
    assert_eq!(character_roots(&project), vec![1]);
    let report = detect_humanoid(&project, Some(1));
    assert_eq!(report.resolved.assignments.len(), 17);
    assert!(report.resolved.chain(BoneRole::LeftHand, &project).is_err());
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn arbitrary_names_and_generic_root_are_not_guessed() {
    let mut project = Project::default();
    project.nodes = vec![
        Node::new(1, "Rig"),
        joint(2, "Root", 1),
        joint(3, "Bone.001", 2),
    ];
    let report = detect_humanoid(&project, None);
    assert!(report.resolved.assignments.is_empty());
    assert_eq!(
        report.candidates[&BoneRole::Pelvis][0].confidence,
        BoneConfidence::Review
    );
}

#[test]
fn malformed_hierarchy_and_unknown_scope_do_not_panic() {
    let mut project = simple_character();
    project.node_mut(2).unwrap().parent = Some(6);
    let report = detect_humanoid(&project, None);
    assert!(report.resolved.assignments.is_empty());
    assert!(!report.warnings.is_empty());
    let report = detect_humanoid(&project, Some(9999));
    assert!(report.resolved.assignments.is_empty());
    assert!(!report.warnings.is_empty());
}

#[test]
fn grace_manifest_recognizes_deform_roles_and_reports_non_direct_leg_chains() {
    let text = include_str!("../../../examples/grace-rig.txt");
    let rows: Vec<_> = text
        .lines()
        .filter(|line| line.contains("\tparent:"))
        .collect();
    assert_eq!(rows.len(), 623);
    let ids: BTreeMap<_, _> = rows
        .iter()
        .enumerate()
        .map(|(i, line)| (line.split('\t').next().unwrap(), i as u64 + 2))
        .collect();
    let mut project = Project::default();
    project.nodes.push(Node::new(1, "Grace_rig"));
    for line in rows {
        let columns: Vec<_> = line.split('\t').collect();
        let parent = columns[1].trim().strip_prefix("parent:").unwrap().trim();
        let parent_id = if parent == "-" { 1 } else { ids[parent] };
        project
            .nodes
            .push(joint(ids[columns[0]], columns[0], parent_id));
    }
    let report = detect_humanoid(&project, Some(1));
    assert_eq!(report.resolved.assignments.len(), 17);
    assert_eq!(
        report.resolved.assignments[&BoneRole::Pelvis],
        ids["root.x"]
    );
    assert_eq!(
        report.resolved.assignments[&BoneRole::Chest],
        ids["spine_03.x"]
    );
    assert_eq!(
        report.resolved.assignments[&BoneRole::LeftHand],
        ids["hand.l"]
    );
    assert!(report.resolved.chain(BoneRole::LeftHand, &project).is_ok());
    assert!(report.resolved.chain(BoneRole::LeftFoot, &project).is_err());
    assert!(report.candidates[&BoneRole::LeftHand]
        .iter()
        .any(|c| c.confidence == BoneConfidence::Ignored));
}

#[test]
fn profile_json_round_trip_preserves_ids_and_partial_mappings() {
    let project = simple_character();
    let mut profile = RigProfile {
        root: Some(1),
        ..RigProfile::default()
    };
    profile.assign(BoneRole::Head, 6, &project).unwrap();
    let encoded = serde_json::to_vec(&profile).unwrap();
    let decoded: RigProfile = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, profile);
    decoded.validate(&project).unwrap();
    assert!(decoded.chain(BoneRole::LeftHand, &project).is_err());
}

#[test]
fn mixed_skinned_and_unskinned_characters_stay_independent() {
    let mut project = Project::demo();
    project.nodes.push(Node::new(1000, "Scene"));
    project.node_mut(1).unwrap().parent = Some(1000);
    for mut node in simple_character().nodes {
        node.id += 100;
        node.parent = node.parent.map(|id| id + 100).or(Some(1000));
        project.nodes.push(node);
    }
    assert_eq!(character_roots(&project), vec![1, 101]);
    assert!(detect_humanoid(&project, None)
        .resolved
        .assignments
        .is_empty());
    assert!(detect_humanoid(&project, Some(1000))
        .resolved
        .assignments
        .is_empty());
    let skinned = detect_humanoid(&project, Some(1));
    assert_eq!(skinned.resolved.assignments.len(), 17);
    assert!(skinned.resolved.assignments.values().all(|id| *id < 100));
    let unskinned = detect_humanoid(&project, Some(101));
    assert_eq!(unskinned.resolved.assignments.len(), 17);
    assert!(unskinned
        .resolved
        .assignments
        .values()
        .all(|id| *id > 100 && *id < 1000));
}
