//! Surface-constrained slides, connected falloff and polygon knife cuts.
use crate::{CoreError, Mat4, Mesh, MeshConnectivity, MeshTopology, Vec3, MAX_EDIT_VERTICES};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
#[derive(Clone, Copy, Debug)]
pub struct EdgePoint {
    pub edge: (usize, usize),
    pub t: f32,
}
fn error(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}
fn detach(mesh: &mut Mesh) {
    if let Some(t) = &mut mesh.topology {
        t.primitive = None;
    }
}
/// World-space geodesic falloff, computed once per gesture. Disconnected shells
/// cannot pull each other simply because they are close in screen space.
pub fn proportional_weights(
    mesh: &Mesh,
    selected: &BTreeSet<usize>,
    radius: f32,
    model: Mat4,
) -> Vec<f32> {
    let g = MeshConnectivity::new(mesh);
    let mut dist = vec![f32::INFINITY; mesh.vertices.len()];
    let mut queue = BinaryHeap::new();
    for &v in selected {
        if let Some(&c) = g.canonical.get(v) {
            dist[c] = 0.;
            queue.push((std::cmp::Reverse(0_u32), c));
        }
    }
    if radius.is_finite() && radius > 0. && model.is_finite() {
        while let Some((std::cmp::Reverse(bits), v)) = queue.pop() {
            let d = f32::from_bits(bits);
            if d > dist[v] || d >= radius {
                continue;
            }
            let p = model.transform_point3(Vec3::from_array(mesh.vertices[v].position));
            for &n in &g.vertex_neighbors[v] {
                let next = d + p
                    .distance(model.transform_point3(Vec3::from_array(mesh.vertices[n].position)));
                if next < dist[n] && next < radius {
                    dist[n] = next;
                    queue.push((std::cmp::Reverse(next.to_bits()), n));
                }
            }
        }
    }
    g.canonical
        .iter()
        .map(|&c| {
            if dist[c] == 0. {
                1.
            } else if radius > 0. && dist[c] < radius {
                let t = 1. - dist[c] / radius;
                t * t * (3. - 2. * t)
            } else {
                0.
            }
        })
        .collect()
}
pub(crate) fn weighted_transform(
    mesh: &mut Mesh,
    weights: &[f32],
    matrix: Mat4,
) -> Result<(), CoreError> {
    if weights.len() != mesh.vertices.len()
        || weights
            .iter()
            .any(|w| !w.is_finite() || !(0. ..=1.).contains(w))
        || !matrix.is_finite()
        || matrix.determinant() <= 1e-12
        || matrix.x_axis.w != 0.
        || matrix.y_axis.w != 0.
        || matrix.z_axis.w != 0.
        || matrix.w_axis.w != 1.
    {
        return Err(error("Некорректная пропорциональная трансформация"));
    }
    if matrix == Mat4::IDENTITY || weights.iter().all(|&w| w == 0.) {
        return Ok(());
    }
    for (v, &weight) in mesh.vertices.iter_mut().zip(weights) {
        if weight > 0. {
            let p = Vec3::from_array(v.position);
            let result = p.lerp(matrix.transform_point3(p), weight);
            if !result.is_finite() || result.abs().max_element() > 1e8 {
                return Err(error("Трансформация выходит за допустимые координаты"));
            }
            v.position = result.to_array();
        }
    }
    crate::recompute_normals(mesh, mesh.smooth_normals_enabled());
    detach(mesh);
    Ok(())
}
pub(crate) fn slide(
    mesh: &mut Mesh,
    vertices: &BTreeSet<usize>,
    edges: &BTreeSet<(usize, usize)>,
    amount: f32,
) -> Result<(), CoreError> {
    if !amount.is_finite() || !(-0.999..=0.999).contains(&amount) {
        return Err(error("Сдвиг вдоль поверхности: от −99,9 до 99,9%"));
    }
    let g = MeshConnectivity::new(mesh);
    let mut selected = BTreeSet::new();
    for &i in vertices {
        selected.insert(
            *g.canonical
                .get(i)
                .ok_or_else(|| error("Недопустимая вершина"))?,
        );
    }
    for &(a, b) in edges {
        if !g.edges.contains(&edge(a, b)) {
            return Err(error("Недопустимое ребро"));
        }
        selected.extend([a, b]);
    }
    if selected.is_empty() {
        return Err(error("Выберите вершины или рёбра для сдвига"));
    }
    if amount == 0. {
        return Ok(());
    }
    let mut rails = BTreeMap::<usize, [Option<usize>; 2]>::new();
    if !edges.is_empty() {
        let mut links = BTreeMap::<usize, Vec<usize>>::new();
        for &(a, b) in edges {
            links.entry(a).or_default().push(b);
            links.entry(b).or_default().push(a);
        }
        if links.values().any(|n| n.len() > 2) {
            return Err(error("Сдвиг рёбер требует цепь или петлю без ветвлений"));
        }
        let start = links
            .iter()
            .find(|(_, v)| v.len() == 1)
            .map_or(*links.keys().next().unwrap(), |(&k, _)| k);
        let mut current = start;
        let mut previous = None;
        let mut visited = BTreeSet::new();
        while let Some(&next) = links[&current]
            .iter()
            .find(|&&n| Some(n) != previous && !visited.contains(&edge(current, n)))
        {
            visited.insert(edge(current, next));
            for &f in &g.edge_faces[&edge(current, next)] {
                let face = mesh.face_vertices(f).unwrap();
                let canonical: Vec<_> = face.iter().map(|&v| g.canonical[v as usize]).collect();
                for (vertex, other) in [(current, next), (next, current)] {
                    let i = canonical.iter().position(|&v| v == vertex).unwrap();
                    let before = canonical[(i + canonical.len() - 1) % canonical.len()];
                    let after = canonical[(i + 1) % canonical.len()];
                    let forward = if vertex == current {
                        after == other
                    } else {
                        before == other
                    };
                    let candidate = if before == other { after } else { before };
                    if !selected.contains(&candidate) {
                        rails.entry(vertex).or_default()[usize::from(!forward)] = Some(candidate);
                    }
                }
            }
            previous = Some(current);
            current = next;
            if current == start {
                break;
            }
        }
        if visited.len() != edges.len() {
            return Err(error("Выберите одну связанную цепь рёбер для сдвига"));
        }
    } else {
        for &v in &selected {
            let candidates: Vec<_> = g.vertex_neighbors[v]
                .iter()
                .copied()
                .filter(|n| !selected.contains(n))
                .collect();
            let Some(&first) = candidates.first() else {
                continue;
            };
            let p = Vec3::from_array(mesh.vertices[v].position);
            let direction =
                (Vec3::from_array(mesh.vertices[first].position) - p).normalize_or_zero();
            let opposite = candidates
                .iter()
                .copied()
                .filter(|&n| n != first)
                .min_by(|&a, &b| {
                    let dot = |n: usize| {
                        direction.dot(
                            (Vec3::from_array(mesh.vertices[n].position) - p).normalize_or_zero(),
                        )
                    };
                    dot(a).total_cmp(&dot(b))
                });
            rails.insert(v, [Some(first), opposite]);
        }
    }
    if rails.is_empty() {
        return Err(error("У выделения нет свободных рёбер для сдвига"));
    }
    let original = mesh.vertices.clone();
    for (i, v) in mesh.vertices.iter_mut().enumerate() {
        if let Some(rail) = rails
            .get(&g.canonical[i])
            .and_then(|r| r[usize::from(amount < 0.)])
        {
            let p = Vec3::from_array(original[i].position);
            v.position = p
                .lerp(Vec3::from_array(original[rail].position), amount.abs())
                .to_array();
        }
    }
    crate::recompute_normals(mesh, mesh.smooth_normals_enabled());
    detach(mesh);
    Ok(())
}
pub(crate) fn knife(
    mesh: &mut Mesh,
    face: usize,
    first: EdgePoint,
    second: EdgePoint,
) -> Result<(), CoreError> {
    let g = MeshConnectivity::new(mesh);
    let cuts = [first, second];
    if face >= mesh.face_count()
        || first.edge == second.edge
        || cuts.iter().any(|c| {
            !g.edge_faces.get(&c.edge).is_some_and(|f| f.contains(&face))
                || !c.t.is_finite()
                || !(0.0..=1.0).contains(&c.t)
        })
    {
        return Err(error("Нож: выберите две разные стороны одной грани"));
    }
    let faces: Vec<_> = (0..mesh.face_count())
        .map(|f| mesh.face_vertices(f).unwrap().to_vec())
        .collect();
    let smooth = mesh.smooth_normals_enabled();
    let seams = crate::uv_seams(mesh);
    let mut added = BTreeMap::<(u32, u32), u32>::new();
    let mut output = Vec::new();
    let mut new_seams = Vec::new();
    let mut face_materials = Vec::new();
    let has_materials = mesh
        .topology
        .as_ref()
        .is_some_and(|t| !t.face_materials.is_empty());
    for (f, ring) in faces.iter().enumerate() {
        if has_materials {
            face_materials.push(mesh.face_material(f));
        }
        let mut expanded = Vec::new();
        let mut split = Vec::new();
        for j in 0..ring.len() {
            let a = ring[j];
            let b = ring[(j + 1) % ring.len()];
            if cuts.iter().any(|c| (c.t==0. && g.canonical[a as usize]==c.edge.0)
                || (c.t==1. && g.canonical[a as usize]==c.edge.1)) {
                split.push(expanded.len());
            }
            expanded.push(a);
            let e = edge(g.canonical[a as usize], g.canonical[b as usize]);
            if let Some(cut) = cuts.iter().find(|c| c.edge == e && c.t>0. && c.t<1.) {
                let key = (a.min(b), a.max(b));
                let mid = if let Some(&v) = added.get(&key) {
                    v
                } else {
                    if mesh.vertices.len() >= MAX_EDIT_VERTICES {
                        return Err(error("Нож превысит предел вершин"));
                    }
                    let t = if g.canonical[a as usize] == e.0 {
                        cut.t
                    } else {
                        1. - cut.t
                    };
                    let va = mesh.vertices[a as usize];
                    let vb = mesh.vertices[b as usize];
                    let mut v = va;
                    v.position = Vec3::from_array(va.position)
                        .lerp(Vec3::from_array(vb.position), t)
                        .to_array();
                    v.normal = Vec3::from_array(va.normal)
                        .lerp(Vec3::from_array(vb.normal), t)
                        .normalize_or_zero()
                        .to_array();
                    v.color = std::array::from_fn(|i| va.color[i] * (1. - t) + vb.color[i] * t);
                    v.uv = [
                        va.uv[0] + (vb.uv[0] - va.uv[0]) * t,
                        va.uv[1] + (vb.uv[1] - va.uv[1]) * t,
                    ];
                    let id = mesh.vertices.len() as u32;
                    mesh.vertices.push(v);
                    added.insert(key, id);
                    id
                };
                split.push(expanded.len());
                expanded.push(mid);
                if seams.contains(&e) {
                    new_seams.extend([[a, mid], [mid, b]]);
                }
            }
        }
        if f == face {
            if split.len() != 2 {
                return Err(error("Нож не пересёк две стороны грани"));
            }
            let (a, b) = (split[0], split[1]);
            let one = expanded[a..=b].to_vec();
            let two = expanded[b..]
                .iter()
                .chain(&expanded[..=a])
                .copied()
                .collect::<Vec<_>>();
            if one.len()<3 || two.len()<3 { return Err(error("Разрез совпадает с существующим ребром")); }
            let area = |r: &[u32]| crate::polygon_edit::face_normal(mesh, r).length();
            let original_area = area(ring);
            let one_area=area(&one);let two_area=area(&two);
            if one_area<=original_area.max(1e-8)*1e-7 || two_area<=original_area.max(1e-8)*1e-7
                || (one_area + two_area - original_area).abs() > original_area.max(1e-8) * 0.001 {
                return Err(error("Разрез выходит за грань; выберите другой отрезок"));
            }
            output.extend([one, two]);
            if has_materials {
                face_materials.push(mesh.face_material(f));
            }
        } else {
            output.push(expanded);
        }
    }
    let retained = seams
        .into_iter()
        .filter(|e| !cuts.iter().any(|c| c.edge == *e && c.t>0. && c.t<1.))
        .map(|(a, b)| [a as u32, b as u32]);
    new_seams.extend(retained);
    mesh.topology = Some(MeshTopology { loose_edges: mesh.topology.as_ref().map_or_else(Vec::new, |t|t.loose_edges.clone()),
        face_materials: face_materials.into(),
        faces: output.into(),
        smooth_normals: smooth,
        primitive: None,
        uv_seams: new_seams,
    });
    mesh.rebuild_polygon_indices()?;
    crate::recompute_normals(mesh, smooth);
    Ok(())
}
