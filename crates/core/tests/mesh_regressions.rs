use poseforge_core::*;
use std::collections::BTreeSet;
fn editor(mut mesh: Mesh) -> Editor {
    mesh.id = 2;
    let mut n = Node::new(1, "Model");
    n.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![n],
        meshes: vec![mesh],
        ..Default::default()
    })
}
fn cube() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap()
}
fn mesh(e: &Editor) -> &Mesh {
    e.editable_mesh(1).unwrap()
}
fn top(m: &Mesh) -> BTreeSet<usize> {
    (0..m.face_count())
        .filter(|&f| {
            m.face_vertices(f)
                .unwrap()
                .iter()
                .all(|&v| m.vertices[v as usize].position[1] > 0.99)
        })
        .collect()
}
fn closed(m: &Mesh) {
    let r = inspect_topology(m);
    assert_eq!(
        (
            r.boundary_edges,
            r.nonmanifold_edges,
            r.winding_edges,
            r.degenerate_faces
        ),
        (0, 0, 0, 0),
        "{r:?}"
    );
}
fn volume(m: &Mesh) -> f32 {
    m.indices
        .chunks_exact(3)
        .map(|t| {
            let p = |i| Vec3::from_array(m.vertices[t[i] as usize].position);
            p(0).dot(p(1).cross(p(2))) / 6.
        })
        .sum::<f32>()
        .abs()
}
fn custom(points: Vec<[f32; 3]>, faces: Vec<Vec<u32>>) -> Mesh {
    let mut m = cube();
    m.vertices = points
        .into_iter()
        .map(|position| Vertex {
            position,
            ..Default::default()
        })
        .collect();
    let t = m.topology.as_mut().unwrap();
    t.faces = faces.into();
    t.primitive = None;
    t.face_materials = Default::default();
    m.rebuild_polygon_indices().unwrap();
    m
}
#[test]
fn inward_manifold_crosses_triangulated_and_subdivided_neighbor_walls() {
    for subdivide in [false, true] {
        let mut e = editor(cube());
        let all = (0..6).collect();
        e.edit_mesh(
            1,
            if subdivide {
                MeshEdit::Subdivide
            } else {
                MeshEdit::Topology {
                    region: MeshRegion {
                        faces: all,
                        ..Default::default()
                    },
                    edit: TopologyEdit::Triangulate,
                }
            },
        )
        .unwrap();
        let before = e.project.clone();
        let faces = top(mesh(&e));
        e.edit_mesh(
            1,
            MeshEdit::ExtrudeManifold {
                faces,
                distance: -0.3,
            },
        )
        .unwrap();
        closed(mesh(&e));
        assert!((volume(mesh(&e)) - 6.8).abs() < 1e-3);
        e.undo();
        assert_eq!(e.project, before);
    }
}
#[test]
fn knife_cuts_warped_quad_without_flattening_its_rendered_surface() {
    let mut e = editor(custom(
        vec![[0., 0., 0.], [2., 0., 0.4], [2., 2., 0.], [0., 2., -0.2]],
        vec![vec![0, 1, 2, 3]],
    ));
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::KnifeStroke {
            cuts: vec![KnifeCut {
                face: 0,
                first: [1., 0., 0.2],
                second: [1., 2., -0.1],
            }],
        },
    )
    .unwrap();
    assert!(mesh(&e).vertices.iter().any(|v| v.position[0] > 0.1
        && v.position[0] < 1.9
        && (v.position[0] - v.position[1]).abs() < 1e-4
        && v.position[2].abs() < 1e-4));
    let r = inspect_topology(mesh(&e));
    assert_eq!(r.nonmanifold_edges, 0);
    e.undo();
    assert_eq!(e.project, before);
}
fn path(m: &Mesh, axis: usize) -> Vec<KnifeCut> {
    let g = MeshConnectivity::new(m);
    let e = *g
        .edges
        .iter()
        .find(|&&(a, b)| (m.vertices[a].position[axis] - m.vertices[b].position[axis]).abs() > 1.9)
        .unwrap();
    loop_cut_path(m, e, 0.5).unwrap()
}
#[test]
fn crossing_capped_loop_gaps_produce_four_closed_pieces_with_axis_aligned_caps() {
    let mut e = editor(cube());
    let mut cuts = path(mesh(&e), 0);
    cuts.extend(path(mesh(&e), 1));
    e.edit_mesh(1, MeshEdit::KnifeGapClosed { cuts, width: 0.2 })
        .unwrap();
    closed(mesh(&e));
    assert!(
        (volume(mesh(&e)) - 6.48).abs() < 1e-3,
        "volume {}",
        volume(mesh(&e))
    );
    for f in 0..mesh(&e).face_count() {
        let r = mesh(&e).face_vertices(f).unwrap();
        assert!(
            (0..3).any(
                |a| r.iter().all(|&v| (mesh(&e).vertices[v as usize].position[a]
                    - mesh(&e).vertices[r[0] as usize].position[a])
                    .abs()
                    < 1e-4)
            ),
            "nonplanar/diagonal cap {f}"
        );
    }
}
#[test]
fn loop_traverses_triangles_and_caps_from_previous_cuts() {
    let mut e = editor(cube());
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion {
                faces: (0..6).collect(),
                ..Default::default()
            },
            edit: TopologyEdit::Triangulate,
        },
    )
    .unwrap();
    let cuts = path(mesh(&e), 0);
    e.edit_mesh(1, MeshEdit::KnifeStroke { cuts }).unwrap();
    closed(mesh(&e));
    let mut e = editor(cube());
    let cuts = path(mesh(&e), 0);
    e.edit_mesh(1, MeshEdit::KnifeGapClosed { cuts, width: 0.2 })
        .unwrap();
    let cuts = path(mesh(&e), 1);
    e.edit_mesh(1, MeshEdit::KnifeGapClosed { cuts, width: 0.2 })
        .unwrap();
    closed(mesh(&e));
    assert!(
        (volume(mesh(&e)) - 6.84).abs() < 1e-3,
        "volume {}",
        volume(mesh(&e))
    );
}
#[test]
fn bevel_handles_reflex_and_collinear_corners_without_collapsed_side_faces() {
    for points in [
        vec![
            [0., 0., 0.],
            [3., 0., 0.],
            [3., 1., 0.],
            [1., 1., 0.],
            [1., 3., 0.],
            [0., 3., 0.],
        ],
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
        ],
    ] {
        let n = points.len();
        let e = editor(custom(points, vec![(0..n as u32).collect()]));
        for shift in [0., 0.2, -0.2] {
            let mut e = editor(mesh(&e).clone());
            e.edit_mesh(
                1,
                MeshEdit::Bevel {
                    faces: BTreeSet::from([0]),
                    inset: 0.25,
                    shift,
                },
            )
            .unwrap();
            let m = mesh(&e);
            for t in m.indices.chunks_exact(3) {
                let p = |i| Vec3::from_array(m.vertices[t[i] as usize].position);
                assert!(
                    (p(1) - p(0)).cross(p(2) - p(0)).length() > 1e-7,
                    "collapsed bevel triangle"
                );
            }
        }
    }
}

