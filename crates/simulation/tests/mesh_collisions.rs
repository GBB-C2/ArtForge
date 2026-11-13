use glam::{Quat, Vec3};
use poseforge_simulation::{FluidConfig, FluidWorld, mesh_contact::{MeshCollider, Triangle},
    mesh_obstacles::MeshObstacles, shard_contact::{ShardForces, ShardMotion, ShardShape}};
use std::sync::{Arc, atomic::AtomicBool};

fn running() -> AtomicBool { AtomicBool::new(false) }
fn plane(y: f32) -> Vec<Triangle> {
    let p = [Vec3::new(-8., y, -8.), Vec3::new(8., y, -8.), Vec3::new(8., y, 8.), Vec3::new(-8., y, 8.)];
    vec![Triangle([p[0], p[1], p[2]]), Triangle([p[0], p[2], p[3]])]
}
fn cube(center: Vec3, half: f32) -> Vec<Triangle> {
    let p = [(-1.,-1.,-1.),(1.,-1.,-1.),(1.,1.,-1.),(-1.,1.,-1.),
        (-1.,-1.,1.),(1.,-1.,1.),(1.,1.,1.),(-1.,1.,1.)].map(|(x,y,z)| center + Vec3::new(x,y,z)*half);
    [[0,3,2],[0,2,1],[4,5,6],[4,6,7],[0,1,5],[0,5,4],
        [3,7,6],[3,6,2],[0,4,7],[0,7,3],[1,2,6],[1,6,5]]
        .map(|t| Triangle(t.map(|i| p[i]))).to_vec()
}
fn obstacles(mesh: Vec<Triangle>) -> Arc<MeshObstacles> {
    Arc::new(MeshObstacles::new(vec![mesh], &running()).unwrap())
}
fn fluid(point: Vec3) -> FluidWorld {
    FluidWorld::new(FluidConfig { bounds_min: Vec3::splat(-4.), bounds_max: Vec3::splat(4.),
        boundary_friction: 0.8, max_substeps: 1, ..Default::default() }, vec![point]).unwrap()
}
fn motion(position: Vec3) -> ShardMotion {
    ShardMotion { position, rotation: Quat::IDENTITY, velocity: Vec3::ZERO,
        angular_axis: Vec3::Y, angular_speed: 0. }
}
fn forces() -> ShardForces { ShardForces { gravity: 9.81, floor: -2., restitution: 0. } }

#[test]
fn fluid_seeding_excludes_closed_solids_but_not_the_interior_of_open_mesh_bounds() {
    let closed = obstacles(cube(Vec3::ZERO, 0.5));
    let open = obstacles(plane(0.));
    let mut query = vec![];
    assert!(!closed.accepts_particle(Vec3::ZERO, 0.035, &mut query));
    assert!(!closed.accepts_particle(Vec3::new(0.52, 0., 0.), 0.035, &mut query));
    assert!(closed.accepts_particle(Vec3::ONE, 0.035, &mut query));
    assert!(open.accepts_particle(Vec3::NEG_Y, 0.035, &mut query));
    assert!(!open.accepts_particle(Vec3::Y * 0.02, 0.035, &mut query));
}

#[test]
fn fluid_lands_on_a_sloped_mesh_and_uses_its_surface_normal() {
    let ramp: Vec<_> = plane(0.8).into_iter().map(|t| Triangle(t.0.map(|p| Vec3::new(p.x, p.y + p.x*0.25, p.z)))).collect();
    let normal = Vec3::new(-0.25, 1., 0.).normalize();
    let mut world = fluid(Vec3::Y * 1.5);
    world.set_mesh_obstacles(Some(obstacles(ramp)));
    for _ in 0..240 { world.step(); }
    let distance = (world.positions()[0] - Vec3::Y*0.8).dot(normal);
    assert!((distance - world.config().particle_radius).abs() < 0.001, "{distance}");
    assert!(world.velocities()[0].dot(normal) >= -0.0001);
}

