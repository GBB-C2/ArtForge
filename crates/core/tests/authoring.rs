use poseforge_core::*;
use std::collections::BTreeSet;

fn editor() -> Editor {
    let mut p = Project::default();
    p.meshes.push(box_mesh(2, "Cube", Vec3::ONE, None));
    let mut n = Node::new(1, "Object");
    n.mesh = Some(2);
    n.transform = Transform {
        translation: Vec3::new(2., 1., -1.),
        rotation: Quat::from_rotation_z(0.3),
        scale: Vec3::new(-1., 2., 0.7),
    };
    p.nodes.push(n);
    Editor::new(p)
}
#[test]
fn empty_bone_is_an_independent_serializable_object_and_one_undo_step() {
    let mut e = editor();
    let before = e.project.clone();
    let root = e.create_skeleton(SkeletonTemplate::Bone, None).unwrap();
    let children: Vec<_> = e
        .project
        .nodes
        .iter()
        .filter(|n| n.parent == Some(root))
        .collect();
    assert_eq!(children.len(), 1);
    let bone = children[0];
    assert!(bone.is_joint);
    assert!(bone.mesh.is_none() && bone.skin.is_none());
    assert_eq!(bone.transform.translation, Vec3::ZERO);
    assert_eq!(bone.rest_transform, Some(bone.transform));
    assert_eq!(e.project.meshes, before.meshes);
    assert_eq!(e.project.node(1), before.node(1));
    assert!(e.project.rig_profiles.is_empty());
    e.project.validate().unwrap();
    let saved = serde_json::to_vec(&e.project).unwrap();
    let reopened: Project = serde_json::from_slice(&saved).unwrap();
    assert_eq!(reopened, e.project);
    assert_eq!(e.undo_len(), 1);
    assert!(e.undo());
    assert_eq!(e.project, before);
    assert!(e.redo());
    assert_eq!(e.project, reopened);
    let joint = root + 1;
    let tip = e.extend_joint(joint, Vec3::Y * 0.5).unwrap();
    assert_eq!(e.project.node(tip).unwrap().parent, Some(joint));
    assert_eq!(e.undo_len(), 2);
    assert!(e.undo());
    assert_eq!(e.project, reopened);
}

