//! Original ArtForge fox head: a connected quad cage, including ears and sockets.
//! Small, bounded Catmull-Clark refinement is local to this creation primitive.
use crate::{CoreError, Mesh, PrimitiveSettings, Vec3, Vertex};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Cage {
    vertices: Vec<[f32; 3]>,
    faces: Vec<[usize; 4]>,
}

pub(crate) fn build(mesh: &mut Mesh, settings: PrimitiveSettings) -> Result<(), CoreError> {
    let cage: Cage = serde_json::from_str(include_str!("../assets/fox_head.json"))
        .map_err(|e| CoreError::Invalid(format!("Геометрия лисы: {e}")))?;
    let mut points: Vec<_> = cage.vertices.into_iter().map(Vec3::from_array).collect();
    let mut faces = cage.faces;
    for _ in 1..settings.segments[0] {
        (points, faces) = subdivide(&points, &faces);
    }
    let min = points
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let max = points
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let center = (min + max) * 0.5;
    let scale = Vec3::from_array(settings.size) / (max - min);
    for point in points {
        let p = point - center;
        mesh.vertices.push(Vertex {
            position: (p * scale).to_array(),
            // Cylindrical UVs with an explicit back seam; no baked material.
            uv: [
                0.5 + p.x.atan2(p.z) / std::f32::consts::TAU,
                (point.y - min.y) / (max.y - min.y),
            ],
            ..Vertex::default()
        });
    }
    let topology = mesh.topology.as_mut().unwrap();
    let mut seams = BTreeMap::new();
    for face in faces {
        let min_u = face
            .iter()
            .map(|&i| mesh.vertices[i].uv[0])
            .fold(f32::INFINITY, f32::min);
        let max_u = face
            .iter()
            .map(|&i| mesh.vertices[i].uv[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let mut polygon = Vec::with_capacity(4);
        for i in face {
            let index = if max_u - min_u > 0.5 && mesh.vertices[i].uv[0] < 0.5 {
                *seams.entry(i).or_insert_with(|| {
                    let mut vertex = mesh.vertices[i];
                    vertex.uv[0] += 1.;
                    let id = mesh.vertices.len() as u32;
                    mesh.vertices.push(vertex);
                    id
                })
            } else {
                i as u32
            };
            polygon.push(index);
        }
        topology.faces.push(polygon);
    }
    mesh.rebuild_polygon_indices()?;
    if settings.smooth {
        crate::recompute_normals(mesh, true);
    } else {
        crate::polygon_edit::flat_normals(mesh)?;
    }
    Ok(())
}

fn subdivide(points: &[Vec3], faces: &[[usize; 4]]) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let centers: Vec<_> = faces
        .iter()
        .map(|face| face.iter().map(|&v| points[v]).sum::<Vec3>() * 0.25)
        .collect();
    let mut edges: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let mut adjacent = vec![Vec::new(); points.len()];
    let mut neighbors = vec![Vec::new(); points.len()];
    let edge = |a: usize, b: usize| (a.min(b), a.max(b));
    for (fi, face) in faces.iter().enumerate() {
        for i in 0..4 {
            let a = face[i];
            let b = face[(i + 1) % 4];
            edges.entry(edge(a, b)).or_default().push(fi);
            adjacent[a].push(fi);
        }
    }
    for &(a, b) in edges.keys() {
        neighbors[a].push(b);
        neighbors[b].push(a);
    }
    let mut result = Vec::with_capacity(points.len() + edges.len() + faces.len());
    for (i, &p) in points.iter().enumerate() {
        let n = neighbors[i].len() as f32;
        let f = adjacent[i].iter().map(|&f| centers[f]).sum::<Vec3>() / n;
        let r = neighbors[i]
            .iter()
            .map(|&b| (p + points[b]) * 0.5)
            .sum::<Vec3>()
            / n;
        result.push((f + 2. * r + (n - 3.) * p) / n);
    }
    let mut edge_indices = BTreeMap::new();
    for ((a, b), linked) in edges {
        debug_assert_eq!(linked.len(), 2, "the original fox cage is closed");
        edge_indices.insert((a, b), result.len());
        result.push((points[a] + points[b] + centers[linked[0]] + centers[linked[1]]) * 0.25);
    }
    let face_start = result.len();
    result.extend(centers);
    let mut quads = Vec::with_capacity(faces.len() * 4);
    for (fi, face) in faces.iter().enumerate() {
        for i in 0..4 {
            quads.push([
                face[i],
                edge_indices[&edge(face[i], face[(i + 1) % 4])],
                face_start + fi,
                edge_indices[&edge(face[(i + 3) % 4], face[i])],
            ]);
        }
    }
    (result, quads)
}
