//! The drawn glyph and its drag use the same projected measurement.
use super::*;
use crate::app::viewport_grips::{segment, Glyph, Grip, GripArea};

fn meters(v: f32) -> String {
    format!("{} м", ui::number_caption(v as f64))
}
fn percent(v: f32) -> String {
    format!("{}%", ui::number_caption(v as f64))
}

impl App {
    pub(super) fn spacing_basis(&self, op: &LiveOperation) -> (Vec2, f32) {
        let tip = op.anchor + op.model.transform_vector3(op.offset());
        self.grip_axis(
            tip,
            op.model.transform_vector3(op.array_axis),
            op.scale * 0.12,
            (Vec2::X, op.scale * 0.002),
        )
    }
    pub(super) fn array_parameter_basis(
        &self,
        op: &LiveOperation,
        field: usize,
    ) -> ([f32; 2], f32) {
        if field == 0 {
            let offset = op.model.transform_vector3(op.offset());
            let last = op.anchor + offset * (op.values[0] - 1.);
            if let (Some(a), Some(b)) = (
                self.grip_projection(last - offset),
                self.grip_projection(last),
            ) {
                let delta = b - a;
                if let Some(dir) = delta.try_normalize() {
                    return (dir.to_array(), 1. / delta.length().max(14.));
                }
            }
            let (dir, unit) = self.spacing_basis(op);
            return (dir.to_array(), 1. / (op.offset().length() / unit).max(14.));
        }
        let world = op
            .model
            .transform_vector3([Vec3::X, Vec3::Y, Vec3::Z][field - 1]);
        let (dir, step) = self.grip_axis(
            op.anchor,
            world,
            op.scale * 0.12,
            (Vec2::X, op.spec(field).3),
        );
        (dir.to_array(), step)
    }
    pub(super) fn surface_basis(&self, op: &LiveOperation, i: usize) -> Option<(Vec2, f32)> {
        let s = op.surface.as_ref()?;
        let offset = op.direction * op.values[1];
        let (origin, axis) = match (op.kind, i) {
            (Kind::Inset, 0) => (s.corner + offset, s.miter),
            (Kind::Edge | Kind::Face, 0) => (s.corner + s.next * 0.67, s.inward),
            (Kind::Edge, 2) | (Kind::Face, 3) => {
                let origin = s.corner + s.next * 0.27;
                let (width, _) = self.grip_axis(origin, s.inward, op.scale * 0.12, (Vec2::X, 1.));
                let bend = if op.kind == Kind::Face {
                    op.direction - s.inward
                } else {
                    -(s.opposite + s.inward)
                };
                let (dir, _) = self.grip_axis(origin, bend, op.scale * 0.12, (width.perp(), 1.));
                return Some((dir, 2.));
            }
            (Kind::Vertex | Kind::VertexInset, 0) => (s.corner, (s.previous + s.next) * 0.005),
            _ => return None,
        };
        let vertex = matches!(op.kind, Kind::Vertex | Kind::VertexInset);
        Some(self.grip_axis(
            origin,
            axis,
            if vertex { 15. } else { op.scale * 0.12 },
            (Vec2::X, op.spec(i).3),
        ))
    }
    pub(super) fn seam_basis(&self, op: &LiveOperation) -> Option<(Vec2, f32)> {
        let s = op.seam.as_ref()?;
        let n = s.targets.len();
        let i = (op.values[1] as isize).rem_euclid(n as isize) as usize;
        let axis =
            self.grip_projection(s.targets[(i + 1) % n])? - self.grip_projection(s.targets[i])?;
        Some((
            axis.try_normalize().unwrap_or(Vec2::X),
            1. / axis.length().max(12.),
        ))
    }
    fn surface_grips(&self, op: &LiveOperation, grips: &mut Vec<(Grip, Vec2)>) {
        let Some(s) = &op.surface else {
            return;
        };
        let field = |i| Vec2::from_array(self.live_parameter_basis(op, i).0);
        let project = |p| self.grip_projection(p);
        if matches!(op.kind, Kind::Inset | Kind::VertexInset) {
            let vertex = op.kind == Kind::VertexInset;
            let origin = s.corner
                + if vertex {
                    Vec3::ZERO
                } else {
                    op.direction * op.values[1]
                };
            let miter = if vertex {
                (s.previous + s.next) * 0.005
            } else {
                s.miter
            };
            let Some(at) = project(origin + miter * op.values[0]) else {
                return;
            };
            let Some(src) = project(origin) else {
                return;
            };
            let dir = field(0);
            let side = |v: Vec3, backup: Vec2| {
                project(origin + v * 0.3)
                    .and_then(|p| (p - src).try_normalize())
                    .unwrap_or(backup)
            };
            let mut g = Grip::new(
                at,
                dir,
                Glyph::Corner(side(s.previous, dir.perp()), side(s.next, dir)),
                A::LiveDrag(0),
                if vertex {
                    "Срез угла"
                } else {
                    "Отступ"
                },
                if vertex {
                    percent(op.values[0])
                } else {
                    meters(op.values[0])
                },
            );
            g.tether(src);
            grips.push((g, src));
        } else {
            let vertex = op.kind == Kind::Vertex;
            let origin = if vertex {
                s.corner
            } else {
                s.corner + s.next * 0.67
            };
            let Some(src) = project(origin) else {
                return;
            };
            let axis = field(0);
            let side = if vertex { s.next * 0.01 } else { s.inward };
            let other = if vertex {
                s.previous * 0.01
            } else {
                s.opposite
            };
            let actual = project(origin + side * op.values[0]).unwrap_or(src);
            let mut g = Grip::new(
                actual + axis * 13.,
                axis,
                Glyph::Caliper,
                A::LiveDrag(0),
                if vertex {
                    "Срез угла"
                } else {
                    "Ширина"
                },
                if vertex {
                    percent(op.values[0])
                } else {
                    meters(op.values[0])
                },
            );
            if let Some(opp) = project(origin + other * op.values[0]) {
                g.guides.extend([
                    segment(opp, src),
                    segment(src, actual),
                    segment(opp - axis.perp() * 4., opp + axis.perp() * 4.),
                ]);
            }
            g.tether(actual);
            grips.push((g, src));
            if !vertex {
                let i = if op.kind == Kind::Edge { 2 } else { 3 };
                let origin = s.corner + s.next * 0.27;
                if let Some(src) = project(origin) {
                    let dir = field(i);
                    let tip = src + dir * (24. + 0.5 * op.values[i]);
                    let mut g = Grip::new(
                        tip,
                        dir,
                        Glyph::Lens,
                        A::LiveDrag(i as u8),
                        "Профиль",
                        percent(op.values[i]),
                    );
                    g.tether(src);
                    grips.push((g, src));
                }
            }
        }
    }
    fn array_grips(&self, op: &LiveOperation, base: Vec2, grips: &mut Vec<(Grip, Vec2)>) {
        let (dir, _) = self.spacing_basis(op);
        let offset = op.model.transform_vector3(op.offset());
        let second = self
            .grip_projection(op.anchor + offset)
            .unwrap_or(base + dir * 64.);
        let end = self
            .grip_projection(op.anchor + offset * (op.values[0] - 1.))
            .unwrap_or(second);
        let lateral = dir.perp() * 15.;
        let mut spacing = Grip::new(
            second + lateral,
            dir,
            Glyph::Caliper,
            A::ArraySpacing,
            "Шаг",
            meters(op.offset().length()),
        );
        spacing.guides.extend([
            segment(base, base + lateral),
            segment(base + lateral, second + lateral),
            segment(second + lateral, second),
        ]);
        grips.push((spacing, base));
        let count_axis = Vec2::from_array(self.array_parameter_basis(op, 0).0);
        let mut count = Grip::new(
            end + count_axis * 19. - lateral,
            count_axis,
            Glyph::Stack,
            A::LiveDrag(0),
            "Копии",
            format!("{}", op.values[0] as u32 - 1),
        );
        count.tether(end);
        grips.push((count, base));
    }
    fn bridge_grip(&self, op: &LiveOperation, grips: &mut Vec<(Grip, Vec2)>) {
        let Some(s) = &op.seam else {
            return;
        };
        let n = s.targets.len();
        let i = (op.values[1] as isize).rem_euclid(n as isize) as usize;
        let Some(at) = self.grip_projection(s.targets[i]) else {
            return;
        };
        let dir = self.seam_basis(op).map_or(Vec2::X, |b| b.0);
        let mut g = Grip::new(
            at,
            dir,
            Glyph::Eye,
            A::LiveDrag(1),
            "Шов",
            format!("{} / {}", i + 1, n),
        );
        if let Some(src) = self.grip_projection(s.source) {
            g.tether(src);
        }
        if let Some(rim) = self.projected_rim(op) {
            for j in 0..n {
                let a = rim[j];
                let b = rim[(j + 1) % n];
                g.guides.push(segment(a, b));
            }
        }
        grips.push((g, at));
    }
    pub(super) fn draw_live_handles(&self, u: &mut Ui, op: &LiveOperation) {
        if op.selecting {
            return;
        }
        let Some(base) = self.grip_projection(op.anchor) else {
            return;
        };
        let mut grips = Vec::with_capacity(3);
        match op.kind {
            Kind::Array => self.array_grips(op, base, &mut grips),
            Kind::Bridge => self.bridge_grip(op, &mut grips),
            Kind::Face | Kind::Edge | Kind::Inset | Kind::Vertex | Kind::VertexInset => {
                self.surface_grips(op, &mut grips)
            }
            _ => {}
        }
        let axial = match op.kind {
            Kind::Extrude | Kind::VertexExtrude | Kind::Individual => {
                Some((0, op.values[0], "Высота", Glyph::Bar))
            }
            Kind::Face | Kind::Inset => Some((1, op.values[1], "Высота", Glyph::Bar)),
            Kind::Cut => Some((0, op.values[0] / 100. - 0.5, "Положение", Glyph::Arrow)),
            Kind::Slide => Some((0, 0., "Сдвиг", Glyph::Arrow)),
            _ => None,
        };
        if let Some((i, displacement, name, glyph)) = axial {
            if let Some(tip) = self.grip_projection(op.anchor + op.direction * displacement) {
                let axis = Vec2::from_array(self.live_parameter_basis(op, i).0);
                let value = if matches!(op.kind, Kind::Cut | Kind::Slide) {
                    percent(op.values[i])
                } else {
                    meters(op.values[i])
                };
                let mut g = Grip::new(
                    tip + axis * 26.,
                    axis,
                    glyph,
                    A::LiveDrag(i as u8),
                    name,
                    value,
                );
                g.guides.push(segment(base, tip));
                g.tether(tip);
                grips.push((g, base));
            }
        }
        let mut area = GripArea::new(self);
        grips.retain_mut(|(g, source)| area.place(g, *source));
        for (g, _) in grips {
            let active = matches!(&self.drag,Some(Drag::LiveParameter(d)) if d.viewport && d.action()==g.action);
            area.paint(u, &g, self.mouse, active);
        }
    }