#[test]
fn polygon_bevel_profiles_bend_both_ways_keep_closed_and_do_not_extrude_a_side_wall() {
    let mut results = Vec::new();
    for profile in [-1., 0., 1.] {
        let mut e = editor(cube());
        let faces = top(mesh(&e));
        let before = e.project.clone();
        e.edit_mesh(
            1,
            MeshEdit::FaceBevel {
                faces,
                inset: 0.2,
                shift: 0.,
                segments: 4,
                profile,
            },
        )
        .unwrap();
        closed(mesh(&e));
        assert!(mesh(&e).vertices.iter().all(|v| v.position[1] <= 1.00001));
        results.push(volume(mesh(&e)));
        e.undo();
        assert_eq!(e.project, before);
    }
    assert!(
        results[0] < results[1] && results[1] < results[2],
        "{results:?}"
    );
}
#[test]
fn complex_polygon_bevel_handles_concave_solid_and_small_edges() {
    let points = vec![
        [0., 0., 0.],
        [3., 0., 0.],
        [3., 0.02, 0.],
        [3., 1., 0.],
        [1., 1., 0.],
        [1., 3., 0.],
        [0., 3., 0.],
    ];
    let n = points.len();
    let mut vertices = points.clone();
    vertices.extend(points.iter().map(|p| [p[0], p[1], 1.]));
    let mut faces = vec![
        (0..n as u32).rev().collect(),
        (n as u32..2 * n as u32).collect(),
    ];
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![i as u32, j as u32, (j + n) as u32, (i + n) as u32]);
    }
    let mut e = editor(custom(vertices, faces));
    e.edit_mesh(
        1,
        MeshEdit::FaceBevel {
            faces: BTreeSet::from([1]),
            inset: 0.25,
            shift: 0.,
            segments: 3,
            profile: -0.5,
        },
    )
    .unwrap();
    closed(mesh(&e));
}

