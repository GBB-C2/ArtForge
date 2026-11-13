//! Truncate selected geometric corners while retaining polygon boundaries and UV seams.
use crate::mesh_topology_tools::{faces, finish, invalid};
use crate::{
    position_key, CoreError, Mesh, MeshConnectivity, MeshTopology, Vec3, Vertex,
    MAX_EDIT_TRIANGLES, MAX_EDIT_VERTICES, MAX_FACE_CORNERS,
};
use std::collections::{BTreeMap, BTreeSet};

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

// A consistently oriented manifold fan is one cycle, or one path at a boundary.
fn fan_order(arcs: &[(usize, usize)]) -> Result<Vec<usize>, CoreError> {
    let mut next = BTreeMap::new();
    let mut incoming = BTreeSet::new();
    for &(a, b) in arcs {
        if a == b || next.insert(a, b).is_some() || !incoming.insert(b) {
            return Err(invalid("У вершины разветвлённые или перевёрнутые грани"));
        }
    }
    let starts: Vec<_> = next.keys().filter(|a| !incoming.contains(*a)).copied().collect();
    let closed = starts.is_empty();
    if starts.len() > 1 || next.is_empty() {
        return Err(invalid("Грани вокруг вершины должны образовывать одну окрестность"));
    }
    let start = starts.first().copied().unwrap_or_else(|| *next.first_key_value().unwrap().0);
    let mut order = Vec::new();
    let mut at = start;
    loop {
        if order.len() >= MAX_FACE_CORNERS || order.contains(&at) {
            return Err(invalid("Слишком сложная окрестность вершины для фаски"));
        }
        order.push(at);
        let Some(n) = next.remove(&at) else { break; };
        at = n;
        if closed && at == start { break; }
    }
    if !next.is_empty() || (closed && at != start) || order.len() < if closed { 3 } else { 2 } {
        return Err(invalid("Несвязная окрестность вершины для фаски"));
    }
    Ok(order)
}

fn cut_vertex(a: Vertex, b: Vertex, amount: f32, position: Vec3) -> Vertex {
    Vertex {
        position: position.to_array(),
        uv: std::array::from_fn(|i| a.uv[i] + (b.uv[i] - a.uv[i]) * amount),
        color: std::array::from_fn(|i| a.color[i] + (b.color[i] - a.color[i]) * amount),
        ..a
    }
}

