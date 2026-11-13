//! Material authoring with compact assigned slots and a searchable library popup.
use super::surfaces::{
    color_hex, emissive_tint, linear_to_srgb, material_number, numeric, texture_slot,
};
use super::App;
use crate::icons::Icon;
use crate::ui::{Action, DropdownKind, SliderRange, Ui};
use poseforge_core::{MaterialAssignment, MaterialPreset};
use winit::keyboard::{Key, NamedKey};

pub(super) const MATERIAL_LIST_Y: f32 = 211.;
pub(super) const EMISSION_EXTRA_HEIGHT: f32 = 40.;
const MATERIAL_FOOTER_HEIGHT: f32 = 40.;


impl App {
    pub(super) fn material_faces_available(&self) -> bool {
        self.authoring.selection_mode == crate::ui::MeshSelection::Face
            && !self.authoring.faces.is_empty()
            && self
                .authoring
                .editing
                .and_then(|id| self.editor.project.node(id))
                .and_then(|n| n.mesh)
                == self.surface_mesh_id()
            && self.surface_mesh_id().is_some()
    }
    pub(super) fn material_locked(&self) -> bool {
        if self
            .editor
            .selected
            .is_some_and(|id| self.editor.project.node_locked(id))
        {
            return true;
        }
        let mut cursor = self.editor.selected;
        let mut seen = std::collections::BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return true;
            }
            let Some(node) = self.editor.project.node(id) else {
                return true;
            };
            if node.locked {
                return true;
            }
            cursor = node.parent;
        }
        false
    }

    pub(super) fn material_owners(&self, mesh: u64) -> Vec<u64> {
        self.editor
            .project
            .nodes
            .iter()
            .filter(|node| {
                node.mesh == Some(mesh)
                    && self
                        .editor
                        .selected
                        .is_some_and(|root| self.is_descendant(node.id, root))
            })
            .map(|node| node.id)
            .collect()
    }

    pub(super) fn material_authoring_available(&self) -> bool {
        self.job.is_none()
            && self.native_picker.is_none()
            && !self.has_modal()
            && !self.material_locked()
    }

    pub(super) fn remember_material_assignment(
        &mut self,
        assignment: MaterialAssignment,
        status: &str,
    ) {
        self.surface.mesh = Some(assignment.mesh);
        self.surface.selection = self.editor.selected;
        self.surface.object_material = Some((assignment.mesh, assignment.material));
        self.reveal_material_row(assignment.material);
        self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
        self.status = status.into();
    }

    pub(super) fn material_picker_active(&self) -> bool {
        self.material_dropdown_active()
    }

    /// Only these commands create or rebind material resources. All mutations
    /// go through Core so shared mesh instances retain their ownership rules.
    pub(super) fn material_action(&mut self, action: &Action) -> bool {
        if !matches!(
            action,
            Action::SurfaceCreateMaterial
                | Action::SurfaceUnassignMaterial
                | Action::SurfacePruneMaterials
                | Action::SurfaceUniqueMaterial
                | Action::SurfaceCreateMaterialPreset(_)
                | Action::SurfaceResetMapping
                | Action::SurfaceNprToggle
                | Action::SurfaceArtisticToggle
                | Action::SurfaceArtisticNextStyle
                | Action::SurfaceArtisticReset
                | Action::SurfaceMaterialName
                | Action::SurfaceMaterialList
                | Action::SurfaceAssignMaterial(_)
                | Action::MaterialAssign(..)
                | Action::SurfacePreset(_)
        ) {
            return false;
        }
        if matches!(
            action,
            Action::SurfaceUnassignMaterial | Action::SurfacePruneMaterials
        ) && self.surface.tab != 1
        {
            return true;
        }
        if !self.material_authoring_available() {
            self.status = if self.material_locked() {
                "Объект или его родитель заблокирован."
            } else {
                "Завершите текущую операцию перед изменением материала."
            }
            .into();
            return true;
        }
        if !self.surface.material_library && self.surface_mesh_id().is_none() {
            return true;
        }
        if let Action::SurfaceCreateMaterialPreset(index) = *action {
            if let (Some(node), Some(&preset)) = (self.material_layer_owner(), MaterialPreset::ALL.get(index)) {
                let result = self.editor.add_material_layer_preset(node, preset);
                if let Some(id) = self.result(result) {
                    self.select_added_material(node, id);
                }
                return true;
            }
        }
        if matches!(action, Action::SurfaceCreateMaterial | Action::SurfaceUniqueMaterial) && self.material_layer_owner().is_some() {
            self.append_material_layer(if *action == Action::SurfaceUniqueMaterial { self.selected_material_id() } else { None });
            return true;
        }
        let mesh = self.surface_mesh_id().unwrap_or(0);
        let owners = self.material_owners(mesh);
        match *action {
            Action::SurfaceUnassignMaterial => {
                if let (Some(node), Some(material)) = (self.material_layer_owner(), self.selected_material_id()) {
                    self.action(Action::SceneMaterialDetach(node, material));
                    return true;
                }
                let result = self.editor.unassign_material(mesh, &owners);
                if let Some(mesh) = self.result(result) {
                    self.surface.mesh = Some(mesh);
                    self.surface.selection = self.editor.selected;
                    self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
                    self.status = "Материал снят с объекта и сохранён в библиотеке.".into();
                }
            }
            Action::SurfacePruneMaterials => {
                let result = self.editor.remove_unused_materials();
                if let Some(count) = self.result(result) {
                    self.surface.material_offset = 0;
                    self.status =
                        format!("Удалено неиспользуемых материалов: {count}. Ctrl+Z — отменить.");
                }
            }
            Action::SurfaceArtisticToggle | Action::SurfaceArtisticNextStyle | Action::SurfaceArtisticReset => {
                if let Some(id) = self.selected_material_id() {
                    let result = self.editor.transact("Художественная поверхность", |p| {
                        if let Some(m) = p.materials.iter_mut().find(|m| m.id == id) {
                            match *action {
                                Action::SurfaceArtisticToggle => m.artistic.enabled = !m.artistic.enabled,
                                Action::SurfaceArtisticNextStyle => {
                                    // Change the pattern, not the user's color, PBR response or sliders.
                                    m.artistic.style = m.artistic.style.next();
                                },
                                Action::SurfaceArtisticReset => {
                                    let enabled = m.artistic.enabled;
                                    m.artistic = poseforge_core::ArtisticSettings::for_style(m.artistic.style);
                                    m.artistic.enabled = enabled;
                                },
                                _ => {},
                            }
                        }
                    });
                    if self.result(result).is_some() {
                        self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
                        self.status = "Поверхность обновлена · Ctrl+Z — отменить".into();
                    }
                }
            }
            Action::SurfaceNprToggle => {
                if let Some(id) = self.selected_material_id() {
                    let result = self.editor.transact("Переключить NPR", |p| {
                        if let Some(m) = p.materials.iter_mut().find(|m| m.id == id) {
                            m.npr.enabled = !m.npr.enabled;
                        }
                    });
                    self.result(result);
                }
            }
            Action::SurfaceResetMapping => {
                if let Some(id) = self.selected_material_id() {
                    let result =
                        self.editor.transact("Сбросить UV материала", |p| {
                            if let Some(m) = p.materials.iter_mut().find(|m| m.id == id) {
                                m.uv_scale = [1.; 2];
                                m.uv_offset = [0.; 2];
                                m.uv_rotation = 0.;
                            }
                        });
                    self.result(result);
                }
            }
            Action::SurfaceCreateMaterial | Action::SurfaceCreateMaterialPreset(_) => {
                let preset = if let Action::SurfaceCreateMaterialPreset(index) = *action {
                    let Some(preset) = MaterialPreset::ALL.get(index) else {
                        return true;
                    };
                    *preset
                } else {
                    MaterialPreset::Matte
                };
                if self.surface.material_library {
                    let result = self.editor.create_library_material(preset);
                    if let Some(id) = self.result(result) {
                        self.surface.library_material = Some(id);
                        self.status = "Материал создан в библиотеке.".into();
                    }
                    return true;
                }
                let result = if self.material_faces_available() {
                    if self.editor.has_active_edit() {
                        return true;
                    }
                    self.editor.begin_edit("Создать материал для граней");
                    let created =
                        self.editor
                            .create_library_material(preset)
                            .and_then(|material| {
                                self.editor.assign_face_material(
                                    mesh,
                                    &owners,
                                    &self.authoring.faces,
                                    material,
                                )
                            });
                    match created {
                        Ok(assignment) => self.editor.commit_edit().map(|()| assignment),
                        Err(error) => {
                            self.editor.cancel_edit();
                            Err(error)
                        }
                    }
                } else {
                    self.editor.create_material(mesh, &owners, preset)
                };
                if let Some(assignment) = self.result(result) {
                    self.remember_material_assignment(
                        assignment,
                        "Создан новый материал. Двойной щелчок по строке — переименовать.",
                    );
                }
            }
            Action::SurfaceUniqueMaterial => {
                let Some(source) = self.selected_material_id() else {
                    return true;
                };
                if self.surface.material_library {
                    let result = self.editor.duplicate_library_material(source);
                    if let Some(id) = self.result(result) {
                        self.surface.library_material = Some(id);
                    }
                    return true;
                }
                let result = self
                    .editor
                    .duplicate_assigned_material(mesh, &owners, source);
                if let Some(assignment) = self.result(result) {
                    self.remember_material_assignment(
                        assignment,
                        "Создана независимая копия материала.",
                    );
                }
            }
            Action::SurfaceMaterialName => {
                if let Some(name) = self
                    .selected_material_id()
                    .and_then(|id| self.editor.project.material(id))
                    .map(|m| m.name.clone())
                {
                    if self.preferences.collapsed_sections.remove(&100) {
                        self.persist_preferences();
                    }
                    self.inspector_scroll = 0.;
                    let (rows, capacity) = self.material_scroll_rows();
                    if let Some(index) = rows.iter().position(|row| matches!(row, super::material_users::MaterialRow::Material(id) if Some(*id) == self.selected_material_id())) {
                        let offset = index.saturating_sub(capacity - 1);
                        self.set_material_offset(offset);
                        let start = MATERIAL_LIST_Y;
                        let y = start + 6. + (index-offset) as f32*self.material_row_height();
                        if let Some(y) = self.property_y(y) {
                            let viewport = self.properties_content_rect();
                            self.inspector_scroll = (y-super::properties::CONTENT_SHIFT+38.-viewport[1]-viewport[3]).max(0.).min(self.inspector_max_scroll());
                        }
                    }
                    self.input(action.clone(), name);
                }
            }
            Action::SurfaceMaterialList => {
                self.open_dropdown(DropdownKind::MaterialLibrary);
            }
            Action::SurfaceAssignMaterial(material) | Action::MaterialAssign(material, _, _) => {
                if matches!(action, Action::SurfaceAssignMaterial(_))
                    && !self.material_picker_active()
                {
                    return true;
                }
                if let Action::MaterialAssign(_, source, selection) = action {
                    if *source != mesh || *selection != self.editor.selected {
                        return true;
                    }
                }
                let result = if matches!(action, Action::SurfaceAssignMaterial(_))
                    && self.material_faces_available()
                {
                    self.editor
                        .assign_face_material(mesh, &owners, &self.authoring.faces, material)
                } else {
                    self.editor.assign_material(mesh, &owners, material)
                };
                if let Some(assignment) = self.result(result) {
                    self.remember_material_assignment(
                        assignment,
                        "Материал назначен. Общие параметры изменяют все его объекты.",
                    );
                }
            }
            Action::SurfacePreset(index) => {
                if let (Some(material), Some(&preset)) =
                    (self.selected_material_id(), MaterialPreset::ALL.get(index))
                {
                    let result = self.editor.apply_material_preset(material, preset);
                    if self.result(result).is_some() {
                        self.status = format!(
                            "Основа «{}»: цвет, карты и свечение сохранены.",
                            preset.label()
                        );
                    }
                }
            }
            _ => {}
        }
        true
    }

    /// Alt+Down opens the searchable library. Arrows only move a candidate; committing
    /// is explicit. Text composition and all unrelated editing shortcuts retain
    /// ownership until a valid draft can be resolved.
    pub(super) fn material_key(&mut self, key: &Key, ctrl: bool, alt: bool, repeat: bool) -> bool {
        if self.material_dropdown_active() { return self.dropdown_key(key, false, ctrl, alt); }
        if self.surface.tab == 0
            && self.active_light_index().is_some()
            && self.preferences.right_panel
            && self.job.is_none()
            && self.native_picker.is_none()
            && !self.has_modal()
            && self.input.is_none()
            && alt
            && !ctrl
            && matches!(key, Key::Character(ch) if ch.eq_ignore_ascii_case("s"))
        {
            if !repeat {
                self.action(Action::SurfaceToggleLightShadow);
            }
            self.redraw();
            return true;
        }
        if self.surface.tab != 1
            || self.modifiers.control_key()
            || self.modifiers.super_key()
            || !self.preferences.right_panel
            || !self.material_authoring_available()
        {
            return false;
        }
        if alt && !ctrl {
            let command = match key {
                Key::Named(NamedKey::ArrowDown) => Some(Action::SurfaceMaterialList),
                Key::Character(ch) if ch.eq_ignore_ascii_case("n") => {
                    Some(Action::SurfaceCreateMaterial)
                }
                Key::Character(ch) if ch.eq_ignore_ascii_case("d") => {
                    Some(Action::SurfaceUniqueMaterial)
                }
                Key::Character(ch) => ch
                    .parse::<usize>()
                    .ok()
                    .filter(|index| (1..=5).contains(index))
                    .map(|index| Action::SurfacePreset(index - 1)),
                _ => None,
            };
            if let Some(action) = command {
                if !repeat && self.resolve_input() {
                    self.action(action);
                }
                self.redraw();
                return true;
            }
        }
        false
    }

    pub(super) fn material_list_height(&self) -> f32 {
        self.material_capacity() as f32 * self.material_row_height() + 12. + MATERIAL_FOOTER_HEIGHT
    }
    pub(super) fn material_controls_shift(&self) -> f32 {
        MATERIAL_LIST_Y + self.material_list_height() + 12. - 353.
    }
    pub(super) fn material_panel_bottom(&self) -> f32 {
        self.material_legacy_bottom() + if self.artistic_enabled() {
            super::artistic_controls::PAINT_CONTROLS_HEIGHT
        } else { 0. }
    }
    pub(super) fn material_legacy_bottom(&self) -> f32 {
        if self.selected_material_id().is_none() { return 387. + self.material_controls_shift(); }
        (if self.surface.maps { 2232. } else { 1892. })
            + self.material_controls_shift() + EMISSION_EXTRA_HEIGHT + self.material_map_details_height() + if self.npr_enabled() { 800. } else { 0. }
    }
    pub(super) fn material_list_ids(&self) -> Vec<u64> {
        self.material_layer_owner().map(|node| self.scene_material_ids(node)).unwrap_or_default()
    }
    pub(super) fn visible_material_ids(&self) -> Vec<u64> {
        let (rows, capacity) = self.material_scroll_rows();
        rows.into_iter().skip(self.material_list_offset()).take(capacity)
            .filter_map(|row| if let super::material_users::MaterialRow::Material(id) = row { Some(id) } else { None }).collect()
    }
    fn material_scroll_rows(&self) -> (Vec<super::material_users::MaterialRow>, usize) {
        (self.material_rows(), self.material_capacity())
    }
    fn material_list_offset(&self) -> usize {
        let (rows, capacity) = self.material_scroll_rows();
        self.surface.material_offset.min(rows.len().saturating_sub(capacity))
    }
    fn material_list_rect(&self) -> [f32; 4] {
        let [x, _, w, _] = self.right_panel_rect();
        [x + super::properties::RAIL_WIDTH + 16.,
            self.property_y(MATERIAL_LIST_Y).unwrap_or(-10000.) - super::properties::CONTENT_SHIFT - self.inspector_scroll,
            w - super::properties::RAIL_WIDTH - 62., self.material_list_height()]
    }
    fn material_scrollbar(&self) -> [f32; 4] {
        let r = self.material_list_rect(); [r[0]+r[2]-17.,r[1]+6.,12.,r[3]-12.-MATERIAL_FOOTER_HEIGHT]
    }
    fn set_material_offset(&mut self, value: usize) {
        self.surface.material_offset = value;
    }
    pub(super) fn reveal_material_row(&mut self, material: u64) {
        let (rows, capacity) = self.material_scroll_rows();
        if let Some(index) = rows.iter().position(|row| matches!(row, super::material_users::MaterialRow::Material(id) if *id == material)) {
            let offset = self.material_list_offset();
            if index < offset { self.set_material_offset(index); }
            else if index >= offset + capacity { self.set_material_offset(index + 1 - capacity); }
        }
    }
    pub(super) fn start_material_scroll(&mut self) {
        if self.has_modal() || self.native_picker.is_some() { return; }
        let (rows, capacity) = self.material_scroll_rows();
        let max = rows.len().saturating_sub(capacity); let r = self.material_scrollbar();
        let thumb = (r[3]*capacity as f32/rows.len().max(1) as f32).max(28.).min(r[3]);
        let offset = self.material_list_offset();
        let top = r[1]+(r[3]-thumb)*offset as f32/max.max(1) as f32;
        let offset = if self.mouse[1] < top || self.mouse[1] > top+thumb {
            ((self.mouse[1]-r[1]-thumb*0.5)/(r[3]-thumb).max(1.)*max as f32).round().clamp(0.,max as f32) as usize
        } else { offset };
        self.set_material_offset(offset);
        self.drag = Some(super::Drag::MaterialScroll { start:self.mouse[1], value:offset });
        self.drag_button = Some(winit::event::MouseButton::Left);
    }
    pub(super) fn update_material_scroll(&mut self, start: f32, value: usize, y: f32) {
        let (rows, capacity) = self.material_scroll_rows();
        let max = rows.len().saturating_sub(capacity); let r = self.material_scrollbar();
        let thumb = (r[3]*capacity as f32/rows.len().max(1) as f32).max(28.).min(r[3]);
        let offset = (value as f32+(y-start)*max as f32/(r[3]-thumb).max(1.)).round().clamp(0.,max as f32) as usize;
        self.set_material_offset(offset);
    }
    pub(super) fn material_scroll_wheel(&mut self, delta: f32) -> bool {
        if self.surface.tab != 1 || self.modifiers.control_key() || self.modifiers.super_key()
            || !self.preferences.right_panel || self.has_modal() || self.native_picker.is_some()
            || self.drag.is_some() || !crate::ui::contains(self.properties_content_rect(),self.mouse) { return false; }
        if !crate::ui::contains(self.material_list_rect(),self.mouse) { return false; }
        if !delta.is_finite() || (self.input.is_some() && !self.resolve_input()) { return true; }
        let (rows,capacity) = self.material_scroll_rows();
        let offset = (self.material_list_offset() as isize-(delta*2.) as isize).clamp(0,rows.len().saturating_sub(capacity) as isize) as usize;
        self.set_material_offset(offset); self.redraw(); true
    }
    fn draw_material_list(&self, ui: &mut Ui, x: f32, width: f32) {
        let p = ui.palette(); let enabled = self.material_authoring_available();
        for (row,label,action,available) in [
            (0,"+",Action::SurfaceCreateMaterial,enabled && self.material_layer_owner().is_some()),
            (1,"…",Action::DropdownOpen(DropdownKind::MaterialActions),(enabled || self.dropdown_is_open(DropdownKind::MaterialActions)) && self.selected_material_id().is_some()),
        ] { ui.outline_button([x+width-26.,MATERIAL_LIST_Y+2.+row as f32*32.,26.,28.],label,action,available); }
        let library = [x+width-26.,MATERIAL_LIST_Y+66.,26.,28.];
        ui.outlined_surface(library,p.panel,p.border);
        if enabled || self.dropdown_is_open(DropdownKind::MaterialLibrary) {
            ui.quiet_icon_button(library,"",Action::DropdownOpen(DropdownKind::MaterialLibrary),self.dropdown_is_open(DropdownKind::MaterialLibrary),Icon::Library);
        } else { ui.disabled_icon_button(library,"",Icon::Library); }
        let list_width = width-32.;
        let y = MATERIAL_LIST_Y;
        let height = self.material_list_height();
        ui.outlined_surface([x,y,list_width,height],p.panel,p.border);
        let (rows,capacity) = self.material_scroll_rows(); let offset = self.material_list_offset();
        for (row,item) in rows.iter().skip(offset).take(capacity).enumerate() {
            self.draw_material_tree_row(ui,[x+4.,y+6.+row as f32*self.material_row_height(),list_width-22.,38.],*item,true);
        }
        if rows.is_empty() {
            ui.fitted_label([x+8.,y+12.,list_width-16.,40.],"Нет назначенных материалов",11.,p.muted);
        }
        let max = rows.len().saturating_sub(capacity);
        if max>0 { ui.vertical_scrollbar([x+list_width-17.,y+6.,12.,height-12.-MATERIAL_FOOTER_HEIGHT],offset as f32/max as f32,capacity as f32/rows.len() as f32,Action::MaterialScroll); }
        let y = MATERIAL_LIST_Y + height - MATERIAL_FOOTER_HEIGHT;
        ui.surface([x+1.,y,list_width-2.,1.],p.border);
        let can_assign = enabled && self.material_faces_available() && self.selected_material_id().is_some()
            && !self.editor.has_active_edit() && !self.editor.has_staged() && !self.editor.playing;
        let can_remove = can_assign && self.authoring.editing.zip(self.selected_material_id()).is_some_and(|(node,material)| {
            self.editor.project.node(node).and_then(|n| n.mesh).and_then(|id| self.editor.project.mesh(id))
                .is_some_and(|mesh| self.authoring.faces.iter().any(|&face| mesh.face_material(face)==Some(material)))
        });
        for (column,action,available,icon) in [
            (0,Action::MaterialApplyFaces,can_assign,Icon::MaterialAssign),
            (1,Action::MaterialUnassignFaces,can_remove,Icon::MaterialUnassign),
        ] {
            let rect=[x+6.+column as f32*36.,y+4.,32.,32.];
            if available { ui.quiet_icon_button(rect,"",action,false,icon); }
            else { ui.disabled_icon_button(rect,"",icon); }
        }
    }

    pub(super) fn draw_material_panel(&self, ui: &mut Ui, rx: f32, rw: f32) {
        let palette = ui.palette();
        let x = rx + 16.;
        let width = rw - 32.;
        let material = self.selected_material_id().and_then(|id| self.editor.project.material(id));
        let uses = material.map_or(0, |m| self.editor.project.nodes.iter()
            .filter(|n| self.scene_material_ids(n.id).contains(&m.id)).count());
        let metadata = self.input.as_ref().map(|i| &i.purpose);
        let unlisted_name = material.is_some_and(|m| !self.material_list_ids().contains(&m.id));
        if let Some(m) = material.filter(|_| matches!(metadata, Some(Action::MaterialCategory | Action::MaterialTags))
            || (unlisted_name && metadata == Some(&Action::SurfaceMaterialName))) {
            let action = metadata.unwrap().clone();
            let text = match action { Action::SurfaceMaterialName => m.name.clone(), Action::MaterialCategory => m.category.clone(), _ => m.tags.join(", ") };
            ui.field([x, 171., width, 30.], &text, action);
        } else {
            let title = if self.surface.material_library { material.map(|m| m.name.as_str()).unwrap_or("Материал") } else { "Материалы меша" };
            ui.fitted_label([x, 178., width - 62., 18.], title, 11., palette.muted);
            if material.is_some() {
                ui.quiet_icon_button([x + width - 58., 171., 58., 30.], "Узлы", Action::Node(crate::ui::NodeAction::Open), self.nodes_open(), Icon::Material);
            }
        }
        self.draw_material_list(ui, x, width);
        let shift = self.material_controls_shift();
        let Some(material) = material else {
            ui.fitted_label(
                [x, 361. + shift, width, 16.],
                "Создайте материал или выберите в библиотеке.",
                11.,
                palette.muted,
            );
            return;
        };
        self.draw_paint_controls(ui, x, width, self.material_legacy_bottom());
        ui.checkbox(
            [x, 353. + shift, width, 28.],
            "NPR · рисованный материал",
            Action::SurfaceNprToggle,
            material.npr.enabled,
        );
        let color = [
            material.base_color[0],
            material.base_color[1],
            material.base_color[2],
        ];
        ui.label(x, 398. + shift, "Цвет", 11., palette.muted);
        ui.color_field(
            [x + 84., 390. + shift, width - 84., 30.],
            &color_hex(color),
            Action::SurfaceMaterialColor(false),
            color.map(linear_to_srgb),
        );
        let response_fields = if material.npr.enabled {
            [
                ("Ступени света", 19, SliderRange::new(2., 8., 1.)),
                ("Порог тени", 20, SliderRange::new(0.05, 0.95, 0.01)),
                ("Мягкость границ", 21, SliderRange::new(0., 0.25, 0.005)),
                ("Размер блика", 22, SliderRange::new(0.01, 0.5, 0.01)),
                ("Сила блика", 23, SliderRange::new(0., 2., 0.01)),
            ]
        } else {
            [
                ("Металл", 0, SliderRange::new(0., 1., 0.01)),
                ("Шероховатость", 1, SliderRange::new(0., 1., 0.01)),
                ("Показатель IOR", 7, SliderRange::new(1., 3., 0.01)),
                ("Слой лака", 8, SliderRange::new(0., 1., 0.01)),
                ("Шерох. лака", 9, SliderRange::new(0.03, 1., 0.01)),
            ]
        };
        for (row, (label, field, range)) in response_fields.into_iter().enumerate() {
            numeric(
                ui,
                [x, 432. + row as f32 * 40. + shift, width, 30.],
                label,
                material_number(material, field),
                Action::SurfaceMaterialNumber(field),
                range,
            );
        }
        ui.label(x, 644. + shift, "Свечение", 11., palette.muted);
        let emission = emissive_tint(material.emissive);
        ui.color_field(
            [x + 84., 636. + shift, width - 84., 30.],
            &color_hex(emission),
            Action::SurfaceMaterialColor(true),
            emission.map(linear_to_srgb),
        );
        numeric(
            ui,
            [x, 676. + shift, width, 30.],
            "Сила свечения",
            material_number(material, 6),
            Action::SurfaceMaterialNumber(6),
            SliderRange::new(0., 20., 0.1),
        );
        ui.checkbox(
            [x, 716. + shift, width, 30.],
            "Объёмный свет",
            Action::SurfaceEmissionVolume,
            material.emission_volume,
        );
        let shift = shift + EMISSION_EXTRA_HEIGHT;
        numeric(
            ui,
            [x, 716. + shift, width, 30.],
            "Непрозрачность",
            material_number(material, 2),
            Action::SurfaceMaterialNumber(2),
            SliderRange::new(0., 1., 0.01),
        );
        self.draw_dropdown_button(
            ui,
            [x, 756. + shift, width, 30.],
            DropdownKind::MaterialAlpha,
            "Режим прозрачности",
        );
        ui.checkbox(
            [x, 796. + shift, width, 30.],
            "Две стороны",
            Action::SurfaceDoubleSided,
            material.double_sided,
        );
        ui.button(
            [x, 836. + shift, width, 30.],
            if self.surface.maps {
                "Свернуть карты"
            } else {
                "Карты текстур…"
            },
            Action::SurfaceMaps,
            self.surface.maps,
        );
        self.draw_material_studio(
            ui,
            x,
            width,
            if self.surface.maps { 1364. } else { 1024. } + shift + self.material_map_details_height(),
        );
        let uv_y = if self.surface.maps { 1940. } else { 1600. } + shift + self.material_map_details_height();
        ui.label(x, uv_y, "МАСШТАБ И ПОЛОЖЕНИЕ ТЕКСТУР", 10., palette.muted);
        for (row, (label, field, range)) in [
            ("Повтор U", 14, SliderRange::new(0.01, 20., 0.01)),
            ("Повтор V", 15, SliderRange::new(0.01, 20., 0.01)),
            ("Сдвиг U", 16, SliderRange::new(-2., 2., 0.01)),
            ("Сдвиг V", 17, SliderRange::new(-2., 2., 0.01)),
            ("Поворот, °", 18, SliderRange::new(-180., 180., 1.)),
        ]
        .into_iter()
        .enumerate()
        {
            numeric(
                ui,
                [x, uv_y + 26. + row as f32 * 40., width, 30.],
                label,
                material_number(material, field),
                Action::SurfaceMaterialNumber(field),
                range,
            );
        }
        ui.button(
            [x, uv_y + 232., width, 30.],
            "Сбросить UV",
            Action::SurfaceResetMapping,
            false,
        );
        let mut y = 880. + shift;
        if self.surface.maps {
            for (slot, label) in [
                "Цвет · sRGB",
                "Металл B / шероховатость G",
                "Нормали · linear",
                "AO · красный канал",
                "Свечение · sRGB",
            ]
            .into_iter()
            .enumerate()
            {
                ui.label(x, y, label, 11., palette.muted);
                let name = texture_slot(material, slot)
                    .and_then(|id| self.editor.project.textures.iter().find(|t| t.id == id))
                    .map(|t| t.name.as_str())
                    .unwrap_or("Без карты");
                ui.button(
                    [x, y + 20., width - 112., 30.],
                    name,
                    Action::SurfaceNextTexture(slot),
                    false,
                );
                ui.icon_button(
                    [x + width - 72., y + 20., 40., 30.],
                    "+",
                    Action::SurfaceImportTexture(slot),
                    false,
                    Icon::Import,
                );
                ui.icon_button(
                    [x + width - 26., y + 20., 26., 30.],
                    "×",
                    Action::SurfaceClearTexture(slot),
                    false,
                    Icon::Close,
                );
                ui.icon_button([x+width-106.,y+20.,28.,30.], "Параметры карты", Action::Media(crate::ui::MediaAction::Slot(slot)), self.surface.media_slot==Some(slot), Icon::Settings);
                y += 68.;
                if self.surface.media_slot == Some(slot) { self.draw_material_map_details(ui,x,y,width,slot); y += self.material_map_details_height(); }
            }
        }
        for (label, field, value, range) in [
            (
                "Сила нормалей",
                3,
                material.normal_scale,
                SliderRange::new(0., 4., 0.01),
            ),
            (
                "Сила AO",
                4,
                material.occlusion_strength,
                SliderRange::new(0., 1., 0.01),
            ),
            (
                "Порог отсечения",
                5,
                material.alpha_cutoff,
                SliderRange::new(0., 1., 0.01),
            ),
        ] {
            numeric(
                ui,
                [x, y, width, 30.],
                label,
                value,
                Action::SurfaceMaterialNumber(field),
                range,
            );
            y += 40.;
        }
        ui.fitted_label(
            [x, y + 4., width, 16.],
            if uses > 1 {
                "Для отдельной правки нажмите «Копия»."
            } else {
                "Карты сохраняются внутри проекта."
            },
            10.,
            palette.muted,
        );
    }
}
