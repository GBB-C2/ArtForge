//! Original Y-up human template: 29 body bones plus 38 palm/finger bones.
//! Uses familiar Rigify-style names; no Blender generator or control code is embedded.
use crate::{BoneRole as R, Node, RigProfile, Transform};
use glam::{Quat, Vec3};

pub const HUMANOID_JOINT_COUNT: usize = 67;

struct Bone {
    name: String,
    parent: Option<usize>,
    head: Vec3,
    tip: Vec3,
    role: Option<R>,
}

fn add(
    bones: &mut Vec<Bone>,
    name: impl Into<String>,
    parent: Option<usize>,
    head: Vec3,
    tip: Vec3,
    role: Option<R>,
) -> usize {
    let index = bones.len();
    bones.push(Bone {
        name: name.into(),
        parent,
        head,
        tip,
        role,
    });
    index
}

fn specification() -> Vec<Bone> {
    let mut bones = Vec::with_capacity(HUMANOID_JOINT_COUNT);
    let heights = [0.5, 0.58, 0.65, 0.72, 0.80, 0.86, 0.90, 1.0];
    for i in 0usize..7 {
        let role = match i {
            0 => Some(R::Pelvis),
            1 => Some(R::Spine),
            3 => Some(R::Chest),
            5 => Some(R::Neck),
            6 => Some(R::Head),
            _ => None,
        };
        add(
            &mut bones,
            if i == 0 {
                "spine".into()
            } else {
                format!("spine.{i:03}")
            },
            i.checked_sub(1),
            Vec3::Y * heights[i],
            Vec3::Y * heights[i + 1],
            role,
        );
    }
    for (side, sign, roles) in [
        (
            "L",
            1_f32,
            [
                R::LeftUpperArm,
                R::LeftForearm,
                R::LeftHand,
                R::LeftThigh,
                R::LeftShin,
                R::LeftFoot,
            ],
        ),
        (
            "R",
            -1_f32,
            [
                R::RightUpperArm,
                R::RightForearm,
                R::RightHand,
                R::RightThigh,
                R::RightShin,
                R::RightFoot,
            ],
        ),
    ] {
        let point = |x: f32, y: f32, z: f32| Vec3::new(x * sign, y, z);
        let hip = point(0.09, 0.50, 0.);
        let knee = point(0.09, 0.275, 0.025);
        let ankle = point(0.09, 0.055, 0.);
        let toes = point(0.09, 0.035, 0.10);
        let pelvis = add(
            &mut bones,
            format!("pelvis.{side}"),
            Some(0),
            Vec3::Y * 0.5,
            hip,
            None,
        );
        let thigh = add(
            &mut bones,
            format!("thigh.{side}"),
            Some(pelvis),
            hip,
            knee,
            Some(roles[3]),
        );
        let shin = add(
            &mut bones,
            format!("shin.{side}"),
            Some(thigh),
            knee,
            ankle,
            Some(roles[4]),
        );
        let foot = add(
            &mut bones,
            format!("foot.{side}"),
            Some(shin),
            ankle,
            toes,
            Some(roles[5]),
        );
        add(
            &mut bones,
            format!("toe.{side}"),
            Some(foot),
            toes,
            point(0.09, 0.035, 0.17),
            None,
        );
        add(
            &mut bones,
            format!("heel.02.{side}"),
            Some(foot),
            point(0.09, 0.035, -0.02),
            point(0.09, 0.035, -0.07),
            None,
        );
        add(
            &mut bones,
            format!("breast.{side}"),
            Some(3),
            point(0.065, 0.765, 0.025),
            point(0.065, 0.765, 0.10),
            None,
        );
        let shoulder_tip = point(0.115, 0.795, 0.);
        let elbow = point(0.28, 0.78, 0.025);
        let wrist = point(0.405, 0.77, 0.);
        let shoulder = add(
            &mut bones,
            format!("shoulder.{side}"),
            Some(3),
            point(0.025, 0.795, 0.),
            shoulder_tip,
            None,
        );
        let arm = add(
            &mut bones,
            format!("upper_arm.{side}"),
            Some(shoulder),
            shoulder_tip,
            elbow,
            Some(roles[0]),
        );
        let forearm = add(
            &mut bones,
            format!("forearm.{side}"),
            Some(arm),
            elbow,
            wrist,
            Some(roles[1]),
        );
        let hand = add(
            &mut bones,
            format!("hand.{side}"),
            Some(forearm),
            wrist,
            point(0.455, 0.77, 0.),
            Some(roles[2]),
        );
        for (finger, (name, z, length)) in [
            ("index", -0.024, 0.075),
            ("middle", -0.008, 0.082),
            ("ring", 0.009, 0.074),
            ("pinky", 0.026, 0.059),
        ]
        .into_iter()
        .enumerate()
        {
            let start = point(0.46, 0.77, z);
            let palm = add(
                &mut bones,
                format!("palm.{:02}.{side}", finger + 1),
                Some(hand),
                point(0.425, 0.77, z * 0.3),
                start,
                None,
            );
            let mut parent = palm;
            let lengths = [0., 0.45, 0.78, 1.];
            for segment in 0..3 {
                parent = add(
                    &mut bones,
                    format!("f_{name}.{:02}.{side}", segment + 1),
                    Some(parent),
                    point(0.46 + length * lengths[segment], 0.77, z),
                    point(0.46 + length * lengths[segment + 1], 0.77, z),
                    None,
                );
            }
        }
        let thumb = [
            point(0.425, 0.765, -0.025),
            point(0.45, 0.76, -0.052),
            point(0.47, 0.758, -0.074),
            point(0.485, 0.756, -0.09),
        ];
        let mut parent = hand;
        for segment in 0..3 {
            parent = add(
                &mut bones,
                format!("thumb.{:02}.{side}", segment + 1),
                Some(parent),
                thumb[segment],
                thumb[segment + 1],
                None,
            );
        }
    }
    bones
}

