//! Reference cases were exercised with Blender 4.3.2's mesh.extrude_manifold.
use poseforge_core::*;
use std::collections::BTreeSet;

fn editor() -> Editor {
    let mut node = Node::new(1, "Model");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    })
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
fn vertex_count(m: &Mesh) -> usize {
    MeshConnectivity::new(m)
        .canonical
        .into_iter()
        .collect::<BTreeSet<_>>()
        .len()
}
fn clean(m: &Mesh, boundaries: usize) {
    let r = inspect_topology(m);
    assert_eq!(
        (
            r.boundary_edges,
            r.nonmanifold_edges,
            r.winding_edges,
            r.degenerate_faces,
            r.duplicate_faces
        ),
        (boundaries, 0, 0, 0, 0),
        "{r:?}"
    );
}

#[test]
fn manifold_matches_blender_for_both_directions_and_passing_the_opposite_edge() {
    for distance in [-0.4, 0.4, -2.4] {
        let mut e = editor();
        let source = e.project.clone();
        let faces = top(mesh(&e));
        e.edit_mesh(1, MeshEdit::ExtrudeManifold { faces, distance })
            .unwrap();
        clean(mesh(&e), 0);
        assert_eq!((vertex_count(mesh(&e)), mesh(&e).face_count()), (8, 6));
        assert!((volume(mesh(&e)) - 4. * (2. + distance).abs()).abs() < 1e-4);
        assert_eq!(e.undo_len(), 1);
        assert!(e.undo());
        assert_eq!(e.project, source);
        assert!(e.redo());
        clean(mesh(&e), 0);
    }
}

#[test]
fn manifold_at_the_opposite_face_keeps_one_cap_without_overlap() {
    let mut e = editor();
    let faces = top(mesh(&e));
    e.edit_mesh(
        1,
        MeshEdit::ExtrudeManifold {
            faces,
            distance: -2.,
        },
    )
    .unwrap();
    clean(mesh(&e), 4);
    assert_eq!((vertex_count(mesh(&e)), mesh(&e).face_count()), (4, 1));
    assert!(mesh(&e)
        .vertices
        .iter()
        .all(|v| (v.position[1] + 1.).abs() < 1e-5));
}

#[test]
fn manifold_preserves_the_surrounding_grid_like_blender() {
    for distance in [-0.4, 0.4] {
        let mut e = editor();
        e.edit_mesh(1, MeshEdit::Subdivide).unwrap();
        let before = e.project.clone();
        let faces = top(mesh(&e))
            .into_iter()
            .filter(|&f| {
                let ring = mesh(&e).face_vertices(f).unwrap();
                let sum: Vec3 = ring
                    .iter()
                    .map(|&v| Vec3::from_array(mesh(&e).vertices[v as usize].position))
                    .sum();
                sum.x > 0. && sum.z > 0.
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(faces.len(), 1);
        e.edit_mesh(1, MeshEdit::ExtrudeManifold { faces, distance })
            .unwrap();
        clean(mesh(&e), 0);
        assert_eq!((vertex_count(mesh(&e)), mesh(&e).face_count()), (29, 26));
        assert!((volume(mesh(&e)) - (8. + distance)).abs() < 1e-4);
        assert!(e.undo());
        assert_eq!(e.project, before);
    }
}

#[test]
fn manifold_clips_oblique_sweeps_for_adjacent_faces_without_rejecting_inward_motion() {
    for distance in [-0.4, 0.4] {
        let mut e = editor();
        let mut faces = top(mesh(&e));
        faces.extend((0..mesh(&e).face_count()).filter(|&f| {
            mesh(&e)
                .face_vertices(f)
                .unwrap()
                .iter()
                .all(|&v| mesh(&e).vertices[v as usize].position[0] > 0.99)
        }));
        e.edit_mesh(1, MeshEdit::ExtrudeManifold { faces, distance })
            .unwrap();
        clean(mesh(&e), 0);
        assert!((volume(mesh(&e)) - (8. + 4. * 2_f32.sqrt() * distance)).abs() < 1e-4);
    }
}

#[test]
fn manifold_keeps_material_boundaries_and_cap_corner_attributes() {
    for distance in [-0.4, 0.4] {
        let mut e = editor();
        let faces = top(mesh(&e));
        let top = *faces.first().unwrap();
        for f in 0..6 {
            e.project.materials.push(Material {
                id: 100 + f,
                ..Default::default()
            });
        }
        e.project.meshes[0]
            .topology
            .as_mut()
            .unwrap()
            .face_materials = (100..106).map(Some).collect();
        let corners: Vec<_> = mesh(&e)
            .face_vertices(top)
            .unwrap()
            .iter()
            .map(|&v| mesh(&e).vertices[v as usize])
            .collect();
        e.edit_mesh(1, MeshEdit::ExtrudeManifold { faces, distance })
            .unwrap();
        clean(mesh(&e), 0);
        e.project.validate().unwrap();
        assert_eq!(
            (0..mesh(&e).face_count())
                .filter_map(|f| mesh(&e).face_material(f))
                .collect::<BTreeSet<_>>(),
            (100..106).collect()
        );
        for corner in corners {
            let position = Vec3::from_array(corner.position) + Vec3::Y * distance;
            assert!(mesh(&e)
                .vertices
                .iter()
                .any(
                    |v| Vec3::from_array(v.position).distance_squared(position) < 1e-10
                        && v.uv == corner.uv
                        && v.color == corner.color
                ));
        }
    }
}