#[test]
fn vertex_edit_welds_seams_detaches_instances_and_undo_is_atomic() {
    let mut e = editor();
    let mut copy = e.project.nodes[0].clone();
    copy.id = 3;
    e.project.nodes.push(copy);
    let before = e.project.clone();
    let position = before.meshes[0].vertices[0].position;
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::X * 0.25,
        },
    )
    .unwrap();
    let id = e.project.node(1).unwrap().mesh.unwrap();
    assert_ne!(id, 2);
    assert_eq!(e.project.mesh(2), before.mesh(2));
    for (old, new) in before
        .mesh(2)
        .unwrap()
        .vertices
        .iter()
        .zip(&e.project.mesh(id).unwrap().vertices)
    {
        if old.position == position {
            assert_eq!(
                Vec3::from_array(new.position),
                Vec3::from_array(position) + Vec3::X * 0.25
            );
        } else {
            assert_eq!(new.position, old.position);
        }
    }
    assert_eq!(e.undo_len(), 1);
    assert!(e.undo());
    assert_eq!(e.project, before);
    assert!(e.redo());
    e.project.validate().unwrap();
}
#[test]
fn mesh_region_extrusion_subdivision_delete_and_normals_keep_valid_uvs() {
    let mut e = editor();
    let before = e.project.meshes[0].clone();
    e.edit_mesh(
        1,
        MeshEdit::Extrude {
            faces: BTreeSet::from([0, 1]),
            distance: 0.3,
        },
    )
    .unwrap();
    assert!(e.project.meshes[0].indices.len() > before.indices.len());
    e.project.validate().unwrap();
    let count = e.project.meshes[0].indices.len();
    e.edit_mesh(1, MeshEdit::Subdivide).unwrap();
    assert_eq!(e.project.meshes[0].indices.len(), count * 4);
    e.edit_mesh(1, MeshEdit::DeleteFaces(BTreeSet::from([0])))
        .unwrap();
    assert_eq!(e.project.meshes[0].indices.len(), count * 4 - 3);
    e.edit_mesh(1, MeshEdit::SmoothNormals).unwrap();
    e.edit_mesh(1, MeshEdit::FlatNormals).unwrap();
    let mesh = &e.project.meshes[0];
    for t in mesh.indices.as_chunks::<3>().0.iter() {
        assert_eq!(
            mesh.vertices[t[0] as usize].normal,
            mesh.vertices[t[1] as usize].normal
        );
    }
    assert!(mesh
        .vertices
        .iter()
        .all(|v| v.uv.iter().all(|n| n.is_finite())
            && (Vec3::from_array(v.normal).length() - 1.).abs() < 1e-4));
    e.project.validate().unwrap();
}
#[test]
fn invalid_locked_skinned_and_empty_mesh_edits_do_not_change_history() {
    let mut e = editor();
    let before = e.project.clone();
    for operation in [
        MeshEdit::Translate {
            vertices: BTreeSet::from([999999]),
            delta: Vec3::X,
        },
        MeshEdit::Extrude {
            faces: BTreeSet::from([0]),
            distance: f32::NAN,
        },
        MeshEdit::DeleteFaces((0..12).collect()),
    ] {
        assert!(e.edit_mesh(1, operation).is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.undo_len(), 0);
    }
    e.project.nodes[0].locked = true;
    assert!(e.edit_mesh(1, MeshEdit::Subdivide).is_err());
    e.project.nodes[0].locked = false;
    let root = e.create_skeleton(SkeletonTemplate::Chain, Some(1)).unwrap();
    e.bind_mesh(1, root).unwrap();
    let bound = e.project.clone();
    assert!(e.edit_mesh(1, MeshEdit::Subdivide).is_err());
    assert_eq!(e.project, bound);
}
#[test]
fn authoring_gesture_cancel_restores_geometry_and_shared_resource_identity() {
    let mut e = editor();
    let before = e.project.clone();
    e.begin_edit("Vertices");
    for _ in 0..5 {
        e.edit_mesh(
            1,
            MeshEdit::Translate {
                vertices: BTreeSet::from([0]),
                delta: Vec3::Y * 0.1,
            },
        )
        .unwrap();
    }
    assert_eq!(e.undo_len(), 0);
    e.cancel_edit();
    assert_eq!(e.project, before);
    e.begin_edit("Vertices");
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::Z * 0.1,
        },
    )
    .unwrap();
    e.commit_edit().unwrap();
    assert_eq!(e.undo_len(), 1);
}
#[test]
fn humanoid_template_has_valid_ik_chains_and_binding_has_no_rest_jump() {
    let mut e = editor();
    let before = e.project.clone();
    let root = e
        .create_skeleton(SkeletonTemplate::Humanoid, Some(1))
        .unwrap();
    let profile = e.project.rig_profiles.last().unwrap();
    assert_eq!(profile.root, Some(root));
    assert_eq!(profile.assignments.len(), 17);
    for role in [
        BoneRole::LeftHand,
        BoneRole::RightHand,
        BoneRole::LeftFoot,
        BoneRole::RightFoot,
    ] {
        profile.chain(role, &e.project).unwrap();
    }
    e.bind_mesh(1, root).unwrap();
    let globals = e.project.global_transforms().unwrap();
    let n = e.project.node(1).unwrap();
    let skin = n.skin.as_ref().unwrap();
    assert_eq!(skin.joints.len(), HUMANOID_JOINT_COUNT);
    let mesh = e.project.mesh(n.mesh.unwrap()).unwrap();
    for v in &mesh.vertices {
        assert!((v.weights.iter().sum::<f32>() - 1.).abs() < 1e-5);
        let mut world = Vec4::ZERO;
        for (&joint, &weight) in v.joints.iter().zip(&v.weights) {
            world += globals[&skin.joints[joint as usize]]
                * skin.inverse_bind_matrices[joint as usize]
                * Vec3::from_array(v.position).extend(1.)
                * weight;
        }
        assert!(
            world
                .truncate()
                .distance(globals[&1].transform_point3(Vec3::from_array(v.position)))
                < 1e-4
        );
    }
    let bound = e.project.clone();
    e.unbind_mesh(1).unwrap();
    assert!(e.project.node(1).unwrap().skin.is_none());
    assert!(e.undo());
    assert_eq!(e.project, bound);
    assert!(e.undo());
    assert!(e.undo());
    assert_eq!(e.project, before);
}
#[test]
fn bone_extension_and_auto_weights_deform_with_joint_motion_and_preserve_other_instance() {
    let mut e = editor();
    let mut instance = e.project.nodes[0].clone();
    instance.id = 3;
    e.project.nodes.push(instance);
    let mesh_before = e.project.meshes[0].clone();
    let root = e.create_skeleton(SkeletonTemplate::Chain, Some(1)).unwrap();
    let last = root + 3;
    let tip = e.extend_joint(last, Vec3::Y * 0.4).unwrap();
    assert_eq!(e.project.node(tip).unwrap().parent, Some(last));
    e.bind_mesh(1, root).unwrap();
    assert_ne!(e.project.node(1).unwrap().mesh, Some(2));
    assert_eq!(e.project.mesh(2).unwrap(), &mesh_before);
    let joint = root + 1;
    e.project.node_mut(joint).unwrap().transform.translation.x += 0.7;
    let globals = e.project.global_transforms().unwrap();
    let n = e.project.node(1).unwrap();
    let skin = n.skin.as_ref().unwrap();
    let mesh = e.project.mesh(n.mesh.unwrap()).unwrap();
    let v = mesh.vertices[0];
    let mut deformed = Vec4::ZERO;
    for (&j, &w) in v.joints.iter().zip(&v.weights) {
        deformed += globals[&skin.joints[j as usize]]
            * skin.inverse_bind_matrices[j as usize]
            * Vec3::from_array(v.position).extend(1.)
            * w;
    }
    let original = globals[&1].transform_point3(Vec3::from_array(v.position));
    assert!((deformed.truncate() - original - Vec3::X * 0.7).length() < 1e-4);
    e.project.validate().unwrap();
}

