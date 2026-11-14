//! Screen-sized transform handles. All pointer/viewport coordinates are logical
//! pixels; the view-projection matrix uses wgpu's [0, 1] depth convention.
use crate::ui::{self, Theme, Ui};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use poseforge_core::Transform;
use poseforge_render::View;

const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
const LENGTH_PX: f32 = 82.;
const RING_RADIUS_PX: f32 = 76.;
const VIEW_RADIUS_PX: f32 = 96.;
const RING_STEPS: usize = 128;
const PICK_RADIUS: f32 = 7.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Move,
    Rotate,
    Scale,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Axis(usize),
    Center,
}

#[derive(Clone)]
struct Stroke {
    a: Vec2,
    b: Vec2,
    handle: Handle,
    front: bool,
}

#[derive(Clone)]
pub struct Gizmo {
    pub mode: Mode,
    pub origin: Vec3,
    pub axes: [Vec3; 3],
    pub length: f32,
    pub view: View,
    pub viewport: [f32; 4],
    center: Vec2,
    strokes: Vec<Stroke>,
    tips: Vec<(usize, Vec2)>,
}

fn project_unclipped(view: &View, viewport: [f32; 4], point: Vec3) -> Option<Vec2> {
    let clip = view.view_projection() * point.extend(1.);
    if !clip.is_finite() || clip.w <= 0. {
        return None;
    }
    let p = clip.truncate() / clip.w;
    if !(0.0..=1.0).contains(&p.z) {
        return None;
    }
    Some(Vec2::new(
        viewport[0] + (p.x + 1.) * viewport[2] * 0.5,
        viewport[1] + (1. - p.y) * viewport[3] * 0.5,
    ))
}

fn project(view: &View, viewport: [f32; 4], point: Vec3) -> Option<Vec2> {
    let screen = project_unclipped(view, viewport, point)?;
    ui::contains(viewport, screen.to_array()).then_some(screen)
}

/// Parameter interval of a line segment inside a logical-pixel rectangle.
pub(super) fn clip_range(a: Vec2, b: Vec2, rect: [f32; 4]) -> Option<(f32, f32)> {
    if !a.is_finite()
        || !b.is_finite()
        || rect.iter().any(|v| !v.is_finite())
        || rect[2] <= 0.
        || rect[3] <= 0.
    {
        return None;
    }
    let mut lo: f32 = 0.;
    let mut hi: f32 = 1.;
    let delta = b - a;
    for i in 0..2 {
        if delta[i].abs() < 1e-6 {
            if a[i] < rect[i] || a[i] > rect[i] + rect[i + 2] {
                return None;
            }
        } else {
            let first = (rect[i] - a[i]) / delta[i];
            let second = (rect[i] + rect[i + 2] - a[i]) / delta[i];
            lo = lo.max(first.min(second));
            hi = hi.min(first.max(second));
            if hi < lo {
                return None;
            }
        }
    }
    Some((lo, hi))
}

fn rect_visible(rect: [f32; 4], viewport: [f32; 4], exclude: [f32; 4]) -> bool {
    rect[0] >= viewport[0]
        && rect[1] >= viewport[1]
        && rect[0] + rect[2] <= viewport[0] + viewport[2]
        && rect[1] + rect[3] <= viewport[1] + viewport[3]
        && !(rect[0] < exclude[0] + exclude[2]
            && rect[0] + rect[2] > exclude[0]
            && rect[1] < exclude[1] + exclude[3]
            && rect[1] + rect[3] > exclude[1])
}

/// Clip all pieces before a single coverage pass. Applying opacity once to a
/// complete ring avoids light stitches at separately antialiased segment joins.
fn draw_segments(
    ui: &mut Ui,
    segments: &[(Vec2, Vec2)],
    width: f32,
    color: [f32; 4],
    viewport: [f32; 4],
    exclude: [f32; 4],
) {
    let pad = width * 0.5 + 1.;
    let inside = [
        viewport[0] + pad,
        viewport[1] + pad,
        viewport[2] - pad * 2.,
        viewport[3] - pad * 2.,
    ];
    let forbidden = [
        exclude[0] - pad,
        exclude[1] - pad,
        exclude[2] + pad * 2.,
        exclude[3] + pad * 2.,
    ];
    let mut clipped = Vec::with_capacity(segments.len() + 2);
    for &(a, b) in segments {
        let Some((lo, hi)) = clip_range(a, b, inside) else {
            continue;
        };
        let mut append = |start: f32, end: f32| {
            if end - start > 1e-6 {
                clipped.push((a.lerp(b, start).to_array(), a.lerp(b, end).to_array()));
            }
        };
        if exclude[2] > 0. && exclude[3] > 0. {
            if let Some((cut_lo, cut_hi)) = clip_range(a, b, forbidden) {
                append(lo, hi.min(cut_lo));
                append(lo.max(cut_hi), hi);
                continue;
            }
        }
        append(lo, hi);
    }
    ui.smooth_segments(&clipped, width, color);
}

fn draw_segment(
    ui: &mut Ui,
    a: Vec2,
    b: Vec2,
    width: f32,
    color: [f32; 4],
    viewport: [f32; 4],
    exclude: [f32; 4],
) {
    draw_segments(ui, &[(a, b)], width, color, viewport, exclude);
}

pub(crate) fn axis_color(theme: Theme, axis: usize) -> [f32; 4] {
    match theme {
        Theme::Dark => [
            [0.98, 0.43, 0.45, 1.],
            [0.43, 0.87, 0.63, 1.],
            [0.43, 0.67, 1., 1.],
        ][axis],
        Theme::Light => [
            [0.76, 0.19, 0.23, 1.],
            [0.12, 0.49, 0.28, 1.],
            [0.19, 0.39, 0.79, 1.],
        ][axis],
    }
}

