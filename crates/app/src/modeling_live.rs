//! Frozen-source previews: fields and viewport handles share one undo transaction.
use super::component_selection::SelectionSnapshot;
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S};
use poseforge_core::{Mesh, MeshEdit};
#[path = "modeling_live/handles.rs"]
mod handles;
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Individual,
    VertexExtrude,
    VertexInset,
    Vertex,
    Edge,
    Face,
    Inset,
    Extrude,
    Cut,
    Slide,
}
pub(super) struct LiveOperation {
    node: u64,
    original: Mesh,
    selection: SelectionSnapshot,
    faces: BTreeSet<usize>,
    vertices: BTreeSet<usize>,
    kind: Kind,
    values: [f32; 3],
    defaults: [f32; 3],
    anchor: Vec3,
    direction: Vec3,
    scale: f32,
    error: Option<String>,
    expanded: bool,
}
impl LiveOperation {
    pub(super) fn default_value(&self, field: u8) -> Option<f64> {
        (usize::from(field) < self.fields()).then(|| self.defaults[usize::from(field)] as f64)
    }
    fn label(&self) -> &'static str {
        match self.kind {
            Kind::Individual=>"Индивидуальный экструд",
            Kind::VertexExtrude => "Экструдирование точек",
            Kind::VertexInset => "Вставка вокруг точек",
            Kind::Vertex => "Фаска вершин",
            Kind::Edge => "Фаска рёбер",
            Kind::Face => "Фаска граней",
            Kind::Inset => "Вставка граней",
            Kind::Extrude => "Экструдирование",
            Kind::Cut => "Разрез петлёй",
            Kind::Slide => "Сдвиг по поверхности",
        }
    }
    fn fields(&self) -> usize {
        match self.kind {
            Kind::Edge => 3,
            Kind::Face => 2,
            _ => 1,
        }
    }
    fn spec(&self, i: usize) -> (&'static str, f32, f32, f32) {
        match (self.kind, i) {
            (Kind::Vertex | Kind::VertexInset, 0) => ("Срез угла, %", 0., 45., 0.25),
            (Kind::Edge, 0) => ("Ширина, м", 0., 1000., self.scale * 0.002),
            (Kind::Edge, 1) => ("Сегменты", 1., 32., 0.1),
            (Kind::Edge, 2) => ("Профиль, %", 0., 100., 0.5),
            (Kind::Face | Kind::Inset, 0) => ("Отступ, %", 0., 95., 0.25),
            (Kind::Slide, 0) => ("Сдвиг, %", -99., 99., 0.25),
            (Kind::Cut, 0) => ("Положение, %", 1., 99., 0.25),
            _ => ("Расстояние, м", -1000., 1000., self.scale * 0.002),
        }
    }
    fn slider_bounds(&self, i: usize) -> (f32, f32) {
        let (_, min, max, _) = self.spec(i);
        if self.kind == Kind::Edge && i == 0 {
            (min, (self.scale * 0.3).max(self.values[i] * 1.2).min(max))
        } else if matches!(self.kind, Kind::Extrude | Kind::Individual | Kind::VertexExtrude) || (self.kind == Kind::Face && i == 1) {
            let extent = self.scale.max(self.values[i].abs() * 1.2);
            ((-extent).max(min), extent.min(max))
        } else {
            (min, max)
        }
    }
    fn edit(&self, v: [f32; 3]) -> MeshEdit {
        match self.kind {
            Kind::Individual=>MeshEdit::Bevel {faces:self.faces.clone(),inset:0.,shift:v[0]},
            Kind::VertexExtrude => MeshEdit::ExtrudeVertices { vertices:self.vertices.clone(), distance:v[0] },
            Kind::Vertex | Kind::VertexInset => MeshEdit::VertexBevel {
                vertices: self.vertices.clone(),
                amount: v[0] / 100.,
            },
            Kind::Edge => MeshEdit::EdgeBevel {
                edges: self.selection.edges.clone(),
                width: v[0],
                segments: v[1] as u32,
                profile: v[2] / 100.,
            },
            Kind::Face | Kind::Inset => MeshEdit::Bevel {
                faces: self.faces.clone(),
                inset: v[0] / 100.,
                shift: if self.kind == Kind::Face { v[1] } else { 0. },
            },
            Kind::Extrude => MeshEdit::Extrude {
                faces: self.faces.clone(),
                distance: v[0],
            },
            Kind::Slide => MeshEdit::Slide {
                vertices: self.vertices.clone(),
                edges: if self.selection.mode == S::Edge {
                    self.selection.edges.clone()
                } else {
                    BTreeSet::new()
                },
                amount: v[0] / 100.,
            },
            Kind::Cut => MeshEdit::LoopCut {
                edge: *self.selection.edges.first().unwrap(),
                position: v[0] / 100.,
            },
        }
    }
}
#[derive(Clone)]
pub(super) struct ParameterDrag {
    start: [f32; 3],
    pointer: [f32; 2],
    value: f32,
    field: usize,
    fine: bool,
    snapped: bool,
    snap_origin: f32,
    step: f32,
    direction: [f32; 2],
    requested: [f32; 3],
    viewport: bool,
    range: SliderRange,
}
impl App {
    pub(super) fn modeling_live_action(&mut self, a: A) -> bool {
        let mut kind = match a {
            A::Slide => Kind::Slide,
            A::EdgeBevel => Kind::Edge,
            A::Bevel => match self.authoring.selection_mode {
                S::Vertex => Kind::Vertex, S::Edge => Kind::Edge, S::Face => Kind::Face,
            },
            A::Inset => Kind::Inset,
            A::Extrude => Kind::Extrude,
            A::ExtrudeIndividual=>Kind::Individual,
            A::LoopCut => Kind::Cut,
            A::LiveAccept => {
                self.accept_modeling_live();
                return true;
            }
            A::LiveCancel => {
                self.cancel_modeling_live();
                return true;
            }
            A::LiveBackdrop => return true,
            A::LiveToggle => {
                if let Some(op) = &mut self.modeling_live {
                    op.expanded = !op.expanded;
                }
                return true;
            }
            A::LiveDrag(i) => {
                self.begin_live_parameter(i as usize, None);
                return true;
            }
            A::LiveField(i) => {
                if let Some(op) = &self.modeling_live {
                    if (i as usize) < op.fields() {
                        self.input(
                            Action::Author(a),
                            ui::number_caption(op.values[i as usize] as f64),
                        );
                    }
                }
                return true;
            }
            _ => return false,
        };
        self.cancel_modeling_live();
        let Some(node) = self.authoring.editing else {
            return true;
        };
        let faces=self.selected_face_region();
        let vertices=self.edit_vertices();
        if faces.is_empty() && self.authoring.selection_mode!=S::Face {
            if kind==Kind::Extrude {kind=Kind::VertexExtrude;}
            if kind==Kind::Inset {kind=Kind::VertexInset;}
        }
        let valid = match kind {
            Kind::VertexExtrude | Kind::VertexInset => !vertices.is_empty(),
            Kind::Vertex => self.authoring.selection_mode == S::Vertex && !self.authoring.vertices.is_empty(),
            Kind::Slide => {
                !vertices.is_empty()
            }
            Kind::Edge => {
                self.authoring.selection_mode == S::Edge && !self.authoring.edges.is_empty()
            }
            Kind::Cut => {
                self.authoring.selection_mode == S::Edge && self.authoring.edges.len() == 1
            }
            _ => !faces.is_empty(),
        };
        if !valid {
            self.status = "Выберите компоненты в подходящем режиме для этой операции".into();
            return true;
        }
        let Ok(original) = self.editor.editable_mesh(node).cloned() else {
            return true;
        };
        let center = self.edit_center().unwrap_or(Vec3::ZERO);
        let model = self
            .editor
            .project
            .global_transforms()
            .ok()
            .and_then(|g| g.get(&node).copied())
            .unwrap_or(Mat4::IDENTITY);
        let anchor = model.transform_point3(center);
        let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for v in &original.vertices {
            let p = Vec3::from_array(v.position);
            min = min.min(p);
            max = max.max(p);
        }
        let scale = (max - min).length().max(0.001);
        // Match the area-weighted extrusion direction in the mesh command.
        // Transform a displacement, not a shading normal: object scaling must
        // affect the projected distance in exactly the same way as geometry.
        let mut direction = Vec3::ZERO;
        if matches!(kind, Kind::Extrude | Kind::Individual | Kind::VertexExtrude | Kind::Face) {
            let normal_faces=if kind==Kind::VertexExtrude {
                let g=self.mesh_cache(&original);
                let points:BTreeSet<_>=vertices.iter().map(|&v|g.graph.canonical[v]).collect();
                (0..original.face_count()).filter(|&f|original.face_vertices(f).unwrap().iter().any(|&v|points.contains(&g.graph.canonical[v as usize]))).collect()
            } else {faces.clone()};
            for &f in &normal_faces {
                if let Some(face) = original.face_vertices(f) {
                    let p =
                        |i: usize| Vec3::from_array(original.vertices[face[i] as usize].position);
                    for i in 1..face.len().saturating_sub(1) {
                        direction += (p(i) - p(0)).cross(p(i + 1) - p(0));
                    }
                }
            }
            direction = model.transform_vector3(direction.try_normalize().unwrap_or(Vec3::Y));
        }
        if kind == Kind::Cut {
            if let Some(&(a, b)) = self.authoring.edges.first() {
                direction = model.transform_vector3(
                    Vec3::from_array(original.vertices[b].position)
                        - Vec3::from_array(original.vertices[a].position),
                );
            }
        }
        let values = match kind {
            Kind::Vertex | Kind::VertexInset => [15., 0., 0.],
            Kind::Edge => [scale * 0.025, 3., 100.],
            Kind::Face => [self.authoring.inset * 100., self.authoring.bevel_height, 0.],
            Kind::Inset => [self.authoring.inset * 100., 0., 0.],
            Kind::Extrude | Kind::Individual | Kind::VertexExtrude => [scale * 0.05, 0., 0.],
            Kind::Cut => [self.authoring.loop_position * 100., 0., 0.],
            Kind::Slide => [0., 0., 0.],
        };
        let op = LiveOperation {
            node,
            original,
            selection: SelectionSnapshot::capture(&self.authoring),
            faces,
            vertices,
            kind,
            values,
            defaults: values,
            anchor,
            direction,
            scale,
            error: None,
            expanded: self.modeling_area()[2] >= 420. && self.modeling_area()[3] >= 420.,
        };
        self.editor.begin_edit(op.label());
        let result = self
            .editor
            .preview_mesh(node, &op.original, op.edit(values));
        if let Err(e) = result {
            self.editor.cancel_edit();
            self.status = e.to_string();
            return true;
        }
        if matches!(kind, Kind::Vertex | Kind::VertexInset | Kind::Edge | Kind::Cut) {
            self.authoring.vertices.clear();
            self.authoring.edges.clear();
            self.authoring.faces.clear();
        }
        self.modeling_live = Some(op);
        self.sync_live_selection();
        self.invalidate_geometry();
        self.status =
            "Предпросмотр · Enter — применить · Esc — отменить · Shift — точно · Ctrl — шаг".into();
        true
    }
    fn preview_live_values(&mut self, values: [f32; 3]) -> Result<(), String> {
        let op = self.modeling_live.as_ref().ok_or("Предпросмотр закрыт")?;
        if values == op.values {
            self.modeling_live.as_mut().unwrap().error = None;
            return Ok(());
        }
        let result = self
            .editor
            .preview_mesh(op.node, &op.original, op.edit(values));
        let op = self.modeling_live.as_mut().unwrap();
        match result {
            Ok(()) => {
                op.values = values;
                op.error = None;
                self.sync_live_selection();
                self.invalidate_geometry();
                Ok(())
            }
            Err(e) => {
                let text = e.to_string();
                op.error = Some(text.clone());
                self.status = text.clone();
                Err(text)
            }
        }
    }
    fn sync_live_selection(&mut self) {
        let Some(op)=&self.modeling_live else {return;};
        if self.editor.editable_mesh(op.node).is_ok_and(|mesh| *mesh == op.original) {
            op.selection.restore(&mut self.authoring);
            return;
        }
        if op.kind==Kind::VertexExtrude {
            self.authoring.selection_mode=S::Vertex;
            self.authoring.vertices=(op.original.vertices.len()..self.editor.editable_mesh(op.node).map_or(0,|m|m.vertices.len())).collect();
            self.authoring.edges.clear();self.authoring.faces.clear();
        } else if matches!(op.kind,Kind::Face|Kind::Inset|Kind::Extrude|Kind::Individual) {
            self.authoring.selection_mode=S::Face;
            self.authoring.faces=op.faces.clone();self.authoring.vertices.clear();self.authoring.edges.clear();
        }
    }
    pub(super) fn modeling_live_confirm(&mut self, a: &Action, text: &str) -> Result<bool, String> {
        let Action::Author(A::LiveField(i)) = a else {
            return Ok(false);
        };
        let op = self.modeling_live.as_ref().ok_or("Предпросмотр закрыт")?;
        let i = *i as usize;
        if i >= op.fields() {
            return Err("Неизвестный параметр".into());
        }
        let (_, min, max, _) = op.spec(i);
        let v = surfaces::number(text, min, max)?;
        if op.kind == Kind::Edge && i == 1 && v.fract() != 0. {
            return Err("Число сегментов должно быть целым".into());
        }
        let mut values = op.values;
        values[i] = v;
        self.preview_live_values(values)?;
        Ok(true)
    }
    pub(super) fn accept_modeling_live(&mut self) {
        if self.modeling_live.is_none() {
            return;
        }
        if self.input.is_some() && !self.resolve_input() {
            return;
        }
        self.release_live_capture();
        let op = self.modeling_live.take().unwrap();
        match self.editor.commit_edit() {
            Ok(()) => self.status = format!("{} · применено", op.label()),
            Err(e) => {
                op.selection.restore(&mut self.authoring);
                self.status = e.to_string();
            }
        }
        self.authoring.diagnostics = None;
        self.invalidate_geometry();
        self.update_dirty();
        self.redraw();
    }
    pub(super) fn cancel_modeling_live(&mut self) {
        self.cancel_knife();
        if let Some(op) = self.modeling_live.take() {
            self.release_live_capture();
            if self
                .input
                .as_ref()
                .is_some_and(|i| matches!(i.purpose, Action::Author(A::LiveField(_))))
            {
                self.end_input();
            }
            self.editor.cancel_edit();
            op.selection.restore(&mut self.authoring);
            self.invalidate_geometry();
            self.update_dirty();
            self.status = "Предпросмотр отменён".into();
            self.redraw();
        }
    }
    pub(super) fn route_live_action(&mut self, a: &Action) -> bool {
        if self.modeling_live.is_none() {
            return false;
        }
        match a {
            Action::ToggleSection(id) if matches!(*id, super::modeling_palette::TOOLS_SECTION | super::modeling_palette::PROPERTIES_SECTION) => false,
            Action::ScrollPanel(0) | Action::ToggleRightPanel => false,
            Action::Author(
                v @ (A::LiveField(_)
                | A::LiveDrag(_)
                | A::LiveAccept
                | A::LiveCancel
                | A::LiveToggle
                | A::LiveBackdrop),
            ) => {
                self.modeling_live_action(*v);
                self.redraw();
                true
            }
            Action::Slide(field, _, _) if matches!(**field, Action::Author(A::LiveField(_))) => {
                let Action::Author(A::LiveField(i)) = **field else {
                    unreachable!()
                };
                let rect = self
                    .ui
                    .hits
                    .iter()
                    .rev()
                    .find(|h| h.action == *a)
                    .map(|h| h.rect);
                self.begin_live_parameter(i as usize, rect);
                true
            }
            Action::Cancel | Action::Undo => {
                self.cancel_modeling_live();
                true
            }
            _ => {
                self.cancel_modeling_live();
                false
            }
        }
    }
    fn begin_live_parameter(&mut self, i: usize, track: Option<[f32; 4]>) {
        let Some(op) = &self.modeling_live else {
            return;
        };
        if i >= op.fields() {
            return;
        }
        let (mut direction, mut step) = self.live_parameter_basis(op, i);
        if track.is_none() && i == 0 && matches!(op.kind, Kind::Vertex | Kind::VertexInset | Kind::Edge | Kind::Face | Kind::Inset) {
            if let Some(center) = self.project_point(
                op.anchor
                    + if op.kind == Kind::Face {
                        op.direction * op.values[1]
                    } else {
                        Vec3::ZERO
                    },
            ) {
                let radial = glam::Vec2::from_array(self.mouse) - glam::Vec2::from_array(center);
                direction = radial.try_normalize().unwrap_or(glam::Vec2::X).to_array();
            }
        }
        let range = self.live_field_range(op, i);
        if let Some(r) = track {
            let (min, max) = (range.min as f32, range.max as f32);
            step = (max - min) / r[2].max(1.);
            direction = [1., 0.];
        }
        self.drag = Some(Drag::LiveParameter(ParameterDrag {
            start: op.values,
            pointer: self.mouse,
            value: op.values[i],
            field: i,
            fine: self.modifiers.shift_key(),
            snapped: self.modifiers.control_key(),
            snap_origin: op.values[i],
            step,
            direction,
            requested: op.values,
            viewport: track.is_none(),
            range,
        }));
        self.drag_button = Some(MouseButton::Left);
        self.redraw();
    }
    pub(super) fn update_live_parameter(&mut self, mut d: ParameterDrag, p: [f32; 2]) {
        if p.iter().any(|v| !v.is_finite()) {
            return;
        }
        let Some(op) = &self.modeling_live else {
            return;
        };
        let fine = self.modifiers.shift_key();
        let snapped = self.modifiers.control_key();
        if fine != d.fine || snapped != d.snapped {
            d.fine = fine;
            d.snapped = snapped;
            d.pointer = p;
            d.value = op.values[d.field];
            d.snap_origin = d.value;
        }
        let motion =
            (p[0] - d.pointer[0]) * d.direction[0] + (p[1] - d.pointer[1]) * d.direction[1];
        d.value += motion * d.step * if fine { 0.1 } else { 1. };
        d.pointer = p;
        let (_, min, max, _) = op.spec(d.field);
        // Keep reversing at a hard limit responsive after a long captured drag.
        d.value = d.value.clamp(min, max);
        let mut values = op.values;
        let mut value = d.value;
        if op.kind == Kind::Edge && d.field == 1 {
            value = value.round();
        } else if snapped {
            let step = if matches!(op.kind, Kind::Vertex | Kind::VertexInset | Kind::Cut | Kind::Inset | Kind::Slide)
                || (op.kind == Kind::Face && d.field == 0)
                || (op.kind == Kind::Edge && d.field == 2)
            {
                5.
            } else {
                op.scale * 0.01
            };
            value = d.snap_origin + ((value - d.snap_origin) / step).round() * step;
        }
        values[d.field] = value.clamp(min, max);
        // Vertical motion, sub-step segment changes, and repeated invalid
        // values must not clone/rebuild an unchanged mesh on every event.
        if values != d.requested {
            self.slider_click = None;
            d.requested = values;
            let _ = self.preview_live_values(values);
        }
        self.drag = Some(Drag::LiveParameter(d));
        self.redraw();
    }
    pub(super) fn restore_live_parameter(&mut self, d: &ParameterDrag) {
        let _ = self.preview_live_values(d.start);
    }
    fn release_live_capture(&mut self) {
        self.drag = None;
        self.drag_button = None;
        if let Some(window) = &self.window {
            let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
        }
    }
    pub(super) fn modeling_live_owns_scroll(&self) -> bool {
        if self.modeling_live_in_properties() && ui::contains(self.properties_content_rect(), self.mouse) {
            return matches!(self.drag, Some(Drag::LiveParameter(_)));
        }
        self.modeling_live.is_some()
            && (matches!(self.drag, Some(Drag::LiveParameter(_)))
                || matches!(
                    self.ui.hit(self.mouse),
                    Some(Action::Author(
                        A::LiveDrag(_)
                            | A::LiveField(_)
                            | A::LiveBackdrop
                            | A::LiveAccept
                            | A::LiveCancel
                            | A::LiveToggle
                    ))
                )
                || matches!(self.ui.hit(self.mouse), Some(Action::Slide(a, _, _))
                if matches!(*a, Action::Author(A::LiveField(_)))))
    }
    fn live_parameter_basis(&self, op: &LiveOperation, i: usize) -> ([f32; 2], f32) {
        let fallback = ([1., 0.], op.spec(i).3);
        if op.kind == Kind::Cut {
            if let (Some(a), Some(b)) = (
                self.project_point(op.anchor - op.direction * 0.5),
                self.project_point(op.anchor + op.direction * 0.5),
            ) {
                let delta = glam::Vec2::from_array(b) - glam::Vec2::from_array(a);
                let length = delta.length();
                if length.is_finite() && length >= 8. {
                    return ((delta / length).to_array(), 100. / length);
                }
            }
            return fallback;
        }
        if !matches!((op.kind, i), (Kind::Extrude | Kind::Individual | Kind::VertexExtrude, 0) | (Kind::Face, 1)) {
            return fallback;
        }
        let sample = op.scale * 0.05;
        let (Some(a), Some(b)) = (
            self.project_point(op.anchor - op.direction * sample),
            self.project_point(op.anchor + op.direction * sample),
        ) else {
            return fallback;
        };
        let delta = glam::Vec2::from_array(b) - glam::Vec2::from_array(a);
        let pixels = delta.length();
        // End-on/foreshortened axes use the same visible horizontal fallback
        // for painting, cursor feedback and input, avoiding huge deltas.
        if !pixels.is_finite() || pixels < 8. {
            return fallback;
        }
        ((delta / pixels).to_array(), sample * 2. / pixels)
    }
    pub(super) fn live_parameter_cursor(&self) -> Option<winit::window::CursorIcon> {
        use winit::window::CursorIcon as C;
        let direction = if let Some(Drag::LiveParameter(d)) = &self.drag {
            d.direction
        } else if self.drag.is_none() && !self.has_modal() {
            match self.ui.hit(self.mouse)? {
                Action::Author(A::LiveDrag(i)) => {
                    let op = self.modeling_live.as_ref()?;
                    if i == 0 && matches!(op.kind, Kind::Vertex | Kind::VertexInset | Kind::Edge | Kind::Face | Kind::Inset) {
                        let center = self.project_point(
                            op.anchor
                                + if op.kind == Kind::Face {
                                    op.direction * op.values[1]
                                } else {
                                    Vec3::ZERO
                                },
                        )?;
                        (glam::Vec2::from_array(self.mouse) - glam::Vec2::from_array(center))
                            .try_normalize()
                            .unwrap_or(glam::Vec2::X)
                            .to_array()
                    } else {
                        self.live_parameter_basis(op, i as usize).0
                    }
                }
                _ => return None,
            }
        } else {
            return None;
        };
        Some(if direction[1].abs() < 0.38 {
            C::EwResize
        } else if direction[0].abs() < 0.38 {
            C::NsResize
        } else if direction[0] * direction[1] > 0. {
            C::NwseResize
        } else {
            C::NeswResize
        })
    }
    fn modeling_live_panel_rect(&self, op: &LiveOperation) -> [f32; 4] {
        let v = self.modeling_area();
        let w = 300_f32.min((v[2] - 24.).max(1.));
        let h = if op.expanded {
            112. + op.fields() as f32 * 38.
        } else {
            38.
        };
        [v[0] + 12., (v[1] + v[3] - h - 12.).max(v[1] + 12.), w, h]
    }
    pub(super) fn draw_modeling_live(&self, u: &mut Ui) {
        let Some(op) = &self.modeling_live else {
            return;
        };
        let p = u.palette();
        let [x, y, w, h] = self.modeling_live_panel_rect(op);
        self.draw_live_handles(u, op);
        if self.modeling_live_in_properties() { return; }
        let mut bg = p.panel;
        bg[3] = 0.92;
        u.outlined_surface([x, y, w, h], bg, p.border);
        u.rect([x + 1., y + 5., 3., h - 10.], p.accent);
        u.hits.push(ui::Hit {
            rect: [x, y, w, h],
            action: Action::Author(A::LiveBackdrop),
        });
        u.hits.push(ui::Hit {
            rect: [x, y, w - 72., 38.],
            action: Action::Author(A::LiveToggle),
        });
        u.disclosure_button(
            [x + 7., y + 3., 28., 32.],
            Some(Action::Author(A::LiveToggle)),
            op.expanded,
        );
        u.fitted_label([x + 38., y + 10., w - 114., 22.], op.label(), 13., p.text);
        if !op.expanded {
            u.quiet_icon_button(
                [x + w - 70., y + 3., 32., 32.],
                "",
                Action::Author(A::LiveAccept),
                false,
                crate::icons::Icon::Check,
            );
            u.quiet_icon_button(
                [x + w - 35., y + 3., 32., 32.],
                "",
                Action::Author(A::LiveCancel),
                false,
                crate::icons::Icon::Close,
            );
            return;
        }
        for i in 0..op.fields() {
            let (label, _, _, _) = op.spec(i);
            u.number_field(
                [x + 14., y + 40. + i as f32 * 38., w - 28., 32.],
                label,
                op.values[i] as f64,
                Action::Author(A::LiveField(i as u8)),
                self.live_field_range(op, i),
            );
        }
        let row = y + 40. + op.fields() as f32 * 38.;
        let half = (w - 34.) * 0.5;
        u.button(
            [x + 14., row, half, 28.],
            "Применить ↵",
            Action::Author(A::LiveAccept),
            true,
        );
        u.quiet_button(
            [x + 20. + half, row, half, 28.],
            "Отмена · Esc",
            Action::Author(A::LiveCancel),
            false,
        );
        u.fitted_label(
            [x + 14., row + 34., w - 28., 18.],
            op.error.as_deref().unwrap_or(if op.kind == Kind::Edge {
                "Колесо — параметр / сегменты кольца"
            } else {
                "Shift — точно · Ctrl — шаг"
            }),
            10.,
            p.muted,
        );
    }