#[test]
fn knife_and_loop_apply_on_curved_primitive_surfaces() {
    for kind in [
        PrimitiveKind::Sphere,
        PrimitiveKind::Cylinder,
        PrimitiveKind::Torus,
    ] {
        let original = PrimitiveSettings::new(kind).mesh(2, None).unwrap();
        let graph = MeshConnectivity::new(&original);
        for &seed in graph
            .edges
            .iter()
            .step_by((graph.edges.len() / 5).max(1))
            .take(5)
        {
            let mut e = editor(original.clone());
            let cuts = loop_cut_path(mesh(&e), seed, 0.37).unwrap();
            e.edit_mesh(1, MeshEdit::KnifeStroke { cuts })
                .unwrap_or_else(|err| panic!("{kind:?} {seed:?}: {err}"));
            closed(mesh(&e));
            assert!(
                (volume(mesh(&e)) - volume(&original)).abs() < 0.002,
                "{kind:?} volume changed"
            );
        }
    }
}

#[test]
fn polygon_bevel_on_curved_primitive_faces_stays_closed() {
    for kind in [
        PrimitiveKind::Sphere,
        PrimitiveKind::Cylinder,
        PrimitiveKind::Torus,
    ] {
        let original = PrimitiveSettings::new(kind).mesh(2, None).unwrap();
        for face in (0..original.face_count())
            .step_by((original.face_count() / 5).max(1))
            .take(5)
        {
            let mut e = editor(original.clone());
            e.edit_mesh(
                1,
                MeshEdit::FaceBevel {
                    faces: BTreeSet::from([face]),
                    inset: 0.15,
                    shift: 0.,
                    segments: 4,
                    profile: -0.5,
                },
            )
            .unwrap_or_else(|err| panic!("{kind:?} face {face}: {err}"));
            closed(mesh(&e));
        }
    }
}

#[test]
fn closed_gap_preserves_both_contours_of_a_torus_section() {
    let mut e = editor(
        PrimitiveSettings::new(PrimitiveKind::Torus)
            .mesh(2, None)
            .unwrap(),
    );
    let faces = (0..mesh(&e).face_count()).collect();
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion {
                faces,
                ..Default::default()
            },
            edit: TopologyEdit::Triangulate,
        },
    )
    .unwrap();
    let loose = [[8., 0., 0.], [9., 0., 0.], [10., 1., 0.]];
    let m = &mut e.project.meshes[0];
    let start = m.vertices.len() as u32;
    m.vertices.extend(loose.map(|position| Vertex {
        position,
        ..Default::default()
    }));
    m.topology
        .as_mut()
        .unwrap()
        .loose_edges
        .push([start, start + 1]);
    let before = e.project.clone();
    let before_volume = volume(mesh(&e));
    let graph = MeshConnectivity::new(mesh(&e));
    let seed = *graph
        .edges
        .iter()
        .find(|&&(a, b)| {
            let p = Vec3::from_array(mesh(&e).vertices[a].position);
            let q = Vec3::from_array(mesh(&e).vertices[b].position);
            (q - p).normalize().y.abs() > 0.9
        })
        .unwrap();
    let cuts = loop_cut_path(mesh(&e), seed, 0.37).unwrap();
    e.edit_mesh(1, MeshEdit::KnifeGapClosed { cuts, width: 0.03 })
        .unwrap();
    closed(mesh(&e));
    for point in loose {
        assert!(mesh(&e).vertices.iter().any(|v| v.position == point));
    }
    assert_eq!(mesh(&e).topology.as_ref().unwrap().loose_edges.len(), 1);
    assert!(volume(mesh(&e)) < before_volume && volume(mesh(&e)) > before_volume * 0.8);
    e.undo();
    assert_eq!(e.project, before);
}
