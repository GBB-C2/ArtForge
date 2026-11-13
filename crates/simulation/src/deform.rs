//! Small-step XPBD distance constraints with static swept AABB contacts.
use crate::mesh_contact::{MeshCollider, Triangle};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::atomic::{AtomicBool, Ordering}};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Collider {
    pub min: Vec3,
    pub max: Vec3,
}
impl Collider {
    pub fn valid(self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.min.cmplt(self.max).all()
    }
    pub fn project(self, start: Vec3, mut end: Vec3, radius: f32) -> Vec3 {
        let min = self.min - Vec3::splat(radius);
        let max = self.max + Vec3::splat(radius);
        if end.cmpge(min).all() && end.cmple(max).all() {
            let mut distance = f32::INFINITY;
            let mut side = (0, min.x);
            for axis in 0..3 {
                for edge in [min[axis], max[axis]] {
                    let d = (end[axis] - edge).abs();
                    if d < distance {
                        distance = d;
                        side = (axis, edge);
                    }
                }
            }
            end[side.0] = side.1;
            return end;
        }
        // Swept slab test catches a particle crossing a thin collider in one step.
        if start.cmpgt(min).all() && start.cmplt(max).all() {
            return end;
        }
        let direction = end - start;
        let mut enter = 0.0_f32;
        let mut exit = 1.0_f32;
        let mut normal = Vec3::ZERO;
        for axis in 0..3 {
            if direction[axis].abs() < 1e-8 {
                if start[axis] < min[axis] || start[axis] > max[axis] {
                    return end;
                }
                continue;
            }
            let a = (min[axis] - start[axis]) / direction[axis];
            let b = (max[axis] - start[axis]) / direction[axis];
            let near = a.min(b);
            if near > enter {
                enter = near;
                normal = Vec3::ZERO;
                normal[axis] = -direction[axis].signum();
            }
            exit = exit.min(a.max(b));
            if enter > exit {
                return end;
            }
        }
        if (0. ..=1.).contains(&enter) && normal != Vec3::ZERO {
            let hit = start + direction * enter + normal * 0.0001;
            let rest = end - hit;
            return hit + rest - normal * rest.dot(normal).min(0.);
        }
        end
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeformConfig {
    pub self_collision: bool,
    pub bending: f32,
    pub friction: f32,
    pub iterations: u32,
    pub compliance: f32,
    pub damping: f32,
    pub gravity: f32,
    pub wind: f32,
    pub radius: f32,
    pub activation: f32,
    pub contraction: f32,
    pub muscle: bool,
}
#[derive(Clone)]
struct Edge {
    a: usize,
    b: usize,
    length: f32,
    axial: f32,
    bending: bool,
    lambda: f32,
}
#[derive(Clone)]
pub struct DeformWorld {
    fields:crate::fields::Fields,
    pub positions: Vec<Vec3>,
    rest: Vec<Vec3>,
    velocities: Vec<Vec3>,
    inverse_mass: Vec<f32>,
    edges: Vec<Edge>,
    triangles: Vec<[usize; 3]>,
    neighbors: Vec<std::collections::BTreeSet<usize>>,
    weld_roots: Vec<usize>,
    weld_groups: Vec<Vec<usize>>,
    self_contacts: Option<MeshCollider>,
    pub time: f32,
}
impl DeformWorld {
    pub fn new(positions: Vec<Vec3>, indices: &[u32], pinned: &[bool]) -> Result<Self, String> {
        if positions.is_empty()
            || positions.len() > 8192
            || pinned.len() != positions.len()
            || positions.iter().any(|p| !p.is_finite())
            || !indices.len().is_multiple_of(3)
            || indices.len() > 150_000
            || indices.iter().any(|&i| i as usize >= positions.len())
        {
            return Err("Физика: требуется корректная сетка до 8192 вершин".into());
        }
        let mut pairs = BTreeMap::<(usize, usize), bool>::new();
        let mut triangles = BTreeMap::<(usize, usize), usize>::new();
        for tri in indices.as_chunks::<3>().0 {
            for j in 0..3 {
                let a = tri[j] as usize;
                let b = tri[(j + 1) % 3] as usize;
                let c = tri[(j + 2) % 3] as usize;
                let key = (a.min(b), a.max(b));
                pairs.insert(key, false);
                if let Some(opposite) = triangles.insert(key, c) {
                    if opposite != c {
                        pairs
                            .entry((opposite.min(c), opposite.max(c)))
                            .or_insert(true);
                    }
                }
            }
        }
        // Weld duplicate UV seam vertices without changing the render topology.
        let mut weld = BTreeMap::new();
        let mut weld_roots: Vec<_> = (0..positions.len()).collect();
        let mut groups = BTreeMap::<usize, Vec<usize>>::new();
        for (i, p) in positions.iter().enumerate() {
            let key = p.to_array().map(|v| (v * 100000.).round() as i64);
            if let Some(&j) = weld.get(&key) {
                pairs.insert((j, i), false);
                weld_roots[i] = j;
                groups.entry(j).or_insert_with(|| vec![j]).push(i);
            } else {
                weld.insert(key, i);
            }
        }
        let mut neighbors = vec![std::collections::BTreeSet::new(); positions.len()];
        for &(a, b) in pairs.keys() {
            let a = weld_roots[a]; let b = weld_roots[b];
            neighbors[a].insert(b);
            neighbors[b].insert(a);
        }
        let edges = pairs
            .into_iter()
            .filter(|((a, b), _)| a != b)
            .map(|((a, b), bending)| {
                let d = positions[a] - positions[b];
                Edge {
                    a,
                    b,
                    length: d.length(),
                    axial: d.y * d.y / d.length_squared().max(1e-12),
                    bending,
                    lambda: 0.,
                }
            })
            .collect();
        let weld_groups: Vec<_> = groups.into_values().collect();
        let mut inverse_mass: Vec<_> = pinned.iter().map(|p| if *p { 0. } else { 1. }).collect();
        for group in &weld_groups {
            if group.iter().any(|&i| pinned[i]) {
                for &i in group { inverse_mass[i] = 0.; }
            }
        }
        Ok(Self {
            fields:Default::default(),
            velocities: vec![Vec3::ZERO; positions.len()],
            inverse_mass,
            weld_roots, weld_groups, self_contacts: None,
            rest: positions.clone(),
            positions,
            edges,
            triangles: indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| t.map(|i| i as usize))
                .collect(),
            neighbors,
            time: 0.,
        })
    }
    pub fn set_fields(&mut self,fields:crate::fields::Fields) {self.fields=fields;}
    pub fn set_axis(&mut self, axis: Vec3) {
        let axis = axis.try_normalize().unwrap_or(Vec3::Y);
        for e in &mut self.edges {
            let d = self.rest[e.a] - self.rest[e.b];
            e.axial = d.dot(axis).powi(2) / d.length_squared().max(1e-12);
        }
    }
    pub fn step(&mut self, cfg: DeformConfig, colliders: &[Collider]) {
        self.step_with_mesh(cfg, colliders, &MeshCollider::default());
    }
    pub fn step_with_mesh(&mut self, cfg: DeformConfig, colliders: &[Collider], mesh: &MeshCollider) {
        self.step_internal(cfg, colliders, mesh, None);
    }
    /// Cancellation restores positions, velocities and time to the previous complete tick.
    pub fn step_with_mesh_cancellable(&mut self, cfg: DeformConfig, colliders: &[Collider],
        mesh: &MeshCollider, cancel: &AtomicBool) -> bool {
        self.step_internal(cfg, colliders, mesh, Some(cancel))
    }
    fn step_internal(&mut self, cfg: DeformConfig, colliders: &[Collider], mesh: &MeshCollider,
        cancel: Option<&AtomicBool>) -> bool {
        let cancelled = || cancel.is_some_and(|c| c.load(Ordering::Relaxed));
        if cancelled() { return false; }
        let dt = 1. / 120.;
        let previous_time = self.time;
        let previous = self.positions.clone();
        let previous_velocities = cancel.map(|_| self.velocities.clone());
        let completed = (|| {
            self.time += dt;
            let mut contacts = vec![Vec3::ZERO; self.positions.len()];
            let mut contact_push = vec![0_f32; self.positions.len()];
            let mut candidates = vec![];
            for (i, p) in self.positions.iter_mut().enumerate() {
                if self.inverse_mass[i] == 0. {
                    *p = self.rest[i];
                    continue;
                }
                let field=self.fields.acceleration(*p,self.velocities[i]);
                self.velocities[i] += field * dt;
                self.velocities[i] += Vec3::new(
                    cfg.wind * (self.time * 1.4 + self.rest[i].x * 0.7 + self.rest[i].y * 0.4).sin(),
                    -cfg.gravity,
                    0.25 * cfg.wind,
                ) * dt;
                self.velocities[i] *= (1. - cfg.damping.clamp(0., 1.)).powf(dt * 60.);
                *p += self.velocities[i].clamp_length_max(30.) * dt;
            }
            for e in &mut self.edges {
                e.lambda = 0.;
            }
            for _ in 0..cfg.iterations.clamp(1, 24) {
                if cancelled() { return false; }
                for (edge_index, e) in self.edges.iter_mut().enumerate() {
                    if edge_index % 256 == 0 && cancelled() { return false; }
                    let d = self.positions[e.a] - self.positions[e.b];
                    let length = d.length();
                    if length < 1e-8 {
                        continue;
                    }
                    let sum = self.inverse_mass[e.a] + self.inverse_mass[e.b];
                    if sum == 0. {
                        continue;
                    }
                    let contraction =
                        1. - cfg.activation.clamp(0., 1.) * cfg.contraction.clamp(0., 0.6);
                    let target = e.length
                        * if cfg.muscle {
                            (e.axial * contraction * contraction + (1. - e.axial) / contraction).sqrt()
                        } else {
                            1.
                        };
                    let alpha = cfg.compliance.max(0.)
                        * if e.bending {
                            cfg.bending.clamp(1., 100.)
                        } else {
                            1.
                        }
                        / (dt * dt);
                    let dl = (-(length - target) - alpha * e.lambda) / (sum + alpha);
                    e.lambda += dl;
                    let correction = d / length * dl;
                    self.positions[e.a] += correction * self.inverse_mass[e.a];
                    self.positions[e.b] -= correction * self.inverse_mass[e.b];
                }
                if cfg.self_collision {
                    if !self.solve_self_contacts_cancellable(&previous, cfg.radius * 2., cancel) { return false; }
                }
                for (i, p) in self.positions.iter_mut().enumerate() {
                    if i % 64 == 0 && cancelled() { return false; }
                    if self.inverse_mass[i] == 0. {
                        continue;
                    }
                    if p.y < cfg.radius {
                        contact_push[i] += cfg.radius - p.y;
                        contacts[i] += Vec3::Y;
                        p.y = cfg.radius;
                    }
                    for collider in colliders {
                        let next = collider.project(previous[i], *p, cfg.radius);
                        let correction = next - *p;
                        if let Some(normal) = correction.try_normalize() {
                            contacts[i] += normal;
                            contact_push[i] += correction.length();
                        }
                        *p = next;
                    }
                    let next = mesh.project(previous[i], *p, cfg.radius, &mut candidates);
                    let correction = next - *p;
                    if let Some(normal) = correction.try_normalize() {
                        contacts[i] += normal;
                        contact_push[i] += correction.length();
                    }
                    *p = next;
                }
                // UV splits belong to one physical vertex; compliance must not open seams.
                for group in &self.weld_groups {
                    let p = group.iter().map(|&i| self.positions[i]).sum::<Vec3>() / group.len() as f32;
                    for &i in group { self.positions[i] = p; }
                }
            }
            for (i, p) in self.positions.iter().enumerate() {
                let mut velocity = (*p - previous[i]) / dt;
                if let Some(normal) = contacts[i].try_normalize() {
                    velocity -= normal * velocity.dot(normal).min(0.);
                    let tangent = velocity - normal * velocity.dot(normal);
                    let speed = tangent.length();
                    if speed > 1e-8 {
                        velocity -= tangent
                            * (cfg.friction.clamp(0., 1.) * contact_push[i] / (dt * speed)).min(1.);
                    }
                }
                self.velocities[i] = velocity;
            }
            !cancelled()
        })();
        if !completed {
            self.positions = previous;
            if let Some(velocities) = previous_velocities { self.velocities = velocities; }
            self.time = previous_time;
        }
        completed
    }
    #[cfg(test)]
    fn solve_self_contacts(&mut self, previous: &[Vec3], radius: f32) {
        self.solve_self_contacts_cancellable(previous, radius, None);
    }
    fn solve_self_contacts_cancellable(&mut self, previous: &[Vec3], radius: f32,
        cancel: Option<&AtomicBool>) -> bool {
        if self.self_contacts.is_none() {
            let triangles = self.triangles.iter().map(|t| Triangle(t.map(|i| self.positions[i]))).collect();
            let Ok(bvh) = MeshCollider::new(triangles) else { return true; };
            self.self_contacts = Some(bvh);
        }
        let positions = &self.positions; let triangles = &self.triangles;
        self.self_contacts.as_mut().unwrap().refit(|i| Triangle(triangles[i].map(|j| positions[j])));
        let bvh = self.self_contacts.as_ref().unwrap();
        let mut candidates = vec![];
        for (i, &start) in previous.iter().enumerate() {
            if i % 32 == 0 && cancel.is_some_and(|c| c.load(Ordering::Relaxed)) { return false; }
            bvh.candidates(start, self.positions[i], radius, &mut candidates);
            for &index in &candidates {
                let ids = self.triangles[index];
                if ids.iter().any(|j| {
                    *j == i
                        || self.neighbors[self.weld_roots[i]].contains(&self.weld_roots[*j])
                        || self.rest[*j].distance_squared(self.rest[i]) < 1e-10
                }) {
                    continue;
                }
                let triangle = Triangle(ids.map(|j| self.positions[j]));
                if let Some((push, bary)) = triangle.contact(start, self.positions[i], radius) {
                    let sum = self.inverse_mass[i]
                        + (0..3)
                            .map(|j| self.inverse_mass[ids[j]] * bary[j] * bary[j])
                            .sum::<f32>();
                    if sum <= 0. {
                        continue;
                    }
                    self.positions[i] += push * (self.inverse_mass[i] / sum);
                    for j in 0..3 {
                        self.positions[ids[j]] -=
                            push * (self.inverse_mass[ids[j]] * bary[j] / sum);
                    }
                }
            }
        }
        true
    }
    pub fn normals(&self, indices: &[u32]) -> Vec<Vec3> {
        let mut normals = vec![Vec3::ZERO; self.positions.len()];
        for t in indices.as_chunks::<3>().0 {
            let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
            let n = (self.positions[b] - self.positions[a])
                .cross(self.positions[c] - self.positions[a]);
            for i in [a, b, c] {
                normals[i] += n;
            }
        }
        for group in &self.weld_groups {
            let normal = group.iter().map(|&i| normals[i]).sum::<Vec3>();
            for &i in group { normals[i] = normal; }
        }
        normals
            .into_iter()
            .map(|v| v.try_normalize().unwrap_or(Vec3::Y))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> DeformConfig {
        DeformConfig {
            self_collision: false,
            bending: 8.,
            friction: 0.,
            iterations: 8,
            compliance: 0.00001,
            damping: 0.,
            gravity: 9.81,
            wind: 0.,
            radius: 0.02,
            activation: 0.,
            contraction: 0.,
            muscle: false,
        }
    }
    #[test]
    fn self_contact_keeps_free_vertex_above_nonadjacent_pinned_triangle() {
        let mut world = DeformWorld::new(
            vec![
                Vec3::new(-1., 1., -1.),
                Vec3::new(1., 1., -1.),
                Vec3::new(0., 1., 1.),
                Vec3::new(0., 1.2, 0.),
            ],
            &[0, 1, 2],
            &[true, true, true, false],
        )
        .unwrap();
        let mut unchecked = world.clone();
        let cfg = DeformConfig {
            self_collision: true,
            ..config()
        };
        for _ in 0..120 {
            world.step(cfg, &[]);
            unchecked.step(config(), &[]);
        }
        assert!(world.positions[3].y >= 1.0399);
        assert!(unchecked.positions[3].y < 0.1);
        assert_eq!(world.positions[0], Vec3::new(-1., 1., -1.));
    }
    #[test]
    fn contact_friction_slows_sliding_without_pulling_cloth_into_ground() {
        let points = vec![
            Vec3::new(0., 0.02, 0.),
            Vec3::new(0.2, 0.02, 0.),
            Vec3::new(0., 0.02, 0.2),
        ];
        let mut slippery = DeformWorld::new(points.clone(), &[0, 1, 2], &[false; 3]).unwrap();
        slippery.velocities.fill(Vec3::X * 2.);
        let mut rough = slippery.clone();
        let smooth = config();
        let friction = DeformConfig {
            friction: 0.8,
            ..smooth
        };
        for _ in 0..120 {
            slippery.step(smooth, &[]);
            rough.step(friction, &[]);
        }
        assert!(slippery.positions[0].x > rough.positions[0].x * 3.);
        assert!(rough
            .positions
            .iter()
            .all(|p| p.is_finite() && p.y >= 0.0199));
        assert!(rough.velocities[0].x.abs() < 0.05);
    }
    #[test]
    fn wind_is_spatially_coherent_across_duplicate_uv_vertices() {
        let points = vec![
            Vec3::new(0., 1., 0.),
            Vec3::new(0.2, 1., 0.),
            Vec3::new(0., 1., 0.2),
            Vec3::new(0., 1., 0.),
        ];
        let mut world = DeformWorld::new(points, &[], &[false; 4]).unwrap();
        let cfg = DeformConfig {
            gravity: 0.,
            wind: 10.,
            ..config()
        };
        for _ in 0..120 {
            world.step(cfg, &[]);
        }
        assert_eq!(world.positions[0], world.positions[3]);
        assert!(world.positions.iter().all(|p| p.is_finite()));
    }
    #[test]
    fn swept_box_blocks_fast_particles_and_allows_tangential_motion() {
        let c = Collider {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        let p = c.project(Vec3::new(-2., 0.5, 0.5), Vec3::new(2., 0.8, 0.5), 0.01);
        assert!(p.x < 0.);
        assert!(p.y > 0.5);
    }
    #[test]
    fn cloth_pins_and_contacts_remain_finite() {
        let mut w = DeformWorld::new(
            vec![
                Vec3::new(-0.5, 2., 0.),
                Vec3::new(0.5, 2., 0.),
                Vec3::new(-0.5, 1., 0.),
                Vec3::new(0.5, 1., 0.),
            ],
            &[0, 1, 2, 1, 3, 2],
            &[true, true, false, false],
        )
        .unwrap();
        let start = w.positions.clone();
        let cfg = DeformConfig {
            self_collision: false,
            bending: 8.,
            friction: 0.35,
            iterations: 8,
            compliance: 0.00001,
            damping: 0.02,
            gravity: 9.81,
            wind: 2.,
            radius: 0.02,
            activation: 0.,
            contraction: 0.3,
            muscle: false,
        };
        for _ in 0..600 {
            w.step(cfg, &[]);
        }
        assert_eq!(w.positions[0], start[0]);
        assert!(w.positions.iter().all(|v| v.is_finite() && v.y >= 0.));
        assert!((w.positions[2] - w.positions[0]).length() < 1.2);
    }
    #[test]
    fn muscle_activation_shortens_a_free_segment() {
        let mut w = DeformWorld::new(
            vec![
                Vec3::new(0., 1., 0.),
                Vec3::new(0., 2., 0.),
                Vec3::new(0.1, 2., 0.),
            ],
            &[0, 1, 2],
            &[true, false, false],
        )
        .unwrap();
        let cfg = DeformConfig {
            self_collision: false,
            bending: 8.,
            friction: 0.35,
            iterations: 8,
            compliance: 0.000001,
            damping: 0.2,
            gravity: 0.,
            wind: 0.,
            radius: 0.01,
            activation: 1.,
            contraction: 0.4,
            muscle: true,
        };
        for _ in 0..240 {
            w.step(cfg, &[]);
        }
        assert!((w.positions[1] - w.positions[0]).length() < 0.7);
    }

    #[test]
    fn cancelled_cloth_tick_keeps_the_previous_state_and_can_resume() {
        let mut world = DeformWorld::new(vec![Vec3::new(0., 2., 0.), Vec3::new(1., 2., 0.), Vec3::new(0., 3., 0.)],
            &[0, 1, 2], &[true, false, false]).unwrap();
        world.step(config(), &[]);
        let mut expected = world.clone();
        let positions = world.positions.clone(); let velocities = world.velocities.clone(); let time = world.time;
        let cancel = AtomicBool::new(true);
        assert!(!world.step_with_mesh_cancellable(config(), &[], &MeshCollider::default(), &cancel));
        assert_eq!(world.positions, positions); assert_eq!(world.velocities, velocities); assert_eq!(world.time, time);
        cancel.store(false, Ordering::Relaxed);
        assert!(world.step_with_mesh_cancellable(config(), &[], &MeshCollider::default(), &cancel));
        expected.step(config(), &[]);
        assert_eq!(world.positions, expected.positions); assert_eq!(world.time, expected.time);
    }
    #[test]
    fn uv_duplicates_stay_welded_under_soft_constraints_and_share_normals() {
        let mut world = DeformWorld::new(vec![Vec3::new(0., 2., 0.), Vec3::new(1., 2., 0.), Vec3::new(0., 3., 0.),
            Vec3::new(0., 2., 0.), Vec3::new(0., 2., 1.), Vec3::new(0., 3., 0.)],
            &[0, 1, 2, 3, 5, 4], &[false; 6]).unwrap();
        world.velocities[0] = Vec3::X * 10.; world.velocities[3] = -Vec3::X * 10.;
        let cfg = DeformConfig { compliance: 0.1, self_collision: true, ..config() };
        for _ in 0..12 { world.step(cfg, &[]); }
        assert_eq!(world.positions[0], world.positions[3]);
        assert_eq!(world.positions[2], world.positions[5]);
        let normals = world.normals(&[0, 1, 2, 3, 5, 4]);
        assert_eq!(normals[0], normals[3]); assert_eq!(normals[2], normals[5]);
        assert!(world.positions.iter().all(|p| p.is_finite()));
    }
    #[test]
    fn pinning_one_uv_copy_pins_the_whole_physical_vertex() {
        let world = DeformWorld::new(vec![Vec3::Y, Vec3::Y, Vec3::X + Vec3::Y],
            &[0, 1, 2], &[true, false, false]).unwrap();
        assert_eq!(world.inverse_mass, vec![0., 0., 1.]);
    }

    #[test]
    fn external_wind_moves_free_particles_and_preserves_pins() {
        let p=vec![Vec3::new(0.,1.,0.),Vec3::new(1.,1.,0.),Vec3::new(0.,2.,0.)];
        let mut world=DeformWorld::new(p.clone(),&[0,1,2],&[true,false,false]).unwrap();
        world.set_fields(crate::fields::Fields::new(vec![crate::fields::Field{kind:1,center:Vec3::Y,axis:Vec3::Z,strength:10.,radius:20.,falloff:0.,damping:0.}],glam::Mat4::IDENTITY).unwrap());
        let cfg=DeformConfig{gravity:0.,wind:0.,..config()};for _ in 0..24 {world.step(cfg,&[]);}
        assert_eq!(world.positions[0],p[0]);assert!(world.positions[1].z>0.01);assert!(world.positions.iter().all(|p|p.is_finite()));
    }

}
