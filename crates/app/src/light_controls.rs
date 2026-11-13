//! Scene-light selection and direct manipulation reuse the object gizmo solver.
//! Light positions are world coordinates; changing a directional light's anchor
//! never changes its illumination. Only its direction affects that lighting.
use super::{App, Drag, Tool};
use crate::{
    gizmo::{Gizmo, GizmoDrag, Handle, Mode},
    ui::{self, Action, Theme, Ui},
};
use glam::{Mat4, Quat, Vec2, Vec3};
use poseforge_core::{Light, LightKind, Transform};
use poseforge_render::View;
use winit::event::MouseButton;

const MAX_MARKERS: usize = 128;
const PICK_RADIUS: f32 = 13.;

#[derive(Clone)]
pub(super) struct LightDrag {
    pub(super) index: usize,
    pub(super) gesture: GizmoDrag,
    original: Light,
    curve: Option<std::sync::Arc<poseforge_core::CurveTranslation>>,
}

/// Place newly added/reframed lights inside the current view, away from the
/// selected object's center. A camera-coincident position cannot be picked.
pub(super) fn placement_in_view(view: &View) -> (Vec3, Vec3) {
    let forward = (view.target - view.eye)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    let right = forward.cross(view.up).try_normalize().unwrap_or(Vec3::X);
    let up = right.cross(forward).normalize_or_zero();
    let depth = (view.target.distance(view.eye) * 0.65).clamp(0.15, 500.);
    let half_height = view
        .orthographic
        .unwrap_or_else(|| depth * (view.fov_y * 0.5).tan())
        .clamp(0.01, 500.);
    let aspect = (view.viewport[2] / view.viewport[3].max(1.)).clamp(0.01, 100.);
    let position = view.eye
        + forward * depth
        + right * (half_height * aspect * 0.30)
        + up * (half_height * 0.22);
    let direction = (view.target - position).try_normalize().unwrap_or(forward);
    (position, direction)
}

fn light_transform(light: &Light) -> Option<Transform> {
    if !light.position.is_finite() {
        return None;
    }
    Some(Transform {
        translation: light.position,
        rotation: Quat::from_rotation_arc(Vec3::NEG_Z, light.direction.try_normalize()?),
        scale: Vec3::ONE,
    })
}

fn marker_position(view: &View, viewport: [f32; 4], position: Vec3) -> Option<Vec2> {
    if viewport.iter().any(|v| !v.is_finite()) || viewport[2] <= 0. || viewport[3] <= 0. {
        return None;
    }
    let clip = view.view_projection() * position.extend(1.);
    if !clip.is_finite() || clip.w <= 1e-6 || clip.z < 0. || clip.z > clip.w {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    let point = Vec2::new(
        viewport[0] + (ndc.x + 1.) * viewport[2] * 0.5,
        viewport[1] + (1. - ndc.y) * viewport[3] * 0.5,
    );
    (point.is_finite() && ui::contains(viewport, point.to_array())).then_some(point)
}

fn marker_indices(count: usize, selected: Option<usize>) -> impl Iterator<Item = usize> {
    (0..count.min(MAX_MARKERS)).chain(selected.filter(|&i| i >= MAX_MARKERS && i < count))
}

fn pick_marker(
    lights: &[Light],
    selected: Option<usize>,
    view: &View,
    viewport: [f32; 4],
    pointer: [f32; 2],
) -> Option<usize> {
    if !ui::contains(viewport, pointer) {
        return None;
    }
    marker_indices(lights.len(), selected)
        .filter_map(|index| {
            let light = &lights[index];
            let center = marker_position(view, viewport, light.position)?;
            let distance = center.distance(Vec2::from_array(pointer));
            (distance <= PICK_RADIUS).then_some((
                index,
                distance,
                light.position.distance(view.eye),
            ))
        })
        .min_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then_with(|| (Some(b.0) == selected).cmp(&(Some(a.0) == selected)))
                .then_with(|| a.2.total_cmp(&b.2))
                .then_with(|| a.0.cmp(&b.0))
        })
        .map(|entry| entry.0)
}

fn disc(ui: &mut Ui, center: Vec2, radius: f32, color: [f32; 4]) {
    let points: Vec<_> = (0..16)
        .map(|step| {
            let angle = step as f32 * std::f32::consts::TAU / 16.;
            (center + Vec2::new(angle.cos(), angle.sin()) * radius).to_array()
        })
        .collect();
    ui.convex_polygon(&points, color);
}

