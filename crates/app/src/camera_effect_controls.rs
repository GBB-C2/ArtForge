//! Camera-local effect rows reuse lens parameters, numeric editing and history.
use super::*;
use crate::ui::{CameraTool as C, DropdownKind};
use poseforge_core::CameraEffects;

const LIST_Y: f32 = 1234.;
const ROW: f32 = 32.;

#[derive(Default)]
pub(super) struct State {
    selection: Option<(u64, usize)>,
    focused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut a = App::new(None);
        a.preferences = Preferences::default();
        a.preferences.right_panel = true;
        a.preferences.animation_visible = false;
        a.width = 1200.;
        a.height = 820.;
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        a.action(Action::CreateCamera);
        a.preferences
            .collapsed_sections
            .extend([1000, 1001, 1002, 1003]);
        a.ui = a.build_ui();
        a
    }
    fn open(a: &mut App) {
        a.ui = a.build_ui();
        a.mouse =
            a.ui.action_point(&Action::DropdownOpen(DropdownKind::CameraEffectAdd))
                .unwrap();
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
        a.ui = a.build_ui();
        assert!(a.dropdown_is_open(DropdownKind::CameraEffectAdd));
    }
    #[test]
    fn camera_effect_dropdown_cancel_add_and_duplicate_use_one_transaction() {
        let mut a = app();
        let original = a.editor.project.clone();
        let undo = a.editor.undo_len();
        open(&mut a);
        assert_eq!(
            a.ui.hits
                .iter()
                .filter(|h| matches!(h.action, Action::DropdownChoose(_)))
                .count(),
            6
        );
        a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
        assert_eq!(a.editor.project, original);
        assert_eq!(a.editor.undo_len(), undo);
        open(&mut a);
        a.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
        a.ui = a.build_ui();
        assert!(a.ui.action_point(&Action::DropdownChoose(6)).is_some());
        a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
        let (id, effects) = a.selected_camera_effects().unwrap();
        assert!(effects.contains(10) && effects.enabled);
        assert_eq!(a.editor.project.nodes.len(), original.nodes.len());
        assert_eq!(a.editor.undo_len(), undo + 1);
        assert!(a.dropdown_focus.is_none());
        a.action(Action::CameraTool(C::AddEffect(10)));
        assert_eq!(a.editor.undo_len(), undo + 1);
        assert!(a.editor.undo());
        assert_eq!(a.editor.project, original);
        assert!(a.editor.redo());
        assert_eq!(
            a.editor.project.node(id).unwrap().camera.unwrap().effects,
            effects
        );
    }
    #[test]
    fn camera_effect_rows_toggle_reset_remove_and_keyboard_leave_camera_intact() {
        let mut a = app();
        a.action(Action::CameraTool(C::AddEffect(8)));
        a.action(Action::CameraTool(C::AddEffect(9)));
        let (id, initial) = a.selected_camera_effects().unwrap();
        let count = a.editor.project.nodes.len();
        assert!(a.camera_effect_key(&Key::Named(NamedKey::ArrowUp), false, false));
        assert_eq!(a.selected_camera_effect(), Some(8));
        assert!(a.camera_effect_key(&Key::Named(NamedKey::Space), false, false));
        let hidden = a.selected_camera_effects().unwrap().1;
        assert_eq!(hidden.vignette, initial.vignette);
        assert_eq!(hidden.uniforms().1[0], 0.);
        assert!(a.camera_effect_key(&Key::Named(NamedKey::Enter), false, false));
        assert_eq!(a.input.as_ref().unwrap().purpose, Action::CameraNumber(8));
        a.end_input();
        a.action(Action::ResetNumber(Box::new(Action::CameraNumber(8))));
        let effects = a.selected_camera_effects().unwrap().1;
        assert!(effects.contains(8) && !effects.effect_enabled(8));
        assert_eq!(effects.vignette, 0.);
        let saved = serde_json::to_vec(&a.editor.project).unwrap();
        let loaded: Project = serde_json::from_slice(&saved).unwrap();
        assert_eq!(loaded, a.editor.project);
        loaded.validate().unwrap();
        assert!(a.camera_effect_key(&Key::Named(NamedKey::Delete), false, false));
        assert!(!a.selected_camera_effects().unwrap().1.contains(8));
        assert_eq!(a.editor.project.nodes.len(), count);
        assert!(a.editor.project.node(id).unwrap().camera.is_some());
        assert!(a.editor.undo());
        assert_eq!(a.editor.project, loaded);
        a.action(Action::CameraTool(C::RemoveEffect(8)));
        a.action(Action::CameraTool(C::RemoveEffect(9)));
        let empty = a.editor.project.clone();
        assert!(a.camera_effect_key(&Key::Named(NamedKey::Delete), false, false));
        assert_eq!(a.editor.project, empty);
        a.mouse = [1., 1.];
        a.camera_effect_pointer_focus();
        assert!(!a.camera_effect_key(&Key::Named(NamedKey::Delete), false, false));
    }
    #[test]
    fn camera_effect_popup_cannot_write_to_a_different_or_locked_camera() {
        let mut a = app();
        let first = a.editor.selected.unwrap();
        a.action(Action::CreateCamera);
        let second = a.editor.selected.unwrap();
        a.editor.selected = Some(first);
        a.inspector_scroll = 0.;
        open(&mut a);
        a.editor.selected = Some(second);
        let before = a.editor.project.clone();
        a.choose_dropdown(4);
        assert_eq!(a.editor.project, before);
        a.inspector_scroll = 0.;
        open(&mut a);
        a.editor.project.node_mut(second).unwrap().locked = true;
        let before = a.editor.project.clone();
        a.choose_dropdown(4);
        a.action(Action::CameraTool(C::AddEffect(8)));
        assert_eq!(a.editor.project, before);
        a.ui = a.build_ui();
        assert!(!a
            .ui
            .hits
            .iter()
            .any(|h| h.action == Action::DropdownOpen(DropdownKind::CameraEffectAdd)));
    }
    #[test]
    fn camera_effect_layout_and_restored_fx_menu_work_in_both_themes_and_scales() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for scale in [1., 2.] {
                for size in [[640., 480.], [1200., 820.]] {
                    let mut a = app();
                    a.preferences.theme = theme;
                    a.scale = scale;
                    a.width = size[0];
                    a.height = size[1];
                    a.inspector_scroll = a.inspector_max_scroll();
                    a.ui = a.build_ui();
                    // The old four-item FX menu remains independent from camera effects.
                    let before = a.editor.project.clone();
                    a.open_dropdown(DropdownKind::FxCreate);
                    a.ui = a.build_ui();
                    assert!(a.dropdown_is_open(DropdownKind::FxCreate));
                    assert_eq!(
                        a.ui.hits
                            .iter()
                            .filter(|h| matches!(h.action, Action::DropdownChoose(_)))
                            .count(),
                        4
                    );
                    a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
                    assert_eq!(a.editor.project, before);
                    for field in 4..=10 {
                        a.action(Action::CameraTool(C::AddEffect(field)));
                    }
                    a.camera_effect_key(&Key::Named(NamedKey::End), false, false);
                    a.camera_effect_key(&Key::Named(NamedKey::Enter), false, false);
                    a.ui = a.build_ui();
                    assert!(a.ui.action_point(&Action::CameraNumber(10)).is_some());
                    assert!(a.inspector_scroll <= a.inspector_max_scroll());
                    assert_eq!(a.camera_panel_bottom(), 1576.);
                }
            }
        }
    }
}

