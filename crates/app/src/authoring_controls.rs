//! Mesh component tools and skeleton authoring use the editor's normal transactions.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S, Ui};
use poseforge_core::{MeshEdit, SkeletonTemplate};

pub(super) struct AuthoringState {
    pub workflow: super::modeling_workflow::State,
    pub snapping:super::snapping::State,
    pub modeling_tool: Option<ui::ModelingTool>,
    pub knife: Option<advanced_tools::KnifeState>,
    pub proportional: bool,
    pub proportional_radius: f32,
    pub editing: Option<u64>,
    pub selection_tool: u8,
    pub last_click: Option<(Instant, [f32; 2], S)>,
    pub weld_distance: f32,
    pub bridge_segments: u32,
    pub bridge_twist: i32,
    pub bridge_smooth: f32,
    pub preview_modifiers: bool,
    pub repair_kind:u8,
    pub knife_snap:bool,
    pub knife_angle:u8,
    pub diagnostics: Option<((u64, u64, u64, u64), poseforge_core::TopologyReport)>,
    pub selection_mode: S,
    pub vertices: BTreeSet<usize>,
    pub edges: BTreeSet<(usize, usize)>,
    pub faces: BTreeSet<usize>,
    pub skeleton: Option<u64>,
    pub rig_stage: u8,
    pub bind_target: Option<u64>,
    pub reference_pick: Option<ui::RigReference>,
    pub xray: bool,
    pub pivot_origin: bool,
    pub constraint: Option<usize>,
    pub inset: f32,
    pub bevel_height: f32,
    pub loop_position: f32,
    pub cache: std::cell::RefCell<Option<Arc<super::mesh_pick::MeshCache>>>,
}
impl Default for AuthoringState {
    fn default() -> Self {
        Self {
            workflow: Default::default(),
            snapping:Default::default(),
            modeling_tool: None,
            knife: None,
            proportional: false,
            proportional_radius: 1.,
            editing: None,
            selection_tool: 0,
            last_click: None,
            weld_distance: 0.0001,
            bridge_segments: 1,
            bridge_twist:0,
            bridge_smooth:0.,
            preview_modifiers:true,
            repair_kind:0,
            knife_snap:true,
            knife_angle:0,
            diagnostics: None,
            selection_mode: S::default(),
            vertices: BTreeSet::new(),
            edges: BTreeSet::new(),
            faces: BTreeSet::new(),
            skeleton: None,
            rig_stage: 0,
            bind_target: None,
            reference_pick: None,
            xray: false,
            pivot_origin: false,
            constraint: None,
            inset: 0.15,
            bevel_height: 0.1,
            loop_position: 0.5,
            cache: Default::default(),
        }
    }
}

/// One geometric edge across duplicated UV/normal vertices, matching the
/// core editor's position-welded translation policy. Only authored polygon
/// boundaries are selectable; legacy triangle meshes retain their edges.
#[cfg(test)]
pub(super) fn mesh_edges(mesh: &poseforge_core::Mesh) -> BTreeSet<(usize, usize)> {
    poseforge_core::MeshConnectivity::new(mesh).edges
}
#[derive(Clone)]
pub(super) struct MeshDrag {
    weights: Option<Vec<f32>>,
    node: u64,
    vertices: BTreeSet<usize>,
    inverse_model: Mat4,
    inverse_view: Mat4,
    viewport: [f32; 4],
    anchor: Vec3,
    normal: Vec3,
    previous: Vec3,
    accumulated: Vec3,
    applied: Mat4,
    original: poseforge_core::Mesh,
    tool: Tool,
    pivot: Vec3,
    axis: Option<usize>,
    axis_world: Vec3,
    pointer: [f32; 2],
    amount: f32,
    fine: bool,
}
pub(super) fn ray(inverse: Mat4, viewport: [f32; 4], pointer: [f32; 2]) -> Option<(Vec3, Vec3)> {
    let x = (pointer[0] - viewport[0]) / viewport[2] * 2. - 1.;
    let y = 1. - (pointer[1] - viewport[1]) / viewport[3] * 2.;
    let a = inverse.project_point3(Vec3::new(x, y, 0.));
    let b = inverse.project_point3(Vec3::new(x, y, 0.5));
    Some((a, (b - a).try_normalize()?))
}
pub(super) fn project(matrix: Mat4, viewport: [f32; 4], point: Vec3) -> Option<[f32; 2]> {
    let clip = matrix * point.extend(1.);
    if clip.w <= 0. {
        return None;
    }
    let q = clip.truncate() / clip.w;
    if !(0.0..=1.0).contains(&q.z) {
        return None;
    }
    Some([
        viewport[0] + (q.x * 0.5 + 0.5) * viewport[2],
        viewport[1] + (0.5 - q.y * 0.5) * viewport[3],
    ])
}
fn plane(
    inverse: Mat4,
    viewport: [f32; 4],
    pointer: [f32; 2],
    anchor: Vec3,
    normal: Vec3,
) -> Option<Vec3> {
    let (origin, direction) = ray(inverse, viewport, pointer)?;
    let denominator = direction.dot(normal);
    if denominator.abs() < 1e-6 {
        return None;
    }
    let point = origin + direction * ((anchor - origin).dot(normal) / denominator);
    point.is_finite().then_some(point)
}

