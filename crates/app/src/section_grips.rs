//! Position and gap measurements for the selected cut plane.
use super::*;
use crate::app::viewport_grips::{segment, Glyph, Grip, GripArea};
use crate::ui::SectionHandle;
use glam::Vec2;

#[derive(Clone)]
pub(in crate::app) struct SectionGripDrag {
    plane: usize,
    handle: SectionHandle,
    start: f32,
    previous_gap: f32,
    pointer: Vec2,
    value: f32,
    requested: f32,
    pub(in crate::app) axis: Vec2,
    units: f32,
}
impl SectionHandle {
    fn field(self) -> usize {
        match self {
            Self::Position => 0,
            Self::Gap => 3,
        }
    }
    fn bounds(self) -> (f32, f32) {
        match self {
            Self::Position => (-10000., 10000.),
            Self::Gap => (0., 1000.),
        }
    }
}

impl App {
    pub(in crate::app) fn section_grip_basis(&self, handle: SectionHandle) -> Option<(Vec2, f32)> {
        let k = self
            .authoring
            .knife
            .as_ref()
            .filter(|k| k.loop_mode && k.section_edit)?;
        let panel = k.sections.get(k.section_selected?)?;
        let model = self
            .editor
            .project
            .global_transforms()
            .ok()?
            .get(&k.node)
            .copied()?;
        Some(self.plane_basis(panel, model, handle))
    }
    fn plane_basis(&self, panel: &Panel, model: Mat4, handle: SectionHandle) -> (Vec2, f32) {
        let (c, a, b) = panel.frame();
        let n = Vec3::from_array(panel.cut().normal);
        let anchor = match handle {
            SectionHandle::Position => c + (a + b) * panel.radius * 0.55,
            SectionHandle::Gap => c - (a + b) * panel.radius * 0.38 + n * panel.values[3] * 0.5,
        };
        let (dir, unit) = self.grip_axis(
            model.transform_point3(anchor),
            model.transform_vector3(n),
            panel.radius * 0.15,
            (Vec2::NEG_Y, panel.radius.max(0.001) / 180.),
        );
        (
            dir,
            if handle == SectionHandle::Gap {
                unit * 2.
            } else {
                unit
            },
        )
    }
    pub(super) fn begin_section_grip(&mut self, handle: SectionHandle) {
        let Some((axis, units)) = self.section_grip_basis(handle) else {
            return;
        };
        let k = self.authoring.knife.as_mut().unwrap();
        let plane = k.section_selected.unwrap();
        k.press = None;
        k.section_drag = None;
        let start = k.sections[plane].values[handle.field()];
        self.drag = Some(Drag::SectionGrip(SectionGripDrag {
            plane,
            handle,
            start,
            previous_gap: self.authoring.workflow.values[1],
            pointer: Vec2::from_array(self.mouse),
            value: start,
            requested: start,
            axis,
            units,
        }));
        self.drag_button = Some(MouseButton::Left);
    }
    pub(in crate::app) fn update_section_grip(&mut self, mut d: SectionGripDrag, p: [f32; 2]) {
        let Some(k) = self
            .authoring
            .knife
            .as_ref()
            .filter(|k| k.section_edit && k.section_selected == Some(d.plane))
        else {
            return;
        };
        let pointer = Vec2::from_array(p);
        if pointer.is_finite() {
            let (min, max) = d.handle.bounds();
            let precision = if self.modifiers.shift_key() { 0.1 } else { 1. };
            d.value =
                (d.value + (pointer - d.pointer).dot(d.axis) * d.units * precision).clamp(min, max);
            d.pointer = pointer;
            let step = (k.sections[d.plane].radius * 0.02).max(0.00001);
            let value = if self.modifiers.control_key() {
                (d.start + ((d.value - d.start) / step).round() * step).clamp(min, max)
            } else {
                d.value
            };
            if value != d.requested {
                d.requested = value;
                if let Err(e) = self.set_section_value(d.plane, d.handle.field(), value) {
                    self.status = e;
                }
            }
        }
        self.drag = Some(Drag::SectionGrip(d));
    }
    pub(in crate::app) fn restore_section_grip(&mut self, d: &SectionGripDrag) {
        if let Err(e) = self.set_section_value(d.plane, d.handle.field(), d.start) {
            self.status = e;
        }
        if d.handle == SectionHandle::Gap {
            self.authoring.workflow.values[1] = d.previous_gap;
        }
    }
    pub(super) fn draw_section_grips(&self, u: &mut Ui, model: Mat4, panel: &Panel) {
        let (c, a, b) = panel.frame();
        let n = Vec3::from_array(panel.cut().normal);
        let project = |v| self.grip_projection(model.transform_point3(v));
        let Some(center) = project(c) else {
            return;
        };
        let mut grips = Vec::with_capacity(2);
        let place = SectionHandle::Position;
        if let Some(tip) = project(c + (a + b) * panel.radius * 0.55) {
            let (axis, _) = self.plane_basis(panel, model, place);
            let mut g = Grip::new(
                tip,
                axis,
                Glyph::Plate,
                A::SectionGrip(place),
                "Положение",
                format!("{} м", ui::number_caption(panel.values[0] as f64)),
            );
            g.tether(center);
            grips.push(g);
        }
        let gap = SectionHandle::Gap;
        let origin = c - (a + b) * panel.radius * 0.38;
        if let (Some(lo), Some(hi)) = (
            project(origin - n * panel.values[3] * 0.5),
            project(origin + n * panel.values[3] * 0.5),
        ) {
            let (axis, _) = self.plane_basis(panel, model, gap);
            let lift = axis.perp() * 10.;
            let mut g = Grip::new(
                hi + lift + axis * 14.,
                axis,
                Glyph::Caliper,
                A::SectionGrip(gap),
                "Зазор",
                format!("{} м", ui::number_caption(panel.values[3] as f64)),
            );
            g.guides.extend([
                segment(lo, lo + lift * 1.4),
                segment(lo + lift, hi + lift),
                segment(hi, hi + lift * 1.4),
            ]);
            g.tether(hi + lift);
            grips.push(g);
        }
        let mut area = GripArea::new(self);
        grips.retain_mut(|g| area.place(g, center));
        for g in grips {
            let active = matches!(&self.drag,Some(Drag::SectionGrip(d)) if A::SectionGrip(d.handle)==g.action);
            area.paint(u, &g, self.mouse, active);
        }
    }
}
