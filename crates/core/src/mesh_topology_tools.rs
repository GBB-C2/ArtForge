//! Bounded polygon cleanup, boundary bridging, diagnostics and edge paths.
use crate::{
    CoreError, Mesh, MeshConnectivity, MeshTopology, Vec3, MAX_EDIT_TRIANGLES, MAX_EDIT_VERTICES,
    MAX_FACE_CORNERS,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub(crate) fn invalid(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
pub(crate) fn faces(mesh: &Mesh) -> Vec<Vec<u32>> {
    (0..mesh.face_count())
        .map(|f| mesh.face_vertices(f).unwrap().to_vec())
        .collect()
}
pub(crate) fn finish(
    mesh: &mut Mesh,
    source: Vec<Vec<u32>>,
    smooth: bool,
) -> Result<(), CoreError> {
    if source.is_empty()
        || source
            .iter()
            .any(|f| !(3..=MAX_FACE_CORNERS).contains(&f.len()))
        || source.iter().map(|f| f.len() - 2).sum::<usize>() > MAX_EDIT_TRIANGLES
    {
        return Err(invalid(
            "Операция превышает предел граней или оставляет пустую сетку",
        ));
    }
    let materials: BTreeMap<_, _> = (0..mesh.face_count())
        .map(|i| {
            let mut key = mesh.face_vertices(i).unwrap().to_vec();
            key.sort_unstable();
            (
                key,
                mesh.topology
                    .as_ref()
                    .and_then(|t| t.face_materials.get(i).copied().flatten()),
            )
        })
        .collect();
    let has_materials = mesh
        .topology
        .as_ref()
        .is_some_and(|t| !t.face_materials.is_empty());
    let face_materials = if has_materials {
        source
            .iter()
            .map(|face| {
                let mut key = face.clone();
                key.sort_unstable();
                materials.get(&key).copied().flatten()
            })
            .collect()
    } else {
        Default::default()
    };
    let mut map = BTreeMap::new();
    let mut vertices = Vec::new();
    let mut output = Vec::new();
    for mut face in source {
        // New concave quads need an interior diagonal. Rotate their boundary so
        // the legacy fan layout remains compatible with existing project files.
        if face.len() == 4 {
            let normal = crate::polygon_edit::face_normal(mesh, &face);
            if normal.length_squared() <= f32::MIN_POSITIVE {
                return Err(invalid("Операция создаёт вырожденную грань"));
            }
            let mut valid = false;
            for _ in 0..4 {
                let p = |i: usize| Vec3::from_array(mesh.vertices[face[i] as usize].position);
                if (p(1) - p(0)).cross(p(2) - p(0)).dot(normal) > 0.
                    && (p(2) - p(0)).cross(p(3) - p(0)).dot(normal) > 0.
                {
                    valid = true;
                    break;
                }
                face.rotate_left(1);
            }
            if !valid {
                return Err(invalid(
                    "Операция создаёт пересечение или вырожденную грань",
                ));
            }
        }
        let mut next = Vec::new();
        for id in face {
            let id = *map.entry(id).or_insert_with(|| {
                let v = vertices.len() as u32;
                vertices.push(mesh.vertices[id as usize]);
                v
            });
            next.push(id);
        }
        output.push(next);
    }
    if vertices.len() > MAX_EDIT_VERTICES {
        return Err(invalid("Операция превышает предел вершин"));
    }
    mesh.vertices = vertices.into();
    mesh.topology = Some(MeshTopology { loose_edges: Vec::new(),
        face_materials,
        uv_seams: Vec::new(),
        faces: output.into(),
        primitive: None,
        smooth_normals: smooth,
    });
    mesh.rebuild_polygon_indices()?;
    crate::recompute_normals(mesh, smooth);
    mesh.validate_topology()
}
pub(crate) fn loops(edges: &[(usize, usize)]) -> Result<Vec<Vec<usize>>, CoreError> {
    let mut next = BTreeMap::new();
    let mut incoming = BTreeSet::new();
    for &(a, b) in edges {
        if a == b || next.insert(a, b).is_some() || !incoming.insert(b) {
            return Err(invalid("Контур имеет развилку или повторяющееся ребро"));
        }
    }
    if next.keys().copied().collect::<BTreeSet<_>>() != incoming {
        return Err(invalid("Выберите замкнутые контуры"));
    }
    let mut result = Vec::new();
    while let Some((&start, _)) = next.first_key_value() {
        let mut ring = Vec::new();
        let mut at = start;
        loop {
            ring.push(at);
            at = next.remove(&at).ok_or_else(|| invalid("Контур разорван"))?;
            if at == start {
                break;
            }
            if ring.len() > MAX_FACE_CORNERS {
                return Err(invalid("Слишком длинный контур: максимум 128 вершин"));
            }
        }
        if ring.len() < 3 {
            return Err(invalid("Контур должен содержать хотя бы три вершины"));
        }
        result.push(ring);
    }
    Ok(result)
}
fn directed(mesh: &Mesh, g: &MeshConnectivity, f: usize, edge: (usize, usize)) -> (usize, usize) {
    let face = mesh.face_vertices(f).unwrap();
    for j in 0..face.len() {
        let a = g.canonical[face[j] as usize];
        let b = g.canonical[face[(j + 1) % face.len()] as usize];
        if (a.min(b), a.max(b)) == edge {
            return (a, b);
        }
    }
    unreachable!()
}
fn selected_edges(
    mesh: &Mesh,
    selection: &BTreeSet<(usize, usize)>,
    g: &MeshConnectivity,
) -> Result<BTreeSet<(usize, usize)>, CoreError> {
    if selection.is_empty()
        || selection
            .iter()
            .any(|&(a, b)| a >= mesh.vertices.len() || b >= mesh.vertices.len())
    {
        return Err(invalid("Выберите рёбра"));
    }
    let edges: BTreeSet<_> = selection
        .iter()
        .map(|&(a, b)| {
            let (a, b) = (g.canonical[a], g.canonical[b]);
            (a.min(b), a.max(b))
        })
        .collect();
    if !edges.is_subset(&g.edges) {
        return Err(invalid("Выбрано несуществующее ребро"));
    }
    Ok(edges)
}
pub(crate) fn merge(
    mesh: &mut Mesh,
    selection: &BTreeSet<usize>,
    distance: Option<f32>,
) -> Result<(), CoreError> {
    if selection.iter().any(|&i| i >= mesh.vertices.len()) {
        return Err(invalid("Некорректный выбор вершин"));
    }
    let g = MeshConnectivity::new(mesh);
    let selected: BTreeSet<_> = selection.iter().map(|&i| g.canonical[i]).collect();
    if selected.len() < 2 {
        return Err(invalid("Выберите минимум две разные вершины"));
    }
    let mut representatives = BTreeMap::new();
    if let Some(distance) = distance {
        if !distance.is_finite() || !(0.000001..=10000.).contains(&distance) {
            return Err(invalid("Расстояние сварки: 0,000001–10000 м"));
        }
        // Only separated representatives enter buckets; dense coincident points
        // cannot create a quadratic all-pairs search.
        let mut cells = BTreeMap::<[i64; 3], Vec<usize>>::new();
        for &i in &selected {
            let p = Vec3::from_array(mesh.vertices[i].position);
            let cell = p
                .to_array()
                .map(|v| (v as f64 / distance as f64).floor() as i64);
            let mut best = None;
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        if let Some(ids) = cells.get(&[
                            cell[0].saturating_add(x),
                            cell[1].saturating_add(y),
                            cell[2].saturating_add(z),
                        ]) {
                            for &j in ids {
                                if p.distance(Vec3::from_array(mesh.vertices[j].position))
                                    <= distance
                                    && best.is_none_or(|old| j < old)
                                {
                                    best = Some(j);
                                }
                            }
                        }
                    }
                }
            }
            let r = best.unwrap_or(i);
            if best.is_none() {
                cells.entry(cell).or_default().push(i);
            }
            representatives.insert(i, r);
        }
    } else {
        let first = *selected.first().unwrap();
        for &i in &selected {
            representatives.insert(i, first);
        }
    }
    if representatives.iter().all(|(a, b)| a == b)
        && !g
            .canonical
            .iter()
            .enumerate()
            .any(|(i, &c)| i != c && selected.contains(&c))
    {
        return Ok(());
    }
    let mut centers = BTreeMap::<usize, (Vec3, usize)>::new();
    for (&i, &r) in &representatives {
        let e = centers.entry(r).or_default();
        e.0 += Vec3::from_array(mesh.vertices[i].position);
        e.1 += 1;
    }
    let smooth = mesh.smooth_normals_enabled();
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    let mut attributes = Vec::new();
    for (face_index, face) in faces(mesh).into_iter().enumerate() {
        let mut next = Vec::new();
        for i in face {
            let canonical = g.canonical[i as usize];
            let target = representatives
                .get(&canonical)
                .copied()
                .unwrap_or(i as usize) as u32;
            if next.last() != Some(&target) {
                next.push(target);
            }
        }
        if next.len() > 1 && next.first() == next.last() {
            next.pop();
        }
        if next.len() < 3 {
            continue;
        }
        if next.iter().copied().collect::<BTreeSet<_>>().len() != next.len() {
            return Err(invalid(
                "Сварка создаёт самокасание грани; уменьшите расстояние",
            ));
        }
        let mut key = next.clone();
        key.sort_unstable();
        if seen.insert(key) {
            attributes.push(mesh.face_material(face_index));
            output.push(next);
        }
    }
    for (i, (sum, count)) in centers {
        mesh.vertices[i].position = (sum / count as f32).to_array();
    }
    if mesh
        .topology
        .as_ref()
        .is_some_and(|t| !t.face_materials.is_empty())
    {
        let t = mesh.topology.as_mut().unwrap();
        t.faces = output.clone().into();
        t.face_materials = attributes.into();
    }
    finish(mesh, output, smooth)
}
pub(crate) fn bridge(
    mesh: &mut Mesh,
    selection: &BTreeSet<(usize, usize)>,
    segments: u32,
) -> Result<(), CoreError> {
    bridge_options(mesh,selection,segments,0,0.)
}
pub(crate) fn bridge_options(mesh:&mut Mesh,selection:&BTreeSet<(usize,usize)>,segments:u32,twist:i32,smooth:f32)->Result<(),CoreError> {
    if !(1..=32).contains(&segments) {
        return Err(invalid("Сегменты моста: 1–32"));
    }
    let g = MeshConnectivity::new(mesh);
    let edges = selected_edges(mesh, selection, &g)?;
    if edges.iter().any(|e| g.edge_faces[e].len() > 1) {
        return Err(invalid("Мост соединяет два открытых граничных контура"));
    }
    let mut degrees = BTreeMap::<usize,usize>::new();
    for &(a,b) in &edges { *degrees.entry(a).or_default()+=1; *degrees.entry(b).or_default()+=1; }
    if degrees.values().any(|&n|n!=2) || edges.iter().any(|e|g.edge_faces[e].is_empty()) {
        return crate::modeling_workflow::bridge_open(mesh,&edges,segments,twist,smooth);
    }
    let directed: Vec<_> = edges
        .iter()
        .map(|&e| directed(mesh, &g, g.edge_faces[&e][0], e))
        .collect();
    let rings = loops(&directed)?;
    if rings.len() != 2 {
        return Err(invalid("Нужны два отдельные замкнутые контура"));
    }
    let mut output = faces(mesh);
    crate::advanced_bridge::append(mesh, &rings, segments, twist, smooth, &mut output)?;
    finish(mesh, output, mesh.smooth_normals_enabled())
}

