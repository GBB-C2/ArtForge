//! Polygon-preserving geometry kernels. All coordinates are object-local.
use crate::modifiers::{cancelled, geometry_budget};
use crate::{CoreError, Mesh, MeshTopology, ModifierSettings, Vec3, Vertex};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};

fn error(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
fn position(mesh: &Mesh, index: usize) -> Vec3 {
    Vec3::from_array(mesh.vertices[index].position)
}
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}
fn topology(mesh: &Mesh) -> MeshTopology {
    MeshTopology {
        faces: (0..mesh.face_count())
            .map(|f| mesh.face_vertices(f).unwrap().to_vec())
            .collect(),
        face_materials: (0..mesh.face_count())
            .map(|f| mesh.face_material(f))
            .collect(),
        uv_seams: vec![],
        smooth_normals: mesh.smooth_normals_enabled(),
        primitive: None,
    }
}
fn finish(
    mut mesh: Mesh,
    groups: &[u64],
    cancel: &AtomicBool,
    normals: bool,
) -> Result<Mesh, CoreError> {
    cancelled(cancel)?;
    mesh.rebuild_polygon_indices()?;
    geometry_budget(mesh.vertices.len(), mesh.indices.len() / 3)?;
    if normals {
        let mut normals = vec![Vec3::ZERO; mesh.vertices.len()];
        for tri in mesh.indices.chunks_exact(3) {
            let a = tri[0] as usize;
            let b = tri[1] as usize;
            let c = tri[2] as usize;
            let n = (position(&mesh, b) - position(&mesh, a))
                .cross(position(&mesh, c) - position(&mesh, a));
            for i in [a, b, c] {
                normals[i] += n;
            }
        }
        if mesh.smooth_normals_enabled() {
            let mut sums = BTreeMap::<([i64; 3], u64), Vec3>::new();
            for (i, v) in mesh.vertices.iter().enumerate() {
                *sums
                    .entry((crate::position_key(v.position), groups[i]))
                    .or_default() += normals[i];
            }
            for (i, v) in mesh.vertices.iter().enumerate() {
                normals[i] = sums[&(crate::position_key(v.position), groups[i])];
            }
        }
        for (v, n) in mesh.vertices.iter_mut().zip(normals) {
            v.normal = n.try_normalize().unwrap_or(Vec3::Y).to_array();
        }
    }
    if mesh.vertices.iter().any(|v| {
        v.position
            .iter()
            .chain(&v.normal)
            .any(|x| !x.is_finite() || x.abs() > 1e8)
    }) {
        return Err(error("Геометрия выходит за допустимые координаты"));
    }
    cancelled(cancel)?;
    Ok(mesh)
}
pub(crate) fn apply(
    source: &Mesh,
    settings: ModifierSettings,
    groups: &mut Vec<u64>,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    settings.validate()?;
    cancelled(cancel)?;
    if source.indices.is_empty() || source.vertices.is_empty() {
        return Ok(source.clone());
    }
    if !source.indices.len().is_multiple_of(3)
        || source
            .indices
            .iter()
            .any(|&v| v as usize >= source.vertices.len())
    {
        return Err(error("Некорректные индексы исходного меша"));
    }
    match settings {
        ModifierSettings::Mirror { axis, plane } => copies(
            source,
            2,
            Some((axis as usize, plane)),
            Vec3::ZERO,
            groups,
            cancel,
        ),
        ModifierSettings::Array { count, offset } => copies(
            source,
            count as usize,
            None,
            Vec3::from_array(offset),
            groups,
            cancel,
        ),
        ModifierSettings::Subdivision { levels, smooth } => {
            let mut mesh = source.clone();
            for _ in 0..levels {
                mesh = subdivide(&mesh, smooth, groups, cancel)?;
            }
            Ok(mesh)
        }
        ModifierSettings::Solidify { thickness, offset } => {
            solidify(source, thickness, offset, groups, cancel)
        }
    }
}
fn copies(
    source: &Mesh,
    count: usize,
    mirror: Option<(usize, f32)>,
    offset: Vec3,
    groups: &mut Vec<u64>,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    if count == 1 {
        return Ok(source.clone());
    }
    geometry_budget(
        source.vertices.len().saturating_mul(count),
        (source.indices.len() / 3).saturating_mul(count),
    )?;
    let mut mesh = source.clone();
    let base = topology(source);
    let stride = groups.iter().copied().max().unwrap_or(0) + 1;
    let next_groups: Vec<_> = (0..count)
        .flat_map(|copy| groups.iter().map(move |g| g + copy as u64 * stride))
        .collect();
    let mut vertices = Vec::with_capacity(source.vertices.len() * count);
    let mut faces = Vec::with_capacity(base.faces.len() * count);
    let mut materials = Vec::with_capacity(faces.capacity());
    let mut seams = Vec::new();
    for copy in 0..count {
        cancelled(cancel)?;
        let start = vertices.len() as u32;
        if let Some(t) = &source.topology {
            seams.extend(t.uv_seams.iter().map(|[a, b]| [a + start, b + start]));
        }
        for (i, v) in source.vertices.iter().enumerate() {
            if i % 4096 == 0 {
                cancelled(cancel)?;
            }
            let mut v = *v;
            if let Some((axis, plane)) = mirror.filter(|_| copy == 1) {
                v.position[axis] = 2. * plane - v.position[axis];
                v.normal[axis] = -v.normal[axis];
            } else {
                v.position = (Vec3::from_array(v.position) + offset * copy as f32).to_array();
            }
            vertices.push(v);
        }
        for (face, material) in base.faces.iter().zip(base.face_materials.iter()) {
            let mut face: Vec<_> = face.iter().map(|i| i + start).collect();
            if mirror.is_some() && copy == 1 {
                face.reverse();
            }
            faces.push(face);
            materials.push(*material);
        }
    }
    mesh.vertices = vertices.into();
    mesh.topology = Some(MeshTopology {
        faces: faces.into(),
        face_materials: materials.into(),
        uv_seams: seams,
        ..base
    });
    *groups = next_groups;
    finish(mesh, groups, cancel, false)
}
fn average(mesh: &Mesh, indices: &[u32]) -> Vertex {
    let mut v = Vertex {
        normal: [0.; 3],
        color: [0.; 4],
        ..Default::default()
    };
    let weight = 1. / indices.len() as f32;
    for &index in indices {
        let a = mesh.vertices[index as usize];
        for i in 0..3 {
            v.position[i] += a.position[i] * weight;
            v.normal[i] += a.normal[i] * weight;
        }
        for i in 0..2 {
            v.uv[i] += a.uv[i] * weight;
        }
        for i in 0..4 {
            v.color[i] += a.color[i] * weight;
        }
    }
    v.color = v.color.map(|v| v.clamp(0., 1.));
    v
}