fn line(ui: &mut Ui, a: Vec2, b: Vec2, color: [f32; 4]) {
    ui.smooth_segment(a.to_array(), b.to_array(), 1.5, color);
}

fn marker_color(theme: Theme, accent: [f32; 4], selected: bool, enabled: bool) -> [f32; 4] {
    let mut color = if selected {
        accent
    } else if theme == Theme::Dark {
        [0.98, 0.78, 0.37, 0.94]
    } else {
        [0.53, 0.31, 0.05, 0.94]
    };
    if !enabled {
        color[3] *= 0.46;
    }
    color
}

fn draw_marker(ui: &mut Ui, center: Vec2, kind: LightKind, color: [f32; 4], selected: bool) {
    if selected {
        let mut halo = color;
        halo[3] *= 0.16;
        disc(ui, center, PICK_RADIUS, halo);
    }
    match kind {
        LightKind::Directional => {
            disc(ui, center, 4., color);
            for step in 0..8 {
                let angle = step as f32 * std::f32::consts::TAU / 8.;
                let radial = Vec2::new(angle.cos(), angle.sin());
                line(ui, center + radial * 7., center + radial * 11., color);
            }
        }
        LightKind::Point => {
            disc(ui, center - Vec2::Y * 2., 5.5, color);
            line(
                ui,
                center + Vec2::new(-3., 6.),
                center + Vec2::new(3., 6.),
                color,
            );
            line(
                ui,
                center + Vec2::new(-2., 9.),
                center + Vec2::new(2., 9.),
                color,
            );
        }
        LightKind::Area => {
            for (a, b) in [
                ([-8., -6.], [8., -6.]),
                ([8., -6.], [8., 6.]),
                ([8., 6.], [-8., 6.]),
                ([-8., 6.], [-8., -6.]),
            ] {
                line(
                    ui,
                    center + Vec2::from_array(a),
                    center + Vec2::from_array(b),
                    color,
                );
            }
            line(ui, center, center + Vec2::new(0., 12.), color);
        }
        LightKind::Spot => {
            let tip = center + Vec2::new(0., -7.);
            let left = center + Vec2::new(-7., 6.);
            let right = center + Vec2::new(7., 6.);
            line(ui, tip, left, color);
            line(ui, left, right, color);
            line(ui, right, tip, color);
            disc(ui, tip, 2.5, color);
        }
    }
}

fn draw_light_ray(
    ui: &mut Ui,
    view: &View,
    viewport: [f32; 4],
    light: &Light,
    center: Vec2,
    color: [f32; 4],
) {
    if light.kind == LightKind::Point {
        return;
    }
    let Some(direction) = light.direction.try_normalize() else {
        return;
    };
    let forward = (view.target - view.eye).normalize_or_zero();
    let depth = (light.position - view.eye).dot(forward);
    let visible_height = view
        .orthographic
        .map(|height| height * 2.)
        .unwrap_or(2. * depth * (view.fov_y * 0.5).tan());
    let endpoint = light.position + direction * (visible_height / viewport[3] * 48.);
    let Some(end) = marker_position(view, viewport, endpoint) else {
        return;
    };
    let Some(axis) = (end - center).try_normalize() else {
        return;
    };
    if end.distance(center) < 18. {
        return;
    }
    let side = Vec2::new(-axis.y, axis.x);
    line(ui, center + axis * 14., end, color);
    line(ui, end, end - axis * 6. + side * 3., color);
    line(ui, end, end - axis * 6. - side * 3., color);
}

fn clip_new_rectangles(ui: &mut Ui, start: usize, viewport: [f32; 4]) {
    let viewport = viewport.map(|value| value * ui.scale);
    let mut index = 0;
    ui.rects.retain_mut(|primitive| {
        index += 1;
        if index <= start {
            return true;
        }
        let r = &mut primitive.rect;
        let x = r[0].max(viewport[0]);
        let y = r[1].max(viewport[1]);
        let right = (r[0] + r[2]).min(viewport[0] + viewport[2]);
        let bottom = (r[1] + r[3]).min(viewport[1] + viewport[3]);
        *r = [x, y, right - x, bottom - y];
        r.iter().all(|value| value.is_finite()) && r[2] > 0. && r[3] > 0.
    });
}