    pub(in crate::app) fn modeling_live_scroll(&mut self, delta: f32) -> bool {
        if !self.modeling_live_owns_scroll() {
            return false;
        }
        let merge_distance = matches!(
            self.ui.hit(self.mouse),
            Some(Action::Author(A::ToolMergeDistance))
        ) || matches!(self.ui.hit(self.mouse),Some(Action::Slide(field,_,_)) if *field==Action::Author(A::ToolMergeDistance));
        if merge_distance {
            if self.drag.is_none()
                && self.input.is_none()
                && !self.has_modal()
                && delta.is_finite()
                && delta != 0.
            {
                let step = 0.001 * if self.modifiers.shift_key() { 0.1 } else { 1. };
                let value = (self.tool_merge_distance()
                    + delta.signum() * delta.abs().round().max(1.) * step)
                    .clamp(0., 10000.);
                let _ = self.set_tool_merge_distance(value);
            }
            return true;
        }
        let grip = matches!(self.drag, Some(Drag::LiveParameter(ref d)) if d.field == 0 && d.viewport)
            || matches!(
                self.ui.hit(self.mouse),
                Some(Action::Author(A::LiveDrag(0)))
            );
        if self.input.is_none() && !self.has_modal() && delta.is_finite() && delta != 0. {
            if let Some(op) = &self.modeling_live {
                let spacing = matches!(&self.drag,Some(Drag::LiveParameter(d)) if d.action()==A::ArraySpacing)
                    || (self.drag.is_none()
                        && self.ui.hit(self.mouse) == Some(Action::Author(A::ArraySpacing)));
                if spacing && op.kind == Kind::Array {
                    let step = op.scale
                        * if self.modifiers.control_key() {
                            0.01
                        } else {
                            0.002
                        };
                    let precision = if self.modifiers.shift_key() { 0.1 } else { 1. };
                    let values = op.with_spacing(
                        op.array_axis,
                        op.offset().length()
                            + delta.signum() * delta.abs().round().max(1.) * step * precision,
                    );
                    if values != op.values && self.preview_live_values(values).is_ok() {
                        if let Some(Drag::LiveParameter(d)) = &mut self.drag {
                            d.value = Vec3::new(values[1], values[2], values[3]).length();
                            d.snap_origin = d.value;
                            d.requested = values;
                        }
                    }
                    self.redraw();
                    return true;
                }
                let hovered_field = match self.ui.hit(self.mouse) {
                    Some(Action::Slide(action, _, _)) => match *action {
                        Action::Author(A::LiveField(i)) => Some(i as usize),
                        _ => None,
                    },
                    Some(Action::Author(A::LiveField(i) | A::LiveDrag(i))) => Some(i as usize),
                    _ => None,
                };
                let field = if grip && op.kind == Kind::Edge {
                    Some(1)
                } else {
                    hovered_field
                };
                if let Some(field) = field.filter(|i| *i < op.fields()) {
                    let mut values = op.values;
                    let (_, min, max, step) = op.spec(field);
                    let integer = op.integer(field);
                    let step = if integer {
                        1.
                    } else {
                        let step = if self.modifiers.control_key() {
                            if op.spec(field).0.ends_with('%') {
                                5.
                            } else {
                                op.scale * 0.01
                            }
                        } else {
                            step
                        };
                        step * if self.modifiers.shift_key() { 0.1 } else { 1. }
                    };
                    values[field] = (values[field]
                        + delta.signum() * delta.abs().round().max(1.) * step)
                        .clamp(min, max);
                    if values != op.values && self.preview_live_values(values).is_ok() {
                        if let Some(Drag::LiveParameter(d)) = &mut self.drag {
                            d.requested = values;
                            if d.field == field {
                                d.value = values[field];
                                d.snap_origin = d.value;
                            }
                        }
                    }
                    self.redraw();
                }
            }
        }
        true
    }
}
