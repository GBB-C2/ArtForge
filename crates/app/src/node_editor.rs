//! Material DAG authoring. Click or drag an output to an input; edits use the document history.
use super::{clip_scrolled, App, Drag};
use crate::ui::{self, Action, Hit, NodeAction, Ui};
use poseforge_core::{MaterialGraph, MaterialNode, MaterialNodeKind, MAX_MATERIAL_NODES};
use winit::{
    event::MouseButton,
    keyboard::{Key, NamedKey},
};

#[derive(Clone)]
pub(super) struct NodeDrag {
    material: u64,
    id: u32,
    start: [f32; 2],
    position: [f32; 2],
}
#[derive(Clone)]
pub(super) struct NodePan {
    start: [f32; 2],
    pan: [f32; 2],
}
pub(super) struct NodeEditor {
    pub material: Option<u64>,
    pub selected: u32,
    pub source: Option<u32>,
    pub add_menu: bool,
    candidate: usize,
    pan: [f32; 2],
    zoom: f32,
    connection_press: bool,
    inspector_scroll: f32,
}
impl Default for NodeEditor {
    fn default() -> Self {
        Self {
            material: None,
            selected: 0,
            source: None,
            add_menu: false,
            candidate: 0,
            pan: [20., 24.],
            zoom: 0.85,
            connection_press: false,
            inspector_scroll: 0.,
        }
    }
}

