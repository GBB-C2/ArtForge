//! Camera-local effect rows reuse lens parameters, numeric editing and history.
use super::*;
use crate::ui::{CameraTool as C, DropdownKind};
use poseforge_core::CameraEffects;

const LIST_Y: f32 = 1234.;
const SETTINGS_ROW: f32 = 88.;

#[derive(Default)]
pub(super) struct State {
    selection: Option<(u64, usize)>,
    focused: bool,
    in_scene: bool,
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
                    assert_eq!(a.camera_panel_bottom(), 1894.);
                }
            }
        }
    }

    fn row_rect(a: &mut App, id: u64, field: usize, scene: bool) -> [f32; 4] {
        a.ui = a.build_ui();
        let area = if scene { a.scene_list_rect() } else { a.properties_content_rect() };
        a.ui.hits.iter().find(|hit| hit.action == Action::CameraEffect(id, C::SelectEffect(field))
            && ui::contains(area, [hit.rect[0] + 4., hit.rect[1] + hit.rect[3] * 0.5]))
            .unwrap_or_else(|| panic!("Missing effect row {field}, scene={scene}")).rect
    }
    fn scene_fields(a: &App, id: u64) -> Vec<usize> {
        a.outliner_rows().into_iter().filter_map(|row| match row {
            scene_tree::SceneEntry::CameraEffect(owner, field, _) if owner == id => Some(field),
            _ => None,
        }).collect()
    }
    #[test]
    fn camera_effect_settings_accumulate_and_follow_saved_order_in_both_themes() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for scale in [1., 2.] {
                for size in [[640., 480.], [1200., 820.]] {
                    let mut a = app(); a.preferences.theme = theme; a.scale = scale;
                    a.width = size[0]; a.height = size[1];
                    for field in [9, 4, 10, 8, 7, 5, 6] { a.action(Action::CameraTool(C::AddEffect(field))); }
                    let id = a.editor.selected.unwrap();
                    assert_eq!(scene_fields(&a, id), [9, 4, 10, 8, 7, 5, 6]);
                    let mut seen = BTreeSet::new();
                    for step in 0..=((a.inspector_max_scroll() / 80.).ceil() as usize) {
                        a.inspector_scroll = (step as f32 * 80.).min(a.inspector_max_scroll());
                        a.ui = a.build_ui();
                        for hit in &a.ui.hits {
                            if let Action::CameraNumber(field @ 4..=10) = hit.action { seen.insert(field); }
                        }
                    }
                    assert_eq!(seen, BTreeSet::from([4,5,6,7,8,9,10]));
                    let before = a.editor.project.clone();
                    a.action(Action::CameraEffect(id, C::SelectEffect(4)));
                    a.camera_confirm(&Action::CameraNumber(4), "0.75").unwrap();
                    assert_eq!(a.selected_camera_effects().unwrap().1.exposure, 0.75);
                    assert_eq!(a.selected_camera_effects().unwrap().1.grain, before.node(id).unwrap().camera.unwrap().effects.grain);
                    let loaded: Project = serde_json::from_slice(&serde_json::to_vec(&a.editor.project).unwrap()).unwrap();
                    loaded.validate().unwrap(); assert_eq!(loaded, a.editor.project);
                    assert!(a.editor.undo()); assert_eq!(a.editor.project, before);
                }
            }
        }
    }
    #[test]
    fn camera_effect_scene_rows_expand_and_controls_keep_their_camera_owner() {
        let mut a = app();
        let first = a.editor.selected.unwrap();
        for field in [9, 8] { a.action(Action::CameraTool(C::AddEffect(field))); }
        assert_eq!(scene_fields(&a, first), [9,8]);
        a.action(Action::SceneExpand(first)); assert!(scene_fields(&a, first).is_empty());
        a.action(Action::SceneExpand(first)); assert_eq!(scene_fields(&a, first), [9,8]);
        a.action(Action::CreateCamera); let second = a.editor.selected.unwrap();
        a.action(Action::CameraTool(C::AddEffect(8)));
        let second_effects = a.selected_camera_effects().unwrap().1;
        a.action(Action::CameraEffect(first, C::ToggleEffect(8)));
        assert_eq!(a.editor.selected, Some(second));
        assert_eq!(a.selected_camera_effects().unwrap().1, second_effects);
        assert!(!a.editor.project.node(first).unwrap().camera.unwrap().effects.effect_enabled(8));
        let before = a.editor.project.clone();
        a.action(Action::CameraEffect(first, C::RemoveEffect(9)));
        assert_eq!(scene_fields(&a, first), [8]);
        assert!(a.editor.undo()); assert_eq!(a.editor.project, before);
        a.editor.project.node_mut(first).unwrap().locked = true;
        let before = a.editor.project.clone();
        for command in [C::ToggleEffect(8), C::RemoveEffect(8), C::MoveEffect(8,-1)] {
            a.action(Action::CameraEffect(first, command));
        }
        assert_eq!(a.editor.project, before);
        a.editor.project.nodes.retain(|n| n.id != first);
        let before = a.editor.project.clone();
        a.action(Action::CameraEffect(first, C::RemoveEffect(8)));
        assert_eq!(a.editor.project, before);
    }
    #[test]
    fn camera_effect_drag_reorders_both_lists_once_and_supports_undo_cancel_and_stale_sources() {
        for scene in [false, true] {
            let mut a = app();
            for field in [9, 4, 8] { a.action(Action::CameraTool(C::AddEffect(field))); }
            let id = a.editor.selected.unwrap(); a.inspector_scroll = 0.;
            let start = row_rect(&mut a, id, 8, scene);
            let target = row_rect(&mut a, id, 9, scene);
            let before = a.editor.project.clone(); let undo = a.editor.undo_len();
            a.mouse_move([start[0] + 34., start[1] + 12.]); a.mouse_down(MouseButton::Left);
            assert!(matches!(a.drag, Some(Drag::Scene(_))));
            a.mouse_move([target[0] + 34., target[1] + 3.]);
            assert_eq!(a.editor.project, before, "dragging only previews the insertion");
            a.mouse_up(MouseButton::Left);
            assert_eq!(scene_fields(&a, id), [8,9,4]);
            assert_eq!(a.editor.undo_len(), undo + 1);
            assert_eq!(a.selected_camera_effects().unwrap().1.uniforms(), before.node(id).unwrap().camera.unwrap().effects.uniforms());
            assert!(a.editor.undo()); assert_eq!(a.editor.project, before);
            let start = row_rect(&mut a, id, 8, scene);
            let target = row_rect(&mut a, id, 9, scene);
            a.mouse_move([start[0] + 34., start[1] + 12.]); a.mouse_down(MouseButton::Left);
            a.mouse_move([target[0] + 34., target[1] + 3.]);
            a.cancel_drag(); a.mouse_up(MouseButton::Left); assert_eq!(a.editor.project, before);
            a.ui = a.build_ui();
            a.mouse_move([start[0] + 34., start[1] + 12.]); a.mouse_down(MouseButton::Left);
            // A replaced list with the same history length must not accept an old drag.
            a.editor.project.node_mut(id).unwrap().camera.as_mut().unwrap().effects.toggle(9);
            let changed = a.editor.project.clone();
            a.mouse_move([target[0] + 34., target[1] + 3.]); a.mouse_up(MouseButton::Left);
            assert_eq!(a.editor.project, changed);
        }
    }
    #[test]
    fn camera_effect_outliner_focus_routes_keyboard_to_effects_not_camera() {
        let mut a = app();
        for field in [9,4,8] { a.action(Action::CameraTool(C::AddEffect(field))); }
        let id = a.editor.selected.unwrap(); let count = a.editor.project.nodes.len();
        let row = row_rect(&mut a, id, 8, true);
        a.mouse_move([row[0] + 34., row[1] + 12.]); a.mouse_down(MouseButton::Left); a.mouse_up(MouseButton::Left);
        assert!(a.camera_effects.in_scene);
        a.preferences.right_panel = false;
        assert!(a.camera_effect_key(&Key::Named(NamedKey::ArrowUp), false, true));
        assert_eq!(scene_fields(&a,id), [9,8,4]);
        assert!(a.camera_effect_key(&Key::Named(NamedKey::Space), false, false));
        assert!(!a.selected_camera_effects().unwrap().1.effect_enabled(8));
        assert!(a.camera_effect_key(&Key::Named(NamedKey::Delete), false, false));
        assert_eq!(scene_fields(&a,id), [9,4]); assert_eq!(a.editor.project.nodes.len(), count);
        assert!(a.editor.undo()); assert_eq!(scene_fields(&a,id), [9,8,4]);
        assert!(a.camera_effect_key(&Key::Named(NamedKey::Enter), false, false));
        assert!(a.preferences.right_panel);
    }
    #[test]
    fn camera_effect_drag_rejects_other_cameras_outside_rows_and_new_locks() {
        let mut a = app();
        let first = a.editor.selected.unwrap();
        for field in [9,8] { a.action(Action::CameraTool(C::AddEffect(field))); }
        a.action(Action::CreateCamera); let second = a.editor.selected.unwrap();
        a.action(Action::CameraTool(C::AddEffect(8)));
        for target in [Some(second), None] {
            let source = row_rect(&mut a, first, 9, true);
            let destination = target.map(|id| row_rect(&mut a, id, 8, true));
            let before = a.editor.project.clone(); let undo = a.editor.undo_len();
            a.mouse_move([source[0] + 34., source[1] + 12.]); a.mouse_down(MouseButton::Left);
            a.mouse_move(destination.map_or([450., 10.], |r| [r[0] + 34., r[1] + 3.]));
            a.mouse_up(MouseButton::Left);
            assert_eq!(a.editor.project, before); assert_eq!(a.editor.undo_len(), undo);
        }
        let source = row_rect(&mut a, first, 9, true);
        let target = row_rect(&mut a, first, 8, true);
        a.mouse_move([source[0] + 34., source[1] + 12.]); a.mouse_down(MouseButton::Left);
        a.editor.project.node_mut(first).unwrap().locked = true;
        let before = a.editor.project.clone();
        a.mouse_move([target[0] + 34., target[1] + 22.]); a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.project, before);
    }
}

