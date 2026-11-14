//! Constrained triangulation for finite knife segments inside a polygon.
//! Endpoints interpolate attributes; an edge flip inserts the requested segment.
use crate::{CoreError, Mesh, Vec3, Vertex};
use std::collections::BTreeMap;
fn error() -> CoreError {
    CoreError::Invalid("Нож: отрезок вне поверхности или вырожден".into())
}
fn p(mesh: &Mesh, i: u32) -> Vec3 {
    Vec3::from_array(mesh.vertices[i as usize].position)
}
fn bary(mesh: &Mesh, t: [u32; 3], point: Vec3) -> Option<[f32; 3]> {
    let a = p(mesh, t[0]);
    let b = p(mesh, t[1]) - a;
    let c = p(mesh, t[2]) - a;
    let d = point - a;
    let n = b.cross(c);
    let area = n.length_squared();
    if area < 1e-20 || d.dot(n).abs() > area.sqrt() * 1e-5 {
        return None;
    }
    let v = d.cross(c).dot(n) / area;
    let w = b.cross(d).dot(n) / area;
    let r = [1. - v - w, v, w];
    r.iter().all(|x| *x >= -1e-5).then_some(r)
}
fn triangles(mesh: &Mesh, ring: &[u32]) -> Result<Vec<u32>, CoreError> {
    let mut ring = ring.to_vec();
    if ring.len() == 4 {
        let n = crate::polygon_edit::face_normal(mesh, &ring);
        let good = |r: &[u32]| {
            (p(mesh, r[1]) - p(mesh, r[0]))
                .cross(p(mesh, r[2]) - p(mesh, r[0]))
                .dot(n)
                > 1e-16
                && (p(mesh, r[2]) - p(mesh, r[0]))
                    .cross(p(mesh, r[3]) - p(mesh, r[0]))
                    .dot(n)
                    > 1e-16
        };
        if !good(&ring) {
            ring.rotate_left(1);
            if !good(&ring) {
                return Err(error());
            }
        }
    }
    crate::triangulate::triangulate(mesh, &ring)
}
pub(crate) fn contains(mesh: &Mesh, face: usize, point: Vec3) -> bool {
    mesh.face_vertices(face)
        .and_then(|f| triangles(mesh, f).ok())
        .is_some_and(|ts| {
            ts.chunks_exact(3)
                .any(|t| bary(mesh, [t[0], t[1], t[2]], point).is_some())
        })
}
fn interpolate(mesh: &Mesh, t: [u32; 3], b: [f32; 3], point: Vec3) -> Vertex {
    let v = t.map(|i| mesh.vertices[i as usize]);
    let mut out = v[0];
    out.position = point.to_array();
    out.normal = (Vec3::from_array(v[0].normal) * b[0]
        + Vec3::from_array(v[1].normal) * b[1]
        + Vec3::from_array(v[2].normal) * b[2])
        .normalize_or_zero()
        .to_array();
    out.uv = std::array::from_fn(|i| (0..3).map(|j| v[j].uv[i] * b[j]).sum());
    out.color = std::array::from_fn(|i| (0..3).map(|j| v[j].color[i] * b[j]).sum());
    out
}
fn insert(mesh: &mut Mesh, triangles: &mut Vec<[u32; 3]>, point: Vec3) -> Result<u32, CoreError> {
    if let Some(i) = triangles
        .iter()
        .flatten()
        .find(|&&i| p(mesh, i).distance_squared(point) < 1e-12)
    {
        return Ok(*i);
    }
    let (triangle, weights) = triangles
        .iter()
        .find_map(|&t| bary(mesh, t, point).map(|b| (t, b)))
        .ok_or_else(error)?;
    if mesh.vertices.len() >= crate::MAX_EDIT_VERTICES {
        return Err(error());
    }
    let id = mesh.vertices.len() as u32;
    let v = interpolate(mesh, triangle, weights, point);
    mesh.vertices.push(v);
    let mut result = Vec::new();
    for t in triangles.drain(..) {
        if bary(mesh, t, point).is_some() {
            for j in 0..3 {
                let a = t[j];
                let b = t[(j + 1) % 3];
                if (p(mesh, b) - p(mesh, a))
                    .cross(point - p(mesh, a))
                    .length_squared()
                    > 1e-20
                {
                    result.push([a, b, id]);
                }
            }
        } else {
            result.push(t);
        }
    }
    *triangles = result;
    Ok(id)
}
fn crossing(mesh: &Mesh, a: u32, b: u32, c: u32, d: u32, n: Vec3) -> bool {
    let ab = p(mesh, b) - p(mesh, a);
    let cd = p(mesh, d) - p(mesh, c);
    let u = ab.cross(p(mesh, c) - p(mesh, a)).dot(n);
    let v = ab.cross(p(mesh, d) - p(mesh, a)).dot(n);
    let s = cd.cross(p(mesh, a) - p(mesh, c)).dot(n);
    let t = cd.cross(p(mesh, b) - p(mesh, c)).dot(n);
    u * v < -1e-16 && s * t < -1e-16
}
fn constrain(mesh: &Mesh, ts: &mut [[u32; 3]], a: u32, b: u32, n: Vec3) -> Result<(), CoreError> {
    for _ in 0..ts.len() * ts.len() + 1 {
        let mut edges = BTreeMap::<(u32, u32), Vec<(usize, u32)>>::new();
        for (f, t) in ts.iter().enumerate() {
            for j in 0..3 {
                let x = t[j];
                let y = t[(j + 1) % 3];
                edges
                    .entry((x.min(y), x.max(y)))
                    .or_default()
                    .push((f, t[(j + 2) % 3]));
            }
        }
        if edges.contains_key(&(a.min(b), a.max(b))) {
            return Ok(());
        }
        let candidate = edges.iter().find(|((x, y), faces)| {
            faces.len() == 2
                && crossing(mesh, a, b, *x, *y, n)
                && crossing(mesh, *x, *y, faces[0].1, faces[1].1, n)
        });
        let Some((&(x, y), adj)) = candidate else {
            return Err(error());
        };
        let (f, c) = adj[0];
        let (g, d) = adj[1];
        for (i, mut t) in [(f, [c, d, x]), (g, [d, c, y])] {
            if (p(mesh, t[1]) - p(mesh, t[0]))
                .cross(p(mesh, t[2]) - p(mesh, t[0]))
                .dot(n)
                < 0.
            {
                t.swap(1, 2);
            }
            ts[i] = t;
        }
    }
    Err(error())
}
pub(crate) fn cut(mesh: &mut Mesh, face: usize, a: Vec3, b: Vec3) -> Result<(), CoreError> {
    if a.distance_squared(b) < 1e-14 {
        return Err(error());
    }
    let ring = mesh.face_vertices(face).ok_or_else(error)?.to_vec();
    let n = crate::polygon_edit::face_normal(mesh, &ring).normalize_or_zero();
    let mut triangles: Vec<[u32; 3]> = triangles(mesh, &ring)?
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let first = insert(mesh, &mut triangles, a)?;
    let last = insert(mesh, &mut triangles, b)?;
    let d = b - a;
    let len = d.length_squared();
    let mut chain: Vec<_> = triangles
        .iter()
        .flatten()
        .copied()
        .filter_map(|i| {
            let q = p(mesh, i);
            let t = (q - a).dot(d) / len;
            ((-1e-5..=1.00001).contains(&t) && q.distance_squared(a + d * t) < 1e-12)
                .then_some((t, i))
        })
        .collect();
    chain.sort_by(|x, y| x.0.total_cmp(&y.0));
    chain.dedup_by_key(|x| x.1);
    if chain.first().map(|v| v.1) != Some(first) || chain.last().map(|v| v.1) != Some(last) {
        return Err(error());
    }
    for pair in chain.windows(2) {
        constrain(mesh, &mut triangles, pair[0].1, pair[1].1, n)?;
    }
    let material = mesh.face_material(face);
    // Split neighbouring polygon boundaries too, with their own UV interpolation.
    let mut faces: Vec<_> = (0..mesh.face_count())
        .map(|f| mesh.face_vertices(f).unwrap().to_vec())
        .collect();
    let old_seams = crate::uv_seams(mesh);
    let graph = crate::MeshConnectivity::new(mesh);
    let mut seams = Vec::new();
    for (f, poly) in faces.iter_mut().enumerate() {
        if f == face {
            continue;
        }
        let mut expanded = Vec::new();
        for j in 0..poly.len() {
            let x = poly[j];
            let y = poly[(j + 1) % poly.len()];
            expanded.push(x);
            let pa = p(mesh, x);
            let pb = p(mesh, y);
            let delta = pb - pa;
            let length = delta.length_squared();
            let mut points: Vec<_> = [a, b]
                .into_iter()
                .filter_map(|point| {
                    let t = (point - pa).dot(delta) / length;
                    ((1e-5..1. - 1e-5).contains(&t)
                        && point.distance_squared(pa + delta * t) < 1e-12)
                        .then_some((t, point))
                })
                .collect();
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut previous = x;
            let key = (
                graph.canonical[x as usize].min(graph.canonical[y as usize]),
                graph.canonical[x as usize].max(graph.canonical[y as usize]),
            );
            for (t, point) in points {
                let id = mesh.vertices.len() as u32;
                let vertex = interpolate(mesh, [x, y, x], [1. - t, t, 0.], point);
                mesh.vertices.push(vertex);
                expanded.push(id);
                if old_seams.contains(&key) {
                    seams.push([previous, id]);
                }
                previous = id;
            }
            if old_seams.contains(&key) {
                seams.push([previous, y]);
            }
        }
        *poly = expanded;
    }
    let mut materials: Vec<_> = (0..faces.len()).map(|f| mesh.face_material(f)).collect();
    faces.splice(face..=face, triangles.iter().map(|t| t.to_vec()));
    materials.splice(face..=face, std::iter::repeat_n(material, triangles.len()));
    let smooth = mesh.smooth_normals_enabled();
    let topology = mesh.topology.get_or_insert_with(|| crate::MeshTopology {
        faces: Default::default(),
        face_materials: Default::default(),
        uv_seams: Vec::new(),
        loose_edges: Vec::new(),
        primitive: None,
        smooth_normals: smooth,
    });
    topology.faces = faces.into();
    topology.face_materials = materials.into();
    topology.primitive = None;
    topology.smooth_normals = smooth;
    let old_segments: Vec<_> = old_seams
        .iter()
        .map(|&(x, y)| (p(mesh, x as u32), p(mesh, y as u32)))
        .collect();
    mesh.topology.as_mut().unwrap().uv_seams.clear();
    let graph = crate::MeshConnectivity::new(mesh);
    let seams = graph
        .edges
        .iter()
        .filter(|&&(x, y)| {
            old_segments.iter().any(|&(a, b)| {
                let d = b - a;
                [p(mesh, x as u32), p(mesh, y as u32)].iter().all(|p| {
                    let t = (*p - a).dot(d) / d.length_squared();
                    (-1e-5..=1.00001).contains(&t) && p.distance_squared(a + d * t) < 1e-10
                })
            })
        })
        .map(|&(x, y)| [x as u32, y as u32])
        .collect();
    mesh.topology.as_mut().unwrap().uv_seams = seams;
    mesh.rebuild_polygon_indices()?;
    crate::recompute_normals(mesh, smooth);
    mesh.validate_topology()
}