fn position(graph: &MaterialGraph, id: u32) -> Option<[f32; 2]> {
    if id == 0 {
        Some(graph.output_position)
    } else {
        graph.nodes.iter().find(|n| n.id == id).map(|n| n.position)
    }
}
fn position_mut(graph: &mut MaterialGraph, id: u32) -> Option<&mut [f32; 2]> {
    if id == 0 {
        Some(&mut graph.output_position)
    } else {
        graph
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .map(|n| &mut n.position)
    }
}
fn input_label(id: u32, slot: usize) -> &'static str {
    if id == 0 {
        [
            "Цвет RGBA",
            "Металл",
            "Шероховатость",
            "Свечение",
            "Непрозрачность",
            "Пропускание",
            "IOR",
            "Нормаль",
        ][slot]
    } else {
        ["A", "B", "Фактор"][slot]
    }
}
fn wire(u: &mut Ui, a: [f32; 2], b: [f32; 2], color: [f32; 4]) {
    let reach = ((b[0] - a[0]).abs() * 0.45).max(35.);
    let c = [a[0] + reach, a[1]];
    let d = [b[0] - reach, b[1]];
    let mut prev = a;
    for i in 1..=24 {
        let t = i as f32 / 24.;
        let q = 1. - t;
        let point = [
            q * q * q * a[0] + 3. * q * q * t * c[0] + 3. * q * t * t * d[0] + t * t * t * b[0],
            q * q * q * a[1] + 3. * q * q * t * c[1] + 3. * q * t * t * d[1] + t * t * t * b[1],
        ];
        u.smooth_segment(prev, point, 1.8, color);
        prev = point;
    }
}
impl App {
    pub(super) fn nodes_open(&self) -> bool {
        self.nodes.material.is_some()
    }
    fn node_graph(&self) -> Option<&MaterialGraph> {
        self.nodes
            .material
            .and_then(|id| self.editor.project.material(id))
            .and_then(|m| m.graph.as_ref())
    }
    fn node_graph_mut(&mut self) -> Option<&mut MaterialGraph> {
        let id = self.nodes.material?;
        self.editor
            .project
            .materials
            .iter_mut()
            .find(|m| m.id == id)?
            .graph
            .as_mut()
    }
    pub(super) fn node_editor_rect(&self) -> [f32; 4] {
        let top = (self.height * 0.48).max(230.);
        [
            12.,
            top,
            self.width - 24.,
            (self.height - top - 36.).max(180.),
        ]
    }
    fn node_canvas(&self) -> [f32; 4] {
        let r = self.node_editor_rect();
        [r[0] + 8., r[1] + 48., (r[2] - 244.).max(200.), r[3] - 76.]
    }
    fn node_screen(&self, p: [f32; 2]) -> [f32; 2] {
        let c = self.node_canvas();
        [
            c[0] + self.nodes.pan[0] + p[0] * self.nodes.zoom,
            c[1] + self.nodes.pan[1] + p[1] * self.nodes.zoom,
        ]
    }
    fn node_socket(
        &self,
        graph: &MaterialGraph,
        id: u32,
        slot: usize,
        output: bool,
    ) -> Option<[f32; 2]> {
        let p = self.node_screen(position(graph, id)?);
        Some([
            p[0] + if output { 168. * self.nodes.zoom } else { 0. },
            p[1] + (48. + slot as f32 * 28.) * self.nodes.zoom,
        ])
    }
    pub(super) fn nodes_prune(&mut self) {
        if self.nodes_open()
            && (self.nodes.material != self.selected_material_id()
                || self.material_locked()
                || self.node_graph().is_none())
        {
            if matches!(
                self.drag,
                Some(Drag::Node(_) | Drag::NodePan(_) | Drag::NodeScroll { .. })
            ) {
                self.cancel_drag();
            }
            if self
                .input
                .as_ref()
                .is_some_and(|i| matches!(i.purpose, Action::Node(_)))
            {
                self.end_input();
            }
            self.nodes = NodeEditor::default();
        }
        if let Some(graph) = self.node_graph() {
            if position(graph, self.nodes.selected).is_none() {
                self.nodes.selected = 0;
            }
        }
        if let Some(source) = self.nodes.source {
            if self
                .node_graph()
                .is_none_or(|g| !g.nodes.iter().any(|n| n.id == source))
            {
                self.nodes.source = None;
            }
        }
    }
    fn edit_graph(&mut self, label: &str, edit: impl FnOnce(&mut MaterialGraph)) {
        let Some(id) = self.nodes.material else {
            return;
        };
        if self.selected_material_id() != Some(id) || self.material_locked() {
            return;
        }
        let result = self.editor.transact(label, |p| {
            if let Some(graph) = p
                .materials
                .iter_mut()
                .find(|m| m.id == id)
                .and_then(|m| m.graph.as_mut())
            {
                edit(graph);
            }
        });
        if self.result(result).is_some() {
            self.status = label.into();
        }
    }
    pub(super) fn node_action(&mut self, action: NodeAction) {
        if action == NodeAction::Open {
            let Some(id) = self.selected_material_id() else {
                return;
            };
            if self.material_locked() {
                return;
            }
            if self
                .editor
                .project
                .material(id)
                .is_some_and(|m| m.graph.is_none())
            {
                let r = self.editor.transact("Создать граф материала", |p| {
                    p.materials.iter_mut().find(|m| m.id == id).unwrap().graph =
                        Some(MaterialGraph::default());
                });
                if self.result(r).is_none() {
                    return;
                }
            }
            self.nodes = NodeEditor {
                material: Some(id),
                ..NodeEditor::default()
            };
            self.editor.playing = false;
            return;
        }
        self.nodes_prune();
        if !self.nodes_open() {
            return;
        }
        match action {
            NodeAction::LoadImage => {
                if self
                    .node_graph()
                    .and_then(|g| g.nodes.iter().find(|n| n.id == self.nodes.selected))
                    .is_some_and(|n| n.kind == MaterialNodeKind::Texture)
                {
                    self.node_texture_target =
                        self.nodes.material.map(|m| (m, self.nodes.selected));
                    self.texture_target = None;
                    self.show_files(super::FilePurpose::Texture);
                }
            }
            NodeAction::NextImage => {
                let current = self
                    .node_graph()
                    .and_then(|g| g.nodes.iter().find(|n| n.id == self.nodes.selected))
                    .and_then(|n| n.texture);
                let ids: Vec<_> = std::iter::once(None)
                    .chain(self.editor.project.textures.iter().map(|t| Some(t.id)))
                    .collect();
                let next =
                    ids[(ids.iter().position(|x| *x == current).unwrap_or(0) + 1) % ids.len()];
                let id = self.nodes.selected;
                self.edit_graph("Изображение узла", |g| {
                    if let Some(n) = g.nodes.iter_mut().find(|n| n.id == id) {
                        n.texture = next;
                    }
                });
            }
            NodeAction::ImageData => {
                let id = self.nodes.selected;
                self.edit_graph(
                    "Цветовое пространство изображения",
                    |g| {
                        if let Some(n) = g.nodes.iter_mut().find(|n| n.id == id) {
                            n.data_texture = !n.data_texture;
                        }
                    },
                );
            }
            NodeAction::Open => {}
            NodeAction::InspectorScroll => {
                let max = self.node_inspector_max_scroll();
                let r = self.node_inspector_scrollbar();
                let thumb = (r[3] * r[3] / (r[3] + max)).max(28.).min(r[3]);
                let top = r[1] + (r[3] - thumb) * self.nodes.inspector_scroll / max.max(1.);
                if self.mouse[1] < top || self.mouse[1] > top + thumb {
                    self.nodes.inspector_scroll =
                        ((self.mouse[1] - r[1] - thumb * 0.5) / (r[3] - thumb).max(1.) * max)
                            .clamp(0., max);
                }
                self.drag = Some(Drag::NodeScroll {
                    start: self.mouse[1],
                    value: self.nodes.inspector_scroll,
                });
                self.drag_button = Some(MouseButton::Left);
            }
            NodeAction::Close => {
                self.nodes = NodeEditor::default();
            }
            NodeAction::AddMenu => {
                self.nodes.add_menu = !self.nodes.add_menu;
                self.nodes.source = None;
            }
            NodeAction::Add(index) => {
                let Some(&kind) = MaterialNodeKind::ALL.get(index) else {
                    return;
                };
                let Some(graph) = self.node_graph() else {
                    return;
                };
                if graph.nodes.len() >= MAX_MATERIAL_NODES {
                    self.status = "В графе может быть не более 16 узлов.".into();
                    return;
                }
                let id = graph
                    .nodes
                    .iter()
                    .map(|n| n.id)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1);
                let i = graph.nodes.len() as f32;
                let p = [40. + (i % 3.) * 210., 40. + (i / 3.).floor() * 150.];
                self.edit_graph("Добавить узел", |g| {
                    g.nodes.push(MaterialNode::new(id, kind, p))
                });
                self.nodes.selected = id;
                self.nodes.add_menu = false;
            }
            NodeAction::Select(id) => {
                if self.node_graph().and_then(|g| position(g, id)).is_some() {
                    self.nodes.selected = id;
                }
            }
            NodeAction::Move(id) => {
                let Some(p) = self.node_graph().and_then(|g| position(g, id)) else {
                    return;
                };
                self.nodes.selected = id;
                self.nodes.add_menu = false;
                self.editor.begin_edit("Переместить узел");
                self.drag = Some(Drag::Node(NodeDrag {
                    material: self.nodes.material.unwrap(),
                    id,
                    start: self.mouse,
                    position: p,
                }));
                self.drag_button = Some(MouseButton::Left);
            }
            NodeAction::Pan => {
                if self.nodes.add_menu {
                    self.nodes.add_menu = false;
                    return;
                }
                if self.nodes.source.take().is_some() {
                    return;
                }
                self.drag = Some(Drag::NodePan(NodePan {
                    start: self.mouse,
                    pan: self.nodes.pan,
                }));
                self.drag_button = Some(MouseButton::Left);
            }
            NodeAction::Output(id) => {
                if id != 0
                    && self
                        .node_graph()
                        .is_some_and(|g| g.nodes.iter().any(|n| n.id == id))
                {
                    self.nodes.source = Some(id);
                    self.nodes.connection_press = true;
                    self.nodes.selected = id;
                    self.status = "Выберите вход узла. Esc — отменить связь.".into();
                }
            }
            NodeAction::Input(id, slot) => {
                let Some(source) = self.nodes.source else {
                    self.nodes.selected = id;
                    return;
                };
                let Some(graph) = self.node_graph() else {
                    return;
                };
                let mut candidate = graph.clone();
                match candidate.connect(id, slot, Some(source)) {
                    Ok(()) => {
                        self.edit_graph("Соединить узлы", |g| *g = candidate);
                        self.nodes.source = None;
                        self.nodes.selected = id;
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
            NodeAction::Disconnect(id, slot) => {
                self.edit_graph("Удалить связь", |g| {
                    let _ = g.connect(id, slot, None);
                });
            }
            NodeAction::Delete => {
                let id = self.nodes.selected;
                if id != 0 {
                    self.edit_graph("Удалить узел", |g| g.remove(id));
                    self.nodes.selected = 0;
                    self.nodes.source = None;
                }
            }
            NodeAction::Toggle => self
                .edit_graph("Включение узлов материала", |g| {
                    g.enabled = !g.enabled
                }),
            NodeAction::ResetView => {
                self.nodes.pan = [20., 24.];
                self.nodes.zoom = 0.85;
            }
            NodeAction::Value(field) => {
                let Some(node) = self
                    .node_graph()
                    .and_then(|g| g.nodes.iter().find(|n| n.id == self.nodes.selected))
                else {
                    return;
                };
                let value = if field == 5 {
                    super::surfaces::color_hex([
                        node.secondary[0],
                        node.secondary[1],
                        node.secondary[2],
                    ])
                } else if field == 4 {
                    super::surfaces::color_hex([node.value[0], node.value[1], node.value[2]])
                } else if let Some(v) = node.value.get(field) {
                    format!("{v:.5}")
                } else {
                    return;
                };
                self.input(Action::Node(action), value);
            }
            NodeAction::Position(axis) => {
                let Some(p) = self
                    .node_graph()
                    .and_then(|g| position(g, self.nodes.selected))
                else {
                    return;
                };
                if let Some(v) = p.get(axis) {
                    self.input(Action::Node(action), format!("{v:.2}"));
                }
            }
        }
    }
    pub(super) fn node_confirm(&mut self, action: &Action, value: &str) -> Result<bool, String> {
        let Action::Node(command) = action else {
            return Ok(false);
        };
        let id = self.nodes.selected;
        let material = self.nodes.material.ok_or("Граф закрыт")?;
        if self.selected_material_id() != Some(material) || self.material_locked() {
            return Err("Материал недоступен".into());
        }
        let mut graph = self.node_graph().ok_or("Граф недоступен")?.clone();
        match *command {
            NodeAction::Position(axis) => {
                let target = position_mut(&mut graph, id)
                    .and_then(|p| p.get_mut(axis))
                    .ok_or("Узел недоступен")?;
                *target = super::surfaces::number(value, -10000., 10000.)?;
            }
            NodeAction::Value(field) => {
                let node = graph
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == id)
                    .ok_or("Узел недоступен")?;
                if field == 5 && node.kind == MaterialNodeKind::ColorRamp {
                    let color = super::surfaces::parse_color(value)?;
                    node.secondary[..3].copy_from_slice(&color);
                } else if field == 4
                    && matches!(
                        node.kind,
                        MaterialNodeKind::Color | MaterialNodeKind::ColorRamp
                    )
                {
                    let color = super::surfaces::parse_color(value)?;
                    node.value[..3].copy_from_slice(&color);
                } else {
                    let range = match node.kind {
                        MaterialNodeKind::Color
                        | MaterialNodeKind::ColorRamp
                        | MaterialNodeKind::Mix => (0., 1.),
                        MaterialNodeKind::Coordinates => (-1000., 1000.),
                        MaterialNodeKind::Bump | MaterialNodeKind::NormalMap => (0., 10.),
                        MaterialNodeKind::Checker | MaterialNodeKind::Noise => (0.01, 1000.),
                        _ => (0., 10000.),
                    };
                    *node.value.get_mut(field).ok_or("Параметр недоступен")? =
                        super::surfaces::number(value, range.0, range.1)?;
                }
            }
            _ => return Ok(false),
        }
        graph.compile().map_err(|e| e.to_string())?;
        let result = self.editor.transact("Параметр узла", |p| {
            p.materials
                .iter_mut()
                .find(|m| m.id == material)
                .unwrap()
                .graph = Some(graph);
        });
        result.map_err(|e| e.to_string())?;
        Ok(true)
    }
    pub(super) fn update_node_drag(&mut self, drag: NodeDrag, p: [f32; 2]) {
        if self.nodes.material != Some(drag.material) {
            return;
        }
        let zoom = self.nodes.zoom;
        let next = [
            (drag.position[0] + (p[0] - drag.start[0]) / zoom).clamp(-10000., 10000.),
            (drag.position[1] + (p[1] - drag.start[1]) / zoom).clamp(-10000., 10000.),
        ];
        if let Some(target) = self.node_graph_mut().and_then(|g| position_mut(g, drag.id)) {
            *target = next;
        }
    }
    pub(super) fn update_node_pan(&mut self, drag: NodePan, p: [f32; 2]) {
        self.nodes.pan = [
            (drag.pan[0] + p[0] - drag.start[0]).clamp(-16000., 16000.),
            (drag.pan[1] + p[1] - drag.start[1]).clamp(-16000., 16000.),
        ];
    }
    fn node_inspector_scrollbar(&self) -> [f32; 4] {
        let r = self.node_editor_rect();
        let c = self.node_canvas();
        [r[0] + r[2] - 15., c[1], 12., c[3]]
    }
    pub(super) fn update_node_scroll(&mut self, start: f32, value: f32, pointer: f32) {
        let max = self.node_inspector_max_scroll();
        let track = self.node_inspector_scrollbar()[3];
        let thumb = (track * track / (track + max)).max(28.).min(track);
        self.nodes.inspector_scroll =
            (value + (pointer - start) * max / (track - thumb).max(1.)).clamp(0., max);
    }
    pub(super) fn cancel_node_pan(&mut self, drag: &Drag) {
        if let Drag::NodePan(pan) = drag {
            self.nodes.pan = pan.pan;
        } else if let Drag::NodeScroll { value, .. } = drag {
            self.nodes.inspector_scroll = *value;
        }
    }
    pub(super) fn node_pointer_up(&mut self, button: MouseButton) {
        if button == MouseButton::Left && self.nodes.connection_press {
            self.nodes.connection_press = false;
            if let Some(Action::Node(NodeAction::Input(id, slot))) = self.ui.hit(self.mouse) {
                self.node_action(NodeAction::Input(id, slot));
            }
        }
    }
    pub(super) fn node_middle(&mut self, button: MouseButton) -> bool {
        if !self.nodes_open()
            || self.dialog.is_some()
            || self.overwrite.is_some()
            || self.input.is_some()
            || button != MouseButton::Middle
        {
            return false;
        }
        if ui::contains(self.node_canvas(), self.mouse) {
            self.node_action(NodeAction::Pan);
            self.drag_button = Some(button);
            return true;
        }
        if ui::contains(self.viewport(), self.mouse) {
            if self.begin_camera_navigation(false, button) {
                return true;
            }
            self.leave_camera_view();
            self.drag = Some(Drag::Orbit {
                start: self.mouse,
                yaw: self.yaw,
                pitch: self.pitch,
            });
            self.drag_button = Some(button);
            return true;
        }
        true
    }
    pub(super) fn node_scroll(&mut self, delta: f32) -> bool {
        if !self.nodes_open() || self.dialog.is_some() || self.overwrite.is_some() {
            return false;
        }
        if !delta.is_finite() {
            return true;
        }
        if self.drag.is_some() {
            return true;
        }
        if self.nodes.add_menu {
            if delta.abs() > 0.01 {
                self.nodes.candidate = (self.nodes.candidate as isize - delta.signum() as isize)
                    .rem_euclid(MaterialNodeKind::ALL.len() as isize)
                    as usize;
            }
        } else if ui::contains(self.viewport(), self.mouse) {
            if !self.camera_scroll(delta) {
                self.leave_camera_view();
                self.distance = (self.distance * (-delta * 0.1).exp()).clamp(0.1, 1000.);
            }
        } else if ui::contains(self.node_canvas(), self.mouse) {
            let canvas = self.node_canvas();
            let old = self.nodes.zoom;
            let next = (old * (delta * 0.12).exp()).clamp(0.5, 1.5);
            let local = [self.mouse[0] - canvas[0], self.mouse[1] - canvas[1]];
            self.nodes.pan = [
                local[0] - (local[0] - self.nodes.pan[0]) * next / old,
                local[1] - (local[1] - self.nodes.pan[1]) * next / old,
            ];
            self.nodes.zoom = next;
        } else if ui::contains(self.node_editor_rect(), self.mouse) {
            self.nodes.inspector_scroll = (self.nodes.inspector_scroll - delta * 32.)
                .clamp(0., self.node_inspector_max_scroll());
        }
        true
    }
    pub(super) fn node_key(&mut self, key: &Key, ctrl: bool, shift: bool) -> bool {
        if !self.nodes_open() || self.dialog.is_some() || self.overwrite.is_some() {
            return false;
        }
        if self.nodes.add_menu {
            match key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.nodes.candidate = (self.nodes.candidate + 1) % MaterialNodeKind::ALL.len()
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.nodes.candidate = (self.nodes.candidate + MaterialNodeKind::ALL.len() - 1)
                        % MaterialNodeKind::ALL.len()
                }
                Key::Named(NamedKey::Enter) => {
                    self.node_action(NodeAction::Add(self.nodes.candidate))
                }
                Key::Named(NamedKey::Escape) => self.nodes.add_menu = false,
                _ => {}
            }
            return true;
        }
        match key {
            Key::Named(NamedKey::Escape) => {
                if self.drag.is_some() {
                    self.cancel_drag();
                } else if self.nodes.source.take().is_none() {
                    self.node_action(NodeAction::Close);
                }
            }
            Key::Named(NamedKey::Delete) => self.node_action(NodeAction::Delete),
            Key::Named(NamedKey::Tab) => {
                if let Some(g) = self.node_graph() {
                    let ids: Vec<_> = std::iter::once(0)
                        .chain(g.nodes.iter().map(|n| n.id))
                        .collect();
                    let current = ids
                        .iter()
                        .position(|n| *n == self.nodes.selected)
                        .unwrap_or(0);
                    self.nodes.selected =
                        ids[(current + if shift { ids.len() - 1 } else { 1 }) % ids.len()];
                }
            }
            Key::Named(
                direction @ (NamedKey::ArrowLeft
                | NamedKey::ArrowRight
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown),
            ) => {
                let id = self.nodes.selected;
                let step = if shift { 50. } else { 10. };
                self.edit_graph("Переместить узел", |g| {
                    if let Some(p) = position_mut(g, id) {
                        match direction {
                            NamedKey::ArrowLeft => p[0] -= step,
                            NamedKey::ArrowRight => p[0] += step,
                            NamedKey::ArrowUp => p[1] -= step,
                            _ => p[1] += step,
                        }
                    }
                });
            }
            Key::Character(ch) => match ch.to_lowercase().as_str() {
                "z" if ctrl => self.action(if shift { Action::Redo } else { Action::Undo }),
                "y" if ctrl => self.action(Action::Redo),
                "s" if ctrl => self.action(Action::Save),

                "n" => self.node_action(NodeAction::AddMenu),
                "o" => self.node_action(NodeAction::Output(self.nodes.selected)),
                "f" => self.node_action(NodeAction::ResetView),
                "1" | "2" | "3" | "4" => {
                    let slot = ch.parse::<usize>().unwrap() - 1;
                    let id = self.nodes.selected;
                    self.node_action(if shift {
                        NodeAction::Disconnect(id, slot)
                    } else {
                        NodeAction::Input(id, slot)
                    });
                }
                _ => {}
            },
            _ => {}
        }
        self.redraw();
        true
    }
    fn node_inspector_max_scroll(&self) -> f32 {
        let id = self.nodes.selected;
        let node = self
            .node_graph()
            .and_then(|g| g.nodes.iter().find(|n| n.id == id));
        let params = node.map_or(0., |n| match n.kind {
            MaterialNodeKind::Color => 162.,
            MaterialNodeKind::ColorRamp => 196.,
            MaterialNodeKind::Coordinates => 128.,
            MaterialNodeKind::Texture => 102.,
            _ => 32.,
        });
        let inputs = node.map_or(8, |n| n.kind.input_count());
        (30. + params + 64. + inputs as f32 * 28. + if id == 0 { 0. } else { 40. }
            - self.node_canvas()[3])
            .max(0.)
    }
    pub(super) fn draw_node_editor(&mut self, u: &mut Ui) {
        self.nodes_prune();
        let Some(graph) = self.node_graph().cloned() else {
            return;
        };
        if !self.nodes_open() {
            return;
        }
        let r = self.node_editor_rect();
        let canvas = self.node_canvas();
        let palette = u.palette();
        let zoom = self.nodes.zoom;
        // Remove covered text and background interaction; keep the top 3D preview visible.
        u.texts
            .retain(|t| t.y + t.size * 1.25 < r[1] * u.scale || t.y >= (r[1] + r[3]) * u.scale);
        u.hits.clear();
        u.outlined_surface(r, palette.panel, palette.border);
        u.label(r[0] + 16., r[1] + 15., "Узлы материала", 14., palette.text);
        u.button(
            [r[0] + 166., r[1] + 9., 112., 30.],
            "Добавить ▾",
            Action::Node(NodeAction::AddMenu),
            self.nodes.add_menu,
        );
        u.checkbox(
            [r[0] + 292., r[1] + 10., 112., 28.],
            "Включены",
            Action::Node(NodeAction::Toggle),
            graph.enabled,
        );
        u.quiet_button(
            [r[0] + 414., r[1] + 9., 112., 30.],
            "Центр · F",
            Action::Node(NodeAction::ResetView),
            false,
        );
        u.quiet_button(
            [r[0] + r[2] - 104., r[1] + 9., 88., 30.],
            "Закрыть",
            Action::Node(NodeAction::Close),
            false,
        );
        u.rect(canvas, palette.bg);
        u.hits.push(Hit {
            rect: canvas,
            action: Action::Node(NodeAction::Pan),
        });
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        for node in &graph.nodes {
            for (slot, input) in node.inputs.iter().enumerate() {
                if let Some(source) = input {
                    if let (Some(a), Some(b)) = (
                        self.node_socket(&graph, *source, 0, true),
                        self.node_socket(&graph, node.id, slot, false),
                    ) {
                        wire(u, a, b, palette.accent);
                    }
                }
            }
        }
        for (slot, input) in graph.outputs.iter().enumerate() {
            if let Some(source) = input {
                if let (Some(a), Some(b)) = (
                    self.node_socket(&graph, *source, 0, true),
                    self.node_socket(&graph, 0, slot, false),
                ) {
                    wire(u, a, b, palette.accent);
                }
            }
        }
        if let Some(a) = self
            .nodes
            .source
            .and_then(|id| self.node_socket(&graph, id, 0, true))
        {
            wire(u, a, self.mouse, palette.warn);
        }
        for id in std::iter::once(0).chain(graph.nodes.iter().map(|n| n.id)) {
            let node = graph.nodes.iter().find(|n| n.id == id);
            let inputs = node.map_or(8, |n| n.kind.input_count());
            let p = self.node_screen(position(&graph, id).unwrap());
            let height = (80. + inputs.saturating_sub(1) as f32 * 28.) * zoom;
            let card = [p[0], p[1], 168. * zoom, height];
            let selected = id == self.nodes.selected;
            u.outlined_surface(
                card,
                palette.panel,
                if selected {
                    palette.accent
                } else {
                    palette.border
                },
            );
            u.hits.push(Hit {
                rect: card,
                action: Action::Node(NodeAction::Select(id)),
            });
            u.surface(
                [p[0] + 1., p[1] + 1., 168. * zoom - 2., 28. * zoom],
                if selected {
                    palette.selection
                } else {
                    palette.elevated
                },
            );
            let title = node.map_or("Поверхность PBR".to_string(), |n| {
                format!("{} · {}", n.id, n.kind.label())
            });
            u.fitted_label(
                [p[0] + 9. * zoom, p[1] + 7. * zoom, 148. * zoom, 17. * zoom],
                &title,
                12. * zoom,
                palette.text,
            );
            u.hits.push(Hit {
                rect: [p[0], p[1], 168. * zoom, 28. * zoom],
                action: Action::Node(NodeAction::Move(id)),
            });
            for slot in 0..inputs {
                let point = self.node_socket(&graph, id, slot, false).unwrap();
                let linked = if id == 0 {
                    graph.outputs[slot].is_some()
                } else {
                    node.unwrap().inputs[slot].is_some()
                };
                u.surface(
                    [point[0] - 5., point[1] - 5., 10., 10.],
                    if linked {
                        palette.accent
                    } else {
                        palette.muted
                    },
                );
                u.hits.push(Hit {
                    rect: [point[0] - 9., point[1] - 11., 24., 22.],
                    action: Action::Node(NodeAction::Input(id, slot)),
                });
                u.fitted_label(
                    [
                        point[0] + 14. * zoom,
                        point[1] - 7. * zoom,
                        120. * zoom,
                        15. * zoom,
                    ],
                    input_label(id, slot),
                    11. * zoom,
                    palette.text,
                );
            }
            if id != 0 {
                let point = self.node_socket(&graph, id, 0, true).unwrap();
                u.surface(
                    [point[0] - 5., point[1] - 5., 10., 10.],
                    if self.nodes.source == Some(id) {
                        palette.warn
                    } else {
                        palette.accent
                    },
                );
                u.hits.push(Hit {
                    rect: [point[0] - 14., point[1] - 11., 24., 22.],
                    action: Action::Node(NodeAction::Output(id)),
                });
                if inputs == 0 {
                    let n = node.unwrap();
                    let value = match n.kind {
                        MaterialNodeKind::Color => {
                            super::surfaces::color_hex([n.value[0], n.value[1], n.value[2]])
                        }
                        MaterialNodeKind::Texture => "Карта цвета материала".into(),
                        _ => format!("{:.3}", n.value[0]),
                    };
                    u.fitted_label(
                        [
                            p[0] + 10. * zoom,
                            p[1] + 45. * zoom,
                            135. * zoom,
                            16. * zoom,
                        ],
                        &value,
                        11. * zoom,
                        palette.muted,
                    );
                }
            }
        }
        clip_scrolled(u, start, canvas, 0.);
        // The renderer batches text after surfaces: trim node labels horizontally too.
        let mut labels = u.texts.split_off(start.1);
        labels.retain_mut(|text| {
            if text.x < canvas[0] * u.scale {
                return false;
            }
            let available = (canvas[0] + canvas[2]) * u.scale - text.x;
            text.text = ui::fit_button_label(&text.text, available.max(0.), text.size);
            available > text.size
        });
        u.texts.extend(labels);
        let inspector_start = (u.rects.len(), u.texts.len(), u.hits.len());
        let x = canvas[0] + canvas[2] + 16.;
        let width = r[0] + r[2] - x - 22.;
        let y = canvas[1];
        let id = self.nodes.selected;
        u.fitted_label(
            [x, y, width, 20.],
            &if id == 0 {
                "Поверхность PBR".into()
            } else {
                format!("Узел {id}")
            },
            13.,
            palette.text,
        );
        let mut cy = y + 30.;
        if let Some(node) = graph.nodes.iter().find(|n| n.id == id) {
            let fields = match node.kind {
                MaterialNodeKind::Color
                | MaterialNodeKind::ColorRamp
                | MaterialNodeKind::Coordinates => 4,
                MaterialNodeKind::Texture => 0,
                _ => 1,
            };
            for field in 0..fields {
                let label = if node.kind == MaterialNodeKind::Color {
                    ["R", "G", "B", "Альфа"][field]
                } else {
                    match node.kind {
                        MaterialNodeKind::Checker | MaterialNodeKind::Noise => "Масштаб",
                        MaterialNodeKind::Mix => "Фактор",
                        MaterialNodeKind::Multiply => "B без связи",
                        MaterialNodeKind::Bump | MaterialNodeKind::NormalMap => "Сила",
                        _ => "Значение",
                    }
                };
                u.label(x, cy + 6., label, 11., palette.muted);
                u.field(
                    [x + 78., cy, width - 78., 26.],
                    &format!("{:.3}", node.value[field]),
                    Action::Node(NodeAction::Value(field)),
                );
                cy += 32.;
            }
            if matches!(
                node.kind,
                MaterialNodeKind::Color | MaterialNodeKind::ColorRamp
            ) {
                u.field(
                    [x, cy, width, 26.],
                    &super::surfaces::color_hex([node.value[0], node.value[1], node.value[2]]),
                    Action::Node(NodeAction::Value(4)),
                );
                cy += 34.;
            }
            if node.kind == MaterialNodeKind::ColorRamp {
                u.field(
                    [x, cy, width, 26.],
                    &super::surfaces::color_hex([
                        node.secondary[0],
                        node.secondary[1],
                        node.secondary[2],
                    ]),
                    Action::Node(NodeAction::Value(5)),
                );
                cy += 34.;
            }
            if node.kind == MaterialNodeKind::Texture {
                let name = node
                    .texture
                    .and_then(|id| self.editor.project.textures.iter().find(|t| t.id == id))
                    .map_or("Карта материала", |t| t.name.as_str());
                u.button(
                    [x, cy, width, 28.],
                    name,
                    Action::Node(NodeAction::NextImage),
                    false,
                );
                cy += 34.;
                u.button(
                    [x, cy, width, 28.],
                    "Загрузить изображение…",
                    Action::Node(NodeAction::LoadImage),
                    false,
                );
                cy += 34.;
                u.checkbox(
                    [x, cy, width, 28.],
                    "Данные · Linear",
                    Action::Node(NodeAction::ImageData),
                    node.data_texture,
                );
                cy += 34.;
            }
        }
        if let Some(pos) = position(&graph, id) {
            for (axis, label) in ["X", "Y"].into_iter().enumerate() {
                u.label(x, cy + 6., label, 11., palette.muted);
                u.field(
                    [x + 78., cy, width - 78., 26.],
                    &format!("{:.1}", pos[axis]),
                    Action::Node(NodeAction::Position(axis)),
                );
                cy += 32.;
            }
        }
        let inputs = if id == 0 {
            graph.outputs.as_slice()
        } else {
            let n = graph.nodes.iter().find(|n| n.id == id).unwrap();
            &n.inputs[..n.kind.input_count()]
        };
        for (slot, input) in inputs.iter().enumerate() {
            u.fitted_label(
                [x, cy + 6., width - 30., 16.],
                &format!(
                    "{} ← {}",
                    input_label(id, slot),
                    input.map_or("основа".into(), |v| v.to_string())
                ),
                11.,
                palette.muted,
            );
            if input.is_some() {
                u.quiet_button(
                    [x + width - 26., cy, 26., 26.],
                    "×",
                    Action::Node(NodeAction::Disconnect(id, slot)),
                    false,
                );
            }
            cy += 28.;
        }
        if id != 0 {
            u.button(
                [x, cy + 6., width, 28.],
                "Удалить узел · Del",
                Action::Node(NodeAction::Delete),
                false,
            );
        }
        self.nodes.inspector_scroll = self
            .nodes
            .inspector_scroll
            .min(self.node_inspector_max_scroll());
        clip_scrolled(
            u,
            inspector_start,
            [x, y, width, canvas[3]],
            self.nodes.inspector_scroll,
        );
        let maximum = self.node_inspector_max_scroll();
        if maximum > 0. {
            u.vertical_scrollbar(
                self.node_inspector_scrollbar(),
                self.nodes.inspector_scroll / maximum,
                canvas[3] / (canvas[3] + maximum),
                Action::Node(NodeAction::InspectorScroll),
            );
        }
        u.fitted_label([r[0]+14.,r[1]+r[3]-21.,r[2]-28.,16.],"Выход → вход · колёсико: масштаб · средняя кнопка: панорама · Tab: узел · O: выход · 1–4: вход",10.,palette.muted);
        if self.nodes.add_menu {
            let page = ((self.height - 100.) / 30.)
                .floor()
                .min(MaterialNodeKind::ALL.len() as f32) as usize;
            let height = page as f32 * 30. + 12.;
            let menu = [
                r[0] + 166.,
                (r[1] + 42.).min(self.height - height - 36.),
                190.,
                height,
            ];
            let offset = self.nodes.candidate.saturating_sub(page - 1);
            u.hits.clear();
            u.hits.push(Hit {
                rect: [0., 0., self.width, self.height],
                action: Action::Node(NodeAction::Pan),
            });
            let physical = menu.map(|v| v * u.scale);
            u.texts.retain(|t| {
                t.x >= physical[0] + physical[2]
                    || t.x + t.text.chars().count() as f32 * t.size <= physical[0]
                    || t.y >= physical[1] + physical[3]
                    || t.y + t.size * 1.4 <= physical[1]
            });
            u.outlined_surface(menu, palette.panel, palette.border);
            for (i, kind) in MaterialNodeKind::ALL
                .into_iter()
                .enumerate()
                .skip(offset)
                .take(page)
            {
                let row = [
                    menu[0] + 6.,
                    menu[1] + 6. + (i - offset) as f32 * 30.,
                    178.,
                    28.,
                ];
                u.menu_row(
                    row,
                    Action::Node(NodeAction::Add(i)),
                    self.nodes.candidate == i,
                );
                u.label(row[0] + 9., row[1] + 7., kind.label(), 12., palette.text);
            }
        }
    }
}