/// Unprojection deliberately accepts points outside the viewport during capture.
pub fn pointer_ray(view: &View, viewport: [f32; 4], point: [f32; 2]) -> Option<(Vec3, Vec3)> {
    if viewport[2] <= 0. || viewport[3] <= 0. || point.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let inverse = view.view_projection().inverse();
    if !inverse.is_finite() {
        return None;
    }
    let x = (point[0] - viewport[0]) / viewport[2] * 2. - 1.;
    let y = 1. - (point[1] - viewport[1]) / viewport[3] * 2.;
    let unproject = |z| {
        let p = inverse * Vec4::new(x, y, z, 1.);
        (p.is_finite() && p.w.abs() > 1e-8).then(|| p.truncate() / p.w)
    };
    let origin = unproject(0.)?;
    let direction = (unproject(0.5)? - origin).try_normalize()?;
    Some((origin, direction))
}

fn line_parameter(ray: (Vec3, Vec3), origin: Vec3, axis: Vec3) -> Option<f32> {
    let cosine = ray.1.dot(axis);
    let denominator = 1. - cosine * cosine;
    if denominator < 0.005 {
        return None;
    }
    let offset = ray.0 - origin;
    let value = (axis.dot(offset) - cosine * ray.1.dot(offset)) / denominator;
    value.is_finite().then_some(value)
}
fn plane_point(ray: (Vec3, Vec3), origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let denominator = ray.1.dot(normal);
    if denominator.abs() < 0.01 {
        return None;
    }
    let t = (origin - ray.0).dot(normal) / denominator;
    let p = ray.0 + ray.1 * t;
    (t >= 0. && p.is_finite()).then_some(p)
}

pub fn uniform_parent_rotation(parent: Mat4) -> Option<Quat> {
    let basis = [
        parent.x_axis.truncate(),
        parent.y_axis.truncate(),
        parent.z_axis.truncate(),
    ];
    let sizes = basis.map(|v| v.length());
    if sizes.iter().any(|s| !s.is_finite() || *s < 1e-6) {
        return None;
    }
    let max = sizes[0].max(sizes[1]).max(sizes[2]);
    let min = sizes[0].min(sizes[1]).min(sizes[2]);
    if max - min > max * 0.001 || parent.determinant() <= 0. {
        return None;
    }
    let axes = [
        basis[0] / sizes[0],
        basis[1] / sizes[1],
        basis[2] / sizes[2],
    ];
    if axes[0].dot(axes[1]).abs() > 0.001
        || axes[0].dot(axes[2]).abs() > 0.001
        || axes[1].dot(axes[2]).abs() > 0.001
    {
        return None;
    }
    Some(Quat::from_mat3(&glam::Mat3::from_cols(axes[0], axes[1], axes[2])).normalize())
}

impl Gizmo {
    pub fn new(
        mode: Mode,
        global: Mat4,
        local: bool,
        view: View,
        viewport: [f32; 4],
    ) -> Option<Self> {
        let origin = global.w_axis.truncate();
        let center = project(&view, viewport, origin)?;
        let forward = (view.target - view.eye).try_normalize()?;
        let depth = (origin - view.eye).dot(forward);
        let visible_height = view
            .orthographic
            .map(|h| h * 2.)
            .unwrap_or(2. * depth * (view.fov_y * 0.5).tan());
        let length = visible_height / viewport[3] * LENGTH_PX;
        if !length.is_finite() || length <= 1e-6 {
            return None;
        }
        let axes = if local || mode == Mode::Scale {
            [
                global.x_axis.truncate().try_normalize()?,
                global.y_axis.truncate().try_normalize()?,
                global.z_axis.truncate().try_normalize()?,
            ]
        } else {
            AXES
        };
        let facing = if view.orthographic.is_some() {
            -forward
        } else {
            (view.eye - origin).try_normalize()?
        };
        let mut result = Self {
            mode,
            origin,
            axes,
            length,
            view,
            viewport,
            center,
            strokes: vec![],
            tips: vec![],
        };
        for (axis, direction) in axes.into_iter().enumerate() {
            let handle = Handle::Axis(axis);
            if mode == Mode::Rotate {
                let a = direction.any_orthonormal_vector();
                let b = direction.cross(a);
                let radius = length * RING_RADIUS_PX / LENGTH_PX;
                let mut previous: Option<(Vec2, Vec3)> = None;
                let mut candidates = Vec::new();
                for step in 0..=RING_STEPS {
                    let angle = step as f32 * std::f32::consts::TAU / RING_STEPS as f32;
                    let radial = a * angle.cos() + b * angle.sin();
                    let point = project_unclipped(&result.view, viewport, origin + radial * radius);
                    if let Some(p) = point {
                        if ui::contains(viewport, p.to_array()) {
                            candidates.push((p, radial.dot(facing)));
                        }
                    }
                    if let (Some((start, previous_radial)), Some(end)) = (previous, point) {
                        if let Some((lo, hi)) = clip_range(start, end, viewport) {
                            result.strokes.push(Stroke {
                                a: start.lerp(end, lo),
                                b: start.lerp(end, hi),
                                handle,
                                front: (radial + previous_radial).dot(facing) >= -1e-5,
                            });
                        }
                    }
                    previous = point.map(|p| (p, radial));
                }
                // Put labels on visible, separated portions of their own ring.
                // A front-facing Z ring must not reuse the X/Y label location.
                let preferred = [
                    Vec2::new(1., -0.25),
                    Vec2::new(-0.25, -1.),
                    Vec2::new(-0.8, 0.6),
                ][axis]
                    .normalize();
                let score = |p: Vec2, front: f32| {
                    let offset = (p - center) / RING_RADIUS_PX;
                    let separation = result
                        .tips
                        .iter()
                        .map(|(_, tip)| p.distance(*tip))
                        .fold(100., f32::min);
                    offset.length() * 2. + offset.dot(preferred) * 0.45 + front.max(0.) * 0.3
                        - (28. - separation).max(0.) * 0.2
                };
                if let Some((tip, _)) = candidates
                    .into_iter()
                    .filter(|(p, _)| {
                        rect_visible([p.x - 10., p.y - 10., 20., 20.], viewport, [0.; 4])
                    })
                    .max_by(|a, b| score(a.0, a.1).total_cmp(&score(b.0, b.1)))
                {
                    result.tips.push((axis, tip));
                }
            } else if let Some(tip) = project(&result.view, viewport, origin + direction * length) {
                if center.distance(tip) < 18. {
                    continue;
                }
                result.strokes.push(Stroke {
                    a: center.lerp(tip, 0.13),
                    b: tip,
                    handle,
                    front: true,
                });
                result.tips.push((axis, tip));
            }
        }
        if mode == Mode::Rotate {
            // This view-plane ring remains circular even for an off-center
            // perspective pivot. It is a real rotation handle, not decoration.
            for step in 0..RING_STEPS {
                let point = |step: usize| {
                    let angle = step as f32 * std::f32::consts::TAU / RING_STEPS as f32;
                    center + Vec2::new(angle.cos(), angle.sin()) * VIEW_RADIUS_PX
                };
                let (a, b) = (point(step), point(step + 1));
                if let Some((lo, hi)) = clip_range(a, b, viewport) {
                    result.strokes.push(Stroke {
                        a: a.lerp(b, lo),
                        b: a.lerp(b, hi),
                        handle: Handle::Center,
                        front: true,
                    });
                }
            }
        }
        Some(result)
    }

