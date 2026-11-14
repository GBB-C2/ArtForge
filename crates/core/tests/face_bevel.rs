use poseforge_core::*;
use std::collections::BTreeSet;

fn editor() -> Editor {
    let mut node = Node::new(1, "Mesh");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    })
}
fn cut_face(e: &mut Editor, position: f32) {
    let mesh = &e.project.meshes[0];
    let ring = mesh.face_vertices(0).unwrap();
    let p = |i: usize| Vec3::from_array(mesh.vertices[ring[i] as usize].position);
    let cuts = vec![KnifeCut {
        face: 0,
        first: p(0).lerp(p(1), position).to_array(),
        second: p(3).lerp(p(2), position).to_array(),
    }];
    e.edit_mesh(1, MeshEdit::KnifeStroke { cuts }).unwrap();
}
fn closed(mesh: &Mesh, original: &Mesh) {
    let report = inspect_topology(mesh);
    assert_eq!(
        (
            report.boundary_edges,
            report.nonmanifold_edges,
            report.winding_edges,
            report.degenerate_faces,
            report.duplicate_faces
        ),
        (0, 0, 0, 0, 0),
        "{report:?}"
    );
    let lo = original
        .vertices
        .iter()
        .map(|v| Vec3::from_array(v.position))
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let hi = original
        .vertices
        .iter()
        .map(|v| Vec3::from_array(v.position))
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    assert!(mesh.vertices.iter().all(|v| {
        let p = Vec3::from_array(v.position);
        p.is_finite()
            && p.cmpge(lo - Vec3::splat(1e-5)).all()
            && p.cmple(hi + Vec3::splat(1e-5)).all()
    }));
}
fn bevel(e: &mut Editor, face: usize, segments: u32, profile: f32) {
    e.edit_mesh(
        1,
        MeshEdit::FaceBevel {
            faces: BTreeSet::from([face]),
            inset: 0.15,
            shift: 0.,
            segments,
            profile,
        },
    )
    .unwrap_or_else(|error| panic!("face {face}, segments {segments}, profile {profile}: {error}"));
}

#[test]
fn polygon_bevel_rounds_new_knife_faces_and_keeps_the_mesh_closed() {
    for position in [0.23, 0.5, 0.77] {
        for face in [0, 1] {
            for segments in [1, 3, 8] {
                for profile in [-1., 0., 1.] {
                    let mut e = editor();
                    cut_face(&mut e, position);
                    let before = e.project.clone();
                    let undo = e.undo_len();
                    bevel(&mut e, face, segments, profile);
                    let result = e.project.clone();
                    assert_ne!(result, before);
                    assert_eq!(e.undo_len(), undo + 1);
                    closed(&e.project.meshes[0], &before.meshes[0]);
                    e.undo();
                    assert_eq!(e.project, before);
                    e.redo();
                    assert_eq!(e.project, result);
                }
            }
        }
    }
}

#[test]
fn edge_bevel_can_end_at_a_new_straight_boundary_vertex() {
    let mut source = editor();
    let original_vertices = source.project.meshes[0].vertices.len();
    cut_face(&mut source, 0.37);
    let mesh = &source.project.meshes[0];
    let graph = MeshConnectivity::new(mesh);
    let seed = *graph
        .edge_faces
        .iter()
        .find(|(&(a, b), faces)| {
            (a >= original_vertices || b >= original_vertices)
                && faces.contains(&0)
                && !faces.contains(&1)
        })
        .unwrap()
        .0;
    for profile in [-1., 0., 1.] {
        let mut e = Editor::new(source.project.clone());
        e.edit_mesh(
            1,
            MeshEdit::EdgeBevel {
                edges: BTreeSet::from([seed]),
                width: 0.05,
                segments: 4,
                profile,
            },
        )
        .unwrap();
        closed(&e.project.meshes[0], &source.project.meshes[0]);
        e.undo();
        assert_eq!(e.project, source.project);
    }
}

#[test]
fn bevel_works_on_faces_created_by_inset_extrude_and_loop_cut() {
    for edit in [
        MeshEdit::Bevel {
            faces: BTreeSet::from([0]),
            inset: 0.25,
            shift: 0.,
        },
        MeshEdit::Extrude {
            faces: BTreeSet::from([0]),
            distance: 0.5,
        },
        MeshEdit::LoopCut {
            edge: (0, 1),
            position: 0.37,
        },
    ] {
        let mut source = editor();
        source.edit_mesh(1, edit).unwrap();
        for face in 6..source.project.meshes[0].face_count() {
            for profile in [-1., 0., 1.] {
                let mut e = Editor::new(source.project.clone());
                bevel(&mut e, face, 4, profile);
                closed(&e.project.meshes[0], &source.project.meshes[0]);
            }
        }
    }
}