/// Catmull–Clark on position-welded polygon adjacency; UV/color corners stay split.
/// Simple mode uses identical quad topology without moving the original surface.
fn subdivide(
    source: &Mesh,
    smooth: bool,
    groups: &mut Vec<u64>,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    let input = topology(source);
    let corners = input.faces.iter().map(Vec::len).sum::<usize>();
    geometry_budget(corners.saturating_mul(4), corners.saturating_mul(2))?;
    let centers: Vec<_> = input.faces.iter().map(|f| average(source, f)).collect();
    let mut points: BTreeMap<(usize, usize), Vec3> = BTreeMap::new();
    let mut moved = Vec::new();
    let graph = if smooth {
        Some(Graph::new(source, groups))
    } else {
        None
    };
    if let Some(graph) = &graph {
        if graph.edge_faces.values().any(|f| f.len() > 2) {
            return Err(error(
                "Для сглаживания нужны рёбра не более чем с двумя гранями",
            ));
        }
        let mut incident = vec![BTreeSet::new(); source.vertices.len()];
        let mut boundary = vec![Vec::new(); source.vertices.len()];
        for (&(a, b), adjacent) in &graph.edge_faces {
            let pa = position(source, a);
            let pb = position(source, b);
            let p = if adjacent.len() == 2 {
                (pa + pb
                    + Vec3::from_array(centers[adjacent[0]].position)
                    + Vec3::from_array(centers[adjacent[1]].position))
                    * 0.25
            } else {
                boundary[a].push(b);
                boundary[b].push(a);
                (pa + pb) * 0.5
            };
            points.insert((a, b), p);
            for &f in adjacent {
                incident[a].insert(f);
                incident[b].insert(f);
            }
        }
        moved = source
            .vertices
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let i = graph.canonical[i];
                let p = position(source, i);
                let n = graph.neighbors[i].len();
                if boundary[i].len() == 2 {
                    p * 0.75
                        + (position(source, boundary[i][0]) + position(source, boundary[i][1]))
                            * 0.125
                } else if !boundary[i].is_empty() || n < 3 || incident[i].len() != n {
                    p
                } else {
                    let nf = n as f32;
                    let f = incident[i]
                        .iter()
                        .map(|&f| Vec3::from_array(centers[f].position))
                        .sum::<Vec3>()
                        / nf;
                    let r = graph.neighbors[i]
                        .iter()
                        .map(|&b| (p + position(source, b)) * 0.5)
                        .sum::<Vec3>()
                        / nf;
                    (f + 2. * r + (nf - 3.) * p) / nf
                }
            })
            .collect();
    }
    let mut next_groups = Vec::with_capacity(corners * 4);
    let mut vertices = Vec::with_capacity(corners * 4);
    let mut faces = Vec::with_capacity(corners);
    let mut materials = Vec::with_capacity(corners);
    for (f, face) in input.faces.iter().enumerate() {
        if f % 1024 == 0 {
            cancelled(cancel)?;
        }
        let edge_vertex = |a: u32, b: u32| {
            let mut v = average(source, &[a, b]);
            if let Some(g) = &graph {
                if let Some(p) = points.get(&edge(g.canonical[a as usize], g.canonical[b as usize]))
                {
                    v.position = p.to_array();
                }
            }
            v
        };
        for j in 0..face.len() {
            let a = face[j];
            let b = face[(j + 1) % face.len()];
            let c = face[(j + face.len() - 1) % face.len()];
            let mut v = source.vertices[a as usize];
            if smooth {
                v.position = moved[a as usize].to_array();
            }
            let start = vertices.len() as u32;
            vertices.extend([v, edge_vertex(a, b), centers[f], edge_vertex(c, a)]);
            next_groups.extend([groups[a as usize]; 4]);
            faces.push(vec![start, start + 1, start + 2, start + 3]);
            materials.push(input.face_materials[f]);
        }
    }
    let mut mesh = source.clone();
    mesh.vertices = vertices.into();
    mesh.topology = Some(MeshTopology {
        faces: faces.into(),
        face_materials: materials.into(),
        smooth_normals: smooth,
        ..input
    });
    *groups = next_groups;
    finish(mesh, groups, cancel, true)
}
fn solidify(
    source: &Mesh,
    thickness: f32,
    offset: f32,
    groups: &mut Vec<u64>,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    if thickness == 0. {
        return Ok(source.clone());
    }
    let input = topology(source);
    let graph = Graph::new(source, groups);
    if graph.edge_faces.values().any(|faces| faces.len() > 2) {
        return Err(error(
            "Толщина недоступна: ребро соединяет больше двух граней",
        ));
    }
    let mut normals = vec![Vec3::ZERO; source.vertices.len()];
    for tri in source.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let normal = (position(source, b) - position(source, a))
            .cross(position(source, c) - position(source, a));
        for i in [a, b, c] {
            normals[graph.canonical[i]] += normal;
        }
    }
    let boundary = graph.edge_faces.values().filter(|f| f.len() == 1).count();
    geometry_budget(
        source.vertices.len().saturating_mul(2),
        (source.indices.len() / 3)
            .saturating_mul(2)
            .saturating_add(boundary * 2),
    )?;
    let count = source.vertices.len() as u32;
    let mut vertices = Vec::with_capacity(source.vertices.len() * 2);
    for side in 0..2 {
        for (i, v) in source.vertices.iter().enumerate() {
            if i % 4096 == 0 {
                cancelled(cancel)?;
            }
            let mut v = *v;
            let normal = normals[graph.canonical[i]].normalize_or_zero();
            let distance = thickness * (offset + if side == 0 { 1. } else { -1. }) * 0.5;
            v.position = (Vec3::from_array(v.position) + normal * distance).to_array();
            if side == 1 {
                v.normal = v.normal.map(|n| -n);
            }
            vertices.push(v);
        }
    }
    let mut faces = Vec::with_capacity(input.faces.len() * 2 + boundary);
    let mut materials = Vec::with_capacity(faces.capacity());
    for (f, face) in input.faces.iter().enumerate() {
        if f % 1024 == 0 {
            cancelled(cancel)?;
        }
        faces.push(face.clone());
        materials.push(input.face_materials[f]);
        faces.push(face.iter().rev().map(|i| i + count).collect());
        materials.push(input.face_materials[f]);
        for j in 0..face.len() {
            let a = face[j];
            let b = face[(j + 1) % face.len()];
            if graph
                .edge_faces
                .get(&edge(
                    graph.canonical[a as usize],
                    graph.canonical[b as usize],
                ))
                .is_some_and(|f| f.len() == 1)
            {
                faces.push(vec![b, a, a + count, b + count]);
                materials.push(input.face_materials[f]);
            }
        }
    }
    let mut mesh = source.clone();
    mesh.vertices = vertices.into();
    mesh.topology = Some(MeshTopology {
        faces: faces.into(),
        face_materials: materials.into(),
        ..input
    });
    groups.extend_from_within(..);
    finish(mesh, groups, cancel, true)
}