pub(crate) fn apply(mesh: &mut Mesh, selection: &BTreeSet<usize>, amount: f32) -> Result<(), CoreError> {
    if selection.is_empty() || selection.iter().any(|&i| i >= mesh.vertices.len()) {
        return Err(invalid("Выберите вершины для фаски"));
    }
    if !amount.is_finite() || !(0.0..=0.45).contains(&amount) {
        return Err(invalid("Срез вершины: от 0 до 45% соседних рёбер"));
    }
    if amount == 0. { return Ok(()); }
    let smooth = mesh.smooth_normals_enabled();
    let seams = crate::uv_seams(mesh);
    let g = MeshConnectivity::new(mesh);
    let selected: BTreeSet<_> = selection.iter().map(|&i| g.canonical[i]).collect();
    let source = faces(mesh);
    let source_materials: Vec<_> = (0..source.len()).map(|i| {
        mesh.topology.as_ref().and_then(|t| t.face_materials.get(i).copied().flatten())
    }).collect();
    let positions: Vec<_> = mesh.vertices.iter().map(|v| Vec3::from_array(v.position)).collect();
    let mut cuts = BTreeMap::new();
    for &v in &selected {
        let neighbors = &g.vertex_neighbors[v];
        if !(2..=MAX_FACE_CORNERS).contains(&neighbors.len()) {
            return Err(invalid("Фаска требует связанную вершину с 2–128 соседями"));
        }
        for &n in neighbors {
            if g.edge_faces[&edge(v, n)].len() > 2 {
                return Err(invalid("Фаска не поддерживает разветвлённое ребро"));
            }
            let p = positions[v].lerp(positions[n], amount);
            let key = position_key(p.to_array());
            if !p.is_finite() || key == position_key(positions[v].to_array())
                || key == position_key(positions[n].to_array()) {
                return Err(invalid("Срез слишком мал для точности сетки"));
            }
            cuts.insert((v, n), p);
        }
    }
    // Preflight the entire selection before allocating replacement geometry.
    let mut fans = BTreeMap::<usize, Vec<(usize, usize)>>::new();
    let mut cap_materials = BTreeMap::<usize, Option<u64>>::new();
    let mut corners = 0;
    let mut triangles = 0;
    for (fi, face) in source.iter().enumerate() {
        let mut count = 0;
        let normal = crate::polygon_edit::face_normal(mesh, face).normalize_or_zero();
        for i in 0..face.len() {
            let v = g.canonical[face[i] as usize];
            if !selected.contains(&v) { continue; }
            let before = g.canonical[face[(i + face.len() - 1) % face.len()] as usize];
            let after = g.canonical[face[(i + 1) % face.len()] as usize];
            let incoming = (positions[v] - positions[before]).normalize_or_zero();
            let outgoing = (positions[after] - positions[v]).normalize_or_zero();
            if incoming.cross(outgoing).dot(normal) <= 0.00001 {
                return Err(invalid("Для среза нужен выпуклый ненулевой угол грани"));
            }
            fans.entry(v).or_default().push((after, before));
            cap_materials.entry(v).and_modify(|m| {
                if *m != source_materials[fi] { *m = None; }
            }).or_insert(source_materials[fi]);
            count += 1;
        }
        if face.len() + count > MAX_FACE_CORNERS {
            return Err(invalid("После фаски грань превысит предел в 128 углов"));
        }
        triangles += face.len() + count - 2;
        corners += count;
    }
    let mut rings = BTreeMap::new();
    for &v in &selected {
        let order = fan_order(fans.get(&v).ok_or_else(|| invalid("Вершина не принадлежит грани"))?)?;
        if order.len() != g.vertex_neighbors[v].len() {
            return Err(invalid("Несвязная окрестность вершины"));
        }
        triangles += order.len().saturating_sub(2);
        rings.insert(v, order);
    }
    let cap_corners: usize = rings.values().filter(|r| r.len() >= 3).map(Vec::len).sum();
    if mesh.vertices.len() + corners * 2 + cap_corners > MAX_EDIT_VERTICES
        || triangles > MAX_EDIT_TRIANGLES {
        return Err(invalid("Фаска превышает предел сетки; уменьшите выбор"));
    }
    let mut output = Vec::with_capacity(source.len() + rings.len());
    let mut materials = source_materials;
    let mut samples = BTreeMap::new();
    for face in &source {
        let mut next = Vec::new();
        for i in 0..face.len() {
            let raw = face[i] as usize;
            let v = g.canonical[raw];
            if !selected.contains(&v) { next.push(raw as u32); continue; }
            for raw_n in [face[(i + face.len() - 1) % face.len()], face[(i + 1) % face.len()]] {
                let n = g.canonical[raw_n as usize];
                let cut = cut_vertex(mesh.vertices[raw], mesh.vertices[raw_n as usize], amount, cuts[&(v, n)]);
                next.push(mesh.vertices.len() as u32);
                mesh.vertices.push(cut);
                samples.entry((v, n)).or_insert(cut);
            }
        }
        output.push(next);
    }
    for (v, order) in rings {
        if order.len() < 3 { continue; } // A single-face boundary corner is just clipped.
        let mut cap = Vec::new();
        for n in order {
            cap.push(mesh.vertices.len() as u32);
            mesh.vertices.push(samples[&(v, n)]);
        }
        let normal = crate::polygon_edit::face_normal(mesh, &cap).try_normalize()
            .ok_or_else(|| invalid("Фаска создаёт вырожденный срез"))?;
        let tangent = normal.any_orthonormal_vector();
        let bitangent = normal.cross(tangent);
        let origin = Vec3::from_array(mesh.vertices[cap[0] as usize].position);
        let projected: Vec<_> = cap.iter().map(|&i| {
            let p = Vec3::from_array(mesh.vertices[i as usize].position) - origin;
            [p.dot(tangent), p.dot(bitangent)]
        }).collect();
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for p in &projected { for i in 0..2 { min[i] = min[i].min(p[i]); max[i] = max[i].max(p[i]); } }
        for (&id, p) in cap.iter().zip(projected) {
            mesh.vertices[id as usize].uv = std::array::from_fn(|i| (p[i] - min[i]) / (max[i] - min[i]).max(1e-8));
        }
        output.push(cap);
        materials.push(cap_materials[&v]);
    }
    mesh.topology = Some(MeshTopology {
        faces: output.clone().into(), face_materials: materials.into(), uv_seams: Vec::new(),
        primitive: None, smooth_normals: smooth,
    });
    finish(mesh, output, smooth)?;
    let result = MeshConnectivity::new(mesh);
    if result.edge_faces.values().any(|fs| fs.len() > 2)
        || (g.edge_faces.values().all(|fs| fs.len() == 2)
            && result.edge_faces.values().any(|fs| fs.len() != 2)) {
        return Err(invalid("Фаска не смогла сохранить связность; уменьшите срез"));
    }
    let points: BTreeMap<_, _> = mesh.vertices.iter().enumerate()
        .map(|(i, v)| (position_key(v.position), result.canonical[i])).collect();
    let mut marked = BTreeSet::new();
    for (a, b) in seams {
        let a = g.canonical[a];
        let b = g.canonical[b];
        let start = cuts.get(&(a, b)).copied().unwrap_or(positions[a]);
        let end = cuts.get(&(b, a)).copied().unwrap_or(positions[b]);
        if let (Some(&a), Some(&b)) = (points.get(&position_key(start.to_array())), points.get(&position_key(end.to_array()))) {
            if result.edges.contains(&edge(a, b)) { marked.insert([a.min(b) as u32, a.max(b) as u32]); }
        }
    }
    mesh.topology.as_mut().unwrap().uv_seams = marked.into_iter().collect();
    Ok(())
}