impl App {
    pub(super) fn selected_camera_effects(&self) -> Option<(u64, CameraEffects)> {
        let id = self.selected_camera_id()?;
        Some((id, self.editor.project.node(id)?.camera?.effects))
    }
    pub(super) fn camera_effect_editable(&self) -> bool {
        self.selected_camera_id().is_some_and(|id| self.camera_effect_node_editable(id))
    }
    pub(super) fn camera_effect_node_editable(&self, id: u64) -> bool {
        self.editor.project.node(id).is_some_and(|n| n.camera.is_some())
            && self.node_selectable(id)
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
            .or_else(|| effects.ordered_fields().next())
    }
    pub(super) fn camera_effect_selected(&self, id: u64, field: usize) -> bool {
        self.selected_camera_id() == Some(id) && self.selected_camera_effect() == Some(field)
    }
    fn reveal_camera_effect_in_scene(&mut self) {
        let Some((id, field)) = self.selected_camera_id().zip(self.selected_camera_effect()) else { return; };
        let Some(index) = self.outliner_rows().iter().position(|row| matches!(row,
            scene_tree::SceneEntry::CameraEffect(owner, effect, _) if *owner == id && *effect == field)) else { return; };
        let page = self.scene_capacity();
        if index < self.scene_scroll { self.scene_scroll = index; }
        else if index >= self.scene_scroll + page { self.scene_scroll = index + 1 - page; }
    }
    pub(super) fn camera_panel_bottom(&self) -> f32 {
        let count = self.selected_camera_effects().map_or(0, |(_, effects)| effects.ordered_fields().count());
        LIST_Y + count.max(1) as f32 * SETTINGS_ROW + 44.
    }
    fn camera_effect_settings_y(&self, field: usize) -> f32 {
        let row = self.selected_camera_effects().and_then(|(_, effects)| effects.ordered_fields().position(|f| f == field)).unwrap_or(0);
        LIST_Y + row as f32 * SETTINGS_ROW
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
        if !matches!(action, C::AddEffect(_) | C::SelectEffect(_) | C::ToggleEffect(_) | C::RemoveEffect(_) | C::MoveEffect(..)) { return false; }
        if let Some(id) = self.selected_camera_id() { self.camera_effect_node_action(id, action); }
        true
    }
    pub(super) fn camera_effect_node_action(&mut self, id: u64, action: C) -> bool {
        let (C::AddEffect(field)
        | C::SelectEffect(field)
        | C::ToggleEffect(field)
        | C::RemoveEffect(field)
        | C::MoveEffect(field, _)) = action
        else {
            return false;
        };
        if !(4..=10).contains(&field) {
            return true;
        }
        if self.input.is_some() && !self.resolve_input() {
            return true;
        }
        let Some(mut effects) = self.editor.project.node(id).and_then(|n| n.camera).map(|lens| lens.effects) else {
            return true;
        };
        if action != C::SelectEffect(field) && !self.camera_effect_node_editable(id) {
            return true;
        }
        if !effects.contains(field) && action != C::AddEffect(field) {
            return true;
        }
        let before = effects;
        let old_index = before.ordered_fields().position(|f| f == field).unwrap_or(0);
        match action {
            C::AddEffect(_) => {
                if let Err(error) = effects.add(field) {
                    self.status = error.to_string();
                    return true;
                }
            }
            C::ToggleEffect(_) => effects.toggle(field),
            C::RemoveEffect(_) => effects.remove(field),
            C::MoveEffect(_, step) => {
                let fields: Vec<_> = effects.ordered_fields().collect();
                let index = (old_index as isize + step as isize).clamp(0, fields.len().saturating_sub(1) as isize) as usize;
                let remaining: Vec<_> = fields.into_iter().filter(|&f| f != field).collect();
                if let Err(error) = effects.move_before(field, remaining.get(index).copied()) {
                    self.status = error.to_string(); return true;
                }
            }
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
        if action == C::SelectEffect(field) {
            if self.selected_camera_id() != Some(id) { self.action(Action::Select(id)); }
            if self.selected_camera_id() != Some(id) { return true; }
            self.set_properties_tab(0);
            self.preferences.right_panel = true;
            self.preferences.collapsed_sections.remove(&1004);
        }
        // Eye/delete on another camera must not steal the current selection.
        if self.selected_camera_id() != Some(id) { self.update_dirty(); self.redraw(); return true; }
        self.camera_effects.selection = if effects.contains(field) {
            Some((id, field))
        } else {
            Some((
                id,
                effects.ordered_fields().nth(old_index.min(effects.ordered_fields().count().saturating_sub(1))).unwrap_or(field),
            ))
        };
        self.camera_effects.focused = true;
        if action == C::SelectEffect(field) {
            self.camera_effects.in_scene = self.preferences.left_panel && ui::contains(self.scene_list_rect(), self.mouse);
        }
        if matches!(action, C::AddEffect(_)) {
            self.camera_effects.in_scene = false;
            self.scene_tools.material_expanded.insert(id);
            self.collapsed.remove(&id);
        }
        self.dropdown_focus = None;
        self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
        if let Some(selected) = self.selected_camera_effect() {
            if matches!(action, C::AddEffect(_)) || action == C::SelectEffect(field) && self.camera_effects.in_scene {
                self.reveal_camera_effect_y(self.camera_effect_settings_y(selected), SETTINGS_ROW);
            } else if !self.camera_effects.in_scene {
                self.reveal_camera_effect_y(self.camera_effect_settings_y(selected), SETTINGS_ROW);
            }
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

    pub(super) fn move_camera_effect_before(&mut self, id: u64, field: usize, before: Option<usize>) {
        if !self.camera_effect_node_editable(id) { return; }
        let Some(mut effects) = self.editor.project.node(id).and_then(|n| n.camera).map(|lens| lens.effects) else { return; };
        match effects.move_before(field, before) {
            Ok(true) => {},
            Ok(false) => return,
            Err(error) => { self.status = error.to_string(); return; },
        }
        let result = self.editor.transact("Порядок эффектов камеры", |p| {
            p.node_mut(id).unwrap().camera.as_mut().unwrap().effects = effects;
        });
        if self.result(result).is_some() {
            if self.selected_camera_id() == Some(id) { self.camera_effects.selection = Some((id, field)); }
            self.status = "Порядок эффектов камеры изменён · Ctrl+Z — отменить".into();
            self.update_dirty(); self.redraw();
        }
    }

    pub(super) fn camera_effect_drop_target(&self, owner: u64) -> Option<(usize, [f32; 4])> {
        self.ui.hits.iter().find_map(|hit| match hit.action {
            Action::CameraEffect(id, C::SelectEffect(field)) if id == owner && ui::contains(hit.rect, self.mouse) => Some((field, hit.rect)),
            _ => None,
        })
    }

    pub(super) fn camera_effect_pointer_focus(&mut self) {
        if !matches!(
            self.ui.hit(self.mouse),
            Some(Action::CameraTool(
                C::SelectEffect(_) | C::AddEffect(_) | C::ToggleEffect(_) | C::RemoveEffect(_) | C::MoveEffect(..)
            )) | Some(Action::CameraEffect(..)) | Some(Action::DropdownOpen(DropdownKind::CameraEffectAdd))
        ) {
            self.camera_effects.focused = false;
        }
    }
    pub(super) fn camera_effect_key(&mut self, key: &Key, ctrl: bool, alt: bool) -> bool {
        if !self.camera_effects.focused
            || ctrl
            || (!self.camera_effects.in_scene && (self.surface.tab != 0 || !self.preferences.right_panel))
            || (self.camera_effects.in_scene && !self.preferences.left_panel)
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
        if alt {
            let Some(field) = self.selected_camera_effect() else { return false; };
            let step = match key { Key::Named(NamedKey::ArrowUp) => -1, Key::Named(NamedKey::ArrowDown) => 1, _ => return false };
            self.camera_effect_action(C::MoveEffect(field, step));
            if self.camera_effects.in_scene { self.reveal_camera_effect_in_scene(); }
            return true;
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
        let fields: Vec<_> = effects.ordered_fields().collect();
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
                self.preferences.right_panel = true;
                self.set_properties_tab(0);
                self.preferences.collapsed_sections.remove(&1004);
                self.reveal_camera_effect_y(self.camera_effect_settings_y(field), SETTINGS_ROW);
                self.action(Action::CameraNumber(field));
                self.redraw();
                return true;
            }
            _ => return false,
        };
        let in_scene = self.camera_effects.in_scene;
        self.camera_effect_action(action);
        self.camera_effects.in_scene = in_scene;
        if in_scene { self.reveal_camera_effect_in_scene(); }
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
        let Some(id) = self.selected_camera_id() else { return; };
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
        if effects.ordered_fields().next().is_none() {
            u.fitted_label([x, LIST_Y + 10., width, 22.], "Добавьте эффект из списка выше", 11., p.muted);
        }
        for field in effects.ordered_fields() {
            let (value, min, max, label) = effects.field(field).unwrap();
            let y = self.camera_effect_settings_y(field);
            let color = if effects.effect_enabled(field) { p.text } else { p.muted };
            let header = [x, y, width, 26.];
            u.hits.push(ui::Hit { rect: header, action: Action::CameraEffect(id, C::SelectEffect(field)) });
            if self.camera_effects.focused && self.camera_effect_selected(id, field) {
                u.rounded_rect([x, y + 5., 2., 16.], p.accent, 1.);
            }
            Icon::Wand.draw(u, [x + 4., y + 4., 18., 18.], color);
            u.fitted_label([x + 28., y + 5., width - 88., 18.], CameraEffects::LABELS[field - 4], 12., color);
            self.draw_camera_effect_buttons(u, header, id, field, effects.effect_enabled(field));
            let rect = [x, y + 26., width, 44.];
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
            [x, self.camera_panel_bottom() - 32., width, 22.],
            "Вид из камеры · Материал / Рендер",
            10.,
            p.muted,
        );
    }

    pub(super) fn draw_camera_effect_buttons(&self, u: &mut Ui, rect: [f32; 4], id: u64, field: usize, on: bool) {
        let eye = [rect[0] + rect[2] - 58., rect[1] + 1., 28., rect[3] - 2.];
        let close = [rect[0] + rect[2] - 30., rect[1] + 1., 28., rect[3] - 2.];
        if self.camera_effect_node_editable(id) {
            u.eye_button(eye, Action::CameraEffect(id, C::ToggleEffect(field)), on);
            u.scene_close_button(close, Action::CameraEffect(id, C::RemoveEffect(field)));
        } else {
            let p = u.palette();
            for (r, icon) in [(eye, Icon::Eye), (close, Icon::Close)] {
                icon.draw(u, [r[0] + (r[2] - 16.) * 0.5, r[1] + (r[3] - 16.) * 0.5, 16., 16.], p.disabled);
            }
            if !on {
                u.smooth_segment([eye[0] + 8., eye[1] + eye[3] - 6.], [eye[0] + 20., eye[1] + 6.], 1.5, p.disabled);
            }
        }
    }
}