impl App {
    pub(super) fn authoring_prune(&mut self) {
        if self
            .authoring
            .skeleton
            .is_some_and(|id| self.editor.project.node(id).is_none())
        {
            self.authoring.skeleton = None;
        }
        if self.authoring.bind_target.is_some_and(|id| {
            self.editor
                .project
                .node(id)
                .is_none_or(|n| n.mesh.is_none())
        }) {
            self.authoring.bind_target = None;
        }
        if self.authoring.editing.is_some_and(|id| {
            self.editor.selected != Some(id)
                || !self.node_selectable(id)
                || self.editor.mode != EditorMode::Scene
                || self.editor.editable_mesh(id).is_err()
        }) {
            self.authoring.editing = None;
            self.authoring.modeling_tool = None;
            self.authoring.vertices.clear();
            self.authoring.edges.clear();
            self.authoring.faces.clear();
        }
        if let Some(mesh) = self
            .authoring
            .editing
            .and_then(|id| self.editor.editable_mesh(id).ok())
        {
            self.authoring.vertices.retain(|i| *i < mesh.vertices.len());
            let cache = self.mesh_cache(mesh);
            self.authoring
                .edges
                .retain(|e| cache.graph.edges.contains(e));
            self.authoring.faces.retain(|i| *i < mesh.face_count());
        }
    }
    pub(super) fn edit_vertices(&self) -> BTreeSet<usize> {
        if self.authoring.selection_mode == S::Vertex {
            return self.authoring.vertices.clone();
        }
        if self.authoring.selection_mode == S::Edge {
            return self
                .authoring
                .edges
                .iter()
                .flat_map(|&(a, b)| [a, b])
                .collect();
        }
        self.authoring
            .editing
            .and_then(|id| self.editor.editable_mesh(id).ok())
            .map(|mesh| {
                self.authoring
                    .faces
                    .iter()
                    .filter_map(|&i| mesh.face_vertices(i))
                    .flatten()
                    .map(|i| *i as usize)
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(super) fn edit_center(&self) -> Option<Vec3> {
        let mesh = self.editor.editable_mesh(self.authoring.editing?).ok()?;
        let vertices = self.edit_vertices();
        if vertices.is_empty() {
            return None;
        }
        let canonical = &self.mesh_cache(mesh).graph.canonical;
        let unique: BTreeSet<_> = vertices.iter().map(|&i| canonical[i]).collect();
        Some(
            unique
                .iter()
                .map(|&i| Vec3::from_array(mesh.vertices[i].position))
                .sum::<Vec3>()
                / unique.len() as f32,
        )
    }
    pub(super) fn skeleton_roots(&self) -> Vec<u64> {
        rig_references::roots(&self.editor.project)
    }
    pub(super) fn frame_edit_mesh(&mut self) -> bool {
        let Some(id) = self.authoring.editing else {
            return false;
        };
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            return false;
        };
        let Ok(evaluated) = self.evaluated_interactive() else {
            return false;
        };
        let Some(model) = evaluated.global.get(&id) else {
            return false;
        };
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in &mesh.vertices {
            let point = model.transform_point3(Vec3::from_array(vertex.position));
            min = min.min(point);
            max = max.max(point);
        }
        if !min.is_finite() || !max.is_finite() {
            return false;
        }
        self.leave_camera_view();
        let view = self.view();
        let aspect = (view.viewport[2] / view.viewport[3].max(1.)).max(0.01);
        let radius = ((max - min).length() * 0.5).max(0.05) * 1.15;
        self.target = (min + max) * 0.5;
        self.distance = if self.orthographic {
            radius / aspect.min(1.) / 0.42
        } else {
            let half_fov = (view.fov_y * 0.5).tan();
            radius / (half_fov * aspect.min(1.)).atan().sin().max(0.001)
        };
        true
    }
    pub(super) fn rig_target(&self) -> Option<u64> {
        self.editor
            .selected
            .filter(|id| {
                self.editor
                    .project
                    .node(*id)
                    .is_some_and(|n| n.mesh.is_some())
            })
            .or(self.authoring.bind_target.filter(|id| {
                self.editor
                    .project
                    .node(*id)
                    .is_some_and(|n| n.mesh.is_some())
            }))
    }
    pub(super) fn authoring_action(&mut self, command: A) {
        if self.job.is_some() || self.scene_write_pending() || self.editor.playing {
            return;
        }
        if self.drag.is_some() {
            self.cancel_drag();
        }
        self.authoring_prune();
        self.remember_modeling_tool(&Action::Author(command));
        if matches!(command, A::Smooth | A::Flat) {
            self.normals_action(if command == A::Smooth {
                ui::NormalAction::Smooth
            } else {
                ui::NormalAction::Flat
            });
            return;
        }
        if self.weight_action(command) {
            return;
        }
        if self.workflow_action(command) || self.advanced_action(command)
            || self.topology_action(command)
            || self.modeling_live_action(command)
            || self.modeling_action(command)
        {
            self.update_dirty();
            self.redraw();
            return;
        }
        match command {
            A::SnapMode(_) | A::SnapStep | A::ModifierPreview | A::BridgeTwist | A::BridgeSmooth | A::FillHoles | A::Triangulate | A::Separate | A::ExtrudeIndividual | A::RepairMode(_) | A::SelectIssue | A::RepairIssue | A::KnifeSnap | A::KnifeAngle(_)
            | A::Knife
            | A::KnifeAccept
            | A::KnifeCancel
            | A::Slide
            | A::Proportional
            | A::ProportionalRadius
            | A::SelectTool(_)
            | A::EdgeLoop
            | A::EdgeRing
            | A::Merge
            | A::Weld
            | A::WeldDistance
            | A::Bridge
            | A::BridgeFaces
            | A::ToolField(_) | A::ToolArray | A::ExtrudeMode(_) | A::ToolPanelDrag | A::ToolPanelToggle
            | A::BridgeSegments
            | A::Dissolve
            | A::Inspect
            | A::SelectDefects
            | A::EdgeBevel
            | A::LiveField(_)
            | A::LiveDrag(_)
            | A::LiveAccept
            | A::LiveCancel
            | A::LiveBackdrop
            | A::LiveToggle
            | A::Xray
            | A::PivotOrigin
            | A::Constrain(_)
            | A::Invert
            | A::Grow
            | A::Shrink
            | A::Connected
            | A::RotateDegrees(_)
            | A::ScaleFactor(_)
            | A::InsetAmount
            | A::BevelHeight
            | A::Bevel
            | A::Inset
            | A::LoopPosition
            | A::LoopCut => unreachable!(),
            A::Paint | A::PaintJoint(_) | A::PaintRadius | A::PaintStrength => {}
            A::ToggleEdit => {
                self.weights.target = None;
                self.authoring.modeling_tool = None;
                if self.authoring.editing.take().is_some() {
                    self.authoring.vertices.clear();
                    self.authoring.edges.clear();
                    self.authoring.faces.clear();
                    self.status = "Сцена · выбор объектов".into();
                    return;
                }
                if self.editor.has_staged() {
                    self.status = "Сначала поставьте ключ или отбросьте временную позу.".into();
                    return;
                }
                let Some(id) = self.editor.selected else {
                    self.status = "Выберите меш в сцене".into();
                    return;
                };
                if !self.node_selectable(id) {
                    self.status = "Объект скрыт или заблокирован".into();
                    return;
                }
                if let Err(e) = self.editor.editable_mesh(id) {
                    self.status = e.to_string();
                    return;
                }
                self.editor.mode = EditorMode::Scene;
                self.authoring.editing = Some(id);
                self.authoring.vertices.clear();
                self.authoring.edges.clear();
                self.authoring.faces.clear();
                self.tool = Tool::Move;
                self.studio.physics_running = false;
                self.studio.physics.reset();
                if !matches!(self.surface.tab, 1 | 10) {
                    self.set_properties_tab(7);
                }
                self.preferences.right_panel = true;
                self.status =
                    "Сетка: щелчок — выбор, Shift — добавить; тяните выделение. F2 — завершить."
                        .into();
            }
            A::SelectMode(mode) => {
                if self.authoring.editing.is_none() {
                    self.authoring_action(A::ToggleEdit);
                }
                let Some(mesh) = self
                    .authoring
                    .editing
                    .and_then(|id| self.editor.editable_mesh(id).ok())
                else {
                    return;
                };
                if mode != self.authoring.selection_mode {
                    let keys: BTreeSet<_> = self
                        .edit_vertices()
                        .into_iter()
                        .map(|i| poseforge_core::position_key(mesh.vertices[i].position))
                        .collect();
                    let selected = |i: usize| {
                        keys.contains(&poseforge_core::position_key(mesh.vertices[i].position))
                    };
                    self.authoring.vertices =
                        (0..mesh.vertices.len()).filter(|&i| selected(i)).collect();
                    self.authoring.edges = self
                        .mesh_cache(mesh)
                        .graph
                        .edges
                        .iter()
                        .copied()
                        .filter(|&(a, b)| selected(a) && selected(b))
                        .collect();
                    self.authoring.faces = (0..mesh.face_count())
                        .filter(|&i| {
                            mesh.face_vertices(i)
                                .unwrap()
                                .iter()
                                .all(|&v| selected(v as usize))
                        })
                        .collect();
                    self.authoring.selection_mode = mode;
                }
                self.tool = match self.authoring.modeling_tool {
                    Some(ui::ModelingTool::Rotate) => Tool::Rotate,
                    Some(ui::ModelingTool::Scale) => Tool::Scale,
                    _ => Tool::Move,
                };
                self.status = match mode {
                    S::Vertex => "Вершины · 1 · Shift: добавить · потяните выделение",
                    S::Edge => "Рёбра · 2 · Shift: добавить · потяните выделение",
                    S::Face => "Грани · 3 · Shift: добавить · E: экструдировать",
                }
                .into();
            }
            A::SelectAll => {
                if let Some(mesh) = self
                    .authoring
                    .editing
                    .and_then(|id| self.editor.editable_mesh(id).ok())
                {
                    self.authoring.vertices = (0..mesh.vertices.len()).collect();
                    self.authoring.edges = self.mesh_cache(mesh).graph.edges.clone();
                    self.authoring.faces = (0..mesh.face_count()).collect();
                }
            }
            A::Clear => {
                self.authoring.vertices.clear();
                self.authoring.edges.clear();
                self.authoring.faces.clear();
            }
            A::Coordinate(axis) => {
                if let Some(center) = self.edit_center() {
                    self.input(
                        Action::Author(command),
                        format!("{:.6}", center[axis.min(2) as usize]),
                    );
                }
            }
            A::Extrude => unreachable!("live operation handled above"),
            A::DeleteFaces | A::Subdivide | A::Smooth | A::Flat => {
                let Some(id) = self.authoring.editing else {
                    return;
                };
                let op = match command {
                    A::DeleteFaces | A::Subdivide => {
                        let vertices=if self.authoring.selection_mode==S::Vertex {self.authoring.vertices.clone()} else {BTreeSet::new()};
                        let edges=if self.authoring.selection_mode==S::Edge {self.authoring.edges.clone()} else {BTreeSet::new()};
                        let faces=if self.authoring.selection_mode==S::Face {self.authoring.faces.clone()} else {BTreeSet::new()};
                        if command==A::DeleteFaces {MeshEdit::DeleteComponents {vertices,edges,faces}}
                        else {MeshEdit::SubdivideSelected {vertices,edges,faces}}
                    },
                    A::Smooth => MeshEdit::SmoothNormals,
                    _ => MeshEdit::FlatNormals,
                };
                let result = self.editor.edit_mesh(id, op);
                if self.result(result).is_some() {
                    self.authoring.vertices.clear();
                    self.authoring.edges.clear();
                    self.authoring.faces.clear();
                    self.invalidate_geometry();
                }
            }
            A::CreateSkeleton(_) | A::CreateBone => {
                let target = if command == A::CreateBone { None } else { self.rig_target() };
                let result = self.editor.create_skeleton(
                    match command {
                        A::CreateBone => SkeletonTemplate::Bone,
                        A::CreateSkeleton(true) => SkeletonTemplate::Humanoid,
                        _ => SkeletonTemplate::Chain,
                    },
                    target,
                );
                if let Some(root) = self.result(result) {
                    self.authoring.skeleton = Some(root);
                    self.authoring.bind_target = target;
                    self.active_rig_root = Some(root);
                    self.editor.mode = EditorMode::Scene;
                    self.preferences.skeleton = true;
                    self.preferences.left_panel = true;
                    self.preferences.right_panel = true;
                    self.authoring.editing = None;
                    self.authoring.modeling_tool = None;
                    self.authoring.vertices.clear();
                    self.authoring.edges.clear();
                    self.authoring.faces.clear();
                    self.authoring.rig_stage = 0;
                    self.weights.target = None;
                    self.editor.selected = Some(root);
                    self.scene_tools.selected = self.skeleton_object_selection(root).1;
                    self.scene_tools.collection = None;
                    self.ik = false;
                    self.tool = Tool::Move;
                    self.set_properties_tab(0);
                    self.inspector_scroll = 0.;
                    self.reveal_scene_node(root);
                    self.sync_rig_pose();
                    self.invalidate();
                    self.status = "Скелет создан. Его параметры, привязка и поза находятся в свойствах объекта.".into();
                }
            }
            A::ExtendJoint => {
                self.input(Action::Author(A::ExtendJoint), "0.25".into());
            }
            A::Bind => {
                if let (Some(mesh), Some(root)) = (self.rig_target(), self.authoring.skeleton) {
                    if let Err(e) = self
                        .editor
                        .authoring_unlocked(mesh)
                        .and_then(|_| self.editor.authoring_unlocked(root))
                    {
                        self.status = e.to_string();
                        return;
                    }
                    let before = self.editor.project.clone();
                    let (sender, receiver) = mpsc::channel();
                    let cancel = Arc::new(AtomicBool::new(false));
                    let worker_cancel = cancel.clone();
                    self.editor.playing = false;
                    self.studio.physics_running = false;
                    std::thread::spawn(move || {
                        let mut editor = Editor::new(before.clone());
                        let result = editor.bind_mesh_cancellable(mesh, root, || {
                            worker_cancel.load(Ordering::Relaxed)
                        });
                        let message = match result {
                            Ok(()) => JobMessage::Bound(
                                Box::new(before),
                                Box::new(editor.project),
                                mesh,
                                root,
                            ),
                            Err(e) => JobMessage::Error(e.to_string()),
                        };
                        let _ = sender.send(message);
                    });
                    self.job = Some(Job {
                        receiver,
                        cancel,
                        label: "Привязка скелета".into(),
                    });
                    self.status = "Расчёт весов привязки… Esc — отмена".into();
                } else {
                    self.status = "Выберите меш и скелет".into();
                }
            }
            A::Unbind => {
                if let Some(mesh) = self.rig_target() {
                    let r = self.editor.unbind_mesh(mesh);
                    if self.result(r).is_some() {
                        self.invalidate();
                    }
                }
            }
        }
        self.update_dirty();
        self.redraw();
    }
    pub(super) fn authoring_confirm(
        &mut self,
        action: &Action,
        text: &str,
    ) -> Result<bool, String> {
        if matches!(action,Action::Author(A::SnapStep)) {self.authoring.snapping.step=surfaces::number(text,0.000001,10000.)?;return Ok(true);}
        if self.workflow_confirm(action, text)? || self.topology_confirm(action, text)?
            || self.modeling_live_confirm(action, text)?
            || self.modeling_confirm(action, text)?
        {
            return Ok(true);
        }
        let Action::Author(command) = *action else {
            return Ok(false);
        };
        let value = surfaces::number(text, -10000., 10000.)?;
        match command {
            A::PaintRadius => {
                self.weights.radius = surfaces::number(text, 4., 256.)?;
                return Ok(true);
            }
            A::PaintStrength => {
                self.weights.strength = surfaces::number(text, 0.01, 1.)?;
                return Ok(true);
            }
            A::Coordinate(axis) => {
                let center = self.edit_center().ok_or("Нет выделенных вершин")?;
                let mut delta = Vec3::ZERO;
                delta[axis.min(2) as usize] = value - center[axis.min(2) as usize];
                self.editor
                    .edit_mesh(
                        self.authoring.editing.ok_or("Режим сетки закрыт")?,
                        MeshEdit::Translate {
                            vertices: self.edit_vertices(),
                            delta,
                        },
                    )
                    .map_err(|e| e.to_string())?;
            }
            A::ExtendJoint => {
                if !(0.001..=1000.).contains(&value) {
                    return Err("Длина кости: 0,001–1000".into());
                }
                let id = self
                    .editor
                    .extend_joint(
                        self.editor.selected.ok_or("Выберите родительскую кость")?,
                        Vec3::Y * value,
                    )
                    .map_err(|e| e.to_string())?;
                self.editor.selected = Some(id);
                self.set_properties_tab(0);
                self.focus_rig_settings();
            }
            _ => return Ok(false),
        }
        self.invalidate_geometry();
        Ok(true)
    }
    pub(super) fn begin_mesh_drag(&mut self) {
        let Some(node) = self.authoring.editing else {
            return;
        };
        let Some(center) = self.edit_center() else {
            return;
        };
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        let model = globals[&node];
        let view = self.view();
        let anchor = model.transform_point3(center);
        let normal = (view.target - view.eye).normalize();
        let inverse = view.view_projection().inverse();
        let viewport = self.viewport();
        let Some(previous) = plane(inverse, viewport, self.mouse, anchor, normal) else {
            return;
        };
        let original = self.editor.editable_mesh(node).unwrap().clone();
        let pivot = if self.authoring.pivot_origin {
            Vec3::ZERO
        } else {
            center
        };
        let axis = self.authoring.constraint;
        let axis_world = axis.map_or(normal, |i| {
            model
                .transform_vector3([Vec3::X, Vec3::Y, Vec3::Z][i])
                .normalize()
        });
        self.begin_snapping(true);
        self.editor.begin_edit(match self.tool {
            Tool::Rotate => "Повернуть компоненты",
            Tool::Scale => "Масштаб компонентов",
            _ => "Переместить компоненты",
        });
        let weights = self.authoring.proportional.then(|| {
            poseforge_core::proportional_weights(
                &original,
                &self.edit_vertices(),
                self.authoring.proportional_radius,
                model,
            )
        });
        self.drag = Some(Drag::Mesh(MeshDrag {
            weights,
            node,
            vertices: self.edit_vertices(),
            inverse_model: model.inverse(),
            inverse_view: inverse,
            viewport,
            anchor,
            normal,
            previous,
            accumulated: Vec3::ZERO,
            applied: Mat4::IDENTITY,
            original,
            tool: self.tool,
            pivot,
            axis,
            axis_world,
            pointer: self.mouse,
            amount: 0.,
            fine: self.modifiers.shift_key(),
        }));
        self.drag_button = Some(MouseButton::Left);
    }
    pub(super) fn update_mesh_drag(&mut self, mut drag: MeshDrag, pointer: [f32; 2]) {
        if self
            .authoring
            .last_click
            .is_some_and(|(_, p, _)| (p[0] - pointer[0]).hypot(p[1] - pointer[1]) > 3.)
        {
            self.authoring.last_click = None;
        }
        let fine = self.modifiers.shift_key();
        if fine != drag.fine {
            drag.fine = fine;
            drag.pointer = pointer;
            if let Some(hit) = plane(
                drag.inverse_view,
                drag.viewport,
                pointer,
                drag.anchor,
                drag.normal,
            ) {
                drag.previous = hit;
            }
            self.drag = Some(Drag::Mesh(drag));
            return;
        }
        if let Some(hit) = plane(
            drag.inverse_view,
            drag.viewport,
            pointer,
            drag.anchor,
            drag.normal,
        ) {
            drag.accumulated +=
                (hit - drag.previous) * if self.modifiers.shift_key() { 0.1 } else { 1. };
            drag.previous = hit;
            let mut delta = drag.inverse_model.transform_vector3(drag.accumulated);
            if let Some(axis) = drag.axis {
                let value = delta[axis];
                delta = Vec3::ZERO;
                delta[axis] = value;
            }
            if self.modifiers.control_key() && self.authoring.snapping.mode==0 {
                delta = (delta * 10.).round() / 10.;
            }
            if drag.tool==Tool::Move {
                let world=drag.anchor+drag.inverse_model.inverse().transform_vector3(delta);
                delta=drag.inverse_model.transform_vector3(self.snap_translation(world)-drag.anchor);
                if let Some(axis)=drag.axis {let value=delta[axis];delta=Vec3::ZERO;delta[axis]=value;}
            }
            drag.amount += (pointer[0] - drag.pointer[0] - (pointer[1] - drag.pointer[1]))
                * if fine { 0.1 } else { 1. };
            drag.pointer = pointer;
            let around =
                |m| Mat4::from_translation(drag.pivot) * m * Mat4::from_translation(-drag.pivot);
            let matrix = match drag.tool {
                Tool::Rotate => {
                    let mut angle = drag.amount * 0.01;
                    if self.modifiers.control_key() {
                        let step = 15_f32.to_radians();
                        angle = (angle / step).round() * step;
                    }
                    let rotation = Mat4::from_axis_angle(drag.axis_world, angle);
                    if let Some(axis) = drag.axis {
                        around(Mat4::from_axis_angle(
                            [Vec3::X, Vec3::Y, Vec3::Z][axis],
                            angle,
                        ))
                    } else {
                        let linear = Mat4::from_mat3(glam::Mat3::from_mat4(drag.inverse_model));
                        around(linear * rotation * linear.inverse())
                    }
                }
                Tool::Scale => {
                    let mut factor = (drag.amount * 0.01).exp().clamp(0.001, 1000.);
                    if self.modifiers.control_key() {
                        factor = ((factor * 10.).round() / 10.).max(0.01);
                    }
                    let mut scale = Vec3::splat(factor);
                    if let Some(axis) = drag.axis {
                        scale = Vec3::ONE;
                        scale[axis] = factor;
                    }
                    around(Mat4::from_scale(scale))
                }
                _ => Mat4::from_translation(delta),
            };
            if matrix.is_finite() && matrix != drag.applied {
                let r = self.editor.preview_mesh(
                    drag.node,
                    &drag.original,
                    match &drag.weights {
                        Some(weights) => MeshEdit::WeightedTransform {
                            weights: weights.clone(),
                            matrix,
                        },
                        None => MeshEdit::Transform {
                            vertices: drag.vertices.clone(),
                            matrix,
                        },
                    },
                );
                if self.result(r).is_some() {
                    drag.applied = matrix;
                    self.invalidate_geometry();
                }
            }
        }
        self.drag = Some(Drag::Mesh(drag));
    }
    pub(super) fn mesh_pointer_down(&mut self) -> bool {
        if self.authoring.knife.is_some() {
            self.knife_click();
            return true;
        }
        self.authoring_prune();
        if self.begin_component_selection() {
            return true;
        }
        if self.tool == Tool::Orbit {
            return false;
        }
        let Some(id) = self.authoring.editing else {
            return false;
        };
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            return true;
        };
        let Ok(globals) = self.editor.project.global_transforms() else {
            return true;
        };
        let model = globals[&id];
        let cache = self.mesh_cache(mesh);
        let inverse_model = model.inverse();
        let view = self.view();
        let projection = view.view_projection();
        let viewport = self.viewport();
        let project_world = |p| project(projection, viewport, p);
        let inverse_view = projection.inverse();
        let visible = |world: Vec3| {
            if self.authoring.xray {
                return true;
            }
            let Some(point) = project_world(world) else {
                return false;
            };
            let Some((origin, direction)) = ray(inverse_view, viewport, point) else {
                return false;
            };
            let distance = (world - origin).dot(direction);
            cache
                .hit(
                    inverse_model.transform_point3(origin),
                    inverse_model.transform_vector3(direction),
                )
                .is_none_or(|(_, hit)| hit + distance.abs().max(1.) * 0.0001 >= distance)
        };
        if self.authoring.selection_mode == S::Face {
            let Some((origin, direction)) = ray(inverse_view, viewport, self.mouse) else {
                return true;
            };
            let inverse_model = model.inverse();
            let hit = cache
                .hit(
                    inverse_model.transform_point3(origin),
                    inverse_model.transform_vector3(direction),
                )
                .and_then(|(triangle, _)| mesh.triangle_face(triangle));
            if let Some(i) = hit {
                if self.modifiers.shift_key() {
                    if !self.authoring.faces.insert(i) {
                        self.authoring.faces.remove(&i);
                    }
                    return true;
                }
                if !self.authoring.faces.contains(&i) {
                    self.authoring.faces.clear();
                    self.authoring.faces.insert(i);
                }
            } else {
                if !self.modifiers.shift_key() {
                    self.authoring.faces.clear();
                }
                return true;
            }
        } else if self.authoring.selection_mode == S::Edge {
            let eye = view.eye;
            let mouse = glam::Vec2::from_array(self.mouse);
            let hit = cache
                .graph
                .edges
                .iter()
                .copied()
                .filter_map(|edge @ (a, b)| {
                    let wa = model.transform_point3(Vec3::from_array(mesh.vertices[a].position));
                    let wb = model.transform_point3(Vec3::from_array(mesh.vertices[b].position));
                    let pa = glam::Vec2::from_array(project_world(wa)?);
                    let pb = glam::Vec2::from_array(project_world(wb)?);
                    let ab = pb - pa;
                    if ab.length_squared() < 1. {
                        return None;
                    }
                    let t = ((mouse - pa).dot(ab) / ab.length_squared()).clamp(0., 1.);
                    let distance = mouse.distance(pa + t * ab);
                    (distance <= 9. && visible(wa.lerp(wb, t)))
                        .then_some((edge, distance + wa.lerp(wb, t).distance(eye) * 0.0001))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(edge, _)| edge);
            if let Some(edge) = hit {
                if self.modifiers.shift_key() {
                    if !self.authoring.edges.insert(edge) {
                        self.authoring.edges.remove(&edge);
                    }
                    return true;
                }
                if !self.authoring.edges.contains(&edge) {
                    self.authoring.edges.clear();
                    self.authoring.edges.insert(edge);
                }
            } else {
                if !self.modifiers.shift_key() {
                    self.authoring.edges.clear();
                }
                return true;
            }
        } else {
            let eye = view.eye;
            let hit = mesh
                .vertices
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    if cache.graph.canonical[i] != i {
                        return None;
                    }
                    let world = model.transform_point3(Vec3::from_array(v.position));
                    let p = project_world(world)?;
                    let d = (p[0] - self.mouse[0]).hypot(p[1] - self.mouse[1]);
                    (d < 9. && visible(world)).then_some((i, d + world.distance(eye) * 0.0001))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            if let Some(i) = hit {
                if self.modifiers.shift_key() {
                    if !self.authoring.vertices.insert(i) {
                        self.authoring.vertices.remove(&i);
                    }
                    return true;
                }
                if !self.authoring.vertices.contains(&i) {
                    self.authoring.vertices.clear();
                    self.authoring.vertices.insert(i);
                }
            } else {
                if !self.modifiers.shift_key() {
                    self.authoring.vertices.clear();
                }
                return true;
            }
        }
        if !self.mesh_double_click() {
            self.begin_mesh_drag();
        }
        true
    }
    pub(super) fn authoring_key(&mut self, key: &Key, ctrl: bool, alt: bool) -> bool {
        if ctrl || self.input.is_some() || self.has_modal() || self.job.is_some() {
            return false;
        }
        if *key == Key::Named(NamedKey::F2) {
            self.action(Action::Author(A::ToggleEdit));
            return true;
        }
        if self.authoring.editing.is_none() {
            return false;
        }
        if self.drag.is_some() {
            if let Key::Character(c) = key {
                if let Some(axis) = ["x", "y", "z"].iter().position(|v| *v == c.to_lowercase()) {
                    if let Some(Drag::Mesh(d)) = &mut self.drag {
                        d.axis = Some(axis);
                        self.authoring.constraint = Some(axis);
                        d.axis_world = d
                            .inverse_model
                            .inverse()
                            .transform_vector3([Vec3::X, Vec3::Y, Vec3::Z][axis])
                            .normalize();
                    }
                }
            }
            return true;
        }
        let command = match key {
            Key::Named(NamedKey::Delete) => Some(A::DeleteFaces),
            Key::Character(c) => match c.to_lowercase().as_str() {
                "a" => Some(if alt { A::Clear } else { A::SelectAll }),
                "1" => Some(A::SelectMode(S::Vertex)),
                "2" => Some(A::SelectMode(S::Edge)),
                "3" => Some(A::SelectMode(S::Face)),
                "e" => Some(A::Extrude),
                "k" => Some(A::Knife),
                "j" => Some(A::Slide),
                "o" => Some(A::Proportional),
                "b" => Some(A::Bevel),
                "i" => Some(A::Inset),
                "c" => Some(A::LoopCut),
                "l" => Some(A::Connected),
                "+" | "=" => Some(A::Grow),
                "-" => Some(A::Shrink),
                "z" if alt => Some(A::Xray),
                "x" => Some(A::Constrain(0)),
                "y" => Some(A::Constrain(1)),
                "z" => Some(A::Constrain(2)),
                "g" | "r" | "s" => {
                    self.authoring.selection_tool = 0;
                    self.tool = match c.to_lowercase().as_str() {
                        "r" => Tool::Rotate,
                        "s" => Tool::Scale,
                        _ => Tool::Move,
                    };
                    self.remember_modeling_tool(&match self.tool {
                        Tool::Rotate => Action::Rotate,
                        Tool::Scale => Action::Scale,
                        _ => Action::Move,
                    });
                    self.begin_mesh_drag();
                    return true;
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(command) = command {
            self.action(Action::Author(command));
            return true;
        }
        false
    }
    pub(super) fn draw_mesh_overlay(&self, u: &mut Ui) {
        let preview = self.authoring.editing.is_none() && self.primitive_panel.is_some();
        let Some(id) = self.authoring.editing.or_else(|| {
            self.primitive_panel.filter(|id| {
                self.selected_primitive()
                    .is_some_and(|(selected, _)| selected == *id)
            })
        }) else {
            return;
        };
        if self.editor.project.node(id).is_some_and(|node| node.shadow_only) {
            return;
        }
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            return;
        };
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        let model = globals[&id];
        let p = u.palette();
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        let projection = self.view().view_projection() * model;
        let viewport = self.viewport();
        let points: Vec<_> = mesh
            .vertices
            .iter()
            .map(|v| project(projection, viewport, Vec3::from_array(v.position)))
            .collect();
        let cache = self.mesh_cache(mesh);
        let g = &cache.graph;
        let depths: Vec<_> = mesh
            .vertices
            .iter()
            .map(|v| {
                let clip = projection * Vec3::from_array(v.position).extend(1.);
                clip.z / clip.w
            })
            .collect();
        let cage_preview=!preview&&self.authoring.preview_modifiers&&self.editor.project.node(id).is_some_and(|n|n.modifiers.iter().any(|m|m.enabled));
        let edge_depth = |a: usize, b: usize| {
            (!cage_preview&&(preview || !self.authoring.xray)).then_some([depths[a], depths[b]])
        };
        let uv_selection = self.uv.open && self.uv.node == Some(id);
        let faces = if uv_selection {
            (0..mesh.face_count())
                .filter(|f| {
                    mesh.face_vertices(*f).is_some_and(|face| {
                        face.iter()
                            .all(|i| self.uv.selected.contains(&(*i as usize)))
                    })
                })
                .collect()
        } else {
            self.authoring.faces.clone()
        };
        if self.authoring.selection_mode == S::Face || uv_selection {
            super::face_overlay::draw(u, mesh, &faces, projection, viewport, self.authoring.xray||cage_preview);
        }
        let mut selected_edges = self.authoring.edges.clone();
        if self.authoring.selection_mode == S::Face || uv_selection {
            selected_edges.clear();
            for &f in &faces {
                if let Some(face) = mesh.face_vertices(f) {
                    for j in 0..face.len() {
                        let a = g.canonical[face[j] as usize];
                        let b = g.canonical[face[(j + 1) % face.len()] as usize];
                        selected_edges.insert((a.min(b), a.max(b)));
                    }
                }
            }
        } else if self.authoring.selection_mode != S::Edge {
            selected_edges.clear();
        }
        // Bound UI instance upload cost on dense meshes; the editable geometry and picking stay exact.
        let edge_step = g.edges.len().div_ceil(24_000).max(1);
        for &(a, b) in g.edges.iter().step_by(edge_step) {
            if let (Some(pa), Some(pb)) = (points[a], points[b]) {
                u.depth_mesh_segment(pa, pb, 0.8, [0.5, 0.55, 0.6, 0.5], edge_depth(a, b));
            }
        }
        if preview {
            clip_scrolled(u, start, viewport, 0.);
            return;
        }
        if let Some(topology) = &mesh.topology {
            for &[a, b] in &topology.uv_seams {
                let a = g.canonical[a as usize];
                let b = g.canonical[b as usize];
                if let (Some(pa), Some(pb)) = (points[a], points[b]) {
                    u.depth_mesh_segment(pa, pb, 1.4, [1., 0.4, 0.2, 1.], edge_depth(a, b));
                }
            }
        }
        for &(a, b) in selected_edges
            .iter()
            .step_by(selected_edges.len().div_ceil(32_000).max(1))
        {
            if let (Some(pa), Some(pb)) = (points[a], points[b]) {
                u.depth_mesh_segment(pa, pb, 2., p.accent, edge_depth(a, b));
            }
        }
        if self.authoring.selection_mode != S::Face && !uv_selection {
            let selected = self.edit_vertices();
            let keys: BTreeSet<_> = selected
                .iter()
                .map(|&i| poseforge_core::position_key(mesh.vertices[i].position))
                .collect();
            let point_step = points.len().div_ceil(12_000).max(1);
            for (i, point) in points.into_iter().enumerate() {
                if g.canonical[i] != i {
                    continue;
                }
                if let Some([x, y]) = point {
                    let selected =
                        keys.contains(&poseforge_core::position_key(mesh.vertices[i].position));
                    if !selected && i % point_step != 0 {
                        continue;
                    }
                    let size = if selected { 6. } else { 4. };
                    u.rect(
                        [x - size / 2., y - size / 2., size, size],
                        if selected { p.accent } else { p.text },
                    );
                    u.rects.last_mut().unwrap().depth = edge_depth(i, i);
                }
            }
        }
        if uv_selection {
            clip_scrolled(u, start, viewport, 0.);
            return;
        }
        let v = self.viewport();
        if edge_step > 1 {
            u.label(
                (self.left_panel_rect()[0] + self.left_panel_rect()[2] + 12.).max(v[0] + 12.),
                v[1] + 14.,
                "Плотная сетка · разреженный обзор",
                10.,
                p.muted,
            );
        }
        clip_scrolled(u, start, v, 0.);
    }
    pub(super) fn draw_authoring_panel(&self, u: &mut Ui, rx: f32, rw: f32) {
        let x = rx + 16.;
        let w = rw - 32.;
        self.draw_modeling_palette(u, x, w);
        self.draw_morph_panel(u,x,w);
    }
}
