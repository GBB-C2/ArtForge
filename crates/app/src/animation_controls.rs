//! Animation workspace commands, captured key gestures and preview-only playback.
use super::*;
use crate::animation_timeline::{
    self, TimelineControl as Control, TimelineHit, TimelineLayout, TimelineOptions,
};
use poseforge_core::{Clip, KeyClipboard, KeyInterpolationPreset, KeyRef};

pub(super) struct AnimationSession {
    pub focused: bool,
    pub layers_offset: usize,
    pub preview_enabled: bool,
    pub preview_start: u32,
    pub preview_end: Option<u32>,
    pub loop_enabled: bool,
    pub speed: f64,
    clipboard: Option<KeyClipboard>,
    context: Option<usize>,
    initialized: bool,
    clock: f64,
}
impl Default for AnimationSession {
    fn default() -> Self {
        Self {
            focused: false,
            layers_offset: 0,
            preview_enabled: false,
            preview_start: 0,
            preview_end: None,
            loop_enabled: true,
            speed: 1.,
            clipboard: None,
            context: None,
            initialized: false,
            clock: 0.,
        }
    }
}
#[derive(Clone)]
pub(super) struct KeyDrag {
    clip: usize,
    original: Clip,
    duration: f64,
    keys: Vec<KeyRef>,
    selection: Vec<KeyRef>,
    anchor: f32,
    frames_per_pixel: f64,
    fps: f64,
    duplicate: bool,
    delta: i32,
    pub valid: bool,
    error: Option<String>,
}
#[derive(Clone)]
pub(super) struct KeyBoxDrag {
    pub start: [f32; 2],
    pub current: [f32; 2],
    selection: Vec<KeyRef>,
    additive: bool,
    frame: f64,
}

fn union_keys(mut first: Vec<KeyRef>, second: Vec<KeyRef>) -> Vec<KeyRef> {
    let mut seen: BTreeSet<_> = first
        .iter()
        .map(|k| (k.node, k.property, k.time.to_bits()))
        .collect();
    first.extend(
        second
            .into_iter()
            .filter(|k| seen.insert((k.node, k.property, k.time.to_bits()))),
    );
    first
}
fn box_rect(start: [f32; 2], end: [f32; 2]) -> [f32; 4] {
    [
        start[0].min(end[0]),
        start[1].min(end[1]),
        (start[0] - end[0]).abs(),
        (start[1] - end[1]).abs(),
    ]
}