#[test]
fn high_speed_fluid_cannot_tunnel_through_either_side_of_a_thin_plane() {
    for side in [-1., 1.] {
        let mut world = fluid(Vec3::Y * (0.08 * side));
        world.set_mesh_obstacles(Some(obstacles(plane(0.))));
        let mut snapshot = world.snapshot();
        snapshot.velocities[0] = Vec3::Y * (-world.config().max_speed * side);
        world.restore(&snapshot).unwrap();
        world.step();
        assert!(world.positions()[0].y * side >= world.config().particle_radius - 0.0001);
    }
}

#[test]
fn fluid_falls_through_empty_space_inside_triangle_bounding_box() {
    let triangle = Triangle([Vec3::new(-2.,0.,-2.), Vec3::new(2.,0.,-2.), Vec3::new(-2.,0.,2.)]);
    let mut world = fluid(Vec3::new(1.4, 1., 1.4));
    world.set_mesh_obstacles(Some(obstacles(vec![triangle])));
    for _ in 0..90 { world.step(); }
    assert!(world.positions()[0].y < -0.5);
}

#[test]
fn obstacle_geometry_invalidates_cache_and_world_clones_share_collision_geometry() {
    let obstacle = obstacles(plane(0.));
    let mut a = fluid(Vec3::Y);
    a.set_mesh_obstacles(Some(obstacle.clone()));
    let mut b = a.clone();
    assert_eq!(Arc::strong_count(&obstacle), 3);
    for _ in 0..60 { a.step(); b.step(); }
    assert_eq!(a.positions(), b.positions());
    let snapshot = a.snapshot();
    a.set_mesh_obstacles(Some(obstacles(plane(0.5))));
    assert_ne!(a.signature(), b.signature());
    assert!(a.restore(&snapshot).is_err());
    a.set_mesh_obstacles(None);
    assert!(a.restore(&snapshot).is_err());
}

#[test]
fn shard_lands_on_a_raised_mesh_and_can_ignore_it() {
    let platform = MeshCollider::new(plane(1.)).unwrap();
    for enabled in [true, false] {
        let mut shape = ShardShape::new(cube(Vec3::ZERO, 0.2)).unwrap();
        let mut state = motion(Vec3::Y*2.);
        for _ in 0..360 { shape.step(&mut state, forces(), enabled.then_some(&platform), 1./120., &running()).unwrap(); }
        let bottom = state.position.y - 0.2;
        if enabled { assert!((bottom - 1.).abs() < 0.005, "{bottom}"); }
        else { assert!((bottom + 2.).abs() < 0.005, "{bottom}"); }
    }
}

#[test]
fn small_mesh_obstacle_under_a_broad_shard_face_is_not_missed() {
    let post = MeshCollider::new(cube(Vec3::Y, 0.15)).unwrap();
    let mut shape = ShardShape::new(cube(Vec3::ZERO, 0.6)).unwrap();
    let mut state = motion(Vec3::Y*2.5);
    for _ in 0..360 { shape.step(&mut state, forces(), Some(&post), 1./120., &running()).unwrap(); }
    assert!((state.position.y - 0.6 - 1.15).abs() < 0.005, "{}", state.position.y);
}

#[test]
fn shard_high_speed_wall_contact_bounces_in_the_mesh_normal_direction() {
    let wall = MeshCollider::new(plane(0.).into_iter().map(|t| Triangle(t.0.map(|p| Vec3::new(p.y,p.x,p.z)))).collect()).unwrap();
    let mut shape = ShardShape::new(cube(Vec3::ZERO, 0.1)).unwrap();
    let mut state = motion(Vec3::new(-0.3, 1., 0.));
    state.velocity = Vec3::X*100.;
    shape.step(&mut state, ShardForces { gravity: 0., restitution: 0.5, ..forces() }, Some(&wall), 1./120., &running()).unwrap();
    assert!(state.position.x <= -0.099);
    assert!(state.velocity.x < -49.);
}

