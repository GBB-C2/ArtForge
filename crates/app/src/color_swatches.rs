//! Shared color-field commands and copy-on-drop gestures, without preview edits.
use super::*;
use crate::color_picker::{ColorControl, ColorDraft};

#[derive(Default)]
pub(super) struct Swatches {
    menu: Option<SwatchMenu>,
    copied: Option<[f32; 4]>,
}
struct SwatchMenu {
    source: Action,
    selection: Option<u64>,
    position: [f32; 2],
    candidate: usize,
    paste: Option<[f32; 4]>,
}
#[derive(Clone)]
pub(super) struct SwatchDrag {
    source: Action,
    rgba: [f32; 4],
    start: [f32; 2],
    moved: bool,
}

impl App {
    pub(super) fn swatch_value(&self, source: &Action) -> Option<[f32; 4]> {
        let mut alpha = 1.;
        let rgb = match *source {
            Action::BoneDisplay(
                kind @ (ui::BoneDisplayAction::BodyColor | ui::BoneDisplayAction::JointColor),
            ) => {
                let d = self.bone_display_node()?.bone_display.unwrap_or_default();
                let rgb = if kind == ui::BoneDisplayAction::JointColor {
                    d.joint_color
                } else {
                    d.color
                };
                return Some([rgb[0], rgb[1], rgb[2], 1.]);
            }
            Action::SurfaceMaterialColor(emissive) => {
                let m = self.editor.project.material(self.selected_material_id()?)?;
                if emissive {
                    surfaces::emissive_tint(m.emissive)
                } else {
                    alpha = m.base_color[3];
                    [m.base_color[0], m.base_color[1], m.base_color[2]]
                }
            }
            Action::SurfaceAttenuationColor => {
                let m = self.editor.project.material(self.selected_material_id()?)?;
                m.attenuation_color
                    .unwrap_or([m.base_color[0], m.base_color[1], m.base_color[2]])
            }
            Action::SurfaceNprColor(field) => npr_controls::npr_color(
                self.editor.project.material(self.selected_material_id()?)?,
                field,
            )?,
            Action::SurfaceLightColor => {
                self.editor
                    .project
                    .lights
                    .get(self.surface_light_index()?)?
                    .color
            }
            Action::SurfaceLightVolumeColor => {
                self.editor
                    .project
                    .lights
                    .get(self.surface_light_index()?)?
                    .volume
                    .tint
            }
            Action::RenderColor(field) => {
                let r = self.editor.project.render_settings;
                match field {
                    0 => r.sky_color,
                    1 => r.ground_color,
                    2 => r.background_color,
                    4 => r.atmosphere.color,
                    5 => r.npr_contours.color,
                    _ => return None,
                }
            }
            _ => return None,
        }
        .map(surfaces::linear_to_srgb)
        .map(|v| v.clamp(0., 1.));
        Some([rgb[0], rgb[1], rgb[2], alpha])
    }

    fn swatch_editable(&self, source: &Action) -> bool {
        if self.job.is_some()
            || self.scene_write_pending()
            || self.native_picker.is_some()
            || self.editor.has_staged()
        {
            return false;
        }
        match source {
            Action::BoneDisplay(_) => self.bone_display_editable(),
            Action::SurfaceMaterialColor(_)
            | Action::SurfaceAttenuationColor
            | Action::SurfaceNprColor(_) => self.material_authoring_available(),
            Action::SurfaceLightColor | Action::SurfaceLightVolumeColor => self
                .surface_light_index()
                .is_some_and(|i| !self.editor.project.light_locked(i)),
            _ => self.swatch_value(source).is_some(),
        }
    }

    fn swatch_hit(&self) -> Option<Action> {
        match self.ui.hit(self.mouse)? {
            Action::OpenColorPicker(source) if self.swatch_value(&source).is_some() => {
                Some(*source)
            }
            _ => None,
        }
    }

    fn clipboard_color(&mut self) -> Option<[f32; 4]> {
        if let Some(clipboard) = &mut self.clipboard {
            // Invalid external content must never paste an older internal color.
            if let Ok(text) = clipboard.get_text() {
                let mut draft = ColorDraft::new([0., 0., 0., 1.], true);
                return draft
                    .set_text(ColorControl::Hex, text.trim())
                    .ok()
                    .map(|_| draft.rgba());
            }
        }
        self.swatches.copied
    }

