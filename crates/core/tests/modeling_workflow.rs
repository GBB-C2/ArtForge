use poseforge_core::*;
use std::collections::BTreeSet;
fn custom(points: Vec<[f32; 3]>, faces: Vec<Vec<u32>>) -> Editor {
    let mut n = Node::new(1, "Model");
    n.mesh = Some(2);
    let mut m = PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap();
    m.vertices = points
        .into_iter()
        .enumerate()
        .map(|(i, position)| Vertex {
            position,
            uv: [i as f32 * 0.1, 0.5],
            ..Default::default()
        })
        .collect();
    let t = m.topology.as_mut().unwrap();
    t.faces = faces.into();
    t.primitive = None;
    t.face_materials = Default::default();
    m.rebuild_polygon_indices().unwrap();
    Editor::new(Project {
        nodes: vec![n],
        meshes: vec![m],
        ..Default::default()
    })
}
fn quad() -> Editor {
    custom(
        vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
        vec![vec![0, 1, 2, 3]],
    )
}
fn mesh(e: &Editor) -> &Mesh {
    e.editable_mesh(1).unwrap()
}
fn apply(e: &mut Editor, op: MeshEdit) {
    e.edit_mesh(1, op).unwrap();
    e.project.validate().unwrap();
    mesh(e).validate_topology().unwrap();
    assert_eq!(inspect_topology(mesh(e)).degenerate_faces, 0);
}
#[test]
fn workflow_fill_one_vertex_or_edge_fills_only_its_complete_boundary() {
    for mode in 0..3 {
        let mut e = custom(
            vec![
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [3., 0., 0.],
                [4., 0., 0.],
                [3., 1., 0.],
            ],
            vec![vec![0, 1, 2], vec![3, 4, 5]],
        );
        let before = e.project.clone();
        let region = match mode {
            0 => MeshRegion {
                vertices: BTreeSet::from([0]),
                ..Default::default()
            },
            1 => MeshRegion {
                edges: BTreeSet::from([(0, 1)]),
                ..Default::default()
            },
            _ => MeshRegion {
                faces: BTreeSet::from([0]),
                ..Default::default()
            },
        };
        apply(
            &mut e,
            MeshEdit::Topology {
                region,
                edit: TopologyEdit::FillHoles,
            },
        );
        assert_eq!(mesh(&e).face_count(), 3);
        assert!(mesh(&e).face_vertices(2).unwrap().iter().all(|&v| v < 3));
        assert!(e.undo());
        assert_eq!(e.project, before);
    }
}
#[test]
fn workflow_dissolve_face_edge_and_vertex_keep_surface() {
    for mode in 0..2 {
        let mut e = custom(
            vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            vec![vec![0, 1, 2], vec![0, 2, 3]],
        );
        let region = if mode == 0 {
            MeshRegion {
                faces: BTreeSet::from([0, 1]),
                ..Default::default()
            }
        } else {
            MeshRegion {
                edges: BTreeSet::from([(0, 2)]),
                ..Default::default()
            }
        };
        apply(&mut e, MeshEdit::Dissolve { region });
        assert_eq!(mesh(&e).face_count(), 1);
        assert_eq!(mesh(&e).face_vertices(0).unwrap().len(), 4);
    }
    let mut e = custom(
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [0.5, 0.5, 0.],
        ],
        vec![vec![0, 1, 4], vec![1, 2, 4], vec![2, 3, 4], vec![3, 0, 4]],
    );
    apply(
        &mut e,
        MeshEdit::Dissolve {
            region: MeshRegion {
                vertices: BTreeSet::from([4]),
                ..Default::default()
            },
        },
    );
    assert_eq!((mesh(&e).face_count(), mesh(&e).vertices.len()), (1, 4));
}
#[test]
fn workflow_dissolve_boundary_vertex_preserves_corner_uv() {
    let mut e = custom(
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
        ],
        vec![vec![0, 1, 2, 3, 4]],
    );
    let uv = mesh(&e).vertices[2].uv;
    apply(
        &mut e,
        MeshEdit::Dissolve {
            region: MeshRegion {
                vertices: BTreeSet::from([1]),
                ..Default::default()
            },
        },
    );
    assert_eq!(mesh(&e).vertices.len(), 4);
    assert!(mesh(&e)
        .vertices
        .iter()
        .any(|v| v.position == [2., 0., 0.] && v.uv == uv));
}
#[test]
fn workflow_bridge_two_boundary_edges_and_unequal_open_chains() {
    for segments in [1, 3] {
        let mut e = custom(
            vec![
                [0., 0., 0.],
                [1., 0., 0.],
                [1., -1., 0.],
                [0., -1., 0.],
                [0., 2., 0.],
                [0.5, 2., 0.],
                [1., 2., 0.],
                [1., 3., 0.],
                [0., 3., 0.],
            ],
            vec![vec![0, 3, 2, 1], vec![4, 5, 6, 7, 8]],
        );
        let before = e.project.clone();
        apply(
            &mut e,
            MeshEdit::BridgeAdvanced {
                faces: Default::default(),
                edges: BTreeSet::from([(0, 1), (4, 5), (5, 6)]),
                segments,
                twist: 0,
                smooth: 0.,
            },
        );
        assert!(mesh(&e).face_count() > 2);
        let r = inspect_topology(mesh(&e));
        assert_eq!((r.nonmanifold_edges, r.winding_edges), (0, 0));
        assert!(e.undo());
        assert_eq!(e.project, before);
    }
}
#[test]
fn workflow_bridge_loose_edges_creates_surface() {
    let mut e = custom(
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [1., 1., 0.],
            [3., 0., 0.],
            [4., 0., 0.],
            [3., 1., 0.],
        ],
        vec![vec![4, 5, 6]],
    );
    e.project.meshes[0].topology.as_mut().unwrap().loose_edges = vec![[0, 1], [2, 3]];
    apply(
        &mut e,
        MeshEdit::BridgeAdvanced {
            faces: Default::default(),
            edges: BTreeSet::from([(0, 1), (2, 3)]),
            segments: 1,
            twist: 0,
            smooth: 0.,
        },
    );
    assert_eq!(mesh(&e).face_count(), 2);
    assert!(mesh(&e).topology.as_ref().unwrap().loose_edges.is_empty());
}
#[test]
fn workflow_knife_gap_removes_band_and_preserves_uv_and_undo() {
    let mut e = quad();
    let before = e.project.clone();
    apply(
        &mut e,
        MeshEdit::KnifeGap {
            cuts: vec![KnifeCut {
                face: 0,
                first: [1., 0., 0.],
                second: [1., 2., 0.],
            }],
            width: 0.2,
        },
    );
    assert_eq!(mesh(&e).face_count(), 2);
    for f in 0..2 {
        let xs: Vec<_> = mesh(&e)
            .face_vertices(f)
            .unwrap()
            .iter()
            .map(|&v| mesh(&e).vertices[v as usize].position[0])
            .collect();
        assert!(xs.iter().all(|&x| x <= 0.90001) || xs.iter().all(|&x| x >= 1.09999));
    }
    assert!(mesh(&e)
        .vertices
        .iter()
        .any(|v| (v.position[0] - 0.9).abs() < 1e-5 && (v.uv[0] - 0.045).abs() < 1e-5));
    assert!(e.undo());
    assert_eq!(e.project, before);
}
#[test]
fn workflow_loop_path_gap_and_repeated_cuts_are_atomic() {
    let mut e = custom(
        vec![
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [0., 1., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
        ],
        vec![vec![0, 1, 2, 3], vec![3, 2, 4, 5]],
    );
    let before = e.project.clone();
    let cuts = loop_cut_path(mesh(&e), (0, 1), 0.5).unwrap();
    assert_eq!(cuts.len(), 2);
    e.begin_edit("Loops");
    apply(&mut e, MeshEdit::KnifeGap { cuts, width: 0.2 });
    assert_eq!(mesh(&e).face_count(), 4);
    e.cancel_edit();
    assert_eq!(e.project, before);
}
#[test]
fn workflow_oversized_gap_is_rejected_without_partial_changes() {
    let mut e = quad();
    let before = e.project.clone();
    assert!(e
        .edit_mesh(
            1,
            MeshEdit::KnifeGap {
                cuts: vec![KnifeCut {
                    face: 0,
                    first: [1., 0., 0.],
                    second: [1., 2., 0.]
                }],
                width: 5.
            }
        )
        .is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), 0);
}
#[test]
fn workflow_distributed_extrude_handles_closed_selection_without_common_normal() {
    let mut n = Node::new(1, "Cube");
    n.mesh = Some(2);
    let mut e = Editor::new(Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    });
    let selection = (0..mesh(&e).face_count()).collect();
    apply(
        &mut e,
        MeshEdit::ExtrudeDistributed {
            faces: selection,
            distance: 0.2,
        },
    );
    assert_eq!(inspect_topology(mesh(&e)).boundary_edges, 0);
    assert_eq!(inspect_topology(mesh(&e)).isolated_vertices, 0);
}
#[test]
fn workflow_array_copies_only_selection_with_rotation_and_one_undo() {
    let mut e = quad();
    let before = e.project.clone();
    apply(
        &mut e,
        MeshEdit::Array {
            region: MeshRegion {
                faces: BTreeSet::from([0]),
                ..Default::default()
            },
            count: 3,
            offset: Vec3::X * 3.,
            rotation: Vec3::Z * 90.,
        },
    );
    assert_eq!(mesh(&e).face_count(), 3);
    assert_eq!(mesh(&e).vertices.len(), 12);
    for i in 0..4 {
        assert_eq!(mesh(&e).vertices[i], before.meshes[0].vertices[i]);
        assert_eq!(mesh(&e).vertices[i + 4].uv, mesh(&e).vertices[i].uv);
    }
    assert!(e.undo());
    assert_eq!(e.project, before);
}

#[test]
fn workflow_array_of_vertex_pair_keeps_connecting_edges() {
    let mut e = quad();
    let original = mesh(&e).vertices.clone();
    apply(
        &mut e,
        MeshEdit::Array {
            region: MeshRegion {
                vertices: BTreeSet::from([0, 1]),
                ..Default::default()
            },
            count: 3,
            offset: Vec3::Y * 3.,
            rotation: Vec3::ZERO,
        },
    );
    assert_eq!(mesh(&e).vertices.len(), 8);
    assert_eq!(mesh(&e).topology.as_ref().unwrap().loose_edges.len(), 2);
    assert_eq!(&mesh(&e).vertices[..4], original.as_slice());
}
