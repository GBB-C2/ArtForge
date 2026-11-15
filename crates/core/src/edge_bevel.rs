//! Selected-edge bevel with bounded miter offsets, segmented profiles and end caps.
use crate::mesh_topology_tools::{faces, finish, invalid};
use crate::{
    CoreError, Mesh, MeshConnectivity, MeshTopology, Vec3, Vertex, MAX_EDIT_TRIANGLES,
    MAX_EDIT_VERTICES,
};
use std::collections::{BTreeMap, BTreeSet};
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}
fn add(mesh: &mut Mesh, tags: &mut Vec<usize>, vertex: Vertex, position: Vec3, tag: usize) -> u32 {
    let id = mesh.vertices.len() as u32;
    let mut v = vertex;
    v.position = position.to_array();
    mesh.vertices.push(v);
    tags.push(tag);
    id
}
// A near-flat miter may have several cap contours touching at one corner.
// Walk directed boundary cycles rather than rejecting every valence above two.
fn corner_cycles(edges: &[(usize, usize)]) -> Result<Vec<Vec<usize>>, CoreError> {
    let mut remaining: BTreeSet<_> = edges.iter().copied().collect();
    let mut cycles = Vec::new();
    while let Some(&(start, _)) = remaining.first() {
        let mut path = vec![start];
        let mut cursor = start;
        loop {
            let next = remaining
                .range((cursor, 0)..=(cursor, usize::MAX))
                .next()
                .copied();
            let Some(e) = next else {
                if path.len() != 1 {
                    return Err(invalid("Не удалось соединить контур угла фаски"));
                }
                break;
            };
            remaining.remove(&e);
            cursor = e.1;
            if let Some(i) = path.iter().position(|&v| v == cursor) {
                if path.len() - i >= 3 {
                    cycles.push(path[i..].to_vec());
                }
                path.truncate(i + 1);
            } else {
                path.push(cursor);
            }
        }
    }
    Ok(cycles)
}
pub(crate) fn apply(
    mesh: &mut Mesh,
    selection: &BTreeSet<(usize, usize)>,
    width: f32,
    segments: u32,
    profile: f32,
) -> Result<(), CoreError> {
    if selection.is_empty()
        || selection
            .iter()
            .any(|&(a, b)| a >= mesh.vertices.len() || b >= mesh.vertices.len())
    {
        return Err(invalid("Выберите рёбра для фаски"));
    }
    if !width.is_finite()
        || !(0.0..=1000.).contains(&width)
        || !(1..=32).contains(&segments)
        || !profile.is_finite()
        || !(-1.0..=1.0).contains(&profile)
    {
        return Err(invalid(
            "Ширина: 0–1000 м; сегменты: 1–32; профиль: −100–100%",
        ));
    }
    if width == 0. {
        return Ok(());
    }
    let g = MeshConnectivity::new(mesh);
    let selection: BTreeSet<_> = selection
        .iter()
        .map(|&(a, b)| edge(g.canonical[a], g.canonical[b]))
        .collect();
    if !selection.is_subset(&g.edges) {
        return Err(invalid("Выбрано несуществующее ребро"));
    }
    let affected: BTreeSet<_> = selection.iter().flat_map(|&(a, b)| [a, b]).collect();
    for (e, fs) in &g.edge_faces {
        if (affected.contains(&e.0) || affected.contains(&e.1)) && fs.len() != 2 {
            return Err(invalid(
                "Фаска требует замкнутую окрестность рёбер без разветвлений",
            ));
        }
    }
    let source = faces(mesh);
    let corners = source
        .iter()
        .flatten()
        .filter(|&&v| affected.contains(&g.canonical[v as usize]))
        .count();
    if mesh.vertices.len() + corners * 3 + selection.len() * 2 * (segments as usize - 1)
        > MAX_EDIT_VERTICES
        || mesh.indices.len() / 3 + corners * 3 + selection.len() * 4 * segments as usize
            > MAX_EDIT_TRIANGLES
    {
        return Err(invalid(
            "Фаска превышает предел сетки; уменьшите выбор или сегменты",
        ));
    }
    let positions: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| Vec3::from_array(v.position))
        .collect();
    // First find local collision limits, then share the tightest limit along
    // each connected selection so the bevel cannot taper between its endpoints.
    let mut widths: BTreeMap<usize, f32> = affected.iter().map(|&v| (v, width)).collect();
    for &(a, b) in &g.edges {
        let limit = positions[a].distance(positions[b]) * 0.2;
        for v in [a, b] {
            if let Some(w) = widths.get_mut(&v) {
                *w = w.min(limit);
            }
        }
    }
    for face in &source {
        let normal = crate::polygon_edit::face_normal(mesh, face).normalize_or_zero();
        for i in 0..face.len() {
            let v = g.canonical[face[i] as usize];
            if !affected.contains(&v) {
                continue;
            }
            let a = g.canonical[face[(i + face.len() - 1) % face.len()] as usize];
            let b = g.canonical[face[(i + 1) % face.len()] as usize];
            let left = selection.contains(&edge(a, v));
            let right = selection.contains(&edge(v, b));
            if !left && !right {
                continue;
            }
            let incoming = (positions[v] - positions[a]).normalize();
            let outgoing = (positions[b] - positions[v]).normalize();
            let ni = normal.cross(incoming);
            let no = normal.cross(outgoing);
            let cosine = ni.dot(no);
            let det = 1. - cosine * cosine;
            if det > 1e-8 {
                let wi = if left { 1. } else { 0. };
                let wo = if right { 1. } else { 0. };
                let miter = (ni * (wi - cosine * wo) + no * (wo - cosine * wi)) / det;
                let limit = positions[a]
                    .distance(positions[v])
                    .min(positions[b].distance(positions[v]))
                    * 0.2
                    / miter.length().max(1.);
                *widths.get_mut(&v).unwrap() = widths[&v].min(limit);
            }
        }
    }
    let mut pending = affected.clone();
    while let Some(seed) = pending.pop_first() {
        let mut component = vec![seed];
        let mut cursor = 0;
        let mut limit = widths[&seed];
        while cursor < component.len() {
            let vertex = component[cursor];
            cursor += 1;
            for &neighbor in &g.vertex_neighbors[vertex] {
                if selection.contains(&edge(vertex, neighbor)) && pending.remove(&neighbor) {
                    limit = limit.min(widths[&neighbor]);
                    component.push(neighbor);
                }
            }
        }
        for vertex in component { widths.insert(vertex, limit); }
    }
    let mut cuts = BTreeMap::<(usize, usize), f32>::new();
    for &(a, b) in &g.edges {
        if affected.contains(&a) || affected.contains(&b) {
            if !selection.contains(&edge(a, b)) {
                if affected.contains(&a) {
                    cuts.insert((a, b), widths[&a]);
                }
                if affected.contains(&b) {
                    cuts.insert((b, a), widths[&b]);
                }
            }
        }
    }
    let mut anchors = BTreeMap::<(usize, usize), Vec3>::new();
    for (f, face) in source.iter().enumerate() {
        let normal = crate::polygon_edit::face_normal(mesh, face)
            .try_normalize()
            .ok_or_else(|| invalid("Вырожденная грань рядом с фаской"))?;
        for i in 0..face.len() {
            let v = g.canonical[face[i] as usize];
            if !affected.contains(&v) {
                continue;
            }
            let before = g.canonical[face[(i + face.len() - 1) % face.len()] as usize];
            let after = g.canonical[face[(i + 1) % face.len()] as usize];
            let p = positions[v];
            let incoming = (p - positions[before])
                .try_normalize()
                .ok_or_else(|| invalid("Нулевое ребро"))?;
            let outgoing = (positions[after] - p)
                .try_normalize()
                .ok_or_else(|| invalid("Нулевое ребро"))?;
            let left = selection.contains(&edge(before, v));
            let right = selection.contains(&edge(v, after));
            if !left && !right {
                continue;
            }
            // A knife can split one side of a crease while leaving a straight
            // corner in the neighbouring polygon. A bevel ending there needs
            // an inset anchor and a short end cap, not an intersection of two
            // parallel offset lines.
            let straight = incoming.dot(outgoing) > 0.99999;
            if !straight && incoming.cross(outgoing).dot(normal).abs() <= 0.00001 {
                return Err(invalid("Угол грани рядом с фаской должен быть ненулевым"));
            }
            let ni = normal.cross(incoming);
            let no = normal.cross(outgoing);
            let cosine = ni.dot(no);
            let determinant = 1. - cosine * cosine;
            if determinant < 1e-8 && !straight {
                return Err(invalid("Слишком острый угол для устойчивой фаски"));
            }
            let width = widths[&v];
            let wi = if left { width } else { 0. };
            let wo = if right { width } else { 0. };
            let delta = if straight {
                ni * width
            } else {
                (ni * (wi - cosine * wo) + no * (wo - cosine * wi)) / determinant
            };
            if delta.length()
                > positions[before]
                    .distance(p)
                    .min(positions[after].distance(p))
                    * 0.49
            {
                return Err(invalid("Фаска пересекает соседний угол; уменьшите ширину"));
            }
            let mut q = p + delta;
            if left != right && !straight {
                let neighbor = if left { after } else { before };
                let direction = (positions[neighbor] - p).normalize();
                let distance = delta.dot(direction);
                if distance <= 0. {
                    return Err(invalid("Неподдерживаемый вогнутый угол фаски"));
                }
                let d = cuts.entry((v, neighbor)).or_default();
                *d = d.max(distance);
                q = p + direction * distance;
            }
            anchors.insert((f, v), q);
        }
    }
    for (&(a, b), &d) in &cuts {
        if d + cuts.get(&(b, a)).copied().unwrap_or(0.)
            >= positions[a].distance(positions[b]) * 0.98
        {
            return Err(invalid("Фаски соседних вершин пересекаются"));
        }
    }
    let cut_point = |a: usize, b: usize| {
        positions[a] + (positions[b] - positions[a]).normalize() * cuts[&(a, b)]
    };
    let smooth = mesh.smooth_normals_enabled();
    let mut tags = g.canonical.clone();
    let mut output = Vec::new();
    let source_materials = mesh
        .topology
        .as_ref()
        .map(|t| t.face_materials.clone())
        .unwrap_or_default();
    let mut endpoint = BTreeMap::<(usize, usize), u32>::new();
    for (f, face) in source.iter().enumerate() {
        let mut out = Vec::new();
        for i in 0..face.len() {
            let raw = face[i] as usize;
            let v = g.canonical[raw];
            if !affected.contains(&v) {
                out.push(raw as u32);
                continue;
            }
            let previous = face[(i + face.len() - 1) % face.len()] as usize;
            let following = face[(i + 1) % face.len()] as usize;
            let before = g.canonical[previous];
            let after = g.canonical[following];
            let left = selection.contains(&edge(before, v));
            let right = selection.contains(&edge(v, after));
            let mut vertex = mesh.vertices[raw];
            let p = positions[v];
            let va = positions[before] - p;
            let vb = positions[after] - p;
            let determinant = va.length_squared() * vb.length_squared() - va.dot(vb).powi(2);
            let uv_a = mesh.vertices[previous].uv;
            let uv_b = mesh.vertices[following].uv;
            let original_uv = vertex.uv;
            let original_color = vertex.color;
            let color_a = mesh.vertices[previous].color;
            let color_b = mesh.vertices[following].color;
            let mut points = Vec::new();
            if !left {
                points.push((cut_point(v, before), false));
            }
            if left || right {
                let mut q = anchors[&(f, v)];
                // At a partial bevel, both incident faces must end at the same
                // point of the untouched edge. Keeping each miter's own distance
                // creates a backtracking spike in the neighbouring polygon.
                if left != right && (-va).normalize().dot(vb.normalize()) < 0.99999 {
                    q = cut_point(v, if left { after } else { before });
                }
                if !left {
                    let c = cut_point(v, before);
                    if q.distance(c) <= width.max(0.001) * 1e-5 {
                        q = c;
                    }
                }
                if !right {
                    let c = cut_point(v, after);
                    if q.distance(c) <= width.max(0.001) * 1e-5 {
                        q = c;
                    }
                }
                points.push((q, true));
            }
            if !right {
                points.push((cut_point(v, after), false));
            }
            let mut last = None;
            for (q, anchor) in points {
                let id = if let Some((position, id)) = last.filter(|(position, _)| *position == q) {
                    let _ = position;
                    id
                } else {
                    let d = q - p;
                    let (a, b) = if determinant.abs() > 1e-12 {
                        (
                            (d.dot(va) * vb.length_squared() - d.dot(vb) * va.dot(vb))
                                / determinant,
                            (d.dot(vb) * va.length_squared() - d.dot(va) * va.dot(vb))
                                / determinant,
                        )
                    } else {
                        (d.dot(va) / va.length_squared().max(1e-12), 0.)
                    };
                    for axis in 0..2 {
                        vertex.uv[axis] = original_uv[axis]
                            + a * (uv_a[axis] - original_uv[axis])
                            + b * (uv_b[axis] - original_uv[axis]);
                    }
                    vertex.color = std::array::from_fn(|i| {
                        (original_color[i]
                            + a * (color_a[i] - original_color[i])
                            + b * (color_b[i] - original_color[i]))
                            .clamp(0., 1.)
                    });
                    let id = add(mesh, &mut tags, vertex, q, v);
                    out.push(id);
                    last = Some((q, id));
                    id
                };
                if anchor {
                    endpoint.insert((f, v), id);
                }
            }
        }
        output.push(out);
    }
    for &selected in &selection {
        let adjacent = &g.edge_faces[&selected];
        let (fa, fb) = (adjacent[0], adjacent[1]);
        let af = &source[fa];
        let j = (0..af.len())
            .find(|&i| {
                edge(
                    g.canonical[af[i] as usize],
                    g.canonical[af[(i + 1) % af.len()] as usize],
                ) == selected
            })
            .unwrap();
        let a = g.canonical[af[j] as usize];
        let b = g.canonical[af[(j + 1) % af.len()] as usize];
        let bf = &source[fb];
        if !(0..bf.len()).any(|i| {
            g.canonical[bf[i] as usize] == b && g.canonical[bf[(i + 1) % bf.len()] as usize] == a
        }) {
            return Err(invalid("У соседних граней несогласованное направление"));
        }
        let na = crate::polygon_edit::face_normal(mesh, af).normalize();
        let nb = crate::polygon_edit::face_normal(mesh, bf).normalize();
        if na.dot(nb).abs() > 0.99999 {
            return Err(invalid("Выбранное ребро плоское; используйте растворение"));
        }
        let weight = ((1. + na.dot(nb)) * 0.5).sqrt().max(0.05);
        let mut sides = [Vec::new(), Vec::new()];
        for (end, v) in [a, b].into_iter().enumerate() {
            let first = endpoint[&(fa, v)];
            let last = endpoint[&(fb, v)];
            let av = mesh.vertices[first as usize];
            let bv = mesh.vertices[last as usize];
            let pa = Vec3::from_array(av.position);
            let pb = Vec3::from_array(bv.position);
            sides[end].push(first);
            for s in 1..segments {
                let t = s as f32 / segments as f32;
                let u = 1. - t;
                let arc = (pa * u * u + positions[v] * (2. * weight * t * u) + pb * t * t)
                    / (u * u + 2. * weight * t * u + t * t);
                let p = pa.lerp(pb, t).lerp(arc, profile);
                let mut vertex = av;
                for axis in 0..2 {
                    vertex.uv[axis] = av.uv[axis] * (1. - t) + bv.uv[axis] * t;
                }
                vertex.color = std::array::from_fn(|i| av.color[i] * (1. - t) + bv.color[i] * t);
                let id = add(mesh, &mut tags, vertex, p, v);
                sides[end].push(id);
            }
            sides[end].push(last);
        }
        for s in 0..segments as usize {
            output.push(vec![
                sides[1][s],
                sides[0][s],
                sides[0][s + 1],
                sides[1][s + 1],
            ]);
        }
    }
    // Miter paths on almost coplanar faces can coincide to f32 precision.
    // Weld only samples belonging to the SAME original corner, then remove
    // backtracking boundary spurs; otherwise tiny caps form false branches.
    let mut corner_samples = BTreeMap::<usize, Vec<Vec3>>::new();
    for (i, v) in mesh.vertices.iter_mut().enumerate().skip(positions.len()) {
        let tolerance = (widths[&tags[i]] * 0.002).max(1e-6);
        let p = Vec3::from_array(v.position);
        let samples = corner_samples.entry(tags[i]).or_default();
        if let Some(&q) = samples
            .iter()
            .find(|q| q.distance_squared(p) < tolerance * tolerance)
        {
            v.position = q.to_array();
        } else {
            samples.push(p);
        }
    }
    for face in &mut output {
        let same = |a: u32, b: u32| {
            crate::position_key(mesh.vertices[a as usize].position)
                == crate::position_key(mesh.vertices[b as usize].position)
        };
        loop {
            let duplicate = (0..face.len()).find(|&i| same(face[i], face[(i + 1) % face.len()]));
            if let Some(i) = duplicate {
                face.remove(i);
                continue;
            }
            let spur = (0..face.len()).find(|&i| {
                same(
                    face[(i + face.len() - 1) % face.len()],
                    face[(i + 1) % face.len()],
                )
            });
            if let Some(i) = spur {
                face.remove(i);
                continue;
            }
            break;
        }
    }
    if output[..source.len()].iter().any(|f| f.len() < 3) {
        return Err(invalid("Фаска поглощает исходную грань"));
    }
    output.retain(|f| {
        f.len() >= 3
            && crate::polygon_edit::face_normal(mesh, f).length_squared() > f32::MIN_POSITIVE
    });
    mesh.topology = Some(MeshTopology {
        loose_edges: Vec::new(),
        face_materials: {
            let mut materials = source_materials;
            if !materials.is_empty() {
                materials.resize(output.len(), None);
            }
            materials
        },
        uv_seams: Vec::new(),
        faces: output.clone().into(),
        primitive: None,
        smooth_normals: smooth,
    });
    let boundary = MeshConnectivity::new(mesh);
    let mut cap_edges = BTreeMap::<usize, Vec<(usize, usize)>>::new();
    for (&e, adj) in &boundary.edge_faces {
        if adj.len() != 1 {
            continue;
        }
        if tags[e.0] != tags[e.1] || !affected.contains(&tags[e.0]) {
            continue;
        }
        let face = &output[adj[0]];
        for j in 0..face.len() {
            let a = boundary.canonical[face[j] as usize];
            let b = boundary.canonical[face[(j + 1) % face.len()] as usize];
            if edge(a, b) == e {
                cap_edges.entry(tags[a]).or_default().push((a, b));
                break;
            }
        }
    }
    for (tag, edges) in &cap_edges {
        let width = widths[tag];
        for mut cap in corner_cycles(edges)? {
            cap.reverse();
            // A straight profile subdivides the neighbouring face's cut edge.
            // Stitch those samples into that face instead of adding a zero-area cap.
            let polygon: Vec<u32> = cap.iter().map(|&i| i as u32).collect();
            if crate::polygon_edit::face_normal(mesh, &polygon).length_squared()
                < (width * width * 1e-5).powi(2)
            {
                let k = (0..cap.len())
                    .max_by(|&a, &b| {
                        let length = |i: usize| {
                            Vec3::from_array(mesh.vertices[cap[i]].position).distance_squared(
                                Vec3::from_array(mesh.vertices[cap[(i + 1) % cap.len()]].position),
                            )
                        };
                        length(a).total_cmp(&length(b))
                    })
                    .unwrap();
                let a = cap[(k + 1) % cap.len()];
                let b = cap[k];
                let path: Vec<u32> = (0..cap.len())
                    .map(|j| cap[(k + 1 + j) % cap.len()] as u32)
                    .collect();
                let mut stitched = false;
                for face in &mut output {
                    if let Some(i) = (0..face.len()).find(|&i| {
                        boundary.canonical[face[i] as usize] == a
                            && boundary.canonical[face[(i + 1) % face.len()] as usize] == b
                    }) {
                        face.splice(i + 1..i + 1, path[1..path.len() - 1].iter().copied());
                        stitched = true;
                        break;
                    }
                }
                if !stitched {
                    return Err(invalid("Не удалось соединить прямой профиль фаски"));
                }
                continue;
            }
            output.push(cap.into_iter().map(|i| i as u32).collect());
        }
    }
    if let Some(t) = &mut mesh.topology {
        if !t.face_materials.is_empty() {
            t.face_materials.resize(output.len(), None);
        }
        t.faces = output.clone().into();
    }
    finish(mesh, output, smooth)?;
    let result = MeshConnectivity::new(mesh);
    if result.edge_faces.values().any(|f| f.len() != 2)
        && g.edge_faces.values().all(|f| f.len() == 2)
    {
        return Err(invalid(
            "Фаска не смогла замкнуть угол; уменьшите ширину или сегменты",
        ));
    }
    Ok(())
}
