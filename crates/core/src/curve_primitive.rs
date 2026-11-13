//! Editable curve parameters generate a portable polygonal tube for existing render/export paths.
use crate::{CoreError, Mesh, PrimitiveKind, PrimitiveSettings, Vec3, Vertex};

pub(super) fn build(mesh: &mut Mesh, settings: PrimitiveSettings) -> Result<(), CoreError> {
    let closed = settings.kind == PrimitiveKind::CurveCircle;
    let steps = settings.segments[0] as usize;
    let sides = settings.segments[1] as usize;
    let p = settings.curve_points.map(Vec3::from_array);
    let sample = |t: f32| {
        if closed {
            let angle = t * std::f32::consts::TAU;
            Vec3::new(angle.cos(), angle.sin(), 0.) * settings.size[0]
        } else {
            let u = 1. - t;
            p[0] * (u*u*u) + p[1] * (3.*u*u*t) + p[2] * (3.*u*t*t) + p[3] * (t*t*t)
        }
    };
    let mut centers: Vec<_> = (0..=steps).map(|i| sample(i as f32 / steps as f32)).collect();
    if closed { centers[steps] = centers[0]; }
    if centers.windows(2).all(|pair| pair[0].distance_squared(pair[1]) < 1e-12) {
        return Err(CoreError::Invalid("Контрольные точки кривой должны задавать ненулевую длину".into()));
    }
    let mut previous_normal = Vec3::Z;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let tangent = if closed {
            let a = if i == steps { 0. } else { t * std::f32::consts::TAU };
            Vec3::new(-a.sin(), a.cos(), 0.)
        } else {
            let u = 1. - t;
            let derivative = (p[1] - p[0]) * (3.*u*u) + (p[2] - p[1]) * (6.*u*t) + (p[3] - p[2]) * (3.*t*t);
            derivative.try_normalize().or_else(|| (centers[(i + 1).min(steps)] - centers[i.saturating_sub(1)]).try_normalize()).unwrap_or(Vec3::X)
        };
        let normal = (previous_normal - tangent * previous_normal.dot(tangent)).try_normalize()
            .unwrap_or_else(|| tangent.cross(if tangent.x.abs() < 0.8 { Vec3::X } else { Vec3::Y }).normalize());
        let binormal = tangent.cross(normal).normalize();
        previous_normal = normal;
        for j in 0..=sides {
            let angle = if j == sides { 0. } else { j as f32 / sides as f32 * std::f32::consts::TAU };
            let radial = normal * angle.cos() + binormal * angle.sin();
            mesh.vertices.push(Vertex { position: (centers[i] + radial * settings.radius).to_array(), normal: radial.to_array(),
                uv: [i as f32 / steps as f32, j as f32 / sides as f32], ..Default::default() });
        }
    }
    let faces = &mut mesh.topology.as_mut().unwrap().faces;
    for i in 0..steps {
        for j in 0..sides {
            let a = (i * (sides + 1) + j) as u32;
            let b = a + (sides + 1) as u32;
            faces.push(vec![a, a + 1, b + 1, b]);
        }
    }
    if !closed {
        // Separate cap vertices keep the tube smooth without rounding its ends.
        for end in [0, steps] {
            let base = mesh.vertices.len() as u32;
            let ring = end * (sides + 1);
            let mut cap: Vec<_> = mesh.vertices[ring..ring + sides].to_vec();
            let a = Vec3::from_array(cap[0].position);
            let b = Vec3::from_array(cap[1].position);
            let c = Vec3::from_array(cap[2].position);
            let n = (b - a).cross(c - a).normalize_or_zero() * if end == 0 { -1. } else { 1. };
            for vertex in &mut cap { vertex.normal = n.to_array(); }
            mesh.vertices.extend(cap);
            let mut face: Vec<_> = (base..base + sides as u32).collect();
            if end == 0 { face.reverse(); }
            mesh.topology.as_mut().unwrap().faces.push(face);
        }
    }
    mesh.rebuild_polygon_indices()?;
    if !settings.smooth { crate::polygon_edit::flat_normals(mesh)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curve_parameters_roundtrip_and_rebuild_real_geometry() {
        for kind in [PrimitiveKind::Bezier, PrimitiveKind::CurveCircle] {
            let mut settings = PrimitiveSettings::new(kind);
            let mesh = settings.mesh(1, None).unwrap();
            mesh.validate_topology().unwrap();
            let mut edges = std::collections::BTreeMap::new();
            for face in &mesh.topology.as_ref().unwrap().faces {
                let key = |index: u32| mesh.vertices[index as usize].position.map(|v| (v * 100000.).round() as i64);
                for i in 0..face.len() {
                    let a = key(face[i]); let b = key(face[(i + 1) % face.len()]);
                    *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                }
            }
            assert!(edges.values().all(|count| *count == 2));
            assert_eq!(mesh.face_count(), 32 * 8 + if kind == PrimitiveKind::Bezier { 2 } else { 0 });
            assert!(mesh.vertices.iter().all(|v| Vec3::from_array(v.position).is_finite() && Vec3::from_array(v.normal).length() > 0.99));
            let saved = serde_json::to_string(&mesh).unwrap();
            let restored: Mesh = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored, mesh);
            settings.curve_points[1][2] = 0.75;
            if kind == PrimitiveKind::Bezier { assert_ne!(settings.mesh(1, None).unwrap().vertices, mesh.vertices); }
        }
    }
    #[test]
    fn collapsed_bezier_is_rejected_and_legacy_parameters_load() {
        let mut settings = PrimitiveSettings::new(PrimitiveKind::Bezier);
        settings.curve_points = [[0.; 3]; 4];
        assert!(settings.mesh(1, None).is_err());
        let mut json = serde_json::to_value(PrimitiveSettings::new(PrimitiveKind::Cube)).unwrap();
        json.as_object_mut().unwrap().remove("curve_points");
        let legacy: PrimitiveSettings = serde_json::from_value(json).unwrap();
        assert_eq!(legacy, PrimitiveSettings::new(PrimitiveKind::Cube));
    }
}
