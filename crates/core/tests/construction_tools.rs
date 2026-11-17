use poseforge_core::*;
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn cube() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap()
}
fn editor(mesh: Mesh) -> Editor {
    let mut n = Node::new(1, "Object");
    n.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![n],
        meshes: vec![mesh],
        ..Default::default()
    })
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
    m.validate_topology().unwrap();
}
fn volume(m: &Mesh) -> f32 {
    m.indices
        .chunks_exact(3)
        .map(|t| {
            let p = |i: usize| Vec3::from_array(m.vertices[t[i] as usize].position);
            p(0).dot(p(1).cross(p(2))) / 6.
        })
        .sum::<f32>()
        .abs()
}
#[test]
fn boolean_modes_make_closed_volumes_and_leave_sources_untouched() {
    let a = cube();
    let b = cube();
    for (mode, want) in [
        (BooleanMode::Union, 12.),
        (BooleanMode::Difference, 4.),
        (BooleanMode::Intersection, 4.),
    ] {
        let r = boolean_mesh(
            &a,
            &b,
            Mat4::from_translation(Vec3::X),
            mode,
            &AtomicBool::new(false),
        )
        .unwrap();
        closed(&r);
        assert!((volume(&r) - want).abs() < 1e-3, "{mode:?} {}", volume(&r));
    }
    assert_eq!(a, cube());
}
#[test]
fn booleans_handle_identical_disjoint_negative_scale_and_cancellation() {
    let a = cube();
    for (mode, want) in [
        (BooleanMode::Union, 8.),
        (BooleanMode::Difference, 0.),
        (BooleanMode::Intersection, 8.),
    ] {
        let r = boolean_mesh(&a, &a, Mat4::IDENTITY, mode, &AtomicBool::new(false)).unwrap();
        closed(&r);
        assert!((volume(&r) - want).abs() < 1e-3);
    }
    let r = boolean_mesh(
        &a,
        &a,
        Mat4::from_translation(Vec3::X * 4.),
        BooleanMode::Union,
        &AtomicBool::new(false),
    )
    .unwrap();
    closed(&r);
    assert!((volume(&r) - 16.).abs() < 1e-3);
    let r = boolean_mesh(
        &a,
        &a,
        Mat4::from_scale_rotation_translation(Vec3::new(-1., 1., 1.), Quat::IDENTITY, Vec3::X),
        BooleanMode::Intersection,
        &AtomicBool::new(false),
    )
    .unwrap();
    closed(&r);
    assert!((volume(&r) - 4.).abs() < 1e-3);
    assert!(boolean_mesh(
        &a,
        &a,
        Mat4::IDENTITY,
        BooleanMode::Union,
        &AtomicBool::new(true)
    )
    .is_err());
}
#[test]
fn simplify_closed_mesh_preserves_topology_and_undo() {
    let m = PrimitiveSettings::new(PrimitiveKind::Sphere)
        .mesh(2, None)
        .unwrap();
    let count = m.indices.len();
    let faces = (0..m.face_count()).collect();
    let mut e = editor(m);
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Simplify {
            region: MeshRegion {
                faces,
                ..Default::default()
            },
            ratio: 0.5,
        },
    )
    .unwrap();
    let m = e.editable_mesh(1).unwrap();
    closed(m);
    assert!(m.indices.len() < count * 3 / 4);
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, before);
}
#[test]
fn simplify_is_scoped_and_invalid_values_are_atomic() {
    let mut e = editor(cube());
    e.edit_mesh(
        1,
        MeshEdit::SubdivideSelectedCount {
            region: MeshRegion {
                faces: BTreeSet::from([0]),
                ..Default::default()
            },
            segments: 8,
        },
    )
    .unwrap();
    let original = e.editable_mesh(1).unwrap().clone();
    let faces = (0..original.face_count())
        .filter(|&f| {
            original
                .face_vertices(f)
                .unwrap()
                .iter()
                .all(|&v| (original.vertices[v as usize].position[0] - 1.).abs() < 1e-6)
        })
        .collect();
    let region = MeshRegion {
        faces,
        ..Default::default()
    };
    let before = e.project.clone();
    for ratio in [0., f32::NAN, 1.1] {
        assert!(e
            .edit_mesh(
                1,
                MeshEdit::Simplify {
                    region: region.clone(),
                    ratio
                }
            )
            .is_err());
        assert_eq!(e.project, before);
    }
    e.edit_mesh(1, MeshEdit::Simplify { region, ratio: 0.5 })
        .unwrap();
    closed(e.editable_mesh(1).unwrap());
}
#[test]
fn mirror_welds_the_plane_reverses_winding_and_undoes() {
    let mut m = cube();
    for v in &mut m.vertices {
        v.position[0] += 1.;
    }
    let mut e = editor(m);
    let before = e.project.clone();
    e.edit_mesh(
        1,
        MeshEdit::Mirror {
            region: MeshRegion {
                faces: (0..6).collect(),
                ..Default::default()
            },
            axis: 0,
            plane: 0.,
            merge: true,
            distance: 0.001,
        },
    )
    .unwrap();
    let m = e.editable_mesh(1).unwrap();
    closed(m);
    assert!((volume(m) - 16.).abs() < 1e-3);
    e.undo();
    assert_eq!(e.project, before);
}
#[test]
fn cable_mesh_has_fixed_cross_section_and_roundtrips() {
    let s = CableSettings::default();
    let mesh = s.mesh(2, None).unwrap();
    closed(&mesh);
    let centers = s.centers().unwrap();
    assert_eq!(centers.len(), s.segments as usize + 1);
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<CableSettings>(&json).unwrap(), s);
    let mut e = editor(mesh);
    e.project.nodes[0].cable = Some(s);
    e.project.nodes[0].physics = Some(PhysicsSettings {
        kind: PhysicsKind::Cable,
        ..Default::default()
    });
    e.project.validate().unwrap();
    let saved = serde_json::to_string(&e.project).unwrap();
    assert_eq!(serde_json::from_str::<Project>(&saved).unwrap(), e.project);
}
#[test]
fn boolean_rotated_curved_and_contained_solids_stay_closed() {
    let a = cube();
    let b = PrimitiveSettings::new(PrimitiveKind::Sphere)
        .mesh(4, None)
        .unwrap();
    for mode in [
        BooleanMode::Union,
        BooleanMode::Difference,
        BooleanMode::Intersection,
    ] {
        let m = boolean_mesh(
            &a,
            &b,
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.7),
                Quat::from_rotation_y(0.3),
                Vec3::new(0.5, 0.2, 0.1),
            ),
            mode,
            &AtomicBool::new(false),
        )
        .unwrap();
        closed(&m);
    }
    let m = boolean_mesh(
        &a,
        &a,
        Mat4::from_scale(Vec3::splat(0.4)),
        BooleanMode::Difference,
        &AtomicBool::new(false),
    )
    .unwrap();
    closed(&m);
    assert!((volume(&m) - (8. - 0.512)).abs() < 1e-3);
}
#[test]
fn mirror_does_not_duplicate_an_already_symmetric_surface() {
    let mut e = editor(cube());
    e.edit_mesh(
        1,
        MeshEdit::Mirror {
            region: MeshRegion {
                faces: (0..6).collect(),
                ..Default::default()
            },
            axis: 0,
            plane: 0.,
            merge: true,
            distance: 0.001,
        },
    )
    .unwrap();
    let m = e.editable_mesh(1).unwrap();
    closed(m);
    assert_eq!(m.face_count(), 6);
    assert!((volume(m) - 8.).abs() < 1e-3);
}
#[test]
fn boolean_empty_results_can_be_used_again() {
    let a = cube();
    let empty = boolean_mesh(
        &a,
        &a,
        Mat4::IDENTITY,
        BooleanMode::Difference,
        &AtomicBool::new(false),
    )
    .unwrap();
    for mode in [
        BooleanMode::Union,
        BooleanMode::Difference,
        BooleanMode::Intersection,
    ] {
        let r = boolean_mesh(&empty, &a, Mat4::IDENTITY, mode, &AtomicBool::new(false)).unwrap();
        closed(&r);
        assert!((volume(&r) - if mode == BooleanMode::Union { 8. } else { 0. }).abs() < 1e-3);
    }
    let r = boolean_mesh(
        &a,
        &empty,
        Mat4::IDENTITY,
        BooleanMode::Difference,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!((volume(&r) - 8.).abs() < 1e-3);
}
