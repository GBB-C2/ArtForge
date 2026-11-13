//! Scene selection, parent-safe group transforms and reversible workspace resizing.
use super::*;
use crate::gizmo::{Gizmo, GizmoDrag, Handle, Mode};
use crate::ui::SceneTool as S;
use poseforge_core::{Collection, Project, Transform};
#[derive(Default)]
pub(super) struct SceneTools {
    pub selected: BTreeSet<u64>,
    pub isolated: BTreeSet<u64>,
    pub collection: Option<u64>,
    pub material_expanded: BTreeSet<u64>,
    pub hidden_types: u8,
    pub last_drag_scroll: Option<Instant>,
    pub edge_scroll_since: Option<Instant>,
}
#[derive(Clone)]
pub(super) struct GroupDrag {
    gesture: GizmoDrag,
    origin: Mat4,
    roots: Vec<(u64, Mat4, Mat4)>,
    curves: BTreeMap<u64, Arc<poseforge_core::CurveTranslation>>,
}
#[derive(Clone)]
pub(super) struct PanelDrag {
    pub right: bool,
    pub start: f32,
    pub width: f32,
    pub original: Option<f32>,
}
impl App {
    pub(super) fn clear_scene_selection(&mut self) {
        self.path_edit = None;
        self.scene_tools.selected.clear();
        self.scene_tools.collection = None;
        self.editor.selected = None;
        self.clear_light_selection();
        self.authoring = Default::default();
        self.surface.object_material = None;
        self.surface.mesh = None;
        self.surface.selection = None;
        self.primitive_panel = None;
        self.clear_uv_selection();
        self.set_properties_tab(if self.uv.open { 12 } else { 0 });
        self.status = "Ничего не выбрано".into();
        self.redraw();
    }