impl App {
    pub(super) fn selected_camera_effects(&self) -> Option<(u64, CameraEffects)> {
        let id = self.selected_camera_id()?;
        Some((id, self.editor.project.node(id)?.camera?.effects))
    }
    pub(super) fn camera_effect_editable(&self) -> bool {
        self.selected_camera_id()
            .is_some_and(|id| self.node_selectable(id))
            && self.job.is_none()
            && self.native_picker.is_none()
            && (!self.has_modal() || self.dropdown_is_open(DropdownKind::CameraEffectAdd))
            && !self.scene_write_pending()
            && !self.editor.has_staged()
            && !self.editor.playing
    }
    fn selected_camera_effect(&self) -> Option<usize> {
        let (id, effects) = self.selected_camera_effects()?;
        self.camera_effects
            .selection
            .filter(|(owner, field)| *owner == id && effects.contains(*field))
            .map(|(_, field)| field)
            .or_else(|| (4..=10).find(|&field| effects.contains(field)))
    }
    fn camera_effect_list_height(&self) -> f32 {
        let count = self
            .selected_camera_effects()
            .map_or(0, |(_, effects)| effects.effect_list().present.count_ones());
        count.max(3) as f32 * ROW + 12.
    }
    pub(super) fn camera_panel_bottom(&self) -> f32 {
        1340. + self.camera_effect_list_height()
    }
    fn reveal_camera_effect_y(&mut self, raw_y: f32, height: f32) {
        if let Some(mapped) = self.property_y(raw_y) {
            let y = mapped - super::properties::CONTENT_SHIFT - self.inspector_scroll;
            let area = self.properties_content_rect();
            if y < area[1] {
                self.inspector_scroll -= area[1] - y;
            } else if y + height > area[1] + area[3] {
                self.inspector_scroll += y + height - area[1] - area[3];
            }
            self.inspector_scroll = self.inspector_scroll.clamp(0., self.inspector_max_scroll());
        }
    }

