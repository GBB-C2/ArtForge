//! Cancellable rigid-fragment contacts against a frozen scene surface.
//! Fragments retain their shape in preview and bake.
#[path = "shard_pairs.rs"]
mod pairs;
pub use pairs::{solve_shard_contacts, ShardContact};
use crate::mesh_contact::{MeshCollider, Triangle};
use glam::{Quat, Vec3};
use std::{collections::BTreeMap, sync::atomic::{AtomicBool, Ordering}};

pub struct ShardShape {
    vertices: Vec<Vec3>,
    surface: MeshCollider,
    radius: f32,
    margin: f32,
    bounds: (Vec3, Vec3),
    inverse_mass: f32,
    candidates: Vec<usize>,
    query: Vec<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShardMotion {
    pub position: Vec3,
    pub rotation: Quat,
    pub velocity: Vec3,
    pub angular_axis: Vec3,
    pub angular_speed: f32,
}
#[derive(Clone, Copy)]
pub struct ShardForces {
    pub gravity: f32,
    pub floor: f32,
    pub restitution: f32,
}

impl ShardShape {
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, String> {
        // Weld only identical positions: UV seams must not multiply contact work.
        let mut unique = BTreeMap::new();
        for p in triangles.iter().flat_map(|t| t.0) {
            let key = p.to_array().map(|v| if v == 0. { 0 } else { v.to_bits() });
            unique.insert(key, p);
        }
        if unique.is_empty() || unique.len() > 16_384 {
            return Err("Разрушение: упростите осколок до 16384 вершин для записи столкновений".into());
        }
        let vertices: Vec<_> = unique.into_values().collect();
        let radius = vertices.iter().map(|p| p.length()).fold(0_f32, f32::max);
        let margin = (radius * 0.001).clamp(0.0001, 0.005);
        let bounds = vertices.iter().fold((Vec3::splat(f32::INFINITY),Vec3::splat(f32::NEG_INFINITY)),|(lo,hi),p|(lo.min(*p),hi.max(*p)));
        let volume = triangles.iter().map(|t|t.0[0].dot(t.0[1].cross(t.0[2]))/6.).sum::<f32>().abs();
        let size=bounds.1-bounds.0;
        let inverse_mass = 1. / volume.max(size.x*size.y*size.z*0.01).max(1e-8);
        Ok(Self { vertices, surface: MeshCollider::new(triangles)?, radius, margin, bounds, inverse_mass, candidates: vec![], query: vec![] })
    }

    pub fn step(&mut self, state: &mut ShardMotion, forces: ShardForces, scene: Option<&MeshCollider>,
        dt: f32, cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
        let previous = *state;
        state.velocity.y -= forces.gravity * dt;
        state.position += state.velocity * dt;
        state.rotation = (Quat::from_axis_angle(state.angular_axis, state.angular_speed * dt) * state.rotation).normalize();
        self.resolve_contacts(state, previous, forces, scene, dt, cancel)
    }

    /// Re-project after mutual impulses without applying gravity or advancing time twice.
    pub fn resolve_environment(&mut self, state: &mut ShardMotion, previous: ShardMotion,
        forces: ShardForces, scene: Option<&MeshCollider>, cancel: &AtomicBool) -> Result<(), String> {
        self.resolve_contacts(state, previous, forces, scene, 0., cancel)
    }

    fn resolve_contacts(&mut self, state: &mut ShardMotion, previous: ShardMotion,
        forces: ShardForces, scene: Option<&MeshCollider>, dt: f32, cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
        let old_position = previous.position;
        let old_rotation = previous.rotation;
        let mut contact_normal = Vec3::ZERO;
        for _ in 0..4 {
            let bottom = self.vertices.iter().map(|p| (state.rotation * *p).y)
                .fold(f32::INFINITY, f32::min) + state.position.y;
            let mut push = Vec3::Y * (forces.floor - bottom).max(0.);
            if let Some(scene) = scene {
                for (i, &vertex) in self.vertices.iter().enumerate() {
                    if i % 128 == 0 && cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
                    let start = old_position + old_rotation * vertex;
                    let end = state.position + state.rotation * vertex;
                    let correction = scene.project(start, end, self.margin, &mut self.query) - end;
                    if correction.length_squared() > push.length_squared() { push = correction; }
                }
                // Reciprocal vertex/face sweeps catch a small obstacle under a broad
                // shard face, where no shard vertex lies above the obstacle.
                scene.candidates(old_position, state.position, self.radius + self.margin, &mut self.candidates);
                for (i, &triangle) in self.candidates.iter().enumerate() {
                    if i % 64 == 0 && cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
                    for point in scene.triangle(triangle).0 {
                        let start = old_rotation.conjugate() * (point - old_position);
                        let end = state.rotation.conjugate() * (point - state.position);
                        let correction = -(state.rotation * (self.surface.project(start, end, self.margin, &mut self.query) - end));
                        if correction.length_squared() > push.length_squared() { push = correction; }
                    }
                }
            }
            if push.length_squared() < 1e-12 { break; }
            state.position += push;
            let normal = push.normalize();
            contact_normal += normal;
            let incoming = state.velocity.dot(normal);
            if incoming < 0. {
                let bounce = if incoming < -0.15 { forces.restitution } else { 0. };
                state.velocity -= normal * (incoming * (1. + bounce));
            }
        }
        if let Some(normal) = contact_normal.try_normalize() {
            let normal_velocity = normal * state.velocity.dot(normal);
            state.velocity = normal_velocity + (state.velocity - normal_velocity) * 0.96_f32.powf(dt * 120.);
            state.angular_speed *= 0.94_f32.powf(dt * 120.);
        }
        if !state.position.is_finite() || !state.rotation.is_finite() || !state.velocity.is_finite() {
            return Err("Разрушение: некорректное состояние осколка; уменьшите импульс".into());
        }
        Ok(())
    }
}