    /// Bones form one object in Scene mode, separately from attached meshes.
    /// Shared skin joints connect flat exports without selecting skin owners.
    pub(super) fn skeleton_object_selection(&self, id: u64) -> (u64, BTreeSet<u64>) {
        let project = &self.editor.project;
        if !project.node(id).is_some_and(|n| n.is_joint) {
            let mut members = BTreeSet::from([id]);
            if project
                .node(id)
                .is_some_and(|n| n.mesh.is_none() && n.camera.is_none())
            {
                let descendants = project.descendants_of(&members);
                members.extend(project.nodes.iter()
                    .filter(|n| n.is_joint && descendants.contains(&n.id))
                    .map(|n| n.id));
            }
            return (id, members);
        }
        let nodes: std::collections::BTreeMap<_, _> =
            project.nodes.iter().map(|n| (n.id, n)).collect();
        let ancestors = |start| {
            let mut chain = Vec::new();
            let mut seen = BTreeSet::new();
            let mut cursor = Some(start);
            while let Some(id) = cursor {
                if !seen.insert(id) {
                    break;
                }
                let Some(node) = nodes.get(&id) else {
                    break;
                };
                chain.push(id);
                cursor = node.parent;
            }
            chain
        };
        let mut members = BTreeSet::from([id]);
        loop {
            let before = members.len();
            for skin in project.nodes.iter().filter_map(|n| n.skin.as_ref()) {
                if skin.joints.iter().any(|id| members.contains(id)) {
                    members.extend(
                        skin.joints
                            .iter()
                            .filter(|id| nodes.contains_key(id))
                            .copied(),
                    );
                }
            }
            if before == members.len() {
                break;
            }
        }
        let chains: Vec<_> = members.iter().map(|id| ancestors(*id)).collect();
        let common = ancestors(id)
            .into_iter()
            .find(|candidate| nodes[candidate].mesh.is_none()
                && nodes[candidate].camera.is_none()
                && chains.iter().all(|chain| chain.contains(candidate)));
        let mut roots = members.clone();
        if let Some(mut root) = common {
            // Include the armature container, stopping before an unrelated
            // scene collection. Never promote a joint to a mesh or camera.
            for ancestor in ancestors(root).into_iter().skip(1) {
                let parent = nodes[&ancestor];
                if parent.mesh.is_some() || parent.camera.is_some() {
                    break;
                }
                if !nodes[&root].is_joint {
                    break;
                }
                root = ancestor;
                if !parent.is_joint {
                    break;
                }
            }
            roots = BTreeSet::from([root]);
        }
        let descendants = project.descendants_of(&roots);
        members = project.nodes.iter()
            .filter(|n| descendants.contains(&n.id) && (n.is_joint || roots.contains(&n.id)))
            .map(|n| n.id).collect();
        let primary = project
            .selection_roots(&members)
            .into_iter()
            .next()
            .unwrap_or(id);
        (primary, members)
    }
    pub(super) fn object_selection(&self) -> BTreeSet<u64> {
        let Some(primary) = self.editor.selected else {
            return BTreeSet::new();
        };
        if !self.scene_tools.selected.contains(&primary) {
            return BTreeSet::from([primary]);
        }
        self.editor
            .project
            .nodes
            .iter()
            .filter(|n| self.scene_tools.selected.contains(&n.id))
            .map(|n| n.id)
            .collect()
    }
    pub(super) fn object_selected(&self, id: u64) -> bool {
        self.editor.selected.is_some_and(|primary| {
            if self.scene_tools.selected.contains(&primary) {
                self.scene_tools.selected.contains(&id)
            } else {
                primary == id
            }
        })
    }
    pub(super) fn select_object(&mut self, id: u64) {
        self.path_edit = None;
        if self.editor.project.node(id).is_none() {
            return;
        }
        let extend = (self.modifiers.shift_key() || self.modifiers.control_key())
            && self.editor.mode == EditorMode::Scene;
        let (id, members) = if self.editor.mode == EditorMode::Scene {
            self.skeleton_object_selection(id)
        } else {
            (id, BTreeSet::from([id]))
        };
        let mut selected = if extend {
            self.object_selection()
        } else {
            BTreeSet::new()
        };
        if extend && selected.contains(&id) {
            selected.retain(|id| !members.contains(id));
            self.editor.selected = selected.iter().next_back().copied();
        } else {
            selected.extend(members);
            self.editor.selected = Some(id);
        }
        self.scene_tools.selected = selected;
        if self.editor.selected.and_then(|id| self.editor.project.node(id)).is_some_and(|n| n.mesh.is_some()) {
            self.surface.material_library = false;
        }
        self.scene_tools.collection = self.editor.project.node_collection(id);
        if self.editor.selected == Some(id) {
            self.reveal_scene_node(id);
            if self.surface.tab == 1
                && self
                    .editor
                    .project
                    .node(id)
                    .is_some_and(|n| n.camera.is_some())
            {
                self.set_properties_tab(0);
            }
        }
    }
    pub(super) fn in_isolation(&self, id: u64) -> bool {
        self.scene_tools.isolated.is_empty()
            || self
                .scene_tools
                .isolated
                .iter()
                .any(|root| self.is_descendant(id, *root))
    }
    pub(super) fn picking_project(&self) -> std::borrow::Cow<'_, Project> {
        let mut p = self.editor.project.clone();
        self.apply_viewport_backfaces(&mut p);
        self.apply_physics_preview(&mut p);
        self.isolation_preview(&mut p);
        std::borrow::Cow::Owned(p)
    }
    /// Pointer clicks use the same object selection with transform and orbit
    /// tools. Visible bones win on their silhouette; a skin surface stays its
    /// own object instead of being promoted to its armature parent.
    pub(super) fn pick_viewport_node(&self, eval: &poseforge_core::EvaluatedScene) -> Option<u64> {
        let preview = self.picking_project();
        let view = self.view();
        let viewport = self.viewport();
        if self.preferences.skeleton {
            let bones = crate::bones::BoneOverlay::new(&preview, eval, &view, viewport);
            if let Some(id) = bones.hit(self.mouse) { return Some(id); }
        }
        let mesh = crate::picking::pick_mesh(&preview, eval, view.view_projection(), viewport, self.mouse);
        // Invisible parent origins must not steal a mesh's visible surface.
        if self.editor.mode == EditorMode::Scene && mesh.is_some() { return mesh; }
        let origin = self.editor.project.nodes.iter()
            .filter(|n| !n.is_joint && !n.shadow_only && self.node_selectable(n.id))
            .filter_map(|n| {
                let p = self.project_point(eval.global.get(&n.id)?.w_axis.truncate())?;
                let distance = (p[0]-self.mouse[0]).hypot(p[1]-self.mouse[1]);
                (distance < 16.).then_some((distance, n.id))
            })
            .min_by(|a,b| a.0.total_cmp(&b.0)).map(|(_,id)| id);
        origin.or_else(|| mesh.map(|hit| {
            if self.editor.mode == EditorMode::Scene { hit }
            else { self.editor.project.node(hit).filter(|n| n.skin.is_some())
                .and_then(|n| n.parent).unwrap_or(hit) }
        }))
    }
    pub(super) fn isolation_preview(&self, p: &mut Project) {
        if self.scene_tools.isolated.is_empty() {
            return;
        }
        let kept = p.descendants_of(&self.scene_tools.isolated);
        let parents: std::collections::BTreeMap<_, _> =
            p.nodes.iter().map(|n| (n.id, n.parent)).collect();
        let mut ancestors = BTreeSet::new();
        for id in &kept {
            let mut cursor = parents.get(id).copied().flatten();
            while let Some(parent) = cursor {
                if !ancestors.insert(parent) {
                    break;
                }
                cursor = parents.get(&parent).copied().flatten();
            }
        }
        for n in &mut p.nodes {
            if !kept.contains(&n.id) {
                n.mesh = None;
                n.material_layers.clear();
                n.camera = None;
                if !ancestors.contains(&n.id) {
                    n.visible = false;
                }
            }
        }
        if p.active_camera
            .is_some_and(|id| p.node(id).is_none_or(|n| n.camera.is_none()))
        {
            p.active_camera = None;
        }
    }
    pub(super) fn scene_tool_action(&mut self, action: S) {
        match action {
            S::SelectAll => {
                let allowed = self.editor.project.selectable_node_ids();
                let isolated = self
                    .editor
                    .project
                    .descendants_of(&self.scene_tools.isolated);
                self.scene_tools.selected = self
                    .editor
                    .project
                    .nodes
                    .iter()
                    .filter(|n| {
                        allowed.contains(&n.id)
                            && (self.scene_tools.isolated.is_empty() || isolated.contains(&n.id))
                    })
                    .map(|n| n.id)
                    .collect();
                self.editor.selected = self.scene_tools.selected.iter().next().copied();
                self.clear_light_selection();
            }
            S::Clear => {
                self.clear_scene_selection();
            }
            S::Isolate => {
                if self.scene_tools.isolated.is_empty() {
                    self.scene_tools.isolated = self.object_selection();
                } else {
                    self.scene_tools.isolated.clear();
                }
                self.performance.changed();
                self.status = if self.scene_tools.isolated.is_empty() {
                    "Изоляция выключена"
                } else {
                    "Изоляция выделения · кнопка «Изоляция» возвращает сцену"
                }
                .into();
            }
            S::NewCollection => {
                if self.editor.project.collections.len() >= 256 {
                    self.status = "Не более 256 коллекций.".into();
                    return;
                }
                let id = self.editor.project.next_id();
                let members: BTreeSet<_> = self
                    .editor
                    .project
                    .selection_roots(&self.object_selection())
                    .into_iter()
                    .collect();
                let light = self.active_light_index();
                if members
                    .iter()
                    .any(|id| self.editor.project.node_locked(*id))
                    || light.is_some_and(|i| self.editor.project.light_locked(i))
                {
                    self.status = "Выделение заблокировано.".into();
                    return;
                }
                let branches = self.editor.project.descendants_of(&members);
                let name = format!("Папка {}", self.editor.project.collections.len() + 1);
                let r = self.editor.transact("Создать коллекцию", |p| {
                    for c in &mut p.collections {
                        c.members.retain(|id| !branches.contains(id));
                        if let Some(light) = light {
                            c.light_members.retain(|i| *i != light);
                        }
                    }
                    p.collections.push(Collection {
                        id,
                        name: name.clone(),
                        members: members.into_iter().collect(),
                        light_members: light.into_iter().collect(),
                        ..Default::default()
                    });
                });
                if self.result(r).is_some() {
                    self.scene_tools.collection = Some(id);
                    self.reveal_collection(id);
                    self.input(Action::SceneTool(S::CollectionName(id)), name);
                }
            }
            S::CollectionName(id) => {
                if let Some(name) = self
                    .editor
                    .project
                    .collections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
                {
                    self.reveal_collection(id);
                    self.input(Action::SceneTool(action), name);
                }
            }
            S::CollectionSelect(id) => {
                let ids: BTreeSet<_> = self
                    .editor
                    .project
                    .nodes
                    .iter()
                    .filter(|n| {
                        self.editor.project.node_collection(n.id) == Some(id)
                            && self.node_selectable(n.id)
                    })
                    .map(|n| n.id)
                    .collect();
                self.scene_tools.collection = Some(id);
                self.scene_tools.selected = ids;
                self.editor.selected = self.scene_tools.selected.iter().next().copied();
                self.clear_light_selection();
            }
            S::AssignCollection(id) => {
                let items = if let Some(light) = self.active_light_index() {
                    vec![poseforge_core::SceneItem::Light(light)]
                } else {
                    self.editor
                        .project
                        .selection_roots(&self.object_selection())
                        .into_iter()
                        .map(poseforge_core::SceneItem::Node)
                        .collect()
                };
                self.move_scene_items(
                    &items,
                    super::scene_organization::Destination {
                        folder: id,
                        anchor: None,
                        after: true,
                    },
                );
            }
            S::CollectionCollapse(id) => {
                if !self.collapsed.remove(&id) {
                    self.collapsed.insert(id);
                }
            }
            S::CollectionVisible(id)
            | S::CollectionLock(id)
            | S::CollectionRender(id)
            | S::CollectionDelete(id) => {
                if action == S::CollectionDelete(id) {
                    self.action(Action::SceneDelete(poseforge_core::SceneItem::Folder(id)));
                    return;
                }
                let r = self.editor.transact("Настроить коллекцию", |p| {
                    if action == S::CollectionDelete(id) {
                        p.collections.retain(|c| c.id != id);
                        return;
                    }
                    if let Some(c) = p.collections.iter_mut().find(|c| c.id == id) {
                        match action {
                            S::CollectionVisible(_) => c.visible = !c.visible,
                            S::CollectionLock(_) => c.locked = !c.locked,
                            S::CollectionRender(_) => c.render_visible = !c.render_visible,
                            _ => {}
                        }
                    }
                });
                self.result(r);
            }
        }
        self.update_dirty();
        self.redraw();
    }
    fn reveal_collection(&mut self, id: u64) {
        self.preferences.left_panel = true;
        self.scene_query.clear();
        self.joints_only = false;
        self.scene_scroll = self
            .outliner_rows()
            .iter()
            .position(|row| matches!(row,scene_tree::SceneEntry::Collection(cid) if *cid==id))
            .unwrap_or(0);
    }
    pub(super) fn collection_confirm(
        &mut self,
        action: &Action,
        value: &str,
    ) -> Result<bool, String> {
        let Action::SceneTool(S::CollectionName(id)) = action else {
            return Ok(false);
        };
        let value = value.trim();
        if value.is_empty() || value.chars().count() > 64 || value.chars().any(char::is_control) {
            return Err("Название коллекции: от 1 до 64 символов.".into());
        }
        if !self.editor.project.collections.iter().any(|c| c.id == *id) {
            return Err("Коллекция удалена.".into());
        }
        self.editor
            .transact("Переименовать коллекцию", |p| {
                p.collections.iter_mut().find(|c| c.id == *id).unwrap().name = value.into()
            })
            .map_err(|e| e.to_string())?;
        self.update_dirty();
        Ok(true)
    }
    pub(super) fn group_gizmo(&self) -> Option<Gizmo> {
        if self.editor.mode != EditorMode::Scene
            || self.has_modal()
            || self.authoring.editing.is_some()
            || self.tool == Tool::Orbit
        {
            return None;
        }
        let selected = self.object_selection();
        if selected.len() < 2 || selected.iter().any(|id| !self.node_selectable(*id)) {
            return None;
        }
        let eval = self.evaluated_interactive().ok()?;
        let roots = self.editor.project.selection_roots(&selected);
        if roots.is_empty() {
            return None;
        }
        let origin = roots
            .iter()
            .filter_map(|id| eval.global.get(id))
            .map(|m| m.w_axis.truncate())
            .sum::<Vec3>()
            / roots.len() as f32;
        let rotation = if self.gizmo_local {
            self.editor
                .selected
                .and_then(|id| eval.global.get(&id))
                .and_then(|m| crate::gizmo::uniform_parent_rotation(*m))
                .unwrap_or(Quat::IDENTITY)
        } else {
            Quat::IDENTITY
        };
        let mode = match self.tool {
            Tool::Move => Mode::Move,
            Tool::Rotate => Mode::Rotate,
            Tool::Scale => Mode::Scale,
            Tool::Orbit => return None,
        };
        Gizmo::new(
            mode,
            Mat4::from_rotation_translation(rotation, origin),
            self.gizmo_local,
            self.view(),
            self.viewport(),
        )
    }
    pub(super) fn begin_group_drag(&mut self, handle: Handle) -> bool {
        let selected = self.object_selection();
        if selected.len() < 2 || self.editor.mode != EditorMode::Scene {
            return false;
        }
        let Some(gizmo) = self.group_gizmo() else {
            return true;
        };
        let Ok(eval) = self.evaluated_interactive() else {
            return true;
        };
        let origin = Transform {
            translation: gizmo.origin,
            rotation: Quat::from_mat3(&glam::Mat3::from_cols(
                gizmo.axes[0],
                gizmo.axes[1],
                gizmo.axes[2],
            )),
            scale: Vec3::ONE,
        };
        let Some(gesture) = GizmoDrag::begin(gizmo, handle, origin, Mat4::IDENTITY, self.mouse)
        else {
            return true;
        };
        let mut roots = vec![];
        let mut curves = BTreeMap::new();
        let selected_roots = self.editor.project.selection_roots(&selected);
        for &id in &selected_roots {
            if self.tool == Tool::Move {
                let path = match self.object_curve_translation(id, &eval) {
                    Ok(path) => path, Err(e) => { self.status = e.to_string(); return true; }
                };
                if let Some(path) = path {
                    // Moving the source (or its parent) already moves followers.
                    let mut cursor = Some(path.binding().curve); let mut follows_selection = false;
                    while let Some(source) = cursor {
                        if selected_roots.contains(&source) { follows_selection = true; break; }
                        cursor = self.editor.project.node(source).and_then(|n| n.parent);
                    }
                    if follows_selection { continue; }
                    curves.insert(id, path);
                }
            }
            let Some(world) = eval.global.get(&id) else {
                return true;
            };
            let Ok(parent) = self.editor.project.transform_frame(id, &eval.global, self.editor.time) else { return true; };
            if parent.determinant().abs() < 1e-8 {
                self.status = "Родитель имеет нулевой масштаб.".into();
                return true;
            }
            roots.push((id, *world, parent.inverse()));
        }
        self.editor.begin_edit("Трансформация выделения");
        self.editor.playing = false;
        self.drag = Some(Drag::Group(GroupDrag {
            gesture,
            origin: origin.matrix(),
            roots,
            curves,
        }));
        self.drag_button = Some(MouseButton::Left);
        true
    }
    pub(super) fn update_group_drag(&mut self, mut drag: GroupDrag, pointer: [f32; 2]) {
        if let Some((transform, _, feedback)) = drag.gesture.update(
            pointer,
            self.modifiers.shift_key(),
            self.modifiers.control_key(),
        ) {
            let delta = transform.matrix() * drag.origin.inverse();
            let mut transforms = vec![];
            let mut valid = true;
            for (id, world, inverse_parent) in &drag.roots {
                let matrix = *inverse_parent * delta * *world;
                let t = Transform::from_matrix(matrix);
                let tolerance = matrix
                    .to_cols_array()
                    .iter()
                    .map(|x| x.abs())
                    .fold(1., f32::max)
                    * 0.001;
                if !t.is_valid()
                    || t.matrix()
                        .to_cols_array()
                        .iter()
                        .zip(matrix.to_cols_array())
                        .any(|(a, b)| (*a - b).abs() > tolerance)
                {
                    valid = false;
                    break;
                }
                transforms.push((*id, t));
            }
            if valid {
                for (id, t) in transforms {
                    let result = if let Some(path) = drag.curves.get(&id) {
                        let (_, world, _) = drag.roots.iter().find(|(root, _, _)| *root == id).unwrap();
                        self.translate_object_on_curve(id, path, (delta * *world).w_axis.truncate())
                    } else { self.editor.set_transform(id, t) };
                    if result.is_err() {
                        self.editor.cancel_edit();
                        self.drag = None;
                        self.drag_button = None;
                        self.update_dirty();
                        return;
                    }
                }
                self.status = format!("{} объектов · {feedback}", drag.roots.len());
                self.update_dirty();
            } else {
                self.status="Такой поворот или масштаб создаёт сдвиг осей у родителя. Используйте перемещение или равномерный масштаб.".into();
            }
        }
        self.drag = Some(Drag::Group(drag));
    }
    pub(super) fn begin_panel_resize(&mut self, right: bool) {
        let (width, original) = if right {
            (self.right_width(), self.preferences.right_width)
        } else {
            (self.left_width(), self.preferences.left_width)
        };
        self.drag = Some(Drag::Panel(PanelDrag {
            right,
            start: self.mouse[0],
            width,
            original,
        }));
        self.drag_button = Some(MouseButton::Left);
    }
    pub(super) fn update_panel_resize(&mut self, drag: PanelDrag, pointer: [f32; 2]) {
        let width = drag.width + (pointer[0] - drag.start) * if drag.right { -1. } else { 1. };
        if drag.right {
            self.preferences.right_width = Some(width.clamp(280., 600.));
        } else {
            self.preferences.left_width = Some(width.clamp(220., 520.));
        }
        self.drag = Some(Drag::Panel(drag));
    }
    pub(super) fn draw_scene_tools(&self, u: &mut Ui) {
        for (right, r) in [
            (false, self.left_panel_rect()),
            (true, self.right_panel_rect()),
        ] {
            if (right && !self.preferences.right_panel) || (!right && !self.scene_panel_visible()) {
                continue;
            }
            let x = if right { r[0] - 3. } else { r[0] + r[2] - 3. };
            u.hits.push(ui::Hit {
                rect: [x, r[1] + 5., 6., r[3] - 10.],
                action: Action::PanelResize(right),
            });
        }
        if !self.scene_tools.isolated.is_empty() {
            u.button(
                [
                    self.left_panel_rect()[0] + self.left_width() + 12.,
                    112.,
                    150.,
                    28.,
                ],
                "Изоляция · выйти",
                Action::SceneTool(S::Isolate),
                true,
            );
        }
        let ids = self.object_selection();
        if ids.len() > 1 {
            u.label(
                self.left_panel_rect()[0] + self.left_width() + 12.,
                150.,
                format!("Выбрано: {} · Shift + щелчок", ids.len()),
                11.,
                u.palette().accent,
            );
        }
    }
}
impl App {
    pub(super) fn prune_scene_tools(&mut self) {
        let valid: BTreeSet<_> = self.editor.project.nodes.iter().map(|n| n.id).collect();
        self.scene_tools.selected.retain(|id| valid.contains(id));
        self.scene_tools.isolated.retain(|id| valid.contains(id));
        if self
            .scene_tools
            .collection
            .is_some_and(|id| !self.editor.project.collections.iter().any(|c| c.id == id))
        {
            self.scene_tools.collection = None;
        }
    }
    pub(super) fn group_action(&mut self, action: &Action) -> bool {
        if self.editor.mode != EditorMode::Scene || self.authoring.editing.is_some() {
            return false;
        }
        let selected = self.object_selection();
        if selected.len() < 2 {
            return false;
        }
        if !matches!(
            action,
            Action::Delete | Action::Duplicate | Action::ToggleVisible | Action::FrameSelected
        ) {
            return false;
        }
        if self.has_modal() || self.job.is_some() {
            return true;
        }
        if *action == Action::FrameSelected {
            if self.frame_modifier_selection(&selected) {return true;}
            if let Ok(eval) = self.evaluated_interactive() {
                let points: Vec<_> = selected
                    .iter()
                    .filter_map(|id| eval.global.get(id))
                    .map(|m| m.w_axis.truncate())
                    .collect();
                if !points.is_empty() {
                    self.leave_camera_view();
                    self.target = points.iter().copied().sum::<Vec3>() / points.len() as f32;
                    self.distance = (points
                        .iter()
                        .map(|p| p.distance(self.target))
                        .fold(0., f32::max)
                        * 3.
                        + 3.)
                        .clamp(0.1, 500.);
                }
            }
            return true;
        }
        if selected
            .iter()
            .any(|id| self.editor.project.node_locked(*id))
        {
            self.status = "В выделении есть заблокированные объекты.".into();
            return true;
        }
        let roots = self.editor.project.selection_roots(&selected);
        if *action == Action::ToggleVisible {
            let visible = selected
                .iter()
                .all(|id| self.editor.project.node(*id).is_some_and(|n| n.visible));
            let result = self.editor.transact("Видимость выделения", |p| {
                for id in &selected {
                    if let Some(n) = p.node_mut(*id) {
                        n.visible = !visible;
                    }
                }
            });
            self.result(result);
            return true;
        }
        self.editor.begin_edit(if *action == Action::Delete {
            "Удалить выделение"
        } else {
            "Дублировать выделение"
        });
        let mut copies = BTreeSet::new();
        let mut failure = None;
        for id in roots {
            if *action == Action::Delete {
                self.editor.selected = Some(id);
                if let Err(e) = self.editor.delete_selected() {
                    failure = Some(e.to_string());
                    break;
                }
            } else {
                match self.editor.project.duplicate_subtree(id) {
                    Ok(id) => {
                        copies.insert(id);
                    }
                    Err(e) => {
                        failure = Some(e.to_string());
                        break;
                    }
                }
            }
        }
        if let Some(error) = failure {
            self.editor.cancel_edit();
            self.scene_tools.selected = selected;
            self.status = error;
        } else {
            let result = self.editor.commit_edit();
            if self.result(result).is_some() {
                self.scene_tools.selected = copies;
                self.editor.selected = self.scene_tools.selected.iter().next().copied();
            }
        }
        self.invalidate();
        self.update_dirty();
        true
    }
}
