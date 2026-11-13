//! Seam-aware conformal charts, deterministic packing and editable UV transforms.
use crate::{CoreError, Mesh, MeshConnectivity, MeshTopology, Vec3, MAX_EDIT_VERTICES};
use glam::Vec2;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug)]
pub enum UvEdit {
    Seams {
        edges: BTreeSet<(usize, usize)>,
        marked: bool,
    },
    Unwrap,
    Pack {
        margin: f32,
    },
    Transform {
        vertices: BTreeSet<usize>,
        offset: [f32; 2],
        angle: f32,
        scale: f32,
    },
}
fn error(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}
fn topology(mesh: &mut Mesh) {
    if mesh.topology.is_none() {
        mesh.topology = Some(MeshTopology { loose_edges: Vec::new(),
            face_materials: Default::default(),
            faces: (0..mesh.face_count())
                .map(|f| mesh.face_vertices(f).unwrap().to_vec())
                .collect(),
            smooth_normals: false,
            primitive: None,
            uv_seams: Vec::new(),
        });
    }
}
pub fn uv_seams(mesh: &Mesh) -> BTreeSet<(usize, usize)> {
    let graph = MeshConnectivity::new(mesh);
    mesh.topology
        .iter()
        .flat_map(|t| &t.uv_seams)
        .filter_map(|&[a, b]| {
            Some(edge(
                *graph.canonical.get(a as usize)?,
                *graph.canonical.get(b as usize)?,
            ))
        })
        .collect()
}
fn uv_key(v: [f32; 2]) -> [i64; 2] {
    v.map(|x| (x as f64 * 1e7).round() as i64)
}
/// Islands follow both explicit seams and existing discontinuities in UV0.
pub fn uv_islands(mesh: &Mesh) -> Vec<Vec<usize>> {
    let g = MeshConnectivity::new(mesh);
    let seams = uv_seams(mesh);
    let mut neighbors = vec![Vec::new(); mesh.face_count()];
    type Side = (usize, [i64; 2], [i64; 2]);
    let mut sides = BTreeMap::<(usize, usize), Vec<Side>>::new();
    for f in 0..mesh.face_count() {
        let face = mesh.face_vertices(f).unwrap();
        for j in 0..face.len() {
            let a = face[j] as usize;
            let b = face[(j + 1) % face.len()] as usize;
            let ca = g.canonical[a];
            let cb = g.canonical[b];
            let (a, b) = if ca < cb { (a, b) } else { (b, a) };
            sides.entry(edge(ca, cb)).or_default().push((
                f,
                uv_key(mesh.vertices[a].uv),
                uv_key(mesh.vertices[b].uv),
            ));
        }
    }
    for (edge, faces) in sides {
        if faces.len() == 2
            && !seams.contains(&edge)
            && faces[0].1 == faces[1].1
            && faces[0].2 == faces[1].2
        {
            neighbors[faces[0].0].push(faces[1].0);
            neighbors[faces[1].0].push(faces[0].0);
        }
    }
    components(&neighbors)
}
fn components(neighbors: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; neighbors.len()];
    let mut groups = Vec::new();
    for start in 0..neighbors.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut group = Vec::new();
        while let Some(f) = queue.pop_front() {
            group.push(f);
            for &n in &neighbors[f] {
                if !seen[n] {
                    seen[n] = true;
                    queue.push_back(n);
                }
            }
        }
        groups.push(group);
    }
    groups
}
fn root(parent: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while parent[r] != r {
        r = parent[r];
    }
    let mut k = i;
    while parent[k] != k {
        let next = parent[k];
        parent[k] = r;
        k = next;
    }
    r
}
fn join(parent: &mut [usize], a: usize, b: usize) {
    let a = root(parent, a);
    let b = root(parent, b);
    parent[b] = a;
}

