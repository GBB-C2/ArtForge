//! Outliner interactions commit only on release; Escape never edits the document.
use super::{destruction_controls, scene_tree::SceneEntry, App, Drag};
use crate::ui::{self, Action, SceneTool, Ui};
use poseforge_core::{Light, SceneItem};
use std::collections::BTreeSet;
use winit::event::MouseButton;

#[derive(Clone)]
pub(super) enum Source {
    Items(Vec<SceneItem>),
    Material { node: u64, mesh: u64, material: u64 },
}
#[derive(Clone)]
pub(super) struct SceneDrag {
    pub source: Source,
    action: Action,
    start: [f32; 2],
    lights: Vec<(usize, Light)>,
    history: usize,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Destination {
    pub anchor: Option<SceneItem>,
    pub folder: Option<u64>,
    pub after: bool,
}

impl SceneEntry {
    pub(super) fn item(&self) -> SceneItem {
        match self {
            Self::Node(n) => SceneItem::Node(n.id),
            Self::Collection(id) => SceneItem::Folder(*id),
            Self::Light(i) => SceneItem::Light(*i),
            Self::Material(node, _, _) | Self::Fluid(node, _) | Self::Cloth(node, _) | Self::Destruction(node, _) => SceneItem::Node(*node),
        }
    }
}

impl App {
    pub(super) fn scene_list_rect(&self) -> [f32; 4] {
        let r = self.left_panel_rect();
        [
            r[0] + 8.,
            r[1] + 74.,
            r[2] - 24.,
            self.scene_capacity() as f32 * 27.,
        ]
    }

    pub(super) fn scene_material_ids(&self, node: u64) -> Vec<u64> {
        let Some(node) = self.editor.project.node(node) else { return vec![]; };
        let Some(mesh) = node.mesh.and_then(|m| self.editor.project.mesh(m)) else { return vec![]; };
        self.scene_material_ids_for(node, mesh)
    }

    /// The outliner already has indexed nodes and meshes. Reuse those references
    /// instead of searching the entire document for every expanded object.
    pub(super) fn scene_material_ids_for(&self, node: &poseforge_core::Node, mesh: &poseforge_core::Mesh) -> Vec<u64> {
        let assigned = self.assigned_material_ids(mesh);
        let mut ids: Vec<_> = self
            .editor
            .project
            .outliner
            .materials
            .get(&node.id)
            .into_iter()
            .flatten()
            .copied()
            .filter(|m| assigned.contains(m))
            .collect();
        for id in mesh.material.into_iter().chain(assigned.iter().copied()) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        let layers: Vec<_> = node.material_layers.iter().rev().map(|l| l.material).collect();
        ids.retain(|id| !layers.contains(id));
        let mut result:Vec<_> = layers.into_iter().chain(ids).collect();
        if let Some(interior)=node.destruction.and_then(|d|d.interior_material) {
            if !result.contains(&interior) {result.push(interior);}
        }
        result
    }

