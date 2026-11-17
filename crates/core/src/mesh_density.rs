//! Selection-scoped edge collapse with boundary, material and fold protection.
use crate::mesh_topology_tools::{faces, invalid};
use crate::{CoreError, Mesh, MeshConnectivity, MeshRegion, Vec3};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn selected(mesh: &Mesh, region: &MeshRegion) -> BTreeSet<usize> {
    let graph = MeshConnectivity::new(mesh);
    let vertices: BTreeSet<_> = region
        .vertices
        .iter()
        .filter_map(|&v| graph.canonical.get(v).copied())
        .collect();
    let edges: BTreeSet<_> = region
        .edges
        .iter()
        .filter_map(|&(a, b)| {
            let a = *graph.canonical.get(a)?;
            let b = *graph.canonical.get(b)?;
            Some((a.min(b), a.max(b)))
        })
        .collect();
    (0..mesh.face_count())
        .filter(|&f| {
            if region.faces.contains(&f) {
                return true;
            }
            let ring = mesh.face_vertices(f).unwrap();
            if !vertices.is_empty()
                && ring
                    .iter()
                    .all(|&v| vertices.contains(&graph.canonical[v as usize]))
            {
                return true;
            }
            (0..ring.len()).any(|i| {
                let a = graph.canonical[ring[i] as usize];
                let b = graph.canonical[ring[(i + 1) % ring.len()] as usize];
                edges.contains(&(a.min(b), a.max(b)))
            })
        })
        .collect()
}

pub(crate) fn simplify(mesh: &mut Mesh, region: &MeshRegion, ratio: f32) -> Result<(), CoreError> {
    if !ratio.is_finite() || !(0.01..=1.).contains(&ratio) {
        return Err(invalid("Доля оставшейся сетки: 1–100%"));
    }
    let selection = selected(mesh, region);
    if selection.is_empty() {
        return Err(invalid("Выберите область для упрощения"));
    }
    if ratio == 1. {
        return Ok(());
    }
    let graph = MeshConnectivity::new(mesh);
    let source = faces(mesh);
    let mut output = Vec::new();
    let mut materials = Vec::new();
    let mut active = Vec::new();
    for (f, face) in source.iter().enumerate() {
        if selection.contains(&f) {
            for tri in crate::triangulate::triangulate(mesh, face)?.chunks_exact(3) {
                output.push(tri.to_vec());
                materials.push(mesh.face_material(f));
                active.push(true);
            }
        } else {
            output.push(face.clone());
            materials.push(mesh.face_material(f));
            active.push(false);
        }
    }
    let initial = active.iter().filter(|&&a| a).count();
    // Small tools have a strict work budget. Larger meshes use the same command on a smaller region.
    if initial > 20000 {
        return Err(invalid("Упрощайте участками до 20000 треугольников"));
    }
    let target = ((initial as f32 * ratio).ceil() as usize).max(2);
    let mut roots = graph.canonical.clone();
    let mut positions: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| Vec3::from_array(v.position))
        .collect();
    let mut locked = BTreeSet::new();
    for (&(a, b), adj) in &graph.edge_faces {
        let selected_count = adj.iter().filter(|f| selection.contains(f)).count();
        if selected_count != 2
            || adj.len() != 2
            || mesh.face_material(adj[0]) != mesh.face_material(adj[1])
        {
            locked.extend([a, b]);
        }
    }
    for (a, b) in crate::uv_seams(mesh) {
        locked.extend([graph.canonical[a], graph.canonical[b]]);
    }
    let mut remaining = initial;
    while remaining > target {
        let mut adj = BTreeMap::<(usize, usize), Vec<usize>>::new();
        let mut neighbors = BTreeMap::<usize, BTreeSet<usize>>::new();
        let mut incident = BTreeMap::<usize, BTreeSet<usize>>::new();
        for (f, ring) in output.iter().enumerate().filter(|(_, r)| r.len() >= 3) {
            for i in 0..ring.len() {
                let a = roots[ring[i] as usize];
                let b = roots[ring[(i + 1) % ring.len()] as usize];
                incident.entry(a).or_default().insert(f);
                adj.entry((a.min(b), a.max(b))).or_default().push(f);
                neighbors.entry(a).or_default().insert(b);
                neighbors.entry(b).or_default().insert(a);
            }
        }
        let mut candidates = Vec::new();
        for (&(a, b), around) in &adj {
            if locked.contains(&a)
                || locked.contains(&b)
                || around.len() != 2
                || around.iter().any(|&f| !active[f])
            {
                continue;
            }
            if neighbors[&a].intersection(&neighbors[&b]).count() != 2 {
                continue;
            }
            let midpoint = (positions[a] + positions[b]) * 0.5;
            let mut cost = 0.;
            let mut valid = true;
            for &f in incident[&a].union(&incident[&b]) {
                let ring = &output[f];
                let ids: Vec<_> = ring.iter().map(|&v| roots[v as usize]).collect();
                if !ids.contains(&a) && !ids.contains(&b) {
                    continue;
                }
                if !active[f] {
                    valid = false;
                    break;
                }
                let p: Vec<_> = ids.iter().map(|&v| positions[v]).collect();
                let normal = (p[1] - p[0]).cross(p[2] - p[0]);
                let n = normal.normalize_or_zero();
                cost += (midpoint - p[0]).dot(n).powi(2);
                if ids.contains(&a) && ids.contains(&b) {
                    continue;
                }
                let q: Vec<_> = ids
                    .iter()
                    .map(|&v| {
                        if v == a || v == b {
                            midpoint
                        } else {
                            positions[v]
                        }
                    })
                    .collect();
                let next = (q[1] - q[0]).cross(q[2] - q[0]);
                if next.length_squared() < normal.length_squared() * 1e-8
                    || n.dot(next.normalize_or_zero()) < 0.2
                {
                    valid = false;
                    break;
                }
            }
            if valid {
                candidates.push((
                    cost + positions[a].distance_squared(positions[b]) * 1e-4,
                    a,
                    b,
                    midpoint,
                ));
            }
        }
        candidates.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| (a.1, a.2).cmp(&(b.1, b.2)))
        });
        if candidates.is_empty() {
            break;
        }
        // A batch of disjoint one-rings avoids rebuilding adjacency per edge.
        let mut touched = BTreeSet::new();
        let mut changed = false;
        for (_, a, b, p) in candidates {
            if remaining <= target {
                break;
            }
            if touched.contains(&a) || touched.contains(&b) {
                continue;
            }
            touched.extend(neighbors[&a].iter().copied());
            touched.extend(neighbors[&b].iter().copied());
            touched.extend([a, b]);
            for root in &mut roots {
                if *root == b {
                    *root = a;
                }
            }
            positions[a] = p;
            for ring in &mut output {
                if ring.len() == 3
                    && ring
                        .iter()
                        .map(|&v| roots[v as usize])
                        .collect::<BTreeSet<_>>()
                        .len()
                        < 3
                {
                    ring.clear();
                    remaining -= 1;
                }
            }
            changed = true;
        }
        if !changed {
            break;
        }
    }
    if remaining == initial {
        return Err(invalid(
            "Нет безопасных рёбер для упрощения: расширьте выделение",
        ));
    }
    for (i, v) in mesh.vertices.iter_mut().enumerate() {
        v.position = positions[roots[i]].to_array();
    }
    let kept: Vec<_> = output
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.is_empty())
        .map(|(i, _)| materials[i])
        .collect();
    let smooth = mesh.smooth_normals_enabled();
    crate::mesh_topology_tools::finish(
        mesh,
        output.into_iter().filter(|f| !f.is_empty()).collect(),
        smooth,
    )?;
    mesh.topology.as_mut().unwrap().face_materials = kept.into();
    Ok(())
}

