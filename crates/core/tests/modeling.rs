use poseforge_core::*;
use std::collections::{BTreeMap, BTreeSet};
fn editor(kind: PrimitiveKind) -> Editor {
    let mut n = Node::new(1, "Mesh");
    n.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(kind).mesh(2, None).unwrap()],
        ..Default::default()
    })
}
fn closed_quads(m: &Mesh) {
    let graph = MeshConnectivity::new(m);
    assert!(graph.edge_faces.values().all(|f| f.len() == 2));
    assert!((0..m.face_count()).all(|f| m.face_vertices(f).unwrap().len() == 4));
    for t in m.indices.as_chunks::<3>().0 {
        let p = |i: u32| Vec3::from_array(m.vertices[i as usize].position);
        assert!((p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).length() > 1e-8);
    }
    m.validate_topology().unwrap();
}
fn seamless(m: &Mesh) {
    let mut normal = BTreeMap::new();
    for v in &m.vertices {
        let n = Vec3::from_array(v.normal);
        if let Some(previous) = normal.insert(position_key(v.position), n) {
            assert!((n - previous).length() < 1e-5);
        }
    }
}
#[test]
fn smooth_sphere_keeps_normals_on_translation_and_seams_after_deformation_and_reload() {
    let mut e = editor(PrimitiveKind::Sphere);
    let before = e.project.meshes[0].clone();
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: (0..before.vertices.len()).collect(),
            delta: Vec3::X * 0.25,
        },
    )
    .unwrap();
    assert!(e.project.meshes[0]
        .vertices
        .iter()
        .zip(&before.vertices)
        .all(|(a, b)| a.normal == b.normal));
    assert!(e.project.meshes[0].smooth_normals_enabled());
    e.edit_mesh(
        1,
        MeshEdit::Transform {
            vertices: BTreeSet::from([0]),
            matrix: Mat4::from_translation(Vec3::Y * 0.03),
        },
    )
    .unwrap();
    seamless(&e.project.meshes[0]);
    let p: Project = serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
    assert!(p.meshes[0].smooth_normals_enabled());
    let mut e = Editor::new(p);
    e.edit_mesh(1, MeshEdit::Subdivide).unwrap();
    seamless(&e.project.meshes[0]);
}
#[test]
fn shading_commands_preserve_generator_settings_and_undo() {
    let mut e = editor(PrimitiveKind::Cube);
    let before = e.project.clone();
    e.edit_mesh(1, MeshEdit::SmoothNormals).unwrap();
    assert!(
        e.project.meshes[0]
            .topology
            .as_ref()
            .unwrap()
            .primitive
            .unwrap()
            .smooth
    );
    e.edit_mesh(1, MeshEdit::FlatNormals).unwrap();
    assert!(
        !e.project.meshes[0]
            .topology
            .as_ref()
            .unwrap()
            .primitive
            .unwrap()
            .smooth
    );
    e.undo();
    e.undo();
    assert_eq!(e.project, before);
}
#[test]
fn legacy_triangle_smoothing_survives_edits_without_changing_subdivision_semantics() {
    let mut e = editor(PrimitiveKind::Cube);
    e.project.meshes[0] = box_mesh(2, "Legacy", Vec3::ONE, None);
    let original = e.project.clone();
    let faces = e.project.meshes[0].face_count();
    e.edit_mesh(1, MeshEdit::SmoothNormals).unwrap();
    let normals: Vec<_> = e.project.meshes[0]
        .vertices
        .iter()
        .map(|v| v.normal)
        .collect();
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: (0..e.project.meshes[0].vertices.len()).collect(),
            delta: Vec3::X * 0.2,
        },
    )
    .unwrap();
    assert_eq!(
        e.project.meshes[0]
            .vertices
            .iter()
            .map(|v| v.normal)
            .collect::<Vec<_>>(),
        normals
    );
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::Y * 0.1,
        },
    )
    .unwrap();
    seamless(&e.project.meshes[0]);
    e.edit_mesh(1, MeshEdit::Subdivide).unwrap();
    let mesh = &e.project.meshes[0];
    assert!(mesh.smooth_normals_enabled());
    assert_eq!(mesh.face_count(), faces * 4);
    assert!((0..mesh.face_count()).all(|f| mesh.face_vertices(f).unwrap().len() == 3));
    seamless(mesh);
    let loaded: Project = serde_json::from_slice(&serde_json::to_vec(&e.project).unwrap()).unwrap();
    assert!(loaded.meshes[0].smooth_normals_enabled());
    e.edit_mesh(1, MeshEdit::FlatNormals).unwrap();
    assert!(!e.project.meshes[0].smooth_normals_enabled());
    for _ in 0..5 {
        e.undo();
    }
    assert_eq!(e.project, original);
}
#[test]
fn individual_bevel_and_inset_keep_closed_quads_and_selected_caps() {
    for faces in [BTreeSet::from([0]), BTreeSet::from([0, 2])] {
        for shift in [0., 0.2, -0.1] {
            let mut e = editor(PrimitiveKind::Cube);
            let before = e.project.clone();
            e.edit_mesh(
                1,
                MeshEdit::Bevel {
                    faces: faces.clone(),
                    inset: 0.2,
                    shift,
                },
            )
            .unwrap();
            assert_eq!(e.project.meshes[0].face_count(), 6 + faces.len() * 4);
            closed_quads(&e.project.meshes[0]);
            e.project.validate().unwrap();
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            closed_quads(&e.project.meshes[0]);
        }
    }
}
#[test]
fn loop_slice_is_continuous_across_seams_for_every_cube_edge_and_sphere() {
    for kind in [PrimitiveKind::Cube, PrimitiveKind::Sphere] {
        let mut e = editor(kind);
        let before = e.project.clone();
        let edges = MeshConnectivity::new(&e.project.meshes[0]).edges;
        for edge in edges
            .into_iter()
            .take(if kind == PrimitiveKind::Cube { 12 } else { 8 })
        {
            for position in [0.25, 0.5, 0.75] {
                e.edit_mesh(1, MeshEdit::LoopCut { edge, position })
                    .unwrap();
                assert!(e.project.meshes[0].face_count() > before.meshes[0].face_count());
                closed_quads(&e.project.meshes[0]);
                if kind == PrimitiveKind::Cube {
                    assert_eq!(e.project.meshes[0].face_count(), 10);
                }
                e.undo();
                assert_eq!(e.project, before);
            }
        }
    }
    let mut e = editor(PrimitiveKind::Plane);
    e.edit_mesh(
        1,
        MeshEdit::LoopCut {
            edge: (0, 1),
            position: 0.3,
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 2);
}
#[test]
fn modeling_rejects_invalid_parameters_without_partial_changes() {
    let mut e = editor(PrimitiveKind::Cube);
    let before = e.project.clone();
    for op in [
        MeshEdit::Bevel {
            faces: BTreeSet::from([99]),
            inset: 0.2,
            shift: 0.1,
        },
        MeshEdit::Bevel {
            faces: BTreeSet::from([0]),
            inset: 1.,
            shift: 0.,
        },
        MeshEdit::LoopCut {
            edge: (0, 99),
            position: 0.5,
        },
        MeshEdit::LoopCut {
            edge: (0, 1),
            position: f32::NAN,
        },
        MeshEdit::Transform {
            vertices: BTreeSet::from([0]),
            matrix: Mat4::from_scale(Vec3::ZERO),
        },
    ] {
        assert!(e.edit_mesh(1, op).is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.undo_len(), 0);
    }
    e.project.meshes[0] = box_mesh(2, "Triangles", Vec3::ONE, None);
    let before = e.project.clone();
    assert!(e
        .edit_mesh(
            1,
            MeshEdit::LoopCut {
                edge: (0, 1),
                position: 0.5
            }
        )
        .is_err());
    assert_eq!(e.project, before);
}
#[test]
fn component_preview_is_absolute_detaches_instances_and_has_one_undo_or_noop() {
    let mut e = editor(PrimitiveKind::Cube);
    let mut n = e.project.nodes[0].clone();
    n.id = 3;
    e.project.nodes.push(n);
    let source = e.project.meshes[0].clone();
    let before = e.project.clone();
    let vertices = (0..source.vertices.len()).collect::<BTreeSet<_>>();
    let operation = |x| MeshEdit::Transform {
        vertices: vertices.clone(),
        matrix: Mat4::from_translation(Vec3::X * x),
    };
    assert!(e.preview_mesh(1, &source, operation(1.)).is_err());
    e.begin_edit("Move");
    for x in [0.1, 0.5, 1.] {
        e.preview_mesh(1, &source, operation(x)).unwrap();
        assert_eq!(e.undo_len(), 0);
        assert_eq!(e.project.mesh(2), Some(&source));
    }
    let id = e.project.node(1).unwrap().mesh.unwrap();
    assert_ne!(id, 2);
    assert_eq!(
        e.project.mesh(id).unwrap().vertices[0].position[0],
        source.vertices[0].position[0] + 1.
    );
    e.cancel_edit();
    assert_eq!(e.project, before);
    e.begin_edit("Move");
    e.preview_mesh(1, &source, operation(2.)).unwrap();
    e.commit_edit().unwrap();
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, before);
    e.begin_edit("No op");
    e.preview_mesh(1, &source, operation(1.)).unwrap();
    e.preview_mesh(1, &source, operation(0.)).unwrap();
    e.commit_edit().unwrap();
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), 0);
}

#[test]
fn subdivision_interpolates_vertex_colors_and_undo_restores_them() {
    for polygonal in [true, false] {
        let mut e = editor(PrimitiveKind::Cube);
        let mesh = &mut e.project.meshes[0];
        if !polygonal {
            mesh.topology = None;
        }
        for vertex in &mut mesh.vertices {
            vertex.color = [
                vertex.position[0] * 0.5 + 0.5,
                vertex.position[1] * 0.5 + 0.5,
                vertex.position[2] * 0.5 + 0.5,
                0.4,
            ];
        }
        let before = e.project.clone();
        e.edit_mesh(1, MeshEdit::Subdivide).unwrap();
        assert!(e.project.meshes[0].vertices.len() > before.meshes[0].vertices.len());
        for vertex in &e.project.meshes[0].vertices {
            for i in 0..3 {
                assert!((vertex.color[i] - (vertex.position[i] * 0.5 + 0.5)).abs() < 1e-5);
            }
            assert!((vertex.color[3] - 0.4).abs() < 1e-5);
        }
        let subdivided = e.project.clone();
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, subdivided);
    }
}
