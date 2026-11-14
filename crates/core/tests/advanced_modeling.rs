use poseforge_core::*;
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn editor() -> Editor {
    let mut node = Node::new(1, "Cube");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    })
}
fn custom(points: Vec<[f32; 3]>, faces: Vec<Vec<u32>>) -> Editor {
    let mut e = editor();
    let mesh = &mut e.project.meshes[0];
    mesh.vertices = points
        .into_iter()
        .map(|position| Vertex {
            position,
            ..Default::default()
        })
        .collect();
    let t = mesh.topology.as_mut().unwrap();
    t.faces = faces.into();
    t.primitive = None;
    mesh.rebuild_polygon_indices().unwrap();
    e.project.validate().unwrap();
    e
}
fn evaluate(mesh: &Mesh, settings: ModifierSettings) -> Mesh {
    evaluate_modifier_mesh(
        mesh,
        &[MeshModifier {
            id: 1,
            name: "Test".into(),
            enabled: true,
            settings,
        }],
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn close(a: Vec3, b: Vec3) {
    assert!(a.distance(b) < 0.0001, "{a:?} != {b:?}");
}
fn center(m: &Mesh, copy: usize, count: usize) -> Vec3 {
    m.vertices[copy * count..(copy + 1) * count]
        .iter()
        .map(|v| Vec3::from_array(v.position))
        .sum::<Vec3>()
        / count as f32
}
fn closed(m: &Mesh) {
    m.validate_topology().unwrap();
    let r = inspect_topology(m);
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

#[test]
fn bevel_is_reversible_parameterized_and_preserves_source() {
    for segments in [1, 3, 8] {
        for profile in [-1., -0.5, 0.2, 0.5, 0.8] {
            let mut e = editor();
            let original = e.project.meshes[0].clone();
            let id = e.add_modifier(1, ModifierKind::Bevel).unwrap();
            e.update_modifier(
                1,
                MeshModifier {
                    id,
                    name: "Bevel".into(),
                    enabled: true,
                    settings: ModifierSettings::Bevel {
                        width: 0.1,
                        segments,
                        profile,
                        angle: 0.5,
                    },
                },
            )
            .unwrap();
            let evaluated = e.project.evaluated(None, 0.).unwrap();
            let result = &evaluated.modifier_meshes[&1];
            closed(result);
            assert!(result.face_count() > original.face_count());
            assert_eq!(e.project.meshes[0], original);
            let saved: Project =
                serde_json::from_slice(&serde_json::to_vec(&e.project).unwrap()).unwrap();
            assert_eq!(saved, e.project);
            e.apply_modifiers(1).unwrap();
            closed(&e.project.meshes[0]);
            assert!(e.project.node(1).unwrap().modifiers.is_empty());
            e.undo();
            assert_eq!(e.project.meshes[0], original);
        }
    }
}
#[test]
fn radial_array_has_no_duplicate_full_circle_endpoint_and_supports_arcs() {
    let e = editor();
    let source = &e.project.meshes[0];
    let count = source.vertices.len();
    for (angle, expected) in [
        (
            std::f32::consts::TAU,
            vec![
                Vec3::X * 3.,
                Vec3::NEG_Z * 3.,
                Vec3::NEG_X * 3.,
                Vec3::Z * 3.,
            ],
        ),
        (
            std::f32::consts::PI,
            vec![
                Vec3::X * 3.,
                Vec3::new(1.5, 0., -2.598076),
                Vec3::new(-1.5, 0., -2.598076),
                Vec3::NEG_X * 3.,
            ],
        ),
    ] {
        let m = evaluate(
            source,
            ModifierSettings::RadialArray {
                count: 4,
                axis: 1,
                radius: 3.,
                angle,
                rotate: true,
            },
        );
        assert_eq!(m.face_count(), 24);
        for (i, p) in expected.into_iter().enumerate() {
            close(center(&m, i, count), p);
        }
    }
}
#[test]
fn curve_array_tracks_parent_animation_and_invalidates_cached_frames() {
    let mut e = editor();
    let mut curve = Node::new(3, "Path");
    curve.parent = Some(4);
    curve.curve = Some(CurveData {
        points: vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 0.]],
        interpolation: CurveInterpolation::Polyline,
        ..Default::default()
    });
    e.project.nodes.extend([curve, Node::new(4, "Parent")]);
    let id = e.add_modifier(1, ModifierKind::Array).unwrap();
    e.update_modifier(
        1,
        MeshModifier {
            id,
            name: "Curve array".into(),
            enabled: true,
            settings: ModifierSettings::CurveArray {
                count: 3,
                curve: Some(3),
                spacing: 2.,
                start: 0.,
                roll: 0.,
                follow: false,
            },
        },
    )
    .unwrap();
    let source_count = e.project.meshes[0].vertices.len();
    let first = e.project.evaluated(None, 0.).unwrap();
    for (i, p) in [Vec3::ZERO, Vec3::X * 2., Vec3::new(2., 2., 0.)]
        .into_iter()
        .enumerate()
    {
        close(center(&first.modifier_meshes[&1], i, source_count), p);
    }
    let before = e.project.prepare_modifiers(1, &first.global).unwrap().key;
    let mut clip = Clip::new("Path", 2.);
    clip.key_transform(4, 0., Transform::default());
    clip.key_transform(4, 1., Transform::from_translation(Vec3::Z * 5.));
    e.project.clips.push(clip);
    let moved = e.project.evaluated(Some(0), 1.).unwrap();
    assert_ne!(
        e.project.prepare_modifiers(1, &moved.global).unwrap().key,
        before
    );
    close(
        center(&moved.modifier_meshes[&1], 2, source_count),
        Vec3::new(2., 2., 5.),
    );
    let original = e.project.clone();
    e.transact("Delete path", |p| p.nodes.retain(|n| n.id != 3))
        .unwrap();
    assert!(matches!(
        e.project.node(1).unwrap().modifiers[0].settings,
        ModifierSettings::CurveArray { curve: None, .. }
    ));
    assert!(!e.project.node(1).unwrap().modifiers[0].enabled);
    e.undo();
    assert_eq!(e.project, original);
}
#[test]
fn bevel_after_touching_array_keeps_instances_independent() {
    let e = editor();
    let stack = [
        ModifierSettings::Array {
            count: 2,
            offset: [2., 0., 0.],
        },
        ModifierSettings::Bevel {
            width: 0.08,
            segments: 3,
            profile: 0.5,
            angle: 0.5,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(i, settings)| MeshModifier {
        id: i as u64 + 1,
        name: "M".into(),
        enabled: true,
        settings,
    })
    .collect::<Vec<_>>();
    let result =
        evaluate_modifier_mesh(&e.project.meshes[0], &stack, &AtomicBool::new(false)).unwrap();
    result.validate_topology().unwrap();
    assert!(result.face_count() > 12);
    let one = evaluate(&e.project.meshes[0], stack[1].settings);
    assert_eq!(result.face_count(), one.face_count() * 2);
}
#[test]
fn unequal_bridge_has_manifold_join_and_supports_twist_smoothing() {
    for (n, m) in [(3, 4), (4, 7), (5, 8)] {
        for segments in [1, 3] {
            for twist in [-1, 0, 1] {
                let mut points = vec![];
                for (count, z) in [(n, 0.), (m, 3.)] {
                    for i in 0..count {
                        let a = i as f32 * std::f32::consts::TAU / count as f32;
                        points.push([a.cos(), a.sin(), z]);
                    }
                }
                let mut e = custom(
                    points,
                    vec![
                        (0..n as u32).rev().collect(),
                        (n as u32..(n + m) as u32).collect(),
                    ],
                );
                let original = e.project.clone();
                let edges = MeshConnectivity::new(&e.project.meshes[0]).edges;
                e.edit_mesh(
                    1,
                    MeshEdit::BridgeAdvanced {
                        faces: BTreeSet::new(),
                        edges,
                        segments,
                        twist,
                        smooth: 0.7,
                    },
                )
                .unwrap_or_else(|err| panic!("{n}/{m} {segments} {twist}: {err}"));
                closed(&e.project.meshes[0]);
                e.undo();
                assert_eq!(e.project, original);
                e.redo();
                closed(&e.project.meshes[0]);
            }
        }
    }
}
#[test]
fn knife_stroke_crosses_adjacent_faces_without_t_junctions_and_undoes_atomically() {
    let mut e = custom(
        vec![
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
    let cuts = vec![
        KnifeCut {
            face: 0,
            first: [0., 0.5, 0.],
            second: [1., 0.5, 0.],
        },
        KnifeCut {
            face: 1,
            first: [1., 0.5, 0.],
            second: [2., 0.5, 0.],
        },
    ];
    e.edit_mesh(1, MeshEdit::KnifeStroke { cuts }).unwrap();
    let mesh = &e.project.meshes[0];
    assert_eq!(mesh.face_count(), 4);
    assert_eq!(mesh.vertices.len(), 9);
    let r = inspect_topology(mesh);
    assert_eq!(
        (r.nonmanifold_edges, r.winding_edges, r.degenerate_faces),
        (0, 0, 0)
    );
    e.undo();
    assert_eq!(e.project, original);
    assert!(e
        .edit_mesh(
            1,
            MeshEdit::KnifeStroke {
                cuts: vec![
                    KnifeCut {
                        face: 1,
                        first: [1., 0.5, 0.],
                        second: [2., 0.5, 0.]
                    },
                    KnifeCut {
                        face: 0,
                        // Interior endpoints are valid; an off-surface segment
                        // must still roll back the whole multi-face stroke.
                        first: [0.2, 0.5, 1.],
                        second: [1., 0.5, 1.]
                    }
                ]
            }
        )
        .is_err());
    assert_eq!(e.project, original);
}
#[test]
fn fill_and_triangulate_only_change_selected_regions() {
    let mut e = editor();
    e.edit_mesh(
        1,
        MeshEdit::DeleteComponents {
            vertices: BTreeSet::new(),
            edges: BTreeSet::new(),
            faces: BTreeSet::from([0]),
        },
    )
    .unwrap();
    let before = e.project.clone();
    let report = inspect_topology(&e.project.meshes[0]);
    assert_eq!(report.boundary_edges, 4);
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion {
                edges: report.boundary_edge_ids,
                ..Default::default()
            },
            edit: TopologyEdit::FillHoles,
        },
    )
    .unwrap();
    closed(&e.project.meshes[0]);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion {
                faces: BTreeSet::from([0]),
                ..Default::default()
            },
            edit: TopologyEdit::Triangulate,
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 7);
    closed(&e.project.meshes[0]);
}
#[test]
fn repair_inward_shell_duplicates_and_selected_bad_edges_has_exact_undo() {
    let mut e = editor();
    for f in &mut e.project.meshes[0].topology.as_mut().unwrap().faces {
        f.reverse();
    }
    e.project.meshes[0].rebuild_polygon_indices().unwrap();
    let before = e.project.clone();
    assert_eq!(
        inspect_topology(&e.project.meshes[0])
            .inverted_face_ids
            .len(),
        6
    );
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion::default(),
            edit: TopologyEdit::Repair(RepairKind::Winding),
        },
    )
    .unwrap();
    assert!(inspect_topology(&e.project.meshes[0])
        .inverted_face_ids
        .is_empty());
    closed(&e.project.meshes[0]);
    e.undo();
    assert_eq!(e.project, before);
    let mut e = custom(
        vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 0.]],
        vec![vec![0, 1, 2], vec![3, 1, 2]],
    );
    assert_eq!(
        inspect_topology(&e.project.meshes[0])
            .duplicate_vertex_ids
            .len(),
        2
    );
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion::default(),
            edit: TopologyEdit::Repair(RepairKind::Duplicates),
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 1);
    assert_eq!(e.project.meshes[0].vertices.len(), 3);
    e.undo();
    assert_eq!(e.project, before);
    let mut e = custom(
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., -1., 0.],
            [0., 0., 1.],
        ],
        vec![vec![0, 1, 2], vec![1, 0, 3], vec![0, 1, 4]],
    );
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion {
                faces: BTreeSet::from([2]),
                ..Default::default()
            },
            edit: TopologyEdit::Repair(RepairKind::NonManifold),
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 2);
    assert_eq!(inspect_topology(&e.project.meshes[0]).nonmanifold_edges, 0);
    e.undo();
    assert_eq!(e.project, before);
}
#[test]
fn diagnostics_and_repair_handle_collapsed_edges_without_panics() {
    let mut e = custom(
        vec![[0., 0., 0.], [0., 0., 0.], [1., 0., 0.]],
        vec![vec![0, 1, 2]],
    );
    let report = inspect_topology(&e.project.meshes[0]);
    assert_eq!(report.degenerate_faces, 1);
    e.edit_mesh(
        1,
        MeshEdit::Topology {
            region: MeshRegion::default(),
            edit: TopologyEdit::Repair(RepairKind::Degenerate),
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 0);
}
#[test]
fn separating_shared_mesh_keeps_other_instances_and_world_frame() {
    let mut e = editor();
    let mut other = e.project.nodes[0].clone();
    other.id = 3;
    e.project.nodes.push(other);
    e.project.node_mut(1).unwrap().transform.translation = Vec3::X * 5.;
    let before = e.project.clone();
    let id = e.separate_faces(1, &BTreeSet::from([0, 1])).unwrap();
    assert_eq!(e.project.mesh(2).unwrap(), &before.meshes[0]);
    assert_eq!(
        e.project.node(id).unwrap().transform,
        e.project.node(1).unwrap().transform
    );
    assert_eq!(
        e.project
            .mesh(e.project.node(id).unwrap().mesh.unwrap())
            .unwrap()
            .face_count(),
        2
    );
    e.project.validate().unwrap();
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, before);
}
