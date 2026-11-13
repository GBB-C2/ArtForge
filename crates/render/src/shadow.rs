//! Conservative, cached bounds for a directional shadow camera.
use super::GpuVertex;
use glam::{Mat4, Vec3};
use poseforge_core::{Light, LightKind, Node};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}
impl Bounds {
    /// Homogeneous half-space rejection remains conservative across the near
    /// plane, mirrored transforms, perspective and orthographic projections.
    pub fn intersects_clip(self, view_projection: Mat4) -> bool {
        let corners = self.corners().map(|p| view_projection * p.extend(1.));
        if corners.iter().any(|p| !p.is_finite()) {
            return true;
        }
        !(0..6).any(|plane| {
            corners.iter().all(|p| match plane {
                0 => p.x < -p.w,
                1 => p.x > p.w,
                2 => p.y < -p.w,
                3 => p.y > p.w,
                4 => p.z < 0.,
                _ => p.z > p.w,
            })
        })
    }
    pub fn point(point: Vec3) -> Option<Self> {
        point.is_finite().then_some(Self {
            min: point,
            max: point,
        })
    }
    pub fn include(&mut self, point: Vec3) {
        if point.is_finite() {
            self.min = self.min.min(point);
            self.max = self.max.max(point);
        }
    }
    pub fn corners(self) -> [Vec3; 8] {
        std::array::from_fn(|i| {
            Vec3::new(
                if i & 1 == 0 { self.min.x } else { self.max.x },
                if i & 2 == 0 { self.min.y } else { self.max.y },
                if i & 4 == 0 { self.min.z } else { self.max.z },
            )
        })
    }
    pub fn transformed(self, matrix: Mat4) -> Option<Self> {
        let mut bounds = None;
        for point in self.corners().map(|p| matrix.transform_point3(p)) {
            add_point(&mut bounds, point);
        }
        bounds
    }
}
pub(super) fn add_point(bounds: &mut Option<Bounds>, point: Vec3) {
    match bounds {
        Some(bounds) => bounds.include(point),
        None => *bounds = Bounds::point(point),
    }
}
pub(super) fn merge(bounds: &mut Option<Bounds>, addition: Option<Bounds>) {
    if let Some(addition) = addition {
        add_point(bounds, addition.min);
        add_point(bounds, addition.max);
    }
}

/// For skinning, a weighted vertex lies in the convex hull of its influencing
/// joints' transformed points. Per-joint local bounds therefore remain
/// conservative without reskinning every vertex on the CPU every frame.
pub(super) struct MeshBounds {
    whole: Option<Bounds>,
    rigid: Option<Bounds>,
    influences: BTreeMap<usize, Bounds>,
}
impl MeshBounds {
    pub fn new(vertices: &[GpuVertex]) -> Self {
        let mut result = Self {
            whole: None,
            rigid: None,
            influences: BTreeMap::new(),
        };
        for vertex in vertices {
            let position = Vec3::from_array(vertex.position);
            add_point(&mut result.whole, position);
            if vertex.weights.iter().sum::<f32>() <= 0.0001 {
                add_point(&mut result.rigid, position);
            } else {
                for (&joint, &weight) in vertex.joints.iter().zip(&vertex.weights) {
                    if weight > 0. {
                        if let Some(bounds) = result.influences.get_mut(&(joint as usize)) {
                            bounds.include(position);
                        } else if let Some(bounds) = Bounds::point(position) {
                            result.influences.insert(joint as usize, bounds);
                        }
                    }
                }
            }
        }
        result
    }
    pub fn world(&self, node: &Node, globals: &BTreeMap<u64, Mat4>) -> Option<Bounds> {
        let model = globals.get(&node.id).copied().unwrap_or(Mat4::IDENTITY);
        let Some(skin) = &node.skin else {
            return self.whole.and_then(|b| b.transformed(model));
        };
        let mut bounds = self.rigid.and_then(|b| b.transformed(model));
        for (index, local) in &self.influences {
            if let (Some(joint), Some(bind)) = (
                skin.joints.get(*index),
                skin.inverse_bind_matrices.get(*index),
            ) {
                let matrix = globals.get(joint).copied().unwrap_or(Mat4::IDENTITY) * *bind;
                merge(&mut bounds, local.transformed(matrix));
            }
        }
        bounds
    }
}

