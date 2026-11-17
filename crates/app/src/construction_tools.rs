//! Scene construction tools with frozen inputs, cancelable workers and one undo step.
use super::*;
use crate::ui::ConstructionAction as C;
use poseforge_core::{
    BooleanMode, CableSettings, CollisionTarget, Material, Node, PhysicsKind, PhysicsSettings,
};
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Boolean,
    Cable,
}
pub(super) struct Session {
    pub(super) kind: Kind,
    source: Arc<Project>,
    pick_scene: Option<poseforge_core::EvaluatedScene>,
    owner_ancestors: BTreeSet<u64>,
    owner: u64,
    selected: Option<u64>,
    selection: BTreeSet<u64>,
    targets: Vec<u64>,
    cleanup: bool,
    outputs: BTreeSet<u64>,
    mode: u8,
    pub(super) cable: CableSettings,
    pub(super) physics: PhysicsSettings,
    pub(super) collisions: bool,
    hide: bool,
    offset: [f32; 3],
    pub(super) pick: Option<u8>,
    ready: bool,
    error: Option<String>,
    worker: Option<(Receiver<Result<BooleanPreview, String>>, Arc<AtomicBool>)>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some((_, c)) = &self.worker {
            c.store(true, Ordering::Relaxed);
        }
    }
}
#[derive(Default)]
pub(super) struct State {
    pub session: Option<Session>,
    position: Option<[f32; 2]>,
    scroll: f32,
    collapsed: bool,
    pub(super) cable_tab: u8,
}
#[derive(Clone)]
pub(super) enum Gesture {
    Scroll {
        start: f32,
        value: f32,
        scale: f32,
        maximum: f32,
    },
    Panel {
        start: [f32; 2],
        origin: [f32; 2],
        original: Option<[f32; 2]>,
    },
    End {
        index: u8,
        original: CableSettings,
        origin: Vec3,
        normal: Vec3,
    },
}
struct BooleanPreview {
    project: Project,
    selection: BTreeSet<u64>,
}
fn owner_ancestors(source: &Project, owner: u64) -> BTreeSet<u64> {
    let mut parents = BTreeSet::new();
    if source.node(owner).is_none_or(|n| n.parent.is_none()) {
        return parents;
    }
    let nodes: std::collections::HashMap<_, _> =
        source.nodes.iter().map(|n| (n.id, n.parent)).collect();
    let mut current = owner;
    while let Some(&Some(parent)) = nodes.get(&current) {
        if !parents.insert(parent) {
            break;
        }
        current = parent;
    }
    parents
}

