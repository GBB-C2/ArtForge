//! Revision-keyed mesh adjacency and a lazily built triangle BVH for component picking.
use glam::Vec3;
use poseforge_core::{Mesh, MeshConnectivity};
use std::sync::OnceLock;

pub(super) struct MeshCache {
    pub key: (u64, u64, u64, u64),
    pub graph: MeshConnectivity,
    mesh: Mesh,
    tree: OnceLock<PickTree>,
}
impl MeshCache {
    pub fn matches(&self, mesh: &Mesh) -> bool {
        self.key == Self::key(mesh)
            && self.mesh.topology.as_ref().map(|t| t.loose_edges.as_slice())
                == mesh.topology.as_ref().map(|t| t.loose_edges.as_slice())
    }
    pub fn key(mesh: &Mesh) -> (u64, u64, u64, u64) {
        (
            mesh.id,
            mesh.vertices.revision(),
            mesh.indices.revision(),
            mesh.topology.as_ref().map_or(0, |t| t.faces.revision()),
        )
    }
    pub fn new(mesh: &Mesh) -> Self {
        Self {
            key: Self::key(mesh),
            graph: MeshConnectivity::new(mesh),
            mesh: mesh.clone(),
            tree: OnceLock::new(),
        }
    }
    pub fn hit(&self, origin: Vec3, direction: Vec3) -> Option<(usize, f32)> {
        self.tree
            .get_or_init(|| PickTree::new(&self.mesh))
            .hit(&self.mesh, origin, direction)
    }
}
struct Branch {
    min: Vec3,
    max: Vec3,
    start: usize,
    count: usize,
    children: Option<(usize, usize)>,
}
struct PickTree {
    order: Vec<usize>,
    nodes: Vec<Branch>,
}
impl PickTree {
    fn new(mesh: &Mesh) -> Self {
        let mut tree = Self {
            order: (0..mesh.indices.len() / 3).collect(),
            nodes: Vec::new(),
        };
        if !tree.order.is_empty() {
            tree.build(mesh, 0, tree.order.len());
        }
        tree
    }
    fn build(&mut self, mesh: &Mesh, start: usize, count: usize) -> usize {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &t in &self.order[start..start + count] {
            for &v in &mesh.indices[t * 3..t * 3 + 3] {
                let p = Vec3::from_array(mesh.vertices[v as usize].position);
                min = min.min(p);
                max = max.max(p);
            }
        }
        let id = self.nodes.len();
        self.nodes.push(Branch {
            min,
            max,
            start,
            count,
            children: None,
        });
        if count > 8 {
            let size = max - min;
            let axis = if size.x >= size.y && size.x >= size.z {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let center = |t: usize| {
                mesh.indices[t * 3..t * 3 + 3]
                    .iter()
                    .map(|&v| mesh.vertices[v as usize].position[axis])
                    .sum::<f32>()
            };
            self.order[start..start + count]
                .select_nth_unstable_by(count / 2, |&a, &b| center(a).total_cmp(&center(b)));
            let left = self.build(mesh, start, count / 2);
            let right = self.build(mesh, start + count / 2, count - count / 2);
            self.nodes[id].children = Some((left, right));
        }
        id
    }
    fn hit(&self, mesh: &Mesh, origin: Vec3, direction: Vec3) -> Option<(usize, f32)> {
        if self.nodes.is_empty() || !origin.is_finite() || !direction.is_finite() {
            return None;
        }
        let mut best = f32::INFINITY;
        let mut result = None;
        let mut stack = vec![0];
        while let Some(id) = stack.pop() {
            let n = &self.nodes[id];
            let mut near = 0_f32;
            let mut far = best;
            let mut inside = true;
            for axis in 0..3 {
                if direction[axis].abs() < 1e-12 {
                    if origin[axis] < n.min[axis] - 1e-5 || origin[axis] > n.max[axis] + 1e-5 {
                        inside = false;
                        break;
                    }
                } else {
                    let a = (n.min[axis] - origin[axis]) / direction[axis];
                    let b = (n.max[axis] - origin[axis]) / direction[axis];
                    near = near.max(a.min(b) - 1e-5);
                    far = far.min(a.max(b) + 1e-5);
                    if near > far {
                        inside = false;
                        break;
                    }
                }
            }
            if !inside {
                continue;
            }
            if let Some((a, b)) = n.children {
                stack.extend([b, a]);
                continue;
            }
            for &t in &self.order[n.start..n.start + n.count] {
                let p =
                    |i| Vec3::from_array(mesh.vertices[mesh.indices[t * 3 + i] as usize].position);
                let a = p(0);
                let e1 = p(1) - a;
                let e2 = p(2) - a;
                let h = direction.cross(e2);
                let det = e1.dot(h);
                if det.abs() < 1e-12 {
                    continue;
                }
                let s = origin - a;
                let u = s.dot(h) / det;
                let q = s.cross(e1);
                let v = direction.dot(q) / det;
                let distance = e2.dot(q) / det;
                if u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 && distance >= 0. && distance < best
                {
                    best = distance;
                    result = Some((t, distance));
                }
            }
        }
        result
    }
}
