//! A complete projected stroke is applied atomically from an immutable source.
use crate::{CoreError, EdgePoint, Mesh, MeshConnectivity, Vec3};

#[derive(Clone, Debug)]
pub struct KnifeCut {
    pub face: usize,
    pub first: [f32; 3],
    pub second: [f32; 3],
}
fn hits(mesh: &Mesh, g: &MeshConnectivity, face: usize, point: Vec3) -> Vec<EdgePoint> {
    let Some(face) = mesh.face_vertices(face) else {
        return vec![];
    };
    let mut result = Vec::new();
    for i in 0..face.len() {
        let a = g.canonical[face[i] as usize];
        let b = g.canonical[face[(i + 1) % face.len()] as usize];
        let edge = (a.min(b), a.max(b));
        let start = Vec3::from_array(mesh.vertices[edge.0].position);
        let end = Vec3::from_array(mesh.vertices[edge.1].position);
        let delta = end - start;
        let length = delta.length_squared();
        if length < 1e-16 {
            continue;
        }
        let t = (point - start).dot(delta) / length;
        if (-1e-5..=1.00001).contains(&t)
            && start.lerp(end, t).distance_squared(point) <= length.max(1.) * 1e-12
        {
            result.push(EdgePoint {
                edge,
                t: if t < 1e-5 {
                    0.
                } else if t > 1. - 1e-5 {
                    1.
                } else {
                    t
                },
            });
        }
    }
    result
}
fn segments(mesh: &Mesh, face: usize, a: Vec3, b: Vec3) -> Vec<(Vec3, Vec3)> {
    let Some(ring) = mesh.face_vertices(face) else {
        return Vec::new();
    };
    let n = crate::polygon_edit::face_normal(mesh, ring).normalize_or_zero();
    let d = b - a;
    let mut ts = vec![0., 1.];
    for i in 0..ring.len() {
        let p = Vec3::from_array(mesh.vertices[ring[i] as usize].position);
        let q = Vec3::from_array(mesh.vertices[ring[(i + 1) % ring.len()] as usize].position);
        let e = q - p;
        let div = d.cross(e).dot(n);
        if div.abs() < 1e-10 {
            continue;
        }
        let t = (p - a).cross(e).dot(n) / div;
        let u = (p - a).cross(d).dot(n) / div;
        if (0. ..=1.).contains(&t) && (-1e-5..=1.00001).contains(&u) {
            ts.push(t);
        }
    }
    ts.sort_by(f32::total_cmp);
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    ts.windows(2)
        .filter(|t| crate::knife_interior::contains(mesh, face, a + d * (t[0] + t[1]) * 0.5))
        .map(|t| (a + d * t[0], a + d * t[1]))
        .collect()
}
pub(crate) fn apply(mesh: &mut Mesh, cuts: &[KnifeCut]) -> Result<(), CoreError> {
    apply_tracked(mesh, cuts).map(|_| ())
}
fn apply_tracked(mesh: &mut Mesh, cuts: &[KnifeCut]) -> Result<Vec<usize>, CoreError> {
    let source = mesh.clone();
    let mut origins = Vec::new();
    if cuts.is_empty()
        || cuts.len() > 2048
        || cuts.iter().any(|c| {
            c.face >= mesh.face_count()
                || !Vec3::from_array(c.first).is_finite()
                || !Vec3::from_array(c.second).is_finite()
                || Vec3::from_array(c.first).distance_squared(Vec3::from_array(c.second)) < 1e-14
        })
    {
        return Err(CoreError::Invalid(
            "Нож: нет пересечённых граней или слишком длинный разрез".into(),
        ));
    }
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    let mut projection = Vec::new();
    for f in 0..source.face_count() {
        let ring = source.face_vertices(f).unwrap();
        let normal = crate::polygon_edit::face_normal(&source, ring).normalize_or_zero();
        let origin = Vec3::from_array(source.vertices[ring[0] as usize].position);
        let warped = cuts.iter().any(|c| c.face == f)
            && ring.iter().any(|&v| {
                (Vec3::from_array(source.vertices[v as usize].position) - origin)
                    .dot(normal)
                    .abs()
                    > 1e-5
            });
        projection.push(warped.then_some(normal));
        if warped {
            for cut in cuts.iter().filter(|c| c.face == f) {
                if !crate::knife_interior::contains(&source, f, Vec3::from_array(cut.first))
                    || !crate::knife_interior::contains(&source, f, Vec3::from_array(cut.second))
                {
                    return Err(CoreError::Invalid(
                        "Концы разреза должны лежать на поверхности".into(),
                    ));
                }
            }
            for tri in crate::triangulate::triangulate(&source, ring)?.chunks_exact(3) {
                faces.push(tri.to_vec());
                origins.push(f);
                materials.push(source.face_material(f));
            }
        } else {
            faces.push(ring.to_vec());
            origins.push(f);
            materials.push(source.face_material(f));
        }
    }
    crate::component_mesh::ensure_topology(mesh);
    mesh.topology.as_mut().unwrap().faces = faces.into();
    mesh.topology.as_mut().unwrap().face_materials = materials.into();
    mesh.rebuild_polygon_indices()?;
    let mut cuts = cuts.to_vec();
    cuts.sort_by_key(|c| std::cmp::Reverse(c.face));
    for cut in cuts {
        let candidates: Vec<_> = origins
            .iter()
            .enumerate()
            .filter(|(_, origin)| **origin == cut.face)
            .map(|(f, _)| f)
            .rev()
            .collect();
        let mut covered = false;
        for f in candidates {
            let mut a = Vec3::from_array(cut.first);
            let mut b = Vec3::from_array(cut.second);
            if let Some(direction) = projection[cut.face] {
                let ring = mesh.face_vertices(f).unwrap();
                let n = crate::polygon_edit::face_normal(mesh, ring).normalize_or_zero();
                let origin = Vec3::from_array(mesh.vertices[ring[0] as usize].position);
                let denominator = direction.dot(n);
                if denominator.abs() < 1e-6 {
                    continue;
                }
                a -= direction * ((a - origin).dot(n) / denominator);
                b -= direction * ((b - origin).dot(n) / denominator);
            }
            let pieces = segments(mesh, f, a, b);
            let mut end = f;
            for (a, b) in pieces.into_iter().rev() {
                for current in f..=end {
                    if !crate::knife_interior::contains(mesh, current, (a + b) * 0.5) {
                        continue;
                    }
                    let graph = MeshConnectivity::new(mesh);
                    let first = hits(mesh, &graph, current, a);
                    let second = hits(mesh, &graph, current, b);
                    // At a vertex both incident edges are hits. Recognize an
                    // already present side before trying the other edge pair.
                    if first
                        .iter()
                        .any(|x| second.iter().any(|y| x.edge == y.edge))
                    {
                        covered = true;
                        continue;
                    }
                    let mut applied = None;
                    'pairs: for x in &first {
                        for y in &second {
                            if x.edge == y.edge {
                                continue;
                            }
                            let mut candidate = mesh.clone();
                            if crate::mesh_advanced::knife(&mut candidate, current, *x, *y).is_ok()
                            {
                                applied = Some(candidate);
                                break 'pairs;
                            }
                        }
                    }
                    if applied.is_none() {
                        let mut candidate = mesh.clone();
                        if crate::knife_interior::cut(&mut candidate, current, a, b).is_ok() {
                            applied = Some(candidate);
                        }
                    }
                    if let Some(candidate) = applied {
                        let added = candidate.face_count() - mesh.face_count();
                        origins.splice(
                            current + 1..current + 1,
                            std::iter::repeat_n(origins[current], added),
                        );
                        *mesh = candidate;
                        covered = true;
                        end += added;
                        break;
                    }
                }
            }
        }
        if !covered {
            return Err(CoreError::Invalid(
                "Линия проходит вне грани или совпадает с ребром".into(),
            ));
        }
    }
    Ok(origins)
}

