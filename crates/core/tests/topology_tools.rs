use poseforge_core::*;
use std::collections::BTreeSet;
fn editor(kind: PrimitiveKind) -> Editor {
    let mut n = Node::new(1, "Test");
    n.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(kind).mesh(2, None).unwrap()],
        ..Default::default()
    })
}
fn closed(mesh: &Mesh) {
    mesh.validate_topology().unwrap();
    let g = MeshConnectivity::new(mesh);
    assert!(g.edge_faces.values().all(|f| f.len() == 2));
    let r = inspect_topology(mesh);
    assert_eq!(r.nonmanifold_edges, 0);
    assert_eq!(r.winding_edges, 0);
    assert_eq!(r.degenerate_faces, 0);
}
#[test]
fn segmented_edge_bevel_closes_single_connected_and_complete_cube_selections() {
    for segments in [1, 2, 4, 8] {
        for profile in [0., 0.5, 1.] {
            let e = editor(PrimitiveKind::Cube);
            let edges: Vec<_> = MeshConnectivity::new(&e.project.meshes[0])
                .edges
                .into_iter()
                .collect();
            let mut selections: Vec<BTreeSet<_>> =
                edges.iter().map(|&e| BTreeSet::from([e])).collect();
            selections.push(edges.iter().copied().collect());
            selections.push(BTreeSet::from([edges[0], edges[1]]));
            for edges in selections {
                let mut e = editor(PrimitiveKind::Cube);
                let before = e.project.clone();
                e.edit_mesh(
                    1,
                    MeshEdit::EdgeBevel {
                        edges: edges.clone(),
                        width: 0.1,
                        segments,
                        profile,
                    },
                )
                .unwrap_or_else(|error| {
                    panic!("{edges:?} segments {segments} profile {profile}: {error}")
                });
                closed(&e.project.meshes[0]);
                assert!(e.project.meshes[0].face_count() > 6);
                assert!(e.project.meshes[0]
                    .topology
                    .as_ref()
                    .unwrap()
                    .primitive
                    .is_none());
                e.undo();
                assert_eq!(e.project, before);
                e.redo();
                closed(&e.project.meshes[0]);
            }
        }
    }
}
#[test]
fn bevel_rejects_invalid_width_boundary_and_budget_atomically() {
    let mut e = editor(PrimitiveKind::Cube);
    let edges = MeshConnectivity::new(&e.project.meshes[0]).edges;
    let before = e.project.clone();
    for (width, segments, profile) in [
        (f32::NAN, 3, 1.),
        (2., 3, 1.),
        (0.1, 0, 1.),
        (0.1, 33, 1.),
        (0.1, 3, 2.),
    ] {
        assert!(e
            .edit_mesh(
                1,
                MeshEdit::EdgeBevel {
                    edges: edges.clone(),
                    width,
                    segments,
                    profile
                }
            )
            .is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.undo_len(), 0);
    }
    let mut e = editor(PrimitiveKind::Plane);
    let edges = MeshConnectivity::new(&e.project.meshes[0]).edges;
    assert!(e
        .edit_mesh(
            1,
            MeshEdit::EdgeBevel {
                edges,
                width: 0.1,
                segments: 3,
                profile: 1.
            }
        )
        .is_err());
}
#[test]
fn welding_reduces_duplicate_vertices_and_merge_collapses_edge_without_partial_failures() {
    let mut e = editor(PrimitiveKind::Cube);
    let before = e.project.clone();
    let vertices = (0..e.project.meshes[0].vertices.len()).collect();
    e.edit_mesh(
        1,
        MeshEdit::WeldVertices {
            vertices,
            distance: 0.00001,
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].vertices.len(), 8);
    closed(&e.project.meshes[0]);
    e.undo();
    assert_eq!(e.project, before);
    e.edit_mesh(
        1,
        MeshEdit::MergeVertices {
            vertices: BTreeSet::from([0, 1]),
        },
    )
    .unwrap();
    e.project.validate().unwrap();
    assert!(e.project.meshes[0].vertices.len() < 24);
    let state = e.project.clone();
    assert!(e
        .edit_mesh(
            1,
            MeshEdit::WeldVertices {
                vertices: (0..e.project.meshes[0].vertices.len()).collect(),
                distance: 100.
            }
        )
        .is_err());
    assert_eq!(e.project, state);
}
#[test]
fn bridge_two_open_caps_with_segments_produces_closed_quads() {
    for segments in [1, 3, 8] {
        let mut e = editor(PrimitiveKind::Cube);
        e.edit_mesh(1, MeshEdit::DeleteFaces(BTreeSet::from([0, 1, 4, 5])))
            .unwrap();
        let graph = MeshConnectivity::new(&e.project.meshes[0]);
        assert_eq!(graph.edges.len(), 8);
        let before = e.project.clone();
        e.edit_mesh(
            1,
            MeshEdit::Bridge {
                edges: graph.edges,
                segments,
            },
        )
        .unwrap();
        let m = &e.project.meshes[0];
        assert_eq!(m.face_count(), 2 + 4 * segments as usize);
        closed(m);
        assert!((0..m.face_count()).all(|f| m.face_vertices(f).unwrap().len() == 4));
        e.undo();
        assert_eq!(e.project, before);
    }
}
#[test]
fn dissolve_coplanar_region_keeps_ngon_and_triangulates_concave_boundary() {
    let mut e = editor(PrimitiveKind::Plane);
    let mut settings = PrimitiveSettings::new(PrimitiveKind::Plane);
    settings.segments = [2, 2, 1];
    e.project.meshes[0] = settings.mesh(2, None).unwrap();
    e.edit_mesh(1, MeshEdit::DeleteFaces(BTreeSet::from([3])))
        .unwrap();
    let g = MeshConnectivity::new(&e.project.meshes[0]);
    let edges = g
        .edge_faces
        .iter()
        .filter_map(|(&e, f)| (f.len() == 2).then_some(e))
        .collect();
    e.edit_mesh(1, MeshEdit::DissolveEdges { edges }).unwrap();
    let m = &e.project.meshes[0];
    assert_eq!(m.face_count(), 1);
    assert!(m.face_vertices(0).unwrap().len() > 4);
    m.validate_topology().unwrap();
    let area: f32 = m
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|f| {
            let p = |i| Vec3::from_array(m.vertices[f[i] as usize].position);
            (p(1) - p(0)).cross(p(2) - p(0)).length() * 0.5
        })
        .sum();
    assert!((area - 3.).abs() < 1e-5);
    let text = serde_json::to_string(&e.project).unwrap();
    let loaded: Project = serde_json::from_str(&text).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded, e.project);
}
#[test]
fn edge_ring_and_loop_follow_regular_quad_grid_and_stop_at_boundary() {
    let mut settings = PrimitiveSettings::new(PrimitiveKind::Plane);
    settings.segments = [4, 4, 1];
    let mesh = settings.mesh(2, None).unwrap();
    let seed = BTreeSet::from([(5, 6)]);
    assert_eq!(select_edge_path(&mesh, &seed, true).unwrap().len(), 5);
    assert_eq!(select_edge_path(&mesh, &seed, false).unwrap().len(), 4);
}
#[test]
fn topology_report_distinguishes_open_boundary_and_real_defects() {
    let mut e = editor(PrimitiveKind::Cube);
    assert_eq!(
        inspect_topology(&e.project.meshes[0]),
        TopologyReport::default()
    );
    e.edit_mesh(1, MeshEdit::DeleteFaces(BTreeSet::from([0])))
        .unwrap();
    let r = inspect_topology(&e.project.meshes[0]);
    assert_eq!(r.boundary_edges, 4);
    assert!(r.problem_faces.is_empty());
    let m = &mut e.project.meshes[0];
    let face = m.topology.as_ref().unwrap().faces[0].clone();
    m.topology.as_mut().unwrap().faces.push(face);
    m.vertices.push(Vertex::default());
    m.rebuild_polygon_indices().unwrap();
    let r = inspect_topology(m);
    assert_eq!(r.duplicate_faces, 1);
    assert_eq!(r.isolated_vertices, 1);
    assert!(!r.problem_faces.is_empty());
    assert!(r.nonmanifold_edges > 0);
}