impl App {
    pub(super) fn active_light_index(&self) -> Option<usize> {
        (self.surface.light_selected
            && self.editor.selected.is_none()
            && self.surface.light < self.editor.project.lights.len())
        .then_some(self.surface.light)
    }

    pub(super) fn clear_light_selection(&mut self) {
        self.surface.light_selected = false;
    }

    pub(super) fn select_light(&mut self, index: usize) {
        self.path_edit = None;
        if let Some(id) = self.editor.selected {
            self.last_light_target = Some(id);
        }
        let Some(light) = self.editor.project.lights.get(index) else {
            return;
        };
        self.status = match light.kind {
            LightKind::Directional => format!(
                "{} · перемещение значка и поворот направления света",
                light.name
            ),
            LightKind::Point => {
                format!("{} · потяните центр или ось гизмо", light.name)
            }
            LightKind::Spot => format!("{} · ручки: направление и углы конуса", light.name),
            LightKind::Area => format!(
                "{} · перемещение, направление и размер за ручки на краях",
                light.name
            ),
        };
        self.uv_action(ui::UvAction::Close);
        self.authoring.editing = None;
        self.scene_tools.selected.clear();
        self.editor.selected = None;
        self.editor.playing = false;
        self.animation.focused = false;
        self.surface.light = index;
        self.surface.light_selected = true;
        self.set_properties_tab(0);
        self.inspector_scroll = 0.;
        self.preferences.right_panel = true;
        self.tool = Tool::Move;
        self.ik = false;
    }

    pub(super) fn evaluated_light(&self, index: usize) -> Option<Light> {
        let light = self.editor.project.lights.get(index)?;
        if light.target.is_none() && light.curve_binding.is_none() { return Some(light.clone()); }
        let scene = self.editor.evaluated().ok()?;
        scene.lights.get(index).cloned()
    }
    pub(super) fn light_curve_frame(&self, index: usize) -> Option<Mat4> {
        let light = self.editor.project.lights.get(index)?;
        let Some(binding) = light.curve_binding else { return Some(Mat4::IDENTITY); };
        let scene = self.editor.evaluated().ok()?;
        self.editor.project.curve_binding_frame(binding, &scene.global, self.editor.time, true).ok()
    }

    fn light_gizmo(&self) -> Option<Gizmo> {
        if self.has_modal() || self.tool == Tool::Orbit {
            return None;
        }
        let index = self.active_light_index()?;
        if self.editor.project.light_locked(index) {
            return None;
        }
        let resolved = self.evaluated_light(index)?;
        let light = &resolved;
        let mode = match self.tool {
            Tool::Rotate if light.kind != LightKind::Point => Mode::Rotate,
            _ => Mode::Move,
        };
        Gizmo::new(
            mode,
            light_transform(light)?.matrix(),
            self.gizmo_local,
            self.view(),
            self.viewport(),
        )
    }

    fn begin_light_drag(&mut self, handle: Handle) {
        let Some(index) = self.active_light_index() else {
            return;
        };
        let Some(gizmo) = self.light_gizmo() else {
            return;
        };
        let original = self.editor.project.lights[index].clone();
        let Some(resolved) = self.evaluated_light(index) else {
            return;
        };
        let Some(transform) = light_transform(&resolved) else {
            return;
        };
        let Some(gesture) = GizmoDrag::begin(gizmo, handle, transform, Mat4::IDENTITY, self.mouse)
        else {
            self.status = "Ось смотрит в камеру. Потяните центр гизмо или измените ракурс.".into();
            return;
        };
        if let Handle::Axis(axis) = handle {
            self.axis = axis;
        }
        let curve = if gesture.gizmo.mode == Mode::Move && !self.path_offset_editing_for(super::curve_binding_controls::BindingTarget::Light(index)) {
            if let Some(binding) = original.curve_binding {
                let prepared = self.editor.evaluated().and_then(|scene| self.editor.project.curve_translation(
                    binding, &scene.global, self.editor.time, original.position, true));
                match prepared {
                    Ok(path) => Some(std::sync::Arc::new(path)),
                    Err(e) => { self.status = e.to_string(); return; }
                }
            } else { None }
        } else { None };
        self.editor.playing = false;
        self.editor
            .begin_edit(if gesture.gizmo.mode == Mode::Rotate {
                "Направить свет"
            } else {
                "Переместить свет"
            });
        self.drag = Some(Drag::Light(LightDrag {
            index,
            gesture,
            original,
            curve,
        }));
        self.drag_button = Some(MouseButton::Left);
    }

