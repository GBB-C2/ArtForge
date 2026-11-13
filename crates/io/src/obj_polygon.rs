//! Bounded triangulation without flattening authored 3D corners.
use glam::{DVec2, DVec3, Vec3};

pub(crate) struct Triangulated {
    pub triangles: Vec<[usize; 3]>,
    pub nonplanar: bool,
    pub cleaned: bool,
}

pub(crate) fn triangulate(points: &[Vec3]) -> Result<Triangulated, String> {
    // Exporters sometimes repeat the closing corner or insert zero-length
    // edges. Keep indices into the original corner list, including UV seams.
    let mut corners = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        if corners.last().is_none_or(|&last| points[last] != points[i]) {
            corners.push(i);
        }
    }
    if corners.len() > 1 && points[corners[0]] == points[*corners.last().unwrap()] {
        corners.pop();
    }
    if corners.len() < 3 {
        return Err("менее трёх разных углов грани".into());
    }
    // Center and scale in f64 before predicates. World-space f32 products
    // falsely classified narrow polygons and large coordinates as touching.
    let origin = points[corners[0]].as_dvec3();
    let mut local: Vec<_> = corners
        .iter()
        .map(|&i| points[i].as_dvec3() - origin)
        .collect();
    let extent = local.iter().map(|p| p.length()).fold(0., f64::max);
    if extent == 0. {
        return Err("нулевой размер грани".into());
    }
    for p in &mut local {
        *p /= extent;
    }
    let normal: DVec3 = (0..local.len())
        .map(|i| local[i].cross(local[(i + 1) % local.len()]))
        .sum();
    let normal = normal
        .try_normalize()
        .ok_or_else(|| "вырожденная или самопересекающаяся грань".to_owned())?;
    let nonplanar = local.iter().any(|p| p.dot(normal).abs() > 1e-4);
    let mut axes = [0usize, 1, 2];
    axes.sort_by(|&a, &b| normal[b].abs().total_cmp(&normal[a].abs()));
    let mut reason = String::new();
    for axis in axes {
        let projected: Vec<_> = local
            .iter()
            .map(|p| match axis {
                0 => DVec2::new(p.y, p.z),
                1 => DVec2::new(p.x, p.z),
                _ => DVec2::new(p.x, p.y),
            })
            .collect();
        match clip(&projected) {
            Ok(triangles) => {
                return Ok(Triangulated {
                    triangles: triangles
                        .into_iter()
                        .map(|t| t.map(|i| corners[i]))
                        .collect(),
                    nonplanar,
                    cleaned: corners.len() != points.len(),
                })
            }
            Err(error) => {
                if reason.is_empty() {
                    reason = error;
                }
                // Other projections of a planar polygon cannot repair its
                // topology. Warped boundaries can overlap only in projection.
                if !nonplanar {
                    break;
                }
            }
        }
    }
    Err(reason)
}

fn cross(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}
const EPS: f64 = 1e-12;

fn on_segment(a: DVec2, b: DVec2, p: DVec2) -> bool {
    cross(b - a, p - a).abs() <= EPS
        && p.x >= a.x.min(b.x) - EPS
        && p.x <= a.x.max(b.x) + EPS
        && p.y >= a.y.min(b.y) - EPS
        && p.y <= a.y.max(b.y) + EPS
}

fn clip(points: &[DVec2]) -> Result<Vec<[usize; 3]>, String> {
    let n = points.len();
    let area: f64 = (0..n).map(|i| cross(points[i], points[(i + 1) % n])).sum();
    if area.abs() <= EPS {
        return Err("вырожденная или самопересекающаяся проекция грани".into());
    }
    let sign = area.signum();
    for i in 0..n {
        let ni = (i + 1) % n;
        if points[i].distance_squared(points[ni]) <= EPS * EPS {
            return Err("ребро вырождено в проекции".into());
        }
        for j in i + 1..n {
            let nj = (j + 1) % n;
            if ni == j || nj == i {
                continue;
            }
            let (a, b, c, d) = (points[i], points[ni], points[j], points[nj]);
            let (ac, ad, ca, cb) = (
                cross(b - a, c - a),
                cross(b - a, d - a),
                cross(d - c, a - c),
                cross(d - c, b - c),
            );
            let crosses = ((ac > EPS && ad < -EPS) || (ac < -EPS && ad > EPS))
                && ((ca > EPS && cb < -EPS) || (ca < -EPS && cb > EPS));
            if crosses
                || on_segment(a, b, c)
                || on_segment(a, b, d)
                || on_segment(c, d, a)
                || on_segment(c, d, b)
            {
                return Err("граница полигона пересекает или касается сама себя".into());
            }
        }
    }
    let mut remaining: Vec<_> = (0..n).collect();
    let mut triangles = Vec::with_capacity(n - 2);
    while remaining.len() > 3 {
        let mut ear = None;
        for i in 0..remaining.len() {
            let a = remaining[(i + remaining.len() - 1) % remaining.len()];
            let b = remaining[i];
            let c = remaining[(i + 1) % remaining.len()];
            if sign * cross(points[b] - points[a], points[c] - points[b]) <= EPS {
                continue;
            }
            let occupied = remaining.iter().any(|&q| {
                q != a
                    && q != b
                    && q != c
                    && sign * cross(points[b] - points[a], points[q] - points[a]) >= -EPS
                    && sign * cross(points[c] - points[b], points[q] - points[b]) >= -EPS
                    && sign * cross(points[a] - points[c], points[q] - points[c]) >= -EPS
            });
            if !occupied {
                ear = Some((i, [a, b, c]));
                break;
            }
        }
        if let Some((i, triangle)) = ear {
            triangles.push(triangle);
            remaining.remove(i);
        } else {
            return Err("не удалось надёжно триангулировать границу".into());
        }
    }
    if sign
        * cross(
            points[remaining[1]] - points[remaining[0]],
            points[remaining[2]] - points[remaining[0]],
        )
        <= EPS
    {
        return Err("вырожденный треугольник".into());
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
}