    pub(super) fn begin_swatch_pointer(&mut self, button: MouseButton) -> bool {
        if self.color_picker.is_some()
            || self.has_modal()
            || self.dropdown.is_some()
            || self.menu.is_some()
        {
            return false;
        }
        let Some(source) = self.swatch_hit() else {
            return false;
        };
        // An active HEX editor owns left-button caret and text-selection drags.
        if button == MouseButton::Left
            && self
                .input
                .as_ref()
                .is_some_and(|input| input.purpose == source)
            && self
                .active_field_rect()
                .is_some_and(|rect| ui::contains(rect, self.mouse))
        {
            return false;
        }
        if !matches!(button, MouseButton::Left | MouseButton::Right) {
            return true;
        }
        if !self.resolve_input() || !self.swatch_editable(&source) {
            return true;
        }
        let Some(rgba) = self.swatch_value(&source) else {
            return true;
        };
        if button == MouseButton::Right {
            let paste = self.clipboard_color();
            self.swatches.menu = Some(SwatchMenu {
                source,
                selection: self.editor.selected,
                position: self.mouse,
                candidate: 0,
                paste,
            });
            self.ui = self.build_ui();
        } else {
            self.drag = Some(Drag::Swatch(SwatchDrag {
                source,
                rgba,
                start: self.mouse,
                moved: false,
            }));
            self.drag_button = Some(MouseButton::Left);
        }
        self.redraw();
        true
    }

    pub(super) fn update_swatch_drag(&mut self, mut drag: SwatchDrag) {
        drag.moved |= (self.mouse[0] - drag.start[0]).hypot(self.mouse[1] - drag.start[1]) > 4.;
        self.drag = Some(Drag::Swatch(drag));
    }

    pub(super) fn apply_swatch(&mut self, source: Action, rgba: [f32; 4]) {
        if !self.swatch_editable(&source) {
            return;
        }
        self.open_color_picker(source);
        let Some(session) = &mut self.color_picker else {
            return;
        };
        session.apply_to_all = true;
        if let Err(error) = session.picker.draft.set_rgba(rgba) {
            self.cancel_color_picker();
            self.status = error;
            return;
        }
        if let Err(error) = self.preview_color() {
            self.cancel_color_picker();
            self.status = error;
            return;
        }
        self.commit_color_picker();
        self.ui = self.build_ui();
        self.redraw();
    }

    pub(super) fn finish_swatch_drag(&mut self, drag: SwatchDrag) {
        self.ui = self.build_ui();
        let target = self.swatch_hit();
        if !drag.moved {
            if target == Some(drag.source.clone()) {
                self.open_color_picker(drag.source);
            }
        } else if let Some(target) = target.filter(|target| *target != drag.source) {
            self.apply_swatch(target, drag.rgba);
        }
        self.redraw();
    }

    pub(super) fn swatch_menu_open(&self) -> bool {
        self.swatches.menu.is_some()
    }
    pub(super) fn dismiss_swatch_menu(&mut self) {
        self.swatches.menu = None;
    }

    pub(super) fn swatch_open_key(&mut self, key: &Key) -> bool {
        if *key != Key::Named(NamedKey::ContextMenu)
            && !(*key == Key::Named(NamedKey::F10) && self.modifiers.shift_key())
        {
            return false;
        }
        let Some(source) = self
            .input
            .as_ref()
            .map(|i| i.purpose.clone())
            .filter(|a| self.swatch_value(a).is_some())
        else {
            return false;
        };
        let anchor = self
            .active_field_rect()
            .map(|r| [r[0], r[1] + r[3]])
            .unwrap_or(self.mouse);
        if !self.resolve_input() || !self.swatch_editable(&source) {
            return true;
        }
        let paste = self.clipboard_color();
        self.swatches.menu = Some(SwatchMenu {
            source,
            selection: self.editor.selected,
            position: anchor,
            candidate: 0,
            paste,
        });
        self.ui = self.build_ui();
        self.redraw();
        true
    }

    fn swatch_command_enabled(&self, index: usize) -> bool {
        let Some(menu) = &self.swatches.menu else {
            return false;
        };
        if menu.selection != self.editor.selected || !self.swatch_editable(&menu.source) {
            return false;
        }
        match index {
            0 | 3 => true,
            1 => self.editor.undo_len() > 0,
            2 => self.editor.can_redo(),
            4 => menu.paste.is_some(),
            _ => false,
        }
    }

