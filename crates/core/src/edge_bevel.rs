//! Selected-edge bevel with bounded miter offsets, segmented profiles and end caps.
use crate::mesh_topology_tools::{faces, finish, invalid};
use crate::{
    CoreError, Mesh, MeshConnectivity, MeshTopology, Vec3, Vertex, MAX_EDIT_TRIANGLES,
    MAX_EDIT_VERTICES,
};
use std::collections::{BTreeMap, BTreeSet};
#[path = "edge_bevel/limits.rs"]
mod limits;
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
    build(mesh, selection, width, segments, profile, None)
}

/// Offset selected caps and interpolate that offset through every profile sample.
/// Directions refer to the source mesh's canonical vertices, before beveling.
pub(crate) fn apply_with_face_offsets(
    mesh: &mut Mesh,
    selection: &BTreeSet<(usize, usize)>,
    width: f32,
    segments: u32,
    profile: f32,
    faces: &BTreeSet<usize>,
    offsets: &BTreeMap<usize, Vec3>,
) -> Result<(), CoreError> {
    build(
        mesh,
        selection,
        width,
        segments,
        profile,
        Some((faces, offsets)),
    )
}

fn build(
    mesh: &mut Mesh,
    selection: &BTreeSet<(usize, usize)>,
    width: f32,
    segments: u32,
    profile: f32,
    displacement: Option<(&BTreeSet<usize>, &BTreeMap<usize, Vec3>)>,
) -> Result<(), CoreError> {
    let original = mesh.clone();
    let mut requested = width;
    let mut last_error = None;
    for _ in 0..9 {
        let mut effective = None;
        match build_attempt(
            mesh,
            selection,
            requested,
            segments,
            profile,
            displacement,
            &mut effective,
        ) {
            Ok(()) => return Ok(()),
            Err(error) => {
                *mesh = original.clone();
                let Some(limit) = effective else {
                    return Err(error);
                };
                last_error = Some(error);
                requested = (limit * 0.5).min(requested * 0.5);
            }
        }
    }
    Err(last_error.unwrap())
}

fn build_attempt(
    mesh: &mut Mesh,
    selection: &BTreeSet<(usize, usize)>,
    width: f32,
    segments: u32,
    profile: f32,
    displacement: Option<(&BTreeSet<usize>, &BTreeMap<usize, Vec3>)>,
    retry_width: &mut Option<f32>,
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
    let region = displacement.map(|(faces, _)| faces);
    let widths = limits::widths(
        mesh, &g, &source, &selection, &affected, &positions, width, region,
    )?;
    // Retry only after valid selection and budgets have reached geometric limits.
    // Concave/non-planar neighbours can collide before adjacent edge endpoints do.
    *retry_width = widths.values().copied().reduce(f32::min);
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
        if limits::interior(&g, region, edge(a, b)) {
            continue;
        }
        if d + cuts.get(&(b, a)).copied().unwrap_or(0.)
            >= positions[a].distance(positions[b]) * 0.9995
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
    if let Some(region) = region {
        let mut fixed = BTreeMap::new();
        for &f in region {
            for &raw in &output[f] {
                let v = tags[raw as usize];
                if affected.contains(&v) {
                    fixed.insert(
                        v,
                        Vec3::from_array(mesh.vertices[raw as usize].position) - positions[v],
                    );
                }
            }
        }
        let offsets = crate::region_offset::interpolate(&source, &g, region, &positions, fixed);
        let graph = MeshConnectivity::new(mesh);
        let mut moves = BTreeMap::new();
        for &f in region {
            for &raw in &output[f] {
                if let Some(&delta) = offsets.get(&tags[raw as usize]) {
                    moves.insert(
                        graph.canonical[raw as usize],
                        positions[tags[raw as usize]] + delta,
                    );
                }
            }
        }
        for (i, vertex) in mesh.vertices.iter_mut().enumerate() {
            if let Some(&p) = moves.get(&graph.canonical[i]) {
                vertex.position = p.to_array();
            }
        }
    }
    // Record cap endpoints before straight-profile stitching inserts samples
    // into neighbouring polygons. Those inserted points still need interpolation.
    let cap_vertices: BTreeSet<_> = displacement
        .map(|(faces, _)| {
            faces
                .iter()
                .flat_map(|&f| output[f].iter().copied())
                .collect()
        })
        .unwrap_or_default();
    let mut profile_samples = Vec::new();
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
        let normal = |face: &[u32]| {
            (1..face.len() - 1)
                .map(|i| {
                    (positions[face[i] as usize] - positions[face[0] as usize])
                        .cross(positions[face[i + 1] as usize] - positions[face[0] as usize])
                })
                .sum::<Vec3>()
                .normalize()
        };
        let na = normal(af);
        let nb = normal(bf);
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
                if displacement.is_some() {
                    profile_samples.push((id, first, last, t));
                }
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
    let corner_start = output.len();
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
    if let Some((_, offsets)) = displacement {
        // Keep a parameter-space tessellation for corner patches. Moving caps
        // inward can fold a narrow patch so its 2D projection crosses itself,
        // although its original triangle connectivity remains valid in 3D.
        let corner_triangles = output[corner_start..]
            .iter()
            .map(|face| crate::triangulate::triangulate(mesh, face))
            .collect::<Result<Vec<_>, _>>()?;
        let graph = MeshConnectivity::new(mesh);
        let mut shifts = BTreeMap::new();
        for raw in cap_vertices {
            if let Some(&delta) = offsets.get(&tags[raw as usize]) {
                shifts.insert(graph.canonical[raw as usize], delta);
            }
        }
        let cap_shifts = shifts.clone();
        for (sample, first, last, t) in profile_samples {
            let first = cap_shifts
                .get(&graph.canonical[first as usize])
                .copied()
                .unwrap_or(Vec3::ZERO);
            let last = cap_shifts
                .get(&graph.canonical[last as usize])
                .copied()
                .unwrap_or(Vec3::ZERO);
            shifts
                .entry(graph.canonical[sample as usize])
                .or_insert(first.lerp(last, t));
        }
        // Geometric corner copies at UV seams receive exactly the same shift.
        for (i, vertex) in mesh.vertices.iter_mut().enumerate() {
            if let Some(&delta) = shifts.get(&graph.canonical[i]) {
                vertex.position = (Vec3::from_array(vertex.position) + delta).to_array();
            }
        }
        let corners = output.split_off(corner_start);
        for (face, triangles) in corners.into_iter().zip(corner_triangles) {
            if crate::triangulate::triangulate(mesh, &face).is_ok() {
                output.push(face);
            } else {
                output.extend(triangles.chunks_exact(3).map(|triangle| triangle.to_vec()));
            }
        }
        if let Some(topology) = &mut mesh.topology {
            if !topology.face_materials.is_empty() {
                topology.face_materials.resize(output.len(), None);
            }
            topology.faces = output.clone().into();
        }
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