#[test]
fn binding_large_rig_keeps_rest_pose_and_cancellation_is_atomic() {
    let mut e = editor();
    e.project.nodes.push(Node::new(100, "Rig"));
    for i in 0..300 {
        let mut joint = Node::new(101 + i, "Joint");
        joint.parent = Some(100);
        joint.is_joint = true;
        joint.transform.translation = if i == 299 {
            Vec3::ZERO
        } else {
            Vec3::X * (1000. + i as f32)
        };
        e.project.nodes.push(joint);
    }
    let before = e.project.clone();
    assert!(e.bind_mesh_cancellable(1, 100, || true).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), 0);
    e.bind_mesh(1, 100).unwrap();
    let globals = e.project.global_transforms().unwrap();
    let skin = e.project.node(1).unwrap().skin.as_ref().unwrap();
    assert_eq!(skin.joints.len(), 300);
    for v in &e.project.meshes[0].vertices {
        assert_eq!(v.joints[0], 299);
        let mut position = Vec4::ZERO;
        for (&j, &w) in v.joints.iter().zip(&v.weights) {
            position += globals[&skin.joints[j as usize]]
                * skin.inverse_bind_matrices[j as usize]
                * Vec3::from_array(v.position).extend(1.)
                * w;
        }
        let expected = globals[&1].transform_point3(Vec3::from_array(v.position));
        assert!(position.truncate().distance(expected) < 1e-4);
    }
    assert!(e.undo());
    assert_eq!(e.project, before);
}
#[test]
fn cancelled_binding_and_locked_skeleton_leave_original_project_untouched() {
    let mut e = editor();
    let root = e.create_skeleton(SkeletonTemplate::Chain, Some(1)).unwrap();
    let before = e.project.clone();
    let history = e.undo_len();
    assert!(e.bind_mesh_cancellable(1, root, || true).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), history);
    e.project.node_mut(root + 1).unwrap().locked = true;
    let before = e.project.clone();
    assert!(e.bind_mesh(1, root).is_err());
    assert_eq!(e.project, before);
}

#[test]
fn automatic_weights_follow_explicit_bone_length_instead_of_only_its_origin() {
    let mut project = Project::default();
    project.nodes.push(Node::new(1, "Skeleton"));
    for (id, origin, tip) in [
        (2, Vec3::ZERO, Some(Vec3::X * 10.)),
        (3, Vec3::new(9.8, 1., 0.), None),
    ] {
        let mut joint = Node::new(id, "Bone");
        joint.parent = Some(1);
        joint.is_joint = true;
        joint.transform.translation = origin;
        joint.bone_tip = tip;
        project.nodes.push(joint);
    }
    let mut object = Node::new(10, "Surface near the long bone's tip");
    object.mesh = Some(100);
    object.transform.translation = Vec3::new(9.8, 0., 0.);
    project.nodes.push(object);
    project
        .meshes
        .push(box_mesh(100, "Surface", Vec3::splat(0.01), None));
    project.validate().unwrap();
    let mut editor = Editor::new(project);
    editor.bind_mesh(10, 1).unwrap();
    let skin = editor.project.node(10).unwrap().skin.as_ref().unwrap();
    for vertex in &editor.project.mesh(100).unwrap().vertices {
        assert_eq!(skin.joints[vertex.joints[0] as usize], 2);
        assert!(vertex.weights[0] > 0.99);
    }
    editor.project.validate().unwrap();
}

#[test]
fn extending_an_authored_tip_inherits_appearance_and_undo_restores_its_endpoint() {
    let mut editor = Editor::new(Project::default());
    editor
        .create_skeleton(SkeletonTemplate::Humanoid, None)
        .unwrap();
    let parent = editor
        .project
        .nodes
        .iter()
        .find(|n| n.name == "f_index.03.L")
        .unwrap()
        .id;
    let appearance = BoneDisplay {
        color: [0.8, 0.2, 0.1],
        shape: BoneShape::Rectangle,
        ..Default::default()
    };
    editor.project.node_mut(parent).unwrap().bone_display = Some(appearance);
    let before = editor.project.clone();
    let endpoint = before.node(parent).unwrap().bone_tip.unwrap();
    let child = editor.extend_joint(parent, endpoint).unwrap();
    assert_eq!(editor.project.node(parent).unwrap().bone_tip, None);
    assert_eq!(
        editor.project.node(child).unwrap().bone_display,
        Some(appearance)
    );
    let globals = editor.project.global_transforms().unwrap();
    assert!(
        globals[&child]
            .w_axis
            .truncate()
            .distance(globals[&parent].transform_point3(endpoint))
            < 1e-5
    );
    editor.project.validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    assert!(editor.redo());
    assert_eq!(editor.project.node(parent).unwrap().bone_tip, None);
}