    pub(super) fn scene_organization_action(&mut self, action: &Action) -> bool {
        match action {
            Action::SceneExpand(id) => {
                let children = self
                    .editor
                    .project
                    .nodes
                    .iter()
                    .any(|n| n.parent == Some(*id));
                let open = if children {
                    !self.collapsed.contains(id)
                } else {
                    self.scene_tools.material_expanded.contains(id)
                };
                if open {
                    self.scene_tools.material_expanded.remove(id);
                    self.collapsed.insert(*id);
                } else {
                    self.scene_tools.material_expanded.insert(*id);
                    self.collapsed.remove(id);
                }
            }
            Action::SceneCloth(id) | Action::SceneDestruction(id) => {
                let cloth = matches!(action, Action::SceneCloth(_));
                let exists = self.editor.project.node(*id).is_some_and(|n| if cloth {
                    n.physics.is_some_and(|s| s.kind == poseforge_core::PhysicsKind::Cloth)
                } else { n.destruction.is_some() });
                if exists { self.action(Action::Select(*id)); self.set_properties_tab(if cloth { 11 } else { 13 }); self.preferences.right_panel = true; }
            }
            Action::SceneClothVisible(id) | Action::SceneClothRemove(id) => {
                if self.has_modal() || self.scene_write_pending() || self.editor.project.node_locked(*id) { return true; }
                let Some(settings) = self.editor.project.node(*id).and_then(|n| n.physics).filter(|s| s.kind == poseforge_core::PhysicsKind::Cloth) else { return true; };
                let remove = matches!(action, Action::SceneClothRemove(_));
                let result = self.editor.transact(if remove { "Снять ткань с примитива" } else { "Видимость ткани" }, |p| {
                    p.node_mut(*id).unwrap().physics = if remove { None } else { Some(poseforge_core::PhysicsSettings { visible: !settings.visible, ..settings }) };
                });
                if self.result(result).is_some() { self.studio.physics_running = false; self.studio.physics.reset(); self.invalidate_geometry(); }
            }
            Action::SceneDestructionVisible(id) | Action::SceneDestructionRemove(id) => {
                if self.has_modal() || self.scene_write_pending() || self.editor.project.node_locked(*id) { return true; }
                let Some(settings) = self.editor.project.node(*id).and_then(|n| n.destruction) else { return true; };
                let remove = matches!(action, Action::SceneDestructionRemove(_));
                if settings.result.is_some_and(|root| self.editor.project.node_locked(root)) { self.status = "Осколки заблокированы.".into(); return true; }
                let after = if remove { destruction_controls::without_destruction(&self.editor.project, *id, true) } else {
                    let mut after = self.editor.project.clone();
                    let visible = !settings.visible;
                    if let Some(root) = settings.result.and_then(|root| after.node_mut(root)) { root.visible = visible; root.render_visible = visible; }
                    let owner = after.node_mut(*id).unwrap();
                    owner.destruction.as_mut().unwrap().visible = visible;
                    let show_source = !visible || settings.result.is_none();
                    owner.visible = show_source && settings.source_visible;
                    owner.render_visible = show_source && settings.source_render_visible;
                    Ok(after)
                };
                match after {
                    Ok(after) => {
                        let result = self.editor.transact(if remove { "Снять разрушение с примитива" } else { "Видимость разрушения" }, |p| *p = after);
                        if self.result(result).is_some() { self.editor.playing = false; self.invalidate_geometry(); }
                    }
                    Err(error) => self.status = error,
                }
            }
            Action::SceneFluid(id) => {
                if self.editor.project.node(*id).is_some_and(|n| n.fluid.is_some()) {
                    self.action(Action::Select(*id));
                    self.set_properties_tab(4);
                    self.preferences.right_panel = true;
                }
            }
            Action::SceneFluidVisible(id) | Action::SceneFluidRemove(id) => {
                if self.has_modal() || self.scene_write_pending() || self.editor.project.node_locked(*id) { return true; }
                let Some(settings) = self.editor.project.node(*id).and_then(|n| n.fluid) else { return true; };
                let remove = matches!(action, Action::SceneFluidRemove(_));
                let result = self.editor.transact(if remove { "Снять жидкость с примитива" } else { "Видимость жидкости" }, |p| {
                    let node = p.node_mut(*id).unwrap();
                    node.fluid = if remove { None } else { Some(poseforge_core::FluidDomainSettings { visible: !settings.visible, ..settings }) };
                });
                if self.result(result).is_some() {
                    if remove { self.fluid_domains.reset(*id); }
                    self.tick_fluid_domains();
                    self.invalidate();
                }
            }
            Action::SceneMaterialVisible(material) => {
                if self.has_modal() || self.scene_write_pending() { return true; }
                let Some(visible) = self.editor.project.material(*material).map(|m| m.visible) else { return true; };
                let result = self.editor.transact("Видимость материала", |p| {
                    if let Some(m) = p.materials.iter_mut().find(|m| m.id == *material) { m.visible = !visible; }
                });
                self.result(result);
                self.invalidate_geometry();
            }
            Action::SceneMaterialDetach(node, material) => {
                if self.has_modal() || self.scene_write_pending() || !self.scene_material_ids(*node).contains(material) { return true; }
                if self.detach_destruction_material(*node,*material) {return true;}
                let Some(mesh) = self.editor.project.node(*node).and_then(|n| n.mesh) else { return true; };
                if self.editor.project.node(*node).is_some_and(|n| n.material_layers.iter().any(|l| l.material == *material)) {
                    let result = self.editor.remove_material_layer(*node, *material);
                    self.result(result);
                    if self.editor.selected == Some(*node) { self.surface.object_material = None; }
                    self.invalidate_geometry();
                    return true;
                }
                let result = self.editor.unassign_material_slot(mesh, &[*node], *material);
                if let Some(mesh) = self.result(result) {
                    if self.editor.selected == Some(*node) {
                        self.surface.mesh = Some(mesh); self.surface.object_material = None;
                    }
                    self.invalidate_geometry();
                }
            }
            Action::SceneMaterial(node, material) => {
                if !self.scene_material_ids(*node).contains(material) {
                    return true;
                }
                self.action(Action::Select(*node));
                // A material row addresses this mesh instance even inside a character rig.
                self.editor.selected = Some(*node);
                self.scene_tools.selected = BTreeSet::from([*node]);
                self.set_properties_tab(1);
                self.surface.material_library = false;
                self.surface.selection = Some(*node);
                self.surface.mesh = self.editor.project.node(*node).and_then(|n| n.mesh);
                self.surface.object_material = self.surface.mesh.map(|mesh| (mesh, *material));
                self.preferences.right_panel = true;
                self.inspector_scroll = 0.;
                self.select_material_click(*material);
            }
            Action::SceneDeleteLight(index, expected) => {
                if self.has_modal() || self.scene_write_pending() {
                    return true;
                }
                if self.editor.project.lights.get(*index) != Some(expected.as_ref()) {
                    self.status = "Источник изменился. Выберите его ещё раз.".into();
                    return true;
                }
                self.delete_scene_item(SceneItem::Light(*index));
            }
            Action::SceneDelete(item) => {
                if !self.has_modal() && !self.scene_write_pending() {
                    self.delete_scene_item(*item);
                }
            }
            _ => return false,
        }
        true
    }

