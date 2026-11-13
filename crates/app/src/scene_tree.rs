//! Scene outliner shared by node/camera hierarchy and independently authored lights.
//! A light row carries its displayed snapshot so a delayed hit cannot retarget a
//! different array entry after undo, deletion or replacement of the lighting rig.
use super::{empty_controls, fx_controls, App, SceneRow};
use crate::{
    icons::Icon,
    ui::{self, Action, Ui},
};
use poseforge_core::{Light, LightKind, MAX_ENABLED_LIGHTS};

#[derive(Clone, Debug)]
pub(super) enum SceneEntry {
    Collection(u64),
    Node(SceneRow),
    Light(usize),
    Material(u64, Option<u64>, usize),
    Modifier(u64, u64, usize),
    CameraEffect(u64, usize, usize),
    Fluid(u64, usize),
    Cloth(u64, usize),
    Destruction(u64, usize),
}

pub(super) fn light_label(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Directional => "Солнце",
        LightKind::Point => "Точечный свет",
        LightKind::Spot => "Прожектор",
        LightKind::Area => "Площадной свет",
    }
}

fn matches_light(light: &Light, query: &str) -> bool {
    light.name.to_lowercase().contains(query)
        || light_label(light.kind).to_lowercase().contains(query)
        || "источник света light".contains(query)
}

fn draw_light_icon(u: &mut Ui, x: f32, y: f32, kind: LightKind, color: [f32; 4]) {
    match kind {
        LightKind::Directional => Icon::Light.draw(u, [x, y, 18., 18.], color),
        LightKind::Point => {
            let points: Vec<_> = (0..16)
                .map(|step| {
                    let angle = step as f32 * std::f32::consts::TAU / 16.;
                    [x + 9. + 4.5 * angle.cos(), y + 7. + 4.5 * angle.sin()]
                })
                .collect();
            u.convex_polygon(&points, color);
            u.smooth_segment([x + 6., y + 14.], [x + 12., y + 14.], 1.5, color);
        }
        LightKind::Spot => {
            for (from, to) in [
                ([x + 9., y + 2.], [x + 3., y + 15.]),
                ([x + 3., y + 15.], [x + 15., y + 15.]),
                ([x + 15., y + 15.], [x + 9., y + 2.]),
            ] {
                u.smooth_segment(from, to, 1.5, color);
            }
        }
        LightKind::Area => {
            u.surface_outline([x + 2., y + 2., 14., 10.], color);
            u.smooth_segment([x + 9., y + 12.], [x + 9., y + 17.], 1.5, color);
        }
    }
}

