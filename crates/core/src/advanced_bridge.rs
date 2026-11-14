//! Zipper stitching keeps both boundary loops intact, including unequal counts.
use crate::mesh_topology_tools::invalid;
use crate::{CoreError, Mesh, Vec3, Vertex, MAX_EDIT_TRIANGLES, MAX_EDIT_VERTICES};

fn sample(mesh: &Mesh, ring: &[usize], t: f32) -> Vertex {
    let at = t * ring.len() as f32;
    let i = at.floor() as usize % ring.len();
    crate::component_mesh::interpolate(
        mesh.vertices[ring[i]],
        mesh.vertices[ring[(i + 1) % ring.len()]],
        at.fract(),
    )
}
fn normal(mesh: &Mesh, ring: &[usize]) -> Vec3 {
    (0..ring.len())
        .map(|i| {
            Vec3::from_array(mesh.vertices[ring[i]].position).cross(Vec3::from_array(
                mesh.vertices[ring[(i + 1) % ring.len()]].position,
            ))
        })
        .sum::<Vec3>()
        .normalize_or_zero()
}
pub(crate) fn append(
    mesh: &mut Mesh,
    rings: &[Vec<usize>],
    segments: u32,
    twist: i32,
    smooth: f32,
    output: &mut Vec<Vec<u32>>,
) -> Result<(), CoreError> {
    if rings.len() != 2
        || rings.iter().any(|r| r.len() < 3)
        || !(1..=32).contains(&segments)
        || twist.unsigned_abs() > 4096
        || !smooth.is_finite()
        || !(0. ..=1.).contains(&smooth)
    {
        return Err(invalid(
            "Мост: два контура, 1–32 сегмента, плавность 0–100%",
        ));
    }
    let (n, m) = (rings[0].len(), rings[1].len());
    let count = n.max(m);
    if mesh
        .vertices
        .len()
        .saturating_add((segments as usize - 1) * count)
        > MAX_EDIT_VERTICES
        || mesh.indices.len() / 3 + (n + m + count * segments as usize) * 2 > MAX_EDIT_TRIANGLES
    {
        return Err(invalid("Слишком много сегментов моста"));
    }
    let first = &rings[0];
    let p = |v: usize| Vec3::from_array(mesh.vertices[v].position);
    let stride = m.div_ceil(256);
    let shift = (0..m)
        .step_by(stride)
        .min_by(|&a, &b| {
            let cost = |s: usize| {
                (0..n)
                    .step_by(n.div_ceil(128))
                    .map(|i| {
                        p(first[i]).distance_squared(p(rings[1][(s + m - i * m / n) % m])) as f64
                    })
                    .sum::<f64>()
            };
            cost(a).total_cmp(&cost(b))
        })
        .unwrap();
    let shift = (shift as i64 + twist as i64).rem_euclid(m as i64) as usize;
    let last: Vec<_> = (0..m).map(|i| rings[1][(shift + m - i) % m]).collect();
    let center = |r: &[usize]| r.iter().map(|&v| p(v)).sum::<Vec3>() / r.len() as f32;
    let delta = center(&last) - center(first);
    let length = delta.length();
    let tangent = |normal: Vec3| {
        let mut n = normal;
        if n.dot(delta) < 0. {
            n = -n;
        }
        if n.length_squared() < 0.5 {
            delta
        } else {
            n * length
        }
    };
    let start_tangent = tangent(-normal(mesh, first));
    let end_tangent = tangent(normal(mesh, &rings[1]));
    let mut rows = vec![first.iter().map(|&v| v as u32).collect::<Vec<_>>()];
    for s in 1..segments {
        let t = s as f32 / segments as f32;
        let mut row = Vec::new();
        for i in 0..count {
            let a = sample(mesh, first, i as f32 / count as f32);
            let b = sample(mesh, &last, i as f32 / count as f32);
            let mut v = crate::component_mesh::interpolate(a, b, t);
            let p0 = Vec3::from_array(a.position);
            let p1 = Vec3::from_array(b.position);
            let curved = p0 * (2. * t * t * t - 3. * t * t + 1.)
                + start_tangent * (t * t * t - 2. * t * t + t)
                + p1 * (-2. * t * t * t + 3. * t * t)
                + end_tangent * (t * t * t - t * t);
            v.position = p0.lerp(p1, t).lerp(curved, smooth).to_array();
            row.push(mesh.vertices.len() as u32);
            mesh.vertices.push(v);
        }
        rows.push(row);
    }
    rows.push(last.iter().map(|&v| v as u32).collect());
    for rows in rows.windows(2) {
        let (a, b) = (&rows[0], &rows[1]);
        let (mut i, mut j) = (0, 0);
        while i < a.len() || j < b.len() {
            let an = (i + 1) * b.len();
            let bn = (j + 1) * a.len();
            let face = if i < a.len() && j < b.len() && an == bn {
                let f = vec![
                    a[(i + 1) % a.len()],
                    a[i % a.len()],
                    b[j % b.len()],
                    b[(j + 1) % b.len()],
                ];
                i += 1;
                j += 1;
                f
            } else if i < a.len() && (j == b.len() || an < bn) {
                let f = vec![a[(i + 1) % a.len()], a[i % a.len()], b[j % b.len()]];
                i += 1;
                f
            } else {
                let f = vec![a[i % a.len()], b[j % b.len()], b[(j + 1) % b.len()]];
                j += 1;
                f
            };
            if crate::polygon_edit::face_normal(mesh, &face).length_squared() < 1e-18 {
                return Err(invalid(
                    "Мост схлопывается: измените скручивание или положение контуров",
                ));
            }
            output.push(face);
        }
    }
    Ok(())
}
