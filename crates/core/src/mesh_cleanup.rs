//! Selection-scoped surface construction and explicit, undoable repair policies.
use crate::{
    CoreError, Editor, Mesh, MeshConnectivity, MeshEdit, NodeId, Vec3, Vertex, MAX_EDIT_TRIANGLES,
    MAX_EDIT_VERTICES,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
type Edge = (usize, usize);
fn edge(a: usize, b: usize) -> Edge {
    (a.min(b), a.max(b))
}
fn invalid(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
// Existing files keep their legacy quad diagonal. Newly authored caps and
// explicit triangulation choose the interior diagonal of a concave quad.
fn polygon(mesh: &Mesh, mut face: Vec<u32>) -> Result<Vec<u32>, CoreError> {
    if face.len() == 4 {
        let n = crate::polygon_edit::face_normal(mesh, &face).normalize_or_zero();
        let fits = |f: &[u32]| {
            let p = |i: usize| Vec3::from_array(mesh.vertices[f[i] as usize].position);
            (p(1) - p(0)).cross(p(2) - p(0)).dot(n) > 1e-14
                && (p(2) - p(0)).cross(p(3) - p(0)).dot(n) > 1e-14
        };
        if !fits(&face) {
            face.rotate_left(1);
            if !fits(&face) {
                return Err(invalid("Контур схлопывается или пересекает себя"));
            }
        }
    }
    crate::triangulate::triangulate(mesh, &face)?;
    Ok(face)
}

#[derive(Clone, Debug, Default)]
pub struct MeshRegion {
    pub vertices: BTreeSet<usize>,
    pub edges: BTreeSet<Edge>,
    pub faces: BTreeSet<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairKind {
    Duplicates,
    Holes,
    Winding,
    Degenerate,
    NonManifold,
}
impl RepairKind {
    pub const ALL: [Self; 5] = [
        Self::Duplicates,
        Self::Holes,
        Self::Winding,
        Self::Degenerate,
        Self::NonManifold,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Duplicates => "Дубли точек и граней",
            Self::Holes => "Отверстия",
            Self::Winding => "Направление граней",
            Self::Degenerate => "Вырожденные грани",
            Self::NonManifold => "Лишние грани у рёбер",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum TopologyEdit {
    FillHoles,
    Triangulate,
    Repair(RepairKind),
}
fn signature(v: &Vertex) -> Vec<u32> {
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
pub(crate) fn duplicate_points(mesh: &Mesh) -> BTreeSet<usize> {
    let mut seen = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for (i, v) in mesh.vertices.iter().enumerate() {
        if let Some(&old) = seen.get(&signature(v)) {
            duplicates.extend([old, i]);
        } else {
            seen.insert(signature(v), i);
        }
    }
    duplicates
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
fn scoped_vertices(mesh: &Mesh, region: &MeshRegion) -> BTreeSet<usize> {
    let mut vertices = region.vertices.clone();
    vertices.extend(region.edges.iter().flat_map(|&(a, b)| [a, b]));
    for &f in &region.faces {
        if let Some(face) = mesh.face_vertices(f) {
            vertices.extend(face.iter().map(|&v| v as usize));
        }
    }
    vertices
}
fn selected_faces(mesh: &Mesh, region: &MeshRegion) -> BTreeSet<usize> {
    if !region.faces.is_empty() {
        return region.faces.clone();
    }
    let g = MeshConnectivity::new(mesh);
    let vs: BTreeSet<_> = scoped_vertices(mesh, region)
        .iter()
        .map(|&v| g.canonical[v])
        .collect();
    if vs.is_empty() {
        (0..mesh.face_count()).collect()
    } else {
        (0..mesh.face_count())
            .filter(|&f| {
                mesh.face_vertices(f)
                    .unwrap()
                    .iter()
                    .any(|&v| vs.contains(&g.canonical[v as usize]))
            })
            .collect()
    }
}
fn retain(mesh: &mut Mesh, keep: impl Fn(usize) -> bool) {
    let t = mesh.topology.as_ref().unwrap();
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    for (i, f) in t.faces.iter().enumerate() {
        if keep(i) {
            faces.push(f.clone());
            materials.push(mesh.face_material(i));
        }
    }
    let t = mesh.topology.as_mut().unwrap();
    t.faces = faces.into();
    t.face_materials = materials.into();
}
fn fill(mesh: &mut Mesh, region: &MeshRegion) -> Result<(), CoreError> {
    let graph = MeshConnectivity::new(mesh);
    let vertices: BTreeSet<_> = scoped_vertices(mesh, region)
        .iter()
        .map(|&v| graph.canonical[v])
        .collect();
    let selected: BTreeSet<_> = region
        .edges
        .iter()
        .map(|&(a, b)| edge(graph.canonical[a], graph.canonical[b]))
        .collect();
    let mut boundaries = Vec::new();
    let mut corners = BTreeMap::new();
    for (&e, fs) in &graph.edge_faces {
        if fs.len() != 1 {
            continue;
        }
        let face = mesh.face_vertices(fs[0]).unwrap();
        for i in 0..face.len() {
            let a = graph.canonical[face[i] as usize];
            let b = graph.canonical[face[(i + 1) % face.len()] as usize];
            if edge(a, b) == e {
                boundaries.push((b, a));
                corners.insert(a, face[i]);
                corners.insert(b, face[(i + 1) % face.len()]);
                break;
            }
        }
    }
    if !vertices.is_empty() || !selected.is_empty() {
        let mut adjacency=BTreeMap::<usize,Vec<usize>>::new();
        for &(a,b) in &boundaries {adjacency.entry(a).or_default().push(b);adjacency.entry(b).or_default().push(a);}
        let mut reached=vertices.clone();reached.extend(selected.iter().flat_map(|&(a,b)|[a,b]));
        let mut queue:VecDeque<_>=reached.iter().copied().collect();
        while let Some(v)=queue.pop_front(){if let Some(next)=adjacency.get(&v){for &n in next {if reached.insert(n){queue.push_back(n);}}}}
        boundaries.retain(|&(a,b)|reached.contains(&a)&&reached.contains(&b));
    }
    if boundaries.is_empty() {
        return Err(invalid("Выберите замкнутую открытую границу"));
    }
    let loops = crate::mesh_topology_tools::loops(&boundaries)?;
    let material = boundaries
        .first()
        .and_then(|&(a, b)| graph.edge_faces.get(&edge(a, b)))
        .and_then(|fs| fs.first())
        .and_then(|&f| mesh.face_material(f));
    let has_materials = !mesh.topology.as_ref().unwrap().face_materials.is_empty();
    let mut filled = 0;
    for ring in loops {
        if (!vertices.is_empty() || !selected.is_empty()) && !ring.iter().any(|v| vertices.contains(v))
            && !(0..ring.len()).any(|i| selected.contains(&edge(ring[i],ring[(i+1)%ring.len()]))) { continue; }
        filled += 1;
        let face = polygon(mesh, ring.iter().map(|v| corners[v]).collect())?;
        let t = mesh.topology.as_mut().unwrap();
        t.faces.push(face);
        if has_materials {
            t.face_materials.push(material);
        }
    }
    if filled == 0 { return Err(invalid("Выберите вершину, ребро или грань у отверстия")); }
    Ok(())
}
fn direction(mesh: &Mesh, g: &MeshConnectivity, f: usize, e: Edge) -> bool {
    let face = mesh.face_vertices(f).unwrap();
    (0..face.len()).any(|i| {
        g.canonical[face[i] as usize] == e.0
            && g.canonical[face[(i + 1) % face.len()] as usize] == e.1
    })
}
pub(crate) fn components(mesh: &Mesh, g: &MeshConnectivity) -> Vec<BTreeSet<usize>> {
    let mut remaining: BTreeSet<_> = (0..mesh.face_count()).collect();
    let mut output = Vec::new();
    while let Some(&seed) = remaining.first() {
        remaining.remove(&seed);
        let mut group = BTreeSet::from([seed]);
        let mut queue = VecDeque::from([seed]);
        while let Some(f) = queue.pop_front() {
            for &n in &g.face_neighbors[f] {
                if remaining.remove(&n) {
                    group.insert(n);
                    queue.push_back(n);
                }
            }
        }
        output.push(group);
    }
    output
}
fn closed(mesh: &Mesh, g: &MeshConnectivity, group: &BTreeSet<usize>) -> bool {
    group.iter().all(|&f| {
        let face = mesh.face_vertices(f).unwrap();
        (0..face.len()).all(|i| {
            g.edge_faces
                .get(&edge(
                    g.canonical[face[i] as usize],
                    g.canonical[face[(i + 1) % face.len()] as usize],
                ))
                .is_some_and(|fs| fs.len() == 2)
        })
    })
}
fn volume(mesh: &Mesh, group: &BTreeSet<usize>) -> f64 {
    group
        .iter()
        .map(|&f| {
            let face = mesh.face_vertices(f).unwrap();
            let p = |i| Vec3::from_array(mesh.vertices[face[i] as usize].position).as_dvec3();
            (1..face.len() - 1)
                .map(|i| p(0).dot(p(i).cross(p(i + 1))) / 6.)
                .sum::<f64>()
        })
        .sum()
}
pub(crate) fn inverted_faces(mesh: &Mesh, g: &MeshConnectivity) -> BTreeSet<usize> {
    components(mesh, g)
        .into_iter()
        .filter(|c| closed(mesh, g, c) && volume(mesh, c) < -1e-12)
        .flatten()
        .collect()
}
fn orient(mesh: &mut Mesh, selection: &BTreeSet<usize>) -> Result<(), CoreError> {
    let graph = MeshConnectivity::new(mesh);
    for group in components(mesh, &graph)
        .into_iter()
        .filter(|c| !c.is_disjoint(selection))
    {
        let root = *group
            .difference(selection)
            .next()
            .or_else(|| group.first())
            .unwrap();
        let mut flips = BTreeMap::from([(root, false)]);
        let mut queue = VecDeque::from([root]);
        while let Some(f) = queue.pop_front() {
            let face = mesh.face_vertices(f).unwrap();
            for i in 0..face.len() {
                let e = edge(
                    graph.canonical[face[i] as usize],
                    graph.canonical[face[(i + 1) % face.len()] as usize],
                );
                let Some(fs) = graph.edge_faces.get(&e) else {
                    continue;
                };
                if fs.len() != 2 {
                    continue;
                }
                let next = if fs[0] == f { fs[1] } else { fs[0] };
                let flip =
                    flips[&f] ^ (direction(mesh, &graph, f, e) == direction(mesh, &graph, next, e));
                if let Some(&old) = flips.get(&next) {
                    if old != flip {
                        return Err(invalid(
                            "Противоречивая поверхность: сначала разделите проблемные рёбра",
                        ));
                    }
                } else {
                    flips.insert(next, flip);
                    queue.push_back(next);
                }
            }
        }
        if flips
            .iter()
            .any(|(f, flip)| *flip && !selection.contains(f))
        {
            return Err(invalid(
                "Для согласования нормалей выделите всю связанную область",
            ));
        }
        for (&f, &flip) in &flips {
            if flip {
                mesh.topology.as_mut().unwrap().faces[f].reverse();
            }
        }
        if group.is_subset(selection) && closed(mesh, &graph, &group) && volume(mesh, &group) < 0. {
            for &f in &group {
                mesh.topology.as_mut().unwrap().faces[f].reverse();
            }
        }
    }
    Ok(())
}
pub(crate) fn apply(
    mesh: &mut Mesh,
    region: &MeshRegion,
    edit: TopologyEdit,
) -> Result<(), CoreError> {
    if region.vertices.iter().any(|&v| v >= mesh.vertices.len())
        || region
            .edges
            .iter()
            .any(|&(a, b)| a >= mesh.vertices.len() || b >= mesh.vertices.len())
        || region.faces.iter().any(|&f| f >= mesh.face_count())
    {
        return Err(invalid("Выбор компонентов устарел"));
    }
    let original = mesh.clone();
    crate::component_mesh::ensure_topology(mesh);
    let selected = selected_faces(mesh, region);
    match edit {
        TopologyEdit::FillHoles | TopologyEdit::Repair(RepairKind::Holes) => fill(mesh, region)?,
        TopologyEdit::Triangulate => {
            if region.faces.is_empty() {
                return Err(invalid("Выберите грани для триангуляции"));
            }
            let mut faces = Vec::new();
            let mut materials = Vec::new();
            for f in 0..mesh.face_count() {
                let face = mesh.face_vertices(f).unwrap();
                if selected.contains(&f) {
                    let face = polygon(mesh, face.to_vec())?;
                    for tri in crate::triangulate::triangulate(mesh, &face)?.chunks_exact(3) {
                        faces.push(tri.to_vec());
                        materials.push(mesh.face_material(f));
                    }
                } else {
                    faces.push(face.to_vec());
                    materials.push(mesh.face_material(f));
                }
            }
            let t = mesh.topology.as_mut().unwrap();
            t.faces = faces.into();
            t.face_materials = materials.into();
        }
        TopologyEdit::Repair(RepairKind::Winding) => orient(mesh, &selected)?,
        TopologyEdit::Repair(RepairKind::Degenerate) => {
            let bad: BTreeSet<_> = selected
                .iter()
                .copied()
                .filter(|&f| {
                    crate::polygon_edit::face_normal(mesh, mesh.face_vertices(f).unwrap())
                        .length_squared()
                        < 1e-18
                })
                .collect();
            retain(mesh, |f| !bad.contains(&f));
        }
        TopologyEdit::Repair(RepairKind::NonManifold) => {
            let graph = MeshConnectivity::new(mesh);
            let mut remove = BTreeSet::new();
            for fs in graph
                .edge_faces
                .values()
                .filter(|fs| fs.len() > 2 && fs.iter().any(|f| selected.contains(f)))
            {
                let mut remaining: Vec<_> =
                    fs.iter().copied().filter(|f| !remove.contains(f)).collect();
                remaining.sort_by_key(|f| selected.contains(f));
                if remaining.iter().filter(|f| !selected.contains(f)).count() > 2 {
                    return Err(invalid("Выделите лишние грани у проблемного ребра"));
                }
                remove.extend(remaining.into_iter().skip(2));
            }
            retain(mesh, |f| !remove.contains(&f));
        }
        TopologyEdit::Repair(RepairKind::Duplicates) => {
            let mut allowed = scoped_vertices(mesh, region);
            if allowed.is_empty() {
                allowed.extend(0..mesh.vertices.len());
            }
            let mut seen = BTreeMap::new();
            let mut remap: BTreeMap<_, _> = (0..mesh.vertices.len())
                .map(|i| (i as u32, i as u32))
                .collect();
            for v in allowed {
                let key = signature(&mesh.vertices[v]);
                let id = *seen.entry(key).or_insert(v as u32);
                remap.insert(v as u32, id);
            }
            let t = mesh.topology.as_mut().unwrap();
            for f in &mut t.faces {
                for v in f {
                    *v = remap[v];
                }
            }
            for e in &mut t.loose_edges {
                *e = e.map(|v| remap[&v]);
            }
            t.loose_edges.retain(|e| e[0] != e[1]);
            let graph = MeshConnectivity::new(mesh);
            let mut seen = BTreeMap::new();
            let mut remove = BTreeSet::new();
            for f in 0..mesh.face_count() {
                let face = mesh.face_vertices(f).unwrap();
                let mut key: Vec<_> = face.iter().map(|&v| graph.canonical[v as usize]).collect();
                key.sort_unstable();
                if let Some(&old) = seen.get(&key) {
                    if selected.contains(&f) {
                        remove.insert(f);
                    } else if selected.contains(&old) {
                        remove.insert(old);
                        seen.insert(key, f);
                    }
                } else {
                    seen.insert(key, f);
                }
            }
            retain(mesh, |f| !remove.contains(&f));
            // Remove only merged duplicate records; preserve isolated authored points.
            let mut vertices = Vec::new();
            let mut compact = BTreeMap::new();
            for (i, v) in mesh.vertices.iter().enumerate() {
                if remap[&(i as u32)] == i as u32 {
                    compact.insert(i as u32, vertices.len() as u32);
                    vertices.push(*v);
                }
            }
            let t = mesh.topology.as_mut().unwrap();
            for f in &mut t.faces {
                for v in f {
                    *v = compact[v];
                }
            }
            for e in &mut t.loose_edges {
                *e = e.map(|v| compact[&v]);
            }
            t.uv_seams = t
                .uv_seams
                .iter()
                .filter_map(|e| {
                    let p = e.map(|v| compact[&remap[&v]]);
                    (p[0] != p[1]).then_some(p)
                })
                .collect();
            mesh.vertices = vertices.into();
        }
    }
    if *mesh == original {
        return Ok(());
    }
    finish(mesh)
}

impl Editor {
    pub fn separate_faces(
        &mut self,
        id: NodeId,
        faces: &BTreeSet<usize>,
    ) -> Result<NodeId, CoreError> {
        let source = self.editable_mesh(id)?.clone();
        if faces.is_empty() || faces.iter().any(|&f| f >= source.face_count()) {
            return Err(invalid("Выберите грани для отделения"));
        }
        let mut part = source.clone();
        crate::component_mesh::ensure_topology(&mut part);
        retain(&mut part, |f| faces.contains(&f));
        part.topology.as_mut().unwrap().loose_edges.clear();
        let mut map = BTreeMap::new();
        let mut vertices = Vec::new();
        for face in &mut part.topology.as_mut().unwrap().faces {
            for v in face {
                *v = *map.entry(*v).or_insert_with(|| {
                    let id = vertices.len() as u32;
                    vertices.push(source.vertices[*v as usize]);
                    id
                });
            }
        }
        part.topology.as_mut().unwrap().uv_seams =
            source.topology.as_ref().map_or_else(Vec::new, |t| {
                t.uv_seams
                    .iter()
                    .filter_map(|e| Some([*map.get(&e[0])?, *map.get(&e[1])?]))
                    .collect()
            });
        part.vertices = vertices.into();
        finish(&mut part)?;
        let mut remainder = source.clone();
        crate::authoring::apply_mesh_edit(
            &mut remainder,
            MeshEdit::DeleteComponents {
                vertices: Default::default(),
                edges: Default::default(),
                faces: faces.clone(),
            },
        )?;
        let mut node = self.project.node(id).unwrap().clone();
        let next = self.project.next_id();
        let new_node = next + 1;
        part.id = next;
        part.name = format!("{} · часть", source.name);
        node.id = new_node;
        node.name = format!("{} · часть", node.name);
        node.mesh = Some(part.id);
        if self
            .project
            .nodes
            .iter()
            .any(|n| n.id != id && n.mesh == Some(source.id))
        {
            remainder.id = next + 2;
        }
        self.transact("Отделить выбранные грани", move |p| {
            p.node_mut(id).unwrap().mesh = Some(remainder.id);
            if let Some(m) = p.meshes.iter_mut().find(|m| m.id == remainder.id) {
                *m = remainder;
            } else {
                p.meshes.push(remainder);
            }
            p.meshes.push(part);
            p.nodes.push(node);
        })?;
        Ok(new_node)
    }
}