    /// Called after canvas/modal checks, before the Orbit/Render early return.
    pub(super) fn light_pointer_down(&mut self) -> bool {
        if self.has_modal() || !self.canvas_input_at(self.mouse) || self.drag.is_some() {
            return false;
        }
        if let Some(handle) = self.light_gizmo().and_then(|gizmo| gizmo.hit(self.mouse)) {
            self.begin_light_drag(handle);
            return true;
        }
        if self
            .selected_gizmo()
            .and_then(|gizmo| gizmo.hit(self.mouse))
            .is_some()
        {
            return false;
        }
        let Some(index) = pick_marker(
            &self.editor.evaluated().map(|e| e.lights).unwrap_or_else(|_| self.editor.project.lights.clone()),
            self.active_light_index(),
            &self.view(),
            self.viewport(),
            self.mouse,
        ) else {
            return false;
        };
        let already_selected = self.active_light_index() == Some(index);
        // The first click selects. The next center drag uses the stable viewport
        // after the inspector opens, preventing a jump when its width changes.
        if !already_selected || self.tool == Tool::Orbit {
            self.select_light(index);
        } else if self.tool != Tool::Rotate {
            self.begin_light_drag(Handle::Center);
        }
        self.redraw();
        true
    }

    pub(super) fn update_light_drag(&mut self, mut drag: LightDrag, pointer: [f32; 2]) {
        if let Some((transform, target, feedback)) = drag.gesture.update(
            pointer,
            self.modifiers.shift_key(),
            self.modifiers.control_key(),
        ) {
            let mut preview = drag.original.clone();
            if !drag.gesture.at_start(transform, target) {
                if drag.gesture.gizmo.mode == Mode::Move {
                    if let Some(path) = &drag.curve {
                        match path.position(transform.translation) {
                            Ok(position) => preview.curve_binding.as_mut().unwrap().position = position,
                            Err(e) => { self.cancel_light_gesture(e.to_string()); return; }
                        }
                    } else if preview.curve_binding.is_some() {
                        let Some(frame) = self.light_curve_frame(drag.index) else {
                            self.cancel_light_gesture("Кривая света недоступна; перемещение отменено.".into()); return;
                        };
                        preview.position = frame.inverse().transform_point3(transform.translation);
                    } else { preview.position = transform.translation; }
                } else {
                    let direction = (transform.rotation * Vec3::NEG_Z).normalize_or_zero();
                    preview.direction = if let Some(binding) = preview.curve_binding {
                        let Some(frame) = self.editor.evaluated().ok().and_then(|e|
                            self.editor.project.curve_binding_frame(binding, &e.global, self.editor.time, true).ok()) else {
                                self.cancel_light_gesture("Кривая света недоступна; поворот отменён.".into()); return;
                            };
                        frame.inverse().transform_vector3(direction).normalize_or_zero()
                    } else { direction };
                    preview.target = None;
                }
            }
            if preview.position.is_finite()
                && preview.direction.is_finite()
                && preview.direction.length_squared() > 1e-8
            {
                let Some(light) = self.editor.project.lights.get_mut(drag.index) else {
                    self.editor.cancel_edit();
                    self.drag = None;
                    self.drag_button = None;
                    self.status = "Источник света больше недоступен; перемещение отменено.".into();
                    self.update_dirty();
                    return;
                };
                *light = preview;
                self.status = if let Some(binding) = light.curve_binding.filter(|_| drag.curve.is_some()) {
                    format!("Свет по кривой · {:.1} % · Esc — отмена", binding.position * 100.)
                } else if light.kind == LightKind::Directional
                    && drag.gesture.gizmo.mode == Mode::Move
                {
                    format!("{feedback} · Солнце: освещение меняется поворотом R")
                } else {
                    format!("Свет · {feedback}")
                };
                self.update_dirty();
            }
        }
        self.drag = Some(Drag::Light(drag));
    }

    fn cancel_light_gesture(&mut self, message: String) {
        self.editor.cancel_edit(); self.drag = None; self.drag_button = None;
        self.status = message; self.update_dirty(); self.redraw();
    }

