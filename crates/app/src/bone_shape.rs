//! Project bone geometry once for drawing and picking.
//! Solid and flat shapes use bounded polygon meshes and homogeneous clipping.
use glam::{Mat4, Vec2, Vec3, Vec4};
use poseforge_core::{BoneDisplay, BoneShape};
use poseforge_render::View;

pub(super) struct Face {
    pub points: Vec<Vec3>,
    pub shade: f32,
}

pub(super) struct Shape {
    pub faces: Vec<Face>,
    pub edges: Vec<[Vec2; 2]>,
    pub depth: f32,
}

impl Shape {
    pub fn new(
        head: Mat4,
        tail: Vec3,
        view: &View,
        viewport: [f32; 4],
        display: BoneDisplay,
    ) -> Option<Self> {
        let origin = head.w_axis.truncate();
        let delta = tail - origin;
        let length = delta.length();
        if !head.is_finite() || !tail.is_finite() || !length.is_finite() || length < 1e-6 {
            return None;
        }
        let direction = delta / length;
        let perpendicular = |axis: Vec3| (axis - direction * axis.dot(direction)).try_normalize();
        let forward = (view.target - view.eye).try_normalize()?;
        let right = forward.cross(view.up).try_normalize()?;
        let local_side = perpendicular(head.x_axis.truncate())
            .or_else(|| perpendicular(head.z_axis.truncate()))
            .unwrap_or_else(|| direction.any_orthonormal_vector());
        let side = if display.follow_view {
            direction
                .cross(forward)
                .try_normalize()
                .or_else(|| perpendicular(right))
                .unwrap_or(local_side)
        } else {
            local_side
        };
        let up = direction.cross(side).normalize_or_zero();
        let radius = length * 0.1;
        let (vertices, polygons, flat) =
            geometry(display.shape, origin, tail, side * radius, up * radius);
        let matrix = view.view_projection();
        let camera_up = right.cross(forward);
        let light = (-right * 0.4 + camera_up * 0.6 - forward * 0.7).normalize();
        let center = origin + delta * 0.5;
        let mut faces = Vec::with_capacity(polygons.len());
        let mut edges = std::collections::BTreeSet::new();
        for indices in polygons {
            let points: Vec<_> = indices.iter().map(|&i| vertices[i]).collect();
            let mut normal = (points[1] - points[0])
                .cross(points[2] - points[0])
                .try_normalize()?;
            let face_center = points.iter().copied().sum::<Vec3>() / points.len() as f32;
            // All solid variants are convex. Orient normals away from their
            // interior so each cap and side shares the same culling rule.
            if normal.dot(face_center - center) < 0. {
                normal = -normal;
            }
            let toward_eye = if view.orthographic.is_some() {
                -forward
            } else {
                view.eye - face_center
            };
            if !flat && normal.dot(toward_eye) <= 0. {
                continue;
            }
            let clip: Vec<_> = points.iter().map(|p| matrix * p.extend(1.)).collect();
            let projected = project_face(&clip, viewport);
            if projected.len() < 3 {
                continue;
            }
            let area: f32 = (1..projected.len() - 1)
                .map(|i| {
                    (projected[i].truncate() - projected[0].truncate())
                        .perp_dot(projected[i + 1].truncate() - projected[0].truncate())
                        .abs()
                })
                .sum();
            if area < 0.001 {
                continue;
            }
            faces.push(Face {
                points: projected,
                shade: if flat {
                    1.
                } else {
                    0.72 + 0.28 * normal.dot(light).max(0.)
                },
            });
            for i in 0..indices.len() {
                let (a, b) = (indices[i], indices[(i + 1) % indices.len()]);
                edges.insert((a.min(b), a.max(b)));
            }
        }
        if faces.is_empty() {
            return None;
        }
        let face_depth =
            |face: &Face| face.points.iter().map(|p| p.z).sum::<f32>() / face.points.len() as f32;
        faces.sort_by(|a, b| face_depth(b).total_cmp(&face_depth(a)));
        let depth = faces.iter().map(face_depth).sum::<f32>() / faces.len() as f32;
        let edges = edges
            .into_iter()
            .filter_map(|(a, b)| {
                let (a, b, _) = super::project_segment(matrix, viewport, vertices[a], vertices[b])?;
                let (a, b) = super::clip_segment(a, b, viewport)?;
                Some([a, b])
            })
            .collect();
        Some(Self {
            faces,
            edges,
            depth,
        })
    }

    /// Inside a face uses its interpolated depth. The small outside tolerance
    /// follows the tapered boundary rather than an invisible fixed-width stick.
    pub fn hit(&self, point: Vec2) -> Option<(f32, f32)> {
        self.faces
            .iter()
            .filter_map(|face| {
                for i in 1..face.points.len() - 1 {
                    let [a, b, c] = [face.points[0], face.points[i], face.points[i + 1]];
                    let (ab, ac, ap) =
                        ((b - a).truncate(), (c - a).truncate(), point - a.truncate());
                    let area = ab.perp_dot(ac);
                    if area.abs() < 1e-6 {
                        continue;
                    }
                    let u = ap.perp_dot(ac) / area;
                    let v = ab.perp_dot(ap) / area;
                    if u >= -1e-5 && v >= -1e-5 && u + v <= 1. + 1e-5 {
                        return Some((0., a.z + u * (b.z - a.z) + v * (c.z - a.z)));
                    }
                }
                let mut nearest = (f32::INFINITY, 1.);
                for i in 0..face.points.len() {
                    let (a, b) = (face.points[i], face.points[(i + 1) % face.points.len()]);
                    let delta = (b - a).truncate();
                    let t = ((point - a.truncate()).dot(delta) / delta.length_squared().max(1e-8))
                        .clamp(0., 1.);
                    let distance = point.distance(a.truncate() + delta * t);
                    if distance < nearest.0 {
                        nearest = (distance, a.z + (b.z - a.z) * t);
                    }
                }
                (nearest.0 <= 3.).then_some(nearest)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))
    }

