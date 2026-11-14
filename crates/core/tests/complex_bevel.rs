use poseforge_core::*;
use std::collections::BTreeSet;

fn editor(mut mesh: Mesh) -> Editor {
    mesh.id = 2;
    let mut node = Node::new(1, "Mesh");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![mesh],
        ..Default::default()
    })
}
fn bevel(e: &mut Editor, faces: BTreeSet<usize>, profile: f32) -> Result<(), CoreError> {
    e.edit_mesh(
        1,
        MeshEdit::FaceBevel {
            faces,
            inset: 0.15,
            shift: 0.,
            segments: 3,
            profile,
        },
    )
}
#[test]
fn complex_faces_and_new_topology_support_signed_segmented_bevels() {
    let mut failures = Vec::new();
    for kind in [
        PrimitiveKind::Cube,
        PrimitiveKind::Sphere,
        PrimitiveKind::Cylinder,
        PrimitiveKind::Torus,
        PrimitiveKind::Cone,
        PrimitiveKind::Capsule,
        PrimitiveKind::Fox,
    ] {
        let original = PrimitiveSettings::new(kind).mesh(2, None).unwrap();
        let n = original.face_count();
        for (name, faces) in [
            ("one", BTreeSet::from([0])),
            ("adjacent", BTreeSet::from([0, 1])),
            ("all", (0..n).collect()),
            ("every second", (0..n).step_by(2).collect()),
        ] {
            for profile in [-0.75, 0., 0.75] {
                let mut e = editor(original.clone());
                match bevel(&mut e, faces.clone(), profile) {
                    Ok(()) => {
                        let r = inspect_topology(&e.project.meshes[0]);
                        if r.nonmanifold_edges
                            + r.boundary_edges
                            + r.degenerate_faces
                            + r.winding_edges
                            + r.duplicate_faces
                            > 0
                        {
                            failures.push(format!("{kind:?} {name} {profile}: boundaries {} branches {} degenerate {}",r.boundary_edges,r.nonmanifold_edges,r.degenerate_faces));
                        }
                    }
                    Err(error) => failures.push(format!("{kind:?} {name} {profile}: {error}")),
                }
            }
        }
    }
    for edit in [
        MeshEdit::Subdivide,
        MeshEdit::Topology {
            region: MeshRegion {
                faces: (0..6).collect(),
                ..Default::default()
            },
            edit: TopologyEdit::Triangulate,
        },
    ] {
        let mut source = editor(
            PrimitiveSettings::new(PrimitiveKind::Cube)
                .mesh(2, None)
                .unwrap(),
        );
        source.edit_mesh(1, edit).unwrap();
        let original = source.project.meshes[0].clone();
        for face in 0..original.face_count() {
            let mut e = editor(original.clone());
            if let Err(error) = bevel(&mut e, BTreeSet::from([face]), 0.) {
                failures.push(format!(
                    "tessellated cube ({} faces) face {face}: {error}",
                    original.face_count()
                ));
            }
        }
    }
    for (name, edit) in [
        (
            "extrude",
            MeshEdit::Extrude {
                faces: BTreeSet::from([0]),
                distance: 0.5,
            },
        ),
        (
            "inset",
            MeshEdit::Bevel {
                faces: BTreeSet::from([0]),
                inset: 0.25,
                shift: 0.,
            },
        ),
        (
            "bevel",
            MeshEdit::FaceBevel {
                faces: BTreeSet::from([0]),
                inset: 0.15,
                shift: 0.,
                segments: 3,
                profile: 0.,
            },
        ),
        (
            "loop",
            MeshEdit::LoopCut {
                edge: (0, 1),
                position: 0.5,
            },
        ),
    ] {
        let mut source = editor(
            PrimitiveSettings::new(PrimitiveKind::Cube)
                .mesh(2, None)
                .unwrap(),
        );
        source.edit_mesh(1, edit).unwrap();
        let original = source.project.meshes[0].clone();
        for face in 0..original.face_count() {
            for profile in [-0.75, 0., 0.75] {
                let mut e = editor(original.clone());
                if let Err(error) = bevel(&mut e, BTreeSet::from([face]), profile) {
                    failures.push(format!(
                        "after {name} face {face} profile {profile}: {error}"
                    ));
                }
            }
        }
    }
    let mut cut = editor(
        PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap(),
    );
    let mesh = &cut.project.meshes[0];
    let ring = mesh.face_vertices(0).unwrap();
    let p = |i: usize| Vec3::from_array(mesh.vertices[ring[i] as usize].position);
    let cuts = vec![KnifeCut {
        face: 0,
        first: p(0).lerp(p(1), 0.37).to_array(),
        second: p(2).lerp(p(3), 0.63).to_array(),
    }];
    cut.edit_mesh(1, MeshEdit::KnifeStroke { cuts }).unwrap();
    let original = cut.project.meshes[0].clone();
    for face in 0..original.face_count() {
        let mut e = editor(original.clone());
        if let Err(error) = bevel(&mut e, BTreeSet::from([face]), 0.) {
            failures.push(format!("after single face knife face {face}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn oversized_edge_bevel_clamps_to_neighbours_without_abandoning_the_edit() {
    let mut e = editor(
        PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap(),
    );
    let source = e.project.clone();
    let edge = *MeshConnectivity::new(&source.meshes[0])
        .edges
        .first()
        .unwrap();
    e.edit_mesh(
        1,
        MeshEdit::EdgeBevel {
            edges: BTreeSet::from([edge]),
            width: 5.,
            segments: 5,
            profile: 0.75,
        },
    )
    .unwrap();
    let r = inspect_topology(&e.project.meshes[0]);
    assert_eq!(
        (r.boundary_edges, r.nonmanifold_edges, r.degenerate_faces),
        (0, 0, 0)
    );
    assert_ne!(e.project, source);
    e.undo();
    assert_eq!(e.project, source);
}
#[test]
fn open_concave_face_bevel_does_not_require_a_closed_edge_neighbourhood() {
    let mut m = PrimitiveSettings::new(PrimitiveKind::Plane)
        .mesh(2, None)
        .unwrap();
    m.vertices = [
        [0., 0., 0.],
        [3., 0., 0.],
        [3., 3., 0.],
        [2., 3., 0.],
        [2., 1., 0.],
        [1., 1., 0.],
        [1., 3., 0.],
        [0., 3., 0.],
    ]
    .into_iter()
    .map(|position| Vertex {
        position,
        ..Default::default()
    })
    .collect();
    m.topology.as_mut().unwrap().faces = vec![(0..8).collect()].into();
    m.topology.as_mut().unwrap().primitive = None;
    m.rebuild_polygon_indices().unwrap();
    for profile in [-1., 0., 1.] {
        let mut e = editor(m.clone());
        e.edit_mesh(
            1,
            MeshEdit::FaceBevel {
                faces: BTreeSet::from([0]),
                inset: 0.2,
                shift: 0.1,
                segments: 5,
                profile,
            },
        )
        .unwrap();
        let r = inspect_topology(&e.project.meshes[0]);
        assert_eq!(
            (r.nonmanifold_edges, r.degenerate_faces, r.duplicate_faces),
            (0, 0, 0)
        );
        assert_eq!(r.boundary_edges, 8);
        assert_ne!(e.project.meshes[0], m);
        e.undo();
        assert_eq!(e.project.meshes[0], m);
    }
}
