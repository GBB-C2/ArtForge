//! Bounded, revision-keyed cache; repeated object picks do not deform or scan a whole mesh.
use super::*;
use std::{
    cell::RefCell,
    collections::VecDeque,
    hash::{Hash, Hasher},
    rc::Rc,
};
const CACHE_BYTES: usize = 96 * 1024 * 1024;
thread_local! { static CACHE: RefCell<VecDeque<(u64, Rc<Geometry>)>> = const { RefCell::new(VecDeque::new()) }; }
struct Branch {
    min: DVec3,
    max: DVec3,
    start: usize,
    count: usize,
    children: Option<(usize, usize)>,
}
pub(super) struct Geometry {
    pub positions: Vec<DVec3>,
    pub faces: Vec<usize>,
    order: Vec<usize>,
    branches: Vec<Branch>,
}
pub(super) fn cached(
    mesh: &poseforge_core::Mesh,
    model: Mat4,
    joints: &Option<Vec<Mat4>>,
    weights: &[f32],
    build: impl FnOnce() -> Option<Vec<DVec3>>,
) -> Option<Rc<Geometry>> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    joints.as_ref().map(Vec::len).hash(&mut hash);
    weights.len().hash(&mut hash);
    mesh.vertices.revision().hash(&mut hash);
    mesh.indices.revision().hash(&mut hash);
    mesh.topology
        .as_ref()
        .map(|t| t.faces.revision())
        .hash(&mut hash);
    for v in model
        .to_cols_array()
        .into_iter()
        .chain(joints.iter().flatten().flat_map(|m| m.to_cols_array()))
        .chain(weights.iter().copied())
    {
        v.to_bits().hash(&mut hash);
    }
    for t in &mesh.morph_targets {
        t.positions.revision().hash(&mut hash);
    }
    let key = hash.finish();
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache.iter().position(|(k, _)| *k == key) {
            let entry = cache.remove(index).unwrap();
            let result = entry.1.clone();
            cache.push_back(entry);
            return Some(result);
        }
        let positions = build()?;
        if mesh.indices.iter().any(|i| *i as usize >= positions.len()) {
            return None;
        }
        let mut geometry = Geometry {
            positions,
            faces: Vec::with_capacity(mesh.indices.len() / 3),
            order: (0..mesh.indices.len() / 3).collect(),
            branches: Vec::new(),
        };
        if let Some(topology) = &mesh.topology {
            for (i, face) in topology.faces.iter().enumerate() {
                geometry
                    .faces
                    .extend(std::iter::repeat_n(i, face.len().saturating_sub(2)));
            }
        } else {
            geometry.faces.extend(0..mesh.indices.len() / 3);
        }
        if geometry.faces.len() != geometry.order.len() {
            return None;
        }
        if !geometry.order.is_empty() {
            geometry.build(mesh, 0, geometry.order.len());
        }
        let result = Rc::new(geometry);
        let bytes = result.bytes();
        while !cache.is_empty()
            && (cache.len() >= 16
                || cache.iter().map(|(_, g)| g.bytes()).sum::<usize>() + bytes > CACHE_BYTES)
        {
            cache.pop_front();
        }
        if bytes <= CACHE_BYTES {
            cache.push_back((key, result.clone()));
        }
        Some(result)
    })
}
impl Geometry {
    fn bytes(&self) -> usize {
        self.positions.len() * 24
            + self.faces.len() * 8
            + self.order.len() * 8
            + self.branches.len() * std::mem::size_of::<Branch>()
    }
    fn build(&mut self, mesh: &poseforge_core::Mesh, start: usize, count: usize) -> usize {
        let mut min = DVec3::splat(f64::INFINITY);
        let mut max = DVec3::splat(f64::NEG_INFINITY);
        for &triangle in &self.order[start..start + count] {
            for &vertex in &mesh.indices[triangle * 3..triangle * 3 + 3] {
                let p = self.positions[vertex as usize];
                min = min.min(p);
                max = max.max(p);
            }
        }
        let index = self.branches.len();
        self.branches.push(Branch {
            min,
            max,
            start,
            count,
            children: None,
        });
        if count > 12 {
            let size = max - min;
            let axis = if size.x >= size.y && size.x >= size.z {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let positions = &self.positions;
            let center = |triangle: usize| {
                mesh.indices[triangle * 3..triangle * 3 + 3]
                    .iter()
                    .map(|&v| positions[v as usize][axis])
                    .sum::<f64>()
            };
            self.order[start..start + count]
                .select_nth_unstable_by(count / 2, |&a, &b| center(a).total_cmp(&center(b)));
            let left = self.build(mesh, start, count / 2);
            let right = self.build(mesh, start + count / 2, count - count / 2);
            self.branches[index].children = Some((left, right));
        }
        index
    }
    pub fn candidates(&self, origin: DVec3, direction: DVec3) -> Vec<usize> {
        let mut result = Vec::new();
        if self.branches.is_empty() {
            return result;
        }
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            let b = &self.branches[index];
            if !super::intersects_bounds(origin, direction, b.min, b.max, f64::INFINITY) {
                continue;
            }
            if let Some((left, right)) = b.children {
                stack.push(left);
                stack.push(right);
            } else {
                result.extend_from_slice(&self.order[b.start..b.start + b.count]);
            }
        }
        result.sort_unstable();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Mesh, Vertex};
    #[test]
    fn dense_mesh_picks_reuse_geometry_and_visit_only_ray_neighborhood() {
        let n = 320usize;
        let mut mesh = Mesh {
            id: 1,
            name: "Dense grid".into(),
            vertices: Vec::new().into(),
            indices: Vec::new().into(),
            material: None,
            topology: None,
            morph_targets: vec![],
        };
        for y in 0..=n {
            for x in 0..=n {
                mesh.vertices.push(Vertex {
                    position: [x as f32, y as f32, 0.],
                    ..Default::default()
                });
            }
        }
        for y in 0..n {
            for x in 0..n {
                let a = (y * (n + 1) + x) as u32;
                let b = a + (n + 1) as u32;
                mesh.indices
                    .extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
            }
        }
        let start = std::time::Instant::now();
        let geometry = cached(&mesh, Mat4::IDENTITY, &None, &[], || {
            Some(
                mesh.vertices
                    .iter()
                    .map(|v| Vec3::from_array(v.position).as_dvec3())
                    .collect(),
            )
        })
        .unwrap();
        let cold = start.elapsed();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let reused = cached(&mesh, Mat4::IDENTITY, &None, &[], || {
                panic!("cache must not rebuild unchanged geometry")
            })
            .unwrap();
            assert!(Rc::ptr_eq(&geometry, &reused));
            let candidates = reused.candidates(DVec3::new(140.2, 170.3, 2.), DVec3::NEG_Z);
            assert!(
                !candidates.is_empty() && candidates.len() < 100,
                "{} candidates",
                candidates.len()
            );
        }
        eprintln!(
            "204800 triangles: cold {:?}; 100 cached searches {:?}",
            cold,
            start.elapsed()
        );
        mesh.vertices[0].position[2] = 1.;
        let changed = cached(&mesh, Mat4::IDENTITY, &None, &[], || {
            Some(
                mesh.vertices
                    .iter()
                    .map(|v| Vec3::from_array(v.position).as_dvec3())
                    .collect(),
            )
        })
        .unwrap();
        assert!(!Rc::ptr_eq(&geometry, &changed));
    }
}
