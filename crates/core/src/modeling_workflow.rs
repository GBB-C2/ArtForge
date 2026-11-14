//! Component-aware construction. Every operation runs inside Editor's atomic edit.
use crate::mesh_topology_tools::{faces, invalid};
use crate::{
    CoreError, KnifeCut, Mesh, MeshConnectivity, MeshRegion, Quat, Vec3, MAX_EDIT_TRIANGLES,
    MAX_EDIT_VERTICES,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}
fn finish(mesh: &mut Mesh) -> Result<(), CoreError> {
    let t = mesh.topology.as_mut().unwrap();
    t.primitive = None;
    mesh.rebuild_polygon_indices()?;
    if mesh.vertices.len() > MAX_EDIT_VERTICES || mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES {
        return Err(invalid("Превышен предел редактируемой сетки"));
    }
    crate::recompute_normals(mesh, mesh.smooth_normals_enabled());
    mesh.validate_topology()
}
pub(crate) fn dissolve(mesh: &mut Mesh, region: &MeshRegion) -> Result<(), CoreError> {
    crate::component_mesh::ensure_topology(mesh);
    let g = MeshConnectivity::new(mesh);
    if region.vertices.iter().any(|&v| v >= mesh.vertices.len())
        || region.faces.iter().any(|&f| f >= mesh.face_count())
    {
        return Err(invalid("Выберите существующие компоненты"));
    }
    let selected: BTreeSet<_> = region.vertices.iter().map(|&v| g.canonical[v]).collect();
    let positions: BTreeSet<_> = selected
        .iter()
        .map(|&v| crate::position_key(mesh.vertices[v].position))
        .collect();
    let mut edges = region.edges.clone();
    for (&e, fs) in &g.edge_faces {
        if fs.len() == 2
            && (selected.contains(&e.0)
                || selected.contains(&e.1)
                || fs.iter().all(|f| region.faces.contains(f)))
        {
            edges.insert(e);
        }
    }
    if !edges.is_empty() {
        crate::mesh_topology_tools::dissolve(mesh, &edges)?;
    }
    if !positions.is_empty() {
        let mut changed = false;
        let output: Vec<_> = faces(mesh)
            .into_iter()
            .map(|f| {
                let reduced: Vec<_> = f
                    .iter()
                    .copied()
                    .filter(|&v| {
                        !positions
                            .contains(&crate::position_key(mesh.vertices[v as usize].position))
                    })
                    .collect();
                if reduced.len() >= 3 && reduced.len() != f.len() {
                    changed = true;
                    reduced
                } else {
                    f
                }
            })
            .collect();
        if changed {
            mesh.topology.as_mut().unwrap().faces = output.into();
            finish(mesh)?;
        } else if edges.is_empty() {
            return Err(invalid(
                "Растворение должно оставить не менее трёх углов грани",
            ));
        }
    } else if edges.is_empty() {
        return Err(invalid(
            "Выберите соседние грани, внутренние рёбра или вершины",
        ));
    }
    // Remove unused surface vertices without changing surviving corner attributes.
    let output = faces(mesh);
    crate::mesh_topology_tools::finish(mesh, output, mesh.smooth_normals_enabled())
}
pub(crate) fn extrude(
    mesh: &mut Mesh,
    selection: &BTreeSet<usize>,
    distance: f32,
) -> Result<(), CoreError> {
    if selection.is_empty()
        || selection.iter().any(|&f| f >= mesh.face_count())
        || !distance.is_finite()
        || distance.abs() > 1000.
    {
        return Err(invalid("Выберите грани; расстояние от −1000 до 1000 м"));
    }
    if distance == 0. {
        return Ok(());
    }
    crate::component_mesh::ensure_topology(mesh);
    let source = faces(mesh);
    let g = MeshConnectivity::new(mesh);
    let mut normals = BTreeMap::<usize, Vec3>::new();
    let mut used = BTreeSet::new();
    for &f in selection {
        let n = crate::polygon_edit::face_normal(mesh, &source[f]);
        for &v in &source[f] {
            used.insert(v);
            *normals.entry(g.canonical[v as usize]).or_default() += n;
        }
    }
    if mesh.vertices.len() + used.len() > MAX_EDIT_VERTICES {
        return Err(invalid("Слишком много вершин для экструзии"));
    }
    let mut cap = BTreeMap::new();
    for v in used {
        let mut p = mesh.vertices[v as usize];
        let n = normals[&g.canonical[v as usize]]
            .try_normalize()
            .ok_or_else(|| invalid("Не удалось определить нормаль области"))?;
        p.position = (Vec3::from_array(p.position) + n * distance).to_array();
        cap.insert(v, mesh.vertices.len() as u32);
        mesh.vertices.push(p);
    }
    let mut walls = Vec::new();
    let mut materials = Vec::new();
    for &f in selection {
        let ring = &source[f];
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            if g.edge_faces[&edge(g.canonical[a as usize], g.canonical[b as usize])]
                .iter()
                .filter(|f| selection.contains(f))
                .count()
                == 1
            {
                walls.push(vec![a, b, cap[&b], cap[&a]]);
                materials.push(mesh.face_material(f));
            }
        }
    }
    let t = mesh.topology.as_mut().unwrap();
    for &f in selection {
        t.faces[f] = source[f].iter().map(|v| cap[v]).collect();
    }
    t.faces.extend(walls);
    if !t.face_materials.is_empty() {
        t.face_materials.extend(materials);
    }
    let output = faces(mesh);
    crate::mesh_topology_tools::finish(mesh, output, mesh.smooth_normals_enabled())
}
pub(crate) fn array(
    mesh: &mut Mesh,
    region: &MeshRegion,
    count: u32,
    offset: Vec3,
    rotation: Vec3,
) -> Result<(), CoreError> {
    if !(2..=128).contains(&count)
        || !offset.is_finite()
        || !rotation.is_finite()
        || offset.abs().max_element() > 10000.
        || rotation.abs().max_element() > 3600.
    {
        return Err(invalid("Массив: 2–128 копий, конечные смещение и поворот"));
    }
    crate::component_mesh::ensure_topology(mesh);
    let source = mesh.clone();
    if region.faces.iter().any(|&f| f >= mesh.face_count()) {
        return Err(invalid("Грань массива не существует"));
    }
    let mut used = region.vertices.clone();
    used.extend(region.edges.iter().flat_map(|&(a, b)| [a, b]));
    for &f in &region.faces {
        used.extend(source.face_vertices(f).unwrap().iter().map(|&v| v as usize));
    }
    if used.is_empty() || used.iter().any(|&v| v >= mesh.vertices.len()) {
        return Err(invalid("Выберите вершины, рёбра или грани массива"));
    }
    if mesh.vertices.len() + used.len() * (count as usize - 1) > MAX_EDIT_VERTICES {
        return Err(invalid("Массив превышает предел вершин"));
    }
    let center = used
        .iter()
        .map(|&v| Vec3::from_array(source.vertices[v].position))
        .sum::<Vec3>()
        / used.len() as f32;
    let g = MeshConnectivity::new(&source);
    let representatives: BTreeMap<_, _> = used.iter().map(|&v| (g.canonical[v], v)).collect();
    let mut copied_edges = region.edges.clone();
    if !region.vertices.is_empty() {
        let selected: BTreeSet<_> = region.vertices.iter().map(|&v| g.canonical[v]).collect();
        for &(a, b) in &g.edges {
            if selected.contains(&a) && selected.contains(&b) {
                copied_edges.insert((representatives[&a], representatives[&b]));
            }
        }
    }
    if copied_edges
        .iter()
        .any(|&(a, b)| !g.edges.contains(&edge(g.canonical[a], g.canonical[b])))
    {
        return Err(invalid("Ребро массива не существует"));
    }
    for copy in 1..count {
        let n = copy as f32;
        let r = rotation.map(f32::to_radians) * n;
        let q = Quat::from_euler(glam::EulerRot::XYZ, r.x, r.y, r.z);
        let mut map = BTreeMap::new();
        for &v in &used {
            let mut p = source.vertices[v];
            p.position =
                (center + q * (Vec3::from_array(p.position) - center) + offset * n).to_array();
            p.normal = (q * Vec3::from_array(p.normal)).to_array();
            map.insert(v, mesh.vertices.len() as u32);
            mesh.vertices.push(p);
        }
        let t = mesh.topology.as_mut().unwrap();
        for &f in &region.faces {
            t.faces.push(
                source
                    .face_vertices(f)
                    .unwrap()
                    .iter()
                    .map(|&v| map[&(v as usize)])
                    .collect(),
            );
            if !t.face_materials.is_empty() {
                t.face_materials.push(source.face_material(f));
            }
        }
        for &(a, b) in &copied_edges {
            if !g
                .edge_faces
                .get(&edge(g.canonical[a], g.canonical[b]))
                .is_some_and(|fs| fs.iter().any(|f| region.faces.contains(f)))
            {
                t.loose_edges.push([map[&a], map[&b]]);
            }
        }
        for e in &source.topology.as_ref().unwrap().uv_seams {
            if let (Some(&a), Some(&b)) = (map.get(&(e[0] as usize)), map.get(&(e[1] as usize))) {
                t.uv_seams.push([a, b]);
            }
        }
    }
    // Array is a rigid copy: authored normals on the source and copies stay intact.
    let normals: Vec<_> = mesh.vertices.iter().map(|v| v.normal).collect();
    finish(mesh)?;
    for (v, n) in mesh.vertices.iter_mut().zip(normals) {
        v.normal = n;
    }
    Ok(())
}
/// Positions for viewport preview and for the committed cut use the same traversal.
pub fn loop_cut_path(
    mesh: &Mesh,
    seed: (usize, usize),
    position: f32,
) -> Result<Vec<KnifeCut>, CoreError> {
    if seed.0 >= mesh.vertices.len()
        || seed.1 >= mesh.vertices.len()
        || !position.is_finite()
        || !(0.01..=0.99).contains(&position)
    {
        return Err(invalid("Положение петли: 1–99%"));
    }
    let g = MeshConnectivity::new(mesh);
    let seed = edge(g.canonical[seed.0], g.canonical[seed.1]);
    if !g.edge_faces.contains_key(&seed) {
        return Err(invalid("Наведите указатель на ребро поверхности"));
    }
    let first = *g.edge_faces[&seed]
        .first()
        .ok_or_else(|| invalid("Наведите указатель на ребро поверхности"))?;
    let component = g.connected_faces(first);
    if g.edge_faces
        .values()
        .any(|fs| fs.len() > 2 && fs.iter().any(|f| component.contains(f)))
    {
        return Err(invalid("У петли есть неманифолдное ребро"));
    }
    if component
        .iter()
        .any(|&f| mesh.face_vertices(f).unwrap().len() != 4)
    {
        return plane_loop_path(mesh, &component, seed, position);
    }
    let mut cuts = BTreeMap::from([(seed, position)]);
    let mut queue = VecDeque::from([seed]);
    let mut seen = BTreeMap::new();
    let mut result = Vec::new();
    while let Some(e) = queue.pop_front() {
        if g.edge_faces[&e].len() > 2 {
            return Err(invalid("У петли есть неманифолдное ребро"));
        }
        for &f in &g.edge_faces[&e] {
            let face = mesh.face_vertices(f).unwrap();
            if face.len() != 4 {
                continue;
            }
            let j = (0..4)
                .find(|&j| {
                    edge(
                        g.canonical[face[j] as usize],
                        g.canonical[face[(j + 1) % 4] as usize],
                    ) == e
                })
                .unwrap();
            if let Some(&old) = seen.get(&f) {
                if j != old && j != (old + 2) % 4 {
                    return Err(invalid("Петля пересекает себя"));
                }
                continue;
            }
            seen.insert(f, j);
            let ids: [usize; 4] = std::array::from_fn(|i| g.canonical[face[(j + i) % 4] as usize]);
            let t = if ids[0] < ids[1] {
                cuts[&e]
            } else {
                1. - cuts[&e]
            };
            let opposite = edge(ids[2], ids[3]);
            let next = if ids[3] < ids[2] { t } else { 1. - t };
            if let Some(&v) = cuts.get(&opposite) {
                if (v - next).abs() > 1e-4 {
                    return Err(invalid("Петля меняет ориентацию"));
                }
            } else {
                cuts.insert(opposite, next);
                queue.push_back(opposite);
            }
            let p = |i: usize| Vec3::from_array(mesh.vertices[ids[i]].position);
            result.push(KnifeCut {
                face: f,
                first: p(0).lerp(p(1), t).to_array(),
                second: p(3).lerp(p(2), t).to_array(),
            });
        }
    }
    if result.is_empty() {
        return Err(invalid("Для петли нужна полоса четырёхугольников"));
    }
    Ok(result)
}
// At poles and n-gons there is no unique opposite edge. Continue the visible
// section through the connected surface instead of silently dropping the cut.
fn plane_loop_path(
    mesh: &Mesh,
    faces: &BTreeSet<usize>,
    seed: (usize, usize),
    t: f32,
) -> Result<Vec<KnifeCut>, CoreError> {
    let a = Vec3::from_array(mesh.vertices[seed.0].position);
    let b = Vec3::from_array(mesh.vertices[seed.1].position);
    let origin = a.lerp(b, t);
    let n = (b - a).normalize();
    let mut result = Vec::new();
    for &f in faces {
        let ring = mesh.face_vertices(f).unwrap();
        let start = result.len();
        for triangle in crate::triangulate::triangulate(mesh, ring)?.chunks_exact(3) {
            let mut points = Vec::new();
            for i in 0..3 {
                let p = Vec3::from_array(mesh.vertices[triangle[i] as usize].position);
                let q = Vec3::from_array(mesh.vertices[triangle[(i + 1) % 3] as usize].position);
                let x = (p - origin).dot(n);
                let y = (q - origin).dot(n);
                if x.abs() < 1e-6 {
                    points.push(p);
                }
                if x * y < -1e-12 {
                    points.push(if p.to_array() < q.to_array() {
                        p.lerp(q, x / (x - y))
                    } else {
                        q.lerp(p, y / (y - x))
                    });
                }
            }
            let mut unique = Vec::<Vec3>::new();
            for p in points {
                if unique.iter().all(|q| q.distance_squared(p) > 1e-12) {
                    unique.push(p);
                }
            }
            if unique.len() == 2 {
                result.push(KnifeCut {
                    face: f,
                    first: unique[0].to_array(),
                    second: unique[1].to_array(),
                });
            }
        }
        // Triangulation diagonals are not surface boundaries. Join their
        // intervals before applying the knife to a planar n-gon so the same
        // interior endpoint is not repeatedly inserted into adjacent triangles.
        let face_normal = crate::polygon_edit::face_normal(mesh, ring).normalize_or_zero();
        let face_origin = Vec3::from_array(mesh.vertices[ring[0] as usize].position);
        if result.len() > start + 1
            && ring.iter().all(|&v| {
                (Vec3::from_array(mesh.vertices[v as usize].position) - face_origin)
                    .dot(face_normal)
                    .abs()
                    < 1e-6
            })
        {
            let direction = n.cross(face_normal).normalize_or_zero();
            let mut intervals: Vec<_> = result
                .drain(start..)
                .map(|cut| {
                    let a = Vec3::from_array(cut.first);
                    let b = Vec3::from_array(cut.second);
                    let ta = a.dot(direction);
                    let tb = b.dot(direction);
                    if ta < tb {
                        (ta, tb, a, b)
                    } else {
                        (tb, ta, b, a)
                    }
                })
                .collect();
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut joined: Vec<(f32, f32, Vec3, Vec3)> = Vec::new();
            for interval in intervals {
                if let Some(last) = joined.last_mut().filter(|last| interval.0 <= last.1 + 1e-6) {
                    if interval.1 > last.1 {
                        last.1 = interval.1;
                        last.3 = interval.3;
                    }
                } else {
                    joined.push(interval);
                }
            }
            result.extend(joined.into_iter().map(|(_, _, a, b)| KnifeCut {
                face: f,
                first: a.to_array(),
                second: b.to_array(),
            }));
        }
    }
    if result.is_empty() {
        return Err(invalid("Плоскость петли не пересекает поверхность"));
    }
    Ok(result)
}
pub(crate) fn bridge_open(
    mesh: &mut Mesh,
    edges: &BTreeSet<(usize, usize)>,
    segments: u32,
    twist: i32,
    smooth: f32,
) -> Result<(), CoreError> {
    if !(1..=32).contains(&segments)
        || !smooth.is_finite()
        || !(0. ..=1.).contains(&smooth)
        || twist.unsigned_abs() > 4096
    {
        return Err(invalid("Недопустимые параметры моста"));
    }
    let g = MeshConnectivity::new(mesh);
    let mut adj = BTreeMap::<usize, Vec<usize>>::new();
    for &(a, b) in edges {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    if adj.values().any(|v| v.len() > 2) {
        return Err(invalid("Выберите цепочки без развилок"));
    }
    let mut remaining = edges.clone();
    let mut chains = Vec::new();
    while !remaining.is_empty() {
        let start = adj
            .iter()
            .find(|(v, ns)| ns.len() == 1 && remaining.contains(&edge(**v, ns[0])))
            .map(|(&v, _)| v)
            .ok_or_else(|| invalid("Не смешивайте замкнутые контуры и открытые цепочки"))?;
        let mut chain = vec![start];
        let mut v = start;
        loop {
            let Some(&n) = adj[&v].iter().find(|&&n| remaining.contains(&edge(v, n))) else {
                break;
            };
            remaining.remove(&edge(v, n));
            chain.push(n);
            v = n;
        }
        chains.push(chain);
    }
    if chains.len() < 2 {
        return Err(invalid(
            "Выберите две отдельные цепочки или два отдельных ребра",
        ));
    }
    if chains.len() > 2
        && chains[1..chains.len() - 1].iter().any(|c| {
            c.windows(2)
                .any(|e| !g.edge_faces[&edge(e[0], e[1])].is_empty())
        })
    {
        return Err(invalid(
            "Соединяйте пары граничных цепочек; свободные рёбра можно соединять рядом",
        ));
    }
    crate::component_mesh::ensure_topology(mesh);
    let mut output = faces(mesh);
    let mut materials: Vec<_> = (0..mesh.face_count())
        .map(|f| mesh.face_material(f))
        .collect();
    let direction = |c: &[usize]| -> Option<bool> {
        let fs = &g.edge_faces[&edge(c[0], c[1])];
        let f = *fs.first()?;
        let ring = mesh.face_vertices(f)?;
        Some((0..ring.len()).any(|i| {
            g.canonical[ring[i] as usize] == c[0]
                && g.canonical[ring[(i + 1) % ring.len()] as usize] == c[1]
        }))
    };
    for i in 0..chains.len() {
        if let Some(forward) = direction(&chains[i]) {
            if (i == 0 && forward) || (i > 0 && !forward) {
                chains[i].reverse();
            }
        }
    }
    for k in 0..chains.len() - 1 {
        let first = chains[k].clone();
        let mut last = chains[k + 1].clone();
        let p = |v: usize| Vec3::from_array(mesh.vertices[v].position);
        if g.edge_faces[&edge(last[0], last[1])].is_empty() {
            let direct = p(first[0]).distance_squared(p(last[0]))
                + p(*first.last().unwrap()).distance_squared(p(*last.last().unwrap()));
            let reverse = p(first[0]).distance_squared(p(*last.last().unwrap()))
                + p(*first.last().unwrap()).distance_squared(p(last[0]));
            if (reverse < direct) ^ (twist.rem_euclid(2) != 0) {
                last.reverse();
            }
        }
        chains[k + 1] = last.clone();
        let material = g.edge_faces[&edge(first[0], first[1])]
            .first()
            .and_then(|&f| mesh.face_material(f));
        let count = first.len().max(last.len());
        if mesh.vertices.len() + (segments as usize - 1) * count > MAX_EDIT_VERTICES {
            return Err(invalid("Слишком много сегментов моста"));
        }
        let mut rows = vec![first.iter().map(|&v| v as u32).collect::<Vec<_>>()];
        for s in 1..segments {
            let t = s as f32 / segments as f32;
            let mut row = Vec::new();
            for j in 0..count {
                let sample = |c: &[usize]| {
                    let a = j as f32 / (count - 1) as f32 * (c.len() - 1) as f32;
                    let i = (a.floor() as usize).min(c.len() - 2);
                    crate::component_mesh::interpolate(
                        mesh.vertices[c[i]],
                        mesh.vertices[c[i + 1]],
                        a - i as f32,
                    )
                };
                let (a, b) = (sample(&first), sample(&last));
                let mut v = crate::component_mesh::interpolate(a, b, t);
                let delta = Vec3::from_array(b.position) - Vec3::from_array(a.position);
                let n =
                    (Vec3::from_array(a.normal) + Vec3::from_array(b.normal)).normalize_or_zero();
                v.position = (Vec3::from_array(v.position)
                    + n * delta.length() * smooth * t * (1. - t))
                    .to_array();
                row.push(mesh.vertices.len() as u32);
                mesh.vertices.push(v);
            }
            rows.push(row);
        }
        rows.push(last.iter().map(|&v| v as u32).collect());
        for pair in rows.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let (mut i, mut j) = (0, 0);
            while i + 1 < a.len() || j + 1 < b.len() {
                let x = if i + 1 < a.len() {
                    (i + 1) as f64 / (a.len() - 1) as f64
                } else {
                    2.
                };
                let y = if j + 1 < b.len() {
                    (j + 1) as f64 / (b.len() - 1) as f64
                } else {
                    2.
                };
                let f = if (x - y).abs() < 1e-8 {
                    let f = vec![a[i], a[i + 1], b[j + 1], b[j]];
                    i += 1;
                    j += 1;
                    f
                } else if x < y {
                    let f = vec![a[i], a[i + 1], b[j]];
                    i += 1;
                    f
                } else {
                    let f = vec![a[i], b[j + 1], b[j]];
                    j += 1;
                    f
                };
                output.push(f);
                materials.push(material);
            }
        }
    }
    let t = mesh.topology.as_mut().unwrap();
    t.faces = output.into();
    t.face_materials = materials.into();
    t.loose_edges.retain(|e| {
        !edges.contains(&edge(
            g.canonical[e[0] as usize],
            g.canonical[e[1] as usize],
        ))
    });
    finish(mesh)
}
