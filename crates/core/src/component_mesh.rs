//! Selection-scoped topology edits, including editable wire geometry.
use crate::{
    CoreError, Mesh, MeshConnectivity, MeshTopology, Vec3, Vertex, MAX_EDIT_TRIANGLES,
    MAX_EDIT_VERTICES,
};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);
fn edge(a: usize, b: usize) -> Edge {
    (a.min(b), a.max(b))
}
fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
pub(crate) fn ensure_topology(mesh: &mut Mesh) {
    if mesh.topology.is_none() {
        mesh.topology = Some(MeshTopology {
            faces: mesh.indices.chunks_exact(3).map(|f| f.to_vec()).collect(),
            face_materials: Default::default(),
            loose_edges: vec![],
            uv_seams: vec![],
            smooth_normals: false,
            primitive: None,
        });
    }
}
fn selection(
    mesh: &Mesh,
    vertices: &BTreeSet<usize>,
    edges: &BTreeSet<Edge>,
    faces: &BTreeSet<usize>,
) -> Result<(MeshConnectivity, BTreeSet<usize>, BTreeSet<Edge>), CoreError> {
    if vertices.iter().any(|&v| v >= mesh.vertices.len())
        || edges
            .iter()
            .any(|&(a, b)| a >= mesh.vertices.len() || b >= mesh.vertices.len())
        || faces.iter().any(|&f| f >= mesh.face_count())
    {
        return Err(invalid("Выбор компонентов устарел"));
    }
    let g = MeshConnectivity::new(mesh);
    let vertices = vertices.iter().map(|&v| g.canonical[v]).collect();
    let edges: BTreeSet<_> = edges
        .iter()
        .map(|&(a, b)| edge(g.canonical[a], g.canonical[b]))
        .collect();
    if !edges.is_subset(&g.edges) {
        return Err(invalid("Выбрано несуществующее ребро"));
    }
    Ok((g, vertices, edges))
}
pub(crate) fn interpolate(a: Vertex, b: Vertex, t: f32) -> Vertex {
    Vertex {
        position: Vec3::from_array(a.position)
            .lerp(Vec3::from_array(b.position), t)
            .to_array(),
        normal: Vec3::from_array(a.normal)
            .lerp(Vec3::from_array(b.normal), t)
            .try_normalize()
            .unwrap_or(Vec3::Y)
            .to_array(),
        uv: std::array::from_fn(|i| a.uv[i] + (b.uv[i] - a.uv[i]) * t),
        color: std::array::from_fn(|i| (a.color[i] + (b.color[i] - a.color[i]) * t).clamp(0., 1.)),
        ..a
    }
}
fn finish(mesh: &mut Mesh) -> Result<(), CoreError> {
    mesh.topology.as_mut().unwrap().primitive = None;
    mesh.rebuild_polygon_indices()?;
    if mesh.vertices.len() > MAX_EDIT_VERTICES || mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES {
        return Err(invalid("Операция превышает предел редактируемой сетки"));
    }
    crate::recompute_normals(mesh, mesh.smooth_normals_enabled());
    mesh.validate_topology()
}

pub(crate) fn extrude_vertices(
    mesh: &mut Mesh,
    vertices: &BTreeSet<usize>,
    distance: f32,
) -> Result<(), CoreError> {
    let (g, selected, _) = selection(mesh, vertices, &BTreeSet::new(), &BTreeSet::new())?;
    if selected.is_empty() || !distance.is_finite() || distance.abs() > 1000. {
        return Err(invalid("Выберите точки; расстояние от −1000 до 1000 м"));
    }
    if distance == 0. {
        return Ok(());
    }
    if mesh.vertices.len() + selected.len() > MAX_EDIT_VERTICES {
        return Err(invalid("Слишком много вершин для экструзии"));
    }
    ensure_topology(mesh);
    let mut normals = BTreeMap::<usize, Vec3>::new();
    for f in 0..mesh.face_count() {
        let face = mesh.face_vertices(f).unwrap();
        let normal = crate::polygon_edit::face_normal(mesh, face);
        for &v in face {
            *normals.entry(g.canonical[v as usize]).or_default() += normal;
        }
    }
    let mut tips = BTreeMap::new();
    for &v in &selected {
        let mut vertex = mesh.vertices[v];
        let p = Vec3::from_array(vertex.position);
        let direction = normals
            .get(&v)
            .copied()
            .unwrap_or_default()
            .try_normalize()
            .or_else(|| {
                g.vertex_neighbors[v]
                    .iter()
                    .map(|&n| p - Vec3::from_array(mesh.vertices[n].position))
                    .sum::<Vec3>()
                    .try_normalize()
            })
            .unwrap_or(Vec3::Y);
        vertex.position = (p + direction * distance).to_array();
        if crate::position_key(vertex.position) == crate::position_key(p.to_array()) {
            return Err(invalid("Расстояние меньше точности сетки"));
        }
        tips.insert(v, mesh.vertices.len() as u32);
        mesh.vertices.push(vertex);
    }
    let selected_edges: Vec<_> = g
        .edges
        .iter()
        .copied()
        .filter(|(a, b)| selected.contains(a) && selected.contains(b))
        .collect();
    let linked: BTreeSet<_> = selected_edges.iter().flat_map(|&(a, b)| [a, b]).collect();
    let mut new_faces = Vec::new();
    let mut materials = Vec::new();
    for (a, b) in selected_edges {
        let incident = g.edge_faces[&(a, b)].first().copied();
        let (a, b) = incident
            .and_then(|f| {
                let face = mesh.face_vertices(f)?;
                (0..face.len()).find_map(|i| {
                    let x = g.canonical[face[i] as usize];
                    let y = g.canonical[face[(i + 1) % face.len()] as usize];
                    (edge(x, y) == (a, b)).then_some((y, x))
                })
            })
            .unwrap_or((a, b));
        new_faces.push(vec![a as u32, b as u32, tips[&b], tips[&a]]);
        materials.push(incident.and_then(|f| mesh.face_material(f)));
    }
    let topology = mesh.topology.as_mut().unwrap();
    for &v in selected.difference(&linked) {
        topology.loose_edges.push([v as u32, tips[&v]]);
    }
    if !topology.face_materials.is_empty() {
        topology.face_materials.extend(materials);
    }
    topology.faces.extend(new_faces);
    finish(mesh)
}