/// Include the finite generated floor only where scene geometry can cast onto
/// it. The editor's 100 m helper plane must not dilute a small model's shadow.
pub(super) fn include_floor_projection(bounds: &mut Bounds, direction: Vec3) {
    if direction.y >= -0.01 {
        return;
    }
    for point in bounds.corners() {
        let t = (-0.005 - point.y) / direction.y;
        if t >= 0. && t.is_finite() {
            let projected = point + direction * t;
            bounds.include(Vec3::new(
                projected.x.clamp(-50., 50.),
                -0.005,
                projected.z.clamp(-50., 50.),
            ));
        }
    }
}

pub(super) fn fit_camera(bounds: Bounds, direction: Vec3, resolution: u32) -> Option<Mat4> {
    let direction = direction.try_normalize()?;
    let center = (bounds.min + bounds.max) * 0.5;
    let radius = (bounds.max - bounds.min).length() * 0.5;
    if !radius.is_finite() || !center.is_finite() {
        return None;
    }
    let distance = radius.max(0.1) + 1.;
    let up = if direction.y.abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = Mat4::look_at_rh(center - direction * distance, center, up);
    let light = bounds.transformed(view)?;
    let margin = (radius * 0.03).max(0.02);
    let mut min = light.min - Vec3::splat(margin);
    let mut max = light.max + Vec3::splat(margin);
    // Quantized extents avoid tiny changes in a fitted map's texel spacing.
    let side = (max.x - min.x).max(max.y - min.y).max(0.1);
    let side = (side * 32.).ceil() / 32.;
    let center_xy = (min + max) * 0.5;
    let texel = side / resolution.max(1) as f32;
    let x = (center_xy.x / texel).round() * texel;
    let y = (center_xy.y / texel).round() * texel;
    min.x = x - side * 0.5 - texel;
    max.x = x + side * 0.5 + texel;
    min.y = y - side * 0.5 - texel;
    max.y = y + side * 0.5 + texel;
    let near = (-max.z).max(0.001);
    let far = (-min.z).max(near + 0.1);
    let matrix = Mat4::orthographic_rh(min.x, max.x, min.y, max.y, near, far) * view;
    matrix.is_finite().then_some(matrix)
}

/// One depth pool is shared by all lights. This bound excludes small uniforms
/// and image views, and prevents eight point lights at 2K from allocating 768 MiB.
pub(super) const MAP_BUDGET_BYTES: u64 = 128 * 1024 * 1024;
pub(super) fn allocation_layers(shadow_layers: u32) -> u32 {
    // The GLES backend infers D2 / Cube / CubeArray from square texture layer
    // counts and cannot reinterpret them through a D2Array view. One unused
    // layer avoids those target heuristics; all actual light faces stay intact.
    let count = shadow_layers.max(2);
    if count.is_multiple_of(6) {
        count + 1
    } else {
        count
    }
}
pub(super) fn effective_resolution(requested: u32, layers: u32, limit: u32) -> u32 {
    if layers == 0 {
        return 1;
    }
    let mut size = requested.min(limit).max(1);
    // Match the editor's power-of-two settings and any lower adapter limit.
    size = 1 << (31 - size.leading_zeros());
    while u64::from(size) * u64::from(size) * u64::from(allocation_layers(layers)) * 4
        > MAP_BUDGET_BYTES
    {
        size = (size / 2).max(1);
    }
    size
}

