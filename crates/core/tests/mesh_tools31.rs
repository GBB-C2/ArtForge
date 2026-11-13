use poseforge_core::*;
use std::collections::BTreeSet;
fn editor(mesh: Mesh) -> Editor {
    let mut node = Node::new(1, "Mesh");
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

#[test]
fn concave_corner_uses_its_reflex_angle_when_weighting_normals() {
    let mut m = cube();
    m.vertices = vec![
        [0., 0., 0.],
        [2., 0., 0.],
        [2., 1., 0.],
        [1., 1., 0.],
        [1., 2., 0.],
        [0., 2., 0.],
        [2., 1., 1.],
        [1., 1., 1.],
    ]
    .into_iter()
    .map(|position| Vertex {
        position,
        ..Default::default()
    })
    .collect::<Vec<_>>()
    .into();
    m.topology.as_mut().unwrap().primitive = None;
    m.topology.as_mut().unwrap().faces = vec![vec![0, 1, 2, 3, 4, 5], vec![3, 2, 6, 7]].into();
    m.rebuild_polygon_indices().unwrap();
    let mut e = editor(m);
    normals(
        &mut e,
        BTreeSet::from([0, 1]),
        NormalEdit::Recalculate {
            angle: 180.,
            weighted: false,
        },
    );
    let n = Vec3::from_array(e.project.meshes[0].vertices[3].normal);
    assert!(
        n.distance(Vec3::new(0., -1., 3.).normalize()) < 1e-5,
        "270° polygon corner contributes three times the adjacent 90° corner: {n:?}"
    );
}
fn normals(e: &mut Editor, faces: BTreeSet<usize>, edit: NormalEdit) {
    e.edit_mesh(1, MeshEdit::Normals { faces, edit }).unwrap();
    e.project.validate().unwrap();
}
fn normal(m: &Mesh, f: usize) -> Vec3 {
    let face = m.face_vertices(f).unwrap();
    let p = |i: usize| Vec3::from_array(m.vertices[face[i] as usize].position);
    (1..face.len() - 1)
        .map(|i| (p(i) - p(0)).cross(p(i + 1) - p(0)))
        .sum::<Vec3>()
        .normalize()
}
fn positions(m: &Mesh) -> BTreeSet<[i64; 3]> {
    m.vertices
        .iter()
        .map(|v| position_key(v.position))
        .collect()
}
#[test]
fn angular_and_weighted_normals_use_polygons_and_preserve_geometry() {
    let mut settings = PrimitiveSettings::new(PrimitiveKind::Cube);
    settings.size = [4., 1., 1.];
    let mut e = editor(settings.mesh(2, None).unwrap());
    let original = e.project.clone();
    let all = (0..6).collect();
    normals(
        &mut e,
        all,
        NormalEdit::Recalculate {
            angle: 60.,
            weighted: true,
        },
    );
    let m = &e.project.meshes[0];
    for f in 0..6 {
        for &i in m.face_vertices(f).unwrap() {
            assert!(Vec3::from_array(m.vertices[i as usize].normal).distance(normal(m, f)) < 1e-5);
        }
    }
    normals(
        &mut e,
        (0..6).collect(),
        NormalEdit::Recalculate {
            angle: 180.,
            weighted: false,
        },
    );
    for v in &e.project.meshes[0].vertices {
        assert!(
            Vec3::from_array(v.normal)
                .abs()
                .distance(Vec3::splat(1. / 3_f32.sqrt()))
                < 1e-5
        );
    }
    normals(
        &mut e,
        (0..6).collect(),
        NormalEdit::Recalculate {
            angle: 180.,
            weighted: true,
        },
    );
    for v in &e.project.meshes[0].vertices {
        let n = Vec3::from_array(v.normal).abs();
        assert!(n.x < n.y * 0.3 && (n.y - n.z).abs() < 1e-5);
    }
    assert_eq!(
        positions(&e.project.meshes[0]),
        positions(&original.meshes[0])
    );
    assert!((0..6).all(|f| e.project.meshes[0].face_vertices(f).unwrap().len() == 4));
}
#[test]
fn normal_splits_are_local_and_keep_materials_uv_seams_and_shared_instances() {
    let mut e = editor(cube());
    e.edit_mesh(
        1,
        MeshEdit::WeldVertices {
            vertices: (0..24).collect(),
            distance: 0.000001,
        },
    )
    .unwrap();
    e.project.materials.push(Material {
        id: 3,
        ..Default::default()
    });
    let m = &mut e.project.meshes[0];
    m.topology.as_mut().unwrap().face_materials = vec![Some(3); 6].into();
    let edge = *MeshConnectivity::new(m).edges.iter().next().unwrap();
    m.topology
        .as_mut()
        .unwrap()
        .uv_seams
        .push([edge.0 as u32, edge.1 as u32]);
    let mut instance = e.project.nodes[0].clone();
    instance.id = 8;
    e.project.nodes.push(instance);
    let before = e.project.clone();
    normals(&mut e, BTreeSet::from([0]), NormalEdit::Direction(Vec3::Y));
    let id = e.project.node(1).unwrap().mesh.unwrap();
    let m = e.project.mesh(id).unwrap();
    assert_ne!(id, 2);
    assert_eq!(e.project.mesh(2), Some(&before.meshes[0]));
    assert_eq!(
        m.topology.as_ref().unwrap().face_materials,
        before.meshes[0].topology.as_ref().unwrap().face_materials
    );
    assert_eq!(m.topology.as_ref().unwrap().uv_seams.len(), 1);
    for f in 1..6 {
        let old = before.meshes[0].face_vertices(f).unwrap();
        let new = m.face_vertices(f).unwrap();
        for (&a, &b) in old.iter().zip(new) {
            assert_eq!(
                before.meshes[0].vertices[a as usize],
                m.vertices[b as usize]
            );
        }
    }
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    e.project.validate().unwrap();
}
#[test]
fn flipped_concave_polygon_keeps_its_interior_diagonal_and_corner_data() {
    let mut m = cube();
    m.vertices = vec![[0., 0., 0.], [2., 0., 0.], [0.5, 0.5, 0.], [0., 2., 0.]]
        .into_iter()
        .enumerate()
        .map(|(i, p)| Vertex {
            position: p,
            normal: [0., 0., 1.],
            uv: [i as f32, 0.],
            ..Default::default()
        })
        .collect::<Vec<_>>()
        .into();
    m.topology.as_mut().unwrap().faces = vec![vec![0, 1, 2, 3]].into();
    m.topology.as_mut().unwrap().primitive = None;
    m.rebuild_polygon_indices().unwrap();
    let mut e = editor(m);
    let before = e.project.clone();
    normals(&mut e, BTreeSet::from([0]), NormalEdit::FlipFaces);
    let m = &e.project.meshes[0];
    assert!(normal(m, 0).z < -0.999);
    assert_eq!(m.indices.len(), 6);
    assert_eq!(positions(m), positions(&before.meshes[0]));
    for v in &m.vertices {
        assert!(v.normal[2] < -0.999);
    }
    normals(&mut e, BTreeSet::from([0]), NormalEdit::FlipFaces);
    assert_eq!(e.project.meshes[0].vertices, before.meshes[0].vertices);
    assert_eq!(e.project.meshes[0].indices, before.meshes[0].indices);
}
#[test]
fn invalid_normal_parameters_and_locked_objects_leave_history_untouched() {
    let mut e = editor(cube());
    let before = e.project.clone();
    for edit in [
        NormalEdit::Direction(Vec3::ZERO),
        NormalEdit::Recalculate {
            angle: f32::NAN,
            weighted: true,
        },
        NormalEdit::Rotate(Quat::from_xyzw(0., 0., 0., 0.)),
    ] {
        assert!(e
            .edit_mesh(
                1,
                MeshEdit::Normals {
                    faces: BTreeSet::from([0]),
                    edit
                }
            )
            .is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.undo_len(), 0);
    }
    e.project.node_mut(1).unwrap().locked = true;
    let before = e.project.clone();
    assert!(e
        .edit_mesh(
            1,
            MeshEdit::Normals {
                faces: BTreeSet::from([0]),
                edit: NormalEdit::FlipFaces
            }
        )
        .is_err());
    assert_eq!(e.project, before);
}
#[test]
fn smooth_normals_do_not_join_surfaces_touching_only_at_one_vertex() {
    let mut m = cube();
    m.vertices = vec![
        [0., 0., 0.],
        [1., 0., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [0., -1., 0.],
    ]
    .into_iter()
    .map(|p| Vertex {
        position: p,
        ..Default::default()
    })
    .collect::<Vec<_>>()
    .into();
    m.topology = None;
    m.indices = vec![0, 1, 2, 0, 3, 4].into();
    let mut e = editor(m);
    normals(
        &mut e,
        BTreeSet::from([0, 1]),
        NormalEdit::Recalculate {
            angle: 180.,
            weighted: true,
        },
    );
    let m = &e.project.meshes[0];
    for f in 0..2 {
        for &v in m.face_vertices(f).unwrap() {
            assert!(Vec3::from_array(m.vertices[v as usize].normal).distance(normal(m, f)) < 1e-5);
        }
    }
}
fn strip(bump: bool) -> Mesh {
    let mut m = cube();
    let mut vertices = Vec::new();
    let xs = [0., 0.35, 1.7, 3.];
    for z in 0..3 {
        for (x, px) in xs.iter().enumerate() {
            vertices.push(Vertex {
                position: [
                    *px,
                    if bump && z == 1 && x == 1 { 0.4 } else { 0. },
                    z as f32,
                ],
                normal: [0., 1., 0.],
                uv: [*px / 3., z as f32 / 2.],
                ..Default::default()
            });
        }
    }
    m.vertices = vertices.into();
    m.topology.as_mut().unwrap().primitive = None;
    m.topology.as_mut().unwrap().faces = (0..2)
        .flat_map(|z| {
            (0..3).map(move |x| vec![z * 4 + x, z * 4 + x + 4, z * 4 + x + 5, z * 4 + x + 1])
        })
        .collect::<Vec<_>>()
        .into();
    m.rebuild_polygon_indices().unwrap();
    m
}
fn shape(
    e: &mut Editor,
    vertices: BTreeSet<usize>,
    edges: BTreeSet<(usize, usize)>,
    edit: ShapeEdit,
) -> Result<(), CoreError> {
    e.edit_mesh(
        1,
        MeshEdit::Shape {
            vertices,
            edges,
            edit,
        },
    )
}
#[test]
fn straightening_and_spacing_keep_endpoints_quads_and_uvs_with_single_undo() {
    for edit in [ShapeEdit::Straighten, ShapeEdit::Space] {
        let mut e = editor(strip(matches!(edit, ShapeEdit::Straighten)));
        let before = e.project.clone();
        shape(
            &mut e,
            (4..8).collect(),
            BTreeSet::from([(4, 5), (5, 6), (6, 7)]),
            edit,
        )
        .unwrap();
        let m = &e.project.meshes[0];
        assert_eq!(
            m.vertices[4].position,
            before.meshes[0].vertices[4].position
        );
        assert_eq!(
            m.vertices[7].position,
            before.meshes[0].vertices[7].position
        );
        for (a, b) in m.vertices.iter().zip(&before.meshes[0].vertices) {
            assert_eq!(a.uv, b.uv);
        }
        match edit {
            ShapeEdit::Straighten => {
                assert!(m.vertices[5].position[1].abs() < 1e-6);
            }
            _ => {
                assert!((m.vertices[5].position[0] - 1.).abs() < 1e-5);
                assert!((m.vertices[6].position[0] - 2.).abs() < 1e-5);
            }
        }
        e.project.validate().unwrap();
        assert!((0..6).all(|f| m.face_vertices(f).unwrap().len() == 4));
        assert_eq!(e.undo_len(), 1);
        e.undo();
        assert_eq!(e.project, before);
    }
}
#[test]
fn relaxation_pins_the_boundary_and_rejects_branched_or_closed_alignment() {
    let mut e = editor(strip(true));
    let before = e.project.clone();
    let graph = MeshConnectivity::new(&before.meshes[0]);
    shape(
        &mut e,
        (0..12).collect(),
        BTreeSet::new(),
        ShapeEdit::Relax {
            iterations: 3,
            strength: 0.5,
            pin_boundary: true,
        },
    )
    .unwrap();
    for (&(a, b), faces) in &graph.edge_faces {
        if faces.len() == 1 {
            for i in [a, b] {
                assert_eq!(
                    e.project.meshes[0].vertices[i].position,
                    before.meshes[0].vertices[i].position
                );
            }
        }
    }
    assert!(e.project.meshes[0].vertices[5].position[1] < 0.4);
    e.undo();
    assert_eq!(e.project, before);
    for (v, edges) in [
        ((0..12).collect(), graph.edges),
        (
            BTreeSet::from([0, 1, 4, 5]),
            BTreeSet::from([(0, 1), (1, 5), (4, 5), (0, 4)]),
        ),
    ] {
        assert!(shape(&mut e, v, edges, ShapeEdit::Straighten).is_err());
        assert_eq!(e.project, before);
    }
}
#[test]
fn flatten_moves_uv_duplicates_together_and_rejects_a_collapsed_mesh() {
    let mut m = strip(true);
    let mut duplicate = m.vertices[5];
    duplicate.uv = [0.9, 0.9];
    m.vertices.push(duplicate);
    m.topology.as_mut().unwrap().faces[0][2] = 12;
    m.rebuild_polygon_indices().unwrap();
    let mut e = editor(m);
    shape(
        &mut e,
        (4..8).collect(),
        BTreeSet::new(),
        ShapeEdit::Flatten(1),
    )
    .unwrap();
    assert_eq!(
        e.project.meshes[0].vertices[5].position,
        e.project.meshes[0].vertices[12].position
    );
    assert_eq!(e.project.meshes[0].vertices[12].uv, [0.9, 0.9]);
    let mut e = editor(cube());
    let before = e.project.clone();
    assert!(shape(
        &mut e,
        (0..24).collect(),
        BTreeSet::new(),
        ShapeEdit::Flatten(0)
    )
    .is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), 0);
}
#[test]
fn normal_rotation_survives_project_serialization_and_preserves_isolated_vertices() {
    let mut m = cube();
    m.vertices.push(Vertex {
        position: [10., 10., 10.],
        ..Default::default()
    });
    let mut e = editor(m);
    let original = e.project.clone();
    normals(
        &mut e,
        BTreeSet::from([0]),
        NormalEdit::Rotate(Quat::from_rotation_z(0.3)),
    );
    assert_eq!(
        positions(&e.project.meshes[0]),
        positions(&original.meshes[0])
    );
    let encoded = serde_json::to_string(&e.project).unwrap();
    let decoded: Project = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, e.project);
    decoded.validate().unwrap();
}