pub(crate) fn apply_gap(mesh: &mut Mesh, cuts: &[KnifeCut], width: f32) -> Result<(), CoreError> {
    if !width.is_finite() || !(0. ..=1000.).contains(&width) {
        return Err(CoreError::Invalid("Зазор разреза: 0–1000 м".into()));
    }
    if width == 0. {
        return apply(mesh, cuts);
    }
    let mut boundaries = Vec::new();
    let mut bands = Vec::new();
    for cut in cuts {
        let ring = mesh
            .face_vertices(cut.face)
            .ok_or_else(|| CoreError::Invalid("Грань разреза не существует".into()))?;
        let a = Vec3::from_array(cut.first);
        let b = Vec3::from_array(cut.second);
        let normal = crate::polygon_edit::face_normal(mesh, ring).normalize_or_zero();
        let side = normal
            .cross(b - a)
            .try_normalize()
            .ok_or_else(|| CoreError::Invalid("Слишком короткий разрез".into()))?;
        let along = (b - a).normalize();
        let length = a.distance(b);
        bands.push((cut.face, a, side, along, length));
        for (axis, shift, required) in [
            (side, -width * 0.5, true),
            (side, width * 0.5, true),
            (along, 0., false),
            (along, length, false),
        ] {
            let mut points = Vec::new();
            for i in 0..ring.len() {
                let p = Vec3::from_array(mesh.vertices[ring[i] as usize].position);
                let q =
                    Vec3::from_array(mesh.vertices[ring[(i + 1) % ring.len()] as usize].position);
                let u = (p - a).dot(axis) - shift;
                let v = (q - a).dot(axis) - shift;
                if u.abs() < 1e-6 {
                    points.push(p);
                }
                if u * v < 0. {
                    points.push(p.lerp(q, u / (u - v)));
                }
            }
            let tangent = normal.cross(axis);
            points.sort_by(|p, q| ((*p - a).dot(tangent)).total_cmp(&((*q - a).dot(tangent))));
            points.dedup_by(|p, q| p.distance_squared(*q) < 1e-12);
            if !required && (points.len() < 2 || points.len() % 2 != 0) {
                continue;
            }
            if points.len() < 2 || points.len() % 2 != 0 {
                return Err(CoreError::Invalid(
                    "Зазор выходит за границы грани — уменьшите ширину".into(),
                ));
            }
            for pair in points.chunks_exact(2) {
                if !required {
                    let g = MeshConnectivity::new(mesh);
                    let first = hits(mesh, &g, cut.face, pair[0]);
                    let second = hits(mesh, &g, cut.face, pair[1]);
                    if first
                        .iter()
                        .any(|x| second.iter().any(|y| x.edge == y.edge))
                    {
                        continue;
                    }
                }
                boundaries.push(KnifeCut {
                    face: cut.face,
                    first: pair[0].to_array(),
                    second: pair[1].to_array(),
                });
            }
        }
    }
    let origins = apply_tracked(mesh, &boundaries)?;
    let mut kept = Vec::new();
    let mut materials = Vec::new();
    for (f, origin) in origins.into_iter().enumerate() {
        let ring = mesh.face_vertices(f).unwrap();
        let remove = bands.iter().any(|&(old, a, side, along, length)| {
            old == origin
                && ring.iter().all(|&v| {
                    let d = Vec3::from_array(mesh.vertices[v as usize].position) - a;
                    d.dot(side).abs() <= width * 0.5 + 1e-6
                        && d.dot(along) >= -1e-6
                        && d.dot(along) <= length + 1e-6
                })
        });
        if !remove {
            kept.push(ring.to_vec());
            materials.push(mesh.face_material(f));
        }
    }
    if kept.is_empty() {
        return Err(CoreError::Invalid("Зазор удаляет всю поверхность".into()));
    }
    let t = mesh.topology.as_mut().unwrap();
    t.faces = kept.into();
    if !t.face_materials.is_empty() {
        t.face_materials = materials.into();
    }
    t.primitive = None;
    mesh.rebuild_polygon_indices()?;
    crate::recompute_normals(mesh, mesh.smooth_normals_enabled());
    mesh.validate_topology()
}

