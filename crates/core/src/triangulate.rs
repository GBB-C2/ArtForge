//! Deterministic, bounded triangulation of editable polygon boundaries.
use crate::{CoreError, Mesh, Vec3};
pub const MAX_FACE_CORNERS: usize = 128;
fn invalid() -> CoreError {
    CoreError::Invalid("Многоугольник вырожден, пересекает себя или слишком сложен".into())
}
pub(crate) fn triangulate(mesh: &Mesh, face: &[u32]) -> Result<Vec<u32>, CoreError> {
    if !(3..=MAX_FACE_CORNERS).contains(&face.len())
        || face.iter().any(|&v| v as usize >= mesh.vertices.len())
    {
        return Err(invalid());
    }
    // Preserve the triangulation and compatibility of existing triangle/quad files.
    if face.len() <= 4 {
        return Ok((1..face.len() - 1)
            .flat_map(|i| [face[0], face[i], face[i + 1]])
            .collect());
    }
    let positions: Vec<_> = face
        .iter()
        .map(|&v| Vec3::from_array(mesh.vertices[v as usize].position))
        .collect();
    let origin = positions[0];
    let normal: Vec3 = (0..positions.len())
        .map(|i| (positions[i] - origin).cross(positions[(i + 1) % positions.len()] - origin))
        .sum();
    let axis = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        0
    } else if normal.y.abs() >= normal.z.abs() {
        1
    } else {
        2
    };
    let points: Vec<[f64; 2]> = positions
        .iter()
        .map(|p| {
            let p = *p - origin;
            [p[(axis + 1) % 3] as f64, p[(axis + 2) % 3] as f64]
        })
        .collect();
    let cross = |a: usize, b: usize, c: usize| {
        let (a, b, c) = (points[a], points[b], points[c]);
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let area: f64 = (1..face.len() - 1).map(|i| cross(0, i, i + 1)).sum();
    let extent = points.iter().flatten().fold(0_f64, |a, v| a.max(v.abs()));
    let eps = (extent * extent * 1e-12).max(1e-20);
    if !area.is_finite() || area.abs() <= eps {
        return Err(invalid());
    }
    let sign = area.signum();
    // Reject crossings before clipping; adjacent collinear boundary vertices are legal.
    for a in 0..face.len() {
        let b = (a + 1) % face.len();
        for c in a + 1..face.len() {
            let d = (c + 1) % face.len();
            if a == d || b == c {
                continue;
            }
            if cross(a, b, c) * cross(a, b, d) < -eps * eps
                && cross(c, d, a) * cross(c, d, b) < -eps * eps
            {
                return Err(invalid());
            }
        }
    }
    let mut remaining: Vec<_> = (0..face.len()).collect();
    let mut out = Vec::with_capacity((face.len() - 2) * 3);
    while remaining.len() > 3 {
        let mut ear = None;
        for i in 0..remaining.len() {
            let a = remaining[(i + remaining.len() - 1) % remaining.len()];
            let b = remaining[i];
            let c = remaining[(i + 1) % remaining.len()];
            if cross(a, b, c) * sign <= eps {
                continue;
            }
            let occupied = remaining.iter().any(|&p| {
                p != a
                    && p != b
                    && p != c
                    && cross(a, b, p) * sign >= -eps
                    && cross(b, c, p) * sign >= -eps
                    && cross(c, a, p) * sign >= -eps
            });
            if !occupied {
                ear = Some((i, a, b, c));
                break;
            }
        }
        // A collinear ear preserves boundary segmentation shared with neighboring faces.
        if ear.is_none() {
            for i in 0..remaining.len() {
                let a = remaining[(i + remaining.len() - 1) % remaining.len()];
                let b = remaining[i];
                let c = remaining[(i + 1) % remaining.len()];
                if cross(a, b, c).abs() <= eps {
                    ear = Some((i, a, b, c));
                    break;
                }
            }
        }
        let (i, a, b, c) = ear.ok_or_else(invalid)?;
        out.extend([face[a], face[b], face[c]]);
        remaining.remove(i);
    }
    out.extend(remaining.into_iter().map(|i| face[i]));
    Ok(out)
}
