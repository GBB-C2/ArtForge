use poseforge_core::*;
use std::collections::{BTreeMap, BTreeSet};

fn editor(kind: PrimitiveKind) -> Editor {
    let mesh = PrimitiveSettings::new(kind).mesh(2, None).unwrap();
    let mut node = Node::new(1, "Primitive");
    node.mesh = Some(2);
    Editor::new(Project {
        meshes: vec![mesh],
        nodes: vec![node],
        ..Project::default()
    })
}
#[test]
fn primitives_have_quad_boundaries_outward_winding_and_closed_sphere_without_poles() {
    for kind in [
        PrimitiveKind::Cube,
        PrimitiveKind::Plane,
        PrimitiveKind::Sphere,
        PrimitiveKind::Cloth,
    ] {
        let e = editor(kind);
        let m = &e.project.meshes[0];
        e.project.validate().unwrap();
        let mut edges = BTreeMap::new();
        let mut points = BTreeSet::new();
        for face in 0..m.face_count() {
            let f = m.face_vertices(face).unwrap();
            assert_eq!(f.len(), 4);
            for i in 0..4 {
                let a = position_key(m.vertices[f[i] as usize].position);
                let b = position_key(m.vertices[f[(i + 1) % 4] as usize].position);
                assert_ne!(a, b);
                points.insert(a);
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
            assert_eq!(m.triangle_face(face * 2), Some(face));
            assert_eq!(m.triangle_face(face * 2 + 1), Some(face));
        }
        for t in m.indices.as_chunks::<3>().0 {
            let p = |i: u32| Vec3::from_array(m.vertices[i as usize].position);
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            assert!(n.length() > 1e-8);
            if matches!(kind, PrimitiveKind::Cube | PrimitiveKind::Sphere) {
                assert!(n.dot(p(t[0])) > 0.);
            }
        }
        if matches!(kind, PrimitiveKind::Cube | PrimitiveKind::Sphere) {
            assert!(edges.values().all(|n| *n == 2));
            assert_eq!(points.len() + m.face_count(), edges.len() + 2);
        }
        if kind == PrimitiveKind::Sphere {
            assert!(m
                .vertices
                .iter()
                .all(|v| (Vec3::from_array(v.position).length() - 0.5).abs() < 1e-5));
        }
    }
}
#[test]
fn quad_extrusion_delete_subdivide_and_flat_normals_preserve_whole_faces_and_undo() {
    let mut e = editor(PrimitiveKind::Cube);
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Extrude {
            faces: BTreeSet::from([0]),
            distance: 0.4,
        },
    )
    .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 10);
    assert!(e.project.meshes[0]
        .topology
        .as_ref()
        .unwrap()
        .primitive
        .is_none());
    e.edit_mesh(1, MeshEdit::Subdivide).unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 40);
    e.edit_mesh(1, MeshEdit::DeleteFaces(BTreeSet::from([1])))
        .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 39);
    e.edit_mesh(1, MeshEdit::FlatNormals).unwrap();
    let m = &e.project.meshes[0];
    for f in &m.topology.as_ref().unwrap().faces {
        assert_eq!(f.len(), 4);
        assert!(f
            .iter()
            .all(|&i| m.vertices[i as usize].normal == m.vertices[f[0] as usize].normal));
    }
    e.project.validate().unwrap();
    while e.undo() {}
    assert_eq!(e.project, before);
    while e.redo() {}
    e.project.validate().unwrap();
}
#[test]
fn quad_edits_weld_seams_detach_instances_and_reject_invalid_changes_atomically() {
    let mut e = editor(PrimitiveKind::Cube);
    let mut other = e.project.nodes[0].clone();
    other.id = 3;
    e.project.nodes.push(other);
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::X * 0.25,
        },
    )
    .unwrap();
    let mesh = e
        .project
        .mesh(e.project.node(1).unwrap().mesh.unwrap())
        .unwrap();
    assert_ne!(mesh.id, 2);
    assert_eq!(mesh.face_count(), 6);
    assert_eq!(e.project.mesh(2), before.mesh(2));
    let changed = mesh
        .vertices
        .iter()
        .zip(&before.meshes[0].vertices)
        .filter(|(a, b)| a.position != b.position)
        .count();
    assert_eq!(
        changed, 3,
        "all three copies of the cube corner move together"
    );
    let after = e.project.clone();
    let count = e.undo_len();
    for op in [
        MeshEdit::DeleteFaces((0..6).collect()),
        MeshEdit::Extrude {
            faces: BTreeSet::from([99]),
            distance: 1.,
        },
        MeshEdit::Translate {
            vertices: BTreeSet::from([0]),
            delta: Vec3::splat(f32::NAN),
        },
    ] {
        assert!(e.edit_mesh(1, op).is_err());
        assert_eq!(e.project, after);
        assert_eq!(e.undo_len(), count);
    }
}
#[test]
fn parameter_limits_keep_generated_meshes_editable_and_invalid_topology_is_rejected() {
    for kind in [
        PrimitiveKind::Cube,
        PrimitiveKind::Plane,
        PrimitiveKind::Sphere,
        PrimitiveKind::Cloth,
    ] {
        let mut p = PrimitiveSettings::new(kind);
        p.segments = [p.max_segments(); 3];
        p.smooth = false;
        let m = p.mesh(2, None).unwrap();
        assert!(m.vertices.len() <= MAX_EDIT_VERTICES);
        if matches!(kind, PrimitiveKind::Cube | PrimitiveKind::Sphere) {
            let graph = MeshConnectivity::new(&m);
            assert!(graph.edge_faces.values().all(|faces| faces.len() == 2));
            assert_eq!(graph.edges.len(), m.face_count() * 2);
        }
        assert!(m.indices.len() / 3 <= MAX_EDIT_TRIANGLES);
        m.validate_topology().unwrap();
        p.segments[0] += 1;
        assert!(p.mesh(2, None).is_err());
    }
    let mut e = editor(PrimitiveKind::Cube);
    e.project.meshes[0].topology.as_mut().unwrap().faces[0].pop();
    assert!(e.project.validate().is_err());
    let mut p = PrimitiveSettings::new(PrimitiveKind::Cube);
    p.size[0] = f32::NAN;
    assert!(p.mesh(1, None).is_err());
}

