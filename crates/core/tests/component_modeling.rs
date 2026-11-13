use poseforge_core::*;
use std::{collections::BTreeSet, sync::atomic::AtomicBool};

fn editor(kind: PrimitiveKind) -> Editor {
    let mut n = Node::new(1, "Model");
    n.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(kind).mesh(2, None).unwrap()],
        ..Default::default()
    })
}
fn custom(points: &[[f32; 3]], faces: Vec<Vec<u32>>) -> Editor {
    let mut e = editor(PrimitiveKind::Plane);
    let m = &mut e.project.meshes[0];
    m.vertices = points
        .iter()
        .map(|&position| Vertex {
            position,
            uv: [position[0], position[1]],
            ..Default::default()
        })
        .collect();
    let t = m.topology.as_mut().unwrap();
    t.faces = faces.into();
    t.primitive = None;
    m.rebuild_polygon_indices().unwrap();
    e.project.validate().unwrap();
    e
}
fn solid(m: &Mesh) {
    m.validate_topology().unwrap();
    for tri in m.indices.chunks_exact(3) {
        let p = |i| Vec3::from_array(m.vertices[tri[i] as usize].position);
        assert!(
            (p(1) - p(0)).cross(p(2) - p(0)).length() > 1e-7,
            "degenerate triangle {tri:?}"
        );
    }
}
#[test]
fn vertex_extrusion_can_continue_and_roundtrip_wires_without_touching_faces() {
    let mut e = editor(PrimitiveKind::Cube);
    let original = e.project.clone();
    let count = e.project.meshes[0].vertices.len();
    e.edit_mesh(
        1,
        MeshEdit::ExtrudeVertices {
            vertices: BTreeSet::from([0]),
            distance: 0.3,
        },
    )
    .unwrap();
    let m = &e.project.meshes[0];
    assert_eq!(m.vertices.len(), count + 1);
    assert_eq!(m.face_count(), 6);
    assert_eq!(
        m.topology.as_ref().unwrap().loose_edges,
        vec![[0, count as u32]]
    );
    e.edit_mesh(
        1,
        MeshEdit::ExtrudeVertices {
            vertices: BTreeSet::from([count]),
            distance: 0.4,
        },
    )
    .unwrap();
    assert_eq!(
        e.project.meshes[0]
            .topology
            .as_ref()
            .unwrap()
            .loose_edges
            .len(),
        2
    );
    let p: Project = serde_json::from_slice(&serde_json::to_vec(&e.project).unwrap()).unwrap();
    p.validate().unwrap();
    assert_eq!(p, e.project);
    let graph = MeshConnectivity::new(&p.meshes[0]);
    assert!(graph.edges.contains(&(count, count + 1)));
    e.edit_mesh(
        1,
        MeshEdit::Bevel {
            faces: BTreeSet::from([3]),
            inset: 0.2,
            shift: 0.1,
        },
    )
    .unwrap();
    assert_eq!(
        e.project.meshes[0]
            .topology
            .as_ref()
            .unwrap()
            .loose_edges
            .len(),
        2
    );
    e.edit_mesh(1, MeshEdit::FlatNormals).unwrap();
    assert_eq!(
        e.project.meshes[0]
            .topology
            .as_ref()
            .unwrap()
            .loose_edges
            .len(),
        2
    );
    e.project.validate().unwrap();
    e.edit_mesh(
        1,
        MeshEdit::Extrude {
            faces: BTreeSet::from([0]),
            distance: 0.1,
        },
    )
    .unwrap();
    assert_eq!(
        e.project.meshes[0]
            .topology
            .as_ref()
            .unwrap()
            .loose_edges
            .len(),
        2
    );
    e.project.validate().unwrap();
    for _ in 0..5 {
        e.undo();
    }
    assert_eq!(e.project, original);
}
#[test]
fn extrusion_of_selected_neighbors_creates_a_connected_quad_strip() {
    let mut e = editor(PrimitiveKind::Plane);
    let g = MeshConnectivity::new(&e.project.meshes[0]);
    let &(a, b) = g.edges.first().unwrap();
    e.edit_mesh(
        1,
        MeshEdit::ExtrudeVertices {
            vertices: BTreeSet::from([a, b]),
            distance: 0.5,
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 2);
    solid(&e.project.meshes[0]);
}
#[test]
fn selected_subdivision_splits_neighbor_boundaries_without_subdividing_neighbor_faces() {
    for vertex_mode in [false, true] {
        let mut e = editor(PrimitiveKind::Cube);
        let original = e.project.clone();
        let vertices = if vertex_mode {
            e.project.meshes[0]
                .face_vertices(0)
                .unwrap()
                .iter()
                .map(|&v| v as usize)
                .collect()
        } else {
            BTreeSet::new()
        };
        e.edit_mesh(
            1,
            MeshEdit::SubdivideSelected {
                vertices,
                edges: BTreeSet::new(),
                faces: if vertex_mode {
                    BTreeSet::new()
                } else {
                    BTreeSet::from([0])
                },
            },
        )
        .unwrap();
        let m = &e.project.meshes[0];
        assert_eq!(m.face_count(), 9);
        solid(m);
        assert!(
            MeshConnectivity::new(m)
                .edge_faces
                .values()
                .all(|f| f.len() == 2),
            "subdivision introduced a crack"
        );
        let opposite = original.meshes[0].face_vertices(1).unwrap();
        assert!(
            (0..m.face_count()).any(|f| m.face_vertices(f) == Some(opposite)),
            "untouched face was replaced"
        );
        e.undo();
        assert_eq!(e.project, original);
    }
    let mut e = editor(PrimitiveKind::Cube);
    let edge = *MeshConnectivity::new(&e.project.meshes[0])
        .edges
        .first()
        .unwrap();
    e.edit_mesh(
        1,
        MeshEdit::SubdivideSelected {
            vertices: BTreeSet::new(),
            edges: BTreeSet::from([edge]),
            faces: BTreeSet::new(),
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 6);
    solid(&e.project.meshes[0]);
    assert!(!MeshConnectivity::new(&e.project.meshes[0])
        .edges
        .contains(&edge));
}
#[test]
fn delete_handles_faces_vertices_edges_and_the_entire_selection_atomically() {
    for mode in 0..3 {
        let mut e = editor(PrimitiveKind::Cube);
        let original = e.project.clone();
        let edge = *MeshConnectivity::new(&e.project.meshes[0])
            .edges
            .first()
            .unwrap();
        let deleted = e.project.meshes[0].vertices[edge.0].position;
        e.edit_mesh(
            1,
            MeshEdit::DeleteComponents {
                vertices: if mode == 0 {
                    BTreeSet::from([edge.0])
                } else {
                    BTreeSet::new()
                },
                edges: if mode == 1 {
                    BTreeSet::from([edge])
                } else {
                    BTreeSet::new()
                },
                faces: if mode == 2 {
                    BTreeSet::from([0])
                } else {
                    BTreeSet::new()
                },
            },
        )
        .unwrap();
        e.project.validate().unwrap();
        assert!(e.project.meshes[0].face_count() < 6);
        if mode == 0 {
            assert!(e.project.meshes[0]
                .vertices
                .iter()
                .all(|v| v.position != deleted));
        }
        e.undo();
        assert_eq!(e.project, original);
        e.redo();
        e.project.validate().unwrap();
    }
    let mut e = editor(PrimitiveKind::Plane);
    e.edit_mesh(
        1,
        MeshEdit::DeleteComponents {
            vertices: BTreeSet::new(),
            edges: BTreeSet::new(),
            faces: BTreeSet::from([0]),
        },
    )
    .unwrap();
    assert!(e.project.meshes[0].indices.is_empty());
    assert!(e.project.meshes[0].vertices.is_empty());
    e.project.validate().unwrap();
}
#[test]
fn face_bevel_handles_concave_centers_and_degenerate_inset_side_walls() {
    let points = [
        [0., 0., 0.],
        [3., 0., 0.],
        [3., 1., 0.],
        [1., 1., 0.],
        [1., 3., 0.],
        [0., 3., 0.],
    ];
    for shift in [0., 0.2, -0.2] {
        let mut e = custom(&points, vec![vec![0, 1, 2, 3, 4, 5]]);
        let original = e.project.clone();
        e.edit_mesh(
            1,
            MeshEdit::Bevel {
                faces: BTreeSet::from([0]),
                inset: 0.3,
                shift,
            },
        )
        .unwrap();
        let m = &e.project.meshes[0];
        solid(m);
        for &v in m.face_vertices(0).unwrap() {
            let p = m.vertices[v as usize].position;
            assert!(p[0] < 1. || p[1] < 1.);
        }
        e.undo();
        assert_eq!(e.project, original);
    }
    let u = [
        [0., 0., 0.],
        [3., 0., 0.],
        [3., 3., 0.],
        [2., 3., 0.],
        [2., 1., 0.],
        [1., 1., 0.],
        [1., 3., 0.],
        [0., 3., 0.],
    ];
    let mut e = custom(&u, vec![(0..8).collect()]);
    e.edit_mesh(
        1,
        MeshEdit::Bevel {
            faces: BTreeSet::from([0]),
            inset: 0.3,
            shift: 0.1,
        },
    )
    .unwrap();
    solid(&e.project.meshes[0]);
    let mut e = custom(&u, vec![(0..8).collect()]);
    e.edit_mesh(
        1,
        MeshEdit::SubdivideSelected {
            vertices: BTreeSet::new(),
            edges: BTreeSet::new(),
            faces: BTreeSet::from([0]),
        },
    )
    .unwrap();
    solid(&e.project.meshes[0]);
    let area: f32 = e.project.meshes[0]
        .indices
        .chunks_exact(3)
        .map(|tri| {
            let p = |i| Vec3::from_array(e.project.meshes[0].vertices[tri[i] as usize].position);
            (p(1) - p(0)).cross(p(2) - p(0)).length() * 0.5
        })
        .sum();
    assert!((area - 7.).abs() < 1e-5);
}
#[test]
fn dissolve_accepts_folded_faces_and_ignores_selected_boundary_edges() {
    let mut e = editor(PrimitiveKind::Cube);
    let original = e.project.clone();
    let edge = *MeshConnectivity::new(&e.project.meshes[0])
        .edges
        .first()
        .unwrap();
    e.edit_mesh(
        1,
        MeshEdit::DissolveEdges {
            edges: BTreeSet::from([edge]),
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 5);
    e.project.validate().unwrap();
    e.undo();
    assert_eq!(e.project, original);
    let mut settings = PrimitiveSettings::new(PrimitiveKind::Plane);
    settings.segments = [2, 2, 1];
    e.project.meshes[0] = settings.mesh(2, None).unwrap();
    let edges = MeshConnectivity::new(&e.project.meshes[0]).edges;
    e.edit_mesh(1, MeshEdit::DissolveEdges { edges }).unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 1);
    e.project.validate().unwrap();
}
#[test]
fn bridge_faces_removes_caps_and_adds_segmented_side_faces() {
    let points = [
        [-1., -1., 0.],
        [-1., 1., 0.],
        [1., 1., 0.],
        [1., -1., 0.],
        [-1., -1., 2.],
        [1., -1., 2.],
        [1., 1., 2.],
        [-1., 1., 2.],
    ];
    for segments in [1, 3] {
        let mut e = custom(&points, vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]]);
        let original = e.project.clone();
        e.edit_mesh(
            1,
            MeshEdit::BridgeFaces {
                faces: BTreeSet::from([0, 1]),
                segments,
            },
        )
        .unwrap();
        assert_eq!(e.project.meshes[0].face_count(), 4 * segments as usize);
        solid(&e.project.meshes[0]);
        e.undo();
        assert_eq!(e.project, original);
    }
}
#[test]
fn knife_continues_from_previous_endpoint_and_preserves_neighbor_connectivity() {
    let mut e = custom(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [0., 1., 0.],
            [1., 1., 0.],
            [2., 1., 0.],
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    );
    let original = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Knife {
            face: 0,
            first: EdgePoint {
                edge: (0, 3),
                t: 0.5,
            },
            second: EdgePoint {
                edge: (1, 4),
                t: 0.5,
            },
        },
    )
    .unwrap();
    let m = &e.project.meshes[0];
    let endpoint = m
        .vertices
        .iter()
        .position(|v| v.position == [1., 0.5, 0.])
        .unwrap();
    let graph = MeshConnectivity::new(m);
    let face = *graph.edge_faces[&(2, 5)].first().unwrap();
    let edge = graph
        .edges
        .iter()
        .find(|&&(a, b)| {
            (a == endpoint || b == endpoint) && graph.edge_faces[&(a, b)].contains(&face)
        })
        .copied()
        .unwrap();
    e.edit_mesh(
        1,
        MeshEdit::Knife {
            face,
            first: EdgePoint {
                edge,
                t: if edge.0 == endpoint { 0. } else { 1. },
            },
            second: EdgePoint {
                edge: (2, 5),
                t: 0.5,
            },
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 4);
    solid(&e.project.meshes[0]);
    e.undo();
    e.undo();
    assert_eq!(e.project, original);
}
#[test]
fn array_duplicates_vertex_extrusion_wires_and_leaves_source_unchanged() {
    let mut e = editor(PrimitiveKind::Cube);
    e.edit_mesh(
        1,
        MeshEdit::ExtrudeVertices {
            vertices: BTreeSet::from([0]),
            distance: 0.2,
        },
    )
    .unwrap();
    let source = e.project.meshes[0].clone();
    let mut without_wires = source.clone();
    without_wires.topology.as_mut().unwrap().loose_edges.clear();
    assert_ne!(
        modifier_key(&source, &[]),
        modifier_key(&without_wires, &[])
    );
    let id = e.add_modifier(1, ModifierKind::Array).unwrap();
    let mut modifier = e.project.node(1).unwrap().modifiers[0].clone();
    assert_eq!(modifier.id, id);
    modifier.settings = ModifierSettings::Array {
        count: 3,
        offset: [3., 0., 0.],
    };
    e.update_modifier(1, modifier).unwrap();
    let evaluated = e
        .project
        .evaluated_modifier_meshes(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(evaluated[&1].vertices.len(), source.vertices.len() * 3);
    assert_eq!(
        evaluated[&1].topology.as_ref().unwrap().loose_edges.len(),
        3
    );
    assert_eq!(e.project.meshes[0], source);
    for smooth in [false, true] {
        let subdivision = e.add_modifier(1, ModifierKind::Subdivision).unwrap();
        let mut modifier = e.project.node(1).unwrap().modifiers.last().unwrap().clone();
        modifier.settings = ModifierSettings::Subdivision { levels: 2, smooth };
        e.update_modifier(1, modifier).unwrap();
        let evaluated = e
            .project
            .evaluated_modifier_meshes(&AtomicBool::new(false))
            .unwrap();
        let mesh = &evaluated[&1];
        mesh.validate_topology().unwrap();
        assert_eq!(mesh.topology.as_ref().unwrap().loose_edges.len(), 3);
        assert_eq!(e.project.meshes[0], source);
        e.remove_modifier(1, subdivision).unwrap();
    }
}