    fn delete_scene_item(&mut self, item: SceneItem) {
        if self.scene_item_locked(item) {
            self.status = "Объект или папка заблокированы.".into();
            return;
        }
        let result = match item {
            SceneItem::Node(id) => self.editor.delete_node(id),
            SceneItem::Light(index) => self
                .editor
                .transact("Удалить свет", |p| p.remove_light(index)),
            SceneItem::Folder(id) => self.editor.transact("Удалить папку", |p| {
                p.collections.retain(|c| c.id != id)
            }),
        };
        if self.result(result).is_some() {
            self.scene_tools
                .selected
                .retain(|id| self.editor.project.node(*id).is_some());
            if let SceneItem::Light(index) = item {
                if self.surface.light == index {
                    self.clear_light_selection();
                } else if self.surface.light > index {
                    self.surface.light -= 1;
                }
                self.solo_light = None;
                self.light_aim = None;
            }
            if let SceneItem::Folder(id) = item {
                if self.scene_tools.collection == Some(id) {
                    self.scene_tools.collection = None;
                }
            }
            self.scene_tools
                .material_expanded
                .retain(|id| self.editor.project.node(*id).is_some());
            if self
                .authoring
                .editing
                .is_some_and(|id| self.editor.project.node(id).is_none())
            {
                self.authoring.editing = None;
                self.authoring.vertices.clear();
                self.authoring.edges.clear();
                self.authoring.faces.clear();
            }
            self.invalidate();
            self.update_dirty();
        }
    }

    fn scene_item_locked(&self, item: SceneItem) -> bool {
        let p = &self.editor.project;
        match item {
            SceneItem::Node(id) => p.node_locked(id),
            SceneItem::Light(index) => p.light_locked(index),
            SceneItem::Folder(id) => p
                .collections
                .iter()
                .find(|c| c.id == id)
                .is_none_or(|c| c.locked),
        }
    }

