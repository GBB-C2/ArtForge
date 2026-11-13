//! Embedded panorama library. The current map and inactive maps never duplicate
//! their large pixel buffers in memory, history or project serialization.
use super::*;
use crate::ui::WorldAction as W;
use poseforge_core::EnvironmentMap;
use std::sync::Arc;
type PanoramaPreview = (Arc<EnvironmentMap>, Vec<([f32; 4], [f32; 4])>);
#[derive(Default)]
pub(super) struct EnvironmentLibrary {
    pub open: bool,
    offset: usize,
    selected: usize,
    cache: Vec<PanoramaPreview>,
}
fn thumbnail(map: &EnvironmentMap) -> Vec<([f32; 4], [f32; 4])> {
    let mut rows = vec![];
    for y in 0..40u32 {
        let color = |x: u32| {
            let sx = ((x as f32 + 0.5) * map.width as f32 / 80.) as u32;
            let sy = ((y as f32 + 0.5) * map.height as f32 / 40.) as u32;
            let rgb =
                map.rgb[(sy.min(map.height - 1) * map.width + sx.min(map.width - 1)) as usize];
            let rgb = rgb.map(|c| {
                let c = super::surfaces::linear_to_srgb(c / (1. + c));
                (c * 31.).round() / 31.
            });
            [rgb[0], rgb[1], rgb[2], 1.]
        };
        let mut x = 0;
        while x < 80 {
            let c = color(x);
            let mut end = x + 1;
            while end < 80 && color(end) == c {
                end += 1;
            }
            rows.push(([x as f32, y as f32, (end - x) as f32, 1.], c));
            x = end;
        }
    }
    rows
}
impl App {
    fn environment_items(&self) -> Vec<(usize, Arc<EnvironmentMap>)> {
        self.editor
            .project
            .environment_map
            .iter()
            .cloned()
            .map(|m| (0, m))
            .chain(
                self.editor
                    .project
                    .environment_library
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, m)| (i + 1, m)),
            )
            .collect()
    }
    pub(super) fn environment_library_action(&mut self, action: W) -> bool {
        match action {
            W::Library => {
                self.environments.open = true;
                self.environments.offset = 0;
                self.environments.selected = 0;
                self.menu = None;
                self.dropdown = None;
            }
            W::CloseLibrary => self.environments.open = false,
            W::LibraryPage(direction) => {
                let count = self.environment_items().len();
                let maximum = count.saturating_sub(6);
                self.environments.offset = (self.environments.offset as i32 + direction * 3)
                    .clamp(0, maximum as i32) as usize;
            }
            W::ChooseEnvironment(index) => {
                if index == 0 {
                    self.environments.open = false;
                    return true;
                }
                if index > self.editor.project.environment_library.len() {
                    return true;
                }
                let r = self.editor.transact("Выбрать HDRI", |p| {
                    let selected = p.environment_library.remove(index - 1);
                    if let Some(previous) = p.environment_map.replace(selected) {
                        p.environment_library.insert(index - 1, previous);
                    }
                    p.render_settings.hdri_enabled = true;
                });
                if self.result(r).is_some() {
                    self.environments.open = false;
                    self.invalidate();
                }
            }
            W::ForgetEnvironment(index) => {
                if index > self.editor.project.environment_library.len() {
                    return true;
                }
                let r = self.editor.transact("Удалить панораму HDRI", |p| {
                    if index == 0 {
                        p.environment_map = None;
                    } else {
                        p.environment_library.remove(index - 1);
                    }
                });
                if self.result(r).is_some() {
                    self.environments.offset = self
                        .environments
                        .offset
                        .min(self.environment_items().len().saturating_sub(6));
                    self.invalidate();
                }
            }
            _ => return false,
        }
        self.redraw();
        true
    }
    pub(super) fn environment_key(&mut self, key: &Key) -> bool {
        if !self.environments.open {
            return false;
        }
        let items = self.environment_items();
        match key {
            Key::Named(NamedKey::Escape) => self.environments.open = false,
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => {
                self.environments.selected = self.environments.selected.saturating_sub(1)
            }
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowDown) => {
                self.environments.selected =
                    (self.environments.selected + 1).min(items.len().saturating_sub(1))
            }
            Key::Named(NamedKey::Enter) => {
                if let Some((id, _)) = items.get(self.environments.selected) {
                    self.environment_library_action(W::ChooseEnvironment(*id));
                }
            }
            _ => {}
        }
        if self.environments.selected < self.environments.offset {
            self.environments.offset = self.environments.selected;
        }
        if self.environments.selected >= self.environments.offset + 6 {
            self.environments.offset = self.environments.selected - 5;
        }
        self.redraw();
        true
    }
    pub(super) fn environment_scroll(&mut self, delta: f32) -> bool {
        if !self.environments.open {
            return false;
        }
        if delta.is_finite() && delta != 0. {
            self.environment_library_action(W::LibraryPage(if delta > 0. { -1 } else { 1 }));
        }
        true
    }
    pub(super) fn draw_environment_library(&mut self, u: &mut Ui) {
        if !self.environments.open {
            return;
        }
        let items = self.environment_items();
        let p = u.palette();
        let x = (self.width - 540.) * 0.5;
        let y = (self.height - 400.) * 0.5;
        u.hits.clear();
        u.rect([0., 0., self.width, self.height], [0., 0., 0., 0.65]);
        u.hits.push(ui::Hit {
            rect: [0., 0., self.width, self.height],
            action: Action::World(W::CloseLibrary),
        });
        u.surface([x, y, 540., 400.], p.panel);
        u.hits.push(ui::Hit {
            rect: [x, y, 540., 400.],
            action: Action::World(W::Library),
        });
        u.label(x + 18., y + 18., "Библиотека HDRI", 18., p.text);
        u.button(
            [x + 488., y + 12., 36., 30.],
            "×",
            Action::World(W::CloseLibrary),
            false,
        );
        u.fitted_label(
            [x + 18., y + 48., 500., 18.],
            "Панорамы встроены в проект · колесо — прокрутка · ↑/↓ · Enter",
            11.,
            p.muted,
        );
        for (row, (id, map)) in items
            .iter()
            .enumerate()
            .skip(self.environments.offset)
            .take(6)
        {
            let position = row - self.environments.offset;
            let tx = x + 18. + (position % 3) as f32 * 169.;
            let ty = y + 80. + (position / 3) as f32 * 124.;
            let selected = self.environments.selected == row;
            u.outlined_surface(
                [tx - 3., ty - 3., 160., 114.],
                p.elevated,
                if selected { p.accent } else { p.border },
            );
            u.hits.push(ui::Hit {
                rect: [tx - 3., ty - 3., 160., 114.],
                action: Action::World(W::ChooseEnvironment(*id)),
            });
            if !self
                .environments
                .cache
                .iter()
                .any(|(cached, _)| Arc::ptr_eq(cached, map))
            {
                if self.environments.cache.len() >= 16 {
                    self.environments.cache.remove(0);
                }
                self.environments.cache.push((map.clone(), thumbnail(map)));
            }
            if let Some((_, rects)) = self
                .environments
                .cache
                .iter()
                .find(|(cached, _)| Arc::ptr_eq(cached, map))
            {
                for (r, c) in rects {
                    u.rect(
                        [tx + r[0] * 1.9, ty + r[1] * 1.9, r[2] * 1.9, r[3] * 1.9],
                        *c,
                    );
                }
            }
            u.fitted_label([tx + 3., ty + 82., 146., 18.], &map.name, 11., p.text);
            if *id == 0 {
                u.label(tx + 4., ty + 4., "В сцене", 10., p.text);
            }
            u.button(
                [tx + 124., ty + 4., 26., 24.],
                "×",
                Action::World(W::ForgetEnvironment(*id)),
                false,
            );
        }
        if items.is_empty() {
            u.label(
                x + 24.,
                y + 148.,
                "Загрузите панораму HDR, PNG или JPEG.",
                15.,
                p.muted,
            );
        }
        u.button(
            [x + 18., y + 346., 172., 32.],
            "Загрузить HDRI…",
            Action::World(W::Import),
            false,
        );
        let count = items.len();
        u.label(
            x + 218.,
            y + 356.,
            format!("{count} / 16 панорам"),
            11.,
            p.muted,
        );
        for (dx, label, direction) in [(424., "‹", -1), (474., "›", 1)] {
            u.button(
                [x + dx, y + 346., 40., 32.],
                label,
                Action::World(W::LibraryPage(direction)),
                false,
            );
        }
    }
}