/// Cap only boundaries created by this cut, preserving pre-existing openings.
pub(crate) fn apply_gap_closed(
    mesh: &mut Mesh,
    cuts: &[KnifeCut],
    width: f32,
) -> Result<(), CoreError> {
    if width == 0. {
        return apply(mesh, cuts);
    }
    if crate::knife_caps::apply(mesh, cuts, width)? {
        return Ok(());
    }
    let original = mesh.clone();
    let graph = MeshConnectivity::new(&original);
    let old: Vec<_> = graph
        .edge_faces
        .iter()
        .filter(|(_, fs)| fs.len() == 1)
        .map(|(&(a, b), _)| {
            (
                Vec3::from_array(original.vertices[a].position),
                Vec3::from_array(original.vertices[b].position),
            )
        })
        .collect();
    apply_gap(mesh, cuts, width)?;
    let graph = MeshConnectivity::new(mesh);
    let edges = graph
        .edge_faces
        .iter()
        .filter(|(_, fs)| fs.len() == 1)
        .filter_map(|(&edge, _)| {
            let p = Vec3::from_array(mesh.vertices[edge.0].position);
            let q = Vec3::from_array(mesh.vertices[edge.1].position);
            let existed = old.iter().any(|&(a, b)| {
                let d = b - a;
                [p, q].iter().all(|p| {
                    let t = (*p - a).dot(d) / d.length_squared();
                    (-1e-5..=1.00001).contains(&t) && p.distance_squared(a + d * t) < 1e-10
                })
            });
            (!existed).then_some(edge)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let boundaries: std::collections::BTreeSet<_> = graph
        .edge_faces
        .iter()
        .filter(|(_, fs)| fs.len() == 1)
        .map(|(&e, _)| e)
        .collect();
    let mut remaining = boundaries.clone();
    let mut closed = std::collections::BTreeSet::new();
    while let Some(&seed) = remaining.first() {
        let mut component = std::collections::BTreeSet::from([seed]);
        let mut vertices = std::collections::BTreeSet::from([seed.0, seed.1]);
        loop {
            let connected: Vec<_> = remaining
                .iter()
                .filter(|e| vertices.contains(&e.0) || vertices.contains(&e.1))
                .copied()
                .collect();
            if connected.is_empty() {
                break;
            }
            for e in connected {
                remaining.remove(&e);
                component.insert(e);
                vertices.extend([e.0, e.1]);
            }
        }
        if component.iter().all(|e| edges.contains(e)) {
            closed.extend(component);
        }
    }
    if !closed.is_empty() {
        crate::mesh_cleanup::apply(
            mesh,
            &crate::MeshRegion {
                edges: closed,
                ..Default::default()
            },
            crate::TopologyEdit::FillHoles,
        )?;
    }
    Ok(())
}
