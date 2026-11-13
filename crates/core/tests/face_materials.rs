use poseforge_core::*;
use std::collections::BTreeSet;
fn scene() -> Editor {
    let mesh = PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, Some(10))
        .unwrap();
    let mut node = Node::new(1, "Cube");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![mesh],
        materials: vec![
            Material {
                id: 10,
                ..Material::default()
            },
            Material {
                id: 11,
                ..Material::default()
            },
        ],
        ..Project::default()
    })
}
fn assigned() -> Editor {
    let mut e = scene();
    e.assign_face_material(2, &[1], &BTreeSet::from([0]), 11)
        .unwrap();
    e
}
#[test]
fn polygon_assignment_is_atomic_persistent_and_isolates_instances() {
    let mut e = scene();
    let mut n = Node::new(3, "Instance");
    n.mesh = Some(2);
    e.project.nodes.push(n);
    let original = e.project.clone();
    let a = e
        .assign_face_material(2, &[1], &BTreeSet::from([0, 2]), 11)
        .unwrap();
    assert_ne!(a.mesh, 2);
    assert_eq!(e.project.mesh(2), original.mesh(2));
    let mesh = e.project.mesh(a.mesh).unwrap();
    assert!(mesh.vertices.shares_storage(&original.meshes[0].vertices));
    assert_eq!(mesh.face_count(), 6);
    assert_eq!(mesh.face_material(0), Some(11));
    assert_eq!(mesh.face_material(1), Some(10));
    assert_eq!(mesh.face_vertices(0).unwrap().len(), 4);
    let json = serde_json::to_vec(&e.project).unwrap();
    let restored: Project = serde_json::from_slice(&json).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, e.project);
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, original);
    e.redo();
    assert_eq!(e.project, restored);
}
#[test]
fn pruning_and_whole_object_assignment_handle_face_references() {
    let mut e = assigned();
    assert_eq!(e.remove_unused_materials().unwrap(), 0);
    let before = e.project.clone();
    e.assign_material(2, &[1], 10).unwrap();
    assert!(e.project.meshes[0]
        .topology
        .as_ref()
        .unwrap()
        .face_materials
        .is_empty());
    e.undo();
    assert_eq!(e.project, before);
    e.project.meshes[0].material = None;
    e.unassign_material(2, &[1]).unwrap();
    assert!(e.project.meshes[0].assigned_materials().is_empty());
}
#[test]
fn library_creation_and_copy_do_not_assign_objects() {
    let mut e = assigned();
    let meshes = e.project.meshes.clone();
    let id = e.create_library_material(MaterialPreset::Toon).unwrap();
    let copy = e.duplicate_library_material(id).unwrap();
    assert_ne!(id, copy);
    assert_eq!(e.project.meshes, meshes);
    let a = e.duplicate_assigned_material(2, &[1], 11).unwrap();
    assert_eq!(e.project.meshes[0].material, Some(10));
    assert_eq!(e.project.meshes[0].face_material(0), Some(a.material));
}
#[test]
fn topology_operations_preserve_material_boundaries_and_inherit_daughter_faces() {
    for (edit, count) in [
        (MeshEdit::Subdivide, 4),
        (
            MeshEdit::Extrude {
                faces: BTreeSet::from([0]),
                distance: 0.2,
            },
            5,
        ),
        (
            MeshEdit::Bevel {
                faces: BTreeSet::from([0]),
                inset: 0.2,
                shift: 0.,
            },
            5,
        ),
        (MeshEdit::DeleteFaces(BTreeSet::from([1])), 1),
        (MeshEdit::SmoothNormals, 1),
        (MeshEdit::FlatNormals, 1),
    ] {
        let mut e = assigned();
        let before = e.project.clone();
        e.edit_mesh(1, edit).unwrap();
        e.project.validate().unwrap();
        let m = e.project.meshes.last().unwrap();
        assert_eq!(
            (0..m.face_count())
                .filter(|&f| m.face_material(f) == Some(11))
                .count(),
            count
        );
        e.undo();
        assert_eq!(e.project, before);
    }
}
#[test]
fn knife_loop_cut_and_uv_keep_face_materials() {
    let mut e = assigned();
    let m = &e.project.meshes[0];
    let face = m.face_vertices(0).unwrap();
    let g = MeshConnectivity::new(m);
    let edge = |a: u32, b: u32| {
        let (a, b) = (g.canonical[a as usize], g.canonical[b as usize]);
        (a.min(b), a.max(b))
    };
    let first = edge(face[0], face[1]);
    let second = edge(face[2], face[3]);
    e.edit_mesh(
        1,
        MeshEdit::Knife {
            face: 0,
            first: EdgePoint {
                edge: first,
                t: 0.5,
            },
            second: EdgePoint {
                edge: second,
                t: 0.5,
            },
        },
    )
    .unwrap();
    e.project.validate().unwrap();
    assert_eq!(e.project.meshes[0].face_material(0), Some(11));
    assert_eq!(e.project.meshes[0].face_material(1), Some(11));
    let mut e = assigned();
    e.edit_mesh(
        1,
        MeshEdit::LoopCut {
            edge: first,
            position: 0.5,
        },
    )
    .unwrap();
    e.project.validate().unwrap();
    assert_eq!(
        (0..e.project.meshes[0].face_count())
            .filter(|&f| e.project.meshes[0].face_material(f) == Some(11))
            .count(),
        2
    );
}
#[test]
fn invalid_assignments_and_locked_owners_do_not_change_history() {
    let mut e = scene();
    let original = e.project.clone();
    assert!(e
        .assign_face_material(2, &[1], &BTreeSet::from([99]), 11)
        .is_err());
    assert!(e
        .assign_face_material(2, &[1], &BTreeSet::from([0]), 99)
        .is_err());
    assert_eq!(e.project, original);
    assert_eq!(e.undo_len(), 0);
    e.project.nodes[0].locked = true;
    assert!(e
        .assign_face_material(2, &[1], &BTreeSet::from([0]), 11)
        .is_err());
    assert_eq!(e.undo_len(), 0);
}

#[test]
fn removing_one_slot_preserves_other_faces_and_shared_instances() {
    for source in [10, 11] {
        let mut e = assigned();
        let mut instance = Node::new(3, "Instance");
        instance.mesh = Some(2);
        e.project.nodes.push(instance);
        let original = e.project.clone();
        let undo = e.undo_len();
        let mesh = e.unassign_material_slot(2, &[1], source).unwrap();
        assert_ne!(mesh, 2);
        assert_eq!(e.project.mesh(2), original.mesh(2));
        let output = e.project.mesh(mesh).unwrap();
        assert_eq!(output.face_material(0), if source == 11 { None } else { Some(11) });
        assert_eq!(output.face_material(1), if source == 10 { None } else { Some(10) });
        assert_eq!(e.project.materials, original.materials);
        e.project.validate().unwrap();
        assert_eq!(e.undo_len(), undo + 1);
        e.undo();
        assert_eq!(e.project, original);
    }
}