    /// Handle shortcuts which refer to the light selection instead of nodes.
    /// The caller has already resolved fields, active gestures and busy states.
    pub(super) fn light_action(&mut self, action: &Action) -> bool {
        if matches!(action, Action::Select(_)) || self.editor.selected.is_some() {
            self.clear_light_selection();
        }
        let Some(index) = self.active_light_index() else {
            return false;
        };
        match action {
            Action::Delete => self.surface_action(Action::SurfaceDeleteLight),
            Action::FrameSelected => {
                self.target = self.evaluated_light(index).map_or(self.editor.project.lights[index].position, |l| l.position);
                self.distance = self.distance.clamp(1., 4.);
            }
            Action::Scale => {
                self.tool = Tool::Move;
                self.status =
                    "Для света изменяйте силу, дальность и угол в панели «Свет». G — перемещение."
                        .into();
            }
            Action::Rotate if self.editor.project.lights[index].kind == LightKind::Point => {
                self.tool = Tool::Move;
                self.status =
                    "Точечный свет излучает во все стороны. Потяните гизмо для перемещения.".into();
            }
            Action::Key | Action::KeyPose | Action::Ik => {
                self.status =
                    "Выбран источник света. Для ключей позы и IK выберите объект или кость.".into();
            }
            _ => return false,
        }
        true
    }