impl App {
    fn folder_indent(&self, folder: Option<u64>) -> f32 {
        folder.map_or(0., |id| (self.editor.project.collection_chain(id).len() as f32 * 14.).min(98.))
    }
    pub(super) fn reveal_folder_chain(&mut self, id: u64) {
        for folder in self.editor.project.collection_chain(id) { self.collapsed.remove(&folder); }
    }
    pub(super) fn outliner_rows(&self) -> Vec<SceneEntry> {
        use std::collections::BTreeMap;
        let p = &self.editor.project;
        let query = self.scene_query.trim().to_lowercase();
        let nodes = self.scene_rows();
        if self.joints_only {
            return nodes.into_iter().map(SceneEntry::Node).collect();
        }
        let by_id: BTreeMap<_, _> = p.nodes.iter().map(|n| (n.id, n)).collect();
        let meshes: BTreeMap<_, _> = p.meshes.iter().map(|m| (m.id, m)).collect();
        let membership = p.collection_membership();
        let rank: BTreeMap<_, _> = p
            .outliner
            .order
            .iter()
            .enumerate()
            .map(|(i, item)| (*item, i))
            .collect();
        let rank_of =
            |block: &Vec<SceneEntry>| rank.get(&block[0].item()).copied().unwrap_or(usize::MAX);
        let mut groups: BTreeMap<Option<u64>, Vec<Vec<SceneEntry>>> = BTreeMap::new();
        if self.scene_tools.hidden_types & 4 == 0 {
            for (i, _) in p
                .lights
                .iter()
                .enumerate()
                .filter(|(_, l)| matches_light(l, &query))
            {
                groups
                    .entry(p.light_collection(i))
                    .or_default()
                    .push(vec![SceneEntry::Light(i)]);
            }
        }
        for row in nodes {
            let group = groups.entry(membership.get(&row.id).copied()).or_default();
            if let Some(block) = group.last_mut().filter(
                |block| matches!(&block[0], SceneEntry::Node(root) if root.depth < row.depth),
            ) {
                block.push(SceneEntry::Node(row));
            } else {
                group.push(vec![SceneEntry::Node(row)]);
            }
        }
        for blocks in groups.values_mut() {
            blocks.sort_by_key(rank_of);
        }
        let mut folders: Vec<_> = p.collections.iter().collect();
        folders.sort_by_key(|folder| std::cmp::Reverse(p.collection_chain(folder.id).len()));
        for folder in folders {
            let members = groups.remove(&Some(folder.id)).unwrap_or_default();
            if !query.is_empty()
                && members.is_empty()
                && !folder.name.to_lowercase().contains(&query)
            {
                continue;
            }
            let mut block = vec![SceneEntry::Collection(folder.id)];
            if !self.collapsed.contains(&folder.id) || !query.is_empty() {
                block.extend(members.into_iter().flatten());
            }
            groups.entry(folder.parent).or_default().push(block);
            groups.get_mut(&folder.parent).unwrap().sort_by_key(rank_of);
        }
        let mut roots = groups.remove(&None).unwrap_or_default();
        roots.sort_by_key(rank_of);
        let mut result = vec![];
        for entry in roots.into_iter().flatten() {
            let expanded = match &entry {
                SceneEntry::Node(row) => by_id
                    .get(&row.id)
                    .filter(|n| n.mesh.is_some() || n.camera.is_some())
                    .filter(|_| {
                        self.scene_tools.material_expanded.contains(&row.id)
                            || row.has_children && !self.collapsed.contains(&row.id)
                    })
                    .map(|node| (*node, row.depth + 1)),
                _ => None,
            };
            result.push(entry);
            if let Some((node, depth)) = expanded {
                let id = node.id;
                if let Some(lens) = node.camera {
                    result.extend(lens.effects.ordered_fields().map(|field| SceneEntry::CameraEffect(id, field, depth)));
                }
                if node.mesh.is_none() { continue; }
                result.extend(node.modifiers.iter().map(|m|SceneEntry::Modifier(id,m.id,depth)));
                let materials = node.mesh.and_then(|id| meshes.get(&id))
                    .map(|mesh| self.scene_material_ids_for(node, mesh)).unwrap_or_default();
                result.extend(materials.into_iter().map(|mat| SceneEntry::Material(id, Some(mat), depth)));
                if node.fluid.is_some() {
                    result.push(SceneEntry::Fluid(id, depth));
                }
                if node.physics.is_some_and(|s| s.kind == poseforge_core::PhysicsKind::Cloth) {
                    result.push(SceneEntry::Cloth(id, depth));
                }
                if node.destruction.is_some() { result.push(SceneEntry::Destruction(id, depth)); }
            }
        }
        result
    }

    pub(super) fn scene_node_offset(&self, id: u64) -> usize {
        self.outliner_rows()
            .iter()
            .position(|row| matches!(row, SceneEntry::Node(node) if node.id == id))
            .unwrap_or(0)
    }

    pub(super) fn reveal_scene_light(&mut self, index: usize) {
        self.preferences.left_panel = true;
        self.joints_only = false;
        self.scene_tools.hidden_types &= !4;
        if let Some(id) = self.editor.project.light_collection(index) {
            self.reveal_folder_chain(id);
        }
        self.scene_query.clear();
        if let Some(position) = self
            .outliner_rows()
            .iter()
            .position(|row| matches!(row, SceneEntry::Light(light) if *light == index))
        {
            let capacity = self.scene_capacity();
            if position < self.scene_scroll {
                self.scene_scroll = position;
            } else if position >= self.scene_scroll + capacity {
                self.scene_scroll = position.saturating_sub(capacity - 1);
            }
        }
    }