pub(crate) fn bridge_faces(mesh: &mut Mesh, selection: &BTreeSet<usize>, segments: u32) -> Result<(), CoreError> {
    bridge_faces_options(mesh,selection,segments,0,0.)
}
pub(crate) fn bridge_faces_options(mesh:&mut Mesh,selection:&BTreeSet<usize>,segments:u32,twist:i32,smooth:f32)->Result<(),CoreError> {
    if selection.len()<2 || selection.iter().any(|&f|f>=mesh.face_count()) || !(1..=32).contains(&segments) {
        return Err(invalid("Выберите две области граней; сегменты моста: 1–32"));
    }
    let g=MeshConnectivity::new(mesh);
    let mut remaining=selection.clone(); let mut groups=Vec::new();
    while let Some(&seed)=remaining.first() {
        let mut group=BTreeSet::from([seed]); let mut pending=vec![seed]; remaining.remove(&seed);
        while let Some(f)=pending.pop() { for &next in &g.face_neighbors[f] {
            if remaining.remove(&next) {group.insert(next); pending.push(next);}
        } }
        groups.push(group);
    }
    if groups.len()!=2 { return Err(invalid("Мост требует две отдельные области граней")); }
    let mut rings=Vec::new();
    for group in groups {
        let mut boundary=Vec::new();
        for f in group.iter().copied() {
            let face=mesh.face_vertices(f).unwrap();
            for i in 0..face.len() {
                let a=g.canonical[face[i] as usize]; let b=g.canonical[face[(i+1)%face.len()] as usize];
                if g.edge_faces[&(a.min(b),a.max(b))].iter().filter(|f|group.contains(f)).count()==1 {
                    // The cap is removed, so its adjacent surface runs in the reverse direction.
                    boundary.push((b,a));
                }
            }
        }
        let mut outlines=loops(&boundary)?;
        if outlines.len()!=1 {return Err(invalid("Область моста должна иметь один внешний контур без отверстий"));}
        rings.push(outlines.remove(0));
    }
    if rings[0].iter().any(|v|rings[1].contains(v)) {
        return Err(invalid("Для моста нужны отдельные контуры без общих вершин"));
    }
    let mut output:Vec<_>=faces(mesh).into_iter().enumerate().filter_map(|(f,face)|(!selection.contains(&f)).then_some(face)).collect();
    let mut materials:Vec<_>=(0..mesh.face_count()).filter(|f|!selection.contains(f)).map(|f|mesh.face_material(f)).collect();
    let material=mesh.face_material(*selection.first().unwrap()); let count=output.len();
    crate::advanced_bridge::append(mesh,&rings,segments,twist,smooth,&mut output)?;
    materials.extend(std::iter::repeat_n(material,output.len()-count));
    crate::component_mesh::ensure_topology(mesh);
    let t=mesh.topology.as_mut().unwrap();t.faces=output.clone().into();t.face_materials=materials.into();
    finish(mesh,output,mesh.smooth_normals_enabled())
}