    pub(super) fn camera_effect_action(&mut self, action: C) -> bool {
        let (C::AddEffect(field)
        | C::SelectEffect(field)
        | C::ToggleEffect(field)
        | C::RemoveEffect(field)) = action
        else {
            return false;
        };
        if !(4..=10).contains(&field) {
            return true;
        }
        if self.input.is_some() && !self.resolve_input() {
            return true;
        }
        let Some((id, mut effects)) = self.selected_camera_effects() else {
            return true;
        };
        if action != C::SelectEffect(field) && !self.camera_effect_editable() {
            return true;
        }
        if !effects.contains(field) && action != C::AddEffect(field) {
            return true;
        }
        let before = effects;
        match action {
            C::AddEffect(_) => {
                if let Err(error) = effects.add(field) {
                    self.status = error.to_string();
                    return true;
                }
            }
            C::ToggleEffect(_) => effects.toggle(field),
            C::RemoveEffect(_) => effects.remove(field),
            _ => {}
        }
        if effects != before {
            let result = self.editor.transact("Эффект камеры", |p| {
                p.node_mut(id).unwrap().camera.as_mut().unwrap().effects = effects
            });
            if self.result(result).is_none() {
                return true;
            }
        }
        self.camera_effects.selection = if effects.contains(field) {
            Some((id, field))
        } else {
            Some((
                id,
                (field..=10)
                    .chain(4..field)
                    .find(|&f| effects.contains(f))
                    .unwrap_or(field),
            ))
        };
        self.camera_effects.focused = true;
        self.dropdown_focus = None;
        self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
        if let Some(selected) = self.selected_camera_effect() {
            let row = (4..selected).filter(|&f| effects.contains(f)).count();
            self.reveal_camera_effect_y(LIST_Y + 6. + row as f32 * ROW, ROW);
        }
        self.status = if matches!(action, C::RemoveEffect(_)) {
            "Эффект камеры удалён · Ctrl+Z — отменить".into()
        } else {
            format!(
                "{} · эффект выбранной камеры",
                CameraEffects::LABELS[field - 4]
            )
        };
        self.update_dirty();
        self.redraw();
        true
    }

    pub(super) fn camera_effect_pointer_focus(&mut self) {
        if !matches!(
            self.ui.hit(self.mouse),
            Some(Action::CameraTool(
                C::SelectEffect(_) | C::AddEffect(_) | C::ToggleEffect(_) | C::RemoveEffect(_)
            )) | Some(Action::DropdownOpen(DropdownKind::CameraEffectAdd))
        ) {
            self.camera_effects.focused = false;
        }
    }
    pub(super) fn camera_effect_key(&mut self, key: &Key, ctrl: bool, alt: bool) -> bool {
        if !self.camera_effects.focused
            || ctrl
            || alt
            || self.surface.tab != 0
            || !self.preferences.right_panel
            || self.has_modal()
            || self.input.is_some()
            || self.dropdown.is_some()
            || self.drag.is_some()
        {
            return false;
        }
        let Some((id, effects)) = self.selected_camera_effects() else {
            return false;
        };
        // Do not let focus from another camera consume its scene shortcuts.
        if self
            .camera_effects
            .selection
            .is_some_and(|(owner, _)| owner != id)
        {
            return false;
        }
        if *key == Key::Named(NamedKey::Escape) {
            self.camera_effects.focused = false;
            self.redraw();
            return true;
        }
        let Some(field) = self.selected_camera_effect() else {
            return match key {
                Key::Named(NamedKey::Enter | NamedKey::Tab | NamedKey::Space) => {
                    self.open_dropdown(DropdownKind::CameraEffectAdd);
                    true
                }
                Key::Named(
                    NamedKey::ArrowUp
                    | NamedKey::ArrowDown
                    | NamedKey::Home
                    | NamedKey::End
                    | NamedKey::Delete
                    | NamedKey::Backspace,
                ) => true,
                _ => false,
            };
        };
        let fields: Vec<_> = (4..=10).filter(|&f| effects.contains(f)).collect();
        let index = fields.iter().position(|&f| f == field).unwrap();
        let action = match key {
            Key::Named(NamedKey::ArrowUp) => C::SelectEffect(fields[index.saturating_sub(1)]),
            Key::Named(NamedKey::ArrowDown) => {
                C::SelectEffect(fields[(index + 1).min(fields.len() - 1)])
            }
            Key::Named(NamedKey::Home) => C::SelectEffect(fields[0]),
            Key::Named(NamedKey::End) => C::SelectEffect(*fields.last().unwrap()),
            Key::Named(NamedKey::Space) => C::ToggleEffect(field),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => C::RemoveEffect(field),
            Key::Named(NamedKey::Enter | NamedKey::Tab) if self.camera_effect_editable() => {
                self.reveal_camera_effect_y(LIST_Y + self.camera_effect_list_height() + 12., 44.);
                self.action(Action::CameraNumber(field));
                self.redraw();
                return true;
            }
            _ => return false,
        };
        self.camera_effect_action(action);
        true
    }