pub(super) fn face_count(light: &Light) -> usize {
    // A 180-degree spot cannot use a finite perspective projection. Near that
    // limit, six faces remain exact while the lighting shader still applies its cone.
    if matches!(light.kind, LightKind::Point | LightKind::Area)
        || (light.kind == LightKind::Spot && light.outer_cone_angle >= 89f32.to_radians())
    {
        6
    } else {
        1
    }
}

pub(super) struct Cameras {
    pub matrices: Vec<Mat4>,
    pub near: f32,
    pub far: f32,
}

pub(super) fn light_cameras(
    mut bounds: Bounds,
    light: &Light,
    resolution: u32,
    floor: bool,
) -> Option<Cameras> {
    if light.kind == LightKind::Directional {
        if floor {
            include_floor_projection(&mut bounds, light.direction.normalize_or_zero());
        }
        return Some(Cameras {
            matrices: vec![fit_camera(bounds, light.direction, resolution)?],
            near: 0.,
            far: 0.,
        });
    }
    if floor {
        // Include only reachable parts of the finite helper floor, preserving
        // precision for small assets without clipping their ground shadow.
        for point in bounds.corners() {
            let ray = point - light.position;
            if ray.y.abs() > 0.000001 {
                let t = (-0.005 - point.y) / ray.y;
                if t >= 0. && t.is_finite() {
                    let p = point + ray * t;
                    bounds.include(Vec3::new(
                        p.x.clamp(-50., 50.),
                        -0.005,
                        p.z.clamp(-50., 50.),
                    ));
                }
            }
        }
    }
    let mut far = bounds
        .corners()
        .iter()
        .map(|p| p.distance(light.position))
        .fold(0., f32::max);
    far = far.max(0.01) * 1.05 + 0.01;
    if light.range > 0. {
        far = far.min(light.range);
    }
    if !far.is_finite() || far <= 0. {
        return None;
    }
    let near = (far * 0.0001).clamp(0.00001, 0.01).min(far * 0.1);
    let faces = face_count(light);
    let matrices = if faces == 6 {
        let projection = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1., near, far);
        [
            (Vec3::X, Vec3::NEG_Y),
            (Vec3::NEG_X, Vec3::NEG_Y),
            (Vec3::Y, Vec3::Z),
            (Vec3::NEG_Y, Vec3::NEG_Z),
            (Vec3::Z, Vec3::NEG_Y),
            (Vec3::NEG_Z, Vec3::NEG_Y),
        ]
        .into_iter()
        .map(|(direction, up)| {
            projection * Mat4::look_at_rh(light.position, light.position + direction, up)
        })
        .collect()
    } else {
        let direction = light.direction.try_normalize()?;
        let up = if direction.y.abs() > 0.95 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let projection =
            Mat4::perspective_rh((light.outer_cone_angle * 2.).max(0.0001), 1., near, far);
        vec![projection * Mat4::look_at_rh(light.position, light.position + direction, up)]
    };
    matrices.iter().all(|m| m.is_finite()).then_some(Cameras {
        matrices,
        near,
        far,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitted_camera_contains_all_corners_and_ground_shadow_for_vertical_and_oblique_lights() {
        for direction in [Vec3::NEG_Y, Vec3::new(-0.5, -1., -0.4).normalize()] {
            let mut bounds = Bounds {
                min: Vec3::new(-0.4, 0., -0.4),
                max: Vec3::new(0.4, 2., 0.4),
            };
            include_floor_projection(&mut bounds, direction);
            let matrix = fit_camera(bounds, direction, 1024).unwrap();
            for corner in bounds.corners() {
                let p = matrix.project_point3(corner);
                assert!(
                    p.x.abs() < 1.001 && p.y.abs() < 1.001 && (0. ..=1.).contains(&p.z),
                    "{p:?}"
                );
            }
        }
    }
    #[test]
    fn empty_or_degenerate_light_does_not_create_nonfinite_camera() {
        let bounds = Bounds::point(Vec3::ZERO).unwrap();
        assert!(fit_camera(bounds, Vec3::ZERO, 512).is_none());
        assert!(fit_camera(bounds, Vec3::NEG_Y, 512).unwrap().is_finite());
    }

    #[test]
    fn all_point_faces_and_spot_camera_cover_their_axes_with_webgpu_depth() {
        let bounds = Bounds {
            min: Vec3::splat(-2.),
            max: Vec3::splat(2.),
        };
        let mut light = Light {
            kind: LightKind::Point,
            position: Vec3::ZERO,
            range: 10.,
            ..Light::default()
        };
        let point = light_cameras(bounds, &light, 512, false).unwrap();
        assert_eq!(point.matrices.len(), 6);
        for (matrix, axis) in point.matrices.iter().zip([
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ]) {
            let p = matrix.project_point3(axis * 2.);
            assert!(p.x.abs() < 1e-5 && p.y.abs() < 1e-5 && (0. ..1.).contains(&p.z));
        }
        light.kind = LightKind::Spot;
        light.direction = Vec3::new(0.2, -1., 0.4).normalize();
        let spot = light_cameras(bounds, &light, 1024, false).unwrap();
        assert_eq!(spot.matrices.len(), 1);
        let p = spot.matrices[0].project_point3(light.direction * 2.);
        assert!(p.x.abs() < 1e-5 && p.y.abs() < 1e-5 && (0. ..1.).contains(&p.z));
        light.outer_cone_angle = std::f32::consts::FRAC_PI_2;
        assert_eq!(
            light_cameras(bounds, &light, 512, false)
                .unwrap()
                .matrices
                .len(),
            6
        );
    }

    #[test]
    fn shadow_pool_honors_memory_and_adapter_limits_without_dropping_lights() {
        for count in 1..=48 {
            for requested in [512, 1024, 2048] {
                let size = effective_resolution(requested, count, 2048);
                assert!(size.is_power_of_two() && size <= requested);
                assert!(
                    u64::from(size) * u64::from(size) * u64::from(allocation_layers(count)) * 4
                        <= MAP_BUDGET_BYTES
                );
            }
        }
        assert_eq!(effective_resolution(2048, 6, 8192), 2048);
        assert_eq!(effective_resolution(2048, 48, 8192), 512);
        assert_eq!(effective_resolution(2048, 1, 1024), 1024);
        assert_eq!(effective_resolution(2048, 0, 8192), 1);
        for count in 0..=48 {
            let allocated = allocation_layers(count);
            assert!(allocated >= count && allocated > 1 && !allocated.is_multiple_of(6));
        }
    }
}

#[cfg(test)]
mod culling_tests {
    use super::*;
    #[test]
    fn bounds_culling_is_conservative_at_near_plane_and_for_negative_scale() {
        let view = Mat4::perspective_rh(1., 1., 0.01, 100.);
        let unit = Bounds {
            min: Vec3::splat(-0.5),
            max: Vec3::splat(0.5),
        };
        assert!(unit.intersects_clip(view));
        assert!(unit
            .transformed(Mat4::from_translation(Vec3::new(0., 0., -2.)))
            .unwrap()
            .intersects_clip(view));
        assert!(!unit
            .transformed(Mat4::from_translation(Vec3::new(20., 0., -2.)))
            .unwrap()
            .intersects_clip(view));
        assert!(!unit
            .transformed(Mat4::from_translation(Vec3::new(0., 0., 2.)))
            .unwrap()
            .intersects_clip(view));
        let mirrored = unit
            .transformed(Mat4::from_scale_rotation_translation(
                Vec3::new(-2., 1., 1.),
                glam::Quat::from_rotation_y(0.5),
                Vec3::new(0., 0., -2.),
            ))
            .unwrap();
        assert!(mirrored.intersects_clip(view));
        let ortho = Mat4::orthographic_rh(-1., 1., -1., 1., 0.01, 100.);
        assert!(mirrored.intersects_clip(ortho));
    }
}