    pub(super) fn begin_scene_drag(&mut self, action: &Action) -> bool {
        if !self.preferences.left_panel
            || !ui::contains(self.scene_list_rect(), self.mouse)
            || self.has_modal()
            || self.scene_write_pending()
        {
            return false;
        }
        let source = match action {
            Action::Select(id) => {
                let ids = if self.object_selected(*id) {
                    self.object_selection()
                } else {
                    BTreeSet::from([*id])
                };
                Source::Items(
                    self.editor
                        .project
                        .selection_roots(&ids)
                        .into_iter()
                        .map(SceneItem::Node)
                        .collect(),
                )
            }
            Action::SceneSelectLight(i, expected) => {
                if self.editor.project.lights.get(*i) != Some(expected.as_ref()) {
                    return true;
                }
                Source::Items(vec![SceneItem::Light(*i)])
            }
            Action::SceneTool(SceneTool::CollectionSelect(id)) => {
                Source::Items(vec![SceneItem::Folder(*id)])
            }
            Action::SceneMaterial(node, material) => {
                let Some(mesh) = self.editor.project.node(*node).and_then(|n| n.mesh) else {
                    return true;
                };
                Source::Material {
                    node: *node,
                    mesh,
                    material: *material,
                }
            }
            _ => return false,
        };
        if !self.resolve_input() {
            return true;
        }
        let lights = match &source {
            Source::Items(items) => items
                .iter()
                .filter_map(|item| match item {
                    SceneItem::Light(i) => {
                        self.editor.project.lights.get(*i).cloned().map(|l| (*i, l))
                    }
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        self.scene_tools.edge_scroll_since = None;
        self.drag = Some(Drag::Scene(SceneDrag {
            source,
            action: action.clone(),
            start: self.mouse,
            lights,
            history: self.editor.undo_len(),
        }));
        self.drag_button = Some(MouseButton::Left);
        true
    }

    fn scene_row_at(&self) -> Option<(SceneEntry, f32)> {
        let r = self.scene_list_rect();
        if !ui::contains(r, self.mouse) {
            return None;
        }
        let offset = ((self.mouse[1] - r[1]) / 27.).floor() as usize;
        self.outliner_rows()
            .get(self.scene_scroll + offset)
            .cloned()
            .map(|row| (row, r[1] + offset as f32 * 27.))
    }

    fn scene_destination(&self) -> Option<Destination> {
        if !ui::contains(self.scene_list_rect(), self.mouse) {
            return None;
        }
        let Some((row, y)) = self.scene_row_at() else {
            return Some(Destination {
                anchor: None,
                folder: None,
                after: true,
            });
        };
        let item = row.item();
        if let SceneItem::Folder(id) = item {
            if self.mouse[1] > y + 6. && self.mouse[1] < y + 21. {
                return Some(Destination {
                    anchor: None,
                    folder: Some(id),
                    after: true,
                });
            }
        }
        let folder = match item {
            SceneItem::Node(id) => self.editor.project.node_collection(id),
            SceneItem::Light(i) => self.editor.project.light_collection(i),
            SceneItem::Folder(_) => None,
        };
        Some(Destination {
            anchor: Some(item),
            folder,
            after: self.mouse[1] >= y + 13.5,
        })
    }

    pub(super) fn move_scene_items(&mut self, items: &[SceneItem], destination: Destination) {
        if items.is_empty()
            || items.iter().any(|item| self.scene_item_locked(*item))
            || destination
                .folder
                .is_some_and(|id| self.scene_item_locked(SceneItem::Folder(id)))
        {
            self.status = "Перемещение недоступно: объект или папка заблокированы.".into();
            return;
        }
        if destination
            .anchor
            .is_some_and(|anchor| items.contains(&anchor))
        {
            return;
        }
        if destination.folder.is_some()
            && items
                .iter()
                .any(|item| matches!(item, SceneItem::Folder(_)))
        {
            self.status = "Папки можно переставлять на верхнем уровне списка.".into();
            return;
        }
        // Do not imply a transform/skin reparent through a presentation-only drag.
        if let Some(SceneItem::Node(target)) = destination.anchor {
            let parent = self.editor.project.node(target).and_then(|n| n.parent);
            if items.iter().any(|item| match item {
                SceneItem::Node(id) => {
                    self.editor.project.node_collection(*id) == destination.folder
                        && self.editor.project.node(*id).and_then(|n| n.parent) != parent
                }
                _ => parent.is_some(),
            }) {
                self.status = "Переставляйте соседние объекты или переносите их в папку.".into();
                return;
            }
        }
        let mut order = self.editor.project.outliner.order.clone();
        let mut ordered: BTreeSet<_> = order.iter().copied().collect();
        let p = &self.editor.project;
        let membership = p.collection_membership();
        let mut baseline = vec![];
        for folder in std::iter::once(None).chain(p.collections.iter().map(|c| Some(c.id))) {
            if let Some(id) = folder {
                baseline.push(SceneItem::Folder(id));
            }
            baseline.extend(
                p.lights
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| p.light_collection(*i) == folder)
                    .map(|(i, _)| SceneItem::Light(i)),
            );
            baseline.extend(
                p.nodes
                    .iter()
                    .filter(|n| membership.get(&n.id).copied() == folder)
                    .map(|n| SceneItem::Node(n.id)),
            );
        }
        for item in baseline {
            if ordered.insert(item) {
                order.push(item);
            }
        }
        order.retain(|item| !items.contains(item));
        let at = destination
            .anchor
            .and_then(|target| order.iter().position(|item| *item == target))
            .map_or(order.len(), |i| i + usize::from(destination.after));
        order.splice(at..at, items.iter().copied());
        let roots: BTreeSet<_> = items
            .iter()
            .filter_map(|item| {
                if let SceneItem::Node(id) = item {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let branches = self.editor.project.descendants_of(&roots);
        if branches
            .iter()
            .any(|id| self.editor.project.node_locked(*id))
        {
            self.status = "Ветка содержит заблокированные объекты.".into();
            return;
        }
        if destination.folder.is_none()
            && roots.iter().any(|id| {
                self.editor
                    .project
                    .node(*id)
                    .and_then(|n| n.parent)
                    .is_some_and(|parent| self.editor.project.node_collection(parent).is_some())
            })
        {
            self.status =
                "Чтобы вынести ветку из папки, переместите её родительский объект.".into();
            return;
        }
        let lights: BTreeSet<_> = items
            .iter()
            .filter_map(|item| {
                if let SceneItem::Light(i) = item {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect();
        let result =
            self.editor.transact("Переместить в списке сцены", |p| {
                p.outliner.order = order;
                for folder in &mut p.collections {
                    folder.members.retain(|id| !branches.contains(id));
                    folder.light_members.retain(|i| !lights.contains(i));
                    if Some(folder.id) == destination.folder {
                        folder.members.extend(roots.iter().copied());
                        folder.light_members.extend(lights.iter().copied());
                    }
                }
            });
        if self.result(result).is_some() {
            if let Some(folder) = destination.folder {
                self.collapsed.remove(&folder);
            }
            self.status = "Порядок сцены изменён · Ctrl+Z — отменить".into();
            self.update_dirty();
        }
    }

    pub(super) fn drop_material_in_scene(&mut self, source: Option<u64>, material: u64) -> bool {
        if !ui::contains(self.scene_list_rect(), self.mouse) {
            return false;
        }
        let Some((row, y)) = self.scene_row_at() else {
            return true;
        };
        let (node, target_material) = match row {
            SceneEntry::Node(row) => (row.id, None),
            SceneEntry::Material(node, material, _) => (node, material),
            SceneEntry::Fluid(node, _) | SceneEntry::Cloth(node, _) | SceneEntry::Destruction(node, _) => (node, None),
            _ => {
                self.status = "Перенесите материал на примитив или его материал.".into();
                return true;
            }
        };
        if self.editor.project.node(node).and_then(|n| n.mesh).is_none() { return true; }
        if self.editor.project.node_locked(node) {
            self.status = "Объект заблокирован.".into();
            return true;
        }
        if source == Some(node) {
            if let Some(target) = target_material {
                if material == target {
                    return true;
                }
                let mut order = self.scene_material_ids(node);
                order.retain(|id| *id != material);
                if let Some(index) = order.iter().position(|id| *id == target) {
                    order.insert(index + usize::from(self.mouse[1] >= y + 13.5), material);
                    let result = self.editor.transact("Порядок материалов", |p| {
                        if let Some(n) = p.node_mut(node) {
                            n.material_layers.sort_by_key(|l| std::cmp::Reverse(order.iter().position(|id| *id == l.material).unwrap_or(usize::MAX)));
                        }
                        p.outliner.materials.insert(node, order);
                    });
                    self.result(result);
                }
            }
            return true;
        }
        let result = self.editor.assign_material_region(node, material, None);
        if self.result(result).is_some() {
            self.scene_tools.material_expanded.insert(node);
            self.collapsed.remove(&node);
            self.status = "Материал назначен · Ctrl+Z — отменить".into();
        }
        self.update_dirty();
        true
    }

    pub(super) fn finish_scene_drag(&mut self, drag: SceneDrag) {
        if (self.mouse[0] - drag.start[0]).hypot(self.mouse[1] - drag.start[1]) < 6. {
            self.action(drag.action);
            return;
        }
        if self.has_modal() || self.scene_write_pending() {
            return;
        }
        if self.editor.undo_len() != drag.history {
            self.status = "Сцена изменилась. Повторите перемещение.".into();
            return;
        }
        if drag
            .lights
            .iter()
            .any(|(i, l)| self.editor.project.lights.get(*i) != Some(l))
        {
            self.status = "Список света изменился. Повторите перемещение.".into();
            return;
        }
        match drag.source {
            Source::Items(items) => {
                if let Some(destination) = self.scene_destination() {
                    self.move_scene_items(&items, destination);
                }
            }
            Source::Material {
                node,
                mesh,
                material,
            } => {
                if self.editor.project.node(node).and_then(|n| n.mesh) != Some(mesh)
                    || !self.scene_material_ids(node).contains(&material)
                {
                    return;
                }
                if !self.drop_material_in_scene(Some(node), material) {
                    self.finish_material_drag(super::material_library::MaterialDrag {
                        action: Action::MaterialSelect(material),
                        start: drag.start,
                        target: Some(node),
                        add_layer: false,
                    });
                }
            }
        }
    }

    pub(super) fn scroll_scene_drag(&mut self) {
        let Some(Drag::Scene(d)) = &self.drag else {
            self.scene_tools.edge_scroll_since = None;
            return;
        };
        if (self.mouse[0] - d.start[0]).hypot(self.mouse[1] - d.start[1]) < 6. {
            return;
        }
        let r = self.scene_list_rect();
        let inside_x = self.mouse[0] >= r[0] && self.mouse[0] <= r[0] + r[2];
        let up = inside_x && self.mouse[1] >= r[1] - 10. && self.mouse[1] < r[1] + 15.;
        let down =
            inside_x && self.mouse[1] > r[1] + r[3] - 15. && self.mouse[1] < r[1] + r[3] + 10.;
        if !up && !down {
            self.scene_tools.edge_scroll_since = None;
            return;
        }
        let entered = self
            .scene_tools
            .edge_scroll_since
            .get_or_insert_with(std::time::Instant::now);
        // A quick drop on the last row must not retarget the following row.
        if entered.elapsed().as_millis() < 250
            || self
                .scene_tools
                .last_drag_scroll
                .is_some_and(|t| t.elapsed().as_millis() < 75)
        {
            return;
        }
        self.scene_tools.last_drag_scroll = Some(std::time::Instant::now());
        let previous = self.scene_scroll;
        if up {
            self.scene_scroll = self.scene_scroll.saturating_sub(1);
        } else {
            self.scene_scroll = (self.scene_scroll + 1).min(
                self.outliner_rows()
                    .len()
                    .saturating_sub(self.scene_capacity()),
            );
        }
        if self.scene_scroll != previous {
            self.redraw();
        }
    }

    pub(super) fn draw_scene_drag(&self, u: &mut Ui) {
        let Some(Drag::Scene(drag)) = &self.drag else {
            return;
        };
        if (self.mouse[0] - drag.start[0]).hypot(self.mouse[1] - drag.start[1]) < 6. {
            return;
        }
        let r = self.scene_list_rect();
        let p = u.palette();
        if let Some((row, y)) = self.scene_row_at() {
            let inside = matches!(row, SceneEntry::Collection(_))
                && self.mouse[1] > y + 6.
                && self.mouse[1] < y + 21.;
            let material = matches!(drag.source, Source::Material { .. });
            if inside || material && matches!(row, SceneEntry::Node(_)) {
                u.surface_outline([r[0] + 2., y, r[2] - 4., 25.], p.accent);
            } else {
                u.rect(
                    [
                        r[0] + 4.,
                        y + if self.mouse[1] >= y + 13.5 { 26. } else { 0. },
                        r[2] - 8.,
                        2.,
                    ],
                    p.accent,
                );
            }
        } else if ui::contains(r, self.mouse) {
            let count = self
                .outliner_rows()
                .len()
                .saturating_sub(self.scene_scroll)
                .min(self.scene_capacity());
            u.rect(
                [r[0] + 4., r[1] + count as f32 * 27., r[2] - 8., 2.],
                p.accent,
            );
        }
    }
}
