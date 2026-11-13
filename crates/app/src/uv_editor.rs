//! UV canvas: selection, captured transforms, seams and cancellable chart jobs.
use super::*;
use crate::ui::UvAction as U;
use glam::Vec2;
use poseforge_core::{Mesh, MeshConnectivity, MeshEdit, UvEdit};

pub(super) struct UvState {
    pub open: bool,
    return_panel: Option<(bool, u8)>,
    source_selection: BTreeSet<usize>,
    pub node: Option<u64>,
    mode: u8,
    tool: u8,
    pub selected: BTreeSet<usize>,
    edges: BTreeSet<(usize, usize)>,
    zoom: f32,
    pan: Vec2,
    stretch: bool,
    cache: std::cell::RefCell<Option<Arc<UvCache>>>,
}
impl Default for UvState {
    fn default() -> Self {
        Self {
            open: false,
            return_panel: None,
            source_selection: BTreeSet::new(),
            node: None,
            mode: 2,
            tool: 0,
            selected: BTreeSet::new(),
            edges: BTreeSet::new(),
            zoom: 1.,
            pan: Vec2::ZERO,
            stretch: false,
            cache: Default::default(),
        }
    }
}
struct UvCache {
    key: (u64, u64, u64, u64),
    graph: MeshConnectivity,
    islands: Vec<Vec<usize>>,
    stretch: Vec<f32>,
    seams: BTreeSet<(usize, usize)>,
    raw_seams: Vec<[u32; 2]>,
}
#[derive(Clone)]
pub(super) struct UvDrag {
    node: u64,
    original: Mesh,
    selected: BTreeSet<usize>,
    start: Vec2,
    center: Vec2,
    tool: u8,
    values: [f32; 4],
    fine: bool,
}
#[derive(Clone)]
pub(super) struct UvPan {
    start: Vec2,
    pub original: Vec2,
}
fn uv_key(mesh: &Mesh, i: usize) -> ([i64; 3], [i64; 2]) {
    (
        poseforge_core::position_key(mesh.vertices[i].position),
        mesh.vertices[i].uv.map(|x| (x as f64 * 1e7).round() as i64),
    )
}
fn polygon_contains(points: &[Vec2], p: Vec2) -> bool {
    let mut inside = false;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
/// Prepare off-thread; never mutate the live document or a shared mesh instance.
pub(super) fn prepare(
    project: &Project,
    node: u64,
    edit: UvEdit,
    cancel: &AtomicBool,
) -> Result<Project, String> {
    let checker = Editor::new(project.clone());
    let mut mesh = checker
        .editable_mesh(node)
        .map_err(|e| e.to_string())?
        .clone();
    poseforge_core::edit_uv(&mut mesh, edit, cancel).map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("UV: расчёт отменён".into());
    }
    let mut after = project.clone();
    if after
        .nodes
        .iter()
        .any(|n| n.id != node && n.mesh == Some(mesh.id))
    {
        mesh.id = after.next_id();
        after.node_mut(node).unwrap().mesh = Some(mesh.id);
        after.meshes.push(mesh);
    } else {
        let id = mesh.id;
        *after.meshes.iter_mut().find(|m| m.id == id).unwrap() = mesh;
    }
    after.validate().map_err(|e| e.to_string())?;
    Ok(after)
}
impl App {
    pub(super) fn selected_mesh_for_uv(&self) -> bool {
        self.active_light_index().is_none() && self.editor.selected
            .and_then(|id| self.editor.project.node(id)).is_some_and(|n| n.mesh.is_some() && n.curve.is_none())
    }

    pub(super) fn close_uv_workspace(&mut self, restore_panel: bool) {
        if !self.uv.open { return; }
        self.cancel_drag();
        let return_panel = self.uv.return_panel.take();
        self.uv.open = false;
        self.clear_uv_selection();
        if restore_panel {
            let (visible, tab) = return_panel.unwrap_or((false, 0));
            self.set_properties_tab(if tab == 12 { 0 } else { tab });
            self.preferences.right_panel = visible;
        }
    }

    pub(super) fn clear_uv_selection(&mut self) {
        self.uv.node = None;
        self.uv.selected.clear();
        self.uv.edges.clear();
        self.uv.source_selection.clear();
    }

