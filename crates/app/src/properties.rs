//! Contextual properties share a fixed vertical navigation rail and scroll geometry.
use super::*;

// Keep legacy inspector coordinates; translate both paint and input below the
// creation row through the existing scroll transform.
pub(super) const CONTENT_SHIFT: f32 = -66.;
pub(super) const RAIL_WIDTH: f32 = 40.;
const TABS: [u8; 9] = [3, 6, 7, 8, 0, 1, 12, 10, 9];

impl App {
    pub(super) fn creation_toolbar_rect(&self) -> [f32; 4] {
        let [x, y, w, _] = self.right_panel_rect();
        [x, y, w, 116.]
    }
    pub(super) fn draw_creation_toolbar(&self, u: &mut Ui) {
        let [x, y, w, h] = self.creation_toolbar_rect();
        u.outlined_surface([x, y, w, h], u.palette().panel, u.palette().border);
        // Give the longer captions room even at the minimum panel width.
        let unit = (w - 28.) / 14.;
        let mut left = x + 8.;
        for (i, (label, action, icon)) in [
            (
                "Меш",
                Action::DropdownOpen(ui::DropdownKind::MeshCreate),
                Icon::Object,
            ),
            (
                "Кривые",
                Action::DropdownOpen(ui::DropdownKind::CurveCreate),
                Icon::Curve,
            ),
            (
                "Скелет",
                Action::DropdownOpen(ui::DropdownKind::SkeletonCreate),
                Icon::Bone,
            ),
            (
                "Свет",
                Action::DropdownOpen(ui::DropdownKind::LightCreate),
                Icon::Light,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let width = unit * [3., 4., 4., 3.][i];
            let r = [left, y + 6., width, 32.];
            left += width + 4.;
            if self.job.is_some() || self.native_picker.is_some()
                || (action == Action::DropdownOpen(ui::DropdownKind::SkeletonCreate)
                    && (self.editor.has_staged() || self.editor.playing || self.scene_write_pending()))
            {
                u.disabled_icon_button(r, label, icon);
            } else {
                let active = matches!(action, Action::DropdownOpen(kind) if self.dropdown_is_open(kind));
                u.quiet_icon_button(r, label, action, active, icon);
            }
        }
        let disabled=self.job.is_some() || self.native_picker.is_some() || self.scene_write_pending() || self.editor.has_staged();
        for (label,action,icon,r) in [
            ("Текст",Action::Text(ui::TextAction::Create),Icon::Text,[x+8.,y+42.,unit*3.,32.]),
            ("Пустышка",Action::DropdownOpen(ui::DropdownKind::EmptyCreate),Icon::Empty,[x+12.+unit*3.,y+42.,unit*4.,32.]),
            (if unit*4.>=125. {"Силовое поле"}else{"Поле"},Action::DropdownOpen(ui::DropdownKind::ForceCreate),Icon::Force,[x+16.+unit*7.,y+42.,unit*4.,32.]),
            ("FX",Action::DropdownOpen(ui::DropdownKind::FxCreate),Icon::Fx,[x+20.+unit*11.,y+42.,unit*3.,32.]),
        ] {
            if disabled {u.disabled_icon_button(r,label,icon);} else {let active=matches!(action,Action::DropdownOpen(kind) if self.dropdown_is_open(kind));u.quiet_icon_button(r,label,action,active,icon);}
        }
        if self.rig_context() {
            self.draw_rig_modes(u, x + 8., y + 78., w - 16.);
        } else {
            self.draw_mesh_modes(u, x + 8., y + 78., w - 16.);
        }
    }

    pub(super) fn draw_mesh_modes(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        use crate::ui::{AuthorAction, MeshSelection};
        let width = (w - 12.) / 4.;
        let editable = self
            .editor
            .selected
            .is_some_and(|id| self.node_selectable(id) && self.editor.editable_mesh(id).is_ok());
        for (i, (_label, mode, icon)) in [
            ("Объект", None, if self.selected_curve().is_some() { Icon::Curve }
                else if self.editor.selected.is_some_and(|id| self.is_skeleton_object(id)) { Icon::Bone }
                else if self.selected_text().is_some() {Icon::Text}
                else if let Some((_,f))=self.selected_fx() {fx_controls::fx_icon(f.kind)}
                else if let Some(f)=self.editor.selected.and_then(|id|self.editor.project.node(id)).and_then(|n|n.force_field) {empty_controls::field_icon(f.kind)} else if self.editor.selected.and_then(|id|self.editor.project.node(id)).is_some_and(|n|n.empty) {Icon::Empty} else { Icon::Object }),
            ("Вершина", Some(MeshSelection::Vertex), Icon::Vertex),
            ("Ребро", Some(MeshSelection::Edge), Icon::Edge),
            ("Грань", Some(MeshSelection::Face), Icon::Face),
        ]
        .into_iter()
        .enumerate()
        {
            let r = [x + i as f32 * (width + 4.), y, width, 32.];
            let action = mode.map_or(Action::Mode(0), |m| {
                Action::Author(AuthorAction::SelectMode(m))
            });
            let active = if let Some(m) = mode {
                self.authoring.editing.is_some() && self.authoring.selection_mode == m
            } else {
                self.editor.mode == EditorMode::Scene && self.authoring.editing.is_none()
            };
            let disabled = self.job.is_some()
                || self.native_picker.is_some()
                || self.editor.has_staged()
                || (mode.is_some() && (!editable || self.editor.playing));
            if disabled {
                u.disabled_button(r, "");
            } else {
                u.quiet_button(r, "", action, active);
            }
            let p = u.palette();
            let color = if disabled {
                p.disabled
            } else if active {
                p.accent
            } else {
                p.text
            };
            icon.draw(u, [r[0] + r[2] * 0.5 - 9., r[1] + 7., 18., 18.], color);
        }
    }

    pub(super) fn set_properties_tab(&mut self, tab: u8) {
        if tab == 14 { self.open_dropdown(ui::DropdownKind::AssetLibrary); return; }
        if tab != 12 { self.close_uv_workspace(false); }
        let target = if matches!(tab, 2 | 5) || (tab == 12 && !self.uv.open)
            || (tab == 1 && !self.material_tab_available()
                || tab == 10 && !self.normals_available())
        {
            0
        } else {
            tab.min(14)
        };
        self.surface.scroll_offsets[self.surface.tab.min(14) as usize] = self.inspector_scroll;
        self.surface.tab = target;
        if target != 0 { self.path_edit = None; }
        self.inspector_scroll =
            self.surface.scroll_offsets[target as usize].min(self.inspector_max_scroll());
    }

    pub(super) fn material_tab_available(&self) -> bool {
        self.active_light_index().is_none() && self.selected_camera_id().is_none()
    }

    pub(super) fn properties_content_rect(&self) -> [f32; 4] {
        let [x, _, w, _] = self.right_panel_rect();
        [
            x + RAIL_WIDTH + 8.,
            236.,
            w - RAIL_WIDTH - 16.,
            (self.panel_bottom() - 244.).max(1.),
        ]
    }

    pub(super) fn properties_scrollbar_rect(&self) -> [f32; 4] {
        let [x, _, w, _] = self.right_panel_rect();
        [
            x + w - 15.,
            242.,
            12.,
            (self.panel_bottom() - 254.).max(24.),
        ]
    }

    pub(super) fn properties_cycle(&mut self, direction: i32) {
        let tabs: Vec<_> = TABS
            .into_iter()
            .filter(|tab| {
                (*tab != 12 || self.uv.open) && (*tab != 1 || self.material_tab_available())
                    && (*tab != 10 || self.normals_available())
            })
            .collect();
        let current = if matches!(self.surface.tab, 4 | 11 | 13) { 6 } else { self.surface.tab };
        let index = tabs
            .iter()
            .position(|tab| *tab == current)
            .unwrap_or(0) as i32;
        self.action(Action::SurfaceTab(
            tabs[(index + direction).rem_euclid(tabs.len() as i32) as usize],
        ));
    }

    pub(super) fn properties_key(&mut self, key: &Key, ctrl: bool) -> bool {
        if !ctrl
            || !self.preferences.right_panel
            || self.input.is_some()
            || self.has_modal()
            || self.drag.is_some()
            || self.job.is_some()
            || self.native_picker.is_some()
        {
            return false;
        }
        let direction = match key {
            Key::Named(NamedKey::PageDown) => 1,
            Key::Named(NamedKey::PageUp) => -1,
            _ => return false,
        };
        self.properties_cycle(direction);
        true
    }

    pub(super) fn draw_properties_navigation(&self, u: &mut Ui) {
        let [x, top, w, height] = self.right_panel_rect();
        let toolbar = self.creation_toolbar_rect();
        let y = toolbar[1] + toolbar[3];
        let h = top + height - y;
        let p = u.palette();
        u.surface([x, y, RAIL_WIDTH, h], p.bg);
        u.rect([x + RAIL_WIDTH - 1., y + 8., 1., h - 16.], p.border);
        let object_icon = if self.active_light_index().is_some() {
            Icon::Light
        } else if self.selected_camera_id().is_some() {
            Icon::Camera
        } else if self.selected_curve().is_some() {
            Icon::Curve
        } else if self.selected_text().is_some() {Icon::Text
        } else if let Some((_,f))=self.selected_fx() {fx_controls::fx_icon(f.kind)
        } else if let Some(f)=self.editor.selected.and_then(|id|self.editor.project.node(id)).and_then(|n|n.force_field) {empty_controls::field_icon(f.kind)
        } else if self.editor.selected.and_then(|id|self.editor.project.node(id)).is_some_and(|n|n.empty) {Icon::Empty
        } else if self.selected_joint_properties() || self.editor.selected.is_some_and(|id| self.is_skeleton_object(id)) {
            Icon::Bone
        } else {
            Icon::Object
        };
        // UV owns a split workspace and closes when another property tab is chosen.
        let navigation: Vec<_> = [
            (Some(3), Icon::Render),
            (Some(6), Icon::Physics),
            (Some(7), Icon::Grid),
            (Some(8), Icon::Modifiers),
            (Some(0), object_icon),
            (Some(1), Icon::Material),
            (None, Icon::Uv),
            (Some(10), Icon::Normals),
        ]
        .into_iter()
        .filter(|(tab, _)| {
            (tab.is_some() || self.selected_mesh_for_uv()) && (*tab != Some(1) || self.material_tab_available())
                && (*tab != Some(10) || self.normals_available())
        })
        .collect();
        let count = navigation.len() as f32 + 1.; // Includes settings.
        let group_gap = 16.;
        let step = ((self.panel_bottom() - 236. - group_gap) / (count - 1.)).clamp(20., 36.);
        let button_width = 32.;
        // Contextual skeleton settings live with the selected object.
        u.rect([x + 10., 196. + 3. * step + group_gap * 0.5 - 0.5, 20., 1.], p.border);
        for (index, (tab, icon)) in navigation.into_iter().enumerate() {
            let (action, active) = if tab == Some(14) {
                (Action::DropdownOpen(ui::DropdownKind::AssetLibrary), self.dropdown_is_open(ui::DropdownKind::AssetLibrary))
            } else if let Some(tab) = tab {
                (Action::SurfaceTab(tab), self.surface.tab == tab || (tab == 6 && matches!(self.surface.tab, 4 | 11 | 13)))
            } else {
                (
                    if self.uv.open { Action::SurfaceTab(12) } else { Action::Uv(ui::UvAction::Open) },
                    self.uv.open,
                )
            };
            u.quiet_icon_button(
                [x + 4., 196. + index as f32 * step + if index >= 3 { group_gap } else { 0. }, button_width, step.min(32.)],
                "",
                action,
                active,
                icon,
            );
        }
        let settings_x = x + 4.;
        let settings_y = self.panel_bottom() - 40.;
        u.quiet_icon_button(
            [settings_x, settings_y, button_width, 32.],
            "",
            Action::SurfaceTab(9),
            self.surface.tab == 9,
            Icon::Settings,
        );
        let title = match self.surface.tab {
            12 => "UV",
            13 => "Физика · разрушение",
            1 => "Материал",
            6 => "Физика",
            7 => "Геометрия",
            8 => "Модификаторы",
            10 => "Нормали",
            9 => "Настройки программы",
            3 => "Рендер",
            4 => "Физика · жидкость",
            11 => "Физика · ткань",
            _ if self.active_light_index().is_some() => "Свет",
            _ if self.selected_camera_id().is_some() => "Камера",
            _ if self.selected_joint_properties() => "Кость",
            _ if self.editor.selected.is_some_and(|id| self.is_skeleton_object(id)) => "Скелет",
            _ => "Объект",
        };
        // Keep the navigation rail within the compact animation workspace.
        u.quiet_icon_button(
            [x + w - 66., y + 6., 28., 28.],
            "",
            Action::DropdownOpen(ui::DropdownKind::AssetLibrary),
            self.dropdown_is_open(ui::DropdownKind::AssetLibrary),
            Icon::Library,
        );
        u.quiet_button(
            [x + w - 34., y + 6., 28., 28.],
            "−",
            Action::ToggleRightPanel,
            false,
        );
        let context = if self.surface.tab == 9 {
            "Тема, акцент и язык"
        } else if matches!(self.surface.tab, 3 | 5) {
            self.editor.project.title.as_str()
        } else if let Some(index) = self.active_light_index() {
            self.editor.project.lights[index].name.as_str()
        } else {
            self.editor
                .selected
                .and_then(|id| self.editor.project.node(id))
                .map_or("Выберите объект в сцене", |node| {
                    node.name.as_str()
                })
        };
        u.fitted_label(
            [x + RAIL_WIDTH + 16., y + 12., w - RAIL_WIDTH - 96., 18.],
            title,
            14.,
            p.text,
        );
        u.fitted_label(
            [x + RAIL_WIDTH + 16., y + 33., w - RAIL_WIDTH - 32., 15.],
            context,
            11.,
            p.muted,
        );
    }
}
