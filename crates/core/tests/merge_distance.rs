use poseforge_core::*;
use std::collections::BTreeSet;
fn cube() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap()
}
fn editor(mesh: Mesh) -> Editor {
    let mut node = Node::new(1, "Mesh");
    node.mesh = Some(mesh.id);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![mesh],
        ..Default::default()
    })
}
fn copies() -> MeshEdit {
    MeshEdit::Array {
        region: MeshRegion {
            faces: (0..6).collect(),
            ..Default::default()
        },
        count: 2,
        offset: Vec3::X * 2.02,
        rotation: Vec3::ZERO,
    }
}
fn valid(mesh: &Mesh) {
    mesh.validate_topology().unwrap();
    let r = inspect_topology(mesh);
    assert_eq!(
        (
            r.boundary_edges,
            r.nonmanifold_edges,
            r.degenerate_faces,
            r.duplicate_faces,
            r.winding_edges
        ),
        (0, 0, 0, 0, 0),
        "{r:?}"
    );
}
#[test]
fn merge_distance_joins_nearby_copies_and_restores_a_smaller_threshold() {
    let source = cube();
    let mut e = editor(source.clone());
    let before = e.project.clone();
    e.begin_edit("Array");
    e.preview_mesh_merge_distance(1, &source, copies(), Some(0.01))
        .unwrap();
    assert_eq!(e.editable_mesh(1).unwrap().face_count(), 12);
    let separate = e.project.clone();
    let map = e
        .preview_mesh_merge_distance(1, &source, copies(), Some(0.03))
        .unwrap()
        .unwrap();
    let merged = e.project.clone();
    let mesh = e.editable_mesh(1).unwrap();
    assert_eq!(mesh.face_count(), 10);
    valid(mesh);
    assert_eq!(map.faces.iter().filter(|v| v.is_none()).count(), 2);
    for corner in &source.vertices {
        assert!(mesh
            .vertices
            .iter()
            .any(|v| v.position == corner.position && v.uv == corner.uv));
    }
    e.preview_mesh_merge_distance(1, &source, copies(), Some(0.01))
        .unwrap();
    assert_eq!(e.project, separate);
    e.preview_mesh_merge_distance(1, &source, copies(), None)
        .unwrap();
    assert_eq!(e.editable_mesh(1).unwrap().face_count(), 12);
    valid(e.editable_mesh(1).unwrap());
    e.preview_mesh_merge_distance(1, &source, copies(), Some(0.03))
        .unwrap();
    assert_eq!(e.project, merged);
    assert_eq!(e.undo_len(), 0);
    e.commit_edit().unwrap();
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, merged);
}
#[test]
fn merge_distance_works_for_bevel_and_inset_and_preserves_uv_corners() {
    for mode in [
        FaceTreatment::Region,
        FaceTreatment::Normals,
        FaceTreatment::Individual,
    ] {
        for bevel in [false, true] {
            for faces in [BTreeSet::from([0]), BTreeSet::from([0, 2])] {
                let source = cube();
                let mut e = editor(source.clone());
                let before = e.project.clone();
                e.begin_edit("Shape");
                let edit = || {
                    if bevel {
                        MeshEdit::FaceBevelMode {
                            faces: faces.clone(),
                            inset: 0.15,
                            shift: 0.,
                            segments: 3,
                            profile: 0.,
                            mode,
                        }
                    } else {
                        MeshEdit::InsetFaces {
                            faces: faces.clone(),
                            inset: 0.15,
                            depth: 0.,
                            mode,
                        }
                    }
                };
                e.preview_mesh_merge_distance(1, &source, edit(), Some(0.))
                    .unwrap();
                let unmerged = e.editable_mesh(1).unwrap().clone();
                e.preview_mesh_merge_distance(1, &source, edit(), Some(0.5))
                    .unwrap();
                let mesh = e.editable_mesh(1).unwrap();
                valid(mesh);
                assert!(
                    mesh.face_count() < unmerged.face_count(),
                    "bevel={bevel}, mode={mode:?}"
                );
                assert!(mesh
                    .vertices
                    .iter()
                    .all(|v| unmerged.vertices.iter().any(|old| old.uv == v.uv)));
                e.commit_edit().unwrap();
                e.undo();
                assert_eq!(e.project, before);
            }
        }
    }
}
#[test]
fn merge_distance_rejects_invalid_thresholds_atomically_and_zero_is_exact() {
    let source = cube();
    let mut e = editor(source.clone());
    e.begin_edit("Array");
    e.preview_mesh_merge_distance(1, &source, copies(), Some(0.))
        .unwrap();
    let before = e.project.clone();
    assert_eq!(e.editable_mesh(1).unwrap().face_count(), 12);
    for distance in [-1., f32::NAN, f32::INFINITY, 10001.] {
        assert!(e
            .preview_mesh_merge_distance(1, &source, copies(), Some(distance))
            .is_err());
        assert_eq!(e.project, before);
    }
}
#[test]
fn merge_distance_joins_wire_endpoints_and_drops_collapsed_edges() {
    let mut source = cube();
    source.vertices = vec![
        Vertex {
            position: [0., 0., 0.],
            ..Default::default()
        },
        Vertex {
            position: [1., 0., 0.],
            ..Default::default()
        },
    ]
    .into();
    let topology = source.topology.as_mut().unwrap();
    topology.faces = Default::default();
    topology.primitive = None;
    topology.loose_edges = vec![[0, 1]];
    source.rebuild_polygon_indices().unwrap();
    let mut e = editor(source.clone());
    e.begin_edit("Wire array");
    let edit = || MeshEdit::Array {
        region: MeshRegion {
            edges: BTreeSet::from([(0, 1)]),
            ..Default::default()
        },
        count: 3,
        offset: Vec3::X * 1.02,
        rotation: Vec3::ZERO,
    };
    let map = e
        .preview_mesh_merge_distance(1, &source, edit(), Some(0.03))
        .unwrap()
        .unwrap();
    let mesh = e.editable_mesh(1).unwrap();
    let graph = MeshConnectivity::new(mesh);
    assert_eq!(
        graph
            .canonical
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert_eq!(mesh.topology.as_ref().unwrap().loose_edges.len(), 3);
    assert!(map.vertices.iter().all(|&v| v < mesh.vertices.len()));
    e.preview_mesh_merge_distance(1, &source, edit(), Some(10.))
        .unwrap();
    assert!(e
        .editable_mesh(1)
        .unwrap()
        .topology
        .as_ref()
        .unwrap()
        .loose_edges
        .is_empty());
}
#[test]
fn merge_distance_does_not_weld_unrelated_close_source_points() {
    let mut source = cube();
    source.vertices.push(Vertex {
        position: [10., 0., 0.],
        ..Default::default()
    });
    source.vertices.push(Vertex {
        position: [10.01, 0., 0.],
        ..Default::default()
    });
    let mut e = editor(source.clone());
    e.begin_edit("Array");
    e.preview_mesh_merge_distance(1, &source, copies(), Some(0.03))
        .unwrap();
    let mesh = e.editable_mesh(1).unwrap();
    assert!(mesh.vertices.iter().any(|v| v.position == [10., 0., 0.]));
    assert!(mesh.vertices.iter().any(|v| v.position == [10.01, 0., 0.]));
}