pub(crate) fn mirror(
    mesh: &mut Mesh,
    region: &MeshRegion,
    axis: usize,
    plane: f32,
    merge: bool,
    distance: f32,
) -> Result<(), CoreError> {
    if axis > 2
        || !plane.is_finite()
        || plane.abs() > 10000.
        || !distance.is_finite()
        || !(0. ..=1000.).contains(&distance)
    {
        return Err(invalid(
            "Некорректная плоскость зеркала или расстояние объединения",
        ));
    }
    let selected = selected(mesh, region);
    if selected.is_empty() {
        return Err(invalid("Выберите грани для зеркала"));
    }
    crate::component_mesh::ensure_topology(mesh);
    let original = mesh.clone();
    let mut output = faces(mesh);
    let mut materials: Vec<_> = (0..mesh.face_count())
        .map(|f| mesh.face_material(f))
        .collect();
    if merge
        && selected.iter().all(|&f| {
            original
                .face_vertices(f)
                .unwrap()
                .iter()
                .all(|&v| (original.vertices[v as usize].position[axis] - plane).abs() <= distance)
        })
    {
        return Ok(());
    }
    let key = |positions: Vec<[f32; 3]>| {
        let mut k: Vec<_> = positions
            .into_iter()
            .map(|p| p.map(|x| (x as f64 * 1e6).round() as i64))
            .collect();
        k.sort();
        k
    };
    let mut occupied: BTreeSet<_> = output
        .iter()
        .map(|ring| {
            key(ring
                .iter()
                .map(|&i| {
                    let mut p = original.vertices[i as usize].position;
                    if merge && (p[axis] - plane).abs() <= distance {
                        p[axis] = plane;
                    }
                    p
                })
                .collect())
        })
        .collect();
    let mut copies = BTreeMap::new();
    for &f in &selected {
        let ring = original.face_vertices(f).unwrap();
        if merge
            && ring
                .iter()
                .all(|&v| (original.vertices[v as usize].position[axis] - plane).abs() <= distance)
        {
            output[f].clear();
            continue;
        }
        let mut reflected = Vec::new();
        for &v in ring.iter().rev() {
            let id = *copies.entry(v).or_insert_with(|| {
                let mut vertex = original.vertices[v as usize];
                if merge && (vertex.position[axis] - plane).abs() <= distance {
                    vertex.position[axis] = plane;
                    mesh.vertices[v as usize].position[axis] = plane;
                }
                vertex.position[axis] = 2. * plane - vertex.position[axis];
                vertex.normal[axis] *= -1.;
                let id = mesh.vertices.len() as u32;
                mesh.vertices.push(vertex);
                id
            });
            reflected.push(id);
        }
        if occupied.insert(key(reflected
            .iter()
            .map(|&i| mesh.vertices[i as usize].position)
            .collect()))
        {
            output.push(reflected);
            materials.push(original.face_material(f));
        }
    }
    let kept: Vec<_> = output
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.is_empty())
        .map(|(i, _)| materials[i])
        .collect();
    crate::mesh_topology_tools::finish(
        mesh,
        output.into_iter().filter(|f| !f.is_empty()).collect(),
        original.smooth_normals_enabled(),
    )?;
    mesh.topology.as_mut().unwrap().face_materials = kept.into();
    Ok(())
}
