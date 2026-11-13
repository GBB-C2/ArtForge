//! Constant-screen-size light direction and spotlight cone grips.
//! Gestures retain their initial projection and commit through Editor history.
use super::*;
use crate::gizmo::{Gizmo, GizmoDrag, Handle, Mode};
use glam::Vec2;
use poseforge_core::{Light, LightKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LightGrip {
    Direction,
    Inner,
    Outer,
}
#[derive(Clone)]
pub(super) struct LightShapeDrag {
    index: usize,
    original: Light,
    grip: LightGrip,
    gesture: Option<GizmoDrag>,
    axis: Vec2,
    radians_per_pixel: f32,
    previous: Vec2,
    accumulated: f32,
}
struct Shape {
    origin: Vec3,
    direction: Vec3,
    radial: Vec3,
    length: f32,
}
impl Shape {
    fn point(&self, angle: f32, sign: f32) -> Vec3 {
        self.origin
            + (self.direction * angle.cos() + self.radial * (sign * angle.sin())) * self.length
    }
    fn tip(&self) -> Vec3 {
        self.origin + self.direction * self.length * 1.25
    }
}

impl App {
    fn light_shape(&self, light: &Light) -> Option<Shape> {
        let view = self.view();
        let forward = (view.target - view.eye).try_normalize()?;
        let direction = light.direction.try_normalize()?;
        let depth = (light.position - view.eye).dot(forward);
        if depth <= 0. {
            return None;
        }
        let height = view
            .orthographic
            .map(|h| h * 2.)
            .unwrap_or(2. * depth * (view.fov_y * 0.5).tan());
        let right = forward.cross(view.up).try_normalize()?;
        let radial = (right - direction * right.dot(direction))
            .try_normalize()
            .or_else(|| (view.up - direction * view.up.dot(direction)).try_normalize())?;
        Some(Shape {
            origin: light.position,
            direction,
            radial,
            length: (height / self.viewport()[3] * 112.).max(0.0001),
        })
    }
    pub(super) fn light_shape_handles(&self) -> Vec<(LightGrip, [f32; 2])> {
        let Some(index) = self.active_light_index() else {
            return vec![];
        };
        if self.has_modal()
            || self.tool == Tool::Orbit
            || self.job.is_some()
            || self.editor.project.light_locked(index)
        {
            return vec![];
        }
        let Some(resolved) = self.evaluated_light(index) else {
            return vec![];
        };
        let light = &resolved;
        if light.kind == LightKind::Point {
            return vec![];
        }
        let Some(shape) = self.light_shape(light) else {
            return vec![];
        };
        let Some(origin) = self.project_point(light.position) else {
            return vec![];
        };
        let mut out = vec![];
        if let Some(point) = self
            .project_point(shape.tip())
            .filter(|p| self.canvas_input_at(*p))
        {
            if Vec2::from_array(point).distance(Vec2::from_array(origin)) >= 26. {
                out.push((LightGrip::Direction, point));
            }
        }
        if light.kind == LightKind::Spot {
            for (grip, angle, sign) in [
                (LightGrip::Outer, light.outer_cone_angle, 1.),
                (LightGrip::Inner, light.inner_cone_angle, -1.),
            ] {
                if let Some(point) = self
                    .project_point(shape.point(angle, sign))
                    .filter(|p| self.canvas_input_at(*p))
                {
                    if out
                        .iter()
                        .all(|(_, p)| Vec2::from_array(*p).distance(Vec2::from_array(point)) >= 15.)
                        && Vec2::from_array(point).distance(Vec2::from_array(origin)) >= 20.
                    {
                        out.push((grip, point));
                    }
                }
            }
        }
        out
    }
    pub(super) fn light_shape_pointer_down(&mut self) -> bool {
        if self.drag.is_some() || !self.canvas_input_at(self.mouse) {
            return false;
        }
        let Some((grip, _)) = self
            .light_shape_handles()
            .into_iter()
            .filter_map(|(grip, p)| {
                let d = Vec2::from_array(p).distance(Vec2::from_array(self.mouse));
                (d <= 10.).then_some((grip, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            return false;
        };
        let index = self.active_light_index().unwrap();
        let original = self.editor.project.lights[index].clone();
        let Some(resolved) = self.evaluated_light(index) else {
            return true;
        };
        let Some(shape) = self.light_shape(&resolved) else {
            return true;
        };
        let endpoint = shape.tip();
        let mut gesture = None;
        let mut axis = Vec2::X;
        let mut radians_per_pixel = 0.;
        if grip == LightGrip::Direction {
            let transform = Transform {
                translation: endpoint,
                ..Transform::default()
            };
            gesture = Gizmo::new(
                Mode::Move,
                transform.matrix(),
                false,
                self.view(),
                self.viewport(),
            )
            .and_then(|g| {
                GizmoDrag::begin(g, Handle::Center, transform, Mat4::IDENTITY, self.mouse)
            });
            if gesture.is_none() {
                return true;
            }
        } else {
            let (angle, sign) = if grip == LightGrip::Inner {
                (original.inner_cone_angle, -1.)
            } else {
                (original.outer_cone_angle, 1.)
            };
            let Some(a) = self.project_point(shape.point(angle, sign)) else {
                return true;
            };
            let Some(b) = self.project_point(shape.point(angle + 0.01, sign)) else {
                return true;
            };
            let delta = Vec2::from_array(b) - Vec2::from_array(a);
            let Some(unit) = delta.try_normalize().filter(|_| delta.length() > 0.03) else {
                self.status =
                    "Ручка видна с ребра: поверните вид или задайте угол в свойствах.".into();
                return true;
            };
            axis = unit;
            radians_per_pixel = 0.01 / delta.length();
        }
        self.editor.playing = false;
        self.editor.begin_edit(if grip == LightGrip::Direction {
            "Направить свет"
        } else {
            "Конус прожектора"
        });
        self.drag = Some(Drag::LightShape(LightShapeDrag {
            index,
            original,
            grip,
            gesture,
            axis,
            radians_per_pixel,
            previous: Vec2::from_array(self.mouse),
            accumulated: 0.,
        }));
        self.drag_button = Some(MouseButton::Left);
        true
    }
    pub(super) fn update_light_shape_drag(&mut self, mut drag: LightShapeDrag, pointer: [f32; 2]) {
        if !pointer.iter().all(|v| v.is_finite()) {
            self.drag = Some(Drag::LightShape(drag));
            return;
        }
        let mut next = drag.original.clone();
        if let Some(gesture) = &mut drag.gesture {
            if let Some((transform, target, _)) = gesture.update(
                pointer,
                self.modifiers.shift_key(),
                self.modifiers.control_key(),
            ) {
                if !gesture.at_start(transform, target) {
                    let Some(resolved) = self.evaluated_light(drag.index) else { return; };
                    let Some(inverse) = self.light_curve_frame(drag.index).map(|m| m.inverse()) else { return; };
                    if let Some(direction) = (transform.translation - resolved.position).try_normalize()
                    {
                        next.direction = inverse.transform_vector3(direction).normalize_or_zero();
                        next.target = None;
                    }
                }
            }
        } else {
            let point = Vec2::from_array(pointer);
            drag.accumulated += (point - drag.previous).dot(drag.axis)
                * drag.radians_per_pixel
                * if self.modifiers.shift_key() { 0.1 } else { 1. };
            drag.previous = point;
            if drag.accumulated.abs() > 1e-7 {
                let start = if drag.grip == LightGrip::Inner {
                    next.inner_cone_angle
                } else {
                    next.outer_cone_angle
                };
                let mut value = start + drag.accumulated;
                if self.modifiers.control_key() {
                    value = value.to_degrees().round().to_radians();
                }
                let gap = 0.1_f32.to_radians();
                if drag.grip == LightGrip::Inner {
                    next.inner_cone_angle = value.clamp(0., (next.outer_cone_angle - gap).max(0.));
                } else {
                    next.outer_cone_angle = value.clamp(
                        (next.inner_cone_angle + gap).min(std::f32::consts::FRAC_PI_2),
                        std::f32::consts::FRAC_PI_2,
                    );
                }
            }
        }
        if let Some(light) = self.editor.project.lights.get_mut(drag.index) {
            if next.direction.is_finite() && next.direction.length_squared() > 1e-8 {
                *light = next;
                self.status = match drag.grip {
                    LightGrip::Direction => "Направление света · Shift: точно · Esc: отмена".into(),
                    LightGrip::Inner => format!(
                        "Внутренний угол: {:.1}° · Shift: точно · Ctrl: 1°",
                        light.inner_cone_angle.to_degrees()
                    ),
                    LightGrip::Outer => format!(
                        "Внешний угол: {:.1}° · Shift: точно · Ctrl: 1°",
                        light.outer_cone_angle.to_degrees()
                    ),
                };
                self.update_dirty();
            }
        } else {
            self.editor.cancel_edit();
            self.drag_button = None;
            self.update_dirty();
            return;
        }
        self.drag = Some(Drag::LightShape(drag));
    }
    pub(super) fn draw_light_shape_handles(&self, u: &mut Ui) {
        let handles = self.light_shape_handles();
        if handles.is_empty() {
            return;
        }
        let Some(index) = self.active_light_index() else {
            return;
        };
        let Some(resolved) = self.evaluated_light(index) else {
            return;
        };
        let light = &resolved;
        let Some(shape) = self.light_shape(light) else {
            return;
        };
        let p = u.palette();
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        let line = |u: &mut Ui, a, b, color| {
            if let (Some(a), Some(b)) = (self.project_point(a), self.project_point(b)) {
                u.smooth_segment(a, b, 1.2, color);
            }
        };
        line(u, light.position, shape.tip(), p.slider);
        if light.kind == LightKind::Spot {
            let up = shape.direction.cross(shape.radial).normalize();
            for (angle, color) in [
                (light.inner_cone_angle, p.muted),
                (light.outer_cone_angle, p.slider),
            ] {
                let ring = |step: usize| {
                    let a = step as f32 * std::f32::consts::TAU / 48.;
                    shape.origin
                        + shape.direction * shape.length * angle.cos()
                        + (shape.radial * a.cos() + up * a.sin()) * shape.length * angle.sin()
                };
                for i in 0..48 {
                    line(u, ring(i), ring(i + 1), color);
                }
                if angle == light.outer_cone_angle {
                    for i in [0, 12, 24, 36] {
                        line(u, light.position, ring(i), color);
                    }
                }
            }
        }
        for (grip, point) in handles {
            let active = matches!(&self.drag, Some(Drag::LightShape(d)) if d.grip == grip);
            let hover = self.canvas_input_at(self.mouse)
                && Vec2::from_array(point).distance(Vec2::from_array(self.mouse)) <= 10.;
            let color = p.slider;
            let size = if active || hover { 14. } else { 12. };
            let rect = [point[0] - size * 0.5, point[1] - size * 0.5, size, size];
            u.rounded_rect(rect, if active || hover { p.text } else { p.panel }, size * 0.5);
            u.rounded_rect([rect[0]+1., rect[1]+1., size-2., size-2.], color, (size-2.) * 0.5);
            if grip == LightGrip::Direction {
                Icon::Move.draw(u, [point[0] - 4., point[1] - 4., 8., 8.], p.panel);
            } else {
                u.rect([point[0] - 1., point[1] - 3., 2., 6.], p.panel);
            }
            if hover || active {
                let label = match grip {
                    LightGrip::Direction => "Направление",
                    LightGrip::Inner => "Внутренний угол",
                    LightGrip::Outer => "Внешний угол",
                };
                u.surface([point[0] + 12., point[1] - 12., 122., 23.], p.panel);
                u.label(point[0] + 18., point[1] - 7., label, 11., p.text);
            }
        }
        clip_scrolled(u, start, self.viewport(), 0.);
    }
}