pub(crate) fn delete(
    mesh: &mut Mesh,
    vertices: &BTreeSet<usize>,
    edges: &BTreeSet<Edge>,
    faces: &BTreeSet<usize>,
) -> Result<(), CoreError> {
    let (g, selected, edges) = selection(mesh, vertices, edges, faces)?;
    if selected.is_empty() && edges.is_empty() && faces.is_empty() {
        return Err(invalid("Выберите компоненты для удаления"));
    }
    let source = crate::mesh_topology_tools::faces(mesh);
    let removed: BTreeSet<_> = source
        .iter()
        .enumerate()
        .filter_map(|(f, face)| {
            (faces.contains(&f)
                || face
                    .iter()
                    .any(|&v| selected.contains(&g.canonical[v as usize]))
                || (0..face.len()).any(|i| {
                    edges.contains(&edge(
                        g.canonical[face[i] as usize],
                        g.canonical[face[(i + 1) % face.len()] as usize],
                    ))
                }))
            .then_some(f)
        })
        .collect();
    let mut output = Vec::new();
    let mut materials = Vec::new();
    let mut surface_edges = BTreeSet::new();
    for (f, face) in source
        .into_iter()
        .enumerate()
        .filter(|(f, _)| !removed.contains(f))
    {
        for i in 0..face.len() {
            surface_edges.insert(edge(
                g.canonical[face[i] as usize],
                g.canonical[face[(i + 1) % face.len()] as usize],
            ));
        }
        materials.push(mesh.face_material(f));
        output.push(face);
    }
    let wires: Vec<_> = g
        .edges
        .iter()
        .filter(|&&(a, b)| {
            !selected.contains(&a) && !selected.contains(&b) && !edges.contains(&(a, b))
        })
        .filter(|e| !surface_edges.contains(e))
        .filter(|e| !selected.is_empty() || !edges.is_empty() || g.edge_faces[e].is_empty())
        .map(|&(a, b)| [a as u32, b as u32])
        .collect();
    let mut used: BTreeSet<_> = output
        .iter()
        .flatten()
        .chain(wires.iter().flatten())
        .map(|&v| v as usize)
        .collect();
    if !selected.is_empty() || !edges.is_empty() {
        used.extend((0..mesh.vertices.len()).filter(|&v| !selected.contains(&g.canonical[v])));
    } else {
        used.extend(
            (0..mesh.vertices.len()).filter(|&v| g.vertex_neighbors[g.canonical[v]].is_empty()),
        );
    }
    let remap: BTreeMap<_, _> = used
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i as u32))
        .collect();
    let next = used.iter().map(|&v| mesh.vertices[v]).collect();
    for face in &mut output {
        for v in face {
            *v = remap[&(*v as usize)];
        }
    }
    let wires = wires
        .into_iter()
        .map(|e| e.map(|v| remap[&(v as usize)]))
        .collect();
    let smooth = mesh.smooth_normals_enabled();
    mesh.vertices = next;
    mesh.topology = Some(MeshTopology {
        faces: output.into(),
        face_materials: materials.into(),
        loose_edges: wires,
        uv_seams: vec![],
        smooth_normals: smooth,
        primitive: None,
    });
    finish(mesh)
}

