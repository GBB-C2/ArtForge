//! Two-sided swept particle/triangle contacts with a deterministic median BVH.
use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Triangle(pub [Vec3; 3]);
impl Triangle {
    pub fn closest(self, p: Vec3) -> (Vec3, [f32; 3]) {
        let [a, b, c] = self.0;
        let ab = b - a;
        let ac = c - a;
        let ap = p - a;
        let d1 = ab.dot(ap);
        let d2 = ac.dot(ap);
        if d1 <= 0. && d2 <= 0. {
            return (a, [1., 0., 0.]);
        }
        let bp = p - b;
        let d3 = ab.dot(bp);
        let d4 = ac.dot(bp);
        if d3 >= 0. && d4 <= d3 {
            return (b, [0., 1., 0.]);
        }
        let vc = d1 * d4 - d3 * d2;
        if vc <= 0. && d1 >= 0. && d3 <= 0. {
            let v = d1 / (d1 - d3);
            return (a + v * ab, [1. - v, v, 0.]);
        }
        let cp = p - c;
        let d5 = ab.dot(cp);
        let d6 = ac.dot(cp);
        if d6 >= 0. && d5 <= d6 {
            return (c, [0., 0., 1.]);
        }
        let vb = d5 * d2 - d1 * d6;
        if vb <= 0. && d2 >= 0. && d6 <= 0. {
            let w = d2 / (d2 - d6);
            return (a + w * ac, [1. - w, 0., w]);
        }
        let va = d3 * d6 - d5 * d4;
        if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            return (b + w * (c - b), [0., 1. - w, w]);
        }
        let denominator = va + vb + vc;
        if denominator.abs() < 1e-20 {
            return (a, [1., 0., 0.]);
        }
        let v = vb / denominator;
        let w = vc / denominator;
        (a + ab * v + ac * w, [1. - v - w, v, w])
    }
    /// Correction and triangle barycentrics; preserves the particle's original side.
    pub fn contact(self, start: Vec3, end: Vec3, radius: f32) -> Option<(Vec3, [f32; 3])> {
        let [a, b, c] = self.0;
        let normal = (b - a).cross(c - a).try_normalize()?;
        let before = (start - a).dot(normal);
        let after = (end - a).dot(normal);
        let side = if before < 0. { -1. } else { 1. };
        let n = normal * side;
        if before * after <= 0. && (before - after).abs() > 1e-8 {
            let hit = start + (end - start) * (before / (before - after));
            let (q, weights) = self.closest(hit);
            if q.distance_squared(hit) < 1e-9 {
                return Some((n * (radius - after * side), weights));
            }
        }
        let (q, weights) = self.closest(end);
        let d = end - q;
        let length = d.length();
        if length >= radius {
            return None;
        }
        let direction = d.try_normalize().unwrap_or(n);
        let direction = if direction.dot(n) < 0. { n } else { direction };
        Some((direction * (radius - length), weights))
    }
}
#[derive(Clone)]
struct Branch {
    min: Vec3,
    max: Vec3,
    start: usize,
    end: usize,
    children: Option<(usize, usize)>,
}
#[derive(Clone, Default)]
pub struct MeshCollider {
    triangles: Vec<Triangle>,
    order: Vec<usize>,
    branches: Vec<Branch>,
}
impl MeshCollider {
    /// Indices returned by `candidates` address this immutable triangle array.
    pub fn triangle(&self, index: usize) -> Triangle { self.triangles[index] }
    pub fn occluded(&self, start: Vec3, end: Vec3, scratch: &mut Vec<usize>) -> bool {
        self.candidates(start, end, 0., scratch);
        let direction = end - start;
        for &i in scratch.iter() {
            let [a, b, c] = self.triangles[i].0;
            let edge = b - a;
            let cross = direction.cross(c - a);
            let det = edge.dot(cross);
            if det.abs() < 1e-10 {
                continue;
            }
            let s = start - a;
            let u = s.dot(cross) / det;
            if !(0. ..=1.).contains(&u) {
                continue;
            }
            let q = s.cross(edge);
            let v = direction.dot(q) / det;
            if v < 0. || u + v > 1. {
                continue;
            }
            let t = (c - a).dot(q) / det;
            if t > 1e-5 && t < 0.9999 {
                return true;
            }
        }
        false
    }
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, String> {
        if triangles.len() > 200_000 || triangles.iter().any(|t| t.0.iter().any(|p| !p.is_finite()))
        {
            return Err("Столкновения: до 200000 корректных треугольников".into());
        }
        let mut result = Self {
            order: (0..triangles.len()).collect(),
            triangles,
            branches: vec![],
        };
        if !result.order.is_empty() {
            result.build(0, result.order.len());
        }
        Ok(result)
    }
    /// Refit a fixed topology in linear time. Triangle IDs and the partition stay
    /// stable, so cloth can update contacts without sorting a new BVH each iteration.
    pub fn refit(&mut self, mut triangle: impl FnMut(usize) -> Triangle) {
        for (i, value) in self.triangles.iter_mut().enumerate() { *value = triangle(i); }
        for index in (0..self.branches.len()).rev() {
            let branch = &self.branches[index];
            let (min, max) = if let Some((left, right)) = branch.children {
                (self.branches[left].min.min(self.branches[right].min),
                 self.branches[left].max.max(self.branches[right].max))
            } else {
                self.order[branch.start..branch.end].iter().flat_map(|&i| self.triangles[i].0)
                    .fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                        |(min, max), p| (min.min(p), max.max(p)))
            };
            self.branches[index].min = min;
            self.branches[index].max = max;
        }
    }
    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &id in &self.order[start..end] {
            for p in self.triangles[id].0 {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let index = self.branches.len();
        self.branches.push(Branch {
            min,
            max,
            start,
            end,
            children: None,
        });
        if end - start > 8 {
            let size = max - min;
            let axis = if size.x >= size.y && size.x >= size.z {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let middle = start + (end - start) / 2;
            self.order[start..end].select_nth_unstable_by(middle - start, |a, b| {
                let centroid = |i: usize| self.triangles[i].0.iter().map(|p| p[axis]).sum::<f32>();
                centroid(*a).total_cmp(&centroid(*b)).then(a.cmp(b))
            });
            let left = self.build(start, middle);
            let right = self.build(middle, end);
            self.branches[index].children = Some((left, right));
        }
        index
    }
    pub fn candidates(&self, start: Vec3, end: Vec3, radius: f32, output: &mut Vec<usize>) {
        output.clear();
        if self.branches.is_empty() {
            return;
        }
        let min = start.min(end) - Vec3::splat(radius);
        let max = start.max(end) + Vec3::splat(radius);
        // Median splitting and the 200k-triangle limit bound depth below 20.
        // Avoid one heap allocation for every particle/contact query.
        let mut stack = [0_usize; 64];
        let mut len = 1;
        while len > 0 {
            len -= 1;
            let index = stack[len];
            let b = &self.branches[index];
            if b.min.cmpgt(max).any() || b.max.cmplt(min).any() {
                continue;
            }
            if let Some((left, right)) = b.children {
                stack[len] = right;
                stack[len + 1] = left;
                len += 2;
            } else {
                output.extend_from_slice(&self.order[b.start..b.end]);
            }
        }
        output.sort_unstable();
    }
    pub fn project(
        &self,
        start: Vec3,
        mut end: Vec3,
        radius: f32,
        candidates: &mut Vec<usize>,
    ) -> Vec3 {
        self.candidates(start, end, radius, candidates);
        for &index in candidates.iter() {
            if let Some((push, _)) = self.triangles[index].contact(start, end, radius) {
                end += push;
            }
        }
        end
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_contacts_follow_slope_and_do_not_fill_empty_bounding_box() {
        let t = Triangle([
            Vec3::new(-2., 0., -2.),
            Vec3::new(2., 2., -2.),
            Vec3::new(0., 1., 2.),
        ]);
        let b = MeshCollider::new(vec![t]).unwrap();
        let mut cache = vec![];
        let p = b.project(
            Vec3::new(0., 3., 0.),
            Vec3::new(0., -1., 0.),
            0.02,
            &mut cache,
        );
        let n = (t.0[1] - t.0[0]).cross(t.0[2] - t.0[0]).normalize();
        assert!((p - t.0[0]).dot(n).abs() >= 0.0199);
        let outside = Vec3::new(1.9, 0.8, 1.9);
        assert_eq!(
            b.project(outside + Vec3::Y, outside, 0.02, &mut cache),
            outside
        );
    }
    #[test]
    fn thin_triangle_catches_both_sides_and_edge_proximity() {
        let t = Triangle([Vec3::ZERO, Vec3::X * 2., Vec3::Z * 2.]);
        for sign in [-1., 1.] {
            let (d, _) = t
                .contact(Vec3::new(0.2, sign, 0.2), Vec3::new(0.2, -sign, 0.2), 0.03)
                .unwrap();
            assert!(((-sign + d.y) - sign * 0.03).abs() < 1e-5);
        }
        assert!(t
            .contact(
                Vec3::new(-0.01, 0.01, 0.4),
                Vec3::new(-0.01, 0.01, 0.4),
                0.03
            )
            .is_some());
    }
}

/// Closed triangle volume used as an inward fluid container. UV seams are welded
/// for validation, while queries retain the exact authored triangle positions.
#[derive(Clone)]
pub struct MeshVolume {
    collider: MeshCollider,
    min: Vec3,
    max: Vec3,
    epsilon: f32,
    signature: u64,
}
impl MeshVolume {
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, String> {
        if triangles.is_empty() || triangles.len() > 200_000
            || triangles.iter().any(|t| t.0.iter().any(|p| !p.is_finite())) {
            return Err("Для жидкости нужен замкнутый меш до 200000 треугольников".into());
        }
        let min = triangles.iter().flat_map(|t| t.0).fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let max = triangles.iter().flat_map(|t| t.0).fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        let epsilon = ((max - min).max_element() * 1e-6).max(1e-7);
        let key = |p: Vec3| ((p - min) / epsilon).to_array().map(|x| x.round() as i64);
        let mut edges = std::collections::BTreeMap::new();
        let mut valid = Vec::new();
        let mut signature = 0xcbf29ce484222325u64;
        for triangle in triangles {
            let [a, b, c] = triangle.0;
            let keys = [key(a), key(b), key(c)];
            if keys[0] == keys[1] || keys[1] == keys[2] || keys[2] == keys[0]
                || (b - a).cross(c - a).length_squared() < epsilon.powi(4) { continue; }
            for (a, b) in [(keys[0], keys[1]), (keys[1], keys[2]), (keys[2], keys[0])] {
                *edges.entry((a.min(b), a.max(b))).or_insert(0usize) += 1;
            }
            for byte in triangle.0.into_iter().flat_map(|p| p.to_array()).flat_map(|v| v.to_bits().to_le_bytes()) {
                signature = (signature ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
            valid.push(triangle);
        }
        if valid.is_empty() || edges.values().any(|count| *count != 2) {
            return Err("Жидкости нужна замкнутая поверхность без отверстий и общих рёбер более чем двух граней".into());
        }
        Ok(Self { collider: MeshCollider::new(valid)?, min, max, epsilon, signature })
    }
    pub fn signature(&self) -> u64 { self.signature }

    pub fn contains(&self, point: Vec3) -> bool {
        if !point.is_finite() || point.cmplt(self.min).any() || point.cmpgt(self.max).any() { return false; }
        // An oblique ray avoids the poles and axis-aligned face diagonals of primitives.
        let direction = Vec3::new(1., 0.37139, 0.52917).normalize();
        let ray = direction * ((self.max - self.min).length() * 3. + 1.);
        let mut candidates = Vec::new();
        self.collider.candidates(point, point + ray, self.epsilon, &mut candidates);
        let mut hits = Vec::new();
        for index in candidates {
            let [a, b, c] = self.collider.triangles[index].0;
            let edge = b - a;
            let cross = ray.cross(c - a);
            let determinant = edge.dot(cross);
            if determinant.abs() < 1e-10 { continue; }
            let offset = point - a;
            let u = offset.dot(cross) / determinant;
            let q = offset.cross(edge);
            let v = ray.dot(q) / determinant;
            let t = (c - a).dot(q) / determinant;
            if u >= -1e-6 && v >= -1e-6 && u + v <= 1. + 1e-6 && (0. ..=1.).contains(&t) {
                hits.push(t);
            }
        }
        hits.sort_by(f32::total_cmp);
        let tolerance = self.epsilon / ray.length();
        hits.dedup_by(|a, b| (*a - *b).abs() <= tolerance);
        hits.len() % 2 == 1
    }

    pub fn closest(&self, point: Vec3) -> Vec3 {
        let mut best = point;
        let mut distance = f32::INFINITY;
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            let branch = &self.collider.branches[index];
            if point.distance_squared(point.clamp(branch.min, branch.max)) > distance { continue; }
            if let Some((left, right)) = branch.children {
                stack.push(right); stack.push(left);
            } else {
                for &triangle in &self.collider.order[branch.start..branch.end] {
                    let candidate = self.collider.triangles[triangle].closest(point).0;
                    let d = point.distance_squared(candidate);
                    if d < distance { distance = d; best = candidate; }
                }
            }
        }
        best
    }

    pub fn accepts_particle(&self, point: Vec3, radius: f32) -> bool {
        self.contains(point) && point.distance_squared(self.closest(point)) >= radius * radius
    }
    pub fn project(&self, start: Vec3, end: Vec3, radius: f32, scratch: &mut Vec<usize>) -> Vec3 {
        let first = self.collider.project(start, end, radius, scratch);
        self.collider.project(start, first, radius, scratch)
    }
}

#[cfg(test)]
mod volume_tests {
    use super::*;
    fn octahedron(scale: f32) -> Vec<Triangle> {
        let v = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z].map(|p| p * scale);
        [[2,0,4],[2,4,1],[2,1,5],[2,5,0],[3,4,0],[3,1,4],[3,5,1],[3,0,5]]
            .map(|t| Triangle(t.map(|i| v[i]))).to_vec()
    }
    #[test]
    fn volume_uses_the_surface_instead_of_its_bounding_box_and_rejects_holes() {
        let shape = MeshVolume::new(octahedron(1.)).unwrap();
        assert!(shape.contains(Vec3::ZERO));
        assert!(shape.contains(Vec3::new(0.2, 0.2, 0.2)));
        assert!(!shape.contains(Vec3::new(0.8, 0.8, 0.)));
        assert!(!shape.accepts_particle(Vec3::new(0.99, 0., 0.), 0.03));
        assert!(shape.closest(Vec3::new(2., 0., 0.)).distance(Vec3::X) < 1e-5);
        let mut open = octahedron(1.);
        open.pop();
        assert!(MeshVolume::new(open).is_err());
    }
    #[test]
    fn fluid_keeps_particles_inside_the_authored_volume_and_replays_deterministically() {
        let config = crate::FluidConfig { bounds_min: Vec3::splat(-1.), bounds_max: Vec3::ONE, ..Default::default() };
        let mut world = crate::FluidWorld::new(config, vec![Vec3::ZERO, Vec3::new(0.2, 0.2, 0.)]).unwrap();
        world.set_container(MeshVolume::new(octahedron(1.)).unwrap()).unwrap();
        for _ in 0..240 {
            world.step();
            assert!(world.positions().iter().all(|p| p.x.abs() + p.y.abs() + p.z.abs() <= 1.0001));
        }
        let snapshot = world.snapshot();
        world.reset();
        for _ in 0..240 { world.step(); }
        assert_eq!(world.positions(), snapshot.positions.as_slice());
        world.set_container(MeshVolume::new(octahedron(0.9)).unwrap()).unwrap();
        assert!(matches!(world.restore(&snapshot), Err(crate::FluidError::StaleCache)));
    }

    #[test]
    fn refit_moves_contact_bounds_without_repartitioning_triangle_ids() {
        let triangles: Vec<_> = (0..32).map(|i| {
            let p = Vec3::new(i as f32 * 3., 0., 0.);
            Triangle([p, p + Vec3::X, p + Vec3::Z])
        }).collect();
        let mut collider = MeshCollider::new(triangles.clone()).unwrap();
        let order = collider.order.clone();
        collider.refit(|i| Triangle(triangles[i].0.map(|p| p + Vec3::Y * 5.)));
        assert_eq!(collider.order, order);
        let fresh = MeshCollider::new(triangles.iter().map(|t| Triangle(t.0.map(|p| p + Vec3::Y * 5.))).collect()).unwrap();
        for i in 0..32 {
            let start = Vec3::new(i as f32 * 3. + 0.2, 6., 0.2);
            let end = start - Vec3::Y * 2.;
            let actual = collider.project(start, end, 0.02, &mut vec![]);
            let expected = fresh.project(start, end, 0.02, &mut vec![]);
            assert!(actual.distance(expected) < 1e-6);
            assert!(actual.y > 5.);
        }
    }

}
