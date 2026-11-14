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
pub(crate) fn apply(mesh: &mut Mesh, cuts: &[KnifeCut]) -> Result<(), CoreError> {
    apply_tracked(mesh, cuts).map(|_| ())
}
fn apply_tracked(mesh: &mut Mesh, cuts: &[KnifeCut]) -> Result<Vec<usize>, CoreError> {
    let mut origins: Vec<_> = (0..mesh.face_count()).collect();
    if cuts.is_empty()
        || cuts.len() > 2048
        || cuts.iter().any(|c| {
            c.face >= mesh.face_count()
                || !Vec3::from_array(c.first).is_finite()
                || !Vec3::from_array(c.second).is_finite()
        })
    {
        return Err(CoreError::Invalid(
            "Нож: нет пересечённых граней или слишком длинный разрез".into(),
        ));
    }
    let mut cuts = cuts.to_vec();
    cuts.sort_by_key(|c| std::cmp::Reverse(c.face));
    let mut previous = None;
    let mut parts = 0;
    for cut in cuts {
        if previous != Some(cut.face) {
            previous = Some(cut.face);
            parts = 0;
        }
        let graph = MeshConnectivity::new(mesh);
        let mut applied = None;
        'faces: for f in cut.face..=cut.face + parts {
            let first = hits(mesh, &graph, f, Vec3::from_array(cut.first));
            let second = hits(mesh, &graph, f, Vec3::from_array(cut.second));
            for a in &first {
                for b in &second {
                    if a.edge == b.edge {
                        continue;
                    }
                    let mut candidate = mesh.clone();
                    if crate::mesh_advanced::knife(&mut candidate, f, *a, *b).is_ok() {
                        applied = Some((candidate, f));
                        break 'faces;
                    }
                }
            }
        }
        let (candidate, f) = applied.ok_or_else(|| {
            CoreError::Invalid(
                "Линия проходит вне грани или совпадает с существующим ребром".into(),
            )
        })?;
        *mesh = candidate;
        origins.insert(f + 1, origins[f]);
        parts += 1;
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
        bands.push((cut.face, a, side));
        for shift in [-width * 0.5, width * 0.5] {
            let mut points = Vec::new();
            for i in 0..ring.len() {
                let p = Vec3::from_array(mesh.vertices[ring[i] as usize].position);
                let q =
                    Vec3::from_array(mesh.vertices[ring[(i + 1) % ring.len()] as usize].position);
                let u = (p - a).dot(side) - shift;
                let v = (q - a).dot(side) - shift;
                if u.abs() < 1e-6 {
                    points.push(p);
                }
                if u * v < 0. {
                    points.push(p.lerp(q, u / (u - v)));
                }
            }
            points.sort_by(|p, q| ((*p - a).dot(b - a)).total_cmp(&((*q - a).dot(b - a))));
            points.dedup_by(|p, q| p.distance_squared(*q) < 1e-12);
            if points.len() < 2 || points.len() % 2 != 0 {
                return Err(CoreError::Invalid(
                    "Зазор выходит за границы грани — уменьшите ширину".into(),
                ));
            }
            for pair in points.chunks_exact(2) {
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
        let remove = bands.iter().any(|&(old, a, side)| {
            old == origin
                && ring.iter().all(|&v| {
                    (Vec3::from_array(mesh.vertices[v as usize].position) - a)
                        .dot(side)
                        .abs()
                        <= width * 0.5 + 1e-6
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