#[test]
fn dissolving_concave_quad_chooses_an_interior_render_diagonal() {
    let mut e = editor(PrimitiveKind::Plane);
    let m = &mut e.project.meshes[0];
    m.vertices = [[0., 0., 0.], [0.4, 0.3, 0.], [2., 0., 0.], [0., 2., 0.]]
        .into_iter()
        .map(|position| Vertex {
            position,
            ..Default::default()
        })
        .collect();
    m.topology = Some(MeshTopology { loose_edges: Vec::new(),
        face_materials: Default::default(),
        uv_seams: Vec::new(),
        faces: vec![vec![1, 2, 3], vec![1, 3, 0]].into(),
        primitive: None,
        smooth_normals: false,
    });
    m.rebuild_polygon_indices().unwrap();
    e.edit_mesh(
        1,
        MeshEdit::DissolveEdges {
            edges: BTreeSet::from([(1, 3)]),
        },
    )
    .unwrap();
    let m = &e.project.meshes[0];
    assert_eq!(m.face_count(), 1);
    let mut area = 0.;
    for t in m.indices.as_chunks::<3>().0 {
        let p = |i: usize| Vec3::from_array(m.vertices[t[i] as usize].position);
        let cross = (p(1) - p(0)).cross(p(2) - p(0));
        assert!(cross.z > 0.);
        area += cross.z * 0.5;
    }
    assert!((area - 1.7).abs() < 0.00001);
    m.validate_topology().unwrap();
}