    pub fn hit(&self, pointer: [f32; 2]) -> Option<Handle> {
        if !ui::contains(self.viewport, pointer) {
            return None;
        }
        let p = Vec2::from_array(pointer);
        if self.mode != Mode::Rotate && (p - self.center).abs().max_element() <= 8. {
            return Some(Handle::Center);
        }
        // Edge-on rings cross the pivot. Leave that tiny area for selecting
        // the joint instead of accidentally rotating an arbitrary axis.
        if self.mode == Mode::Rotate {
            if p.distance(self.center) < 10. {
                return None;
            }
            if let Some((axis, _)) = self
                .tips
                .iter()
                .find(|(_, tip)| (p - *tip).abs().max_element() <= 10.)
            {
                return Some(Handle::Axis(*axis));
            }
        }
        self.strokes
            .iter()
            .filter_map(|stroke| {
                let segment = stroke.b - stroke.a;
                let parameter = ((p - stroke.a).dot(segment)
                    / segment.length_squared().max(0.0001))
                .clamp(0., 1.);
                let distance = p.distance(stroke.a + segment * parameter);
                // Prefer the near side only at close crossings; a clearly closer
                // visible back arc remains selectable.
                (distance <= PICK_RADIUS).then_some((
                    distance + if stroke.front { 0. } else { 1.5 },
                    stroke.handle,
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, handle)| handle)
    }

    fn handle_available(&self, handle: Handle) -> bool {
        (handle == Handle::Center && self.mode != Mode::Rotate)
            || self.strokes.iter().any(|stroke| stroke.handle == handle)
    }

    pub fn draw(
        &self,
        ui: &mut Ui,
        hovered: Option<Handle>,
        active: Option<Handle>,
        _preferred_axis: usize,
        exclude: [f32; 4],
    ) {
        let palette = ui.palette();
        let theme = ui.theme;
        let color = |handle| {
            let mut c = match handle {
                Handle::Axis(index) => axis_color(theme, index),
                Handle::Center => {
                    if self.mode == Mode::Rotate {
                        palette.muted
                    } else {
                        palette.accent
                    }
                }
            };
            if active == Some(handle) || hovered == Some(handle) {
                for (channel, text) in c[..3].iter_mut().zip(palette.text) {
                    *channel = *channel * 0.84 + text * 0.16;
                }
            } else if active.is_some() {
                c[3] *= 0.38;
            }
            c
        };
        for front in [false, true] {
            for handle in [
                Handle::Axis(0),
                Handle::Axis(1),
                Handle::Axis(2),
                Handle::Center,
            ] {
                let segments: Vec<_> = self
                    .strokes
                    .iter()
                    .filter(|s| s.front == front && s.handle == handle)
                    .map(|s| (s.a, s.b))
                    .collect();
                if segments.is_empty() {
                    continue;
                }
                let selected =
                    active == Some(handle) || (active.is_none() && hovered == Some(handle));
                let mut c = color(handle);
                let mut outline = palette.panel;
                if active.is_some() && active != Some(handle) {
                    outline[3] *= 0.38;
                }
                if !front {
                    c[3] *= if selected { 0.65 } else { 0.3 };
                    outline[3] *= 0.28;
                }
                let width = if selected {
                    3.2
                } else if !front {
                    1.15
                } else if handle == Handle::Center && self.mode == Mode::Rotate {
                    1.3
                } else {
                    2.
                };
                draw_segments(ui, &segments, width + 2., outline, self.viewport, exclude);
                draw_segments(ui, &segments, width, c, self.viewport, exclude);
            }
        }
        for &(axis, tip) in &self.tips {
            let c = color(Handle::Axis(axis));
            if self.mode == Mode::Rotate {
                let dot = [tip.x - 4., tip.y - 4., 8., 8.];
                if rect_visible(dot, self.viewport, exclude) {
                    ui.surface(dot, c);
                }
                continue;
            }
            if self.mode == Mode::Scale {
                let r = [tip.x - 5., tip.y - 5., 10., 10.];
                if rect_visible(r, self.viewport, exclude) {
                    ui.rect(r, palette.panel);
                    ui.rect([tip.x - 3., tip.y - 3., 6., 6.], c);
                }
            } else if self.mode == Mode::Move {
                let backwards = (self.center - tip).normalize() * 10.;
                let side = Vec2::new(-backwards.y, backwards.x) * 0.45;
                draw_segment(
                    ui,
                    tip,
                    tip + backwards + side,
                    3.,
                    c,
                    self.viewport,
                    exclude,
                );
                draw_segment(
                    ui,
                    tip,
                    tip + backwards - side,
                    3.,
                    c,
                    self.viewport,
                    exclude,
                );
            }
        }
        let pivot = if self.mode == Mode::Rotate {
            [self.center.x - 3., self.center.y - 3., 6., 6.]
        } else {
            [self.center.x - 6., self.center.y - 6., 12., 12.]
        };
        if rect_visible(pivot, self.viewport, exclude) {
            ui.outlined_surface(
                pivot,
                palette.panel,
                if self.mode == Mode::Rotate {
                    palette.text
                } else {
                    color(Handle::Center)
                },
            );
        }
        if self.mode == Mode::Rotate && hovered == Some(Handle::Center) && active.is_none() {
            let label = [
                self.center.x - 46.,
                self.center.y + VIEW_RADIUS_PX + 8.,
                92.,
                23.,
            ];
            if rect_visible(label, self.viewport, exclude) {
                ui.surface(label, palette.panel);
                ui.label(label[0] + 8., label[1] + 4., "По виду", 12., palette.text);
            }
        }
    }
}

#[derive(Clone)]
pub struct GizmoDrag {
    pub gizmo: Gizmo,
    pub handle: Handle,
    start: Transform,
    parent_inverse: Mat4,
    parent_rotation: Quat,
    axis: Vec3,
    scalar_start: f32,
    radial_start: Vec3,
    plane_start: Vec3,
    pointer_start: Vec2,
    rotation_tangent: Option<Vec2>,
    previous_angle: f32,
    accumulated_angle: f32,
    display_angle: f32,
    previous_scalar: f32,
    adjusted_scalar: f32,
    previous_translation: Vec3,
    adjusted_translation: Vec3,
}
impl GizmoDrag {
    pub fn begin(
        gizmo: Gizmo,
        handle: Handle,
        start: Transform,
        parent: Mat4,
        pointer: [f32; 2],
    ) -> Option<Self> {
        if !gizmo.handle_available(handle) {
            return None;
        }
        let parent_inverse = parent.inverse();
        if !parent_inverse.is_finite() {
            return None;
        }
        let parent_rotation = if gizmo.mode == Mode::Rotate {
            uniform_parent_rotation(parent)?
        } else {
            Quat::IDENTITY
        };
        let ray = pointer_ray(&gizmo.view, gizmo.viewport, pointer)?;
        let forward = (gizmo.view.target - gizmo.view.eye).normalize();
        let right = forward.cross(gizmo.view.up).normalize();
        let axis = match handle {
            Handle::Axis(i) => *gizmo.axes.get(i)?,
            Handle::Center if gizmo.mode == Mode::Rotate => -forward,
            Handle::Center => (right + right.cross(forward)).normalize(),
        };
        let scalar_start = if gizmo.mode != Mode::Rotate
            && (handle != Handle::Center || gizmo.mode == Mode::Scale)
        {
            line_parameter(ray, gizmo.origin, axis)?
        } else {
            0.
        };
        let mut rotation_tangent = None;
        let radial_start = if gizmo.mode == Mode::Rotate {
            // Keep the drag mapping fixed for its complete lifetime. A ring
            // viewed edge-on uses a stable screen tangent, never a ray-plane
            // denominator that changes sign and sends the angle through pi.
            let facing = if gizmo.view.orthographic.is_some() {
                -forward
            } else {
                (gizmo.view.eye - gizmo.origin).try_normalize()?
            };
            if axis.dot(facing).abs() < 0.18 || ray.1.dot(axis).abs() < 0.1 {
                let radial = (facing - axis * facing.dot(axis)).try_normalize()?;
                let tangent = axis.cross(radial);
                let radius = gizmo.length * RING_RADIUS_PX / LENGTH_PX;
                let screen = project_unclipped(
                    &gizmo.view,
                    gizmo.viewport,
                    gizmo.origin + tangent * radius,
                )? - gizmo.center;
                if screen.length_squared() < 16. {
                    return None;
                }
                rotation_tangent = Some(screen / screen.length_squared());
                radial
            } else {
                let point = plane_point(ray, gizmo.origin, axis)?;
                let radial = point - gizmo.origin;
                if Vec2::from_array(pointer).distance(gizmo.center) < 4. {
                    return None;
                }
                radial.try_normalize()?
            }
        } else {
            Vec3::ZERO
        };
        let plane_start = if handle == Handle::Center && gizmo.mode == Mode::Move {
            plane_point(ray, gizmo.origin, forward)?
        } else {
            Vec3::ZERO
        };
        Some(Self {
            gizmo,
            handle,
            start,
            parent_inverse,
            parent_rotation,
            axis,
            scalar_start,
            radial_start,
            plane_start,
            pointer_start: Vec2::from_array(pointer),
            rotation_tangent,
            previous_angle: 0.,
            accumulated_angle: 0.,
            display_angle: 0.,
            previous_scalar: 0.,
            adjusted_scalar: 0.,
            previous_translation: Vec3::ZERO,
            adjusted_translation: Vec3::ZERO,
        })
    }

    pub fn retarget_translation(&self,target:Vec3)->(Transform,Vec3) {
        let mut delta=target-self.gizmo.origin;
        if let Handle::Axis(axis)=self.handle {let direction=self.gizmo.axes[axis];delta=direction*delta.dot(direction);}
        let mut transform=self.start;transform.translation+=self.parent_inverse.transform_vector3(delta);
        (transform,self.gizmo.origin+delta)
    }

    pub fn at_start(&self, transform: Transform, target: Vec3) -> bool {
        transform
            .translation
            .distance_squared(self.start.translation)
            < 1e-12
            && transform.scale.distance_squared(self.start.scale) < 1e-12
            && transform
                .rotation
                .normalize()
                .dot(self.start.rotation.normalize())
                .abs()
                > 1. - 1e-7
            && target.distance_squared(self.gizmo.origin) < 1e-12
    }

    /// Derive every preview from the original transform, never from the previous
    /// preview. Returns both the local transform and world-space IK target.
    pub fn update(
        &mut self,
        pointer: [f32; 2],
        fine: bool,
        snap: bool,
    ) -> Option<(Transform, Vec3, String)> {
        let ray = pointer_ray(&self.gizmo.view, self.gizmo.viewport, pointer)?;
        let precision = if fine { 0.1 } else { 1. };
        let quantize = |value: f32, step: f32| {
            if snap {
                (value / step).round() * step
            } else {
                value
            }
        };
        let mut transform = self.start;
        let mut target = self.gizmo.origin;
        let description = match self.gizmo.mode {
            Mode::Move => {
                let delta = if self.handle == Handle::Center {
                    let forward = (self.gizmo.view.target - self.gizmo.view.eye).normalize();
                    let right = forward.cross(self.gizmo.view.up).normalize();
                    let up = right.cross(forward);
                    let raw = plane_point(ray, self.gizmo.origin, forward)? - self.plane_start;
                    self.adjusted_translation += (raw - self.previous_translation) * precision;
                    self.previous_translation = raw;
                    let value = self.adjusted_translation;
                    right * quantize(value.dot(right), 0.1) + up * quantize(value.dot(up), 0.1)
                } else {
                    let raw =
                        line_parameter(ray, self.gizmo.origin, self.axis)? - self.scalar_start;
                    self.adjusted_scalar += (raw - self.previous_scalar) * precision;
                    self.previous_scalar = raw;
                    self.axis * quantize(self.adjusted_scalar, 0.1)
                };
                target += delta;
                transform.translation += self.parent_inverse.transform_vector3(delta);
                format!(
                    "Перемещение: {:.3}, {:.3}, {:.3}",
                    delta.x, delta.y, delta.z
                )
            }
            Mode::Rotate => {
                let (angle, step) = if let Some(tangent) = self.rotation_tangent {
                    let angle = (Vec2::from_array(pointer) - self.pointer_start).dot(tangent);
                    (angle, angle - self.previous_angle)
                } else {
                    // Passing through the pivot has no defined angle. Retain
                    // the last valid preview until the pointer has a radius.
                    if Vec2::from_array(pointer).distance(self.gizmo.center) < 4. {
                        return None;
                    }
                    let radial = (plane_point(ray, self.gizmo.origin, self.axis)?
                        - self.gizmo.origin)
                        .try_normalize()?;
                    let angle = self
                        .axis
                        .dot(self.radial_start.cross(radial))
                        .atan2(self.radial_start.dot(radial));
                    let step = (angle - self.previous_angle + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    (angle, step)
                };
                self.accumulated_angle += step * precision;
                self.previous_angle = angle;
                let angle = quantize(self.accumulated_angle, 15f32.to_radians());
                self.display_angle = angle;
                let local_axis = self.parent_rotation.inverse() * self.axis;
                transform.rotation =
                    (Quat::from_axis_angle(local_axis, angle) * self.start.rotation).normalize();
                let axis_name = match self.handle {
                    Handle::Axis(i) => ["X", "Y", "Z"][i],
                    Handle::Center => "по виду",
                };
                format!("Поворот {axis_name}: {:.1}°", angle.to_degrees())
            }
            Mode::Scale => {
                let raw = (line_parameter(ray, self.gizmo.origin, self.axis)? - self.scalar_start)
                    / self.gizmo.length;
                self.adjusted_scalar += (raw - self.previous_scalar) * precision;
                self.previous_scalar = raw;
                let delta = self.adjusted_scalar;
                let factor = (1. + quantize(delta, 0.1)).clamp(0.01, 100.);
                match self.handle {
                    Handle::Axis(index) => transform.scale[index] *= factor,
                    Handle::Center => transform.scale *= factor,
                }
                format!("Масштаб: {:.3}×", factor)
            }
        };
        let hint = if snap {
            match self.gizmo.mode {
                Mode::Rotate => "привязка 15°",
                Mode::Move => "привязка 0,1",
                Mode::Scale => "шаг масштаба 0,1",
            }
        } else if fine {
            "точно"
        } else {
            ""
        };
        transform.is_valid().then(|| {
            (
                transform,
                target,
                if hint.is_empty() {
                    description
                } else {
                    format!("{description} · {hint}")
                },
            )
        })
    }

    /// Draw against the captured gizmo, after its rings, for a consistent pivot
    /// and constraint space throughout the gesture.
    pub fn draw_feedback(&self, ui: &mut Ui, exclude: [f32; 4]) {
        if self.gizmo.mode != Mode::Rotate {
            return;
        }
        let palette = ui.palette();
        let c = match self.handle {
            Handle::Axis(i) => axis_color(ui.theme, i),
            Handle::Center => palette.text,
        };
        let radius = self.gizmo.length / LENGTH_PX
            * if self.handle == Handle::Center {
                VIEW_RADIUS_PX
            } else {
                RING_RADIUS_PX
            };
        let point = |angle: f32| {
            project_unclipped(
                &self.gizmo.view,
                self.gizmo.viewport,
                self.gizmo.origin
                    + Quat::from_axis_angle(self.axis, angle) * self.radial_start * radius,
            )
        };
        let mut guide = c;
        guide[3] = 0.55;
        if let Some(p) = point(0.) {
            draw_segment(
                ui,
                self.gizmo.center,
                p,
                1.,
                guide,
                self.gizmo.viewport,
                exclude,
            );
        }
        if let Some(p) = point(self.display_angle) {
            draw_segment(
                ui,
                self.gizmo.center,
                p,
                1.8,
                c,
                self.gizmo.viewport,
                exclude,
            );
        }
        let shown_angle = self
            .display_angle
            .clamp(-std::f32::consts::TAU, std::f32::consts::TAU);
        let steps = (shown_angle.abs() / std::f32::consts::TAU * RING_STEPS as f32).ceil() as usize;
        let mut previous = point(0.);
        let mut sweep = Vec::with_capacity(steps);
        for step in 1..=steps {
            let next = point(shown_angle * step as f32 / steps as f32);
            if let (Some(a), Some(b)) = (previous, next) {
                sweep.push((a, b));
            }
            previous = next;
        }
        draw_segments(ui, &sweep, 4., c, self.gizmo.viewport, exclude);
        let text = format!("{:+.1}°", self.display_angle.to_degrees());
        let w = (text.chars().count() as f32 * 7.8 + 20.).max(100.);
        let v = self.gizmo.viewport;
        if v[2] < w + 8. || v[3] < 36. {
            return;
        }
        let x = (self.gizmo.center.x - w * 0.5).clamp(v[0] + 4., v[0] + v[2] - w - 4.);
        let mut y = self.gizmo.center.y + VIEW_RADIUS_PX + 12.;
        if y + 28. > v[1] + v[3] - 4. {
            y = self.gizmo.center.y - VIEW_RADIUS_PX - 40.;
        }
        y = y.clamp(v[1] + 4., v[1] + v[3] - 32.);
        let badge = [x, y, w, 28.];
        if rect_visible(badge, v, exclude) {
            ui.outlined_surface(badge, palette.panel, c);
            ui.label(x + 10., y + 6., text, 13., c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(ortho: bool) -> View {
        View {
            eye: Vec3::new(0., 0., 6.),
            target: Vec3::ZERO,
            viewport: [40., 60., 800., 600.],
            orthographic: ortho.then_some(3.),
            ..View::default()
        }
    }
    #[test]
    fn logical_rays_support_perspective_and_orthographic_and_viewport_offset() {
        for ortho in [false, true] {
            let v = view(ortho);
            let ray = pointer_ray(&v, v.viewport, [440., 360.]).unwrap();
            assert!(ray.1.distance(Vec3::NEG_Z) < 0.0001);
            let p = plane_point(ray, Vec3::ZERO, Vec3::Z).unwrap();
            assert!(p.length() < 0.0001);
        }
    }
    #[test]
    fn handles_keep_screen_size_when_zooming_and_at_double_dpi() {
        for distance in [2., 6., 30.] {
            let mut v = view(false);
            v.eye.z = distance;
            let a = Gizmo::new(Mode::Move, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
            let tip = a.tips.iter().find(|(axis, _)| *axis == 0).unwrap().1;
            assert!((tip.distance(a.center) - 82.).abs() < 0.002);
            let logical = v.viewport;
            v.viewport = v.viewport.map(|n| n * 2.);
            let b = Gizmo::new(Mode::Move, Mat4::IDENTITY, false, v, logical).unwrap();
            assert!(b.center.distance(a.center) < 0.001);
            assert!(b.tips[0].1.distance(a.tips[0].1) < 0.001);
        }
    }
    #[test]
    fn axis_translation_uses_parent_inverse_and_cancelable_original_snapshot() {
        let v = view(true);
        let parent = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.),
            Quat::from_rotation_z(0.6),
            Vec3::ZERO,
        );
        let g = Gizmo::new(Mode::Move, parent, false, v.clone(), v.viewport).unwrap();
        let mut drag = GizmoDrag::begin(
            g,
            Handle::Axis(0),
            Transform::default(),
            parent,
            [480., 360.],
        )
        .unwrap();
        let (first, world, _) = drag.update([530., 360.], false, false).unwrap();
        assert!((world.x - 0.5).abs() < 0.0001);
        assert!(parent.transform_vector3(first.translation).distance(world) < 0.0001);
        let (again, _, _) = drag.update([530., 360.], false, false).unwrap();
        assert_eq!(first, again);
        let (reset, _, _) = drag.update([480., 360.], false, false).unwrap();
        assert!(reset.translation.length() < 0.0001);
    }
    #[test]
    fn near_end_on_axis_is_not_pickable_and_nonuniform_rotation_is_rejected() {
        let v = view(false);
        let g = Gizmo::new(Mode::Move, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
        assert!(g.tips.iter().all(|(axis, _)| *axis != 2));
        assert!(uniform_parent_rotation(Mat4::from_scale(Vec3::new(1., 2., 1.))).is_none());
        assert!(line_parameter((Vec3::Z, Vec3::NEG_Z), Vec3::ZERO, Vec3::Z).is_none());
    }
    #[test]
    fn snap_and_scale_preserve_finite_nonzero_transforms() {
        let v = view(true);
        let g = Gizmo::new(Mode::Scale, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
        let mut drag = GizmoDrag::begin(
            g,
            Handle::Axis(0),
            Transform::default(),
            Mat4::IDENTITY,
            [480., 360.],
        )
        .unwrap();
        let (t, _, _) = drag.update([-5000., 360.], false, true).unwrap();
        assert_eq!(t.scale.x, 0.01);
        assert_eq!(t.scale.y, 1.);
        assert!(t.is_valid());
    }
    #[test]
    fn rotation_continues_across_wrap_and_uses_parent_rotation() {
        let v = view(true);
        let parent = Mat4::from_rotation_x(0.7);
        let g = Gizmo::new(Mode::Rotate, parent, false, v.clone(), v.viewport).unwrap();
        let pointer = |degrees: f32| {
            let angle = degrees.to_radians();
            project(&v, v.viewport, Vec3::new(angle.cos(), angle.sin(), 0.))
                .unwrap()
                .to_array()
        };
        let mut drag = GizmoDrag::begin(
            g,
            Handle::Axis(2),
            Transform::default(),
            parent,
            pointer(170.),
        )
        .unwrap();
        for angle in [-170., 0., 170.] {
            drag.update(pointer(angle), false, false).unwrap();
        }
        let (result, _, feedback) = drag.update(pointer(-170.), false, false).unwrap();
        assert!(feedback.contains("380.0"));
        let world_rotation = Quat::from_rotation_x(0.7) * result.rotation;
        let expected = Quat::from_rotation_z(20f32.to_radians()) * Quat::from_rotation_x(0.7);
        assert!(world_rotation.dot(expected).abs() > 0.99999);
    }
    #[test]
    fn free_move_snap_stays_on_camera_plane() {
        let mut v = view(false);
        v.eye = Vec3::new(4., 3., 6.);
        let g = Gizmo::new(Mode::Move, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
        let center = g.center.to_array();
        let mut drag = GizmoDrag::begin(
            g,
            Handle::Center,
            Transform::default(),
            Mat4::IDENTITY,
            center,
        )
        .unwrap();
        let (result, target, _) = drag
            .update([center[0] + 31., center[1] - 17.], false, true)
            .unwrap();
        assert!(target.dot((v.target - v.eye).normalize()).abs() < 0.0001);
        assert_eq!(result.translation, target);
    }
    #[test]
    fn changing_precision_during_drag_does_not_jump() {
        let v = view(true);
        let g = Gizmo::new(Mode::Move, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
        let mut drag = GizmoDrag::begin(
            g,
            Handle::Axis(0),
            Transform::default(),
            Mat4::IDENTITY,
            [480., 360.],
        )
        .unwrap();
        let (normal, _, _) = drag.update([530., 360.], false, false).unwrap();
        let (fine, _, _) = drag.update([530., 360.], true, false).unwrap();
        assert_eq!(normal, fine);
        let (fine, _, _) = drag.update([535., 360.], true, false).unwrap();
        assert!((fine.translation.x - 0.505).abs() < 0.00001);
        let (normal, _, _) = drag.update([535., 360.], false, false).unwrap();
        assert_eq!(normal, fine);
    }
    #[test]
    fn every_rotation_axis_and_view_ring_are_available_in_canonical_views() {
        for ortho in [false, true] {
            for (eye, up) in [
                (Vec3::new(0., 0., 6.), Vec3::Y),
                (Vec3::new(6., 0., 0.), Vec3::Y),
                (Vec3::new(0., 6., 0.), Vec3::Z),
            ] {
                let mut v = view(ortho);
                v.eye = eye;
                v.up = up;
                let g =
                    Gizmo::new(Mode::Rotate, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
                for axis in 0..3 {
                    assert!(
                        g.handle_available(Handle::Axis(axis)),
                        "missing axis {axis}"
                    );
                    let tip = g.tips.iter().find(|(i, _)| *i == axis).unwrap().1;
                    assert_eq!(g.hit(tip.to_array()), Some(Handle::Axis(axis)));
                    assert!(GizmoDrag::begin(
                        g.clone(),
                        Handle::Axis(axis),
                        Transform::default(),
                        Mat4::IDENTITY,
                        tip.to_array()
                    )
                    .is_some());
                }
                assert_eq!(
                    g.hit((g.center + Vec2::X * VIEW_RADIUS_PX).to_array()),
                    Some(Handle::Center)
                );
                assert_eq!(g.hit(g.center.to_array()), None);
            }
        }
    }
    #[test]
    fn edge_on_rotation_is_continuous_and_precision_does_not_jump() {
        for ortho in [false, true] {
            let v = view(ortho);
            let g = Gizmo::new(Mode::Rotate, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
            let start = g.center + Vec2::new(0., -RING_RADIUS_PX);
            let mut drag = GizmoDrag::begin(
                g,
                Handle::Axis(0),
                Transform::default(),
                Mat4::IDENTITY,
                start.to_array(),
            )
            .unwrap();
            assert!(drag.rotation_tangent.is_some());
            let quarter = start + Vec2::Y * RING_RADIUS_PX * std::f32::consts::FRAC_PI_2;
            let (rotated, _, _) = drag.update(quarter.to_array(), false, false).unwrap();
            assert!(
                rotated
                    .rotation
                    .dot(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .abs()
                    > 0.99999
            );
            let (unchanged, _, _) = drag.update(quarter.to_array(), true, false).unwrap();
            assert_eq!(rotated, unchanged);
            let (precise, _, _) = drag
                .update((quarter + Vec2::Y * 7.6).to_array(), true, false)
                .unwrap();
            assert!(
                precise
                    .rotation
                    .dot(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 + 0.01))
                    .abs()
                    > 0.99999
            );
            let (snapped, _, feedback) = drag
                .update((quarter + Vec2::Y * 7.6).to_array(), false, true)
                .unwrap();
            assert!(snapped.rotation.dot(rotated.rotation).abs() > 0.99999);
            assert!(feedback.contains("15°"));
            let far = quarter + Vec2::Y * 2000.;
            assert!(drag
                .update(far.to_array(), false, false)
                .unwrap()
                .0
                .is_valid());
        }
    }
    #[test]
    fn view_rotation_has_correct_sign_parent_space_and_full_turn_tracking() {
        let v = view(true);
        let parent_rotation = Quat::from_rotation_x(0.7);
        let parent = Mat4::from_rotation_translation(parent_rotation, Vec3::ZERO);
        let g = Gizmo::new(Mode::Rotate, parent, false, v.clone(), v.viewport).unwrap();
        let center = g.center;
        let pointer = |angle: f32| {
            let angle = angle.to_radians();
            (center + Vec2::new(angle.cos(), -angle.sin()) * VIEW_RADIUS_PX).to_array()
        };
        let mut drag =
            GizmoDrag::begin(g, Handle::Center, Transform::default(), parent, pointer(0.)).unwrap();
        let (quarter, _, _) = drag.update(pointer(90.), false, false).unwrap();
        let expected = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2) * parent_rotation;
        assert!((parent_rotation * quarter.rotation).dot(expected).abs() > 0.99999);
        for angle in [179., -90., 0., 90.] {
            drag.update(pointer(angle), false, false).unwrap();
        }
        assert!((drag.display_angle.to_degrees() - 450.).abs() < 0.001);
    }
    #[test]
    fn pivot_and_nonfinite_pointer_leave_rotation_preview_unchanged() {
        let v = view(true);
        let g = Gizmo::new(Mode::Rotate, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
        let center = g.center;
        let mut drag = GizmoDrag::begin(
            g,
            Handle::Axis(2),
            Transform::default(),
            Mat4::IDENTITY,
            (center + Vec2::X * RING_RADIUS_PX).to_array(),
        )
        .unwrap();
        assert!(drag.update(center.to_array(), false, false).is_none());
        assert!(drag.update([f32::NAN, 360.], false, false).is_none());
        assert_eq!(drag.display_angle, 0.);
        let (result, _, _) = drag
            .update((center - Vec2::Y * RING_RADIUS_PX).to_array(), false, false)
            .unwrap();
        assert!(
            result
                .rotation
                .dot(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                .abs()
                > 0.99999
        );
    }
    #[test]
    fn ring_crossing_prefers_visible_front_segment_without_hiding_back_arcs() {
        let v = view(true);
        let mut g = Gizmo::new(Mode::Rotate, Mat4::IDENTITY, false, v.clone(), v.viewport).unwrap();
        g.tips.clear();
        let p = g.center + Vec2::splat(30.);
        g.strokes = vec![
            Stroke {
                a: p - Vec2::X * 12.,
                b: p + Vec2::X * 12.,
                handle: Handle::Axis(0),
                front: false,
            },
            Stroke {
                a: p - Vec2::Y * 12.,
                b: p + Vec2::Y * 12.,
                handle: Handle::Axis(1),
                front: true,
            },
        ];
        assert_eq!(g.hit(p.to_array()), Some(Handle::Axis(1)));
        assert_eq!(g.hit((p + Vec2::X * 11.).to_array()), Some(Handle::Axis(0)));
    }
    #[test]
    fn rotation_render_and_feedback_stay_inside_viewport_and_outside_rail() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 2.] {
                let mut v = view(true);
                v.viewport = [40., 60., 240., 220.];
                let global = Mat4::from_translation(Vec3::new(-2.5, 1.9, 0.));
                let g = Gizmo::new(Mode::Rotate, global, false, v.clone(), v.viewport).unwrap();
                let exclude = [v.viewport[0] + 6., v.viewport[1] + 6., 55., 90.];
                let mut ui = Ui::with_theme(scale, theme, [0., 0.]);
                g.draw(&mut ui, None, Some(Handle::Axis(2)), 2, exclude);
                let start = g.center + Vec2::X * VIEW_RADIUS_PX;
                let mut drag = GizmoDrag::begin(
                    g.clone(),
                    Handle::Center,
                    Transform::default(),
                    Mat4::IDENTITY,
                    start.to_array(),
                )
                .unwrap();
                drag.update(
                    (g.center - Vec2::Y * VIEW_RADIUS_PX).to_array(),
                    false,
                    false,
                )
                .unwrap();
                drag.draw_feedback(&mut ui, exclude);
                assert!(!ui.rects.is_empty());
                for rect in &ui.rects {
                    let r = rect.rect.map(|value| value / scale);
                    assert!(
                        r[0] >= v.viewport[0] - 0.001 && r[1] >= v.viewport[1] - 0.001,
                        "{r:?}"
                    );
                    assert!(
                        r[0] + r[2] <= v.viewport[0] + v.viewport[2] + 0.001,
                        "{r:?}"
                    );
                    assert!(
                        r[1] + r[3] <= v.viewport[1] + v.viewport[3] + 0.001,
                        "{r:?}"
                    );
                    assert!(
                        !(r[0] < exclude[0] + exclude[2]
                            && r[0] + r[2] > exclude[0]
                            && r[1] < exclude[1] + exclude[3]
                            && r[1] + r[3] > exclude[1]),
                        "rail overlap {r:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn rotation_radii_are_constant_with_zoom_and_dpi() {
        for distance in [2., 6., 30.] {
            let mut v = view(false);
            v.eye.z = distance;
            let logical = v.viewport;
            let a = Gizmo::new(Mode::Rotate, Mat4::IDENTITY, false, v.clone(), logical).unwrap();
            v.viewport = v.viewport.map(|n| n * 2.);
            let b = Gizmo::new(Mode::Rotate, Mat4::IDENTITY, false, v, logical).unwrap();
            assert!(a.center.distance(b.center) < 0.001);
            for g in [a, b] {
                let ring = g.strokes.iter().filter(|s| s.handle == Handle::Axis(2));
                for stroke in ring {
                    assert!((stroke.a.distance(g.center) - RING_RADIUS_PX).abs() < 0.002);
                }
                let view_ring = g.strokes.iter().filter(|s| s.handle == Handle::Center);
                for stroke in view_ring {
                    assert!((stroke.a.distance(g.center) - VIEW_RADIUS_PX).abs() < 0.002);
                }
            }
        }
    }
    #[test]
    fn batched_stroke_opacity_is_unchanged_by_a_subpixel_segment_join() {
        let a = Vec2::new(40.25, 100.5);
        let joint = Vec2::new(70.6, 100.5);
        let b = Vec2::new(120.75, 100.5);
        for scale in [1., 2.] {
            let mut whole = Ui::with_theme(scale, Theme::Light, [0., 0.]);
            let mut split = Ui::with_theme(scale, Theme::Light, [0., 0.]);
            draw_segments(
                &mut whole,
                &[(a, b)],
                2.,
                [0.2, 0.3, 0.8, 0.4],
                [0., 0., 200., 200.],
                [0.; 4],
            );
            draw_segments(
                &mut split,
                &[(a, joint), (joint, b)],
                2.,
                [0.2, 0.3, 0.8, 0.4],
                [0., 0., 200., 200.],
                [0.; 4],
            );
            let alpha = |ui: &Ui, x: i32, y: i32| {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                ui.rects
                    .iter()
                    .filter(|r| ui::contains(r.rect, p))
                    .fold(0., |alpha, r| alpha + r.color[3] * (1. - alpha))
            };
            for x in (38. * scale) as i32..(123. * scale) as i32 {
                for y in (98. * scale) as i32..(104. * scale) as i32 {
                    assert!(
                        (alpha(&whole, x, y) - alpha(&split, x, y)).abs() < 0.00002,
                        "join changed coverage at {x}, {y}"
                    );
                }
            }
        }
    }
}