#[test]
fn fox_is_symmetric_closed_quad_surface_with_bounded_detail_and_usable_normals() {
    for level in 1..=4 {
        for smooth in [false, true] {
            let mut settings = PrimitiveSettings::new(PrimitiveKind::Fox);
            settings.segments[0] = level;
            settings.smooth = smooth;
            let mesh = settings.mesh(2, None).unwrap();
            assert_eq!(mesh.face_count(), 168 * 4usize.pow(level - 1));
            mesh.validate_topology().unwrap();
            let connectivity = MeshConnectivity::new(&mesh);
            assert!(connectivity
                .edge_faces
                .values()
                .all(|faces| faces.len() == 2));
            let mut oriented_edges = BTreeMap::new();
            for face in &mesh.topology.as_ref().unwrap().faces {
                for i in 0..4 {
                    let a = position_key(mesh.vertices[face[i] as usize].position);
                    let b = position_key(mesh.vertices[face[(i + 1) % 4] as usize].position);
                    *oriented_edges.entry((a.min(b), a.max(b))).or_insert(0i32) +=
                        if a < b { 1 } else { -1 };
                }
            }
            assert!(
                oriented_edges.values().all(|&n| n == 0),
                "consistent face winding"
            );
            assert!(mesh.vertices.len() <= MAX_EDIT_VERTICES);
            let positions: BTreeSet<_> = mesh
                .vertices
                .iter()
                .map(|v| position_key(v.position))
                .collect();
            let symmetry: BTreeSet<_> = mesh
                .vertices
                .iter()
                .map(|v| v.position.map(|x| (x * 100000.).round() as i64))
                .collect();
            assert_eq!(
                positions.len() + mesh.face_count(),
                connectivity.edges.len() + 2
            );
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for vertex in &mesh.vertices {
                let p = Vec3::from_array(vertex.position);
                min = min.min(p);
                max = max.max(p);
                assert!(p.is_finite());
                assert!((Vec3::from_array(vertex.normal).length() - 1.).abs() < 1e-4);
                assert!(vertex.uv.iter().all(|v| v.is_finite()));
                let key = [
                    (-p.x * 100000.).round() as i64,
                    (p.y * 100000.).round() as i64,
                    (p.z * 100000.).round() as i64,
                ];
                assert!((-1..=1).any(|dx| (-1..=1).any(|dy| (-1..=1)
                    .any(|dz| symmetry.contains(&[key[0] + dx, key[1] + dy, key[2] + dz])))));
            }
            assert!(((max - min) - Vec3::from_array(settings.size)).length() < 1e-4);
            let volume: f32 = mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| {
                    let p = |i: u32| Vec3::from_array(mesh.vertices[i as usize].position);
                    let normal = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
                    assert!(normal.length() > 1e-10);
                    p(t[0]).dot(p(t[1]).cross(p(t[2]))) / 6.
                })
                .sum();
            assert!(volume > 0.1, "outward, nonzero closed volume: {volume}");
            assert_eq!(mesh.topology.as_ref().unwrap().primitive, Some(settings));
        }
    }
    let mut e = editor(PrimitiveKind::Fox);
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Extrude {
            faces: BTreeSet::from([5]),
            distance: 0.05,
        },
    )
    .unwrap();
    assert!(e.project.meshes[0]
        .topology
        .as_ref()
        .unwrap()
        .primitive
        .is_none());
    assert!(e.undo());
    assert_eq!(e.project, before);
}
