use poseforge_core::*;
use std::collections::BTreeSet;
fn editor(mesh: Mesh) -> Editor {
    let mut node = Node::new(1, "Model");
    node.mesh = Some(mesh.id);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![mesh],
        ..Default::default()
    })
}
fn cube() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap()
}
fn custom(points: &[[f32; 3]], faces: Vec<Vec<u32>>) -> Mesh {
    let mut m = cube();
    m.vertices = points
        .iter()
        .enumerate()
        .map(|(i, &position)| Vertex {
            position,
            uv: [i as f32 / 8., 0.5],
            ..Default::default()
        })
        .collect();
    let t = m.topology.as_mut().unwrap();
    t.faces = faces.into();
    t.face_materials = Default::default();
    t.primitive = None;
    m.rebuild_polygon_indices().unwrap();
    m
}
#[test]
fn subdivision_count_is_exact_local_and_stitches_neighbor_edges() {
    for n in [2, 3, 5, 16, 32] {
        let mut e = editor(cube());
        let source = e.project.clone();
        e.edit_mesh(
            1,
            MeshEdit::SubdivideSelectedCount {
                region: MeshRegion {
                    faces: BTreeSet::from([0]),
                    ..Default::default()
                },
                segments: n,
            },
        )
        .unwrap();
        let m = e.editable_mesh(1).unwrap();
        assert_eq!(m.face_count(), 5 + (n * n) as usize);
        let r = inspect_topology(m);
        assert_eq!(
            (
                r.boundary_edges,
                r.nonmanifold_edges,
                r.degenerate_faces,
                r.winding_edges
            ),
            (0, 0, 0, 0),
            "n={n}"
        );
        let quads = (0..m.face_count())
            .filter(|&f| m.face_vertices(f).unwrap().len() == 4)
            .count();
        assert_eq!(quads, (n * n + 1) as usize);
        e.project.validate().unwrap();
        assert!(e.undo());
        assert_eq!(e.project, source);
    }
}
#[test]
fn subdivision_splits_triangle_and_selected_wire_without_touching_other_wire() {
    let source = custom(
        &[[0., 0., 0.], [3., 0., 0.], [0., 3., 0.]],
        vec![vec![0, 1, 2]],
    );
    for n in [3, 4, 7] {
        let mut e = editor(source.clone());
        e.edit_mesh(
            1,
            MeshEdit::SubdivideSelectedCount {
                region: MeshRegion {
                    faces: BTreeSet::from([0]),
                    ..Default::default()
                },
                segments: n,
            },
        )
        .unwrap();
        let m = e.editable_mesh(1).unwrap();
        assert_eq!(m.face_count(), (n * n) as usize);
        assert_eq!(inspect_topology(m).degenerate_faces, 0);
        assert!(m
            .vertices
            .iter()
            .all(|v| v.uv.iter().all(|x| x.is_finite())));
    }
    let mut m = custom(
        &[[0., 0., 0.], [4., 0., 0.], [0., 1., 0.], [4., 1., 0.]],
        vec![],
    );
    m.topology.as_mut().unwrap().loose_edges = vec![[0, 1], [2, 3]];
    let mut e = editor(m);
    e.edit_mesh(
        1,
        MeshEdit::SubdivideSelectedCount {
            region: MeshRegion {
                edges: BTreeSet::from([(0, 1)]),
                ..Default::default()
            },
            segments: 4,
        },
    )
    .unwrap();
    let m = e.editable_mesh(1).unwrap();
    assert_eq!(m.topology.as_ref().unwrap().loose_edges.len(), 5);
    assert!(m.topology.as_ref().unwrap().loose_edges.contains(&[2, 3]));
    for x in [1., 2., 3.] {
        assert!(m.vertices.iter().any(|v| v.position == [x, 0., 0.]));
    }
}
#[test]
fn invalid_subdivision_is_atomic() {
    let mut e = editor(cube());
    let source = e.project.clone();
    for count in [0, 1, 33, u32::MAX] {
        assert!(e
            .edit_mesh(
                1,
                MeshEdit::SubdivideSelectedCount {
                    region: MeshRegion {
                        faces: BTreeSet::from([0]),
                        ..Default::default()
                    },
                    segments: count
                }
            )
            .is_err());
        assert_eq!(e.project, source);
        assert_eq!(e.undo_len(), 0);
    }
}
#[test]
fn merge_array_removes_interior_faces_preserves_uv_and_one_undo() {
    let source = cube();
    let min = source
        .vertices
        .iter()
        .map(|v| v.position[0])
        .fold(f32::INFINITY, f32::min);
    let max = source
        .vertices
        .iter()
        .map(|v| v.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let mut e = editor(source.clone());
    let before = e.project.clone();
    e.begin_edit("Array");
    let operation = || MeshEdit::Array {
        region: MeshRegion {
            faces: (0..source.face_count()).collect(),
            ..Default::default()
        },
        count: 2,
        offset: Vec3::X * (max - min),
        rotation: Vec3::ZERO,
    };
    e.preview_mesh(1, &source, operation()).unwrap();
    assert_eq!(e.editable_mesh(1).unwrap().face_count(), 12);
    let map = e
        .preview_mesh_merged(1, &source, operation(), true)
        .unwrap()
        .unwrap();
    let m = e.editable_mesh(1).unwrap();
    assert_eq!(m.face_count(), 10);
    let r = inspect_topology(m);
    assert_eq!(
        (
            r.boundary_edges,
            r.nonmanifold_edges,
            r.duplicate_faces,
            r.winding_edges
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(map.faces.iter().filter(|f| f.is_none()).count(), 2);
    for original in &source.vertices {
        assert!(m
            .vertices
            .iter()
            .any(|v| v.position == original.position && v.uv == original.uv));
    }
    assert_eq!(e.undo_len(), 0);
    e.commit_edit().unwrap();
    let after = e.project.clone();
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
}
#[test]
fn merge_can_be_toggled_from_frozen_source_without_accumulating_changes() {
    let source = cube();
    let mut e = editor(source.clone());
    let before = e.project.clone();
    e.begin_edit("Array");
    let operation = || MeshEdit::Array {
        region: MeshRegion {
            faces: (0..6).collect(),
            ..Default::default()
        },
        count: 3,
        offset: Vec3::ZERO,
        rotation: Vec3::ZERO,
    };
    let map = e
        .preview_mesh_merged(1, &source, operation(), true)
        .unwrap()
        .unwrap();
    assert_eq!(e.editable_mesh(1).unwrap().face_count(), 6);
    assert!(map.faces.iter().all(Option::is_some));
    e.preview_mesh_merged(1, &source, operation(), false)
        .unwrap();
    assert_eq!(e.editable_mesh(1).unwrap().face_count(), 18);
    e.cancel_edit();
    assert_eq!(e.project, before);
}
#[test]
fn tool_transaction_groups_fields_and_nested_gestures_and_restores_cancel() {
    let mut e = editor(cube());
    let original = e.project.clone();
    e.begin_tool_edit("Transform");
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::X,
        },
    )
    .unwrap();
    e.begin_edit("Drag");
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::Y,
        },
    )
    .unwrap();
    e.commit_edit().unwrap();
    assert_eq!(e.undo_len(), 0);
    assert!(e.has_active_edit());
    e.begin_edit("Cancelled drag");
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::Z,
        },
    )
    .unwrap();
    e.cancel_edit();
    let result = e.project.clone();
    e.commit_tool_edit().unwrap();
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, original);
    e.redo();
    assert_eq!(e.project, result);
    e.begin_tool_edit("Discard");
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::Z,
        },
    )
    .unwrap();
    e.reset_tool_edit();
    assert_eq!(e.project, result);
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::Z,
        },
    )
    .unwrap();
    e.cancel_tool_edit();
    assert_eq!(e.project, result);
    assert_eq!(e.undo_len(), 1);
}
#[test]
fn bevel_width_is_constant_on_both_sides_and_ends_of_a_tapered_solid() {
    let m = custom(
        &[
            [-1., -1., 0.],
            [1., -1., 0.],
            [1., 1., 0.],
            [-1., 1., 0.],
            [-1., -1., 2.],
            [-0.8, -1., 2.],
            [-0.8, 1., 2.],
            [-1., 1., 2.],
        ],
        vec![
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ],
    );
    for width in [0.02, 0.1, 1.] {
        for segments in [1, 3, 8] {
            let mut e = editor(m.clone());
            e.edit_mesh(
                1,
                MeshEdit::EdgeBevel {
                    edges: BTreeSet::from([(0, 4)]),
                    width,
                    segments,
                    profile: 0.5,
                },
            )
            .unwrap();
            let mesh = e.editable_mesh(1).unwrap();
            let mut distances = Vec::new();
            for v in &mesh.vertices {
                let [x, y, z] = v.position;
                if z.abs() < 1e-6 || (z - 2.).abs() < 1e-6 {
                    if (y + 1.).abs() < 1e-6 && x > -0.99999 && x < -0.80001 {
                        distances.push(x + 1.);
                    }
                    if (x + 1.).abs() < 1e-6 && y > -0.99999 && y < -0.80001 {
                        distances.push(y + 1.);
                    }
                }
            }
            assert!(distances.len() >= 4);
            let min = distances.iter().copied().fold(f32::INFINITY, f32::min);
            let max = distances.iter().copied().fold(0., f32::max);
            assert!(
                max - min < 1e-5,
                "width={width}, segments={segments}, offsets={distances:?}"
            );
            let r = inspect_topology(mesh);
            assert_eq!(
                (
                    r.boundary_edges,
                    r.nonmanifold_edges,
                    r.winding_edges,
                    r.degenerate_faces
                ),
                (0, 0, 0, 0)
            );
        }
    }
}
#[test]
fn subdivision_keeps_marked_uv_seams_and_face_materials() {
    for segments in [2, 3, 5] {
        let mut source = cube();
        let ring = source.face_vertices(0).unwrap().to_vec();
        source.topology.as_mut().unwrap().uv_seams = vec![[ring[0], ring[1]]];
        let original_seam = [
            source.vertices[ring[0] as usize].position,
            source.vertices[ring[1] as usize].position,
        ];
        let mut e = editor(source);
        e.project.materials.push(Material {
            id: 10,
            name: "Face".into(),
            ..Default::default()
        });
        e.project.meshes[0]
            .topology
            .as_mut()
            .unwrap()
            .face_materials = vec![Some(10), None, None, None, None, None].into();
        e.edit_mesh(
            1,
            MeshEdit::SubdivideSelectedCount {
                region: MeshRegion {
                    faces: BTreeSet::from([0]),
                    ..Default::default()
                },
                segments,
            },
        )
        .unwrap();
        let m = e.editable_mesh(1).unwrap();
        assert_eq!(
            (0..m.face_count())
                .filter(|&f| m.face_material(f) == Some(10))
                .count(),
            (segments * segments) as usize
        );
        let seams = uv_seams(m);
        assert_eq!(seams.len(), segments as usize);
        let a = Vec3::from_array(original_seam[0]);
        let b = Vec3::from_array(original_seam[1]);
        for (i, j) in seams {
            for v in [i, j] {
                assert!(
                    (Vec3::from_array(m.vertices[v].position) - a)
                        .cross(b - a)
                        .length()
                        < 1e-5
                );
            }
        }
    }
}
#[test]
fn disconnected_bevels_do_not_share_the_smallest_width_limit() {
    let mut points = vec![
        [-1., -1., 0.],
        [1., -1., 0.],
        [1., 1., 0.],
        [-1., 1., 0.],
        [-1., -1., 2.],
        [-0.8, -1., 2.],
        [-0.8, 1., 2.],
        [-1., 1., 2.],
    ];
    points.extend([
        [3., -1., 0.],
        [5., -1., 0.],
        [5., 1., 0.],
        [3., 1., 0.],
        [3., -1., 2.],
        [5., -1., 2.],
        [5., 1., 2.],
        [3., 1., 2.],
    ]);
    let mut faces = vec![
        vec![0, 3, 2, 1],
        vec![4, 5, 6, 7],
        vec![0, 1, 5, 4],
        vec![1, 2, 6, 5],
        vec![2, 3, 7, 6],
        vec![3, 0, 4, 7],
    ];
    faces.extend(
        faces
            .clone()
            .into_iter()
            .map(|f| f.into_iter().map(|v| v + 8).collect()),
    );
    let mut e = editor(custom(&points, faces));
    e.edit_mesh(
        1,
        MeshEdit::EdgeBevel {
            edges: BTreeSet::from([(0, 4), (8, 12)]),
            width: 0.1,
            segments: 1,
            profile: 0.,
        },
    )
    .unwrap();
    let m = e.editable_mesh(1).unwrap();
    for z in [0., 2.] {
        assert!(m
            .vertices
            .iter()
            .any(|v| Vec3::from_array(v.position).distance(Vec3::new(3.1, -1., z)) < 1e-5));
    }
}