    pub fn hit_outline(&self, point: Vec2) -> Option<(f32, f32)> {
        self.edges
            .iter()
            .map(|[a, b]| distance_to_segment(point, *a, *b))
            .filter(|distance| *distance <= 3.)
            .min_by(f32::total_cmp)
            .map(|distance| (distance, self.depth))
    }
}

/// A fixed number of convex faces, independent of screen size and DPI.
fn geometry(
    shape: BoneShape,
    head: Vec3,
    tail: Vec3,
    side: Vec3,
    up: Vec3,
) -> (Vec<Vec3>, Vec<Vec<usize>>, bool) {
    match shape {
        BoneShape::Rectangle => (
            vec![head - side, head + side, tail + side, tail - side],
            vec![vec![0, 1, 2, 3]],
            true,
        ),
        BoneShape::Triangle => (
            vec![head - side, head + side, tail],
            vec![vec![0, 1, 2]],
            true,
        ),
        BoneShape::Octahedron => {
            let ring = head.lerp(tail, 0.2);
            let vertices = vec![head, tail, ring + side, ring + up, ring - side, ring - up];
            let faces = (0..4)
                .flat_map(|i| {
                    let (current, next) = (2 + i, 2 + (i + 1) % 4);
                    [vec![0, next, current], vec![1, current, next]]
                })
                .collect();
            (vertices, faces, false)
        }
        BoneShape::Box | BoneShape::Cylinder | BoneShape::Cone => {
            let offsets: Vec<_> = if shape == BoneShape::Box {
                vec![-side - up, side - up, side + up, -side + up]
            } else {
                (0..12)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 12.;
                        side * angle.cos() + up * angle.sin()
                    })
                    .collect()
            };
            let count = offsets.len();
            let mut vertices: Vec<_> = offsets.iter().map(|offset| head + *offset).collect();
            let mut faces = vec![(0..count).collect()];
            if shape == BoneShape::Cone {
                vertices.push(tail);
                faces.extend((0..count).map(|i| vec![i, (i + 1) % count, count]));
            } else {
                vertices.extend(offsets.iter().map(|offset| tail + *offset));
                faces.push((count..2 * count).collect());
                faces.extend((0..count).map(|i| {
                    let next = (i + 1) % count;
                    vec![i, next, next + count, i + count]
                }));
            }
            (vertices, faces, false)
        }
    }
}

pub(super) fn distance_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let delta = b - a;
    let t = ((point - a).dot(delta) / delta.length_squared().max(1e-8)).clamp(0., 1.);
    point.distance(a + delta * t)
}

pub(super) fn joint_basis(global: Mat4, view: &View, viewport: [f32; 4]) -> Option<[Vec2; 2]> {
    let matrix = view.view_projection();
    let world = global.w_axis.truncate();
    let (center, _) = super::project_point(matrix, viewport, world)?;
    let clip = matrix * world.extend(1.);
    let ndc = clip.truncate() / clip.w;
    let step = matrix
        .inverse()
        .project_point3(ndc + Vec3::X * (2. / viewport[2]));
    let unit = world.distance(step);
    let project_axis = |axis: Vec3| {
        let p = world + axis.try_normalize()? * unit;
        super::project_point(matrix, viewport, p).map(|(point, _)| point - center)
    };
    Some([
        project_axis(global.x_axis.truncate())?,
        project_axis(global.y_axis.truncate())?,
    ])
}

fn project_face(vertices: &[Vec4], viewport: [f32; 4]) -> Vec<Vec3> {
    if vertices.iter().any(|p| !p.is_finite()) {
        return vec![];
    }
    let mut polygon = vertices.to_vec();
    // Clip before perspective division, including left/right/top/bottom. A
    // nearby or partly offscreen bone must never produce huge UI primitives.
    for plane in [
        Vec4::new(1., 0., 0., 1.),
        Vec4::new(-1., 0., 0., 1.),
        Vec4::new(0., 1., 0., 1.),
        Vec4::new(0., -1., 0., 1.),
        Vec4::new(0., 0., 1., 0.),
        Vec4::new(0., 0., -1., 1.),
    ] {
        if polygon.is_empty() {
            break;
        }
        let mut clipped = Vec::with_capacity(polygon.len() + 1);
        let mut previous = *polygon.last().unwrap();
        let mut previous_distance = plane.dot(previous);
        for &current in &polygon {
            let distance = plane.dot(current);
            if (distance >= 0.) != (previous_distance >= 0.) {
                clipped.push(previous.lerp(
                    current,
                    (previous_distance / (previous_distance - distance)).clamp(0., 1.),
                ));
            }
            if distance >= 0. {
                clipped.push(current);
            }
            previous = current;
            previous_distance = distance;
        }
        polygon = clipped;
    }
    if polygon.iter().any(|p| p.w <= 1e-7) {
        return vec![];
    }
    polygon
        .into_iter()
        .map(|p| {
            let ndc = p.truncate() / p.w;
            Vec3::new(
                (viewport[0] + (ndc.x + 1.) * viewport[2] * 0.5)
                    .clamp(viewport[0], viewport[0] + viewport[2]),
                (viewport[1] + (1. - ndc.y) * viewport[3] * 0.5)
                    .clamp(viewport[1], viewport[1] + viewport[3]),
                ndc.z.clamp(0., 1.),
            )
        })
        .collect()
}
