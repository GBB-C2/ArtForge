//! Interpolate a region's inset through boundary subdivisions and interior vertices.
use crate::{Mesh, MeshConnectivity, Vec3};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn boundary(
    source: &[Vec<u32>],
    graph: &MeshConnectivity,
    faces: &BTreeSet<usize>,
) -> BTreeSet<(usize, usize)> {
    let mut edges = BTreeMap::<(usize, usize), usize>::new();
    for &f in faces {
        let face = &source[f];
        for i in 0..face.len() {
            let a = graph.canonical[face[i] as usize];
            let b = graph.canonical[face[(i + 1) % face.len()] as usize];
            if a != b {
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
    }
    edges
        .into_iter()
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
        .collect()
}

pub(crate) fn straighten_samples(
    positions: &[Vec3],
    edges: &BTreeSet<(usize, usize)>,
    fixed: &mut BTreeMap<usize, Vec3>,
) {
    let mut neighbors = BTreeMap::<usize, Vec<usize>>::new();
    for &(a, b) in edges {
        neighbors.entry(a).or_default().push(b);
        neighbors.entry(b).or_default().push(a);
    }
    let straight: BTreeSet<_> = neighbors
        .iter()
        .filter_map(|(&v, near)| {
            (near.len() == 2
                && fixed.contains_key(&v)
                && (positions[near[0]] - positions[v])
                    .normalize_or_zero()
                    .dot((positions[near[1]] - positions[v]).normalize_or_zero())
                    < -0.99999)
                .then_some(v)
        })
        .collect();
    let mut visited = BTreeSet::new();
    for (&start, next) in &neighbors {
        if straight.contains(&start) {
            continue;
        }
        for &first in next {
            if !straight.contains(&first) || visited.contains(&first) {
                continue;
            }
            let mut chain = Vec::new();
            let mut previous = start;
            let mut at = first;
            let mut length = positions[start].distance(positions[first]);
            while straight.contains(&at) && visited.insert(at) {
                chain.push((at, length));
                let next = neighbors[&at]
                    .iter()
                    .copied()
                    .find(|&v| v != previous)
                    .unwrap();
                length += positions[at].distance(positions[next]);
                previous = at;
                at = next;
            }
            let a = fixed.get(&start).copied().unwrap_or(Vec3::ZERO);
            let b = fixed.get(&at).copied().unwrap_or(Vec3::ZERO);
            for (v, d) in chain {
                fixed.insert(v, a.lerp(b, d / length));
            }
        }
    }
}

pub(crate) fn interpolate(
    source: &[Vec<u32>],
    graph: &MeshConnectivity,
    selected: &BTreeSet<usize>,
    positions: &[Vec3],
    mut fixed: BTreeMap<usize, Vec3>,
) -> BTreeMap<usize, Vec3> {
    let outline = boundary(source, graph, selected);
    if outline.is_empty() {
        return BTreeMap::new();
    }
    for &(a, b) in &outline {
        fixed.entry(a).or_default();
        fixed.entry(b).or_default();
    }
    straighten_samples(positions, &outline, &mut fixed);
    let mut neighbors = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &f in selected {
        let face = &source[f];
        for i in 0..face.len() {
            let a = graph.canonical[face[i] as usize];
            let b = graph.canonical[face[(i + 1) % face.len()] as usize];
            if a != b {
                neighbors.entry(a).or_default().insert(b);
                neighbors.entry(b).or_default().insert(a);
            }
        }
    }
    if neighbors.keys().all(|v| fixed.contains_key(v)) {
        return fixed;
    }
    let normals: BTreeMap<_, _> = selected
        .iter()
        .map(|&f| {
            let face = &source[f];
            (
                f,
                (1..face.len() - 1)
                    .map(|i| {
                        (positions[face[i] as usize] - positions[face[0] as usize])
                            .cross(positions[face[i + 1] as usize] - positions[face[0] as usize])
                    })
                    .sum::<Vec3>()
                    .normalize_or_zero(),
            )
        })
        .collect();
    for (&a, near) in &neighbors {
        for &b in near {
            if a >= b {
                continue;
            }
            let adj = &graph.edge_faces[&(a, b)];
            if adj.len() == 2
                && adj.iter().all(|f| selected.contains(f))
                && normals[&adj[0]].dot(normals[&adj[1]]) < 0.9999
            {
                fixed.entry(a).or_default();
                fixed.entry(b).or_default();
            }
        }
    }
    let free: Vec<_> = neighbors
        .keys()
        .copied()
        .filter(|v| !fixed.contains_key(v))
        .collect();
    if free.is_empty() {
        return fixed;
    }
    let ids: BTreeMap<_, _> = free.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    let degree: Vec<_> = free.iter().map(|v| neighbors[v].len() as f32).collect();
    let adjacent: Vec<Vec<_>> = free
        .iter()
        .map(|v| {
            neighbors[v]
                .iter()
                .filter_map(|n| ids.get(n).copied())
                .collect()
        })
        .collect();
    let rhs: Vec<Vec3> = free
        .iter()
        .map(|v| {
            neighbors[v]
                .iter()
                .filter_map(|n| fixed.get(n))
                .copied()
                .sum()
        })
        .collect();
    // Jacobi-preconditioned conjugate gradients solve all three coordinates
    // together. No dense matrix or dependence on the number of boundary samples.
    let dot = |a: &[Vec3], b: &[Vec3]| a.iter().zip(b).map(|(a, b)| a.dot(*b) as f64).sum::<f64>();
    let mut result = vec![Vec3::ZERO; free.len()];
    let mut residual = rhs.clone();
    let mut z: Vec<_> = residual.iter().zip(&degree).map(|(r, &d)| *r / d).collect();
    let mut direction = z.clone();
    let mut rho = dot(&residual, &z);
    let threshold = dot(&rhs, &rhs) * 1e-12;
    for _ in 0..256 {
        if dot(&residual, &residual) <= threshold.max(1e-24) {
            break;
        }
        let product: Vec<_> = direction
            .iter()
            .enumerate()
            .map(|(i, &p)| p * degree[i] - adjacent[i].iter().map(|&j| direction[j]).sum::<Vec3>())
            .collect();
        let denominator = dot(&direction, &product);
        if denominator <= 0. {
            break;
        }
        let alpha = (rho / denominator) as f32;
        for i in 0..free.len() {
            result[i] += direction[i] * alpha;
            residual[i] -= product[i] * alpha;
            z[i] = residual[i] / degree[i];
        }
        let next = dot(&residual, &z);
        let beta = (next / rho) as f32;
        for i in 0..free.len() {
            direction[i] = z[i] + direction[i] * beta;
        }
        rho = next;
    }
    for (v, delta) in free.into_iter().zip(result) {
        fixed.insert(v, delta);
    }
    fixed
}

pub(crate) fn positions(mesh: &Mesh) -> Vec<Vec3> {
    mesh.vertices
        .iter()
        .map(|v| Vec3::from_array(v.position))
        .collect()
}
