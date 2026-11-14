//! Curve creation, control points, optional geometry and viewport drawing.
use super::*;
use crate::ui::CurveAction as C;
use poseforge_core::{CurveData, CurveGeometry, CurveInterpolation as I, CurvePreset, CurveProfile};
use glam::Vec2;

#[derive(Default)]
pub(super) struct CurveState {
    pub drawing: bool,
    pub edit_points: bool,
    pub point: usize,
    pub popup_scroll: f32,
}
#[derive(Clone)]
pub(super) enum CurveDrag {
    Point { id: u64, original: CurveData, index: usize, inverse: Mat4, origin: Vec3, normal: Vec3, start: Vec3 },
    Stroke { points: Vec<Vec3>, last: [f32; 2], origin: Vec3, normal: Vec3 },
}
fn move_point(curve: &mut CurveData, index: usize, p: [f32; 3]) {
    let delta = Vec3::from_array(p) - Vec3::from_array(curve.points[index]);
    curve.points[index] = p;
    if curve.interpolation == I::Bezier && index.is_multiple_of(3) {
        let last = curve.points.len() - 1;
        let mut handles = Vec::new();
        if index > 0 { handles.push(index - 1); }
        if index < last { handles.push(index + 1); }
        if curve.closed && (index == 0 || index == last) {
            curve.points[0] = p; curve.points[last] = p;
            handles.extend([1, last - 1]);
        }
        handles.sort_unstable(); handles.dedup();
        for i in handles { curve.points[i] = (Vec3::from_array(curve.points[i]) + delta).to_array(); }
    }
}
fn plane_point(view: &poseforge_render::View, viewport: [f32; 4], pixel: [f32; 2], origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let (eye, ray) = crate::gizmo::pointer_ray(view, viewport, pixel)?;
    let denominator = ray.dot(normal);
    if denominator.abs() < 1e-6 { return None; }
    let t = (origin - eye).dot(normal) / denominator;
    let p = eye + ray * t;
    (t >= 0. && p.is_finite() && p.abs().max_element() < 10000.).then_some(p)
}
impl App {
    pub(super) fn selected_curve(&self) -> Option<(u64, &CurveData)> {
        let id = self.editor.selected?;
        Some((id, self.editor.project.node(id)?.curve.as_ref()?))
    }
    pub(super) fn frame_curve(&mut self) -> bool {
        let Some((id, curve)) = self.selected_curve() else { return false; };
        let Ok(evaluated) = self.evaluated_interactive() else { return false; };
        let Some(model) = evaluated.global.get(&id) else { return false; };
        let mut min = Vec3::splat(f32::INFINITY); let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &point in &curve.points { let p = model.transform_point3(Vec3::from_array(point)); min = min.min(p); max = max.max(p); }
        self.target = (min + max) * 0.5; self.distance = (max - min).length().max(0.5) * 1.8; true
    }
    pub(super) fn create_curve(&mut self, preset: usize) {
        let Some(preset) = CurvePreset::ALL.get(preset).copied() else { return; };
        let result = self.editor.create_curve(preset.label(), preset.curve());
        if let Some(id) = self.result(result) { self.select_new_curve(id); }
    }
    fn select_new_curve(&mut self, id: u64) {
        self.editor.mode = EditorMode::Scene; self.editor.playing = false;
        self.authoring = Default::default(); self.primitive_panel = None;
        self.curves = CurveState { edit_points: true, ..Default::default() };
        self.action(Action::Select(id)); self.set_properties_tab(0);
        self.preferences.right_panel = true; self.tool = Tool::Move;
        self.primitive_panel = Some(id); self.primitive_collapsed = false;
        self.invalidate(); self.status = "Кривая создана · параметры в полупрозрачной панели · Esc — закрыть панель".into();
    }
    pub(super) fn curve_action(&mut self, action: C) {
        if self.job.is_some() || self.scene_write_pending() || self.editor.playing || self.editor.has_staged() { return; }
        if let C::Create(index) = action { self.create_curve(index as usize); return; }
        if action == C::Draw {
            self.primitive_panel = None;
            self.set_properties_tab(0); self.clear_light_selection();
            self.focus_pick = None; self.light_aim = None; self.weights = Default::default();
            self.curves.drawing = !self.curves.drawing;
            self.authoring = Default::default(); self.editor.mode = EditorMode::Scene;
            self.status = "Рисование кривой: тяните ЛКМ в просмотре · плоскость текущего вида · Esc — отмена".into();
            return;
        }
        let Some((id, source)) = self.selected_curve() else { return; };
        let mut curve = source.clone();
        if !self.node_selectable(id) { self.status = "Кривая скрыта или заблокирована".into(); return; }
        self.curves.point = self.curves.point.min(curve.points.len() - 1);
        let point = self.curves.point;
        match action {
            C::EditPoints => { self.curves.edit_points = !self.curves.edit_points; self.set_properties_tab(0); return; }
            C::Point(step) => { self.curves.point = (point as isize + step as isize).rem_euclid(curve.points.len() as isize) as usize; return; }
            C::Field(field) => {
                if let Some((value, _, _, _)) = self.curve_field(field) { self.input(Action::Curve(action), ui::number_caption(value as f64)); }
                return;
            }
            C::AddGeometry => curve.geometry = Some(CurveGeometry::default()),
            C::RemoveGeometry => curve.geometry = None,
            C::Profile(profile) => { if let Some(g) = &mut curve.geometry { g.profile = if profile == 1 { CurveProfile::Ribbon } else { CurveProfile::Tube }; } }
            C::Caps => { if let Some(g) = &mut curve.geometry { g.caps = !g.caps; } }
            C::Closed => {
                if curve.interpolation == I::Bezier {
                    if curve.closed { curve.points.truncate(curve.points.len() - 3); }
                    else {
                        let first = Vec3::from_array(curve.points[0]); let last = Vec3::from_array(*curve.points.last().unwrap());
                        curve.points.extend([last.lerp(first, 1. / 3.).to_array(), last.lerp(first, 2. / 3.).to_array(), first.to_array()]);
                    }
                }
                curve.closed = !curve.closed;
            }
            C::Interpolation(kind) => {
                let target = match kind { 0 => I::Polyline, 2 => I::Bezier, _ => I::Smooth };
                if target == curve.interpolation { return; }
                if curve.interpolation == I::Bezier {
                    curve.points = curve.points.iter().step_by(3).copied().collect();
                    if curve.closed { curve.points.pop(); }
                }
                if target == I::Bezier {
                    let p: Vec<_> = curve.points.iter().copied().map(Vec3::from_array).collect(); let n = p.len();
                    let at = |i: isize| p[if curve.closed { i.rem_euclid(n as isize) as usize } else { i.clamp(0, n as isize - 1) as usize }];
                    let mut points = vec![p[0].to_array()];
                    for i in 0..(if curve.closed { n } else { n - 1 }) {
                        let i = i as isize;
                        points.extend([(at(i) + (at(i + 1) - at(i - 1)) / 6.).to_array(),
                            (at(i + 1) - (at(i + 2) - at(i)) / 6.).to_array(), at(i + 1).to_array()]);
                    }
                    curve.points = points;
                }
                curve.interpolation = target;
            }
            C::AddPoint if curve.interpolation != I::Bezier && curve.points.len() < poseforge_core::MAX_CURVE_POINTS => {
                let p = Vec3::from_array(curve.points[point]);
                let next = if point + 1 < curve.points.len() { p.lerp(Vec3::from_array(curve.points[point + 1]), 0.5) }
                    else if curve.closed { p.lerp(Vec3::from_array(curve.points[0]), 0.5) }
                    else { p + Vec3::X * 0.25 };
                curve.points.insert(point + 1, next.to_array()); self.curves.point = point + 1;
            }
            C::DeletePoint if curve.interpolation != I::Bezier && curve.points.len() > (if curve.closed { 3 } else { 2 }) => { curve.points.remove(point); }
            C::Convert => { let result = self.editor.convert_curve_to_mesh(id); self.result(result); self.curves.edit_points = false; self.invalidate_geometry(); return; }
            _ => return,
        }
        let result = self.editor.set_curve(id, curve);
        self.result(result); self.invalidate_geometry(); self.update_dirty();
    }
    pub(super) fn curve_field(&self, field: u8) -> Option<(f32, f32, f32, &'static str)> {
        let (_, c) = self.selected_curve()?;
        Some(match field {
            0..=2 => (c.points[self.curves.point.min(c.points.len() - 1)][field as usize], -10000., 10000., ["X", "Y", "Z"][field as usize]),
            3 => (c.resolution as f32, 1., 64., "Детализация кривой"),
            4 => (c.geometry?.radius, 0.001, 1000., "Радиус / полуширина, м"),
            5 => (c.geometry?.sides as f32, 3., 32., "Стороны сечения"),
            6 => (c.geometry?.rotation, -3600., 3600., "Вращение ленты, °"),
            _ => return None,
        })
    }
    pub(super) fn curve_confirm(&mut self, action: &Action, text: &str) -> Result<bool, String> {
        let Action::Curve(C::Field(field)) = action else { return Ok(false); };
        let (_, min, max, _) = self.curve_field(*field).ok_or("Выберите кривую")?;
        let value = surfaces::number(text, min, max)?;
        if matches!(*field, 3 | 5) && value.fract() != 0. { return Err("Введите целое число".into()); }
        let (id, source) = self.selected_curve().ok_or("Выберите кривую")?;
        let mut curve = source.clone();
        match *field {
            0..=2 => { let i = self.curves.point.min(curve.points.len() - 1); let mut p = curve.points[i]; p[*field as usize] = value; move_point(&mut curve, i, p); }
            3 => curve.resolution = value as u32,
            4 => curve.geometry.as_mut().unwrap().radius = value,
            5 => curve.geometry.as_mut().unwrap().sides = value as u32,
            6 => curve.geometry.as_mut().unwrap().rotation = value,
            _ => return Err("Неизвестное поле кривой".into()),
        }
        self.editor.set_curve(id, curve).map_err(|e| e.to_string())?;
        self.invalidate_geometry(); self.update_dirty(); Ok(true)
    }
    pub(super) fn curve_creation_height(&self) -> f32 {
        self.selected_curve().map_or(322., |(_, curve)| match curve.geometry {
            None => 322., Some(g) if g.profile == CurveProfile::Tube => 438., Some(_) => 404.,
        })
    }
    pub(super) fn scroll_curve_creation_panel(&mut self, delta: f32) {
        if self.selected_curve().is_none() || self.primitive_collapsed { return; }
        if let Some(rect) = self.primitive_panel_rect() {
            let maximum = (self.curve_creation_height() - rect[3]).max(0.);
            self.curves.popup_scroll = (self.curves.popup_scroll - delta * 34.).clamp(0., maximum);
            self.redraw();
        }
    }
    pub(super) fn draw_curve_creation_panel(&self, u: &mut Ui, rect: [f32; 4]) {
        use crate::ui::PrimitiveAction as P;
        let Some((id, curve)) = self.selected_curve() else { return; };
        let [x, y, w, h] = rect;
        let p = u.palette();
        u.floating_surface(rect, 0.90, true);
        u.hits.push(ui::Hit { rect, action: Action::Primitive(P::Backdrop) });
        let title = self.editor.project.node(id).map_or("Кривая", |n| n.name.as_str());
        u.quiet_icon_button([x + 8., y + 4., w - 48., 30.], title, Action::Primitive(P::Collapse), false, Icon::Curve);
        u.quiet_icon_button([x + w - 36., y + 4., 28., 28.], "", Action::Primitive(P::Close), false, Icon::Close);
        if self.primitive_collapsed { return; }
        let maximum = (self.curve_creation_height() - h).max(0.);
        let scroll = self.curves.popup_scroll.clamp(0., maximum);
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        u.set_cursor([self.mouse[0], self.mouse[1] + scroll]);
        let left = x + 14.; let width = w - 28.; let mut row = y + 46.;
        self.draw_creation_orientation(u,left,row,width);row+=38.;
        u.quiet_icon_button([left, row, 28., 28.], "", Action::Curve(C::Point(-1)), false, Icon::Previous);
        u.fitted_label([left + 34., row + 6., width - 68., 18.],
            &format!("Точка {} / {}", self.curves.point.min(curve.points.len() - 1) + 1, curve.points.len()), 12., p.text);
        u.quiet_icon_button([left + width - 28., row, 28., 28.], "", Action::Curve(C::Point(1)), false, Icon::Next);
        row += 34.;
        let cell = (width - 8.) / 3.;
        for field in 0..3 {
            let Some((value, _, _, label)) = self.curve_field(field) else { continue; };
            let limit = 10_f64.max(value.abs() as f64 * 2.);
            u.number_field([left + field as f32 * (cell + 4.), row, cell, 44.], label, value as f64,
                Action::Curve(C::Field(field)), SliderRange::new(-limit, limit, 0.01));
        }
        row += 50.;
        u.number_field([left, row, width, 32.], "Детализация", curve.resolution as f64,
            Action::Curve(C::Field(3)), SliderRange::new(1., 64., 1.));
        row += 38.;
        u.checkbox([left, row, width, 28.], "Замкнуть кривую", Action::Curve(C::Closed), curve.closed);
        row += 34.;
        if let Some(g) = curve.geometry {
            let half = (width - 6.) * 0.5;
            for (i, label) in ["Трубка", "Лента"].into_iter().enumerate() {
                u.quiet_button([left + i as f32 * (half + 6.), row, half, 30.], label,
                    Action::Curve(C::Profile(i as u8)), (i == 0) == (g.profile == CurveProfile::Tube));
            }
            row += 36.;
            u.number_field([left, row, half, 44.],
                "Радиус, м", g.radius as f64, Action::Curve(C::Field(4)), SliderRange::new(0.001, 2_f64.max(g.radius as f64), 0.001));
            if g.profile == CurveProfile::Tube {
                u.number_field([left + half + 6., row, half, 44.], "Стороны", g.sides as f64,
                    Action::Curve(C::Field(5)), SliderRange::new(3., 32., 1.));
            } else {
                u.number_field([left + half + 6., row, half, 44.], "Вращение, °", g.rotation as f64,
                    Action::Curve(C::Field(6)), SliderRange::new(-180., 180., 1.));
            }
            row += 50.;
            if g.profile == CurveProfile::Tube {
                u.checkbox([left, row, width, 28.], "Закрыть торцы", Action::Curve(C::Caps), g.caps);
                row += 34.;
            }
            u.quiet_button([left, row, width, 28.], "Убрать геометрию", Action::Curve(C::RemoveGeometry), false);
        } else {
            u.quiet_icon_button([left, row, width, 32.], "Добавить геометрию", Action::Curve(C::AddGeometry), false, Icon::Grid);
        }
        clip_scrolled(u, start, [x + 8., y + 40., w - 16., h - 84.], scroll);
        u.set_cursor(self.mouse);
        if maximum > 0. {
            u.fitted_label([left, y + h - 33., width - 92., 18.], "Колесо · прокрутка", 10., p.muted);
        }
        let button_width = if maximum > 0. { 86. } else { width };
        u.quiet_button([left + width - button_width, y + h - 40., button_width, 28.], "Готово", Action::Primitive(P::Close), true);
    }
    pub(super) fn draw_curve_properties(&self, u: &mut Ui, rx: f32, rw: f32) {
        let Some((_, c)) = self.selected_curve() else { return; }; let x = rx + 16.; let w = rw - 32.;
        let p = u.palette();
        u.checkbox([x, 534., w - 34., 28.], "Редактировать точки в сцене", Action::Curve(C::EditPoints), self.curves.edit_points);
        u.quiet_icon_button([x + w - 28., 534., 28., 28.], "", Action::Primitive(ui::PrimitiveAction::Open), false, Icon::Settings);
        for (i, (label, kind)) in [("Ломаная", I::Polyline), ("Плавная", I::Smooth), ("Безье", I::Bezier)].into_iter().enumerate() {
            u.quiet_button([x + i as f32 * (w + 4.) / 3., 572., (w - 8.) / 3., 28.], label, Action::Curve(C::Interpolation(i as u8)), c.interpolation == kind);
        }
        u.quiet_icon_button([x, 614., 28., 28.], "", Action::Curve(C::Point(-1)), false, Icon::Previous);
        u.fitted_label([x + 34., 621., w - 68., 18.], &format!("Точка {} / {}", self.curves.point.min(c.points.len() - 1) + 1, c.points.len()), 12., p.text);
        u.quiet_icon_button([x + w - 28., 614., 28., 28.], "", Action::Curve(C::Point(1)), false, Icon::Next);
        for field in 0..3 { let (v, _, _, label) = self.curve_field(field).unwrap();
            u.number_field([x + field as f32 * (w + 4.) / 3., 658., (w - 8.) / 3., 44.], label, v as f64,
                Action::Curve(C::Field(field)), SliderRange::new(-10_f64.max(v.abs() as f64 * 2.), 10_f64.max(v.abs() as f64 * 2.), 0.01));
        }
        u.number_field([x, 714., w, 44.], "Детализация", c.resolution as f64, Action::Curve(C::Field(3)), SliderRange::new(1., 64., 1.));
        let half = (w - 6.) * 0.5;
        for (i, label, action, enabled) in [(0, "+ Точка", C::AddPoint, c.points.len() < poseforge_core::MAX_CURVE_POINTS),
            (1, "Удалить точку", C::DeletePoint, c.points.len() > (if c.closed { 3 } else { 2 }))] {
            let r = [x + i as f32 * (half + 6.), 772., half, 28.];
            if enabled && c.interpolation != I::Bezier { u.quiet_button(r, label, Action::Curve(action), false); } else { u.disabled_button(r, label); }
        }
        u.checkbox([x, 812., w, 28.], "Замкнуть кривую", Action::Curve(C::Closed), c.closed);
        if let Some(g) = c.geometry {
            for (i, label) in ["Трубка", "Лента"].into_iter().enumerate() {
                u.quiet_icon_button([x + i as f32 * (half + 6.), 858., half, 30.], label, Action::Curve(C::Profile(i as u8)),
                    (i == 0) == (g.profile == CurveProfile::Tube), if i == 0 { Icon::CurveCircle } else { Icon::Plane });
            }
            u.number_field([x, 902., if g.profile == CurveProfile::Ribbon {half} else {w}, 44.], "Радиус, м", g.radius as f64, Action::Curve(C::Field(4)), SliderRange::new(0.001, 2., 0.001));
            if g.profile == CurveProfile::Ribbon {
                u.number_field([x + half + 6., 902., half, 44.], "Вращение, °", g.rotation as f64,
                    Action::Curve(C::Field(6)), SliderRange::new(-180.,180.,1.));
            }
            if g.profile == CurveProfile::Tube {
                u.number_field([x, 952., w, 44.], "Стороны сечения", g.sides as f64, Action::Curve(C::Field(5)), SliderRange::new(3., 32., 1.));
                u.checkbox([x, 1006., w, 28.], "Закрыть торцы", Action::Curve(C::Caps), g.caps);
            }
            let tail = if g.profile == CurveProfile::Tube { 1044. } else { 952. };
            u.quiet_button([x, tail, w, 28.], "Убрать геометрию", Action::Curve(C::RemoveGeometry), false);
            u.icon_button([x, tail + 40., w, 30.], "Преобразовать в меш", Action::Curve(C::Convert), false, Icon::Grid);
        } else { u.icon_button([x, 858., w, 32.], "Добавить геометрию", Action::Curve(C::AddGeometry), false, Icon::Grid); }
    }
    pub(super) fn curve_pointer_down(&mut self) -> bool {
        if self.has_modal() || self.editor.mode != EditorMode::Scene || self.editor.playing || self.job.is_some() || self.authoring.editing.is_some() { return false; }
        let view = self.view(); let normal = (view.target - view.eye).normalize_or_zero();
        if self.curves.drawing {
            let origin = self.target;
            if let Some(p) = plane_point(&view, self.viewport(), self.mouse, origin, normal) {
                self.drag = Some(Drag::Curve(CurveDrag::Stroke { points: vec![p], last: self.mouse, origin, normal }));
                self.drag_button = Some(MouseButton::Left);
            } return true;
        }
        if self.curves.edit_points {
            if let Some((id, curve)) = self.selected_curve().filter(|(id, _)| self.node_selectable(*id)) {
                let original = curve.clone();
                let model = self.evaluated_interactive().ok().and_then(|e| e.global.get(&id).copied()).unwrap_or(Mat4::IDENTITY);
                let hit = original.points.iter().enumerate().filter_map(|(index, p)| {
                    let world = model.transform_point3(Vec3::from_array(*p)); let screen = self.project_point(world)?;
                    let distance = Vec2::from_array(screen).distance(Vec2::from_array(self.mouse));
                    (distance <= 8.).then_some((index, world, distance))
                }).min_by(|a, b| a.2.total_cmp(&b.2));
                if let Some((index, origin, _)) = hit {
                    let Some(start) = plane_point(&view, self.viewport(), self.mouse, origin, normal) else { return true; };
                    let inverse = model.inverse(); if !inverse.is_finite() { return true; }
                    self.curves.point = index;
                    self.editor.begin_edit("Переместить точку кривой");
                    self.drag = Some(Drag::Curve(CurveDrag::Point { id, original, index, inverse, origin, normal, start }));
                    self.drag_button = Some(MouseButton::Left); return true;
                }
            }
        }
        if self.selected_gizmo().and_then(|g| g.hit(self.mouse)).is_some() { return false; }
        if let Some(id)=self.pick_curve_at(self.mouse) {self.select_in_viewport(id);self.set_properties_tab(0);return true;}false
    }
    pub(super) fn pick_curve_at(&self,pixel:[f32;2])->Option<u64> {
        let matrix=self.view().view_projection();let viewport=self.viewport();let pointer=Vec2::from_array(pixel);
        let evaluated=self.evaluated_interactive().ok()?;let globals=&evaluated.global;let mut best:Option<(u64,f32,f32)>=None;let mut budget=24_000usize;
        for node in &self.editor.project.nodes {
            if budget==0 {break;}
            let Some(curve)=node.curve.as_ref().filter(|_|self.node_selectable(node.id)&&self.in_isolation(node.id)) else{continue;};
            let Some(model)=globals.get(&node.id) else{continue;};let Ok(points)=curve.samples() else{continue;};
            for pair in points.windows(2).take(budget) {
                let Some((a,b,depth))=crate::bones::project_segment(matrix,viewport,model.transform_point3(pair[0]),model.transform_point3(pair[1])) else{continue;};
                let d=b-a;let t=((pointer-a).dot(d)/d.length_squared().max(1e-8)).clamp(0.,1.);let distance=pointer.distance(a+d*t);
                if distance<6. && best.is_none_or(|(_,old,z)|distance<old-0.5 || ((distance-old).abs()<=0.5&&depth<z)) {best=Some((node.id,distance,depth));}
            }budget=budget.saturating_sub(points.len().saturating_sub(1));
        }best.map(|(id,_,_)|id)
    }
    pub(super) fn update_curve_drag(&mut self, mut drag: CurveDrag, pixel: [f32; 2]) {
        match &mut drag {
            CurveDrag::Point { id, original, index, inverse, origin, normal, start } => {
                if let Some(p) = plane_point(&self.view(), self.viewport(), pixel, *origin, *normal) {
                    let mut curve = original.clone();
                    let delta = (p - *start) * if self.modifiers.shift_key() { 0.1 } else { 1. };
                    let mut local = Vec3::from_array(original.points[*index]) + inverse.transform_vector3(delta);
                    if self.modifiers.control_key() { local = (local * 10.).round() / 10.; }
                    move_point(&mut curve, *index, local.to_array());
                    let result = self.editor.set_curve(*id, curve); self.result(result); self.invalidate_geometry();
                }
            }
            CurveDrag::Stroke { points, last, origin, normal } => {
                if ui::contains(self.viewport(), pixel) && Vec2::from_array(*last).distance(Vec2::from_array(pixel)) >= 4. {
                    if let Some(p) = plane_point(&self.view(), self.viewport(), pixel, *origin, *normal) {
                        if points.len() >= poseforge_core::MAX_CURVE_POINTS { *points = points.iter().step_by(2).copied().collect(); }
                        points.push(p); *last = pixel;
                    }
                }
            }
        }
        self.drag = Some(Drag::Curve(drag));
    }
    pub(super) fn finish_curve_drag(&mut self, drag: CurveDrag) {
        match drag {
            CurveDrag::Point { .. } => { let result = self.editor.commit_edit(); self.result(result); }
            CurveDrag::Stroke { points, .. } => {
                self.curves.drawing = false;
                if points.len() < 2 { self.status = "Нарисуйте линию, удерживая левую кнопку".into(); return; }
                let curve = CurveData { points: points.into_iter().map(|p| p.to_array()).collect(), resolution: 6, ..Default::default() };
                let result = self.editor.create_curve("Нарисованная кривая", curve);
                if let Some(id) = self.result(result) { self.select_new_curve(id); }
            }
        }
        self.update_dirty(); self.invalidate_geometry();
    }
    pub(super) fn draw_curves(&self, u: &mut Ui) {
        let editing_path = self.editing_path_curve();
        if self.editor.mode == EditorMode::Render && self.path_pick.is_none() { return; }
        if !self.curves.drawing
            && !matches!(self.drag, Some(Drag::Curve(CurveDrag::Stroke { .. })))
            && !self.editor.project.nodes.iter().any(|n| n.curve.is_some())
        { return; }
        let Ok(evaluated) = self.evaluated_interactive() else { return; };
        let globals = &evaluated.global;
        let viewport = self.viewport(); let matrix = self.view().view_projection(); let palette = u.palette();
        let line = |u: &mut Ui, a: Vec3, b: Vec3, color, width| {
            if let Some((a, b, _)) = crate::bones::project_segment(matrix, viewport, a, b) {
                let clip = [viewport[0] + 3., viewport[1] + 3., (viewport[2] - 6.).max(1.), (viewport[3] - 6.).max(1.)];
                if let Some((a, b)) = crate::bones::clip_segment(a, b, clip) { u.smooth_segment(a.to_array(), b.to_array(), width, color); }
            }
        };
        let mut budget = 24_000;
        for node in &self.editor.project.nodes {
            if budget == 0 { break; }
            let selected = self.editor.selected == Some(node.id);
            let path = editing_path == Some(node.id);
            let Some(curve) = node.curve.as_ref().filter(|_| self.editor.project.node_visible(node.id, false) && self.in_isolation(node.id)) else { continue; };
            if !selected && !path && curve.geometry.is_some() && self.path_pick.is_none() { continue; }
            let Some(model) = globals.get(&node.id) else { continue; };
            let Ok(points) = curve.samples() else { continue; };
            for pair in points.windows(2).take(budget) {
                line(u, model.transform_point3(pair[0]), model.transform_point3(pair[1]), if selected || path { palette.accent } else { palette.text }, if selected || path { 2. } else { 1.5 });
            }
            budget = budget.saturating_sub(points.len());
            if selected && self.curves.edit_points {
                if curve.interpolation == I::Bezier {
                    for segment in curve.points.windows(4).step_by(3) {
                        for (a, b) in [(segment[0], segment[1]), (segment[2], segment[3])] { line(u, model.transform_point3(Vec3::from_array(a)), model.transform_point3(Vec3::from_array(b)), palette.muted, 1.); }
                    }
                }
                for (i, p) in curve.points.iter().enumerate() {
                    if let Some([x, y]) = self.project_point(model.transform_point3(Vec3::from_array(*p))).filter(|p| ui::contains([viewport[0] + 4., viewport[1] + 4., (viewport[2] - 8.).max(0.), (viewport[3] - 8.).max(0.)], *p)) {
                        let active = i == self.curves.point;
                        u.rounded_rect([x - 4., y - 4., 8., 8.], if active { palette.accent } else { palette.text }, 4.);
                    }
                }
            }
        }
        if let Some(Drag::Curve(CurveDrag::Stroke { points, .. })) = &self.drag {
            for pair in points.windows(2) { line(u, pair[0], pair[1], palette.accent, 2.); }
        }
        if self.curves.drawing {
            u.fitted_label([viewport[0] + 18., viewport[1] + viewport[3] - 30., viewport[2] - 36., 22.],
                "Рисуйте ЛКМ · отпустите для создания кривой · Esc — отмена", 12., palette.accent);
        }
    }
}

#[cfg(test)]
mod anchor_motion_tests {
    use super::*;
    #[test]
    fn bezier_anchor_translates_both_handles_and_closed_seam_once() {
        let mut c=CurvePreset::Circle.curve();let source=c.clone();let delta=Vec3::new(0.4,0.7,-0.2);
        move_point(&mut c,3,(Vec3::from_array(source.points[3])+delta).to_array());
        for i in 0..c.points.len() {
            let expected=Vec3::from_array(source.points[i])+if (2..=4).contains(&i) {delta} else {Vec3::ZERO};
            assert!(Vec3::from_array(c.points[i]).distance(expected)<1e-6);
        }
        let mut c=source.clone();let last=c.points.len()-1;
        move_point(&mut c,last,(Vec3::from_array(source.points[0])+delta).to_array());
        for i in [0,1,last-1,last] {assert!(Vec3::from_array(c.points[i]).distance(Vec3::from_array(source.points[i])+delta)<1e-6);}
        c.validate().unwrap();
        let handles=c.clone();move_point(&mut c,1,[0.,0.,0.]);assert_eq!(c.points[0],handles.points[0]);assert_eq!(c.points[last-1],handles.points[last-1]);
    }
}
