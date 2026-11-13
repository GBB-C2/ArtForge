//! Stable, position-welded adjacency for selection and polygon operations.
use crate::{position_key, Mesh};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct MeshConnectivity {
    pub canonical: Vec<usize>,
    pub edges: BTreeSet<(usize, usize)>,
    pub edge_faces: BTreeMap<(usize, usize), Vec<usize>>,
    pub vertex_neighbors: Vec<BTreeSet<usize>>,
    pub face_neighbors: Vec<BTreeSet<usize>>,
}
impl MeshConnectivity {
    pub fn connected_faces(&self, seed: usize) -> BTreeSet<usize> {
        if seed>=self.face_neighbors.len() { return BTreeSet::new(); }
        let mut result = BTreeSet::from([seed]); let mut pending = vec![seed];
        while let Some(face) = pending.pop() { for &next in &self.face_neighbors[face] {
            if result.insert(next) { pending.push(next); }
        } }
        result
    }

    pub fn new(mesh: &Mesh) -> Self {
        let mut weld = BTreeMap::new();
        let canonical: Vec<_> = mesh
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| *weld.entry(position_key(v.position)).or_insert(i))
            .collect();
        let mut edge_faces = BTreeMap::<(usize, usize), Vec<usize>>::new();
        let mut vertex_neighbors = vec![BTreeSet::new(); mesh.vertices.len()];
        for f in 0..mesh.face_count() {
            let face = mesh.face_vertices(f).unwrap();
            for j in 0..face.len() {
                let a = canonical[face[j] as usize];
                let b = canonical[face[(j + 1) % face.len()] as usize];
                if a == b {
                    continue;
                }
                edge_faces.entry((a.min(b), a.max(b))).or_default().push(f);
                vertex_neighbors[a].insert(b);
                vertex_neighbors[b].insert(a);
            }
        }
        let mut face_neighbors = vec![BTreeSet::new(); mesh.face_count()];
        for faces in edge_faces.values() {
            // Invalid nonmanifold fans must not cause a quadratic allocation.
            if faces.len() <= 8 {
                for &a in faces {
                    for &b in faces {
                        if a != b {
                            face_neighbors[a].insert(b);
                        }
                    }
                }
            }
        }
        Self {
            canonical,
            edges: edge_faces.keys().copied().collect(),
            edge_faces,
            vertex_neighbors,
            face_neighbors,
        }
    }
}