    fn modeling_live_in_properties(&self) -> bool {
        self.preferences.right_panel && self.surface.tab == 7 && self.authoring.editing.is_some()
    }
    fn live_field_range(&self, op: &LiveOperation, field: usize) -> SliderRange {
        if let Some(Drag::LiveParameter(drag)) = &self.drag {
            if drag.field == field { return drag.range; }
        }
        let (min, max) = op.slider_bounds(field);
        let step = if op.kind == Kind::Edge && field == 1 { 1. } else { op.spec(field).3 };
        SliderRange::new(min as f64, max as f64, step as f64)
    }
    pub(super) fn live_slider_ui_range(&self) -> Option<(Action, SliderRange)> {
        match &self.drag {
            Some(Drag::LiveParameter(drag)) => Some((Action::Author(A::LiveField(drag.field as u8)), drag.range)),
            _ => None,
        }
    }
    pub(super) fn modeling_live_properties_height(&self) -> f32 {
        self.modeling_live.as_ref().map_or(0., |op| op.fields() as f32 * 50. + 78.)
    }
    pub(super) fn draw_modeling_live_properties(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let Some(op) = &self.modeling_live else { return; };
        for i in 0..op.fields() {
            let (label, _, _, _) = op.spec(i);
            u.number_field([x, y, w, 44.], label, op.values[i] as f64,
                Action::Author(A::LiveField(i as u8)),
                self.live_field_range(op, i));
            y += 50.;
        }
        let half = (w - 6.) * 0.5;
        u.button([x, y, half, 30.], "Применить ↵", Action::Author(A::LiveAccept), true);
        u.quiet_button([x + half + 6., y, half, 30.], "Отмена · Esc", Action::Author(A::LiveCancel), false);
        u.fitted_label([x, y + 38., w, 22.], op.error.as_deref().unwrap_or("Shift — точно · Ctrl — шаг"), 10., u.palette().muted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Node, PrimitiveKind, PrimitiveSettings, Project};

    fn app(command: A) -> App {
        let mut node = Node::new(1, "Model");
        node.mesh = Some(2);
        let mut a = App::new(None);
        a.editor = Editor::new(Project {
            nodes: vec![node],
            meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
                .mesh(2, None)
                .unwrap()],
            ..Default::default()
        });
        a.editor.selected = Some(1);
        a.editor.mode = EditorMode::Scene;
        a.preferences = Preferences::default();
        a.width = 1440.;
        a.height = 900.;
        a.target = Vec3::ZERO;
        a.frame_all();
        a.action(Action::Author(A::SelectMode(
            if matches!(command, A::Extrude | A::Bevel | A::Inset) {
                S::Face
            } else {
                S::Edge
            },
        )));
        a.authoring.faces = BTreeSet::from([0]);
        a.authoring.edges = BTreeSet::from([(0, 1)]);
        a.action(Action::Author(command));
        assert!(a.modeling_live.is_some(), "{}", a.status);
        a.ui = a.build_ui();
        a
    }
    fn press(a: &mut App, action: A) -> [f32; 4] {
        for offset in 0..40 {
            a.inspector_scroll = (offset as f32 * 40.).min(a.inspector_max_scroll());
            a.ui = a.build_ui();
            if a.ui.action_point(&Action::Author(action)).is_some() { break; }
        }
        a.ui = a.build_ui();
        let rect =
            a.ui.hits
                .iter()
                .find(|h| h.action == Action::Author(action))
                .unwrap()
                .rect;
        a.mouse_move(
            a.ui.action_point(&Action::Author(action))
                .expect("reachable control"),
        );
        if matches!(action, A::LiveField(_)) {
            a.click_ui_control();
        } else {
            a.mouse_down(MouseButton::Left);
        }
        rect
    }

