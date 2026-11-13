//! Selected-edge bevel with bounded miter offsets, segmented profiles and end caps.
use crate::mesh_topology_tools::{faces, finish, invalid, loops};
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
        || !(0.0..=1.0).contains(&profile)
    {
        return Err(invalid("Ширина: 0–1000 м; сегменты: 1–32; профиль: 0–100%"));
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
    let mut cuts = BTreeMap::<(usize, usize), f32>::new();
    for &(a, b) in &g.edges {
        let length = positions[a].distance(positions[b]);
        if affected.contains(&a) || affected.contains(&b) {
            if width > length * 0.45 {
                return Err(invalid("Фаска слишком широка для соседнего ребра"));
            }
            if !selection.contains(&edge(a, b)) {
                if affected.contains(&a) {
                    cuts.insert((a, b), width);
                }
                if affected.contains(&b) {
                    cuts.insert((b, a), width);
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
            if incoming.cross(outgoing).dot(normal) <= 0.00001 {
                return Err(invalid(
                    "Угол грани рядом с фаской должен быть выпуклым и ненулевым",
                ));
            }
            if !left && !right {
                continue;
            }
            let ni = normal.cross(incoming);
            let no = normal.cross(outgoing);
            let cosine = ni.dot(no);
            let determinant = 1. - cosine * cosine;
            if determinant < 1e-8 {
                return Err(invalid("Слишком острый угол для устойчивой фаски"));
            }
            let wi = if left { width } else { 0. };
            let wo = if right { width } else { 0. };
            let delta = (ni * (wi - cosine * wo) + no * (wo - cosine * wi)) / determinant;
            if delta.length()
                > positions[before]
                    .distance(p)
                    .min(positions[after].distance(p))
                    * 0.49
            {
                return Err(invalid("Фаска пересекает соседний угол; уменьшите ширину"));
            }
            let mut q = p + delta;
            if left != right {
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
                    let a =
                        (d.dot(va) * vb.length_squared() - d.dot(vb) * va.dot(vb)) / determinant;
                    let b =
                        (d.dot(vb) * va.length_squared() - d.dot(va) * va.dot(vb)) / determinant;
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
    mesh.topology = Some(MeshTopology {
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
    for edges in cap_edges.values() {
        for mut cap in loops(edges)? {
            cap.reverse();
            // A straight profile subdivides the neighbouring face's cut edge.
            // Stitch those samples into that face instead of adding a zero-area cap.
            let polygon: Vec<u32> = cap.iter().map(|&i| i as u32).collect();
            if crate::polygon_edit::face_normal(mesh, &polygon).length_squared() < 1e-14 {
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
