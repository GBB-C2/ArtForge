//! On-model tool gizmos. Rendering and picking share the same clipped strokes.
use super::*;
use crate::gizmo;
use glam::Vec2;

type Segment = ([f32; 2], [f32; 2]);
const PICK: f32 = 8.;

fn padded(r: [f32; 4], amount: f32) -> [f32; 4] {
    [
        r[0] - amount,
        r[1] - amount,
        r[2] + amount * 2.,
        r[3] + amount * 2.,
    ]
}

fn clip(strokes: &[Segment], area: [f32; 4], obstacles: &[[f32; 4]]) -> Vec<Segment> {
    let mut result = Vec::new();
    for &(a, b) in strokes {
        let (a, b) = (Vec2::from_array(a), Vec2::from_array(b));
        let Some(range) = gizmo::clip_range(a, b, padded(area, -PICK - 1.)) else {
            continue;
        };
        let mut ranges = vec![range];
        for &r in obstacles {
            if let Some((cut_lo, cut_hi)) = gizmo::clip_range(a, b, padded(r, PICK + 2.)) {
                ranges = ranges
                    .into_iter()
                    .flat_map(|(lo, hi)| {
                        [(lo, hi.min(cut_lo)), (lo.max(cut_hi), hi)]
                            .into_iter()
                            .filter(|(lo, hi)| hi - lo > 1e-5)
                    })
                    .collect();
            }
        }
        result.extend(
            ranges
                .into_iter()
                .filter(|(lo, hi)| hi - lo > 1e-5)
                .map(|(lo, hi)| (a.lerp(b, lo).to_array(), a.lerp(b, hi).to_array())),
        );
    }
    result
}

fn arrow(strokes: &mut Vec<Segment>, center: Vec2, axis: Vec2, sign: f32, length: f32) {
    let side = Vec2::new(-axis.y, axis.x);
    let tip = center + axis * (length * sign);
    let base = tip - axis * (13. * sign);
    strokes.push(((center + axis * (9. * sign)).to_array(), base.to_array()));
    // A closed triangular head is recognizable at any screen orientation.
    let left = base + side * 5.;
    let right = base - side * 5.;
    strokes.extend([
        (left.to_array(), tip.to_array()),
        (tip.to_array(), right.to_array()),
        (right.to_array(), left.to_array()),
    ]);
}

impl App {
    pub(super) fn draw_live_handles(&self, u: &mut Ui, op: &LiveOperation) {
        let mut world = op.anchor;
        if matches!(op.kind,Kind::Extrude|Kind::VertexExtrude) {
            world += op.direction * op.values[0];
        } else if op.kind == Kind::Cut {
            world += op.direction * (op.values[0] / 100. - 0.5);
        } else if op.kind == Kind::Face {
            world += op.direction * op.values[1];
        }
        let Some(center) = self
            .project_point(world)
            .map(Vec2::from_array)
            .filter(|p| p.is_finite())
        else {
            return;
        };
        let area = self.modeling_area();
        // Clip individual strokes even when the pivot just crossed the edge.
        // A partially visible ring must remain usable, without relocating it
        // to an invented screen position unrelated to the model.
        let mut navigation = self.view_widget_rect();
        navigation[3] += if self.camera_view.is_some() { 84. } else { 52. };
        let mut obstacles = vec![navigation, self.shading_toolbar_rect()];
        if !self.modeling_live_in_properties() {
            obstacles.push(self.modeling_live_panel_rect(op));
        }
        let p = u.palette();
        let fields = if op.kind == Kind::Face { 2 } else { 1 };
        for field in 0..fields {
            let ring = field == 0 && matches!(op.kind, Kind::Vertex | Kind::VertexInset | Kind::Edge | Kind::Face | Kind::Inset);
            let mut strokes = Vec::new();
            if ring {
                let point = |i: usize| {
                    let angle = i as f32 * std::f32::consts::TAU / 96.;
                    (center + Vec2::new(angle.cos(), angle.sin()) * 56.).to_array()
                };
                strokes.extend((0..96).map(|i| (point(i), point(i + 1))));
            } else {
                let axis = Vec2::from_array(self.live_parameter_basis(op, field).0);
                let rail = matches!(op.kind, Kind::Cut | Kind::Slide);
                arrow(&mut strokes, center, axis, 1., if rail { 68. } else { 92. });
                if rail {
                    arrow(&mut strokes, center, axis, -1., 68.);
                }
                let side = Vec2::new(-axis.y, axis.x);
                let points = [
                    center + axis * 5.,
                    center + side * 5.,
                    center - axis * 5.,
                    center - side * 5.,
                ];
                strokes
                    .extend((0..4).map(|i| (points[i].to_array(), points[(i + 1) % 4].to_array())));
            }
            let strokes = clip(&strokes, area, &obstacles);
            if strokes.is_empty() {
                continue;
            }
            let action = Action::Author(A::LiveDrag(field as u8));
            u.path_hit(action.clone(), strokes.clone(), PICK);
            let active = matches!(&self.drag, Some(Drag::LiveParameter(d)) if d.field == field);
            let hovered =
                self.drag.is_none() && !self.has_modal() && u.hit(self.mouse) == Some(action);
            let mut color = p.accent;
            if active || hovered {
                for channel in &mut color[..3] {
                    *channel = *channel * 0.85 + 0.15;
                }
            } else if self.drag.is_some() {
                color[3] = 0.45;
            }
            // Shared clipped strokes keep the accent legible on bright models.
            u.smooth_segments(
                &strokes,
                if active || hovered { 6. } else { 5. },
                [0.015, 0.02, 0.03, 0.7],
            );
            u.smooth_segments(&strokes, if active || hovered { 3. } else { 2.2 }, if ring { p.muted } else { color });
            if ring {
                let fraction = self.live_field_range(op, field).ratio(op.values[field] as f64);
                let point = |angle: f32| (center + Vec2::new(angle.cos(), angle.sin()) * 56.).to_array();
                let start = -std::f32::consts::FRAC_PI_2;
                let end = start + std::f32::consts::TAU * fraction;
                let steps = (fraction * 96.).ceil() as usize;
                let arc: Vec<_> = (0..steps).map(|i| {
                    (point(start + (end-start)*i as f32/steps as f32),
                     point(start + (end-start)*(i+1) as f32/steps as f32))
                }).collect();
                u.smooth_segments(&clip(&arc, area, &obstacles), if active || hovered { 4. } else { 3. }, color);
                let tip = Vec2::from_array(point(end));
                let radial = (tip-center).normalize_or_zero();
                let tick = [((tip-radial*4.).to_array(), (tip+radial*4.).to_array())];
                u.smooth_segments(&clip(&tick, area, &obstacles), 3., color);
            }
            if active && ring {
                let guide = clip(&[(center.to_array(), self.mouse)], area, &obstacles);
                u.smooth_segments(&guide, 1., [color[0], color[1], color[2], 0.35]);
            }
            if active || hovered {
                self.draw_live_handle_value(u, op, field, center, area, &obstacles);
            }
        }
    }