    pub(super) fn swatch_menu_action(&mut self, action: &Action) -> bool {
        if let Action::SwatchCommand(index) = *action {
            if !self.swatch_command_enabled(index) {
                return true;
            }
            let menu = self.swatches.menu.take().unwrap();
            match index {
                0 => self.reset_color(menu.source),
                1 => self.action(Action::Undo),
                2 => self.action(Action::Redo),
                3 => {
                    if let Some(rgba) = self.swatch_value(&menu.source) {
                        self.swatches.copied = Some(rgba);
                        let text = ColorDraft::new(
                            rgba,
                            matches!(menu.source, Action::SurfaceMaterialColor(false)),
                        )
                        .hex();
                        if let Some(clipboard) = &mut self.clipboard {
                            let _ = clipboard.set_text(text);
                        }
                        self.status = "Цвет скопирован".into();
                    }
                }
                4 => {
                    if let Some(rgba) = menu.paste {
                        self.apply_swatch(menu.source, rgba);
                    }
                }
                _ => {}
            }
            self.ui = self.build_ui();
            self.redraw();
            return true;
        }
        let had_menu = self.swatches.menu.take().is_some();
        if *action == Action::SwatchDismiss || (*action == Action::Cancel && had_menu) {
            self.ui = self.build_ui();
            self.redraw();
            return true;
        }
        false
    }

    pub(super) fn swatch_menu_pointer(&mut self, button: MouseButton) -> bool {
        if self.swatches.menu.is_none() {
            return false;
        }
        if button == MouseButton::Left {
            if let Some(action) = self.ui.hit(self.mouse) {
                self.action(action);
            }
        } else {
            self.action(Action::SwatchDismiss);
        }
        true
    }

    pub(super) fn swatch_menu_key(&mut self, key: &Key) -> bool {
        if self.swatches.menu.is_none() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Escape) => self.action(Action::SwatchDismiss),
            Key::Named(NamedKey::Enter | NamedKey::Space) => {
                let index = self.swatches.menu.as_ref().unwrap().candidate;
                self.action(Action::SwatchCommand(index));
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::Tab) => {
                let previous = *key == Key::Named(NamedKey::ArrowUp)
                    || (*key == Key::Named(NamedKey::Tab) && self.modifiers.shift_key());
                let mut index = self.swatches.menu.as_ref().unwrap().candidate;
                for _ in 0..5 {
                    index = (index + if previous { 4 } else { 1 }) % 5;
                    if self.swatch_command_enabled(index) {
                        break;
                    }
                }
                self.swatches.menu.as_mut().unwrap().candidate = index;
            }
            _ => {}
        }
        self.redraw();
        true
    }

    pub(super) fn draw_swatch_menu(&self, u: &mut Ui) {
        let Some(menu) = &self.swatches.menu else {
            return;
        };
        let w = 214.;
        let h = 172.;
        let x = menu.position[0].clamp(4., (self.width - w - 4.).max(4.));
        let y = menu.position[1].clamp(4., (self.height - h - 4.).max(4.));
        let p = u.palette();
        // Glyphs are rendered after rectangles; remove labels beneath this menu.
        let [px, py, pw, ph] = [x, y, w, h].map(|v| v * u.scale);
        u.texts.retain(|text| {
            let width = text.text.chars().count() as f32 * text.size;
            text.x >= px + pw
                || text.x + width <= px
                || text.y >= py + ph
                || text.y + text.size * 1.5 <= py
        });
        u.hits.clear();
        u.hits.push(ui::Hit {
            rect: [0., 0., self.width, self.height],
            action: Action::SwatchDismiss,
        });
        u.outlined_surface([x, y, w, h], p.panel, p.border);
        for (index, label) in ["Сбросить", "Отменить", "Вернуть", "Копировать", "Вставить"]
            .into_iter()
            .enumerate()
        {
            let r = [x + 6., y + 6. + index as f32 * 32., w - 12., 30.];
            let enabled = self.swatch_command_enabled(index);
            if enabled {
                u.menu_row(
                    r,
                    Action::SwatchCommand(index),
                    menu.candidate == index || ui::contains(r, self.mouse),
                );
            }
            u.fitted_label(
                [r[0] + 8., r[1] + 7., r[2] - 16., 18.],
                label,
                12.,
                if enabled { p.text } else { p.disabled },
            );
        }
    }

    pub(super) fn draw_swatch_drag(&self, u: &mut Ui) {
        let Some(Drag::Swatch(drag)) = &self.drag else {
            return;
        };
        if !drag.moved {
            return;
        }
        if let Some(hit) = u.hits.iter().rev().find(|hit| ui::contains(hit.rect,self.mouse)
            && matches!(&hit.action, Action::OpenColorPicker(source) if **source != drag.source && self.swatch_editable(source))) {
            u.surface_outline(hit.rect, u.palette().accent);
        }
        let x = self.mouse[0].clamp(4., (self.width - 38.).max(4.));
        let y = self.mouse[1].clamp(4., (self.height - 38.).max(4.));
        u.outlined_surface(
            [x + 10., y + 10., 24., 24.],
            [drag.rgba[0], drag.rgba[1], drag.rgba[2], 1.],
            u.palette().text,
        );
    }
}