    pub(super) fn draw_uv_properties(&self, u: &mut Ui, rx: f32, rw: f32) {
        let x = rx + 16.; let w = rw - 32.; let gap = 6.; let third = (w - gap * 2.) / 3.;
        let ready = self.uv.node.is_some() && self.job.is_none() && !self.editor.playing;
        let button = |u: &mut Ui, rect, label: &str, action, active| {
            if ready { u.button(rect, label, action, active); } else { u.disabled_button(rect, label); }
        };
        for (i, label) in ["Вершины", "Рёбра", "Острова"].into_iter().enumerate() {
            button(u, [x + i as f32 * (third + gap), 184., third, 30.], label,
                Action::Uv(U::Mode(i as u8)), self.uv.mode == i as u8);
        }
        for (i, (label, action, active)) in [
            ("Выделить всё", U::All, false), ("Вписать UV", U::Frame, false),
            ("Растяжение UV", U::Stretch, self.uv.stretch)].into_iter().enumerate() {
            button(u, [x, 230. + i as f32 * 38., w, 30.], label, Action::Uv(action), active);
        }
        for (i, label) in ["Сдвиг", "Поворот", "Масштаб"].into_iter().enumerate() {
            button(u, [x + i as f32 * (third + gap), 376., third, 30.], label,
                Action::Uv(U::Tool(i as u8)), self.uv.tool == i as u8);
        }
        let half = (w - gap) / 2.;
        for (i, label) in ["Сдвиг U", "Сдвиг V", "Угол, °", "Масштаб"].into_iter().enumerate() {
            let rect = [x + (i % 2) as f32 * (half + gap), 438. + (i / 2) as f32 * 64., half, 30.];
            u.label(rect[0], rect[1] - 18., label, 11., u.palette().muted);
            if ready && !self.uv.selected.is_empty() {
                u.field(rect, if i == 3 { "1" } else { "0" }, Action::Uv(U::PanelNumber(i as u8)));
            } else { u.disabled_button(rect, "—"); }
        }
        for (i, (label, action)) in [("Отметить шов", U::Seams(true)), ("Снять шов", U::Seams(false)),
            ("Развернуть по швам", U::Unwrap), ("Упаковать острова", U::Pack)].into_iter().enumerate() {
            button(u, [x, 584. + i as f32 * 40., w, 30.], label, Action::Uv(action), false);
        }
        u.fitted_label([x, 754., w, 18.], if ready { "Точные значения применяются к выделенным UV" }
            else { "Выберите меш для редактирования UV" }, 10., u.palette().muted);
    }
    pub(super) fn uv_rect(&self) -> [f32; 4] {
        let width = ((self.navigation_right() - 24.) * 0.48).clamp(240., 640.);
        [12., 72., width, (self.panel_bottom() - 72.).max(240.)]
    }
    fn uv_canvas(&self) -> [f32; 4] {
        let [x, y, w, h] = self.uv_rect();
        [x + 12., y + 180., w - 24., (h - 212.).max(24.)]
    }
    fn uv_unit(&self) -> f32 {
        let r = self.uv_canvas();
        r[2].min(r[3]).max(32.) * 0.78 * self.uv.zoom
    }
    fn uv_screen(&self, p: Vec2) -> Vec2 {
        let r = self.uv_canvas();
        Vec2::new(r[0] + r[2] / 2., r[1] + r[3] / 2.)
            + self.uv.pan
            + Vec2::new(p.x - 0.5, 0.5 - p.y) * self.uv_unit()
    }
    fn uv_point(&self, p: [f32; 2]) -> Vec2 {
        let r = self.uv_canvas();
        let q = (Vec2::from_array(p) - Vec2::new(r[0] + r[2] / 2., r[1] + r[3] / 2.) - self.uv.pan)
            / self.uv_unit();
        Vec2::new(q.x + 0.5, 0.5 - q.y)
    }
    fn uv_cache(&self, mesh: &Mesh) -> Arc<UvCache> {
        let key = mesh_pick::MeshCache::key(mesh);
        let mut slot = self.uv.cache.borrow_mut();
        let dragging = matches!(self.drag, Some(Drag::Uv(_)));
        if slot.as_ref().is_none_or(|c| {
            (c.key != key
                || c.raw_seams.as_slice()
                    != mesh
                        .topology
                        .as_ref()
                        .map_or(&[][..], |t| t.uv_seams.as_slice()))
                && !dragging
        }) {
            *slot = Some(Arc::new(UvCache {
                key,
                graph: MeshConnectivity::new(mesh),
                islands: poseforge_core::uv_islands(mesh),
                stretch: (0..mesh.face_count())
                    .map(|f| poseforge_core::uv_stretch(mesh, f))
                    .collect(),
                seams: poseforge_core::uv_seams(mesh),
                raw_seams: mesh
                    .topology
                    .as_ref()
                    .map_or_else(Vec::new, |t| t.uv_seams.clone()),
            }));
        }
        slot.as_ref().unwrap().clone()
    }
    pub(super) fn uv_prune(&mut self) {
        if !self.uv.open {
            return;
        }
        if self.editor.mode != EditorMode::Scene || !self.selected_mesh_for_uv() {
            self.uv_action(U::Close);
            return;
        }
        let node = self
            .authoring
            .editing
            .or(self.editor.selected)
            .filter(|id| self.node_selectable(*id) && self.editor.editable_mesh(*id).is_ok());
        if node != self.uv.node {
            if matches!(self.drag, Some(Drag::Uv(_) | Drag::UvPan(_))) {
                self.cancel_drag();
            }
            self.uv = UvState {
                open: true,
                return_panel: self.uv.return_panel,
                node,
                ..Default::default()
            };
            if let Some(id) = node {
                self.authoring.editing = Some(id);
                self.authoring.selection_mode = ui::MeshSelection::Face;
                self.authoring.vertices.clear();
                self.authoring.edges.clear();
                self.authoring.faces.clear();
            }
        }
        if let Some(mesh) = self
            .uv
            .node
            .and_then(|id| self.editor.editable_mesh(id).ok())
        {
            self.uv.selected.retain(|&i| i < mesh.vertices.len());
        }
        if self.authoring.editing == self.uv.node && self.uv.node.is_some() {
            let selected = self.edit_vertices();
            if selected != self.uv.source_selection {
                self.uv.selected = selected.clone();
                self.uv.source_selection = selected;
            }
        }
    }
    pub(super) fn uv_action(&mut self, a: U) {
        if a == U::Close {
            self.close_uv_workspace(true);
            self.redraw();
            return;
        }
        if self.job.is_some() || self.editor.playing {
            return;
        }
        if a == U::Open {
            if !self.selected_mesh_for_uv() { return; }
            if self.uv.open { self.set_properties_tab(12); self.preferences.right_panel = true; return; }
            self.cancel_modeling_live();
            self.cancel_drag();
            if self.editor.has_staged() {
                self.status = "Сначала поставьте ключ или отбросьте временную позу.".into();
                return;
            }
            self.editor.mode = EditorMode::Scene;
            let node = self
                .authoring
                .editing
                .or(self.editor.selected)
                .filter(|id| self.editor.editable_mesh(*id).is_ok());
            self.nodes.material = None;
            self.uv = UvState {
                open: true,
                return_panel: Some((self.preferences.right_panel, self.surface.tab)),
                node,
                ..Default::default()
            };
            if let Some(id) = node {
                if self.authoring.editing != Some(id) {
                    self.authoring.editing = Some(id);
                    self.authoring.selection_mode = crate::ui::MeshSelection::Face;
                    self.authoring.vertices.clear();
                    self.authoring.edges.clear();
                    self.authoring.faces.clear();
                }
                self.set_properties_tab(12);
            }
            self.set_properties_tab(12);
            self.preferences.right_panel = true;
            self.primitive_panel = None;
            self.menu = None;
            self.status="UV · вершины / рёбра / острова · G/R/S — инструменты · колесо — масштаб · средняя кнопка — панорама".into();
            self.redraw();
            return;
        }
        let Some(node) = self.uv.node else { return };
        match a {
            U::Backdrop => {
                self.uv_click();
            }
            U::Mode(mode) => {
                self.uv.mode = mode.min(2);
                self.uv.selected.clear();
                self.uv.edges.clear();
            }
            U::Tool(tool) => self.uv.tool = tool.min(2),
            U::Stretch => self.uv.stretch = !self.uv.stretch,
            U::Frame => self.uv_frame(),
            U::All => {
                if let Ok(mesh) = self.editor.editable_mesh(node) {
                    self.uv.selected = (0..mesh.vertices.len()).collect();
                }
            }
            U::Number(i) | U::PanelNumber(i) => {
                if !self.uv.selected.is_empty() {
                    self.input(Action::Uv(a), if i == 3 { "1" } else { "0" }.into());
                }
            }
            U::Seams(marked) => {
                let edges = if self.uv.edges.is_empty() {
                    self.authoring.edges.clone()
                } else {
                    self.uv.edges.clone()
                };
                if edges.is_empty() {
                    self.status = "Выберите рёбра слева в UV или справа на модели".into();
                    return;
                }
                let r = self
                    .editor
                    .edit_mesh(node, MeshEdit::Uv(UvEdit::Seams { edges, marked }));
                self.result(r);
                *self.uv.cache.borrow_mut() = None;
                self.update_dirty();
            }
            U::Unwrap | U::Pack => {
                let edit = if a == U::Unwrap {
                    UvEdit::Unwrap
                } else {
                    UvEdit::Pack { margin: 0.02 }
                };
                let before = self.editor.project.clone();
                let (tx, rx) = mpsc::channel();
                let cancel = Arc::new(AtomicBool::new(false));
                let c = cancel.clone();
                std::thread::spawn(move || {
                    let message = match prepare(&before, node, edit, &c) {
                        Ok(after) => JobMessage::UvReady(Box::new(before), Box::new(after)),
                        Err(e) => JobMessage::Error(e),
                    };
                    let _ = tx.send(message);
                });
                self.job = Some(Job {
                    receiver: rx,
                    cancel,
                    label: "Развёртка UV".into(),
                });
                self.status = "UV · расчёт в фоне · Esc — отменить".into();
            }
            U::Open | U::Close => {}
        }
        self.redraw();
    }
    fn uv_frame(&mut self) {
        let Some(mesh) = self
            .uv
            .node
            .and_then(|id| self.editor.editable_mesh(id).ok())
        else {
            return;
        };
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for (i, v) in mesh.vertices.iter().enumerate() {
            if self.uv.selected.is_empty() || self.uv.selected.contains(&i) {
                let p = Vec2::from_array(v.uv);
                min = min.min(p);
                max = max.max(p);
            }
        }
        if !min.is_finite() {
            return;
        }
        let r = self.uv_canvas();
        let span = (max - min).max(Vec2::splat(0.01));
        let unit = (r[2] * 0.85 / span.x).min(r[3] * 0.85 / span.y);
        self.uv.zoom = (unit / (r[2].min(r[3]).max(32.) * 0.78)).clamp(0.05, 100.);
        let c = (min + max) / 2.;
        self.uv.pan = Vec2::new(0.5 - c.x, c.y - 0.5) * self.uv_unit();
    }
    pub(super) fn uv_confirm(&mut self, a: &Action, text: &str) -> Result<bool, String> {
        let Action::Uv(U::Number(i) | U::PanelNumber(i)) = *a else {
            return Ok(false);
        };
        let node = self.uv.node.ok_or("UV закрыт")?;
        let value = surfaces::number(text, if i == 3 { 0.001 } else { -10000. }, 10000.)?;
        let mut offset = [0.; 2];
        if i < 2 {
            offset[i as usize] = value;
        }
        self.editor
            .edit_mesh(
                node,
                MeshEdit::Uv(UvEdit::Transform {
                    vertices: self.uv.selected.clone(),
                    offset,
                    angle: if i == 2 { value } else { 0. },
                    scale: if i == 3 { value } else { 1. },
                }),
            )
            .map_err(|e| e.to_string())?;
        self.invalidate_geometry();
        Ok(true)
    }
    fn uv_click(&mut self) {
        if !ui::contains(self.uv_canvas(), self.mouse) {
            return;
        }
        let Some(node) = self.uv.node else { return };
        let Ok(mesh) = self.editor.editable_mesh(node).cloned() else {
            return;
        };
        let c = self.uv_cache(&mesh);
        let pointer = Vec2::from_array(self.mouse);
        let p = self.uv_point(self.mouse);
        let mut chosen = BTreeSet::new();
        let mut edge = None;
        match self.uv.mode {
            0 => {
                let nearest = mesh
                    .vertices
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i, self.uv_screen(Vec2::from_array(v.uv)).distance(pointer)))
                    .filter(|(_, d)| *d <= 10.)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((v, _)) = nearest {
                    let key = uv_key(&mesh, v);
                    chosen = (0..mesh.vertices.len())
                        .filter(|&i| uv_key(&mesh, i) == key)
                        .collect();
                }
            }
            1 => {
                let mut best = 10.;
                for f in 0..mesh.face_count() {
                    let face = mesh.face_vertices(f).unwrap();
                    for j in 0..face.len() {
                        let a = face[j] as usize;
                        let b = face[(j + 1) % face.len()] as usize;
                        let pa = self.uv_screen(Vec2::from_array(mesh.vertices[a].uv));
                        let pb = self.uv_screen(Vec2::from_array(mesh.vertices[b].uv));
                        let d = pb - pa;
                        let t =
                            ((pointer - pa).dot(d) / d.length_squared().max(1e-12)).clamp(0., 1.);
                        let dist = (pa + d * t).distance(pointer);
                        if dist < best {
                            best = dist;
                            chosen = BTreeSet::from([a, b]);
                            let a = c.graph.canonical[a];
                            let b = c.graph.canonical[b];
                            edge = Some((a.min(b), a.max(b)));
                        }
                    }
                }
                let keys: BTreeSet<_> = chosen.iter().map(|&i| uv_key(&mesh, i)).collect();
                chosen = (0..mesh.vertices.len())
                    .filter(|&i| keys.contains(&uv_key(&mesh, i)))
                    .collect();
            }
            _ => {
                let face = (0..mesh.face_count()).find(|&f| {
                    let points: Vec<_> = mesh
                        .face_vertices(f)
                        .unwrap()
                        .iter()
                        .map(|&v| Vec2::from_array(mesh.vertices[v as usize].uv))
                        .collect();
                    polygon_contains(&points, p)
                });
                if let Some(face) = face {
                    if let Some(island) = c.islands.iter().find(|v| v.contains(&face)) {
                        for &f in island {
                            chosen
                                .extend(mesh.face_vertices(f).unwrap().iter().map(|&v| v as usize));
                        }
                    }
                }
            }
        }
        let shift = self.modifiers.shift_key();
        if shift {
            let remove = !chosen.is_empty() && chosen.iter().all(|i| self.uv.selected.contains(i));
            for i in chosen {
                if remove {
                    self.uv.selected.remove(&i);
                } else {
                    self.uv.selected.insert(i);
                }
            }
            if let Some(e) = edge {
                if remove {
                    self.uv.edges.remove(&e);
                } else {
                    self.uv.edges.insert(e);
                }
            }
            return;
        }
        if chosen.is_empty() {
            self.uv.selected.clear();
            self.uv.edges.clear();
            return;
        }
        if !chosen.iter().all(|i| self.uv.selected.contains(i)) {
            self.uv.selected = chosen;
            self.uv.edges.clear();
            if let Some(e) = edge {
                self.uv.edges.insert(e);
            }
        }
        // Centre uses the same unique UV coordinates as the core transform.
        let unique: BTreeSet<_> = self
            .uv
            .selected
            .iter()
            .map(|&i| mesh.vertices[i].uv.map(|x| (x as f64 * 1e7).round() as i64))
            .collect();
        let center = unique.iter().fold(Vec2::ZERO, |a, p| {
            a + Vec2::new(p[0] as f32 / 1e7, p[1] as f32 / 1e7)
        }) / unique.len().max(1) as f32;
        self.editor.begin_edit("Трансформация UV");
        self.drag = Some(Drag::Uv(UvDrag {
            node,
            original: mesh,
            selected: self.uv.selected.clone(),
            start: p,
            values: [0.; 4],
            fine: false,
            center,
            tool: self.uv.tool,
        }));
        self.drag_button = Some(MouseButton::Left);
    }
    pub(super) fn update_uv_drag(&mut self, mut d: UvDrag, p: [f32; 2]) {
        let current = self.uv_point(p);
        let fine = self.modifiers.shift_key();
        if fine != d.fine {
            d.fine = fine;
            d.start = current;
        }
        let speed = if fine { 0.1 } else { 1. };
        match d.tool {
            1 => {
                let a = d.start - d.center;
                let b = current - d.center;
                d.values[2] += a.perp_dot(b).atan2(a.dot(b)).to_degrees() * speed;
            }
            2 => {
                d.values[3] =
                    (d.values[3] + (current.x - d.start.x) * 3. * speed).clamp(-6.907755, 6.907755);
            }
            _ => {
                let delta = (current - d.start) * speed;
                d.values[0] += delta.x;
                d.values[1] += delta.y;
            }
        }
        d.start = current;
        let mut offset = [d.values[0], d.values[1]];
        let mut angle = d.values[2];
        let mut scale = d.values[3].exp();
        if self.modifiers.control_key() {
            match d.tool {
                1 => angle = (angle / 15.).round() * 15.,
                2 => scale = (scale * 10.).round().max(1.) / 10.,
                _ => offset = offset.map(|v| (v * 100.).round() / 100.),
            }
        }
        let r = self.editor.preview_mesh(
            d.node,
            &d.original,
            MeshEdit::Uv(UvEdit::Transform {
                vertices: d.selected.clone(),
                offset,
                angle,
                scale,
            }),
        );
        if self.result(r).is_some() {
            self.invalidate_geometry();
        }
        self.drag = Some(Drag::Uv(d));
    }
    pub(super) fn uv_middle(&mut self, button: MouseButton) -> bool {
        if !self.uv.open || self.has_modal() {
            return false;
        }
        if button == MouseButton::Middle
            && self.input.is_none()
            && ui::contains(self.uv_canvas(), self.mouse)
        {
            self.drag = Some(Drag::UvPan(UvPan {
                start: Vec2::from_array(self.mouse),
                original: self.uv.pan,
            }));
            self.drag_button = Some(button);
            return true;
        }
        button != MouseButton::Left && ui::contains(self.uv_rect(), self.mouse)
    }
    pub(super) fn update_uv_pan(&mut self, d: UvPan, p: [f32; 2]) {
        self.uv.pan = d.original + Vec2::from_array(p) - d.start;
        self.drag = Some(Drag::UvPan(d));
    }
    pub(super) fn cancel_uv_pan(&mut self, d: &Drag) {
        if let Drag::UvPan(d) = d {
            self.uv.pan = d.original;
        }
    }
    pub(super) fn uv_scroll(&mut self, delta: f32) -> bool {
        if !self.uv.open || self.has_modal() {
            return false;
        }
        if self.drag.is_none() && self.input.is_none() && ui::contains(self.uv_canvas(), self.mouse)
        {
            let p = self.uv_point(self.mouse);
            self.uv.zoom = (self.uv.zoom * (delta * 0.12).exp()).clamp(0.05, 100.);
            self.uv.pan += Vec2::from_array(self.mouse) - self.uv_screen(p);
            self.redraw();
        }
        ui::contains(self.uv_rect(), self.mouse)
    }
    pub(super) fn uv_key(&mut self, key: &Key, ctrl: bool) -> bool {
        if self.has_modal() {
            return false;
        }
        if !self.uv.open {
            if !ctrl
                && self.authoring.editing.is_some()
                && matches!(key,Key::Character(c) if c.eq_ignore_ascii_case("u"))
            {
                self.action(Action::Uv(U::Open));
                return true;
            }
            return false;
        }
        if *key == Key::Named(NamedKey::Escape) {
            if self.drag.is_some() {
                self.cancel_drag();
            } else if self.job.is_some() {
                self.action(Action::Cancel);
            } else {
                self.uv_action(U::Close);
            }
            self.redraw();
            return true;
        }
        if !ui::contains(self.uv_rect(), self.mouse)
            && !matches!(self.drag, Some(Drag::Uv(_) | Drag::UvPan(_)))
        {
            return false;
        }
        if ctrl
            && matches!(key,Key::Character(c) if c.eq_ignore_ascii_case("z") || c.eq_ignore_ascii_case("y") || c.eq_ignore_ascii_case("s") || c.eq_ignore_ascii_case("o"))
        {
            return false;
        }
        if self.drag.is_some() {
            return true;
        }
        let a = match key {
            Key::Character(c) => match c.to_lowercase().as_str() {
                "a" => Some(U::All),
                "g" => Some(U::Tool(0)),
                "r" => Some(U::Tool(1)),
                "s" => Some(U::Tool(2)),
                "1" => Some(U::Mode(0)),
                "2" => Some(U::Mode(1)),
                "3" => Some(U::Mode(2)),
                "f" => Some(U::Frame),
                _ => None,
            },
            _ => None,
        };
        if let Some(a) = a {
            self.action(Action::Uv(a));
        }
        true
    }
    pub(super) fn draw_uv_editor(&self, u: &mut Ui) {
        if !self.uv.open {
            return;
        }
        let p = u.palette();
        let [x, y, w, h] = self.uv_rect();
        u.outlined_surface([x, y, w, h], p.panel, p.border);
        u.hits.push(ui::Hit {
            rect: [x, y, w, h],
            action: Action::Uv(U::Backdrop),
        });
        u.quiet_icon_button(
            [x + w - 34., y + 5., 28., 28.],
            "",
            Action::Uv(U::Close),
            false,
            Icon::Close,
        );
        let mesh = self
            .uv
            .node
            .and_then(|id| self.editor.editable_mesh(id).ok());
        u.fitted_label(
            [x + 12., y + 12., w - 58., 20.],
            &mesh.map_or_else(|| "UV".into(), |m| format!("UV · {}", m.name)),
            13.,
            p.accent,
        );
        let Some(mesh) = mesh else {
            u.fitted_label(
                [x + 14., y + 62., w - 28., 24.],
                "Выберите меш в просмотре справа",
                12.,
                p.text,
            );
            u.fitted_label(
                [x + 14., y + 94., w - 28., 48.],
                "UV доступны для редактируемого меша без skin и shape keys.",
                11.,
                p.muted,
            );
            return;
        };
        let c = self.uv_cache(mesh);
        let commands = [
            (U::Mode(0), Icon::Vertex),
            (U::Mode(1), Icon::Edge),
            (U::Mode(2), Icon::UvIslands),
            (U::Tool(0), Icon::Move),
            (U::Tool(1), Icon::Rotate),
            (U::Tool(2), Icon::Scale),
            (U::Frame, Icon::Frame),
            (U::Stretch, Icon::Eye),
        ];
        let bw = (w - 28. - 7. * 4.) / 8.;
        for (i, (a, icon)) in commands.into_iter().enumerate() {
            let active = match a {
                U::Mode(m) => self.uv.mode == m,
                U::Tool(t) => self.uv.tool == t,
                U::Stretch => self.uv.stretch,
                _ => false,
            };
            u.quiet_icon_button(
                [x + 14. + i as f32 * (bw + 4.), y + 38., bw, 28.],
                "",
                Action::Uv(a),
                active,
                icon,
            );
        }
        let half = (w - 34.) * 0.5;
        for (i, (label, a)) in [
            ("Шов +", U::Seams(true)),
            ("Шов −", U::Seams(false)),
            ("Развернуть", U::Unwrap),
            ("Упаковать", U::Pack),
        ]
        .into_iter()
        .enumerate()
        {
            let r = [
                x + 14. + (i % 2) as f32 * (half + 6.),
                y + 74. + (i / 2) as f32 * 34.,
                half,
                28.,
            ];
            if self.job.is_some() {
                u.disabled_button(r, label);
            } else {
                u.button(r, label, Action::Uv(a), false);
            }
        }
        let sw = (w - 46.) / 4.;
        for (i, label) in ["ΔU", "ΔV", "°", "×"].into_iter().enumerate() {
            let r = [x + 14. + i as f32 * (sw + 6.), y + 142., sw, 28.];
            if self.uv.selected.is_empty() {
                u.disabled_button(r, label);
            } else {
                u.field(r, label, Action::Uv(U::Number(i as u8)));
            }
        }
        let canvas = self.uv_canvas();
        u.rect(canvas, p.bg);
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        for row in 0..10 {
            for col in 0..10 {
                let a = self.uv_screen(Vec2::new(col as f32 / 10., (row + 1) as f32 / 10.));
                let size = self.uv_unit() / 10.;
                let color = if (row + col) % 2 == 0 {
                    [0.32, 0.34, 0.36, 0.3]
                } else {
                    [0.5, 0.52, 0.54, 0.3]
                };
                u.rect([a.x, a.y, size, size], color);
            }
        }
        let a = self.uv_screen(Vec2::new(0., 1.));
        u.surface_outline([a.x, a.y, self.uv_unit(), self.uv_unit()], p.muted);
        for f in 0..mesh.face_count() {
            let face = mesh.face_vertices(f).unwrap();
            for j in 0..face.len() {
                let a = face[j] as usize;
                let b = face[(j + 1) % face.len()] as usize;
                let pa = self.uv_screen(Vec2::from_array(mesh.vertices[a].uv));
                let pb = self.uv_screen(Vec2::from_array(mesh.vertices[b].uv));
                let ca = c.graph.canonical[a];
                let cb = c.graph.canonical[b];
                let selected = self.uv.selected.contains(&a) && self.uv.selected.contains(&b);
                let seam = c.seams.contains(&(ca.min(cb), ca.max(cb)));
                let heat = ((c.stretch[f].max(1.).log2()) / 3.).clamp(0., 1.);
                let color = if selected {
                    p.accent
                } else if seam {
                    [1., 0.4, 0.2, 1.]
                } else if self.uv.stretch {
                    [heat, 0.7 * (1. - heat), 1. - heat, 1.]
                } else {
                    p.text
                };
                u.mesh_segment(
                    pa.to_array(),
                    pb.to_array(),
                    if selected || seam { 1.8 } else { 0.8 },
                    color,
                );
            }
        }
        if self.uv.mode == 0 {
            for (i, v) in mesh.vertices.iter().enumerate() {
                let point = self.uv_screen(Vec2::from_array(v.uv));
                let selected = self.uv.selected.contains(&i);
                let size = if selected { 5. } else { 3. };
                u.rect(
                    [point.x - size / 2., point.y - size / 2., size, size],
                    if selected { p.accent } else { p.text },
                );
            }
        }
        clip_scrolled(u, start, canvas, 0.);
        let hint = if self.job.is_some() {
            "Расчёт в фоне · Esc — отменить"
        } else if self.uv.stretch {
            "Растяжение: синий 1× → красный ≥8× · швы оранжевые"
        } else {
            "Shift — добавить / убрать · Ctrl — шаг · средняя кнопка — панорама · Esc — отменить"
        };
        u.fitted_label([x + 14., y + h - 26., w - 28., 20.], hint, 11., p.muted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new(None);
        app.preferences = Preferences::default();
        app.width = 1280.;
        app.height = 800.;
        let mut node = poseforge_core::Node::new(1, "UV test");
        node.mesh = Some(2);
        app.editor = Editor::new(Project {
            nodes: vec![node],
            meshes: vec![poseforge_core::PrimitiveSettings::new(
                poseforge_core::PrimitiveKind::Cube,
            )
            .mesh(2, None)
            .unwrap()],
            ..Default::default()
        });
        app.editor.mode = EditorMode::Scene;
        app.editor.selected = Some(1);
        app.authoring.editing = Some(1);
        app
    }
    #[test]
    fn uv_drag_cancel_commit_and_pan_keep_document_and_view_ownership() {
        let mut a = app();
        a.action(Action::Uv(U::Open));
        let original = a.editor.project.clone();
        let view = (a.yaw, a.pitch, a.target);
        a.ui = a.build_ui();
        a.mouse = a.uv_screen(Vec2::new(0.4, 0.4)).to_array();
        a.mouse_down(MouseButton::Left);
        assert!(matches!(a.drag, Some(Drag::Uv(_))));
        if let Some(Drag::Uv(d)) = a.drag.take() {
            let p = (Vec2::from_array(a.mouse) + Vec2::new(35., 17.)).to_array();
            a.update_uv_drag(d, p);
        }
        assert_ne!(a.editor.project, original);
        a.action(Action::Cancel);
        assert_eq!(a.editor.project, original);
        assert!(a.uv.node.is_some());
        a.ui = a.build_ui();
        a.mouse_down(MouseButton::Left);
        if let Some(Drag::Uv(d)) = a.drag.take() {
            a.update_uv_drag(
                d,
                (Vec2::from_array(a.mouse) + Vec2::new(22., 0.)).to_array(),
            );
        }
        a.mouse_up(MouseButton::Left);
        assert_ne!(a.editor.project, original);
        a.editor.undo();
        assert_eq!(a.editor.project, original);
        a.mouse_down(MouseButton::Middle);
        let old = a.uv.pan;
        if let Some(Drag::UvPan(d)) = a.drag.take() {
            a.update_uv_pan(d, [a.mouse[0] + 50., a.mouse[1] + 20.]);
        }
        assert_ne!(a.uv.pan, old);
        a.cancel_drag();
        assert_eq!(a.uv.pan, old);
        assert_eq!((a.yaw, a.pitch, a.target), view);
    }
    #[test]
    fn uv_controls_survive_theme_dpi_and_dont_click_through() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for scale in [1., 2.] {
                let mut a = app();
                a.preferences.theme = theme;
                a.scale = scale;
                a.action(Action::Uv(U::Open));
                a.ui = a.build_ui();
                assert!(a.ui.texts.iter().any(|t| t.text.starts_with("UV")));
                assert!(!a.ui.hits.iter().any(|h| h.action == Action::SearchScene));
                for action in [
                    U::Mode(0),
                    U::Mode(1),
                    U::Mode(2),
                    U::Tool(0),
                    U::Tool(1),
                    U::Tool(2),
                    U::Unwrap,
                    U::Pack,
                    U::Seams(true),
                    U::Stretch,
                    U::Close,
                ] {
                    let action = Action::Uv(action);
                    let h = a.ui.hits.iter().find(|h| h.action == action).unwrap();
                    let center = [h.rect[0] + h.rect[2] / 2., h.rect[1] + h.rect[3] / 2.];
                    assert_eq!(a.ui.hit(center), Some(action));
                    assert!(h.rect[0] >= 0. && h.rect[0] + h.rect[2] <= a.width);
                }
                let before = a.editor.project.clone();
                a.mouse = [3., 100.];
                a.mouse_down(MouseButton::Left);
                assert_eq!(a.editor.project, before);
                assert!(a.drag.is_none());
            }
        }
    }
    #[test]
    fn uv_split_keeps_model_navigation_and_restores_scene_preference() {
        for visible in [false, true] {
            let mut a = app();
            a.preferences.left_panel = visible;
            a.action(Action::Uv(U::Open));
            a.ui = a.build_ui();
            assert!(!a.scene_panel_visible());
            assert_eq!(a.preferences.left_panel, visible);
            let uv = a.uv_rect();
            let model = a.viewport();
            assert!(uv[0] + uv[2] < model[0]);
            assert!(model[0] + model[2] <= a.right_panel_rect()[0]);
            let original = a.editor.project.clone();
            a.mouse = a.uv_screen(Vec2::splat(0.5)).to_array();
            let distance = a.distance;
            let zoom = a.uv.zoom;
            a.scroll(1.);
            assert_ne!(a.uv.zoom, zoom);
            assert_eq!(a.distance, distance);
            assert!(a.uv_key(&Key::Character("r".into()), false));
            assert_eq!(a.uv.tool, 1);
            a.mouse = [model[0] + model[2] * 0.5, model[1] + model[3] * 0.5];
            assert!(a.canvas_input_at(a.mouse));
            assert!(!a.uv_key(&Key::Character("g".into()), false));
            assert!(!a.uv_middle(MouseButton::Middle));
            let zoom = a.uv.zoom;
            a.scroll(1.);
            assert_ne!(a.distance, distance);
            assert_eq!(a.uv.zoom, zoom);
            assert_eq!(a.editor.project, original);
            a.action(Action::Uv(U::Close));
            assert_eq!(a.scene_panel_visible(), visible);
        }
    }

    #[test]
    fn uv_face_selection_is_visible_on_model_and_tracks_component_selection() {
        let mut a = app();
        a.authoring.selection_mode = ui::MeshSelection::Face;
        a.action(Action::Uv(U::Open));
        a.authoring.faces = BTreeSet::from([0]);
        a.build_ui();
        let expected: BTreeSet<_> = a
            .editor
            .editable_mesh(1)
            .unwrap()
            .face_vertices(0)
            .unwrap()
            .iter()
            .map(|i| *i as usize)
            .collect();
        assert_eq!(a.uv.selected, expected);
        a.uv_action(U::All);
        a.build_ui();
        assert_eq!(
            a.uv.selected.len(),
            a.editor.editable_mesh(1).unwrap().vertices.len()
        );
        let mut overlay = Ui::with_theme(1., ui::Theme::Dark, [0., 0.]);
        a.draw_mesh_overlay(&mut overlay);
        assert!(overlay.rects.iter().any(|r| r.triangle.is_some()));
        a.authoring.faces.clear();
        a.build_ui();
        assert!(a.uv.selected.is_empty());
    }

    #[test]
    fn uv_tab_requires_a_selected_mesh_and_attaches_it_when_opened() {
        let mut a = app();
        a.authoring.editing = None;
        a.editor.selected = None;
        a.action(Action::Uv(U::Open));
        assert!(!a.uv.open);
        assert!(a.uv.node.is_none());
        a.editor.selected = Some(1);
        a.action(Action::Uv(U::Open));
        a.build_ui();
        assert!(a.uv.open);
        assert_eq!(a.uv.node, Some(1));
        assert_eq!(a.authoring.editing, Some(1));
    }

    #[test]
    fn uv_job_preparation_is_cancelled_or_valid_and_instances_stay_independent() {
        let a = app();
        let original = a.editor.project.clone();
        assert!(prepare(&original, 1, UvEdit::Unwrap, &AtomicBool::new(true)).is_err());
        let after = prepare(&original, 1, UvEdit::Unwrap, &AtomicBool::new(false)).unwrap();
        after.validate().unwrap();
        assert_ne!(after, original);
        assert_eq!(a.editor.project, original);
    }
}
