//! Editable plane sections. Temporary tessellation never becomes surface topology.
use super::{caps, interpolate, normal, position, Polygon};
use crate::{Mesh, MeshConnectivity, MeshTopology, Vec3, Vertex};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PlaneCut {
    pub normal: [f32; 3],
    pub offset: f32,
    pub width: f32,
    pub close_gap: bool,
    /// Face in the original mesh; restricts the plane to its connected component.
    pub seed_face: usize,
}
type Key = [i64; 3];
fn key(v: &Vertex) -> Key {
    crate::authoring::position_key(v.position)
}
fn clip_ring(ring: &[Vertex], n: Vec3, d: f32, eps: f32) -> Vec<Vertex> {
    let mut out = Vec::new();
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let distance = |v: Vertex| {
            let x = n.dot(position(&v)) - d;
            if x.abs() < eps {
                0.
            } else {
                x
            }
        };
        let da = distance(a);
        let db = distance(b);
        if da <= 0. {
            out.push(a);
        }
        if da * db < 0. {
            // Same arithmetic on the two sides of every shared edge, including UV seams.
            let mut v = interpolate(a, b, da / (da - db));
            v.position = if a.position < b.position {
                position(&a).lerp(position(&b), da / (da - db))
            } else {
                position(&b).lerp(position(&a), db / (db - da))
            }
            .to_array();
            out.push(v);
        }
    }
    out.dedup_by(|a, b| key(a) == key(b));
    if out.len() > 1 && key(&out[0]) == key(out.last().unwrap()) {
        out.pop();
    }
    out
}
fn clip_polygon(
    face: &Polygon,
    n: Vec3,
    d: f32,
    eps: f32,
    template: &Mesh,
) -> Result<Vec<Polygon>, String> {
    let distances: Vec<_> = face
        .vertices
        .iter()
        .map(|v| n.dot(position(v)) - d)
        .collect();
    if distances.iter().all(|x| x.abs() <= eps) {
        return Ok(if normal(&face.vertices).dot(n) >= 0. {
            vec![face.clone()]
        } else {
            Vec::new()
        });
    }
    if distances.iter().all(|&x| x <= eps) {
        return Ok(vec![face.clone()]);
    }
    if distances.iter().all(|&x| x >= -eps) {
        return Ok(Vec::new());
    }
    let transitions = (0..distances.len())
        .filter(|&i| (distances[i] < -eps) != (distances[(i + 1) % distances.len()] < -eps))
        .count();
    if transitions <= 2 {
        let vertices = clip_ring(&face.vertices, n, d, eps);
        return Ok(if vertices.len() >= 3 && normal(&vertices) != Vec3::ZERO {
            vec![Polygon {
                vertices,
                material: face.material,
            }]
        } else {
            Vec::new()
        });
    }
    // A concave polygon can split into multiple islands. Clip temporary triangles,
    // cancel their internal edges, then recover each polygon boundary.
    let mut local = template.clone();
    local.vertices = face.vertices.clone().into();
    let ring: Vec<_> = (0..face.vertices.len() as u32).collect();
    let tris = crate::triangulate::triangulate(&local, &ring).map_err(|e| e.to_string())?;
    let mut edges = BTreeSet::<(Key, Key)>::new();
    let mut vertices = BTreeMap::new();
    for tri in tris.chunks_exact(3) {
        let ring: Vec<_> = tri.iter().map(|&i| face.vertices[i as usize]).collect();
        let clipped = clip_ring(&ring, n, d, eps);
        if clipped.len() < 3 || normal(&clipped) == Vec3::ZERO {
            continue;
        }
        for i in 0..clipped.len() {
            let a = clipped[i];
            let b = clipped[(i + 1) % clipped.len()];
            let ka = key(&a);
            let kb = key(&b);
            vertices.entry(ka).or_insert(a);
            vertices.entry(kb).or_insert(b);
            if ka != kb && !edges.remove(&(kb, ka)) {
                edges.insert((ka, kb));
            }
        }
    }
    let mut out = Vec::new();
    while let Some((a, mut b)) = edges.pop_first() {
        let mut ring = vec![vertices[&a]];
        while b != a {
            ring.push(vertices[&b]);
            let next = edges
                .range((b, [i64::MIN; 3])..=(b, [i64::MAX; 3]))
                .next()
                .copied()
                .ok_or("Не удалось собрать контур разрезанной грани")?;
            edges.remove(&next);
            b = next.1;
            if ring.len() > crate::MAX_FACE_CORNERS {
                return Err("Слишком сложный контур разреза".into());
            }
        }
        if ring.len() >= 3 {
            out.push(Polygon {
                vertices: ring,
                material: face.material,
            });
        }
    }
    Ok(out)
}
fn half(
    faces: &[Polygon],
    n: Vec3,
    d: f32,
    eps: f32,
    cap: bool,
    template: &Mesh,
) -> Result<Vec<Polygon>, String> {
    let mut out = Vec::new();
    for face in faces {
        out.extend(clip_polygon(face, n, d, eps, template)?);
    }
    if cap && !out.is_empty() {
        let material = out.first().and_then(|p| p.material).or(template.material);
        let caps = caps::close_cut(&out, n, d, eps, material, &AtomicBool::new(false))?
            .ok_or("Не удалось замкнуть контур разреза: проверьте разветвления исходной сетки")?;
        out.extend(caps);
    }
    Ok(out)
}
pub(crate) fn apply(mesh: &mut Mesh, cuts: &[PlaneCut]) -> Result<(), String> {
    if cuts.is_empty() {
        return Ok(());
    }
    if cuts.len() > 128 {
        return Err("Допускается до 128 разрезов за одну операцию".into());
    }
    for cut in cuts {
        let n = Vec3::from_array(cut.normal);
        if !n.is_finite()
            || (n.length() - 1.).abs() > 0.001
            || !cut.offset.is_finite()
            || !cut.width.is_finite()
            || !(0. ..=1000.).contains(&cut.width)
            || cut.seed_face >= mesh.face_count()
        {
            return Err("Некорректные параметры плоскости разреза".into());
        }
    }
    let source = mesh.clone();
    let g = MeshConnectivity::new(&source);
    let mut unseen: BTreeSet<_> = (0..source.face_count()).collect();
    let mut output = Vec::new();
    while let Some(seed) = unseen.pop_first() {
        let mut component = BTreeSet::from([seed]);
        let mut stack = vec![seed];
        while let Some(f) = stack.pop() {
            for &adj in &g.face_neighbors[f] {
                if unseen.remove(&adj) {
                    component.insert(adj);
                    stack.push(adj);
                }
            }
        }
        let mut polygons: Vec<_> = component
            .iter()
            .map(|&f| Polygon {
                vertices: source
                    .face_vertices(f)
                    .unwrap()
                    .iter()
                    .map(|&v| source.vertices[v as usize])
                    .collect(),
                material: source.face_material(f),
            })
            .collect();
        let relevant: Vec<_> = cuts
            .iter()
            .filter(|c| component.contains(&c.seed_face))
            .collect();
        let closed = g
            .edge_faces
            .values()
            .filter(|fs| fs.iter().any(|f| component.contains(f)))
            .all(|fs| fs.len() == 2);
        for cut in relevant {
            let n = Vec3::from_array(cut.normal);
            let eps = 1e-6;
            let points = polygons.iter().flat_map(|p| &p.vertices);
            let (lo, hi) = points.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                let d = n.dot(position(v));
                (lo.min(d), hi.max(d))
            });
            if cut.offset < lo - eps || cut.offset > hi + eps {
                continue;
            }
            let cap = cut.close_gap && cut.width > eps && closed;
            let mut next = half(
                &polygons,
                n,
                cut.offset - cut.width * 0.5,
                eps,
                cap,
                &source,
            )?;
            next.extend(half(
                &polygons,
                -n,
                -cut.offset - cut.width * 0.5,
                eps,
                cap,
                &source,
            )?);
            if next.is_empty() {
                return Err("Зазор удаляет всю выбранную часть сетки".into());
            }
            if next.iter().map(|p| p.vertices.len()).sum::<usize>() > crate::MAX_EDIT_VERTICES {
                return Err("Разрез превышает предел сетки".into());
            }
            polygons = next;
        }
        output.extend(polygons);
    }
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    for p in output {
        let start = vertices.len() as u32;
        faces.push((start..start + p.vertices.len() as u32).collect());
        vertices.extend(p.vertices);
        materials.push(p.material);
    }
    let used: BTreeSet<_> = (0..source.face_count())
        .flat_map(|f| source.face_vertices(f).unwrap().iter().copied())
        .collect();
    let detached: Vec<_> = source
        .vertices
        .iter()
        .enumerate()
        .filter(|(i, _)| !used.contains(&(*i as u32)))
        .map(|(_, v)| *v)
        .collect();
    if vertices.len() + detached.len() > crate::MAX_EDIT_VERTICES {
        return Err("Разрез превышает предел вершин".into());
    }
    mesh.vertices = vertices.into();
    mesh.topology = Some(MeshTopology {
        faces: faces.into(),
        face_materials: materials.into(),
        smooth_normals: source.smooth_normals_enabled(),
        primitive: None,
        loose_edges: Vec::new(),
        uv_seams: Vec::new(),
    });
    mesh.rebuild_polygon_indices().map_err(|e| e.to_string())?;
    if mesh.indices.len() / 3 > crate::MAX_EDIT_TRIANGLES {
        return Err("Разрез превышает предел сетки".into());
    }
    crate::recompute_normals(mesh, source.smooth_normals_enabled());
    mesh.vertices.extend(detached);
    mesh.validate_topology().map_err(|e| e.to_string())
}

