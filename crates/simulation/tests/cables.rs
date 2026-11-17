use glam::Vec3;
use poseforge_simulation::{
    deform::{DeformConfig, DeformWorld},
    mesh_contact::{MeshCollider, Triangle},
};
fn config() -> DeformConfig {
    DeformConfig {
        self_collision: false,
        bending: 30.,
        friction: 0.5,
        iterations: 24,
        compliance: 0.000001,
        damping: 0.06,
        gravity: 9.81,
        wind: 0.,
        radius: 0.04,
        activation: 0.,
        contraction: 0.,
        muscle: false,
    }
}
#[test]
fn cable_sags_keeps_anchors_and_collides_with_an_elevated_surface() {
    let points: Vec<_> = (0..=48)
        .map(|i| {
            let t = i as f32 / 48.;
            Vec3::new(
                -2. + 4. * t,
                2.5 - 0.4 * (std::f32::consts::PI * t).sin(),
                0.,
            )
        })
        .collect();
    let mut world = DeformWorld::cable(points.clone(), true, true).unwrap();
    let mesh = MeshCollider::new(vec![
        Triangle([
            Vec3::new(-3., 1.8, -2.),
            Vec3::new(3., 1.8, 2.),
            Vec3::new(3., 1.8, -2.),
        ]),
        Triangle([
            Vec3::new(-3., 1.8, -2.),
            Vec3::new(-3., 1.8, 2.),
            Vec3::new(3., 1.8, 2.),
        ]),
    ])
    .unwrap();
    for _ in 0..360 {
        world.step_with_mesh(config(), &[], &mesh);
    }
    assert_eq!(world.positions[0], points[0]);
    assert_eq!(*world.positions.last().unwrap(), *points.last().unwrap());
    assert!(world
        .positions
        .iter()
        .all(|p| p.is_finite() && p.y >= 1.839));
    assert!(world.positions[24].y < points[24].y);
}
#[test]
fn cable_free_end_falls_and_cancellation_is_atomic() {
    let p: Vec<_> = (0..=20)
        .map(|i| Vec3::new(i as f32 * 0.1, 3., 0.))
        .collect();
    let mut w = DeformWorld::cable(p.clone(), true, false).unwrap();
    for _ in 0..120 {
        w.step(config(), &[]);
    }
    assert_eq!(w.positions[0], p[0]);
    assert!(w.positions[20].y < 2.9);
    let before = w.positions.clone();
    let time = w.time;
    assert!(!w.step_with_mesh_cancellable(
        config(),
        &[],
        &MeshCollider::default(),
        &std::sync::atomic::AtomicBool::new(true)
    ));
    assert_eq!(w.positions, before);
    assert_eq!(w.time, time);
}
#[test]
fn a_thin_obstacle_catches_the_interior_of_a_cable_segment() {
    let points: Vec<_> = (0..=8)
        .map(|i| Vec3::new(-2. + i as f32 * 0.5, 2., 0.))
        .collect();
    // The ledge at x=0.25 lies halfway between particles and is narrower than their spacing.
    let mesh = MeshCollider::new(vec![
        Triangle([
            Vec3::new(0.20, 1.5, -1.),
            Vec3::new(0.30, 1.5, 1.),
            Vec3::new(0.30, 1.5, -1.),
        ]),
        Triangle([
            Vec3::new(0.20, 1.5, -1.),
            Vec3::new(0.20, 1.5, 1.),
            Vec3::new(0.30, 1.5, 1.),
        ]),
    ])
    .unwrap();
    let mut w = DeformWorld::cable(points, false, false).unwrap();
    for _ in 0..240 {
        w.step_with_mesh(config(), &[], &mesh);
    }
    assert!(w.positions.iter().all(|p| p.is_finite()));
    let crossing = w
        .positions
        .windows(2)
        .find(|p| (p[0].x - 0.25) * (p[1].x - 0.25) <= 0.)
        .unwrap();
    let t = (0.25 - crossing[0].x) / (crossing[1].x - crossing[0].x);
    assert!(
        crossing[0].lerp(crossing[1], t).y >= 1.53,
        "{:?}",
        w.positions
    );
}