    pub(super) fn reveal_scene_node(&mut self, id: u64) {
        self.joints_only = false;
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            self.collapsed.remove(&current);
            if let Some(folder) = self.editor.project.node_collection(current) {
                self.reveal_folder_chain(folder);
            }
            cursor = self.editor.project.node(current).and_then(|n| n.parent);
        }
        // Keep a useful search/filter whenever it already includes the selection.
        if !self
            .outliner_rows()
            .iter()
            .any(|row| matches!(row, SceneEntry::Node(n) if n.id == id))
        {
            self.scene_query.clear();
            let kind = self.editor.project.node(id).map_or(1, |n| {
                if n.camera.is_some() {
                    8
                } else if n.is_joint {
                    2
                } else {
                    1
                }
            });
            self.scene_tools.hidden_types &= !kind;
        }
        let position = self.scene_node_offset(id);
        let capacity = self.scene_capacity();
        if position < self.scene_scroll {
            self.scene_scroll = position;
        } else if position >= self.scene_scroll + capacity {
            self.scene_scroll = position.saturating_sub(capacity - 1);
        }
    }

    /// Selection does not modify the document. Toggling an eye is a separate
    /// transaction and keeps both the node selection and the light selection.
    pub(super) fn scene_tree_action(&mut self, action: &Action) -> bool {
        if let Action::SceneShadowOnly(id) = action {
            if !self.has_modal() && !self.scene_write_pending() && !self.editor.has_staged()
                && self.editor.selected == Some(*id) && self.node_selectable(*id)
                && self.editor.project.node(*id).is_some_and(|n| n.mesh.is_some() && n.curve.is_none())
            {
                let result = self.editor.transact("Только тень", |p| {
                    let node = p.node_mut(*id).unwrap();
                    node.shadow_only = !node.shadow_only;
                });
                self.result(result);
            }
            return true;
        }
        if let Action::SceneToggleNode(id) = action {
            if !self.has_modal()
                && !self.scene_write_pending()
                && self.editor.project.node(*id).is_some()
            {
                let result = self.editor.transact("Видимость объекта", |p| {
                    let node = p.node_mut(*id).unwrap();
                    node.visible = !node.visible;
                });
                self.result(result);
            }
            return true;
        }
        if matches!(
            action,
            Action::Undo | Action::Redo | Action::LightingPreset(_)
        ) {
            // Lights are stored as an array, not stable-ID nodes. History can
            // replace/reorder that array; do not inherit a different light at
            // the formerly selected index.
            self.solo_light = None;
            self.light_aim = None;
            self.clear_light_selection();
            return false;
        }
        if let Action::SceneSelectLight(index, expected)
        | Action::SceneToggleLight(index, expected) = action
        {
            if self.has_modal() || self.job.is_some() {
                return true;
            }
            if self.editor.project.lights.get(*index) != Some(expected.as_ref()) {
                self.status = "Список света обновился. Выберите источник ещё раз.".into();
                return true;
            }
            if matches!(action, Action::SceneSelectLight(..)) {
                self.select_light(*index);
            } else {
                self.toggle_scene_light(*index);
            }
            return true;
        }
        let Some(index) = self.active_light_index() else {
            return false;
        };
        match action {
            Action::ToggleVisible | Action::Duplicate if self.has_modal() || self.job.is_some() => {
                true
            }
            Action::ToggleVisible => {
                self.toggle_scene_light(index);
                true
            }
            Action::Duplicate => {
                let mut copy = self.editor.project.lights[index].clone();
                let base = format!("{} — копия", copy.name);
                copy.name.clone_from(&base);
                let mut suffix = 2;
                while self
                    .editor
                    .project
                    .lights
                    .iter()
                    .any(|light| light.name == copy.name)
                {
                    copy.name = format!("{base} {suffix}");
                    suffix += 1;
                }
                let disabled_for_limit = copy.enabled
                    && self
                        .editor
                        .project
                        .lights
                        .iter()
                        .filter(|light| light.enabled)
                        .count()
                        >= MAX_ENABLED_LIGHTS;
                if disabled_for_limit {
                    copy.enabled = false;
                }
                let new_index = self.editor.project.lights.len();
                let result = self.editor.transact("Дублировать свет", |project| {
                    project.lights.push(copy);
                });
                if self.result(result).is_some() {
                    self.select_light(new_index);
                    self.reveal_scene_light(new_index);
                    if disabled_for_limit {
                        self.status =
                            "Копия света создана выключенной: уже включены восемь источников."
                                .into();
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn toggle_scene_light(&mut self, index: usize) {
        let result =
            self.editor
                .transact("Включить / выключить свет", |project| {
                    if let Some(light) = project.lights.get_mut(index) {
                        light.enabled = !light.enabled;
                    }
                });
        if self.result(result).is_some() {
            self.status = if self.editor.project.lights[index].enabled {
                "Источник света включён."
            } else {
                "Источник света выключен."
            }
            .into();
        }
    }

    pub(super) fn draw_scene_tree(&mut self, u: &mut Ui) {
        if !self.scene_panel_visible() {
            return;
        }
        let p = u.palette();
        let r = self.left_panel_rect();
        u.outlined_surface(r, p.panel, p.border);
        u.quiet_button(
            [r[0] + 4., r[1] + 4., 28., 28.],
            if self.preferences.left_panel {
                "−"
            } else {
                "+"
            },
            Action::ToggleLeftPanel,
            false,
        );
        u.section_label([r[0] + 36., r[1] + 12., 70., 16.], "СЦЕНА", 12.);
        u.quiet_button(
            [r[0] + r[2] - 38., r[1] + 4., 28., 28.],
            "…",
            Action::DropdownOpen(crate::ui::DropdownKind::SceneActions),
            false,
        );
        if self.preferences.left_panel {
            u.quiet_icon_button(
                [r[0] + r[2] - 102., r[1] + 4., 28., 28.],
                "",
                Action::CreateCamera,
                false,
                Icon::Camera,
            );
            u.quiet_icon_button(
                [r[0] + r[2] - 70., r[1] + 4., 28., 28.],
                "",
                Action::SceneTool(crate::ui::SceneTool::NewCollection),
                false,
                Icon::FolderAdd,
            );
        }
        let count = self.editor.project.nodes.len() + self.editor.project.lights.len();
        u.label(r[0] + 98., r[1] + 13., count.to_string(), 10., p.muted);
        if !self.scene_panel_visible() {
            return;
        }
        u.field(
            [r[0] + 12., r[1] + 39., r[2] - 62., 28.],
            if self.scene_query.is_empty() {
                "Поиск · Ctrl+F"
            } else {
                &self.scene_query
            },
            Action::SearchScene,
        );
        u.quiet_icon_button(
            [r[0] + r[2] - 44., r[1] + 39., 32., 28.],
            "",
            Action::DropdownOpen(crate::ui::DropdownKind::SceneFilter),
            self.scene_tools.hidden_types != 0
                || self.dropdown_is_open(crate::ui::DropdownKind::SceneFilter),
            Icon::Filter,
        );
        let lw = self.left_width() - 12.;
        u.surface_outline(
            [r[0] + 8., r[1] + 35., r[2] - 16., (r[3] - 43.).max(28.)],
            p.border,
        );
        u.hits.push(ui::Hit { rect: [r[0] + 8., r[1] + 74., r[2] - 24., (r[3] - 82.).max(1.)],
            action: Action::SceneTool(crate::ui::SceneTool::Clear) });
        let rows = self.outliner_rows();
        let cap = self.scene_capacity();
        self.scene_scroll = self.scene_scroll.min(rows.len().saturating_sub(cap));
        let offset = self.scene_scroll;
        for (i, row) in rows.iter().skip(offset).take(cap).enumerate() {
            let y = r[1] + 74. + i as f32 * 27.;
            let r = [24., y, lw - 24., 25.];
            match row {
                SceneEntry::Collection(id) => {
                    use crate::ui::SceneTool as S;
                    let Some(collection) =
                        self.editor.project.collections.iter().find(|c| c.id == *id)
                    else {
                        continue;
                    };
                    let indent = (self.editor.project.collection_chain(*id).len().saturating_sub(1) as f32 * 14.).min(84.);
                    let selected = self.scene_tools.collection == Some(*id);
                    if selected {
                        u.surface_outline(r, p.accent);
                    }
                    u.disclosure_button(
                        [28. + indent, y + 1., 22., 23.],
                        self.scene_query
                            .is_empty()
                            .then_some(Action::SceneTool(S::CollectionCollapse(*id))),
                        !self.collapsed.contains(id) || !self.scene_query.is_empty(),
                    );
                    u.quiet_icon_button(
                        [51. + indent, y + 1., 24., 23.],
                        "",
                        Action::SceneTool(S::CollectionSelect(*id)),
                        false,
                        Icon::Folder,
                    );
                    let name_action = Action::SceneTool(S::CollectionName(*id));
                    let naming = self
                        .input
                        .as_ref()
                        .is_some_and(|i| i.purpose == name_action);
                    let rect = [78. + indent, y + 1., (lw - 165. - indent).max(16.), 23.];
                    if naming {
                        u.field(rect, &collection.name, name_action);
                    } else {
                        u.fitted_label(
                            [rect[0], y + 5., rect[2], 17.],
                            &collection.name,
                            12.,
                            if collection.visible { p.text } else { p.muted },
                        );
                        u.hits.push(ui::Hit {
                            rect,
                            action: Action::SceneTool(S::CollectionSelect(*id)),
                        });
                    }
                    u.quiet_icon_button(
                        [lw - 80., y + 1., 24., 23.],
                        "",
                        Action::SceneTool(S::CollectionLock(*id)),
                        collection.locked,
                        Icon::Lock,
                    );
                    u.eye_button(
                        [lw - 54., y + 1., 24., 23.],
                        Action::SceneTool(S::CollectionVisible(*id)),
                        collection.visible,
                    );
                    u.scene_close_button(
                        [lw - 28., y + 1., 24., 23.],
                        Action::SceneDelete(poseforge_core::SceneItem::Folder(*id)),
                    );
                }
                SceneEntry::Light(index) => {
                    let light = &self.editor.project.lights[*index];
                    let indent = self.folder_indent(self.editor.project.light_collection(*index));
                    let selected = self.active_light_index() == Some(*index);
                    if selected {
                        u.surface_outline(r, p.accent);
                    } else if ui::contains(r, self.mouse) {
                        u.surface(r, p.hover);
                    }
                    let color = if !light.enabled {
                        p.muted
                    } else if selected {
                        p.accent
                    } else {
                        p.text
                    };
                    draw_light_icon(u, 30. + indent, y + 3., light.kind, color);
                    u.fitted_label(
                        [54. + indent, y + 5., lw - 113. - indent, 17.],
                        &light.name,
                        12.,
                        color,
                    );
                    u.hits.push(ui::Hit {
                        rect: [24., y, lw - 79., 25.],
                        action: Action::SceneSelectLight(*index, Box::new(light.clone())),
                    });
                    u.eye_button(
                        [lw - 53., y + 1., 25., 23.],
                        Action::SceneToggleLight(*index, Box::new(light.clone())),
                        light.enabled,
                    );
                    u.scene_close_button(
                        [lw - 27., y + 1., 25., 23.],
                        Action::SceneDeleteLight(*index, Box::new(light.clone())),
                    );
                }
                SceneEntry::Cloth(node, depth) | SceneEntry::Destruction(node, depth) => {
                    let Some(owner) = self.editor.project.node(*node) else { continue; };
                    let cloth = matches!(row, SceneEntry::Cloth(..));
                    let (visible, label, icon, tab, select, eye, remove) = if cloth {
                        let Some(settings) = owner.physics else { continue; };
                        (settings.visible, "Ткань", Icon::Cloth, 11, Action::SceneCloth(*node), Action::SceneClothVisible(*node), Action::SceneClothRemove(*node))
                    } else {
                        let Some(settings) = owner.destruction else { continue; };
                        (settings.visible, "Разрушение", Icon::Fracture, 13, Action::SceneDestruction(*node), Action::SceneDestructionVisible(*node), Action::SceneDestructionRemove(*node))
                    };
                    let x = 52. + (*depth).min(5) as f32 * 10. + self.folder_indent(self.editor.project.node_collection(*node));
                    if self.editor.selected == Some(*node) && self.surface.tab == tab { u.surface_outline(r, p.accent); }
                    else if ui::contains(r, self.mouse) { u.surface(r, p.hover); }
                    let color = if visible { p.text } else { p.muted };
                    icon.draw(u, [x + 2., y + 4., 18., 18.], color);
                    u.fitted_label([x + 28., y + 5., (lw - x - 88.).max(1.), 17.], label, 11., color);
                    u.hits.push(ui::Hit { rect: r, action: select });
                    u.eye_button([lw - 54., y + 1., 25., 23.], eye, visible);
                    u.scene_close_button([lw - 27., y + 1., 25., 23.], remove);
                }
                SceneEntry::Fluid(node, depth) => {
                    let Some(settings) = self.editor.project.node(*node).and_then(|n| n.fluid) else { continue; };
                    let x = 52. + (*depth).min(5) as f32 * 10.
                        + self.folder_indent(self.editor.project.node_collection(*node));
                    let selected = self.editor.selected == Some(*node) && self.surface.tab == 4;
                    if selected { u.surface_outline(r, p.accent); }
                    else if ui::contains(r, self.mouse) { u.surface(r, p.hover); }
                    let color = if settings.visible { p.text } else { p.muted };
                    Icon::Fluid.draw(u, [x + 2., y + 4., 18., 18.], color);
                    u.fitted_label([x + 28., y + 5., (lw - x - 88.).max(1.), 17.], "Жидкость", 11., color);
                    u.hits.push(ui::Hit { rect: r, action: Action::SceneFluid(*node) });
                    u.eye_button([lw - 54., y + 1., 25., 23.], Action::SceneFluidVisible(*node), settings.visible);
                    u.scene_close_button([lw - 27., y + 1., 25., 23.], Action::SceneFluidRemove(*node));
                }
                SceneEntry::CameraEffect(node, field, depth) => {
                    let Some(effects) = self.editor.project.node(*node).and_then(|n| n.camera).map(|lens| lens.effects) else { continue; };
                    if !effects.contains(*field) { continue; }
                    let x = 52. + (*depth).min(5) as f32 * 10. + self.folder_indent(self.editor.project.node_collection(*node));
                    let selected = self.camera_effect_selected(*node, *field) && self.surface.tab == 0;
                    if selected { u.surface_outline(r, p.accent); } else if ui::contains(r, self.mouse) { u.surface(r, p.hover); }
                    let on = effects.effect_enabled(*field);
                    let color = if on && effects.enabled { p.text } else { p.muted };
                    Icon::Wand.draw(u, [x + 2., y + 4., 18., 18.], color);
                    u.fitted_label([x + 28., y + 5., (lw - x - 88.).max(1.), 17.], poseforge_core::CameraEffects::LABELS[*field - 4], 11., color);
                    u.hits.push(ui::Hit { rect: r, action: Action::CameraEffect(*node, ui::CameraTool::SelectEffect(*field)) });
                    self.draw_camera_effect_buttons(u, [r[0], y, lw + 3. - r[0], 25.], *node, *field, on);
                }
                SceneEntry::Modifier(node,id,depth) => {
                    let Some(modifier)=self.editor.project.node(*node).and_then(|n|n.modifiers.iter().find(|m|m.id==*id)) else {continue;};
                    let x=52.+(*depth).min(5) as f32*10.+self.folder_indent(self.editor.project.node_collection(*node));
                    let selected=self.editor.selected==Some(*node) && self.procedural.selected==Some((*node,*id)) && self.surface.tab==8;
                    if selected {u.surface_outline(r,p.accent);}else if ui::contains(r,self.mouse) {u.surface(r,p.hover);}
                    let color=if modifier.enabled {p.text}else{p.muted};
                    Icon::Modifiers.draw(u,[x+2.,y+4.,18.,18.],color);
                    u.fitted_label([x+28.,y+5.,(lw-x-88.).max(1.),17.],&modifier.name,11.,color);
                    u.hits.push(ui::Hit {rect:r,action:Action::Modifier(ui::ModifierAction::Select(*node,*id))});
                    if self.modifier_editable(*node) {
                        u.eye_button([lw-54.,y+1.,25.,23.],Action::Modifier(ui::ModifierAction::Toggle(*node,*id)),modifier.enabled);
                        u.scene_close_button([lw-27.,y+1.,25.,23.],Action::Modifier(ui::ModifierAction::Remove(*node,*id)));
                    }
                }
                SceneEntry::Material(node, material, depth) => {
                    let x = 52.
                        + (*depth).min(5) as f32 * 10.
                        + self.folder_indent(self.editor.project.node_collection(*node));
                    if let Some(mat) = material.and_then(|id| self.editor.project.material(id)) {
                        let selected = self.editor.selected == Some(*node)
                            && self.selected_material_id() == Some(mat.id)
                            && self.surface.tab == 1
                            && !self.surface.material_library;
                        if selected {
                            u.surface_outline(r, p.accent);
                        } else if ui::contains(r, self.mouse) {
                            u.surface(r, p.hover);
                        }
                        self.material_previews.draw(
                            u,
                            [x, y + 2., 22., 22.],
                            mat,
                            &self.editor.project.textures,
                        );
                        u.fitted_label(
                            [x + 28., y + 5., (lw - x - 88.).max(1.), 17.],
                            &mat.name,
                            11.,
                            p.text,
                        );
                        u.hits.push(ui::Hit {
                            rect: r,
                            action: Action::SceneMaterial(*node, mat.id),
                        });
                        let layer = self.editor.project.node(*node).and_then(|n| n.material_layers.iter().find(|l| l.material == mat.id));
                        u.eye_button([lw - 54., y + 1., 25., 23.],
                            if layer.is_some() { Action::MaterialLayerVisible(*node, mat.id) } else { Action::SceneMaterialVisible(mat.id) },
                            mat.visible && layer.is_none_or(|l| l.visible));
                        u.scene_close_button([lw - 27., y + 1., 25., 23.], Action::SceneMaterialDetach(*node, mat.id));
                    } else {
                        u.fitted_label(
                            [x, y + 5., (lw - x - 6.).max(20.), 17.],
                            "Нет назначенных материалов",
                            11.,
                            p.muted,
                        );
                    }
                }
                SceneEntry::Node(row) => {
                    let Some(node) = self.editor.project.node(row.id) else {
                        continue;
                    };
                    let selected = self.object_selected(node.id);
                    let indent = row.depth.min(5) as f32 * 10.
                        + self.folder_indent(self.editor.project.node_collection(row.id));
                    if selected {
                        u.surface_outline(r, p.accent);
                    } else if ui::contains(r, self.mouse) {
                        u.surface(r, p.hover);
                    }
                    let x = 30. + indent;
                    if row.has_children || node.mesh.is_some() || node.camera.is_some() {
                        if self.scene_query.is_empty() && !self.joints_only {
                            u.disclosure_button(
                                [x, y + 2., 22., 21.],
                                Some(Action::SceneExpand(node.id)),
                                if row.has_children {
                                    !self.collapsed.contains(&node.id)
                                } else {
                                    self.scene_tools.material_expanded.contains(&node.id)
                                },
                            );
                        } else {
                            u.disclosure_button([x, y + 2., 22., 21.], None, true);
                        }
                    }
                    let color = if selected {
                        p.accent
                    } else if !node.visible || !row.matches {
                        p.muted
                    } else {
                        p.text
                    };
                    let icon = if node.camera.is_some() {
                        Icon::Camera
                    } else if node.is_joint || (row.has_children && self.is_skeleton_object(node.id)) {
                        Icon::Bone
                    } else if node.curve.is_some() || node.mesh.and_then(|id| self.editor.project.mesh(id)).and_then(|m| m.topology.as_ref())
                        .and_then(|t| t.primitive).is_some_and(|p| p.is_curve()) {
                        Icon::Curve
                    } else if let Some(fx)=node.fx {fx_controls::fx_icon(fx.kind)
                    } else if node.force_field.is_some() {empty_controls::field_icon(node.force_field.unwrap().kind)} else if node.empty {Icon::Empty
                    } else if node.text.is_some() {Icon::Text
                    } else if node.mesh.is_some() {
                        Icon::Grid
                    } else {
                        Icon::Folder
                    };
                    icon.draw(u, [x + 25., y + 4., 16., 16.], color);
                    u.fitted_label(
                        [x + 46., y + 5., lw - indent - 132., 17.],
                        &node.name,
                        12.,
                        color,
                    );
                    u.hits.push(ui::Hit {
                        rect: [x + 24., y, lw - indent - 109., 25.],
                        action: Action::Select(node.id),
                    });
                    u.eye_button(
                        [lw - 53., y + 1., 25., 23.],
                        Action::SceneToggleNode(node.id),
                        node.visible,
                    );
                    u.scene_close_button(
                        [lw - 27., y + 1., 25., 23.],
                        Action::SceneDelete(poseforge_core::SceneItem::Node(node.id)),
                    );
                }
            }
        }

        if rows.is_empty() {
            u.label(r[0] + 16., r[1] + 79., "Ничего не найдено", 12., p.muted);
        }
        let track = self.object_scrollbar_rect();
        if rows.len() > cap {
            u.vertical_scrollbar(
                track,
                offset as f32 / (rows.len() - cap) as f32,
                cap as f32 / rows.len() as f32,
                Action::ObjectScroll,
            );
        }
    }
}