    pub(super) fn draw_camera_effect_list(
        &self,
        u: &mut Ui,
        x: f32,
        width: f32,
        effects: CameraEffects,
    ) {
        let p = u.palette();
        let editable = self.camera_effect_editable();
        if editable {
            u.checkbox(
                [x, 1154., width, 28.],
                "Включить эффекты",
                Action::CameraTool(C::Effects),
                effects.enabled,
            );
            self.draw_dropdown_button(
                u,
                [x, 1194., width, 30.],
                DropdownKind::CameraEffectAdd,
                "Добавить эффект…",
            );
        } else {
            u.disabled_checkbox([x, 1154., width, 28.], "Включить эффекты", effects.enabled);
            u.disabled_button([x, 1194., width, 30.], "Добавить эффект…");
        }
        let height = self.camera_effect_list_height();
        u.outlined_surface([x, LIST_Y, width, height], p.panel, p.border);
        let selected = self.selected_camera_effect();
        for (row, field) in (4..=10).filter(|&f| effects.contains(f)).enumerate() {
            let rect = [x + 4., LIST_Y + 6. + row as f32 * ROW, width - 8., ROW - 2.];
            let active = selected == Some(field);
            u.quiet_button(rect, "", Action::CameraTool(C::SelectEffect(field)), active);
            if active && self.camera_effects.focused {
                u.surface_outline(rect, p.accent);
            }
            let on = effects.effect_enabled(field);
            u.fitted_label(
                [rect[0] + 8., rect[1] + 7., rect[2] - 66., 18.],
                CameraEffects::LABELS[field - 4],
                11.,
                if on { p.text } else { p.muted },
            );
            for (offset, action, icon) in [
                (58., C::ToggleEffect(field), Icon::Eye),
                (30., C::RemoveEffect(field), Icon::Close),
            ] {
                let button = [rect[0] + rect[2] - offset, rect[1] + 1., 28., 28.];
                if editable {
                    u.quiet_icon_button(
                        button,
                        "",
                        Action::CameraTool(action),
                        action == C::ToggleEffect(field) && on,
                        icon,
                    );
                } else {
                    u.disabled_icon_button(button, "", icon);
                }
                if action == C::ToggleEffect(field) && !on {
                    u.smooth_segment(
                        [button[0] + 6., button[1] + 22.],
                        [button[0] + 22., button[1] + 6.],
                        1.,
                        p.muted,
                    );
                }
            }
        }
        if selected.is_none() {
            u.fitted_label(
                [x + 10., LIST_Y + 15., width - 20., 22.],
                "Добавьте эффект из списка выше",
                11.,
                p.muted,
            );
        }
        if let Some(field) = selected {
            let (value, min, max, label) = effects.field(field).unwrap();
            let rect = [x, LIST_Y + height + 12., width, 44.];
            if editable {
                u.number_field(
                    rect,
                    label,
                    value as f64,
                    Action::CameraNumber(field),
                    SliderRange::new(min as f64, max as f64, 0.01),
                );
            } else {
                u.disabled_button(rect, &format!("{label}: {value:.2}"));
            }
        }
        u.fitted_label(
            [x, LIST_Y + height + 68., width, 22.],
            "Вид из камеры · Материал / Рендер",
            10.,
            p.muted,
        );
    }
}