impl App {
    pub(super) fn animation_dock_rect(&self) -> [f32; 4] {
        let r = self.timeline.dock_rect(self.width, self.height);
        self.animation_layers_rect()
            .map_or(r, |l| [r[0], l[1], r[2], r[3] + 136.])
    }
    pub(super) fn panel_bottom(&self) -> f32 {
        if self.animation_panel_visible() {
            self.animation_dock_rect()[1] - 12.
        } else {
            self.height - 36.
        }
    }
    pub(super) fn animation_last_frame(&self) -> u32 {
        // This range is navigation/preview state, independent of PNG export.
        (self.editor.project.duration * self.editor.project.fps as f64)
            .round()
            .max(1.) as u32
    }
    fn configured_preview_range(&self) -> [u32; 2] {
        let last = self.animation_last_frame();
        let end = self.animation.preview_end.unwrap_or(last).clamp(1, last);
        [self.animation.preview_start.min(end - 1), end]
    }
    pub(super) fn animation_preview_range(&self) -> [u32; 2] {
        if self.animation.preview_enabled {
            self.configured_preview_range()
        } else {
            [0, self.animation_last_frame()]
        }
    }
    pub(super) fn timeline_layout(&self) -> TimelineLayout {
        TimelineLayout::new(
            &self.editor.project,
            self.editor.active_clip,
            self.editor.selected,
            &self.timeline,
            self.width,
            self.height,
        )
    }
    pub(super) fn sync_animation_context(&mut self) {
        if !self.animation.initialized || self.animation.context != self.editor.active_clip {
            self.animation.context = self.editor.active_clip;
            self.animation.initialized = true;
            self.animation.preview_enabled = false;
            self.animation.preview_start = 0;
            self.animation.preview_end = None;
            self.timeline.selection.clear();
            self.timeline.row_offset = 0;
            self.timeline.fit(animation_timeline::total_frames(
                &self.editor.project,
                self.editor.active_clip,
            ));
            self.animation_overlay.invalidate();
        }
    }
    pub(super) fn prune_animation_selection(&mut self) {
        self.sync_animation_context();
        self.timeline
            .retain_valid_selection(&self.editor.project, self.editor.active_clip);
    }
    pub(super) fn draw_animation_timeline(&mut self, u: &mut Ui) {
        let layout = self.timeline_layout();
        let range = self.configured_preview_range();
        let options = TimelineOptions {
            layers_open: self.preferences.animation_layers_open,
            playing: self.editor.playing,
            auto_key: self.editor.auto_key,
            preview_enabled: self.animation.preview_enabled,
            preview_start: range[0] as f64,
            preview_end: range[1] as f64,
            loop_enabled: self.animation.loop_enabled,
            playback_speed: self.animation.speed,
            clipboard_available: self
                .animation
                .clipboard
                .as_ref()
                .is_some_and(|c| !c.is_empty()),
            can_key: self.editor.selected.is_some(),
            ghosts: self.animation_overlay.ghosts,
            motion_path: self.animation_overlay.path,
            ghost_step_frames: self.animation_overlay.ghost_step_frames,
            pointer: Some(self.mouse),
        };
        let character = self.current_profile().is_some();
        animation_timeline::draw_timeline(
            u,
            &layout,
            &self.editor.project,
            self.editor.active_clip,
            self.editor.selected,
            self.editor.time,
            &self.timeline,
            &options,
            |control| match control {
                Control::Play => Action::Play,
                Control::Start => Action::Start,
                Control::PreviousKey => Action::PreviousKey,
                Control::NextKey => Action::NextKey,
                Control::KeyPose => {
                    if character {
                        Action::KeyPose
                    } else {
                        Action::Key
                    }
                }
                Control::AutoKey => Action::AutoKey,
                Control::Clips => Action::AnimationPanel,
                Control::CurrentFrame => Action::EditFrame,
                _ => Action::Animation(control),
            },
        );
        // The ruler remains a regular action for focus and keyboard discovery.
        u.hits.push(ui::Hit {
            rect: layout.ruler,
            action: Action::Timeline,
        });
        if let Some(Drag::KeyBox(drag)) = &self.drag {
            if let Some(rect) = intersect_rect(box_rect(drag.start, drag.current), layout.grid) {
                let p = u.palette();
                u.rect(rect, [p.accent[0], p.accent[1], p.accent[2], 0.13]);
                u.rect([rect[0], rect[1], rect[2], 1.], p.accent);
                u.rect([rect[0], rect[1] + rect[3] - 1., rect[2], 1.], p.accent);
                u.rect([rect[0], rect[1], 1., rect[3]], p.accent);
                u.rect([rect[0] + rect[2] - 1., rect[1], 1., rect[3]], p.accent);
            }
        }
    }
    pub(super) fn animation_action(&mut self, control: Control) {
        self.animation.focused = true;
        self.sync_animation_context();
        match control {
            Control::ToggleExpanded => {
                self.timeline.expanded = !self.timeline.expanded;
                self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
                self.scene_scroll = self.scene_scroll.min(
                    self.scene_rows()
                        .len()
                        .saturating_sub(self.scene_capacity()),
                );
            }
            Control::PreviewRange => {
                self.editor.playing = false;
                self.animation.preview_enabled = !self.animation.preview_enabled;
            }
            Control::Loop => self.animation.loop_enabled = !self.animation.loop_enabled,
            Control::PreviewStart | Control::PreviewEnd => {
                let range = self.configured_preview_range();
                self.input(
                    Action::Animation(control),
                    range[if control == Control::PreviewStart {
                        0
                    } else {
                        1
                    }]
                    .to_string(),
                );
            }
            Control::Speed => self.input(
                Action::Animation(control),
                format!("{}", self.animation.speed),
            ),
            Control::EditGhostStep => self.input(
                Action::Animation(control),
                self.animation_overlay.ghost_step_frames.to_string(),
            ),
            Control::ToggleGhosts => self.animation_overlay.ghosts = !self.animation_overlay.ghosts,
            Control::ToggleMotionPath => self.animation_overlay.path = !self.animation_overlay.path,
            Control::ZoomIn | Control::ZoomOut => {
                let total =
                    animation_timeline::total_frames(&self.editor.project, self.editor.active_clip);
                self.timeline.zoom(
                    self.editor.time * self.editor.project.fps as f64,
                    if control == Control::ZoomIn {
                        1.6
                    } else {
                        1. / 1.6
                    },
                    total,
                );
            }
            Control::Fit => self.timeline.fit(animation_timeline::total_frames(
                &self.editor.project,
                self.editor.active_clip,
            )),
            Control::OnlySelected => {
                self.timeline.only_selected = !self.timeline.only_selected;
                self.timeline.row_offset = 0;
                self.timeline.selection.clear();
            }
            Control::SelectAll => {
                self.timeline.selection = self
                    .editor
                    .active_clip
                    .and_then(|i| self.editor.project.clips.get(i))
                    .map(|clip| {
                        clip.channels
                            .iter()
                            .filter(|c| {
                                !self.timeline.only_selected || Some(c.node) == self.editor.selected
                            })
                            .flat_map(|c| {
                                c.keys.iter().map(move |k| KeyRef {
                                    node: c.node,
                                    property: c.property,
                                    time: k.time,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            Control::Copy => match self.editor.copy_keys(&self.timeline.selection) {
                Ok(keys) => {
                    self.status = format!(
                        "Скопировано ключей каналов: {}. Ctrl+V — вставить от текущего кадра.",
                        keys.len()
                    );
                    self.animation.clipboard = Some(keys);
                }
                Err(error) => self.status = error.to_string(),
            },
            Control::Paste => {
                self.editor.playing = false;
                if let Some(keys) = &self.animation.clipboard {
                    let frame = (self.editor.time * self.editor.project.fps as f64).round();
                    if !frame.is_finite() || !(0.0..=u32::MAX as f64).contains(&frame) {
                        self.status =
                            "Текущий кадр вне поддерживаемого диапазона вставки ключей.".into();
                        return;
                    }
                    let result = self.editor.paste_keys(keys, frame as u32);
                    if let Some(selection) = self.result(result) {
                        self.timeline.selection = selection;
                        self.status = "Ключи вставлены. Ctrl+Z — отменить вставку.".into();
                    }
                } else {
                    self.status = "Сначала скопируйте ключи на дорожках.".into();
                }
            }
            Control::Duplicate => {
                self.editor.playing = false;
                let first = self
                    .timeline
                    .selection
                    .iter()
                    .map(|k| k.time)
                    .min_by(f64::total_cmp);
                let last = self
                    .timeline
                    .selection
                    .iter()
                    .map(|k| k.time)
                    .max_by(f64::total_cmp);
                let delta = match (first, last) {
                    (Some(a), Some(b)) => {
                        let span = ((b - a) * self.editor.project.fps as f64).ceil();
                        if !span.is_finite() || span > (i32::MAX - 1) as f64 {
                            self.status =
                                "Выделенный диапазон слишком длинный для дублирования.".into();
                            return;
                        }
                        span as i32 + 1
                    }
                    _ => 1,
                };
                let result = self.editor.duplicate_keys(&self.timeline.selection, delta);
                if let Some(selection) = self.result(result) {
                    self.timeline.selection = selection;
                    self.status = format!(
                        "Копия размещена на {delta} кадр. позже. Можно перетащить выделение."
                    );
                }
            }
            Control::Delete => {
                self.editor.playing = false;
                let result = self.editor.delete_keys(&self.timeline.selection);
                if let Some(count) = self.result(result) {
                    self.timeline.selection.clear();
                    self.status =
                        format!("Удалено ключей каналов: {count}. Объекты сцены сохранены.");
                }
            }
            Control::ShiftKeys(frames) => {
                self.editor.playing = false;
                let result = self.editor.move_keys(&self.timeline.selection, frames);
                if let Some(selection) = self.result(result) {
                    self.timeline.selection = selection;
                }
            }
            Control::Interpolation(index) => {
                let preset = match index {
                    0 => KeyInterpolationPreset::Step,
                    1 => KeyInterpolationPreset::Linear,
                    2 => KeyInterpolationPreset::EaseInOut,
                    _ => return,
                };
                self.editor.playing = false;
                let result = self
                    .editor
                    .set_key_interpolation(&self.timeline.selection, preset);
                if let Some(count) = self.result(result) {
                    self.status =
                        format!("Изменена интерполяция целых каналов: {count}. Ctrl+Z — отменить.");
                }
            }
            Control::Play => self.action(Action::Play),
            Control::Start => self.action(Action::Start),
            Control::PreviousKey => self.action(Action::PreviousKey),
            Control::NextKey => self.action(Action::NextKey),
            Control::KeyPose => self.action(if self.current_profile().is_some() {
                Action::KeyPose
            } else {
                Action::Key
            }),
            Control::AutoKey => self.action(Action::AutoKey),
            Control::Clips => self.action(Action::AnimationPanel),
            Control::CurrentFrame => self.action(Action::EditFrame),
        }
    }
    pub(super) fn animation_confirm(
        &mut self,
        action: &Action,
        value: &str,
    ) -> Result<bool, String> {
        let Action::Animation(control) = action else {
            return Ok(false);
        };
        match control {
            Control::PreviewStart | Control::PreviewEnd => {
                let value = value
                    .parse::<u32>()
                    .map_err(|_| "Кадр — целое неотрицательное число.")?;
                let mut range = self.configured_preview_range();
                range[if *control == Control::PreviewStart {
                    0
                } else {
                    1
                }] = value;
                if range[0] >= range[1] || range[1] > self.animation_last_frame() {
                    return Err(format!(
                        "Укажите 0 ≤ начало < конец ≤ {}.",
                        self.animation_last_frame()
                    ));
                }
                self.animation.preview_start = range[0];
                self.animation.preview_end = Some(range[1]);
                self.animation.preview_enabled = true;
            }
            Control::Speed => {
                let speed = value
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && (0.1..=4.).contains(v))
                    .ok_or("Скорость предпросмотра — от 0,1 до 4.")?;
                self.animation.speed = speed;
            }
            Control::EditGhostStep => {
                self.animation_overlay.ghost_step_frames = value
                    .parse::<u32>()
                    .ok()
                    .filter(|v| (1..=120).contains(v))
                    .ok_or("Интервал поз — от 1 до 120 кадров.")?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub(super) fn toggle_animation_playback(&mut self) {
        if self.editor.has_staged() {
            self.pending_time = Some(self.editor.time);
            return;
        }
        self.editor.mode = EditorMode::Animation;
        self.sync_animation_context();
        self.editor.playing = !self.editor.playing;
        if self.editor.playing {
            self.studio.physics_running = false;
            self.destruction.preview.reset();
            self.fluid_domains.follow_timeline();
            self.animation_panel = false;
            let [start, end] = self.animation_preview_range();
            let fps = self.editor.project.fps as f64;
            let begin = start as f64 / fps;
            let end = (end as f64 / fps).min(self.editor.project.duration);
            let time = if self.editor.time < begin || self.editor.time >= end {
                begin
            } else {
                self.editor.time
            };
            self.animation.clock = time;
            if let Err(error) = self.editor.set_time(time) {
                self.status = error.to_string();
                self.editor.playing = false;
            }
        }
        self.last_tick = Instant::now();
    }
    pub(super) fn tick_animation(&mut self, dt: f64) {
        if !self.editor.playing || !dt.is_finite() || dt < 0. {
            return;
        }
        // Hold the clock until the requested gas frame is ready. The worker can
        // now reuse partial batches across clock changes, but animation must not
        // outrun the actual simulated FX frame.
        if self.renderer.as_ref().is_some_and(|r|r.fx_blocked()) {return;}
        let [start, end] = self.animation_preview_range();
        let fps = self.editor.project.fps as f64;
        let begin = (start as f64 / fps).min(self.editor.project.duration);
        let end = (end as f64 / fps).min(self.editor.project.duration);
        let next = self.animation.clock + dt * self.animation.speed;
        let time = if self.animation.loop_enabled {
            let period = (end - begin + 1. / fps).max(1. / fps);
            self.animation.clock = begin + (next - begin).rem_euclid(period);
            self.animation.clock.min(end)
        } else {
            self.animation.clock = next.min(end);
            if next >= end {
                self.editor.playing = false;
            }
            self.animation.clock.clamp(begin, end)
        };
        if let Err(error) = self.editor.set_time(time) {
            self.editor.playing = false;
            self.status = error.to_string();
        }
    }
    pub(super) fn timeline_pointer_down(&mut self) -> bool {
        if !self.animation_panel_visible() {
            return false;
        }
        let layout = self.timeline_layout();
        if !ui::contains(layout.rect, self.mouse) {
            return false;
        }
        self.animation.focused = true;
        self.editor.playing = false;
        match layout.hit(self.mouse) {
            Some(TimelineHit::Keys { keys, frame }) => {
                let selected: BTreeSet<_> = self
                    .timeline
                    .selection
                    .iter()
                    .map(|k| (k.node, k.property, k.time.to_bits()))
                    .collect();
                let clicked: BTreeSet<_> = keys
                    .iter()
                    .map(|k| (k.node, k.property, k.time.to_bits()))
                    .collect();
                let all_selected = clicked.iter().all(|key| selected.contains(key));
                if self.modifiers.shift_key() {
                    if all_selected {
                        self.timeline.selection.retain(|key| {
                            !clicked.contains(&(key.node, key.property, key.time.to_bits()))
                        });
                    } else {
                        self.timeline.selection = union_keys(self.timeline.selection.clone(), keys);
                    }
                    return true;
                }
                if !all_selected {
                    self.timeline.selection = keys.clone();
                }
                if let Some(first) = keys.first() {
                    if keys.iter().all(|key| key.node == first.node) {
                        self.editor.selected = Some(first.node);
                        self.sync_active_character(first.node);
                    }
                }
                if self.editor.has_staged() {
                    self.status = "Сначала запишите или отбросьте изменённую позу.".into();
                    return true;
                }
                self.set_time(frame / self.editor.project.fps as f64);
                if let Err(error) = self.editor.copy_keys(&self.timeline.selection) {
                    self.status = error.to_string();
                    return true;
                }
                let Some(index) = self.editor.active_clip else {
                    return true;
                };
                let (_, span) = self.timeline.view_bounds(animation_timeline::total_frames(
                    &self.editor.project,
                    self.editor.active_clip,
                ));
                let drag = KeyDrag {
                    clip: index,
                    original: self.editor.project.clips[index].clone(),
                    duration: self.editor.project.duration,
                    keys: self.timeline.selection.clone(),
                    selection: self.timeline.selection.clone(),
                    anchor: self.mouse[0],
                    frames_per_pixel: span / layout.grid[2].max(1.) as f64,
                    fps: self.editor.project.fps as f64,
                    duplicate: self.modifiers.alt_key(),
                    delta: 0,
                    valid: true,
                    error: None,
                };
                self.editor.begin_edit(if drag.duplicate {
                    "Копировать ключи перетаскиванием"
                } else {
                    "Переместить ключи"
                });
                self.drag = Some(Drag::Keys(drag));
                self.drag_button = Some(MouseButton::Left);
            }
            Some(TimelineHit::Ruler(frame)) => {
                self.set_time(frame.round() / self.editor.project.fps as f64);
                if self.pending_time.is_none() {
                    self.drag = Some(Drag::Timeline);
                    self.drag_button = Some(MouseButton::Left);
                }
            }
            Some(TimelineHit::Row(id)) => {
                self.editor.selected = Some(id);
                self.sync_active_character(id);
            }
            Some(TimelineHit::Empty(frame)) => {
                let drag = KeyBoxDrag {
                    start: self.mouse,
                    current: self.mouse,
                    selection: self.timeline.selection.clone(),
                    additive: self.modifiers.shift_key(),
                    frame,
                };
                if !drag.additive {
                    self.timeline.selection.clear();
                }
                self.drag = Some(Drag::KeyBox(drag));
                self.drag_button = Some(MouseButton::Left);
            }
            None => {}
        }
        self.redraw();
        true
    }
    pub(super) fn update_key_drag(&mut self, mut drag: KeyDrag, pointer: [f32; 2]) {
        let delta = ((pointer[0] as f64 - drag.anchor as f64) * drag.frames_per_pixel).round();
        if !delta.is_finite() || !(i32::MIN as f64..=i32::MAX as f64).contains(&delta) {
            self.editor.project.clips[drag.clip] = drag.original.clone();
            self.editor.project.duration = drag.duration;
            self.timeline.selection = drag.selection.clone();
            drag.valid = false;
            let error =
                "Сдвиг слишком велик; приблизьте шкалу или переносите ключи меньшими шагами.";
            drag.error = Some(error.into());
            self.status = error.into();
            self.drag = Some(Drag::Keys(drag));
            return;
        }
        let delta = delta as i32;
        // An invalid preview restores the original clip. Returning to the last
        // valid delta must regenerate that preview even though its number matches.
        if delta != drag.delta || !drag.valid {
            drag.delta = delta;
            if delta == 0 && !drag.duplicate {
                self.editor.project.clips[drag.clip] = drag.original.clone();
                self.editor.project.duration = drag.duration;
                self.timeline.selection = drag.selection.clone();
                drag.valid = true;
                drag.error = None;
                self.drag = Some(Drag::Keys(drag));
                return;
            }
            match drag
                .original
                .retimed_keys(&drag.keys, delta as f64 / drag.fps, drag.duplicate)
            {
                Ok((clip, keys)) => {
                    self.editor.project.duration = drag.duration.max(clip.duration);
                    self.editor.project.clips[drag.clip] = clip;
                    self.timeline.selection = keys;
                    drag.valid = true;
                    drag.error = None;
                    self.status = format!(
                        "Сдвиг {delta:+} кадр. · отпустите для применения · Esc — отменить"
                    );
                }
                Err(error) => {
                    self.editor.project.clips[drag.clip] = drag.original.clone();
                    self.editor.project.duration = drag.duration;
                    self.timeline.selection = drag.selection.clone();
                    drag.valid = false;
                    drag.error = Some(error.to_string());
                    self.status = error.to_string();
                }
            }
        }
        self.drag = Some(Drag::Keys(drag));
    }
    pub(super) fn update_key_box(&mut self, mut drag: KeyBoxDrag, pointer: [f32; 2]) {
        drag.current = pointer;
        if (pointer[0] - drag.start[0]).abs() + (pointer[1] - drag.start[1]).abs() >= 3. {
            let keys = self
                .timeline_layout()
                .keys_in_rect(box_rect(drag.start, pointer));
            self.timeline.selection = if drag.additive {
                union_keys(drag.selection.clone(), keys)
            } else {
                keys
            };
        }
        self.drag = Some(Drag::KeyBox(drag));
    }
    pub(super) fn finish_key_drag(&mut self, drag: KeyDrag) {
        if drag.valid {
            let result = self.editor.commit_edit();
            if self.result(result).is_none() {
                self.timeline.selection = drag.selection;
            }
        } else {
            self.editor.cancel_edit();
            self.timeline.selection = drag.selection;
            self.status = format!(
                "Сдвиг отменён: {}",
                drag.error
                    .unwrap_or_else(|| "недопустимое положение".into())
            );
            self.update_dirty();
        }
    }
    pub(super) fn finish_key_box(&mut self, drag: KeyBoxDrag) {
        if (drag.current[0] - drag.start[0]).abs() + (drag.current[1] - drag.start[1]).abs() < 3. {
            self.set_time(drag.frame.round() / self.editor.project.fps as f64);
        }
    }
    pub(super) fn restore_animation_drag(&mut self, drag: &Drag) {
        match drag {
            Drag::Keys(keys) => self.timeline.selection = keys.selection.clone(),
            Drag::KeyBox(keys) => self.timeline.selection = keys.selection.clone(),
            _ => {}
        }
    }
    pub(super) fn timeline_scroll(&mut self, delta: f32) {
        let layout = self.timeline_layout();
        let total = layout.last_frame;
        if self.modifiers.control_key()
            || self.modifiers.super_key()
            || ui::contains(layout.ruler, self.mouse)
        {
            self.timeline.zoom(
                layout.frame_at(self.mouse[0]),
                (delta as f64 * 0.16).exp(),
                total,
            );
        } else if self.modifiers.shift_key() {
            self.timeline
                .pan(-delta as f64 * self.timeline.view_span * 0.08, total);
        } else {
            self.timeline.row_offset = (self.timeline.row_offset as isize - (delta * 2.) as isize)
                .clamp(0, layout.max_row_offset as isize)
                as usize;
        }
    }
    pub(super) fn timeline_shortcut(&self, key: &Key, ctrl: bool, shift: bool) -> Option<Action> {
        if !self.animation_panel_visible() || !self.animation.focused {
            return None;
        }
        let control = match key {
            Key::Named(NamedKey::Delete) => Control::Delete,
            Key::Named(NamedKey::ArrowLeft) if self.modifiers.alt_key() => Control::ShiftKeys(-1),
            Key::Named(NamedKey::ArrowRight) if self.modifiers.alt_key() => Control::ShiftKeys(1),
            Key::Named(NamedKey::ArrowLeft) if shift => return Some(Action::PreviousKey),
            Key::Named(NamedKey::ArrowRight) if shift => return Some(Action::NextKey),
            Key::Character(ch) if ctrl => match ch.to_lowercase().as_str() {
                "a" => Control::SelectAll,
                "c" => Control::Copy,
                "v" => Control::Paste,
                "d" => Control::Duplicate,
                _ => return None,
            },
            _ => return None,
        };
        Some(Action::Animation(control))
    }
}

#[cfg(test)]
mod range_tests {
    use super::*;
    use poseforge_core::{Channel, Interpolation, Keyframe, Node, Property};

    fn animation_app(duration: f64) -> App {
        let mut app = App::new(None);
        app.preferences = Preferences::default();
        let mut project = Project {
            fps: 240,
            duration,
            nodes: vec![Node::new(1, "Кость")],
            ..Project::default()
        };
        let mut clip = Clip::new("Ключи", duration);
        clip.channels.push(Channel {
            node: 1,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            keys: [0.12537, 0.75037]
                .into_iter()
                .map(|time| Keyframe {
                    time,
                    value: vec![time as f32, 2., 3.],
                    in_tangent: Some(vec![0.1, 0.2, 0.3]),
                    out_tangent: Some(vec![-0.4, 0.5, 0.6]),
                })
                .collect(),
        });
        project.clips = vec![clip];
        project.validate().unwrap();
        app.editor = Editor::new(project);
        app.editor.active_clip = Some(0);
        app.editor.mode = EditorMode::Animation;
        app.editor.selected = Some(1);
        app.reset_workflow_selection();
        app.sync_animation_context();
        app.saved_snapshot = app.editor.project.clone();
        app.animation_action(Control::SelectAll);
        app
    }

    fn begin_key_drag(app: &mut App, duplicate: bool) {
        app.editor.copy_keys(&app.timeline.selection).unwrap();
        app.editor.begin_edit("Переместить ключи");
        app.drag = Some(Drag::Keys(KeyDrag {
            clip: 0,
            original: app.editor.project.clips[0].clone(),
            duration: app.editor.project.duration,
            keys: app.timeline.selection.clone(),
            selection: app.timeline.selection.clone(),
            anchor: 0.,
            frames_per_pixel: 10_000_000.,
            fps: app.editor.project.fps as f64,
            duplicate,
            delta: 0,
            valid: true,
            error: None,
        }));
        app.drag_button = Some(MouseButton::Left);
    }

    #[test]
    fn paste_rejects_unrepresentable_playhead_without_clamping_or_history() {
        for duration in [20_000_000., f64::MAX] {
            let mut app = animation_app(duration);
            app.animation_action(Control::Copy);
            assert!(app.animation.clipboard.is_some());
            let original = app.editor.project.clone();
            let selection = app.timeline.selection.clone();
            app.editor.set_time(duration).unwrap();
            app.action(Action::Animation(Control::Paste));
            assert_eq!(app.editor.project, original);
            assert_eq!(app.timeline.selection, selection);
            assert_eq!(app.editor.undo_len(), 0);
            assert!(!app.dirty);
            assert!(app.status.contains("диапазона вставки"));

            // The last representable destination remains valid; relative times
            // and imported values/tangents must survive the boundary check.
            let destination = u32::MAX as f64 / 240.;
            app.editor.set_time(destination).unwrap();
            app.action(Action::Animation(Control::Paste));
            assert_eq!(app.editor.undo_len(), 1);
            let keys = &app.editor.project.clips[0].channels[0].keys;
            assert_eq!(keys.len(), 4);
            assert_eq!(keys[2].time, destination);
            assert!((keys[3].time - destination - 0.625).abs() < 1e-7);
            for (source, copied) in keys[..2].iter().zip(&keys[2..]) {
                assert_eq!(copied.value, source.value);
                assert_eq!(copied.in_tangent, source.in_tangent);
                assert_eq!(copied.out_tangent, source.out_tangent);
            }
            app.action(Action::Undo);
            assert_eq!(app.editor.project, original);
        }
    }

    #[test]
    fn oversized_drag_rolls_back_a_valid_preview_without_history() {
        for duplicate in [false, true] {
            for pointer in [300., -300., f32::INFINITY, f32::NAN] {
                let mut app = animation_app(20_000_000.);
                let original = app.editor.project.clone();
                let selection = app.timeline.selection.clone();
                begin_key_drag(&mut app, duplicate);
                app.mouse_move([1., 0.]);
                assert_ne!(app.editor.project, original);
                assert!(matches!(&app.drag, Some(Drag::Keys(drag)) if drag.valid));
                assert_eq!(app.editor.undo_len(), 0);

                app.mouse_move([pointer, 0.]);
                assert!(matches!(&app.drag, Some(Drag::Keys(drag)) if !drag.valid));
                assert_eq!(app.editor.project, original);
                assert_eq!(app.timeline.selection, selection);
                app.mouse_up(MouseButton::Left);
                assert!(app.drag.is_none());
                assert_eq!(app.editor.project, original);
                assert_eq!(app.timeline.selection, selection);
                assert_eq!(app.editor.undo_len(), 0);
                assert!(!app.dirty);
            }
        }
    }

    #[test]
    fn drag_can_return_to_the_same_valid_delta_after_overflow() {
        let mut app = animation_app(20_000_000.);
        let original = app.editor.project.clone();
        begin_key_drag(&mut app, false);
        app.mouse_move([1., 0.]);
        let preview = app.editor.project.clone();
        let selection = app.timeline.selection.clone();
        assert_ne!(preview, original);
        for (source, moved) in original.clips[0].channels[0]
            .keys
            .iter()
            .zip(&preview.clips[0].channels[0].keys)
        {
            assert!((moved.time - source.time - 10_000_000. / 240.).abs() < 1e-9);
            assert_eq!(moved.value, source.value);
            assert_eq!(moved.in_tangent, source.in_tangent);
            assert_eq!(moved.out_tangent, source.out_tangent);
        }
        app.mouse_move([300., 0.]);
        assert_eq!(app.editor.project, original);
        app.mouse_move([1., 0.]);
        assert!(matches!(&app.drag, Some(Drag::Keys(drag)) if drag.valid));
        assert_eq!(app.editor.project, preview);
        assert_eq!(app.timeline.selection, selection);
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, original);
    }
}