pub(crate) fn create(root: u64, base: Vec3, height: f32) -> (Vec<Node>, RigProfile) {
    let bones = specification();
    let orientations: Vec<_> = bones
        .iter()
        .map(|b| Quat::from_rotation_arc(Vec3::Y, (b.tip - b.head).normalize()))
        .collect();
    let mut nodes = vec![Node::new(root, "Скелет · человек")];
    let mut profile = RigProfile {
        root: Some(root),
        ..Default::default()
    };
    for (i, bone) in bones.iter().enumerate() {
        let id = root + 1 + i as u64;
        let mut node = Node::new(id, &bone.name);
        node.is_joint = true;
        node.parent = Some(bone.parent.map_or(root, |p| root + 1 + p as u64));
        let rotation = orientations[i];
        node.transform = if let Some(parent) = bone.parent {
            let inverse = orientations[parent].conjugate();
            Transform {
                translation: inverse * ((bone.head - bones[parent].head) * height),
                rotation: (inverse * rotation).normalize(),
                ..Default::default()
            }
        } else {
            Transform {
                translation: base + bone.head * height,
                rotation,
                ..Default::default()
            }
        };
        node.rest_transform = Some(node.transform);
        node.bone_tip = Some(Vec3::Y * ((bone.tip - bone.head).length() * height));
        if let Some(role) = bone.role {
            profile.assignments.insert(role, id);
        }
        nodes.push(node);
    }
    (nodes, profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn human_has_67_independent_bones_and_complete_symmetric_hands() {
        let (nodes, profile) = create(1, Vec3::ZERO, 2.);
        let project = crate::Project {
            nodes,
            rig_profiles: vec![profile.clone()],
            ..Default::default()
        };
        project.validate().unwrap();
        assert_eq!(project.nodes.len(), HUMANOID_JOINT_COUNT + 1);
        assert_eq!(profile.assignments.len(), 17);
        let globals = project.global_transforms().unwrap();
        for bone in project.nodes.iter().filter(|n| n.is_joint) {
            let tip = globals[&bone.id].transform_point3(bone.bone_tip.unwrap());
            assert!(tip.is_finite() && tip.distance(globals[&bone.id].w_axis.truncate()) > 0.001);
            if let Some(name) = bone.name.strip_suffix(".L") {
                let right = project
                    .nodes
                    .iter()
                    .find(|n| n.name == format!("{name}.R"))
                    .unwrap();
                let mirrored = globals[&bone.id].w_axis.truncate() * Vec3::new(-1., 1., 1.);
                assert!(mirrored.distance(globals[&right.id].w_axis.truncate()) < 1e-5);
                assert!(
                    (tip * Vec3::new(-1., 1., 1.))
                        .distance(globals[&right.id].transform_point3(right.bone_tip.unwrap()))
                        < 1e-5
                );
            }
        }
        for role in [R::LeftHand, R::RightHand, R::LeftFoot, R::RightFoot] {
            profile.chain(role, &project).unwrap();
        }
        assert_eq!(
            project
                .nodes
                .iter()
                .filter(|n| n.name.starts_with("f_") || n.name.starts_with("thumb."))
                .count(),
            30
        );
    }
}
