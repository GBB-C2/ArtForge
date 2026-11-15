//! Stitch coincident results without discarding per-corner UVs or material seams.
use crate::{CoreError, Mesh, MeshMergeMap, Vertex};
use std::collections::{BTreeMap, BTreeSet};
fn key(mesh: &Mesh, face: &[u32]) -> Vec<[i64; 3]> {
    let mut key: Vec<_> = face
        .iter()
        .map(|&v| crate::position_key(mesh.vertices[v as usize].position))
        .collect();
    key.sort_unstable();
    key.dedup();
    key
}
fn attributes(v: &Vertex) -> Vec<u32> {
    v.position
        .iter()
        .chain(&v.normal)
        .chain(&v.uv)
        .chain(&v.color)
        .chain(&v.weights)
        .map(|&v| if v == 0. { 0 } else { v.to_bits() })
        .chain(v.joints)
        .collect()
}
pub(crate) fn apply(mesh: &mut Mesh, source: &Mesh) -> Result<MeshMergeMap, CoreError> {
    crate::component_mesh::ensure_topology(mesh);
    let old_faces = crate::mesh_topology_tools::faces(mesh);
    let source_faces: BTreeSet<_> = (0..source.face_count())
        .map(|f| key(source, source.face_vertices(f).unwrap()))
        .collect();
    let mut active = vec![false; mesh.vertices.len()];
    let changed: Vec<_> = old_faces
        .iter()
        .enumerate()
        .map(|(i, f)| i >= source.face_count() || !source_faces.contains(&key(mesh, f)))
        .collect();
    for (i, face) in old_faces.iter().enumerate() {
        if changed[i] {
            for &v in face {
                active[v as usize] = true;
            }
        }
    }
    for i in source.vertices.len()..active.len() {
        active[i] = true;
    }
    let mut groups = BTreeMap::<[i64; 3], Vec<usize>>::new();
    for (i, v) in mesh.vertices.iter().enumerate() {
        groups
            .entry(crate::position_key(v.position))
            .or_default()
            .push(i);
    }
    let active_groups: BTreeSet<_> = groups
        .iter()
        .filter_map(|(&key, group)| group.iter().any(|&i| active[i]).then_some(key))
        .collect();
    for (key, indices) in &groups {
        if !active_groups.contains(key) {
            continue;
        }
        let position = mesh.vertices[indices[0]].position;
        for &i in indices {
            mesh.vertices[i].position = position;
        }
    }
    let mut vertices = Vec::new();
    let mut seen = BTreeMap::new();
    let mut vertex_map = Vec::new();
    for &v in mesh.vertices.iter() {
        let signature = attributes(&v);
        let shared = active_groups.contains(&crate::position_key(v.position));
        let id = if shared {
            *seen.entry(signature).or_insert_with(|| {
                let id = vertices.len();
                vertices.push(v);
                id
            })
        } else {
            let id = vertices.len();
            vertices.push(v);
            id
        };
        vertex_map.push(id);
    }
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    let mut origins = Vec::new();
    let mut aliases = vec![None; old_faces.len()];
    let mut seen_faces = BTreeMap::<Vec<[i64; 3]>, usize>::new();
    let mut removed = BTreeSet::new();
    for (i, face) in old_faces.iter().enumerate() {
        let mut ring = Vec::new();
        for &v in face {
            if ring.last().is_none_or(|&last| {
                crate::position_key(mesh.vertices[last as usize].position)
                    != crate::position_key(mesh.vertices[v as usize].position)
            }) {
                ring.push(v);
            }
        }
        if ring.len() > 1
            && crate::position_key(mesh.vertices[ring[0] as usize].position)
                == crate::position_key(mesh.vertices[*ring.last().unwrap() as usize].position)
        {
            ring.pop();
        }
        let signature = key(mesh, &ring);
        if changed[i]
            && (signature.len() < 3
                || crate::polygon_edit::face_normal(mesh, &ring).length_squared() < 1e-20)
        {
            continue;
        }
        if let Some(&previous) = seen_faces.get(&signature) {
            if changed[i] || changed[previous] {
                let a = crate::polygon_edit::face_normal(mesh, &old_faces[previous])
                    .normalize_or_zero();
                let b = crate::polygon_edit::face_normal(mesh, &ring).normalize_or_zero();
                if a.dot(b) < -0.99999 {
                    removed.insert(previous);
                    removed.insert(i);
                    seen_faces.remove(&signature);
                } else {
                    aliases[i] = Some(previous);
                }
                continue;
            }
        }
        seen_faces.insert(signature, i);
        origins.push(i);
        faces.push(
            ring.iter()
                .map(|&v| vertex_map[v as usize] as u32)
                .collect::<Vec<_>>(),
        );
        materials.push(mesh.face_material(i));
    }
    let mut face_map = vec![None; old_faces.len()];
    let mut kept = Vec::new();
    let mut kept_materials = Vec::new();
    for ((face, material), origin) in faces.into_iter().zip(materials).zip(origins) {
        if !removed.contains(&origin) {
            face_map[origin] = Some(kept.len());
            kept.push(face);
            kept_materials.push(material);
        }
    }
    for (i, alias) in aliases.into_iter().enumerate() {
        if let Some(target) = alias {
            face_map[i] = face_map[target];
        }
    }
    let edges: Vec<_> = mesh
        .topology
        .as_ref()
        .unwrap()
        .loose_edges
        .iter()
        .map(|e| e.map(|v| vertex_map[v as usize]))
        .collect();
    let topology = mesh.topology.as_mut().unwrap();
    topology.faces = kept.into();
    topology.face_materials = kept_materials.into();
    let mut unique = BTreeSet::new();
    topology.loose_edges = edges
        .iter()
        .filter_map(|&[a, b]| {
            let key = (a.min(b), a.max(b));
            (a != b && unique.insert(key)).then_some([a as u32, b as u32])
        })
        .collect();
    topology.uv_seams = topology
        .uv_seams
        .iter()
        .filter_map(|e| {
            let a = vertex_map[e[0] as usize];
            let b = vertex_map[e[1] as usize];
            (a != b).then_some([a as u32, b as u32])
        })
        .collect();
    mesh.vertices = vertices.into();
    crate::component_mesh::finish(mesh)?;
    Ok(MeshMergeMap {
        vertices: vertex_map,
        faces: face_map,
        loose_edges: edges,
    })
}
