//! Light aiming and rectangle handles; solo affects the viewport only.
use super::*;
use crate::ui::LightTool as L;
use poseforge_core::{Light, LightKind};
use poseforge_render::View;
#[derive(Clone)]
pub(super) struct AreaDrag {
    index: usize,
    original: Light,
    view: View,
    viewport: [f32; 4],
    anchor: Vec3,
    axis: usize,
    sign: f32,
}
fn on_plane(view: &View, viewport: [f32; 4], pointer: [f32; 2], light: &Light) -> Option<Vec3> {
    let x = (pointer[0] - viewport[0]) / viewport[2] * 2. - 1.;
    let y = 1. - (pointer[1] - viewport[1]) / viewport[3] * 2.;
    let inverse = view.view_projection().inverse();
    let unproject = |z| {
        let h = inverse * glam::Vec4::new(x, y, z, 1.);
        h.truncate() / h.w
    };
    let origin = unproject(0.);
    let direction = (unproject(0.5) - origin).try_normalize()?;
    let n = light.direction.try_normalize()?;
    let denominator = direction.dot(n);
    if denominator.abs() < 0.015 {
        return None;
    }
    let t = (light.position - origin).dot(n) / denominator;
    let p = origin + direction * t;
    (t > 0. && p.is_finite()).then_some(p)
}
impl App {
    pub(super) fn light_tool(&mut self, action: L) {
        if action == L::SoloOff {
            self.solo_light = None;
            self.performance.changed();
            return;
        }
        let Some(index) = self.active_light_index().or(self.surface_light_index()) else {
            return;
        };
        if action == L::Solo {
            self.solo_light = if self.solo_light == Some(index) {
                None
            } else {
                Some(index)
            };
            self.performance.changed();
            return;
        }
        if self.editor.project.lights[index].kind == LightKind::Point {
            self.status = "Точечный источник излучает во все стороны.".into();
            return;
        }
        if action == L::StopTracking {
            if self.editor.project.light_locked(index) {
                return;
            }
            let Ok(eval) = self.evaluated_interactive() else {
                return;
            };
            let Some(inverse) = self.light_curve_frame(index).map(|m| m.inverse()) else { return; };
            let direction = inverse.transform_vector3(eval.lights[index].direction).normalize_or_zero();
            let r = self.editor.transact("Остановить слежение света", |p| {
                p.lights[index].direction = direction;
                p.lights[index].target = None;
            });
            self.result(r);
            return;
        }
        if matches!(action, L::PickTarget | L::PickTrackedTarget) {
            self.light_aim = Some((
                index,
                self.editor.project.lights[index].clone(),
                action == L::PickTrackedTarget,
            ));
            self.status = "Выберите меш — свет направится на его центр. Esc — отмена.".into();
            return;
        }
        let Some(target) = self
            .last_light_target
            .or(self.editor.selected)
            .filter(|id| self.node_selectable(*id))
        else {
            self.status =
                "Выберите объект, затем источник света, или нажмите «Выбрать цель».".into();
            return;
        };
        self.aim_light_at(index, target, false);
    }
    pub(super) fn aim_light_at(&mut self, index: usize, target: u64, tracking: bool) {
        if self.editor.project.light_locked(index) {
            self.status = "Источник в заблокированной папке.".into();
            return;
        }
        let Ok(eval) = self.evaluated_interactive() else {
            return;
        };
        let Some(node) = self.editor.project.node(target) else {
            return;
        };
        let Some(matrix) = eval.global.get(&target) else {
            return;
        };
        let center = node
            .mesh
            .and_then(|id| self.editor.project.mesh(id))
            .filter(|m| !m.vertices.is_empty())
            .map(|m| {
                let mut min = Vec3::splat(f32::INFINITY);
                let mut max = Vec3::splat(f32::NEG_INFINITY);
                for v in &m.vertices {
                    let p = Vec3::from(v.position);
                    min = min.min(p);
                    max = max.max(p);
                }
                (min + max) * 0.5
            })
            .unwrap_or(Vec3::ZERO);
        let local_center = center;
        let center = matrix.transform_point3(center);
        let Some(light) = self.evaluated_light(index) else { return; };
        let Some(direction) = (center - light.position).try_normalize() else {
            self.status = "Цель совпадает с положением источника.".into();
            return;
        };
        let Some(inverse) = self.light_curve_frame(index).map(|m| m.inverse()) else { return; };
        let direction = inverse.transform_vector3(direction).normalize_or_zero();
        let r = self.editor.transact("Направить свет на объект", |p| {
            p.lights[index].direction = direction;
            p.lights[index].target = tracking.then_some(poseforge_core::LightTarget {
                node: target,
                local_center,
            });
        });
        if self.result(r).is_some() {
            self.last_light_target = Some(target);
            self.status = if tracking {
                "Свет следит за объектом."
            } else {
                "Свет направлен на объект один раз."
            }
            .into();
        }
    }
    pub(super) fn light_aim_pointer(&mut self) -> bool {
        let Some((index, expected, tracking)) = self.light_aim.clone() else {
            return false;
        };
        if self.editor.project.lights.get(index) != Some(&expected) {
            self.light_aim = None;
            return true;
        }
        let Ok(eval) = self.evaluated_interactive() else {
            return true;
        };
        if let Some(hit) = crate::picking::pick_mesh(
            &self.picking_project(),
            &eval,
            self.view().view_projection(),
            self.viewport(),
            self.mouse,
        ) {
            self.aim_light_at(index, hit, tracking);
            self.light_aim = None;
        } else {
            self.status = "Щёлкните по поверхности меша. Esc — отмена.".into();
        }
        true
    }
    fn area_handles(&self) -> Vec<(usize, f32, [f32; 2])> {
        let Some(index) = self.active_light_index() else {
            return vec![];
        };
        let Some(resolved) = self.evaluated_light(index) else {
            return vec![];
        };
        let light = &resolved;
        if light.kind != LightKind::Area || self.has_modal() {
            return vec![];
        }
        let (right, up) = light.area_basis();
        let mut out = vec![];
        for (axis, direction) in [right, up].into_iter().enumerate() {
            for sign in [-1., 1.] {
                if let Some(p) = self
                    .project_point(light.position + direction * sign * light.area_size[axis] * 0.5)
                    .filter(|p| self.canvas_input_at(*p))
                {
                    out.push((axis, sign, p));
                }
            }
        }
        out
    }
    pub(super) fn draw_area_handles(&self, u: &mut Ui) {
        for (_, _, p) in self.area_handles() {
            let c = u.palette();
            u.outlined_surface([p[0] - 5., p[1] - 5., 10., 10.], c.panel, c.accent);
        }
        if self.solo_light.is_some() {
            u.button(
                [
                    self.left_panel_rect()[0] + self.left_width() + 12.,
                    76.,
                    150.,
                    28.,
                ],
                "Только этот свет · ×",
                Action::LightTool(L::SoloOff),
                true,
            );
        }
    }
    pub(super) fn area_pointer_down(&mut self) -> bool {
        let Some((axis, sign, _)) = self
            .area_handles()
            .into_iter()
            .find(|(_, _, p)| (p[0] - self.mouse[0]).hypot(p[1] - self.mouse[1]) <= 8.)
        else {
            return false;
        };
        let index = self.active_light_index().unwrap();
        let Some(original) = self.evaluated_light(index) else {
            return true;
        };
        let view = self.view();
        let viewport = self.viewport();
        let Some(anchor) = on_plane(&view, viewport, self.mouse, &original) else {
            self.status = "Поверните вид: площадной источник виден с ребра.".into();
            return true;
        };
        self.editor.begin_edit("Размер площадного света");
        self.drag = Some(Drag::Area(AreaDrag {
            index,
            original,
            view,
            viewport,
            anchor,
            axis,
            sign,
        }));
        self.drag_button = Some(MouseButton::Left);
        true
    }
    pub(super) fn update_area_drag(&mut self, drag: AreaDrag, pointer: [f32; 2]) {
        if let Some(point) = on_plane(&drag.view, drag.viewport, pointer, &drag.original) {
            let (r, u) = drag.original.area_basis();
            let direction = if drag.axis == 0 { r } else { u };
            let delta = 2.
                * (point - drag.anchor).dot(direction)
                * drag.sign
                * if self.modifiers.shift_key() { 0.1 } else { 1. };
            let mut value = (drag.original.area_size[drag.axis] + delta).clamp(0.01, 100.);
            if self.modifiers.control_key() {
                value = (value * 10.).round().max(0.1) / 10.;
            }
            if let Some(light) = self.editor.project.lights.get_mut(drag.index) {
                light.area_size[drag.axis] = value;
                self.status = format!(
                    "Размер света: {:.2} × {:.2} м · Shift: точно · Ctrl: шаг 0.1 м",
                    light.area_size[0], light.area_size[1]
                );
                self.update_dirty();
            }
        }
        self.drag = Some(Drag::Area(drag));
    }
    pub(super) fn apply_solo_light(project: &mut poseforge_core::Project, index: usize) {
        if index >= project.lights.len() {
            return;
        }
        for (i, light) in project.lights.iter_mut().enumerate() {
            light.enabled = i == index;
        }
        project.render_settings.hdri_enabled = false;
        project.render_settings.ambient_intensity = 0.;
    }
}