pub fn edit_uv(mesh: &mut Mesh, edit: UvEdit, cancel: &AtomicBool) -> Result<(), CoreError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(error("Операция UV отменена"));
    }
    match edit {
        UvEdit::Seams { edges, marked } => {
            let g = MeshConnectivity::new(mesh);
            if edges.is_empty() || edges.iter().any(|e| !g.edges.contains(e)) {
                return Err(error("Выберите рёбра для UV-шва"));
            }
            let mut seams = uv_seams(mesh);
            for e in edges {
                if marked {
                    seams.insert(e);
                } else {
                    seams.remove(&e);
                }
            }
            topology(mesh);
            mesh.topology.as_mut().unwrap().uv_seams = seams
                .into_iter()
                .map(|(a, b)| [a as u32, b as u32])
                .collect();
        }
        UvEdit::Unwrap => unwrap(mesh, cancel)?,
        UvEdit::Pack { margin } => pack(mesh, margin, cancel)?,
        UvEdit::Transform {
            vertices,
            offset,
            angle,
            scale,
        } => {
            if vertices.is_empty()
                || vertices.iter().any(|&i| i >= mesh.vertices.len())
                || !offset.iter().all(|v| v.is_finite() && v.abs() <= 10000.)
                || !angle.is_finite()
                || angle.abs() > 36000.
                || !scale.is_finite()
                || !(0.001..=1000.).contains(&scale)
            {
                return Err(error("Выберите UV и задайте конечное преобразование"));
            }
            let unique: BTreeSet<_> = vertices
                .iter()
                .map(|&i| uv_key(mesh.vertices[i].uv))
                .collect();
            let pivot = unique
                .iter()
                .map(|k| Vec2::new(k[0] as f32 / 1e7, k[1] as f32 / 1e7))
                .sum::<Vec2>()
                / unique.len() as f32;
            let (sn, cs) = angle.to_radians().sin_cos();
            for i in vertices {
                let p = (Vec2::from_array(mesh.vertices[i].uv) - pivot) * scale;
                let q = pivot
                    + Vec2::new(cs * p.x - sn * p.y, sn * p.x + cs * p.y)
                    + Vec2::from_array(offset);
                if !q.is_finite() || q.abs().max_element() > 1e6 {
                    return Err(error("UV выходит за допустимые координаты"));
                }
                mesh.vertices[i].uv = q.to_array();
            }
        }
    }
    if let Some(t) = &mut mesh.topology {
        t.primitive = None;
    }
    Ok(())
}
fn unwrap(mesh: &mut Mesh, cancel: &AtomicBool) -> Result<(), CoreError> {
    let g = MeshConnectivity::new(mesh);
    let seams = uv_seams(mesh);
    let faces: Vec<_> = (0..mesh.face_count())
        .map(|f| mesh.face_vertices(f).unwrap().to_vec())
        .collect();
    let normals: Vec<_> = faces
        .iter()
        .map(|f| crate::polygon_edit::face_normal(mesh, f).normalize_or_zero())
        .collect();
    // Without authored seams use bounded-angle charts. Authored seams retain
    // curved charts and split corner connectivity, including a cylinder's slit.
    let mut charts = Vec::new();
    let mut assigned = vec![false; faces.len()];
    for seed in 0..faces.len() {
        if assigned[seed] {
            continue;
        }
        let mut chart = Vec::new();
        let mut q = VecDeque::from([seed]);
        assigned[seed] = true;
        while let Some(f) = q.pop_front() {
            chart.push(f);
            let face = &faces[f];
            for j in 0..face.len() {
                let e = edge(
                    g.canonical[face[j] as usize],
                    g.canonical[face[(j + 1) % face.len()] as usize],
                );
                if seams.contains(&e) {
                    continue;
                }
                let neighbors = &g.edge_faces[&e];
                if neighbors.len() != 2 {
                    continue;
                }
                for &next in neighbors {
                    if !assigned[next]
                        && (!seams.is_empty() || normals[seed].dot(normals[next]) > 0.35)
                    {
                        assigned[next] = true;
                        q.push_back(next);
                    }
                }
            }
        }
        charts.push(chart);
    }
    let mut result = Vec::<(usize, Vec<[f32; 2]>)>::new();
    for chart in charts {
        if cancel.load(Ordering::Relaxed) {
            return Err(error("Развёртка отменена"));
        }
        let members: BTreeSet<_> = chart.iter().copied().collect();
        let mut corners = Vec::new();
        let mut offsets = BTreeMap::new();
        for &f in &chart {
            offsets.insert(f, corners.len());
            corners.extend(faces[f].iter().map(|&v| v as usize));
        }
        let mut parent: Vec<_> = (0..corners.len()).collect();
        for (&e, adj) in &g.edge_faces {
            if adj.len() != 2 || seams.contains(&e) || !adj.iter().all(|f| members.contains(f)) {
                continue;
            }
            for endpoint in [e.0, e.1] {
                let find = |f| {
                    offsets[&f]
                        + faces[f]
                            .iter()
                            .position(|&v| g.canonical[v as usize] == endpoint)
                            .unwrap()
                };
                join(&mut parent, find(adj[0]), find(adj[1]));
            }
        }
        let mut remap = BTreeMap::new();
        let mut positions = Vec::new();
        let mut ids = Vec::new();
        for (i, &v) in corners.iter().enumerate() {
            let r = root(&mut parent, i);
            let id = *remap.entry(r).or_insert_with(|| {
                positions.push(Vec3::from_array(mesh.vertices[v].position));
                positions.len() - 1
            });
            ids.push(id);
        }
        let mut triangles = Vec::new();
        let mut edge_counts = BTreeMap::<(usize, usize), usize>::new();
        for &f in &chart {
            let face = &faces[f];
            let off = offsets[&f];
            for j in 0..face.len() {
                *edge_counts
                    .entry(edge(ids[off + j], ids[off + (j + 1) % face.len()]))
                    .or_default() += 1;
            }
            for tri in crate::triangulate::triangulate(mesh, face)?
                .as_chunks::<3>()
                .0
            {
                triangles.push(
                    [tri[0], tri[1], tri[2]]
                        .map(|v| ids[off + face.iter().position(|&i| i == v).unwrap()]),
                );
            }
        }
        let boundary: BTreeSet<_> = edge_counts
            .iter()
            .filter(|(_, n)| **n == 1)
            .flat_map(|(&(a, b), _)| [a, b])
            .collect();
        if boundary.len() < 2 {
            return Err(error("Замкнутый UV-остров: добавьте шов перед развёрткой"));
        }
        let uv = conformal(&positions, &triangles, &boundary, cancel)?;
        for &f in &chart {
            let off = offsets[&f];
            result.push((
                f,
                (0..faces[f].len())
                    .map(|i| uv[ids[off + i]].to_array())
                    .collect(),
            ));
        }
    }
    assign_corners(mesh, &faces, &result)?;
    pack(mesh, 0.02, cancel)
}
fn conformal(
    p: &[Vec3],
    triangles: &[[usize; 3]],
    boundary: &BTreeSet<usize>,
    cancel: &AtomicBool,
) -> Result<Vec<Vec2>, CoreError> {
    let farthest = |from: usize| {
        *boundary
            .iter()
            .max_by(|&&a, &&b| {
                p[a].distance_squared(p[from])
                    .total_cmp(&p[b].distance_squared(p[from]))
            })
            .unwrap()
    };
    let a = farthest(*boundary.first().unwrap());
    let b = farthest(a);
    let length = p[a].distance(p[b]) as f64;
    if length < 1e-9 {
        return Err(error("Вырожденный UV-остров"));
    }
    let pins = BTreeMap::from([
        (a * 2, 0.),
        (a * 2 + 1, 0.),
        (b * 2, length),
        (b * 2 + 1, 0.),
    ]);
    let mut vars = vec![None; p.len() * 2];
    let mut count = 0;
    for (i, v) in vars.iter_mut().enumerate() {
        if !pins.contains_key(&i) {
            *v = Some(count);
            count += 1;
        }
    }
    let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
    let mut constants = Vec::new();
    for &[i, j, k] in triangles {
        let e = p[j] - p[i];
        let l = e.length();
        let c = p[k] - p[i];
        if l < 1e-10 {
            continue;
        }
        let x = c.dot(e / l);
        let y = (c.length_squared() - x * x).max(0.).sqrt();
        if y < 1e-10 {
            continue;
        }
        let weight = 1. / ((l * y) as f64).sqrt();
        let aa = [-y, y, 0.];
        let bb = [x - l, -x, l];
        for imaginary in [false, true] {
            let mut row = Vec::new();
            let mut constant = 0.;
            for (n, v) in [i, j, k].into_iter().enumerate() {
                for axis in 0..2 {
                    let coeff = match (imaginary, axis) {
                        (false, 0) => aa[n],
                        (false, _) => -bb[n],
                        (true, 0) => bb[n],
                        (true, _) => aa[n],
                    } as f64
                        * weight;
                    let index = v * 2 + axis;
                    if let Some(free) = vars[index] {
                        row.push((free, coeff));
                    } else {
                        constant += coeff * pins[&index];
                    }
                }
            }
            rows.push(row);
            constants.push(constant);
        }
    }
    let mut rhs = vec![0.; count];
    let mut diagonal = vec![1e-12; count];
    for (row, c) in rows.iter().zip(constants) {
        for &(i, a) in row {
            rhs[i] -= a * c;
            diagonal[i] += a * a;
        }
    }
    let multiply = |x: &[f64]| {
        let mut out: Vec<_> = x.iter().map(|v| v * 1e-12).collect();
        for row in &rows {
            let d: f64 = row.iter().map(|&(i, a)| a * x[i]).sum();
            for &(i, a) in row {
                out[i] += a * d;
            }
        }
        out
    };
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let mut x = vec![0.; count];
    let mut residual = rhs;
    let mut z: Vec<_> = residual.iter().zip(&diagonal).map(|(r, d)| r / d).collect();
    let mut direction = z.clone();
    let mut rz = dot(&residual, &z);
    let initial = rz.max(1e-30);
    for _ in 0..800 {
        if cancel.load(Ordering::Relaxed) {
            return Err(error("Развёртка отменена"));
        }
        if rz <= initial * 1e-12 {
            break;
        }
        let ad = multiply(&direction);
        let denominator = dot(&direction, &ad);
        if denominator <= 1e-30 {
            break;
        }
        let alpha = rz / denominator;
        for i in 0..count {
            x[i] += alpha * direction[i];
            residual[i] -= alpha * ad[i];
            z[i] = residual[i] / diagonal[i];
        }
        let next = dot(&residual, &z);
        let beta = next / rz;
        for i in 0..count {
            direction[i] = z[i] + beta * direction[i];
        }
        rz = next;
    }
    if !rz.is_finite() || rz > initial * 1e-8 {
        return Err(error(
            "UV-остров слишком сложный: добавьте швы и повторите развёртку",
        ));
    }
    let result: Vec<_> = (0..p.len())
        .map(|i| {
            let coord = |axis: usize| {
                vars[i * 2 + axis].map_or_else(|| pins[&(i * 2 + axis)], |j| x[j]) as f32
            };
            Vec2::new(coord(0), coord(1))
        })
        .collect();
    if result.iter().any(|v| !v.is_finite()) {
        return Err(error("Не удалось построить конечные UV"));
    }
    Ok(result)
}
fn assign_corners(
    mesh: &mut Mesh,
    faces: &[Vec<u32>],
    coords: &[(usize, Vec<[f32; 2]>)],
) -> Result<(), CoreError> {
    topology(mesh);
    let mut out = faces.to_vec();
    let mut used = BTreeMap::<(u32, [i64; 2]), u32>::new();
    let mut assigned = BTreeSet::new();
    for (f, uvs) in coords {
        for (corner, &uv) in uvs.iter().enumerate() {
            let original = faces[*f][corner];
            let key = (original, uv_key(uv));
            let id = if let Some(&id) = used.get(&key) {
                id
            } else {
                let id = if assigned.insert(original) {
                    original
                } else {
                    if mesh.vertices.len() >= MAX_EDIT_VERTICES {
                        return Err(error("UV-швы превысят предел 65536 вершин"));
                    }
                    let v = mesh.vertices[original as usize];
                    mesh.vertices.push(v);
                    (mesh.vertices.len() - 1) as u32
                };
                mesh.vertices[id as usize].uv = uv;
                used.insert(key, id);
                id
            };
            out[*f][corner] = id;
        }
    }
    mesh.topology.as_mut().unwrap().faces = out.into();
    mesh.rebuild_polygon_indices()
}
fn pack(mesh: &mut Mesh, margin: f32, cancel: &AtomicBool) -> Result<(), CoreError> {
    if !margin.is_finite() || !(0.001..=0.2).contains(&margin) {
        return Err(error("Отступ островов: 0,001–0,2"));
    }
    let islands = uv_islands(mesh);
    if islands.len() > 4096 {
        return Err(error(
            "Слишком много UV-островов: объедините швы перед упаковкой",
        ));
    }
    let faces: Vec<_> = (0..mesh.face_count())
        .map(|f| mesh.face_vertices(f).unwrap().to_vec())
        .collect();
    let mut bounds: Vec<_> = islands
        .iter()
        .enumerate()
        .map(|(i, fs)| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for &f in fs {
                for &v in &faces[f] {
                    let p = Vec2::from_array(mesh.vertices[v as usize].uv);
                    min = min.min(p);
                    max = max.max(p);
                }
            }
            (i, min, (max - min).max(Vec2::splat(1e-6)))
        })
        .collect();
    bounds.sort_by(|a, b| b.2.y.total_cmp(&a.2.y).then(a.0.cmp(&b.0)));
    let layout = |scale: f32| -> Option<Vec<Vec2>> {
        let mut positions = vec![Vec2::ZERO; islands.len()];
        let (mut x, mut y, mut row) = (margin, margin, 0_f32);
        for &(i, _, size) in &bounds {
            let s = size * scale;
            if s.x > 1. - 2. * margin || s.y > 1. - 2. * margin {
                return None;
            }
            if x + s.x > 1. - margin {
                x = margin;
                y += row + margin;
                row = 0.;
            }
            if y + s.y > 1. - margin {
                return None;
            }
            positions[i] = Vec2::new(x, y);
            x += s.x + margin;
            row = row.max(s.y);
        }
        Some(positions)
    };
    let mut low = 0.;
    let mut high = 1.
        / bounds
            .iter()
            .map(|b| b.2.max_element())
            .fold(1e-6, f32::max);
    for _ in 0..40 {
        if cancel.load(Ordering::Relaxed) {
            return Err(error("Упаковка отменена"));
        }
        let m = (low + high) * 0.5;
        if layout(m).is_some() {
            low = m;
        } else {
            high = m;
        }
    }
    if low < 1e-10 {
        return Err(error("Острова не помещаются с выбранным отступом"));
    }
    let positions = layout(low).unwrap();
    let mut coords = Vec::new();
    for (i, min, _) in bounds {
        for &f in &islands[i] {
            coords.push((
                f,
                faces[f]
                    .iter()
                    .map(|&v| {
                        ((Vec2::from_array(mesh.vertices[v as usize].uv) - min) * low
                            + positions[i])
                            .to_array()
                    })
                    .collect(),
            ));
        }
    }
    assign_corners(mesh, &faces, &coords)
}
/// Scale-independent triangle anisotropy: 1 is conformal, larger values stretch.
pub fn uv_stretch(mesh: &Mesh, face: usize) -> f32 {
    let Some(f) = mesh.face_vertices(face) else {
        return 1.;
    };
    let Ok(triangles) = crate::triangulate::triangulate(mesh, f) else {
        return 1000.;
    };
    let mut worst = 1_f32;
    for t in triangles.as_chunks::<3>().0 {
        let v = t
            .iter()
            .map(|&i| mesh.vertices[i as usize])
            .collect::<Vec<_>>();
        let a = Vec3::from_array(v[1].position) - Vec3::from_array(v[0].position);
        let b = Vec3::from_array(v[2].position) - Vec3::from_array(v[0].position);
        let l = a.length();
        if l < 1e-10 {
            continue;
        }
        let x = b.dot(a / l);
        let y = (b.length_squared() - x * x).max(0.).sqrt();
        if y < 1e-10 {
            continue;
        }
        let du = Vec2::from_array(v[1].uv) - Vec2::from_array(v[0].uv);
        let dv = Vec2::from_array(v[2].uv) - Vec2::from_array(v[0].uv);
        let c0 = du / l;
        let c1 = (dv - du * x / l) / y;
        let aa = c0.length_squared();
        let bb = c0.dot(c1);
        let cc = c1.length_squared();
        let disc = ((aa - cc).powi(2) + 4. * bb * bb).sqrt();
        let hi = (aa + cc + disc) * 0.5;
        let lo = (aa + cc - disc) * 0.5;
        if hi <= 1e-20 {
            return 1000.;
        }
        worst = worst.max((hi / lo.max(1e-12)).sqrt().min(1000.));
    }
    worst
}