/// The preview follows the same polygon clipping as the committed section.
pub fn plane_cut_path(
    mesh: &Mesh,
    cut: &PlaneCut,
) -> Result<Vec<crate::KnifeCut>, crate::CoreError> {
    let g = MeshConnectivity::new(mesh);
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let n = Vec3::from_array(cut.normal);
    let d = cut.offset;
    for f in g.connected_faces(cut.seed_face) {
        let face = Polygon {
            vertices: mesh
                .face_vertices(f)
                .unwrap()
                .iter()
                .map(|&v| mesh.vertices[v as usize])
                .collect(),
            material: mesh.face_material(f),
        };
        for p in clip_polygon(&face, n, d, 1e-6, mesh).map_err(|e| crate::CoreError::Invalid(e))? {
            for i in 0..p.vertices.len() {
                let a = p.vertices[i];
                let b = p.vertices[(i + 1) % p.vertices.len()];
                if (n.dot(position(&a)) - d).abs() < 2e-6 && (n.dot(position(&b)) - d).abs() < 2e-6
                {
                    let (a_key, b_key) = (key(&a), key(&b));
                    if a_key != b_key && seen.insert((a_key.min(b_key), a_key.max(b_key))) {
                        result.push(crate::KnifeCut {
                            face: f,
                            first: a.position,
                            second: b.position,
                        });
                    }
                }
            }
        }
    }
    Ok(result)
}
