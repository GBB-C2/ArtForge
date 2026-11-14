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
                        applied = Some(candidate);
                        break 'faces;
                    }
                }
            }
        }
        *mesh = applied.ok_or_else(|| {
            CoreError::Invalid(
                "Линия проходит вне грани или совпадает с существующим ребром".into(),
            )
        })?;
        parts += 1;
    }
    Ok(())
}