fn is_boolean_mesh(node: &Node, mesh: Option<&poseforge_core::Mesh>) -> bool {
    mesh.is_some_and(|m| m.morph_targets.is_empty())
        && node.skin.is_none()
        && node.physics.is_none()
        && node.cable.is_none()
        && node.curve.is_none()
        && node.text.is_none()
        && node.fluid.is_none()
        && node.destruction.is_none()
}
fn build_boolean(
    source: &Project,
    owner: u64,
    targets: &[u64],
    mode: u8,
    offset: [f32; 3],
    hide: bool,
    cleanup: bool,
    cancel: &AtomicBool,
) -> Result<BooleanPreview, String> {
    use poseforge_core::{BooleanOperation, BooleanOptions};
    use std::collections::HashMap;
    if targets.is_empty() {
        return Err("Добавьте объект-резак в сцене".into());
    }
    let ancestors = owner_ancestors(source, owner);
    if targets
        .iter()
        .any(|id| ancestors.contains(id) || *id == owner)
    {
        return Err("Исходный объект и его родители не могут быть резаками".into());
    }
    let globals = source.global_transforms().map_err(|e| e.to_string())?;
    let model = globals.get(&owner).ok_or("Исходник удалён")?;
    if model.determinant().abs() < 1e-8 {
        return Err("Восстановите ненулевой масштаб".into());
    }
    let group_transform = model.inverse() * Mat4::from_translation(Vec3::from_array(offset));
    let nodes: HashMap<_, _> = source.nodes.iter().map(|node| (node.id, node)).collect();
    let meshes: HashMap<_, _> = source.meshes.iter().map(|mesh| (mesh.id, mesh)).collect();

    // Only participating modifiers affect this operation. A heavy unrelated stack
    // must not delay sliding a cutter or prevent a valid pair from being used.
    let participants: std::collections::HashSet<_> = std::iter::once(owner)
        .chain(targets.iter().copied())
        .collect();
    let mut modified = HashMap::new();
    for (id, input) in
        source.prepare_enabled_modifiers(&globals, |node| participants.contains(&node.id))
    {
        if cancel.load(Ordering::Relaxed) {
            return Err("Булева операция отменена".into());
        }
        let mesh = input
            .and_then(|stack| stack.evaluate(cancel))
            .map_err(|e| e.to_string())?;
        modified.insert(id, mesh);
    }
    let mesh_for = |id: u64| {
        modified
            .get(&id)
            .or_else(|| meshes.get(&nodes.get(&id)?.mesh?).copied())
            .ok_or_else(|| "Объект больше недоступен".to_string())
    };
    let cutters: Vec<_> = targets
        .iter()
        .map(|&id| Ok((mesh_for(id)?, group_transform * globals[&id])))
        .collect::<Result<_, String>>()?;
    let operation = match mode {
        0 => BooleanOperation::Combine(BooleanMode::Union),
        2 => BooleanOperation::Combine(BooleanMode::Intersection),
        3 => BooleanOperation::Split,
        _ => BooleanOperation::Combine(BooleanMode::Difference),
    };
    let (mut mesh, mut inside) = poseforge_core::boolean_mesh_many(
        mesh_for(owner)?,
        &cutters,
        operation,
        BooleanOptions {
            dissolve_coplanar: cleanup,
        },
        cancel,
    )
    .map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Булева операция отменена".into());
    }
    if mesh.indices.is_empty() && inside.as_ref().is_some_and(|m| !m.indices.is_empty()) {
        mesh = inside.take().unwrap();
    }
    let mut p = source.clone();
    mesh.id = p.next_id();
    let n = p.node_mut(owner).ok_or("Исходник удалён")?;
    n.mesh = Some(mesh.id);
    n.modifiers.clear();
    p.meshes.push(mesh);
    let mut selection = BTreeSet::from([owner]);
    if let Some(mut mesh) = inside.filter(|m| !m.indices.is_empty()) {
        let mut piece = p.node(owner).unwrap().clone();
        piece.id = p.next_id();
        piece.name = format!("{} · отделённая часть", piece.name);
        mesh.id = piece.id + 1;
        piece.mesh = Some(mesh.id);
        selection.insert(piece.id);
        p.nodes.push(piece);
        p.meshes.push(mesh);
    }
    if hide {
        let hidden: std::collections::HashSet<_> = targets.iter().copied().collect();
        for node in &mut p.nodes {
            if hidden.contains(&node.id) {
                node.visible = false;
            }
        }
    }
    p.validate().map_err(|e| e.to_string())?;
    Ok(BooleanPreview {
        project: p,
        selection,
    })
}
impl App {
    pub(super) fn start_construction(&mut self, cable: bool) {
        if self.job.is_some()
            || self.scene_write_pending()
            || self.editor.playing
            || self.editor.has_staged()
        {
            return;
        }
        self.cancel_construction();
        self.cancel_modeling_session();
        self.cancel_modeling_live();
        self.cancel_drag();
        self.dismiss_modeling_tool_panel();
        let source = Arc::new(self.editor.project.clone());
        let selected = self.editor.selected;
        let selection = self.scene_tools.selected.clone();
        let existing = selected.and_then(|id| {
            source
                .node(id)
                .filter(|n| n.cable.is_some())
                .map(|n| (id, n.cable.unwrap()))
        });
        if cable && existing.is_some() && !selected.is_some_and(|id| self.node_selectable(id)) {
            self.status = "Кабель скрыт или заблокирован".into();
            return;
        }
        let owner = if cable {
            existing.map_or(source.next_id(), |v| v.0)
        } else {
            let Some(id) = selected.filter(|&id| self.boolean_candidate(id)) else {
                self.status = "Выберите статический меш для булевой операции".into();
                return;
            };
            id
        };
        let owner_ancestors = if cable {
            BTreeSet::new()
        } else {
            owner_ancestors(&source, owner)
        };
        let mut targets = Vec::new();
        if !cable && selection.len() > usize::from(selection.contains(&owner)) {
            let selectable = source.selectable_node_ids();
            let isolated = (!self.scene_tools.isolated.is_empty())
                .then(|| source.descendants_of(&self.scene_tools.isolated));
            let meshes: std::collections::HashMap<_, _> =
                source.meshes.iter().map(|m| (m.id, m)).collect();
            for node in &source.nodes {
                if node.id != owner
                    && selection.contains(&node.id)
                    && selectable.contains(&node.id)
                    && !owner_ancestors.contains(&node.id)
                    && isolated.as_ref().is_none_or(|ids| ids.contains(&node.id))
                    && is_boolean_mesh(node, node.mesh.and_then(|id| meshes.get(&id).copied()))
                {
                    targets.push(node.id);
                }
            }
            targets.sort_unstable();
        }
        let pick_scene = if cable {
            None
        } else {
            self.evaluated_interactive().ok()
        };
        let needs_target = targets.is_empty();
        let mut settings = existing.map_or_else(CableSettings::default, |v| v.1);
        if existing.is_none() {
            settings.start = (self.target + Vec3::new(-1., 1., 0.)).to_array();
            settings.end = (self.target + Vec3::new(1., 1., 0.)).to_array();
        }
        let mut physics = source
            .node(owner)
            .and_then(|n| n.physics)
            .unwrap_or(PhysicsSettings {
                kind: PhysicsKind::Cable,
                bending: 30.,
                compliance: 0.000001,
                iterations: 16,
                ..Default::default()
            });
        if cable {
            physics.kind = PhysicsKind::Cable;
            physics.self_collision = false;
        }
        let collisions = source
            .node(owner)
            .and_then(|n| n.collision_targets.cloth.as_ref())
            .is_none_or(|list| list.iter().any(|c| c.enabled));
        self.editor.begin_tool_edit(if cable {
            "Кабель"
        } else {
            "Булева операция"
        });
        self.construction.session = Some(Session {
            kind: if cable { Kind::Cable } else { Kind::Boolean },
            source,
            pick_scene,
            owner_ancestors,
            owner,
            selected,
            selection,
            targets,
            cleanup: true,
            outputs: BTreeSet::from([owner]),
            mode: 1,
            cable: settings,
            physics,
            collisions,
            hide: true,
            offset: [0.; 3],
            pick: if cable && existing.is_none() {
                Some(0)
            } else if !cable && needs_target {
                Some(2)
            } else {
                None
            },
            ready: false,
            error: None,
            worker: None,
        });
        self.construction.cable_tab = 0;
        self.construction.scroll = 0.;
        self.construction.collapsed = false;
        self.primitive_panel = None;
        self.primitive_draw_kind = None;
        self.authoring = Default::default();
        self.editor.mode = EditorMode::Scene;
        self.set_properties_tab(7);
        self.refresh_construction();
    }
    pub(super) fn boolean_content_height(&self) -> f32 {
        let count = self
            .construction
            .session
            .as_ref()
            .map_or(0, |s| s.targets.len());
        68. + 36. + count as f32 * 32. + if count > 0 { 32. } else { 0. } + 126. + 64. + 24.
    }
    pub(super) fn cable_dropdown_target(&self) -> Option<(u64, CableSettings)> {
        let s = self.construction.session.as_ref()?;
        (s.kind == Kind::Cable).then_some((s.owner, s.cable))
    }
    fn boolean_candidate(&self, id: u64) -> bool {
        let project = &self.editor.project;
        self.node_selectable(id)
            && project
                .node(id)
                .is_some_and(|n| is_boolean_mesh(n, n.mesh.and_then(|id| project.mesh(id))))
    }
    pub(super) fn construction_action(&mut self, a: C) {
        match a {
            C::CableTab(tab) => {
                self.construction.cable_tab = tab.min(2);
                self.construction.scroll = 0.;
                return;
            }
            C::Boolean => {
                self.start_construction(false);
                return;
            }
            C::Cable => {
                self.start_construction(true);
                return;
            }
            C::Mirror => {
                self.cancel_construction();
                let Some(id) = self.editor.selected else {
                    return;
                };
                if self.editor.editable_mesh(id).is_err() {
                    return;
                }
                self.authoring_action(ui::AuthorAction::SelectMode(ui::MeshSelection::Face));
                self.authoring_action(ui::AuthorAction::SelectAll);
                self.choose_modeling_tool(ui::ModelingTool::Mirror);
                return;
            }
            C::Apply => {
                self.apply_construction();
                return;
            }
            C::Cancel => {
                self.cancel_construction();
                return;
            }
            C::Backdrop => return,
            C::Collapse => {
                self.construction.collapsed = !self.construction.collapsed;
                return;
            }
            C::Scroll => {
                if let Some(r) = self.construction_rect() {
                    let total = self.construction_content_height();
                    let height = r[3] - 94. - self.construction_tabs_height();
                    let maximum = (total - height).max(0.);
                    let thumb = (height * height / total).max(20.);
                    let value = self.construction.scroll.min(maximum);
                    self.drag = Some(Drag::Construction(Gesture::Scroll {
                        start: self.mouse[1],
                        value,
                        scale: maximum / (height - thumb).max(1.),
                        maximum,
                    }));
                    self.drag_button = Some(MouseButton::Left);
                }
                return;
            }
            C::Panel => {
                if let Some(r) = self.construction_rect() {
                    self.drag = Some(Drag::Construction(Gesture::Panel {
                        start: self.mouse,
                        origin: [r[0], r[1]],
                        original: self.construction.position,
                    }));
                    self.drag_button = Some(MouseButton::Left);
                }
                return;
            }
            C::Handle(i) => {
                if let Some(s) = &self.construction.session {
                    let original = s.cable;
                    let model = s
                        .source
                        .global_transforms()
                        .ok()
                        .and_then(|g| g.get(&s.owner).copied())
                        .unwrap_or(Mat4::IDENTITY);
                    let origin = model.transform_point3(Vec3::from_array(if i == 0 {
                        s.cable.start
                    } else {
                        s.cable.end
                    }));
                    if let Some((_, ray)) =
                        crate::gizmo::pointer_ray(&self.view(), self.viewport(), self.mouse)
                    {
                        self.drag = Some(Drag::Construction(Gesture::End {
                            index: i,
                            original,
                            origin,
                            normal: -ray,
                        }));
                        self.drag_button = Some(MouseButton::Left);
                    }
                }
                return;
            }
            C::Field(i) => {
                if let Some((v, _, _, _)) = self.construction_field(i) {
                    self.input(Action::Construction(a), ui::number_caption(v as f64));
                }
                return;
            }
            C::Play => {
                self.studio.physics_running = !self.studio.physics_running;
                self.studio.physics.reset();
                return;
            }
            _ => {}
        }
        let Some(s) = &mut self.construction.session else {
            return;
        };
        match a {
            C::CableType(i) => {
                if let Some(kind) = poseforge_core::CableKind::ALL.get(i as usize) {
                    s.cable.kind = *kind;
                }
            }
            C::CableCaps => s.cable.caps = !s.cable.caps,
            C::SwapEnds => {
                std::mem::swap(&mut s.cable.start, &mut s.cable.end);
                std::mem::swap(&mut s.cable.pin_start, &mut s.cable.pin_end);
            }
            C::CablePreset(i) => {
                let (compliance, bending, damping, friction) = match i {
                    0 => (0.000001, 60., 0.06, 0.5),
                    1 => (0., 1., 0.1, 0.35),
                    _ => (0.0002, 40., 0.12, 0.6),
                };
                s.physics.compliance = compliance;
                s.physics.bending = bending;
                s.physics.damping = damping;
                s.physics.friction = friction;
            }
            C::RemoveCutter(id) => {
                s.targets.retain(|&t| t != id);
            }
            C::ClearCutters => {
                s.targets.clear();
                s.pick = Some(2);
            }
            C::BooleanCleanup => s.cleanup = !s.cleanup,
            C::Mode(m) => s.mode = m.min(3),
            C::Pick(i) => {
                s.pick = Some(i);
                self.status = if i == 2 {
                    "Щёлкните объект-резак"
                } else {
                    "Щёлкните поверхность или пустое место для конца кабеля"
                }
                .into();
                return;
            }
            C::Pin(i) => {
                if i == 0 {
                    s.cable.pin_start = !s.cable.pin_start
                } else {
                    s.cable.pin_end = !s.cable.pin_end
                }
            }
            C::Collisions => s.collisions = !s.collisions,
            C::Hide => s.hide = !s.hide,
            _ => return,
        }
        self.refresh_construction();
    }
    pub(super) fn refresh_construction(&mut self) {
        let Some(mut s) = self.construction.session.take() else {
            return;
        };
        if let Some((_, cancel)) = &s.worker {
            cancel.store(true, Ordering::Relaxed);
        }
        s.error = None;
        s.ready = false;
        if s.kind == Kind::Boolean {
            if !s.targets.is_empty() {
                if s.worker.is_none() {
                    Self::start_boolean_worker(&mut s, self.window.clone());
                }
                self.status = "Булева операция · предпросмотр рассчитывается".into();
            } else {
                self.editor.reset_tool_edit();
                self.editor.selected = Some(s.owner);
                self.scene_tools.selected = s.selection.clone();
                s.error = Some("Добавьте объект-резак в сцене".into());
            }
        } else {
            let mut p = s.source.as_ref().clone();
            let existing = p.node(s.owner).is_some();
            let mesh_id = if existing { p.next_id() } else { s.owner + 2 };
            let material = if existing {
                p.node(s.owner)
                    .and_then(|n| p.mesh(n.mesh?))
                    .and_then(|m| m.material)
            } else {
                Some(s.owner + 1)
            };
            match s.cable.mesh(mesh_id, material) {
                Ok(mesh) => {
                    let mut node = p
                        .node(s.owner)
                        .cloned()
                        .unwrap_or_else(|| Node::new(s.owner, "Кабель"));
                    node.mesh = Some(mesh_id);
                    node.cable = Some(s.cable);
                    node.physics = Some(s.physics);
                    let visible = p.visible_node_ids(false);
                    node.collision_targets.cloth = Some(if s.collisions {
                        s.source
                            .node(s.owner)
                            .and_then(|n| n.collision_targets.cloth.clone())
                            .filter(|list| list.iter().any(|t| t.enabled))
                            .unwrap_or_else(|| {
                                p.nodes
                                    .iter()
                                    .filter(|n| {
                                        n.id != s.owner
                                            && n.mesh.is_some()
                                            && visible.contains(&n.id)
                                    })
                                    .take(64)
                                    .map(|n| CollisionTarget {
                                        node: n.id,
                                        enabled: true,
                                    })
                                    .collect()
                            })
                    } else {
                        vec![]
                    });
                    if existing {
                        *p.node_mut(s.owner).unwrap() = node;
                    } else {
                        p.nodes.push(node);
                        p.materials.push(Material {
                            id: s.owner + 1,
                            name: "Кабель".into(),
                            base_color: [0.12, 0.15, 0.18, 1.],
                            ..Default::default()
                        });
                    }
                    p.meshes.push(mesh);
                    match p.validate() {
                        Ok(()) => {
                            self.editor.reset_tool_edit();
                            self.editor.project = p;
                            self.editor.selected = Some(s.owner);
                            self.scene_tools.selected = BTreeSet::from([s.owner]);
                            s.ready = true;
                            self.studio.physics.reset();
                            self.invalidate_geometry();
                        }
                        Err(e) => s.error = Some(e.to_string()),
                    }
                }
                Err(e) => s.error = Some(e.to_string()),
            }
        }
        self.construction.session = Some(s);
        self.redraw();
    }
    fn start_boolean_worker(s: &mut Session, window: Option<Arc<Window>>) {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = cancel.clone();
        let source = Arc::clone(&s.source);
        let (owner, mode, offset, hide, cleanup) = (s.owner, s.mode, s.offset, s.hide, s.cleanup);
        let targets = s.targets.clone();
        let started = std::thread::Builder::new()
            .name("boolean-preview".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build_boolean(
                        &source, owner, &targets, mode, offset, hide, cleanup, &worker,
                    )
                }))
                .unwrap_or_else(|_| Err("Расчёт булевой операции прерван".into()));
                let _ = tx.send(result);
                // A stationary camera lets the event loop sleep during a tool edit.
                if let Some(window) = window {
                    window.request_redraw();
                }
            });
        match started {
            Ok(_) => s.worker = Some((rx, cancel)),
            Err(error) => s.error = Some(format!("Не удалось запустить расчёт: {error}")),
        }
    }

    pub(super) fn poll_construction(&mut self) {
        let Some(s) = self.construction.session.as_mut() else {
            return;
        };
        let Some((rx, cancel)) = &s.worker else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Расчёт прерван; измените параметр для повтора".into())
            }
        };
        let superseded = cancel.load(Ordering::Relaxed);
        s.worker = None;
        if superseded {
            // One job at a time. All slider positions received while it was
            // stopping have already been folded into the current parameters.
            if !s.targets.is_empty() {
                Self::start_boolean_worker(s, self.window.clone());
            }
            self.redraw();
            return;
        }
        match result {
            Ok(result) => {
                self.editor.project = result.project;
                self.editor.selected = Some(s.owner);
                s.outputs = result.selection;
                self.scene_tools.selected = s.outputs.clone();
                s.ready = true;
                self.status = "Предпросмотр готов · Применить или Отмена".into();
            }
            Err(e) => {
                s.error = Some(e.clone());
                self.status = e;
            }
        }
        self.invalidate_geometry();
        self.redraw();
    }
    pub(super) fn apply_construction(&mut self) {
        if self.input.is_some() && !self.resolve_input() {
            return;
        }
        if self
            .construction
            .session
            .as_ref()
            .is_none_or(|s| !s.ready || s.worker.is_some() || s.error.is_some())
        {
            return;
        }
        if matches!(self.drag, Some(Drag::Construction(_))) {
            self.drag = None;
            self.drag_button = None;
            self.release_drag_pointer();
        } else {
            self.cancel_drag();
        }
        let s = self.construction.session.take().unwrap();
        self.dropdown = None;
        let result = self.editor.commit_tool_edit();
        if self.result(result).is_some() {
            self.editor.selected = Some(s.owner);
            self.scene_tools.selected = s.outputs.clone();
            self.status = "Инструмент применён · Undo — отменить".into();
        }
        self.studio.physics_running = false;
        self.studio.physics.reset();
        self.invalidate_geometry();
        self.update_dirty();
    }
    pub(super) fn cancel_construction(&mut self) {
        if let Some(s) = self.construction.session.take() {
            self.cancel_drag();
            self.end_input();
            self.dropdown = None;
            self.editor.cancel_tool_edit();
            self.editor.selected = s.selected;
            self.scene_tools.selected = s.selection.clone();
            self.studio.physics_running = false;
            self.studio.physics.reset();
            self.invalidate_geometry();
            self.update_dirty();
        }
    }
    pub(super) fn route_construction(&mut self, a: &Action) -> bool {
        if self.construction.session.is_none() {
            return false;
        }
        if matches!(
            a,
            Action::DropdownOpen(ui::DropdownKind::CableType)
                | Action::DropdownChoose(_)
                | Action::DropdownDismiss
        ) {
            return false;
        }
        if let Action::Construction(_) = a {
            return false;
        }
        if matches!(a, Action::Cancel | Action::Undo | Action::Redo) {
            self.cancel_construction();
            self.redraw();
            return true;
        }
        if matches!(a,Action::Slide(f,..)|Action::ResetNumber(f) if matches!(**f,Action::Construction(C::Field(_))))
            || Self::preview_navigation_action(a)
        {
            return false;
        }
        if matches!(
            a,
            Action::ConfirmInput
                | Action::SurfaceTab(_)
                | Action::ToggleRightPanel
                | Action::InterfaceTheme(_)
        ) {
            return false;
        }
        self.cancel_construction();
        false
    }
    pub(super) fn construction_pointer_down(&mut self) -> bool {
        let Some(s) = &self.construction.session else {
            return false;
        };
        let Some(index) = s.pick else {
            return true;
        };
        if index == 2 {
            if let Some(e) = &s.pick_scene {
                if let Some(hit) = crate::picking::pick_surface(
                    &s.source,
                    e,
                    self.view().view_projection(),
                    self.viewport(),
                    self.mouse,
                ) {
                    if s.owner_ancestors.contains(&hit.node) {
                        self.status = "Родитель исходного объекта не может быть резаком".into();
                        return true;
                    }
                    if hit.node != s.owner && self.boolean_candidate(hit.node) {
                        let s = self.construction.session.as_mut().unwrap();
                        if !s.targets.contains(&hit.node) {
                            s.targets.push(hit.node);
                        }
                        s.pick = None;
                        self.refresh_construction();
                    }
                }
            }
            return true;
        }
        let radius = s.cable.collision_radius();
        let p = self
            .surface_anchor()
            .map(|(p, n)| p + n * radius)
            .or_else(|| self.construction_point(self.target, Vec3::Y))
            .or_else(|| {
                crate::gizmo::pointer_ray(&self.view(), self.viewport(), self.mouse)
                    .and_then(|(_, ray)| self.construction_point(self.target, -ray))
            });
        if let Some(mut p) = p {
            let s = self.construction.session.as_mut().unwrap();
            let model = s
                .source
                .global_transforms()
                .ok()
                .and_then(|g| g.get(&s.owner).copied())
                .unwrap_or(Mat4::IDENTITY);
            p = model.inverse().transform_point3(p);
            if index == 0 {
                s.cable.start = p.to_array();
                s.pick = Some(1);
            } else {
                s.cable.end = p.to_array();
                s.pick = None;
            }
            self.refresh_construction();
        }
        true
    }
    pub(super) fn update_construction_gesture(&mut self, g: Gesture) {
        match &g {
            Gesture::Scroll {
                start,
                value,
                scale,
                maximum,
            } => {
                self.construction.scroll =
                    (value + (self.mouse[1] - start) * scale).clamp(0., *maximum)
            }
            Gesture::Panel { start, origin, .. } => {
                self.construction.position = Some([
                    origin[0] + self.mouse[0] - start[0],
                    origin[1] + self.mouse[1] - start[1],
                ])
            }
            Gesture::End {
                index,
                origin,
                normal,
                ..
            } => {
                if let Some(p) = self.construction_point(*origin, *normal) {
                    if let Some(s) = &mut self.construction.session {
                        let m = s
                            .source
                            .global_transforms()
                            .ok()
                            .and_then(|g| g.get(&s.owner).copied())
                            .unwrap_or(Mat4::IDENTITY);
                        let p = m.inverse().transform_point3(p).to_array();
                        if *index == 0 {
                            s.cable.start = p;
                        } else {
                            s.cable.end = p;
                        }
                        self.refresh_construction();
                    }
                }
            }
        }
        self.drag = Some(Drag::Construction(g));
    }
    pub(super) fn cancel_construction_gesture(&mut self, g: &Gesture) {
        match g {
            Gesture::Scroll { value, .. } => self.construction.scroll = *value,
            Gesture::Panel { original, .. } => self.construction.position = *original,
            Gesture::End { original, .. } => {
                if let Some(s) = &mut self.construction.session {
                    s.cable = *original;
                    self.refresh_construction();
                }
            }
        }
    }
    pub(super) fn construction_field(&self, i: u8) -> Option<(f32, f32, f32, &'static str)> {
        let s = self.construction.session.as_ref()?;
        Some(match i {
            0..=2 => (
                s.cable.start[i as usize],
                -10000.,
                10000.,
                ["Начало X, м", "Начало Y, м", "Начало Z, м"][i as usize],
            ),
            3..=5 => (
                s.cable.end[(i - 3) as usize],
                -10000.,
                10000.,
                ["Конец X, м", "Конец Y, м", "Конец Z, м"][(i - 3) as usize],
            ),
            6 => (s.cable.radius, 0.001, 0.5, "Радиус, м"),
            7 => (s.cable.segments as f32, 4., 256., "Сегменты"),
            8 => (s.cable.sag * 100., 0., 200., "Провисание, %"),
            9 => (s.physics.gravity, 0., 30., "Гравитация"),
            10..=12 => (
                s.offset[(i - 10) as usize],
                -10000.,
                10000.,
                [
                    "Сдвиг резаков X, м",
                    "Сдвиг резаков Y, м",
                    "Сдвиг резаков Z, м",
                ][(i - 10) as usize],
            ),
            _ => return self.cable_extended_field(i),
        })
    }
    pub(super) fn construction_confirm(&mut self, a: &Action, text: &str) -> Result<bool, String> {
        let Action::Construction(C::Field(i)) = *a else {
            return Ok(false);
        };
        let (_, min, max, _) = self.construction_field(i).ok_or("Инструмент закрыт")?;
        let v = surfaces::number(text, min, max)?;
        if matches!(i, 7 | 13 | 15 | 18 | 30) && v.fract() != 0. {
            return Err("Введите целое число".into());
        }
        let s = self.construction.session.as_mut().unwrap();
        match i {
            0..=2 => s.cable.start[i as usize] = v,
            3..=5 => s.cable.end[(i - 3) as usize] = v,
            6 => s.cable.radius = v,
            7 => s.cable.segments = v as u32,
            8 => s.cable.sag = v / 100.,
            9 => s.physics.gravity = v,
            10..=12 => s.offset[(i - 10) as usize] = v,
            _ => cable_controls::set_field(s, i, v),
        }
        self.refresh_construction();
        Ok(true)
    }
    pub(super) fn construction_rect(&self) -> Option<[f32; 4]> {
        let s = self.construction.session.as_ref()?;
        let area = self.modeling_area();
        if area[2] < 180. || area[3] < 100. {
            return None;
        }
        let full = if s.kind == Kind::Cable {
            604_f32
        } else {
            (self.construction_content_height() + 94.).min(640.)
        };
        let w = 300_f32.min(area[2]);
        let mut h = if self.construction.collapsed {
            40.
        } else {
            full.min(area[3])
        };
        if !self.construction.collapsed && area[2] < w + 164. {
            h = h.min((area[3] * 0.55).max(160.).min(area[3]));
        }
        let [x, y] = self
            .construction
            .position
            .unwrap_or([area[0], area[1] + area[3] - h]);
        Some([
            x.clamp(area[0], area[0] + area[2] - w),
            y.clamp(area[1], area[1] + area[3] - h),
            w,
            h,
        ])
    }
    pub(super) fn construction_scroll(&mut self, delta: f32) -> bool {
        if !delta.is_finite()
            || self.construction.collapsed
            || self.drag.is_some()
            || self.has_modal()
        {
            return false;
        }
        if let Some(r) = self
            .construction_rect()
            .filter(|r| ui::contains(*r, self.mouse))
        {
            if self.input.is_some() && !self.resolve_input() {
                return true;
            }
            let total = self.construction_content_height();
            self.construction.scroll = (self.construction.scroll - delta * 34.).clamp(
                0.,
                (total - (r[3] - 94. - self.construction_tabs_height())).max(0.),
            );
            self.redraw();
            true
        } else {
            false
        }
    }
    pub(super) fn draw_construction(&self, u: &mut Ui) {
        let Some(s) = &self.construction.session else {
            return;
        };
        let Some([x, y, w, h]) = self.construction_rect() else {
            return;
        };
        let p = u.palette();
        if s.kind == Kind::Cable {
            let model = self
                .editor
                .project
                .global_transforms()
                .ok()
                .and_then(|g| g.get(&s.owner).copied())
                .unwrap_or(Mat4::IDENTITY);
            for (i, point) in [s.cable.start, s.cable.end].into_iter().enumerate() {
                if let Some(point) =
                    self.project_point(model.transform_point3(Vec3::from_array(point)))
                {
                    u.quiet_icon_button(
                        [point[0] - 9., point[1] - 9., 18., 18.],
                        "",
                        Action::Construction(C::Handle(i as u8)),
                        true,
                        Icon::Vertex,
                    );
                }
            }
        }
        u.floating_surface(
            [x, y, w, h],
            1. - self.preferences.tool_panel_transparency,
            true,
        );
        u.hits.push(ui::Hit {
            rect: [x, y, w, h],
            action: Action::Construction(C::Backdrop),
        });
        u.quiet_icon_button(
            [x + 8., y + 4., w - 48., 30.],
            if s.kind == Kind::Cable {
                "Кабель"
            } else {
                "Булева операция"
            },
            Action::Construction(C::Panel),
            false,
            if s.kind == Kind::Cable {
                Icon::Cable
            } else {
                Icon::Boolean
            },
        );
        u.quiet_icon_button(
            [x + w - 36., y + 4., 28., 28.],
            "",
            Action::Construction(C::Collapse),
            false,
            Icon::ChevronDown,
        );
        if self.construction.collapsed {
            return;
        }
        let total = self.construction_content_height();
        let tabs = self.construction_tabs_height();
        if tabs > 0. {
            let cell = (w - 36.) / 3.;
            for (i, label) in ["Форма", "Концы", "Физика"].into_iter().enumerate() {
                u.quiet_button(
                    [x + 14. + i as f32 * (cell + 4.), y + 40., cell, 28.],
                    label,
                    Action::Construction(C::CableTab(i as u8)),
                    self.construction.cable_tab == i as u8,
                );
            }
        }
        let body = [x + 14., y + 40. + tabs, w - 38., h - 94. - tabs];
        let maximum = (total - body[3]).max(0.);
        let scroll = self.construction.scroll.min(maximum);
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        u.set_cursor([self.mouse[0], self.mouse[1] + scroll]);
        let left = body[0];
        let width = body[2];
        let mut row = body[1];
        if s.kind == Kind::Boolean {
            let cell = (width - 6.) * 0.5;
            for (i, label) in ["Объединение", "Вычитание", "Пересечение", "Разделить"]
                .into_iter()
                .enumerate()
            {
                u.quiet_button(
                    [
                        left + (i % 2) as f32 * (cell + 6.),
                        row + (i / 2) as f32 * 32.,
                        cell,
                        28.,
                    ],
                    label,
                    Action::Construction(C::Mode(i as u8)),
                    s.mode == i as u8,
                );
            }
            row += 68.;
            u.quiet_button(
                [left, row, width, 30.],
                "Добавить резак в сцене",
                Action::Construction(C::Pick(2)),
                s.pick == Some(2),
            );
            row += 36.;
            let first = ((body[1] + scroll - row) / 32.).floor().max(0.) as usize;
            let end = (((body[1] + scroll + body[3] - row) / 32.).ceil().max(0.) as usize)
                .min(s.targets.len());
            let list_y = row;
            for index in first.min(end)..end {
                let id = s.targets[index];
                let row = list_y + index as f32 * 32.;
                let name = s.source.node(id).map_or("Недоступен", |n| n.name.as_str());
                u.fitted_label([left + 6., row + 7., width - 38., 20.], name, 11., p.text);
                u.quiet_icon_button(
                    [left + width - 28., row, 28., 28.],
                    "",
                    Action::Construction(C::RemoveCutter(id)),
                    false,
                    Icon::Close,
                );
            }
            row += s.targets.len() as f32 * 32.;
            if !s.targets.is_empty() {
                u.quiet_button(
                    [left, row, width, 26.],
                    "Очистить список резаков",
                    Action::Construction(C::ClearCutters),
                    false,
                );
                row += 32.;
            }
            for i in 10..=12 {
                let (v, _, _, label) = self.construction_field(i).unwrap();
                u.number_field(
                    [left, row, width, 38.],
                    label,
                    v as f64,
                    Action::Construction(C::Field(i)),
                    SliderRange::new(-10., 10., 0.01),
                );
                row += 42.;
            }
            u.checkbox(
                [left, row, width, 26.],
                "Убирать плоские рёбра",
                Action::Construction(C::BooleanCleanup),
                s.cleanup,
            );
            row += 32.;
            u.checkbox(
                [left, row, width, 26.],
                "Скрыть резаки",
                Action::Construction(C::Hide),
                s.hide,
            );
            row += 32.;
            u.fitted_label(
                [left, row, width, 20.],
                if s.mode == 3 {
                    "Обе части станут отдельными объектами"
                } else {
                    "Резаки объединяются перед пересечением"
                },
                10.,
                p.muted,
            );
        } else {
            self.draw_cable_controls(u, left, row, width);
        }
        clip_scrolled(u, start, body, scroll);
        u.set_cursor(self.mouse);
        if maximum > 0. {
            u.vertical_scrollbar(
                [x + w - 18., body[1], 10., body[3]],
                scroll / maximum,
                body[3] / total,
                Action::Construction(C::Scroll),
            );
        }
        if let Some(error) = &s.error {
            u.fitted_label([x + 14., y + h - 51., w - 28., 17.], error, 10., p.muted);
        } else if s.worker.is_some() {
            u.fitted_label(
                [x + 14., y + h - 51., w - 28., 17.],
                "Расчёт… можно вращать вид",
                10.,
                p.muted,
            );
        }
        let half = (w - 34.) / 2.;
        if s.ready && s.worker.is_none() && s.error.is_none() {
            u.button(
                [x + 14., y + h - 34., half, 28.],
                "Применить",
                Action::Construction(C::Apply),
                true,
            );
        } else {
            u.disabled_button([x + 14., y + h - 34., half, 28.], "Применить");
        }
        u.quiet_button(
            [x + 20. + half, y + h - 34., half, 28.],
            "Отмена · Esc",
            Action::Construction(C::Cancel),
            false,
        );
    }
}
