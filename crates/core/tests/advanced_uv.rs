use poseforge_core::*;
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn editor(mesh: Mesh) -> Editor {
    let mut n = Node::new(1, "Mesh");
    n.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![n],
        meshes: vec![mesh],
        ..Default::default()
    })
}
fn primitive(kind: PrimitiveKind) -> Editor {
    editor(PrimitiveSettings::new(kind).mesh(2, None).unwrap())
}
fn custom(points: Vec<[f32; 3]>, faces: Vec<Vec<u32>>) -> Mesh {
    let mut mesh = Mesh {
        id: 2,
        name: "Chart".into(),
        vertices: points
            .into_iter()
            .map(|p| Vertex {
                position: p,
                uv: [p[0], p[1]],
                ..Default::default()
            })
            .collect(),
        indices: Vec::new().into(),
        topology: Some(MeshTopology {
            face_materials: Default::default(),
            faces: faces.into(),
            primitive: None,
            smooth_normals: false,
            uv_seams: Vec::new(),
        }),
        material: None,
        morph_targets: Vec::new(),
    };
    mesh.rebuild_polygon_indices().unwrap();
    recompute_normals(&mut mesh, false);
    mesh
}
fn strip() -> Mesh {
    custom(
        (0..2)
            .flat_map(|y| (0..3).map(move |x| [x as f32, y as f32, 0.]))
            .collect(),
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
}
fn bounds(mesh: &Mesh, island: &[usize]) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for &f in island {
        for &v in mesh.face_vertices(f).unwrap() {
            for k in 0..2 {
                let x = mesh.vertices[v as usize].uv[k];
                min[k] = min[k].min(x);
                max[k] = max[k].max(x);
            }
        }
    }
    (min, max)
}
#[test]
fn unwrap_cube_preserves_geometry_and_packs_distinct_conformal_charts() {
    let mut e = primitive(PrimitiveKind::Cube);
    let before = e.project.clone();
    e.edit_mesh(1, MeshEdit::Uv(UvEdit::Unwrap)).unwrap();
    let m = &e.project.meshes[0];
    m.validate_topology().unwrap();
    let points = |m: &Mesh| {
        m.vertices
            .iter()
            .map(|v| position_key(v.position))
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(points(m), points(&before.meshes[0]));
    assert_eq!(m.face_count(), 6);
    let islands = uv_islands(m);
    assert_eq!(islands.len(), 6);
    let boxes: Vec<_> = islands.iter().map(|i| bounds(m, i)).collect();
    for (i, &(a, b)) in boxes.iter().enumerate() {
        assert!(a.iter().all(|&v| v >= 0.0199));
        assert!(b.iter().all(|&v| v <= 0.9801));
        for &(c, d) in &boxes[i + 1..] {
            assert!(
                b[0] <= c[0] - 0.0199
                    || d[0] <= a[0] - 0.0199
                    || b[1] <= c[1] - 0.0199
                    || d[1] <= a[1] - 0.0199
            );
        }
    }
    for f in 0..m.face_count() {
        assert!(uv_stretch(m, f) < 1.01, "stretch {}", uv_stretch(m, f));
    }
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    e.project.validate().unwrap();
}
#[test]
fn seam_opens_cylindrical_chart_and_survives_serialization() {
    let points = (0..2)
        .flat_map(|y| {
            (0..8).map(move |i| {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                [a.cos(), y as f32, a.sin()]
            })
        })
        .collect();
    let faces = (0..8)
        .map(|i| vec![i, (i + 1) % 8, (i + 1) % 8 + 8, i + 8])
        .collect();
    let mut e = editor(custom(points, faces));
    e.edit_mesh(
        1,
        MeshEdit::Uv(UvEdit::Seams {
            edges: BTreeSet::from([(0, 8)]),
            marked: true,
        }),
    )
    .unwrap();
    e.edit_mesh(1, MeshEdit::Uv(UvEdit::Unwrap)).unwrap();
    let m = &e.project.meshes[0];
    assert_eq!(uv_islands(m).len(), 1);
    assert!(m.vertices.len() > 16);
    for f in 0..8 {
        assert!(uv_stretch(m, f) < 1.02, "face {f}: {}", uv_stretch(m, f));
    }
    let json = serde_json::to_string(&e.project).unwrap();
    let restored: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, e.project);
    assert_eq!(uv_seams(&restored.meshes[0]).len(), 1);
}
#[test]
fn knife_splits_polygons_and_neighbors_without_t_junctions_and_is_one_undo() {
    let mut e = primitive(PrimitiveKind::Cube);
    let original = e.project.clone();
    let m = &e.project.meshes[0];
    let g = MeshConnectivity::new(m);
    let f = m.face_vertices(0).unwrap();
    let edge = |j: usize| {
        let a = g.canonical[f[j] as usize];
        let b = g.canonical[f[(j + 1) % 4] as usize];
        (a.min(b), a.max(b))
    };
    let first = EdgePoint {
        edge: edge(0),
        t: 0.3,
    };
    let second = EdgePoint {
        edge: edge(2),
        t: 0.65,
    };
    e.edit_mesh(
        1,
        MeshEdit::Uv(UvEdit::Seams {
            edges: BTreeSet::from([first.edge]),
            marked: true,
        }),
    )
    .unwrap();
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Knife {
            face: 0,
            first,
            second,
        },
    )
    .unwrap();
    let m = &e.project.meshes[0];
    assert_eq!(m.face_count(), 7);
    assert!(MeshConnectivity::new(m)
        .edge_faces
        .values()
        .all(|f| f.len() == 2));
    assert_eq!(inspect_topology(m).winding_edges, 0);
    assert_eq!(uv_seams(m).len(), 2);
    e.undo();
    assert_eq!(e.project, before);
    e.undo();
    assert_eq!(e.project, original);
    let state = e.project.clone();
    assert!(e
        .edit_mesh(
            1,
            MeshEdit::Knife {
                face: 0,
                first,
                second: first
            }
        )
        .is_err());
    assert_eq!(e.project, state);
}
#[test]
fn slides_follow_existing_surface_rails_and_preserve_topology() {
    for amount in [-0.4, 0.4] {
        let mut e = editor(strip());
        e.edit_mesh(
            1,
            MeshEdit::Slide {
                vertices: BTreeSet::new(),
                edges: BTreeSet::from([(1, 4)]),
                amount,
            },
        )
        .unwrap();
        let m = &e.project.meshes[0];
        assert_eq!(m.face_count(), 2);
        let a = m.vertices[1].position;
        let b = m.vertices[4].position;
        assert!((a[0] - 1.).abs() > 0.39);
        assert!((a[0] - b[0]).abs() < 1e-6);
        assert_eq!(a[2], 0.);
        assert_eq!(b[2], 0.);
        e.project.validate().unwrap();
    }
}
#[test]
fn proportional_weights_are_connected_world_space_and_undoable() {
    let m = strip();
    let selected = BTreeSet::from([0]);
    let w = proportional_weights(&m, &selected, 1.5, Mat4::IDENTITY);
    assert_eq!(w[0], 1.);
    assert!(w[1] > 0. && w[1] < 1.);
    assert_eq!(w[2], 0.);
    let scaled = proportional_weights(&m, &selected, 1.5, Mat4::from_scale(Vec3::splat(2.)));
    assert_eq!(scaled[1], 0.);
    let mut e = editor(m);
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::WeightedTransform {
            weights: w,
            matrix: Mat4::from_translation(Vec3::Y * 0.2),
        },
    )
    .unwrap();
    assert_eq!(
        e.project.meshes[0].vertices[2],
        before.meshes[0].vertices[2]
    );
    e.undo();
    assert_eq!(e.project, before);
}
#[test]
fn seam_indices_remap_after_flattening_deletion_and_subdivision() {
    for operation in [
        MeshEdit::FlatNormals,
        MeshEdit::Subdivide,
        MeshEdit::DeleteFaces(BTreeSet::from([1])),
    ] {
        let mut e = editor(strip());
        e.edit_mesh(
            1,
            MeshEdit::Uv(UvEdit::Seams {
                edges: BTreeSet::from([(0, 1)]),
                marked: true,
            }),
        )
        .unwrap();
        e.edit_mesh(1, operation).unwrap();
        let m = &e.project.meshes[0];
        assert!(!uv_seams(m).is_empty());
        let g = MeshConnectivity::new(m);
        assert!(uv_seams(m).iter().all(|s| g.edges.contains(s)));
        e.project.validate().unwrap();
    }
}
#[test]
fn uv_failure_cancellation_and_shared_instances_are_atomic() {
    let mut e = primitive(PrimitiveKind::Cube);
    let before = e.project.clone();
    for edit in [
        UvEdit::Pack { margin: f32::NAN },
        UvEdit::Transform {
            vertices: BTreeSet::from([usize::MAX]),
            offset: [0.; 2],
            angle: 0.,
            scale: 1.,
        },
    ] {
        assert!(e.edit_mesh(1, MeshEdit::Uv(edit)).is_err());
        assert_eq!(e.project, before);
    }
    let mut mesh = before.meshes[0].clone();
    assert!(edit_uv(&mut mesh, UvEdit::Unwrap, &AtomicBool::new(true)).is_err());
    assert_eq!(mesh, before.meshes[0]);
    let mut other = e.project.nodes[0].clone();
    other.id = 3;
    e.project.nodes.push(other);
    e.edit_mesh(1, MeshEdit::Uv(UvEdit::Unwrap)).unwrap();
    assert_ne!(
        e.project.node(1).unwrap().mesh,
        e.project.node(3).unwrap().mesh
    );
    assert_eq!(e.project.mesh(2).unwrap(), &before.meshes[0]);
}

#[test]
fn packing_wide_islands_respects_both_tile_margins_and_collapsed_uv_is_visible_in_stretch() {
    let mut mesh = strip();
    for v in &mut mesh.vertices {
        v.uv[0] *= 50.;
    }
    edit_uv(
        &mut mesh,
        UvEdit::Pack { margin: 0.02 },
        &AtomicBool::new(false),
    )
    .unwrap();
    for v in &mesh.vertices {
        assert!(
            v.uv.iter().all(|&x| (0.01999..=0.98001).contains(&x)),
            "{:?}",
            v.uv
        );
    }
    for v in &mut mesh.vertices {
        v.uv = [0.; 2];
    }
    assert!(uv_stretch(&mesh, 0) >= 999.);
}
