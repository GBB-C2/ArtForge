use poseforge_core::*;
use std::sync::atomic::AtomicBool;
fn cube() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap()
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
fn curved_cutters_work_with_cleanup_enabled_and_disabled() {
    let a = cube();
    let b = PrimitiveSettings::new(PrimitiveKind::Sphere)
        .mesh(4, None)
        .unwrap();
    for cleanup in [false, true] {
        for mode in [
            BooleanMode::Union,
            BooleanMode::Difference,
            BooleanMode::Intersection,
        ] {
            let m = boolean_mesh_with_options(
                &a,
                &b,
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.7),
                    Quat::from_rotation_y(0.3),
                    Vec3::new(0.5, 0.2, 0.1),
                ),
                mode,
                BooleanOptions {
                    dissolve_coplanar: cleanup,
                },
                &AtomicBool::new(false),
            );
            assert!(m.is_ok(), "{cleanup} {mode:?} {:?}", m.err());
        }
    }
}
#[test]
fn disjoint_operations_preserve_the_source_and_planar_faces() {
    let a = cube();
    let far = Mat4::from_translation(Vec3::X * 100000.);
    let r = boolean_mesh(
        &a,
        &a,
        far,
        BooleanMode::Difference,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(r, a);
    let joined = boolean_mesh(
        &a,
        &a,
        Mat4::from_translation(Vec3::X * 4.),
        BooleanMode::Union,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(joined.face_count(), 12);
    assert!((volume(&joined) - 16.).abs() < 0.001);
}
#[test]
fn inward_shells_are_classified_as_solids() {
    let a = cube();
    let mut b = a.clone();
    for f in b.topology.as_mut().unwrap().faces.iter_mut() {
        f.reverse();
    }
    b.rebuild_polygon_indices().unwrap();
    let r = boolean_mesh(
        &a,
        &b,
        Mat4::from_translation(Vec3::X),
        BooleanMode::Difference,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!((volume(&r) - 4.).abs() < 0.001);
}
#[test]
fn touching_faces_and_through_holes_keep_watertight_volumes() {
    let a = cube();
    let cancel = AtomicBool::new(false);
    for (mode, expected) in [
        (BooleanMode::Union, 16.),
        (BooleanMode::Difference, 8.),
        (BooleanMode::Intersection, 0.),
    ] {
        let m = boolean_mesh(&a, &a, Mat4::from_translation(Vec3::X * 2.), mode, &cancel).unwrap();
        assert!((volume(&m) - expected).abs() < 0.001);
        let r = inspect_topology(&m);
        assert_eq!(
            (r.boundary_edges, r.nonmanifold_edges, r.winding_edges),
            (0, 0, 0)
        );
    }
    let m = boolean_mesh(
        &a,
        &a,
        Mat4::from_scale(Vec3::new(0.2, 0.2, 2.)),
        BooleanMode::Difference,
        &cancel,
    )
    .unwrap();
    assert!((volume(&m) - 7.68).abs() < 0.001);
    let r = inspect_topology(&m);
    assert_eq!(
        (r.boundary_edges, r.nonmanifold_edges, r.winding_edges),
        (0, 0, 0)
    );
}
#[test]
fn cleanup_reduces_flat_fragments_without_changing_curved_cut_volume() {
    let a = cube();
    let b = PrimitiveSettings::new(PrimitiveKind::Sphere)
        .mesh(4, None)
        .unwrap();
    let transform = Mat4::from_translation(Vec3::X * 0.6);
    let cancel = AtomicBool::new(false);
    let raw = boolean_mesh_with_options(
        &a,
        &b,
        transform,
        BooleanMode::Difference,
        BooleanOptions {
            dissolve_coplanar: false,
        },
        &cancel,
    )
    .unwrap();
    let clean = boolean_mesh(&a, &b, transform, BooleanMode::Difference, &cancel).unwrap();
    assert!(
        clean.face_count() < raw.face_count(),
        "{} vs {}",
        clean.face_count(),
        raw.face_count()
    );
    assert!((volume(&clean) - volume(&raw)).abs() < 0.002);
}
