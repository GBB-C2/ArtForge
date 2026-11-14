use poseforge_core::*;
use std::collections::BTreeSet;

fn editor(points: &[[f32; 3]], faces: Vec<Vec<u32>>) -> Editor {
    let mut mesh = PrimitiveSettings::new(PrimitiveKind::Plane)
        .mesh(2, None)
        .unwrap();
    mesh.vertices = points
        .iter()
        .enumerate()
        .map(|(i, &position)| Vertex {
            position,
            uv: [i as f32 * 0.1, 0.5],
            color: [i as f32 * 0.1, 0.25, 0.5, 1.],
            ..Default::default()
        })
        .collect();
    let topology = mesh.topology.as_mut().unwrap();
    topology.faces = faces.into();
    topology.primitive = None;
    mesh.rebuild_polygon_indices().unwrap();
    mesh.validate_topology().unwrap();
    let mut node = Node::new(1, "Collapsed boundary");
    node.mesh = Some(mesh.id);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![mesh],
        ..Default::default()
    })
}

fn adjacent_faces(offset: f32, corner: usize) -> Editor {
    let mut points = vec![
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [2., 0., 0.],
        [2., 1., 0.],
    ];
    let mut duplicate = points[corner];
    duplicate[0] += offset;
    points.push(duplicate);
    let mut face = vec![0, 1, 2, 3];
    face.insert(corner + 1, 6);
    // Put the collapsed side across the last/first corner as well.
    if corner == 3 {
        face.rotate_right(1);
    }
    editor(&points, vec![face, vec![1, 4, 5, 2]])
}

#[test]
fn dissolve_skips_collapsed_sides_and_keeps_boundary_attributes_and_history() {
    for offset in [0., 0.0000001] {
        for corner in 0..4 {
            for mode in 0..4 {
                let mut e = adjacent_faces(offset, corner);
                let before = e.project.clone();
                let operation = match mode {
                    0 => MeshEdit::DissolveEdges {
                        edges: BTreeSet::from([(1, 2)]),
                    },
                    1 => MeshEdit::Dissolve {
                        region: MeshRegion {
                            edges: BTreeSet::from([(1, 2)]),
                            ..Default::default()
                        },
                    },
                    2 => MeshEdit::Dissolve {
                        region: MeshRegion {
                            faces: BTreeSet::from([0, 1]),
                            ..Default::default()
                        },
                    },
                    _ => MeshEdit::Dissolve {
                        region: MeshRegion {
                            vertices: BTreeSet::from([1]),
                            ..Default::default()
                        },
                    },
                };
                e.edit_mesh(1, operation).unwrap();
                let mesh = &e.project.meshes[0];
                mesh.validate_topology().unwrap();
                assert_eq!(mesh.face_count(), 1);
                let corners = if mode == 3 { 5 } else { 6 };
                assert_eq!(mesh.face_vertices(0).unwrap().len(), corners);
                assert_eq!(mesh.vertices.len(), corners);
                let graph = MeshConnectivity::new(mesh);
                assert_eq!(graph.edges.len(), corners);
                assert!(graph.edge_faces.values().all(|faces| faces.len() == 1));
                let mut area = 0.;
                for triangle in mesh.indices.as_chunks::<3>().0 {
                    let p =
                        |i: usize| Vec3::from_array(mesh.vertices[triangle[i] as usize].position);
                    let normal = (p(1) - p(0)).cross(p(2) - p(0));
                    assert!(normal.z >= 0.);
                    area += normal.z * 0.5;
                }
                assert!((area - 2.).abs() < 0.000001);
                for vertex in &mesh.vertices {
                    assert!(before.meshes[0].vertices.iter().any(|original| {
                        vertex.position == original.position
                            && vertex.uv == original.uv
                            && vertex.color == original.color
                    }));
                }
                let after = e.project.clone();
                assert_eq!(e.undo_len(), 1);
                assert!(e.undo());
                assert_eq!(e.project, before);
                assert!(e.redo());
                assert_eq!(e.project, after);
            }
        }
    }
}

#[test]
fn distributed_extrude_does_not_build_walls_on_collapsed_sides() {
    for offset in [0., 0.0000001] {
        for distance in [-0.25, 0.25] {
            let mut e = adjacent_faces(offset, 1);
            let before = e.project.clone();
            e.edit_mesh(
                1,
                MeshEdit::ExtrudeDistributed {
                    faces: BTreeSet::from([0, 1]),
                    distance,
                },
            )
            .unwrap();
            let mesh = &e.project.meshes[0];
            mesh.validate_topology().unwrap();
            assert_eq!(mesh.face_count(), 8);
            for f in 0..2 {
                assert!(mesh.face_vertices(f).unwrap().iter().all(|&v| {
                    (mesh.vertices[v as usize].position[2] - distance).abs() < 0.000001
                }));
            }
            for f in 2..8 {
                let face = mesh.face_vertices(f).unwrap();
                assert_eq!(face.len(), 4);
                let p = |i: usize| Vec3::from_array(mesh.vertices[face[i] as usize].position);
                assert!((p(1) - p(0)).cross(p(2) - p(0)).length() > 0.2);
                assert!((p(2) - p(0)).cross(p(3) - p(0)).length() > 0.2);
            }
            assert!(e.undo());
            assert_eq!(e.project, before);
        }
    }
}

#[test]
fn collapsed_dissolve_with_no_valid_outline_fails_without_changing_scene_or_history() {
    let mut e = editor(
        &[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 0.]],
        vec![vec![0, 3, 1, 2], vec![0, 2, 1]],
    );
    let operation = MeshEdit::DissolveEdges {
        edges: BTreeSet::from([(1, 2)]),
    };
    let before = e.project.clone();
    assert!(e.edit_mesh(1, operation.clone()).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), 0);
    assert!(!e.can_redo());
    e.begin_edit("Dissolve preview");
    assert!(e.preview_mesh(1, &before.meshes[0], operation).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), 0);
    e.cancel_edit();
    assert_eq!(e.project, before);
    assert!(!e.has_active_edit());
}

#[test]
fn bridge_face_boundaries_skip_coincident_corners() {
    for offset in [0., 0.0000001] {
        let mut e = editor(
            &[
                [0., 0., 0.],
                [1., 0., 0.],
                [1., 1., 0.],
                [0., 1., 0.],
                [0., 0., 1.],
                [1., 0., 1.],
                [1., 1., 1.],
                [0., 1., 1.],
                [offset, 0., 0.],
                [offset, 0., 1.],
            ],
            vec![vec![0, 8, 1, 2, 3], vec![4, 7, 6, 5, 9]],
        );
        let before = e.project.clone();
        e.edit_mesh(
            1,
            MeshEdit::BridgeFaces {
                faces: BTreeSet::from([0, 1]),
                segments: 2,
            },
        )
        .unwrap();
        let mesh = &e.project.meshes[0];
        mesh.validate_topology().unwrap();
        assert_eq!(mesh.face_count(), 8);
        assert!((0..mesh.face_count()).all(|f| mesh.face_vertices(f).unwrap().len() == 4));
        let report = inspect_topology(mesh);
        assert_eq!(report.degenerate_faces, 0);
        assert_eq!(report.nonmanifold_edges, 0);
        assert_eq!(report.winding_edges, 0);
        assert!(e.undo());
        assert_eq!(e.project, before);
    }
}