#[test]
fn rotated_shard_keeps_all_vertices_above_a_sloped_mesh() {
    let mesh = cube(Vec3::ZERO, 0.2);
    let ramp = MeshCollider::new(plane(0.8).into_iter().map(|t| Triangle(t.0.map(|p| Vec3::new(p.x,p.y+p.x*0.2,p.z)))).collect()).unwrap();
    let mut shape = ShardShape::new(mesh.clone()).unwrap();
    let mut state = motion(Vec3::Y*2.);
    state.rotation = Quat::from_rotation_z(0.3);
    let normal = Vec3::new(-0.2,1.,0.).normalize();
    for _ in 0..300 { shape.step(&mut state, forces(), Some(&ramp), 1./120., &running()).unwrap(); }
    for point in mesh.iter().flat_map(|t| t.0) {
        assert!((state.position + state.rotation*point - Vec3::Y*0.8).dot(normal) > -0.001);
    }
}

#[test]
fn collision_preparation_and_shard_step_honor_cancellation() {
    let cancel = AtomicBool::new(true);
    assert!(MeshObstacles::new(vec![cube(Vec3::ZERO,1.)], &cancel).is_err());
    let mut shape = ShardShape::new(cube(Vec3::ZERO,0.2)).unwrap();
    let mut state = motion(Vec3::Y);
    assert!(shape.step(&mut state, forces(), None, 1./120., &cancel).is_err());
    assert_eq!(state.position, Vec3::Y);
}


#[test]
fn relative_fragment_contacts_stop_crossing_only_when_both_flags_are_enabled() {
    use poseforge_simulation::shard_contact::{solve_shard_contacts,ShardContact};
    let shape=ShardShape::new(cube(Vec3::ZERO,0.2)).unwrap();
    for enabled in [true,false] {
        let mut a=motion(Vec3::X*-0.15);a.velocity=Vec3::X*3.;
        let mut b=motion(Vec3::X*0.15);b.velocity=Vec3::X*-3.;
        let old_a=motion(Vec3::X*-0.3);let old_b=motion(Vec3::X*0.3);
        let before=(a,b);
        let mut contacts=[ShardContact {shape:&shape,motion:&mut a,previous:old_a,enabled:true,restitution:0.,floor:-10.},
            ShardContact {shape:&shape,motion:&mut b,previous:old_b,enabled,restitution:0.,floor:-10.}];
        solve_shard_contacts(&mut contacts,&running()).unwrap();
        if enabled {assert!(b.position.x-a.position.x>=0.399);assert!((a.position+b.position).length()<1e-5);assert!(a.velocity.x<3. && b.velocity.x> -3.);}
        else {assert_eq!((a,b),before);}
    }
}
#[test]
fn fragment_pair_broadphase_does_not_fill_empty_triangle_bounds_and_cancels() {
    use poseforge_simulation::shard_contact::{solve_shard_contacts,ShardContact};
    let triangle=ShardShape::new(vec![Triangle([Vec3::new(-2.,0.,-2.),Vec3::new(2.,0.,-2.),Vec3::new(-2.,0.,2.)])]).unwrap();
    let cube=ShardShape::new(cube(Vec3::ZERO,0.1)).unwrap();
    let mut a=motion(Vec3::new(1.4,-0.2,1.4));let old=motion(Vec3::new(1.4,0.2,1.4));let mut b=motion(Vec3::ZERO);
    let before=(a,b);let previous_b=b;
    let mut contacts=[ShardContact {shape:&cube,motion:&mut a,previous:old,enabled:true,restitution:0.,floor:-10.},
        ShardContact {shape:&triangle,motion:&mut b,previous:previous_b,enabled:true,restitution:0.,floor:-10.}];
    solve_shard_contacts(&mut contacts,&running()).unwrap();
    assert!(solve_shard_contacts(&mut contacts,&AtomicBool::new(true)).is_err());drop(contacts);
    assert_eq!((a,b),before);
}