    fn draw_live_handle_value(&self, u: &mut Ui, op: &LiveOperation, field: usize, center: Vec2,
        area: [f32; 4], obstacles: &[[f32; 4]]) {
        let width = 228_f32.min(area[2]-16.);
        if width < 120. || area[3] < 64. { return; }
        let height = 48.;
        let positions = [[center.x+70.,center.y+24.], [center.x-width-70.,center.y+24.],
            [center.x-width/2.,center.y+72.], [center.x-width/2.,center.y-120.]];
        let rect = positions.into_iter().map(|[x,y]| [x.clamp(area[0]+8.,area[0]+area[2]-width-8.),
            y.clamp(area[1]+8.,area[1]+area[3]-height-8.),width,height])
            .find(|r| !obstacles.iter().any(|o| intersect_rect(*r,padded(*o,5.)).is_some()));
        let Some([x,y,w,h]) = rect else { return; };
        let p = u.palette();
        let (label,_,_,_) = op.spec(field);
        let (label,unit) = label.strip_suffix(", м").map(|s| (s," м"))
            .or_else(|| label.strip_suffix(", %").map(|s| (s,"%"))).unwrap_or((label,""));
        u.outlined_surface([x,y,w,h], [p.panel[0],p.panel[1],p.panel[2],0.94],p.border);
        u.rect([x+1.,y+5.,2.,h-10.],p.accent);
        u.fitted_label([x+10.,y+6.,w-20.,20.],
            &format!("{label} · {}{unit}",ui::number_caption(op.values[field] as f64)),13.,p.text);
        u.fitted_label([x+10.,y+28.,w-20.,15.],"Shift — точно · Ctrl — шаг",10.,p.muted);
    }

    pub(in crate::app) fn modeling_live_scroll(&mut self, delta: f32) -> bool {
        if !self.modeling_live_owns_scroll() {
            return false;
        }
        let grip = matches!(self.drag, Some(Drag::LiveParameter(ref d)) if d.field == 0 && d.viewport)
            || matches!(
                self.ui.hit(self.mouse),
                Some(Action::Author(A::LiveDrag(0)))
            );
        if self.input.is_none() && !self.has_modal() && delta.is_finite() && delta != 0. {
            if let Some(op) = &self.modeling_live {
                let hovered_field = match self.ui.hit(self.mouse) {
                    Some(Action::Slide(action,_,_)) => match *action { Action::Author(A::LiveField(i)) => Some(i as usize), _ => None },
                    Some(Action::Author(A::LiveField(i))) => Some(i as usize),
                    _ => None,
                };
                let field = if grip && op.kind == Kind::Edge { Some(1) } else { hovered_field };
                if let Some(field) = field.filter(|i| *i < op.fields()) {
                    let mut values = op.values;
                    let (_,min,max,step) = op.spec(field);
                    let integer = op.kind == Kind::Edge && field == 1;
                    let step = if integer { 1. } else {
                        let step = if self.modifiers.control_key() {
                            if op.spec(field).0.ends_with('%') { 5. } else { op.scale*0.01 }
                        } else { step };
                        step * if self.modifiers.shift_key() { 0.1 } else { 1. }
                    };
                    values[field] = (values[field] + delta.signum()*delta.abs().round().max(1.)*step).clamp(min,max);
                    if values != op.values && self.preview_live_values(values).is_ok() {
                        if let Some(Drag::LiveParameter(d)) = &mut self.drag {
                            d.requested = values;
                            if d.field == field { d.value = values[field]; d.snap_origin = d.value; }
                        }
                    }
                    self.redraw();
                }
            }
        }
        true
    }
}