pub(crate) fn dissolve(
    mesh: &mut Mesh,
    selection: &BTreeSet<(usize, usize)>,
) -> Result<(), CoreError> {
    let g = MeshConnectivity::new(mesh);
    let edges: BTreeSet<_> = selected_edges(mesh, selection, &g)?.into_iter()
        .filter(|e| g.edge_faces[e].len() == 2).collect();
    if edges.is_empty() {
        return Err(invalid("Растворяются внутренние рёбра между двумя гранями"));
    }
    let mut adjacency = BTreeMap::<usize, Vec<usize>>::new();
    for edge in edges {
        let f = &g.edge_faces[&edge];
        adjacency.entry(f[0]).or_default().push(f[1]);
        adjacency.entry(f[1]).or_default().push(f[0]);
    }
    let mut seen = BTreeSet::<usize>::new();
    let mut output = Vec::new();
    let mut attributes = Vec::new();
    for start in 0..mesh.face_count() {
        if seen.contains(&start) {
            continue;
        }
        if !adjacency.contains_key(&start) {
            attributes.push(mesh.face_material(start));
            output.push(mesh.face_vertices(start).unwrap().to_vec());
            continue;
        }
        let mut group = BTreeSet::from([start]);
        let mut queue = vec![start];
        while let Some(f) = queue.pop() {
            for &n in &adjacency[&f] {
                if group.insert(n) {
                    queue.push(n);
                }
            }
        }
        if group
            .iter()
            .any(|&f| mesh.face_material(f) != mesh.face_material(start))
        {
            return Err(invalid(
                "У граней разные материалы: сохраните разделяющее ребро",
            ));
        }
        seen.extend(&group);
        let mut boundary = Vec::new();
        let mut corners = BTreeMap::new();
        for &f in &group {
            let face = mesh.face_vertices(f).unwrap();
            for j in 0..face.len() {
                let a = g.canonical[face[j] as usize];
                let b = g.canonical[face[(j + 1) % face.len()] as usize];
                let edge = (a.min(b), a.max(b));
                if g.edge_faces[&edge]
                    .iter()
                    .filter(|f| group.contains(f))
                    .count()
                    == 1
                {
                    boundary.push((a, b));
                    corners.entry(a).or_insert(face[j]);
                }
            }
        }
        let loops = loops(&boundary)?;
        if loops.len() != 1 {
            return Err(invalid(
                "Растворение создаёт отверстие; выберите меньшую область",
            ));
        }
        attributes.push(mesh.face_material(start));
        output.push(loops[0].iter().map(|v| corners[v]).collect());
    }
    if let Some(t) = &mut mesh.topology {
        if !t.face_materials.is_empty() {
            t.face_materials = attributes.into();
            t.faces = output.clone().into();
        }
    }
    finish(mesh, output, mesh.smooth_normals_enabled())
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopologyReport {
    pub boundary_edges: usize,
    pub nonmanifold_edges: usize,
    pub winding_edges: usize,
    pub isolated_vertices: usize,
    pub duplicate_faces: usize,
    pub degenerate_faces: usize,
    pub problem_faces: BTreeSet<usize>,
    pub duplicate_vertex_ids:BTreeSet<usize>,
    pub boundary_edge_ids:BTreeSet<(usize,usize)>,
    pub nonmanifold_edge_ids:BTreeSet<(usize,usize)>,
    pub winding_face_ids:BTreeSet<usize>,
    pub degenerate_face_ids:BTreeSet<usize>,
    pub duplicate_face_ids:BTreeSet<usize>,
    pub inverted_face_ids:BTreeSet<usize>,
}
pub fn inspect_topology(mesh: &Mesh) -> TopologyReport {
    let g = MeshConnectivity::new(mesh);
    let mut r = TopologyReport::default();
    let mut used = BTreeSet::new();
    let mut seen = BTreeMap::new();
    for (f, face) in faces(mesh).iter().enumerate() {
        used.extend(face.iter().map(|&v| v as usize));
        let mut key: Vec<_> = face.iter().map(|&v| g.canonical[v as usize]).collect();
        key.sort_unstable();
        if let Some(old) = seen.insert(key, f) {
            r.duplicate_faces += 1;
            r.problem_faces.extend([old, f]);
            r.duplicate_face_ids.extend([old,f]);
        }
        let n = crate::polygon_edit::face_normal(mesh, face);
        if n.length_squared() < 1e-20 || !n.is_finite() {
            r.degenerate_faces += 1;
            r.degenerate_face_ids.insert(f);
            r.problem_faces.insert(f);
        }
    }
    if let Some(t)=&mesh.topology {used.extend(t.loose_edges.iter().flatten().map(|&v|v as usize));}
    r.isolated_vertices = mesh.vertices.len() - used.len();
    r.duplicate_vertex_ids=crate::mesh_cleanup::duplicate_points(mesh);
    r.inverted_face_ids=crate::mesh_cleanup::inverted_faces(mesh,&g);
    r.problem_faces.extend(&r.inverted_face_ids);
    for (&e, fs) in &g.edge_faces {
        if fs.len() == 1 {
            r.boundary_edges += 1;
            r.boundary_edge_ids.insert(e);
        } else if fs.len() > 2 {
            r.nonmanifold_edges += 1;
            r.nonmanifold_edge_ids.insert(e);
            r.problem_faces.extend(fs);
        } else if fs.len()==2 && directed(mesh, &g, fs[0], e) == directed(mesh, &g, fs[1], e) {
            r.winding_edges += 1;
            r.winding_face_ids.extend(fs);
            r.problem_faces.extend(fs);
        }
    }
    r
}
/// Ring follows opposite quad edges; loop follows the unique nonadjacent edge
/// at a regular vertex, or the next edge of an unbranched open boundary.
pub fn select_edge_path(
    mesh: &Mesh,
    seeds: &BTreeSet<(usize, usize)>,
    ring: bool,
) -> Result<BTreeSet<(usize, usize)>, CoreError> {
    let g = MeshConnectivity::new(mesh);
    let mut selected = selected_edges(mesh, seeds, &g)?;
    let mut queue: VecDeque<_> = selected.iter().copied().collect();
    let mut incident = vec![Vec::new(); mesh.vertices.len()];
    for &e in &g.edges {
        incident[e.0].push(e);
        incident[e.1].push(e);
    }
    while let Some(e) = queue.pop_front() {
        let mut candidates = Vec::new();
        if ring {
            for &f in &g.edge_faces[&e] {
                let face = mesh.face_vertices(f).unwrap();
                if face.len() != 4 {
                    continue;
                }
                for i in 0..4 {
                    let a = g.canonical[face[i] as usize];
                    let b = g.canonical[face[(i + 1) % 4] as usize];
                    if (a.min(b), a.max(b)) == e {
                        let c = g.canonical[face[(i + 2) % 4] as usize];
                        let d = g.canonical[face[(i + 3) % 4] as usize];
                        candidates.push((c.min(d), c.max(d)));
                    }
                }
            }
        } else {
            for v in [e.0, e.1] {
                if g.edge_faces[&e].len() == 1 {
                    let boundary: Vec<_> = incident[v]
                        .iter()
                        .copied()
                        .filter(|x| g.edge_faces[x].len() == 1)
                        .collect();
                    if boundary.len() == 2 {
                        candidates.extend(boundary.into_iter().filter(|x| *x != e));
                    }
                    continue;
                }
                if incident[v].len() != 4 || g.edge_faces[&e].len() != 2 {
                    continue;
                }
                let next: Vec<_> = incident[v]
                    .iter()
                    .copied()
                    .filter(|x| {
                        *x != e
                            && g.edge_faces[x]
                                .iter()
                                .all(|f| !g.edge_faces[&e].contains(f))
                    })
                    .collect();
                if next.len() == 1 {
                    candidates.extend(next);
                }
            }
        }
        for next in candidates {
            if selected.insert(next) {
                queue.push_back(next);
            }
        }
    }
    Ok(selected)
}