    #[test]
    fn panel_values_validate_and_tab_with_one_undo() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            let mut a = app(A::EdgeBevel);
            a.preferences.theme = theme;
            let history = a.editor.undo_len();
            let preview = a.editor.project.clone();
            let r = press(&mut a, A::LiveField(1));
            a.mouse_up(MouseButton::Left);
            assert!(a.drag.is_none());
            assert_eq!(a.active_field_rect(), Some(r));
            a.input.as_mut().unwrap().select_all();
            a.input.as_mut().unwrap().insert("3.5");
            a.action(Action::ConfirmInput);
            assert!(a.input.is_some());
            assert_eq!(a.editor.project, preview);
            a.input.as_mut().unwrap().select_all();
            a.input.as_mut().unwrap().insert("5");
            a.action(Action::ConfirmInput);
            assert!(a.input.is_none());
            assert_eq!(a.modeling_live.as_ref().unwrap().values[1], 5.);
            assert_eq!(a.editor.undo_len(), history);
            press(&mut a, A::LiveField(0));
            a.mouse_up(MouseButton::Left);
            a.focus_next_field(false);
            assert_eq!(
                a.input.as_ref().unwrap().purpose,
                Action::Author(A::LiveField(1))
            );
            a.input.as_mut().unwrap().select_all();
            a.input.as_mut().unwrap().insert("8");
            a.action(Action::Cancel);
            assert_eq!(a.modeling_live.as_ref().unwrap().values[1], 5.);
            a.action(Action::Author(A::LiveAccept));
            assert_eq!(a.editor.undo_len(), history + 1);
            let committed = a.editor.project.clone();
            a.action(Action::Undo);
            a.action(Action::Redo);
            assert_eq!(a.editor.project, committed);
        }
    }

    #[test]
    fn unchanged_motion_reuses_geometry_and_modifier_toggles_do_not_jump() {
        let mut a = app(A::EdgeBevel);
        let initial = a.editor.project.meshes[0].vertices.clone();
        a.begin_live_parameter(1, Some([0., 0., 310., 10.]));
        let start = a.mouse;
        a.mouse_move([start[0] + 3., start[1] + 80.]);
        assert!(a.editor.project.meshes[0].vertices.shares_storage(&initial));
        a.mouse_move([start[0] + 6., start[1] + 80.]);
        assert_eq!(a.modeling_live.as_ref().unwrap().values[1], 4.);
        let updated = a.editor.project.meshes[0].vertices.clone();
        a.mouse_move([a.mouse[0], a.mouse[1] - 50.]);
        assert!(a.editor.project.meshes[0].vertices.shares_storage(&updated));
        a.mouse_up(MouseButton::Left);
        press(&mut a, A::LiveDrag(0));
        a.mouse_move([a.mouse[0] + 7., a.mouse[1]]);
        let value = a.modeling_live.as_ref().unwrap().values[0];
        let updated = a.editor.project.meshes[0].vertices.clone();
        for modifiers in [
            ModifiersState::CONTROL,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
            ModifiersState::empty(),
        ] {
            a.modifiers = modifiers;
            a.mouse_move(a.mouse);
            assert_eq!(a.modeling_live.as_ref().unwrap().values[0], value);
            assert!(a.editor.project.meshes[0].vertices.shares_storage(&updated));
        }
        a.cancel_drag();
        assert!(a.drag.is_none());
    }

    #[test]
    fn extrusion_drag_tracks_transformed_geometry_at_both_dpi_and_projections() {
        for dpi in [1., 2.] {
            for orthographic in [false, true] {
                for mirror in [1., -1.] {
                    let mut a = app(A::Extrude);
                    a.cancel_modeling_live();
                    a.scale = dpi;
                    a.orthographic = orthographic;
                    let parent = Node {
                        transform: poseforge_core::Transform {
                            rotation: Quat::from_rotation_z(0.4),
                            scale: Vec3::new(1.4 * mirror, 0.8, 1.2),
                            ..Default::default()
                        },
                        ..Node::new(3, "Parent")
                    };
                    a.editor.project.nodes.push(parent);
                    a.editor.project.node_mut(1).unwrap().parent = Some(3);
                    a.editor.project.node_mut(1).unwrap().transform.rotation =
                        Quat::from_rotation_y(0.3);
                    a.frame_all();
                    a.action(Action::Author(A::Extrude));
                    let op = a.modeling_live.as_ref().unwrap();
                    let face = op.original.face_vertices(0).unwrap();
                    let p = |i: usize| {
                        Vec3::from_array(op.original.vertices[face[i] as usize].position)
                    };
                    let normal = (p(1) - p(0)).cross(p(2) - p(0)).normalize();
                    let model = a.editor.project.global_transforms().unwrap()[&1];
                    assert!((model.transform_vector3(normal) - op.direction).length() < 1e-5);
                    let (axis, step) = a.live_parameter_basis(op, 0);
                    let old = op.values[0];
                    let frozen_mesh = op.original.clone();
                    let selection = op.selection.faces.clone();
                    press(&mut a, A::LiveDrag(0));

                    a.mouse_move([a.mouse[0] + axis[0] * 20., a.mouse[1] + axis[1] * 20.]);
                    let actual = a.modeling_live.as_ref().unwrap().values[0];
                    assert!((actual - old - step * 20.).abs() < 1e-5);
                    let mut expected = Editor::new(a.editor.project.clone());
                    expected.begin_edit("Expected extrusion");
                    expected
                        .preview_mesh(
                            1,
                            &frozen_mesh,
                            MeshEdit::Extrude {
                                faces: selection,
                                distance: actual,
                            },
                        )
                        .unwrap();
                    assert_eq!(expected.project.meshes, a.editor.project.meshes);
                    a.cancel_drag();
                    assert_eq!(a.modeling_live.as_ref().unwrap().values[0], old);
                }
            }
        }
    }

    #[test]
    fn end_on_axis_has_a_finite_drag_and_offscreen_selection_keeps_numeric_input() {
        let mut a = app(A::Extrude);
        let view = a.view();
        let forward = (view.target - view.eye).normalize();
        let op = a.modeling_live.as_mut().unwrap();
        op.anchor = view.target;
        op.direction = forward;
        let op = a.modeling_live.as_ref().unwrap();
        assert_eq!(a.live_parameter_basis(op, 0), ([1., 0.], op.spec(0).3));
        let r = press(&mut a, A::LiveDrag(0));
        assert!(r.iter().all(|v| v.is_finite()));
        let distance = a.distance;
        a.scroll(2.);
        assert_eq!(a.distance, distance);
        a.cancel_drag();
        a.modeling_live.as_mut().unwrap().anchor = view.eye - forward;
        a.ui = a.build_ui();
        assert!(a.ui.action_point(&Action::Author(A::LiveDrag(0))).is_none());
        press(&mut a, A::LiveField(0));
        a.mouse_up(MouseButton::Left);
        assert!(a.input.is_some());
    }

    #[test]
    fn ring_picks_only_its_arc_and_wheel_segments_share_the_drag_transaction() {
        let mut a = app(A::EdgeBevel);
        let center = a
            .project_point(a.modeling_live.as_ref().unwrap().anchor)
            .unwrap();
        assert_ne!(a.ui.hit(center), Some(Action::Author(A::LiveDrag(0))));
        let before = a.editor.project.clone();
        let history = a.editor.undo_len();
        press(&mut a, A::LiveDrag(0));
        let distance = a.distance;
        a.scroll(1.);
        assert_eq!(a.modeling_live.as_ref().unwrap().values[1], 4.);
        assert_eq!(a.distance, distance);
        assert_eq!(a.editor.undo_len(), history);
        a.cancel_drag();
        assert_eq!(a.editor.project, before);
        press(&mut a, A::LiveDrag(0));
        a.scroll(-2.);
        a.mouse_up(MouseButton::Left);
        a.action(Action::Author(A::LiveAccept));
        assert_eq!(a.editor.undo_len(), history + 1);
    }
    #[test]
    fn face_height_and_loop_cut_have_separate_projected_handles() {
        for (command, field) in [(A::Bevel, 1), (A::LoopCut, 0)] {
            let mut a = app(command);
            let op = a.modeling_live.as_ref().unwrap();
            let old = op.values;
            let (axis, step) = a.live_parameter_basis(op, field);
            press(&mut a, A::LiveDrag(field as u8));
            assert!(matches!(a.drag, Some(Drag::LiveParameter(_))));
            a.mouse_move([a.mouse[0] + axis[0] * 10., a.mouse[1] + axis[1] * 10.]);
            let values = a.modeling_live.as_ref().unwrap().values;
            assert!((values[field] - old[field] - step * 10.).abs() < 1e-4);
            if field == 1 {
                assert_eq!(values[0], old[0]);
            }
            a.cancel_drag();
            assert_eq!(a.modeling_live.as_ref().unwrap().values, old);
        }
    }

    #[test]
    fn compact_panel_can_expand_edit_and_collapse_without_finishing_preview() {
        let mut a = app(A::EdgeBevel);
        a.cancel_modeling_live();
        a.width = 900.;
        a.height = 600.;
        // A hidden inspector retains the floating fallback panel.
        a.preferences.right_panel = false;
        a.action(Action::Author(A::EdgeBevel));
        if a.modeling_live.as_ref().unwrap().expanded {
            press(&mut a, A::LiveToggle); a.mouse_up(MouseButton::Left);
        }
        assert!(!a.modeling_live.as_ref().unwrap().expanded);
        let history = a.editor.undo_len();
        press(&mut a, A::LiveToggle);
        a.mouse_up(MouseButton::Left);
        assert!(a.modeling_live.as_ref().unwrap().expanded);
        press(&mut a, A::LiveField(1));
        a.mouse_up(MouseButton::Left);
        a.input.as_mut().unwrap().select_all();
        a.input.as_mut().unwrap().insert("6");
        a.action(Action::ConfirmInput);
        press(&mut a, A::LiveToggle);
        a.mouse_up(MouseButton::Left);
        assert!(!a.modeling_live.as_ref().unwrap().expanded);
        assert_eq!(a.modeling_live.as_ref().unwrap().values[1], 6.);
        assert_eq!(a.editor.undo_len(), history);
        press(&mut a, A::LiveAccept);
        a.mouse_up(MouseButton::Left);
        assert!(a.modeling_live.is_none());
        assert_eq!(a.editor.undo_len(), history + 1);
    }

    #[test]
    fn exact_value_expands_the_range_and_drag_keeps_it_frozen_until_release() {
        let mut a = app(A::Extrude);
        a.preferences.right_panel = false;
        let scale = a.modeling_live.as_ref().unwrap().scale;
        a.modeling_live_confirm(&Action::Author(A::LiveField(0)), &(scale*3.).to_string()).unwrap();
        let op = a.modeling_live.as_ref().unwrap();
        let (min,max) = op.slider_bounds(0);
        assert!(min < -scale*3. && max > scale*3.);
        a.begin_live_parameter(0,Some([0.,0.,300.,30.]));
        let frozen = a.live_slider_ui_range().unwrap().1;
        a.mouse_move([a.mouse[0]+90.,a.mouse[1]]);
        assert_eq!(a.live_slider_ui_range().unwrap().1,frozen);
        a.ui = a.build_ui();
        assert!(a.ui.hits.iter().any(|h| matches!(&h.action,Action::Slide(field,range,_)
            if **field == Action::Author(A::LiveField(0)) && *range == frozen)));
        a.mouse_up(MouseButton::Left);
        let op = a.modeling_live.as_ref().unwrap();
        assert!(op.slider_bounds(0).1 > frozen.max as f32);
    }
    #[test]
    fn slider_reverses_immediately_at_the_hard_limit() {
        let mut a = app(A::Extrude);
        a.modeling_live_confirm(&Action::Author(A::LiveField(0)),"999").unwrap();
        a.begin_live_parameter(0,Some([0.,0.,200.,30.]));
        a.mouse_move([a.mouse[0]+200.,a.mouse[1]]);
        assert_eq!(a.modeling_live.as_ref().unwrap().values[0],1000.);
        a.mouse_move([a.mouse[0]-1.,a.mouse[1]]);
        assert!(a.modeling_live.as_ref().unwrap().values[0] < 1000.);
    }
    #[test]
    fn viewport_field_wheel_fine_adjustment_and_reset_keep_one_operation() {
        let mut a = app(A::Extrude);
        a.preferences.right_panel = false;
        a.modeling_live.as_mut().unwrap().expanded = true;
        let before = a.modeling_live.as_ref().unwrap().values;
        let history = a.editor.undo_len();
        a.ui = a.build_ui();
        let h = a.ui.hits.iter().find(|h| matches!(&h.action,Action::Slide(field,_,_)
            if **field == Action::Author(A::LiveField(0)))).unwrap().rect;
        a.mouse = [h[0]+h[2]*0.5,h[1]+h[3]*0.5];
        let camera_distance = a.distance;
        a.modifiers = ModifiersState::SHIFT;
        let step = a.modeling_live.as_ref().unwrap().spec(0).3 * 0.1;
        a.scroll(1.);
        assert!((a.modeling_live.as_ref().unwrap().values[0]-before[0]-step).abs() < 1e-5);
        assert_eq!(a.distance,camera_distance);
        a.action(Action::ResetNumber(Box::new(Action::Author(A::LiveField(0)))));
        assert_eq!(a.modeling_live.as_ref().unwrap().values,before);
        assert_eq!(a.editor.undo_len(),history);
        a.modifiers = ModifiersState::empty();
        a.scroll(2.);
        a.action(Action::Author(A::LiveAccept));
        assert_eq!(a.editor.undo_len(),history+1);
    }
    #[test]
    fn on_model_readout_and_accent_are_drawn_in_both_themes_and_dpi() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {
            for scale in [1.,2.] {
                let mut a = app(A::EdgeBevel); a.preferences.theme = theme; a.scale = scale;
                a.ui = a.build_ui();
                press(&mut a,A::LiveDrag(0));
                let mut u = Ui::with_theme(scale,theme,a.mouse); u.accent = a.preferences.accent;
                a.draw_live_handles(&mut u,a.modeling_live.as_ref().unwrap());
                assert!(u.texts.iter().any(|t| t.text.starts_with("Ширина ·")));
                let accent = u.palette().accent;
                assert!(u.rects.iter().any(|r| r.color == accent));
                let viewport = a.modeling_area().map(|v| v*scale);
                assert!(u.rects.iter().all(|r| r.rect[0] >= viewport[0]-0.01 && r.rect[1] >= viewport[1]-0.01
                    && r.rect[0]+r.rect[2] <= viewport[0]+viewport[2]+0.01
                    && r.rect[1]+r.rect[3] <= viewport[1]+viewport[3]+0.01));
            }
        }
    }
}
