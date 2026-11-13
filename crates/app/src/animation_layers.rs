//! A separately collapsible layer drawer belonging to the animation dock.
use super::*;
use crate::ui::StudioAction as S;

const VISIBLE_ROWS: usize = 3;

impl App {
    pub(super) fn animation_panel_visible(&self) -> bool {
        self.editor.mode == EditorMode::Animation && self.preferences.animation_visible
    }
    pub(super) fn show_animation_layers(&mut self) {
        if self.editor.mode != EditorMode::Animation {
            self.action(Action::Mode(2));
        }
        self.preferences.animation_visible = true;
        self.preferences.animation_layers_open = true;
        self.animation.layers_offset = self.studio.layer.saturating_sub(VISIBLE_ROWS - 1);
        self.persist_preferences();
        self.redraw();
    }
    pub(super) fn animation_layers_rect(&self) -> Option<[f32; 4]> {
        if !self.animation_panel_visible() || !self.preferences.animation_layers_open {
            return None;
        }
        let r = self.timeline.dock_rect(self.width, self.height);
        Some([r[0], r[1] - 136., r[2], 128.])
    }
    fn layers_scrollbar(&self) -> Option<[f32; 4]> {
        let r = self.animation_layers_rect()?;
        Some([r[0] + (r[2] * 0.43).min(480.) - 18., r[1] + 36., 12., 78.])
    }
    fn layers_max_scroll(&self) -> usize {
        self.editor
            .project
            .animation_layers
            .len()
            .saturating_sub(VISIBLE_ROWS)
    }
    pub(super) fn animation_layers_scroll(&mut self, delta: f32) {
        if delta.is_finite() {
            self.animation.layers_offset =
                (self.animation.layers_offset as isize - (delta * 2.).round() as isize)
                    .clamp(0, self.layers_max_scroll() as isize) as usize;
        }
        self.redraw();
    }
    pub(super) fn animation_layers_scroll_to_pointer(&mut self) {
        let Some(r) = self.layers_scrollbar() else {
            return;
        };
        let max = self.layers_max_scroll();
        let count = self.editor.project.animation_layers.len();
        let thumb = (r[3] * VISIBLE_ROWS as f32 / count.max(1) as f32)
            .max(28.)
            .min(r[3]);
        let offset = self.animation.layers_offset.min(max);
        let top = r[1] + (r[3] - thumb) * offset as f32 / max.max(1) as f32;
        if self.mouse[1] < top || self.mouse[1] > top + thumb {
            self.animation.layers_offset =
                ((self.mouse[1] - r[1] - thumb * 0.5) / (r[3] - thumb).max(1.) * max as f32)
                    .round()
                    .clamp(0., max as f32) as usize;
        }
        self.drag = Some(Drag::AnimationLayersScroll {
            start: self.mouse[1],
            value: self.animation.layers_offset,
        });
        self.drag_button = Some(winit::event::MouseButton::Left);
    }
    pub(super) fn update_animation_layers_scroll(&mut self, start: f32, value: usize, y: f32) {
        let Some(r) = self.layers_scrollbar() else {
            return;
        };
        let max = self.layers_max_scroll();
        let count = self.editor.project.animation_layers.len();
        let thumb = (r[3] * VISIBLE_ROWS as f32 / count.max(1) as f32)
            .max(28.)
            .min(r[3]);
        self.animation.layers_offset = (value as f32
            + (y - start) * max as f32 / (r[3] - thumb).max(1.))
        .round()
        .clamp(0., max as f32) as usize;
    }
    pub(super) fn draw_animation_layers(&self, u: &mut Ui) {
        let Some(r) = self.animation_layers_rect() else {
            return;
        };
        let p = u.palette();
        let list_width = (r[2] * 0.43).min(480.);
        u.outlined_surface(r, p.panel, p.border);
        u.quiet_button(
            [r[0] + 6., r[1] + 4., 24., 26.],
            "−",
            Action::ToggleAnimationLayers,
            false,
        );
        u.section_label(
            [r[0] + 36., r[1] + 10., list_width - 76., 16.],
            "Слои анимации",
            12.,
        );
        u.outline_button(
            [r[0] + list_width - 32., r[1] + 5., 24., 24.],
            "+",
            Action::Studio(S::LayerAdd),
            true,
        );
        let offset = self.animation.layers_offset.min(self.layers_max_scroll());
        for (row, (index, layer)) in self
            .editor
            .project
            .animation_layers
            .iter()
            .enumerate()
            .skip(offset)
            .take(VISIBLE_ROWS)
            .enumerate()
        {
            let y = r[1] + 36. + row as f32 * 26.;
            let x = r[0] + 8.;
            let width = list_width - 32.;
            u.quiet_button(
                [x, y, width - 82., 24.],
                &layer.name,
                Action::Studio(S::LayerSelect(index)),
                index == self.studio.layer,
            );
            u.eye_button(
                [x + width - 78., y, 24., 24.],
                Action::Studio(S::LayerEnabled(index)),
                layer.enabled,
            );
            u.quiet_button(
                [x + width - 52., y, 24., 24.],
                "S",
                Action::Studio(S::LayerSolo(index)),
                layer.solo,
            );
            u.scene_close_button(
                [x + width - 26., y, 24., 24.],
                Action::Studio(S::LayerRemove(index)),
            );
        }
        if self.layers_max_scroll() > 0 {
            u.vertical_scrollbar(
                self.layers_scrollbar().unwrap(),
                offset as f32 / self.layers_max_scroll() as f32,
                VISIBLE_ROWS as f32 / self.editor.project.animation_layers.len() as f32,
                Action::AnimationLayersScroll,
            );
        }
        let x = r[0] + list_width + 12.;
        let width = r[2] - list_width - 24.;
        u.rect([x - 6., r[1] + 8., 1., r[3] - 16.], p.border);
        if let Some(layer) = self.editor.project.animation_layers.get(self.studio.layer) {
            u.fitted_label([x, r[1] + 11., width - 72., 16.], &layer.name, 11., p.muted);
            for (dx, label, up, enabled) in [
                (56., "↑", true, self.studio.layer > 0),
                (
                    26.,
                    "↓",
                    false,
                    self.studio.layer + 1 < self.editor.project.animation_layers.len(),
                ),
            ] {
                u.outline_button(
                    [x + width - dx, r[1] + 5., 24., 24.],
                    label,
                    Action::Studio(S::LayerMove(up)),
                    enabled,
                );
            }
            let clip = self
                .editor
                .project
                .clips
                .get(layer.clip)
                .map_or("Клип удалён", |c| c.name.as_str());
            let half = (width - 8.) * 0.5;
            u.button(
                [x, r[1] + 36., half, 28.],
                &format!("Клип: {clip}"),
                Action::Studio(S::LayerClip),
                false,
            );
            u.button(
                [x + half + 8., r[1] + 36., half, 28.],
                if layer.additive {
                    "Сложение"
                } else {
                    "Замещение"
                },
                Action::Studio(S::LayerBlend),
                layer.additive,
            );
            self.studio_number(u, x, r[1] + 70., width, 60);
        } else {
            u.fitted_label(
                [x, r[1] + 47., width, 40.],
                "Добавьте слой, чтобы смешивать анимационные клипы.",
                12.,
                p.muted,
            );
        }
    }
}