pub(crate) fn subdivide(
    mesh: &mut Mesh,
    vertices: &BTreeSet<usize>,
    edges: &BTreeSet<Edge>,
    faces: &BTreeSet<usize>,
) -> Result<(), CoreError> {
    let (g, selected, mut cuts) = selection(mesh, vertices, edges, faces)?;
    let mut selected_faces = faces.clone();
    if !selected.is_empty() {
        cuts.extend(
            g.edges
                .iter()
                .copied()
                .filter(|(a, b)| selected.contains(a) && selected.contains(b)),
        );
        selected_faces.extend((0..mesh.face_count()).filter(|&f| {
            mesh.face_vertices(f)
                .unwrap()
                .iter()
                .all(|&v| selected.contains(&g.canonical[v as usize]))
        }));
    }
    let source = crate::mesh_topology_tools::faces(mesh);
    for &f in &selected_faces {
        let face = &source[f];
        for i in 0..face.len() {
            cuts.insert(edge(
                g.canonical[face[i] as usize],
                g.canonical[face[(i + 1) % face.len()] as usize],
            ));
        }
    }
    if cuts.is_empty() {
        return Err(invalid(
            "Выберите грани, рёбра или соседние точки для подразделения",
        ));
    }
    ensure_topology(mesh);
    let materials: Vec<_> = (0..source.len()).map(|f| mesh.face_material(f)).collect();
    let mut mids = BTreeMap::<(u32, u32), u32>::new();
    let mut midpoint = |mesh: &mut Mesh, a: u32, b: u32| -> u32 {
        *mids.entry((a.min(b), a.max(b))).or_insert_with(|| {
            let mut v = interpolate(mesh.vertices[a as usize], mesh.vertices[b as usize], 0.5);
            let e = edge(g.canonical[a as usize], g.canonical[b as usize]);
            v.position = interpolate(mesh.vertices[e.0], mesh.vertices[e.1], 0.5).position;
            let id = mesh.vertices.len() as u32;
            mesh.vertices.push(v);
            id
        })
    };
    let mut output = Vec::new();
    let mut attributes = Vec::new();
    for (f, face) in source.iter().enumerate() {
        let begin = output.len();
        if selected_faces.contains(&f) {
            let normal = crate::polygon_edit::face_normal(mesh, face);
            let p =
                |i: usize| Vec3::from_array(mesh.vertices[face[i % face.len()] as usize].position);
            let concave = (0..face.len())
                .any(|i| (p(i + 1) - p(i)).cross(p(i + 2) - p(i + 1)).dot(normal) < -1e-10);
            if concave {
                for tri in crate::triangulate::triangulate(mesh, face)?.chunks_exact(3) {
                    let mids = [
                        midpoint(mesh, tri[0], tri[1]),
                        midpoint(mesh, tri[1], tri[2]),
                        midpoint(mesh, tri[2], tri[0]),
                    ];
                    for i in 0..3 {
                        output.push(vec![tri[i], mids[i], mids[(i + 2) % 3]]);
                    }
                    output.push(mids.to_vec());
                }
                attributes.extend(std::iter::repeat_n(materials[f], output.len() - begin));
                continue;
            }
            let middle: Vec<_> = (0..face.len())
                .map(|i| midpoint(mesh, face[i], face[(i + 1) % face.len()]))
                .collect();
            if face.len() == 3 {
                for i in 0..3 {
                    output.push(vec![face[i], middle[i], middle[(i + 2) % 3]]);
                }
                output.push(middle);
            } else {
                let mut center = mesh.vertices[face[0] as usize];
                for (i, &v) in face.iter().enumerate().skip(1) {
                    center = interpolate(center, mesh.vertices[v as usize], 1. / (i + 1) as f32);
                }
                let c = mesh.vertices.len() as u32;
                mesh.vertices.push(center);
                for i in 0..face.len() {
                    output.push(vec![
                        face[i],
                        middle[i],
                        c,
                        middle[(i + face.len() - 1) % face.len()],
                    ]);
                }
            }
        } else {
            let mut expanded = Vec::new();
            for i in 0..face.len() {
                let a = face[i];
                let b = face[(i + 1) % face.len()];
                expanded.push(a);
                if cuts.contains(&edge(g.canonical[a as usize], g.canonical[b as usize])) {
                    expanded.push(midpoint(mesh, a, b));
                }
            }
            output.push(expanded);
        }
        attributes.extend(std::iter::repeat_n(materials[f], output.len() - begin));
    }
    let mut wires = Vec::new();
    for [a, b] in mesh.topology.as_ref().unwrap().loose_edges.clone() {
        if cuts.contains(&edge(g.canonical[a as usize], g.canonical[b as usize])) {
            let mid = midpoint(mesh, a, b);
            wires.extend([[a, mid], [mid, b]]);
        } else {
            wires.push([a, b]);
        }
    }
    let t = mesh.topology.as_mut().unwrap();
    t.faces = output.into();
    t.face_materials = attributes.into();
    t.loose_edges = wires;
    finish(mesh)
}
