//! Selected polygon interiors use the authored triangulation and scene depth.
use crate::ui::Ui;
use poseforge_core::{Mat4, Mesh, Vec3, Vec4};
use std::collections::BTreeSet;

fn clip_triangle(vertices: [Vec4; 3]) -> Vec<Vec4> {
    if vertices.iter().any(|v| !v.is_finite()) {
        return vec![];
    }
    let mut polygon = vertices.to_vec();
    for plane in [
        Vec4::new(1., 0., 0., 1.),
        Vec4::new(-1., 0., 0., 1.),
        Vec4::new(0., 1., 0., 1.),
        Vec4::new(0., -1., 0., 1.),
        Vec4::new(0., 0., 1., 0.),
        Vec4::new(0., 0., -1., 1.),
    ] {
        let mut output = Vec::with_capacity(polygon.len() + 1);
        if polygon.is_empty() {
            break;
        }
        let mut previous = *polygon.last().unwrap();
        let mut pd = plane.dot(previous);
        for &current in &polygon {
            let cd = plane.dot(current);
            if (pd >= 0.) != (cd >= 0.) {
                output.push(previous.lerp(current, (pd / (pd - cd)).clamp(0., 1.)));
            }
            if cd >= 0. {
                output.push(current);
            }
            previous = current;
            pd = cd;
        }
        polygon = output;
    }
    polygon
}

pub(super) fn draw(
    u: &mut Ui,
    mesh: &Mesh,
    selected: &BTreeSet<usize>,
    projection: Mat4,
    viewport: [f32; 4],
    xray: bool,
) {
    if selected.is_empty() {
        return;
    }
    let mut color = u.palette().accent;
    color[3] = 0.22;
    let project = |p: Vec4| {
        let ndc = p.truncate() / p.w;
        [
            viewport[0] + (ndc.x + 1.) * viewport[2] * 0.5,
            viewport[1] + (1. - ndc.y) * viewport[3] * 0.5,
            ndc.z,
        ]
    };
    let mut triangle = |indices: &[u32]| {
        for indices in indices.as_chunks::<3>().0 {
            let clip = std::array::from_fn(|i| {
                projection
                    * Vec3::from_array(mesh.vertices[indices[i] as usize].position).extend(1.)
            });
            // Most selected triangles need neither clipping nor heap allocation.
            if clip.iter().all(|p| {
                p.w > 0.000001 && p.x.abs() <= p.w && p.y.abs() <= p.w && p.z >= 0. && p.z <= p.w
            }) {
                u.mesh_triangle(clip.map(project), color, !xray);
                continue;
            }
            let clipped = clip_triangle(clip);
            if clipped.len() < 3 || clipped.iter().any(|p| p.w <= 0.000001) {
                continue;
            }
            let points: Vec<_> = clipped.iter().copied().map(project).collect();
            for i in 1..points.len() - 1 {
                u.mesh_triangle([points[0], points[i], points[i + 1]], color, !xray);
            }
        }
    };
    if let Some(topology) = &mesh.topology {
        let mut start = 0;
        for (i, face) in topology.faces.iter().enumerate() {
            let end = start + (face.len() - 2) * 3;
            if selected.contains(&i) {
                triangle(&mesh.indices[start..end]);
            }
            start = end;
        }
    } else {
        for &i in selected {
            if let Some(indices) = mesh.indices.get(i * 3..i * 3 + 3) {
                triangle(indices);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn near_plane_clipping_keeps_visible_part_inside_frustum() {
        let polygon = clip_triangle([
            Vec4::new(-0.5, -0.5, -0.4, 1.),
            Vec4::new(0.5, -0.5, 0.5, 1.),
            Vec4::new(0., 0.5, 0.5, 1.),
        ]);
        assert_eq!(polygon.len(), 4);
        assert!(polygon
            .iter()
            .all(|p| p.z >= -1e-6 && p.z <= p.w && p.x.abs() <= p.w && p.y.abs() <= p.w));
    }
    #[test]
    fn interior_fill_is_translucent_bounded_and_depth_tested_at_both_dpi() {
        let mesh = poseforge_core::PrimitiveSettings::new(poseforge_core::PrimitiveKind::Plane)
            .mesh(1, None)
            .unwrap();
        let projection = Mat4::orthographic_rh(-2., 2., -2., 2., 0.1, 10.)
            * Mat4::look_at_rh(Vec3::new(0., 3., 1.), Vec3::ZERO, Vec3::Y);
        for scale in [1., 2.] {
            let mut u = Ui::with_theme(scale, crate::ui::Theme::Dark, [0., 0.]);
            draw(
                &mut u,
                &mesh,
                &BTreeSet::from([0]),
                projection,
                [10., 20., 400., 300.],
                false,
            );
            assert_eq!(u.rects.len(), 2);
            assert!(u.rects.iter().all(|r| r.triangle.is_some()
                && r.depth.is_some()
                && (r.color[3] - 0.22).abs() < 1e-6));
        }
    }
}