/// Keep procedural copies separate even when their boundaries touch. Within each
/// shell, position welding still reconnects UV and hard-normal seams.
struct Graph {
    canonical: Vec<usize>,
    edge_faces: BTreeMap<(usize, usize), Vec<usize>>,
    neighbors: Vec<BTreeSet<usize>>,
}
impl Graph {
    fn new(mesh: &Mesh, groups: &[u64]) -> Self {
        let mut weld = BTreeMap::new();
        let canonical: Vec<_> = mesh
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| {
                *weld
                    .entry((crate::position_key(v.position), groups[i]))
                    .or_insert(i)
            })
            .collect();
        let mut edge_faces = BTreeMap::<(usize, usize), Vec<usize>>::new();
        let mut neighbors = vec![BTreeSet::new(); mesh.vertices.len()];
        for f in 0..mesh.face_count() {
            let face = mesh.face_vertices(f).unwrap();
            for j in 0..face.len() {
                let a = canonical[face[j] as usize];
                let b = canonical[face[(j + 1) % face.len()] as usize];
                if a == b {
                    continue;
                }
                edge_faces.entry(edge(a, b)).or_default().push(f);
                neighbors[a].insert(b);
                neighbors[b].insert(a);
            }
        }
        Self {
            canonical,
            edge_faces,
            neighbors,
        }
    }
}