    pub(super) fn draw_light_controls(&mut self, ui: &mut Ui) {
        if self.editor.selected.is_some() {
            self.clear_light_selection();
        }
        let selected = self.active_light_index();
        let viewport = self.viewport();
        if viewport.iter().any(|value| !value.is_finite()) || viewport[2] <= 0. || viewport[3] <= 0.
        {
            return;
        }
        let view = self.view();
        // Evaluate all constrained lights once, including when many markers are visible.
        let resolved = self.editor.project.lights.iter().any(|l| l.target.is_some() || l.curve_binding.is_some())
            .then(|| self.editor.evaluated().ok().map(|e| e.lights)).flatten();
        let lights = resolved.as_deref().unwrap_or(&self.editor.project.lights);
        let hovered =
            if self.drag.is_none() && self.canvas_input_at(self.mouse) && !self.has_modal() {
                pick_marker(
                    lights,
                    selected,
                    &view,
                    viewport,
                    self.mouse,
                )
            } else {
                None
            };
        let start = ui.rects.len();
        for index in marker_indices(self.editor.project.lights.len(), selected) {
            let Some(light) = lights.get(index) else { continue; };
            let Some(center) = marker_position(&view, viewport, light.position) else {
                continue;
            };
            let active = selected == Some(index);
            let color = marker_color(
                ui.theme,
                ui.palette().accent,
                active || hovered == Some(index),
                light.enabled,
            );
            draw_marker(
                ui,
                center,
                light.kind,
                color,
                active || hovered == Some(index),
            );
            if active && light.kind == LightKind::Area {
                let (right, up) = light.area_basis();
                let points = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]].map(|p| {
                    view.project(
                        light.position
                            + right * p[0] * light.area_size[0] * 0.5
                            + up * p[1] * light.area_size[1] * 0.5,
                    )
                    .map(|p| Vec2::from_array(p) / ui.scale)
                });
                for edge in 0..4 {
                    if let (Some(a), Some(b)) = (points[edge], points[(edge + 1) % 4]) {
                        line(ui, a, b, color);
                    }
                }
            }
            if active || hovered == Some(index) {
                draw_light_ray(ui, &view, viewport, light, center, color);
            }
            if (active || hovered == Some(index)) && viewport[2] >= 32. && viewport[3] >= 32. {
                let width = 180_f32.min(viewport[2] - 12.);
                let label = [
                    (center.x + 17.)
                        .clamp(viewport[0] + 4., viewport[0] + viewport[2] - width - 4.),
                    (center.y + 17.).clamp(viewport[1] + 4., viewport[1] + viewport[3] - 22.),
                    width,
                    18.,
                ];
                if width > 20. && !ui::contains(self.rail_rect(), [label[0], label[1]]) {
                    let text = if light.enabled {
                        light.name.clone()
                    } else {
                        format!("{} · выключен", light.name)
                    };
                    ui.fitted_label(label, &text, 11., color);
                }
            }
        }
        clip_new_rectangles(ui, start, viewport);
        if let Some(Drag::Light(drag)) = &self.drag {
            if drag.gesture.gizmo.mode == Mode::Rotate {
                drag.gesture.gizmo.draw(
                    ui,
                    None,
                    Some(drag.gesture.handle),
                    self.axis,
                    self.rail_rect(),
                );
                drag.gesture.draw_feedback(ui, self.rail_rect());
            } else if let Some(gizmo) = self.light_gizmo() {
                gizmo.draw(
                    ui,
                    None,
                    Some(drag.gesture.handle),
                    self.axis,
                    self.rail_rect(),
                );
            }
        } else if let Some(gizmo) = self.light_gizmo() {
            let hovered = self
                .canvas_input_at(self.mouse)
                .then(|| gizmo.hit(self.mouse))
                .flatten();
            gizmo.draw(ui, hovered, None, self.axis, self.rail_rect());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(orthographic: bool, scale: f32) -> (View, [f32; 4]) {
        let logical = [180., 80., 760., 560.];
        (
            View {
                effects: Default::default(),
                depth_of_field: Default::default(),
                eye: Vec3::new(0., 0., 8.),
                target: Vec3::ZERO,
                up: Vec3::Y,
                fov_y: 45_f32.to_radians(),
                viewport: logical.map(|value| value * scale),
                orthographic: orthographic.then_some(4.),
            },
            logical,
        )
    }

    #[test]
    fn new_lights_are_visible_pickable_and_aim_at_the_target_at_both_dpi() {
        for orthographic in [false, true] {
            for scale in [1., 2.] {
                let (view, viewport) = view(orthographic, scale);
                let (position, direction) = placement_in_view(&view);
                let center = marker_position(&view, viewport, position).unwrap();
                assert!(center.distance(Vec2::new(560., 360.)) > 40.);
                assert!(direction.dot((view.target - position).normalize()) > 0.9999);
                let light = Light {
                    position,
                    direction,
                    ..Light::default()
                };
                assert_eq!(
                    pick_marker(&[light], None, &view, viewport, center.to_array()),
                    Some(0)
                );
            }
        }
    }

    #[test]
    fn light_markers_reject_camera_plane_and_far_outside_viewport() {
        let (view, viewport) = view(false, 1.);
        for position in [
            view.eye,
            view.eye + Vec3::Z,
            Vec3::new(0., 0., -2000.),
            Vec3::splat(f32::NAN),
            Vec3::X * 100.,
        ] {
            assert!(marker_position(&view, viewport, position).is_none());
        }
        let light = Light {
            position: Vec3::ZERO,
            ..Light::default()
        };
        assert_eq!(
            pick_marker(&[light], None, &view, viewport, [viewport[0] - 1., 360.]),
            None
        );
    }

    #[test]
    fn markers_remain_clipped_finite_and_bounded_in_both_themes() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 2.] {
                let (_, viewport) = view(false, scale);
                let mut ui = Ui::with_theme(scale, theme, [0., 0.]);
                for kind in [LightKind::Directional, LightKind::Point, LightKind::Spot] {
                    for center in [
                        Vec2::new(viewport[0], viewport[1]),
                        Vec2::new(viewport[0] + viewport[2], viewport[1] + viewport[3]),
                    ] {
                        draw_marker(
                            &mut ui,
                            center,
                            kind,
                            marker_color(theme, theme.palette().accent, true, true),
                            true,
                        );
                    }
                }
                clip_new_rectangles(&mut ui, 0, viewport);
                assert!(!ui.rects.is_empty());
                assert!(ui.rects.len() < 30_000);
                let bounds = viewport.map(|value| value * scale);
                for primitive in &ui.rects {
                    let r = primitive.rect;
                    assert!(r.iter().all(|value| value.is_finite()));
                    assert!(r[0] >= bounds[0] && r[1] >= bounds[1]);
                    assert!(r[0] + r[2] <= bounds[0] + bounds[2] + 0.01);
                    assert!(r[1] + r[3] <= bounds[1] + bounds[3] + 0.01);
                }
            }
        }
        let indices: Vec<_> = marker_indices(10_000, Some(9_999)).collect();
        assert_eq!(indices.len(), MAX_MARKERS + 1);
        assert_eq!(indices.last(), Some(&9_999));
    }

    #[test]
    fn light_orientation_maps_gizmo_rotation_to_world_light_direction() {
        for direction in [Vec3::Z, Vec3::NEG_Z, Vec3::Y, Vec3::new(-0.5, -1., -0.4)] {
            let light = Light {
                direction,
                ..Light::default()
            };
            let transform = light_transform(&light).unwrap();
            assert!((transform.rotation * Vec3::NEG_Z).distance(direction.normalize()) < 0.00001);
        }
    }
}
