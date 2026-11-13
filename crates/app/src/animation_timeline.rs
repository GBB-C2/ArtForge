//! Docked animation timing view. Layout and hit testing share logical geometry.
//! Model edits, focus, text entry and gesture transactions belong to App.
use crate::icons::Icon;
use crate::ui::{self, Action, Ui};
use poseforge_core::{Interpolation, KeyRef, NodeId, Project, Property};
use std::collections::{BTreeMap, BTreeSet};

pub const EXPANDED_HEIGHT: f32 = 248.;
pub const COMPACT_HEIGHT: f32 = 116.;
const ROW_HEIGHT: f32 = 24.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineControl {
    ToggleExpanded,
    Start,
    PreviousKey,
    Play,
    NextKey,
    KeyPose,
    AutoKey,
    Clips,
    CurrentFrame,
    PreviewStart,
    PreviewEnd,
    PreviewRange,
    Loop,
    Speed,
    ZoomIn,
    ZoomOut,
    Fit,
    Copy,
    Paste,
    Duplicate,
    Delete,
    SelectAll,
    ShiftKeys(i32),
    Interpolation(u8),
    OnlySelected,
    ToggleGhosts,
    ToggleMotionPath,
    EditGhostStep,
}
impl TimelineControl {
    pub fn is_field(self) -> bool {
        matches!(
            self,
            Self::CurrentFrame
                | Self::PreviewStart
                | Self::PreviewEnd
                | Self::Speed
                | Self::EditGhostStep
        )
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::ToggleExpanded => "Развернуть или свернуть дорожки и инструменты анимации",
            Self::Start => "Перейти к началу диапазона предпросмотра",
            Self::PreviousKey => "Предыдущий ключ · Shift+стрелка влево",
            Self::NextKey => "Следующий ключ · Shift+стрелка вправо",
            Self::Play => "Воспроизведение / пауза · Пробел",
            Self::KeyPose => "Записать позу активного персонажа или выбранный объект",
            Self::AutoKey => "Автоматически записывать изменения позы в текущий кадр",
            Self::Clips => "Выбор, создание и переименование клипов",
            Self::CurrentFrame => {
                "Перейти к точному кадру · стрелки влево/вправо — по одному кадру"
            }
            Self::PreviewStart => "Первый кадр предпросмотра; границы экспорта не меняются",
            Self::PreviewEnd => "Последний кадр предпросмотра; границы экспорта не меняются",
            Self::PreviewRange => "Ограничить предпросмотр указанными кадрами",
            Self::Loop => "Повторять выбранный диапазон после последнего кадра",
            Self::Speed => "Скорость предпросмотра; FPS проекта и ключи не меняются",
            Self::ZoomIn => "Приблизить шкалу времени",
            Self::ZoomOut => "Отдалить шкалу времени",
            Self::Fit => "Показать полный диапазон времени",
            Self::Copy => "Скопировать выделенные ключи во внутренний буфер · Ctrl+C",
            Self::Paste => "Вставить ключи, начиная с текущего кадра · Ctrl+V",
            Self::Duplicate => "Создать копию выделенных ключей",
            Self::Delete => "Удалить выделенные ключи; объекты сцены сохраняются · Delete",
            Self::SelectAll => {
                "Выделить все ключи показанных дорожек, включая ключи вне масштаба · Ctrl+A"
            }
            Self::ShiftKeys(_) => {
                "Сдвинуть выделенные ключи на один кадр · Alt+стрелка влево/вправо"
            }
            Self::Interpolation(0) => "Ступенчатый переход во всех каналах выделенных ключей",
            Self::Interpolation(1) => "Линейный переход во всех каналах выделенных ключей",
            Self::Interpolation(_) => "Плавный вход и выход во всех каналах выделенных ключей",
            Self::OnlySelected => "Показывать дорожку только выбранного объекта или кости",
            Self::ToggleGhosts => "Показать соседние позы скелета для сравнения движения",
            Self::ToggleMotionPath => "Показать траекторию выбранного объекта или сустава",
            Self::EditGhostStep => "Интервал между соседними позами, в кадрах",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimelineState {
    pub expanded: bool,
    pub selection: Vec<KeyRef>,
    /// The visible range is in frames. Key references keep exact source seconds.
    pub view_start: f64,
    pub view_span: f64,
    pub row_offset: usize,
    pub only_selected: bool,
}
impl Default for TimelineState {
    fn default() -> Self {
        Self {
            expanded: true,
            selection: Vec::new(),
            view_start: 0.,
            view_span: 120.,
            row_offset: 0,
            only_selected: false,
        }
    }
}
impl TimelineState {
    pub fn height(&self) -> f32 {
        if self.expanded {
            EXPANDED_HEIGHT
        } else {
            COMPACT_HEIGHT
        }
    }

    pub fn dock_rect(&self, width: f32, height: f32) -> [f32; 4] {
        [
            12.,
            height - 32. - self.height(),
            (width - 24.).max(1.),
            self.height(),
        ]
    }

    pub fn fit(&mut self, last_frame: f64) {
        self.view_start = 0.;
        self.view_span = valid_total(last_frame);
    }

    pub fn view_bounds(&self, last_frame: f64) -> (f64, f64) {
        let total = valid_total(last_frame);
        let span = if self.view_span.is_finite() {
            self.view_span.clamp(1., total)
        } else {
            total
        };
        let start = if self.view_start.is_finite() {
            self.view_start.clamp(0., (total - span).max(0.))
        } else {
            0.
        };
        (start, span)
    }

    /// Factors above one zoom in while preserving the anchor's screen position.
    pub fn zoom(&mut self, anchor_frame: f64, factor: f64, last_frame: f64) {
        if !factor.is_finite() || factor <= 0. {
            return;
        }
        let total = valid_total(last_frame);
        let (start, span) = self.view_bounds(total);
        let anchor = if anchor_frame.is_finite() {
            anchor_frame.clamp(start, start + span)
        } else {
            start + span * 0.5
        };
        let ratio = (anchor - start) / span;
        let next_span = (span / factor).clamp(total.min(4.), total);
        self.view_span = next_span;
        self.view_start = (anchor - ratio * next_span).clamp(0., (total - next_span).max(0.));
    }

    pub fn pan(&mut self, delta_frames: f64, last_frame: f64) {
        if !delta_frames.is_finite() {
            return;
        }
        let total = valid_total(last_frame);
        let (start, span) = self.view_bounds(total);
        self.view_span = span;
        self.view_start = (start + delta_frames).clamp(0., (total - span).max(0.));
    }

    pub fn retain_valid_selection(&mut self, project: &Project, active_clip: Option<usize>) {
        let Some(clip) = active_clip.and_then(|i| project.clips.get(i)) else {
            self.selection.clear();
            return;
        };
        let channels: BTreeMap<_, _> = clip
            .channels
            .iter()
            .map(|channel| ((channel.node, channel.property), &channel.keys))
            .collect();
        self.selection.retain_mut(|selected| {
            if !selected.time.is_finite() {
                return false;
            }
            let Some(keys) = channels.get(&(selected.node, selected.property)) else {
                return false;
            };
            let index = keys.partition_point(|key| key.time < selected.time);
            let found = [Some(index), index.checked_sub(1)]
                .into_iter()
                .flatten()
                .filter_map(|index| keys.get(index))
                .find(|key| (key.time - selected.time).abs() < 1e-9);
            if let Some(key) = found {
                selected.time = key.time;
                true
            } else {
                false
            }
        });
    }
}

#[derive(Debug, Clone)]
pub struct TimelineOptions {
    pub layers_open: bool,
    pub playing: bool,
    pub auto_key: bool,
    pub preview_enabled: bool,
    pub preview_start: f64,
    pub preview_end: f64,
    pub loop_enabled: bool,
    pub playback_speed: f64,
    pub clipboard_available: bool,
    pub can_key: bool,
    pub ghosts: bool,
    pub motion_path: bool,
    pub ghost_step_frames: u32,
    pub pointer: Option<[f32; 2]>,
}
impl Default for TimelineOptions {
    fn default() -> Self {
        Self {
            playing: false,
            layers_open: false,
            auto_key: false,
            preview_enabled: false,
            preview_start: 0.,
            preview_end: 120.,
            loop_enabled: true,
            playback_speed: 1.,
            clipboard_available: false,
            can_key: false,
            ghosts: false,
            motion_path: false,
            ghost_step_frames: 6,
            pointer: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimelineRow {
    pub node: Option<NodeId>,
    pub label: String,
    pub rect: [f32; 4],
    pub locked: bool,
}

#[derive(Debug, Clone)]
pub struct TimelineMarker {
    pub rect: [f32; 4],
    pub keys: Vec<KeyRef>,
    pub frame: f64,
    pub node: Option<NodeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimelineHit {
    Ruler(f64),
    Keys { keys: Vec<KeyRef>, frame: f64 },
    Row(NodeId),
    Empty(f64),
}

#[derive(Debug, Clone)]
pub struct TimelineLayout {
    pub rect: [f32; 4],
    pub ruler: [f32; 4],
    pub grid: [f32; 4],
    pub rows: Vec<TimelineRow>,
    pub markers: Vec<TimelineMarker>,
    pub last_frame: f64,
    pub view_start: f64,
    pub view_span: f64,
    pub max_row_offset: usize,
    pub total_rows: usize,
}
impl TimelineLayout {
    pub fn new(
        project: &Project,
        active_clip: Option<usize>,
        selected_node: Option<NodeId>,
        state: &TimelineState,
        width: f32,
        height: f32,
    ) -> Self {
        let rect = state.dock_rect(width, height);
        let last_frame = total_frames(project, active_clip);
        let (view_start, view_span) = state.view_bounds(last_frame);
        let labels_width = 170.;
        let ruler_y = rect[1] + if state.expanded { 80. } else { 44. };
        let ruler = [
            rect[0] + labels_width,
            ruler_y,
            (rect[2] - labels_width - 10.).max(1.),
            24.,
        ];
        let grid = [
            ruler[0],
            ruler_y + 24.,
            ruler[2],
            if state.expanded { 100. } else { 28. },
        ];
        let mut by_node: BTreeMap<NodeId, Vec<KeyRef>> = BTreeMap::new();
        if let Some(clip) = active_clip.and_then(|index| project.clips.get(index)) {
            for channel in &clip.channels {
                if state.only_selected && Some(channel.node) != selected_node {
                    continue;
                }
                if channel.keys.is_empty() {
                    continue;
                }
                let row = by_node.entry(channel.node).or_default();
                let start = (view_start - 1e-8) / project.fps.max(1) as f64;
                let end = (view_start + view_span + 1e-8) / project.fps.max(1) as f64;
                let from = channel.keys.partition_point(|key| key.time < start);
                let to = channel.keys.partition_point(|key| key.time <= end);
                for key in channel.keys.get(from..to).unwrap_or_default() {
                    if !key.time.is_finite() {
                        continue;
                    }
                    row.push(KeyRef {
                        node: channel.node,
                        property: channel.property,
                        time: key.time,
                    });
                }
            }
        }
        let capacity = ((grid[3] / ROW_HEIGHT).floor() as usize).saturating_sub(1);
        let total_rows = by_node.len();
        let max_row_offset = total_rows.saturating_sub(capacity);
        let offset = state.row_offset.min(max_row_offset);
        let mut result = Self {
            rect,
            ruler,
            grid,
            last_frame,
            view_start,
            view_span,
            max_row_offset,
            total_rows,
            rows: vec![TimelineRow {
                node: None,
                label: if state.only_selected {
                    "Сводка выбранного".into()
                } else {
                    "Все дорожки".into()
                },
                rect: [rect[0] + 8., grid[1], rect[2] - 18., ROW_HEIGHT],
                locked: false,
            }],
            markers: Vec::new(),
        };
        let summary = by_node.values().flatten().copied().collect::<Vec<_>>();
        result.add_markers(summary, None, grid[1] + ROW_HEIGHT * 0.5, project.fps);
        for (index, (&node, keys)) in by_node.iter().skip(offset).take(capacity).enumerate() {
            let y = grid[1] + (index + 1) as f32 * ROW_HEIGHT;
            let name = project
                .node(node)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| format!("Узел {node}"));
            result.rows.push(TimelineRow {
                node: Some(node),
                label: if node_locked(project, node) {
                    format!("Блок · {name}")
                } else {
                    name
                },
                rect: [rect[0] + 8., y, rect[2] - 18., ROW_HEIGHT],
                locked: node_locked(project, node),
            });
            result.add_markers(keys.clone(), Some(node), y + ROW_HEIGHT * 0.5, project.fps);
        }
        result
    }

    fn add_markers(&mut self, keys: Vec<KeyRef>, node: Option<NodeId>, y: f32, fps: u32) {
        // Very dense keys share one visible marker. Zoom reveals individual times.
        // This bounds raster work while retaining every source key for selection.
        let mut clusters: BTreeMap<i32, Vec<KeyRef>> = BTreeMap::new();
        for key in keys {
            let frame = key.time * fps as f64;
            if frame < self.view_start - 1e-8 || frame > self.view_start + self.view_span + 1e-8 {
                continue;
            }
            let pixel = ((self.x_at_frame(frame) - self.grid[0]) / 3.).round() as i32;
            clusters.entry(pixel).or_default().push(key);
        }
        for keys in clusters.into_values() {
            let frame = keys
                .iter()
                .map(|key| key.time)
                .min_by(f64::total_cmp)
                .unwrap_or_default()
                * fps as f64;
            let x = self
                .x_at_frame(frame)
                .clamp(self.grid[0] + 7., self.grid[0] + self.grid[2] - 7.);
            self.markers.push(TimelineMarker {
                rect: [x - 7., y - 9., 14., 18.],
                keys,
                frame,
                node,
            });
        }
    }

    pub fn x_at_frame(&self, frame: f64) -> f32 {
        (self.grid[0] as f64 + (frame - self.view_start) / self.view_span * self.grid[2] as f64)
            as f32
    }

    /// Unclamped for drag deltas; callers clamp navigation, not the gesture origin.
    pub fn frame_at(&self, x: f32) -> f64 {
        if !x.is_finite() {
            return self.view_start;
        }
        self.view_start + (x - self.grid[0]) as f64 / self.grid[2] as f64 * self.view_span
    }

    pub fn hit(&self, point: [f32; 2]) -> Option<TimelineHit> {
        if !point.iter().all(|v| v.is_finite()) || !ui::contains(self.rect, point) {
            return None;
        }
        if ui::contains(self.ruler, point) {
            return Some(TimelineHit::Ruler(self.frame_at(point[0])));
        }
        if ui::contains(self.grid, point) {
            if let Some(marker) = self
                .markers
                .iter()
                .filter(|m| ui::contains(m.rect, point))
                .min_by(|a, b| {
                    (point[0] - (a.rect[0] + a.rect[2] * 0.5))
                        .abs()
                        .total_cmp(&(point[0] - (b.rect[0] + b.rect[2] * 0.5)).abs())
                })
            {
                return Some(TimelineHit::Keys {
                    keys: marker.keys.clone(),
                    frame: marker.frame,
                });
            }
            return Some(TimelineHit::Empty(self.frame_at(point[0])));
        }
        self.rows.iter().find_map(|row| {
            if ui::contains(row.rect, point) {
                row.node.map(TimelineHit::Row)
            } else {
                None
            }
        })
    }

    pub fn keys_in_rect(&self, rect: [f32; 4]) -> Vec<KeyRef> {
        let rect = normalized_rect(rect);
        let mut keys = BTreeMap::new();
        for marker in &self.markers {
            let center = [
                marker.rect[0] + marker.rect[2] * 0.5,
                marker.rect[1] + marker.rect[3] * 0.5,
            ];
            if ui::contains(rect, center) {
                for key in &marker.keys {
                    keys.insert(key_identity(*key), *key);
                }
            }
        }
        keys.into_values().collect()
    }

    #[cfg(test)]
    pub fn visible_keys(&self) -> Vec<KeyRef> {
        self.keys_in_rect(self.grid)
    }
}

pub fn total_frames(project: &Project, active_clip: Option<usize>) -> f64 {
    let clip_duration = active_clip
        .and_then(|i| project.clips.get(i))
        .map_or(0., |c| c.duration);
    valid_total(project.duration.max(clip_duration) * project.fps.max(1) as f64)
}

fn valid_total(last_frame: f64) -> f64 {
    if last_frame.is_nan() {
        1.
    } else {
        // Finite project durations can overflow while converting to frames.
        // Positive infinity still represents the supported maximum, not frame 1.
        last_frame.clamp(1., u32::MAX as f64)
    }
}
fn key_identity(key: KeyRef) -> (NodeId, Property, u64) {
    (key.node, key.property, key.time.to_bits())
}
fn node_locked(project: &Project, id: NodeId) -> bool {
    let mut next = Some(id);
    let mut visited = BTreeSet::new();
    while let Some(id) = next {
        if !visited.insert(id) {
            return true;
        }
        let Some(node) = project.node(id) else {
            return true;
        };
        if node.locked {
            return true;
        }
        next = node.parent;
    }
    false
}
fn normalized_rect(rect: [f32; 4]) -> [f32; 4] {
    [
        rect[0].min(rect[0] + rect[2]),
        rect[1].min(rect[1] + rect[3]),
        rect[2].abs(),
        rect[3].abs(),
    ]
}
fn frame_label(frame: f64) -> String {
    if (frame - frame.round()).abs() < 0.001 {
        format!("{frame:.0}")
    } else {
        format!("{frame:.2}")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_timeline(
    u: &mut Ui,
    layout: &TimelineLayout,
    project: &Project,
    active_clip: Option<usize>,
    selected_node: Option<NodeId>,
    current_time: f64,
    state: &TimelineState,
    options: &TimelineOptions,
    action: impl Fn(TimelineControl) -> Action,
) {
    use TimelineControl as C;
    let p = u.palette();
    let [left, top, width, _] = layout.rect;
    u.outlined_surface(layout.rect, p.panel, p.border);
    let mut x = left + 8.;
    let y = top + 8.;
    u.quiet_icon_button(
        [x, y, 32., 28.],
        "",
        Action::ToggleAnimationLayers,
        options.layers_open,
        Icon::Sequence,
    );
    x += 38.;
    let button = |u: &mut Ui,
                  x: &mut f32,
                  width: f32,
                  label: &str,
                  control: C,
                  active: bool,
                  enabled: bool,
                  y: f32| {
        let rect = [*x, y, width, 28.];
        let toggle = matches!(
            control,
            C::AutoKey
                | C::OnlySelected
                | C::PreviewRange
                | C::Loop
                | C::ToggleGhosts
                | C::ToggleMotionPath
        );
        let icon = match control {
            C::Start => Some(Icon::Start),
            C::PreviousKey => Some(Icon::Previous),
            C::NextKey => Some(Icon::Next),
            C::Play => Some(if options.playing {
                Icon::Pause
            } else {
                Icon::Play
            }),
            C::KeyPose => Some(Icon::Key),
            C::Copy => Some(Icon::Copy),
            C::Paste => Some(Icon::Paste),
            C::Duplicate => Some(Icon::Duplicate),
            C::Delete => Some(Icon::Delete),
            _ => None,
        };
        let label = if matches!(control, C::Start | C::PreviousKey | C::NextKey | C::Play) {
            ""
        } else {
            label
        };
        if toggle && enabled {
            u.checkbox(rect, label, action(control), active);
        } else if toggle {
            u.disabled_checkbox(rect, label, active);
        } else if let Some(icon) = icon {
            if enabled {
                u.icon_button(rect, label, action(control), active, icon);
            } else {
                u.disabled_icon_button(rect, label, icon);
            }
        } else if enabled {
            u.button(rect, label, action(control), active);
        } else {
            u.disabled_button(rect, label);
        }
        *x += width + 6.;
    };
    button(
        u,
        &mut x,
        84.,
        "Дорожки",
        C::ToggleExpanded,
        state.expanded,
        true,
        y,
    );
    button(u, &mut x, 28., "|<", C::Start, false, true, y);
    button(
        u,
        &mut x,
        28.,
        "<",
        C::PreviousKey,
        false,
        layout.total_rows > 0,
        y,
    );
    button(
        u,
        &mut x,
        34.,
        if options.playing {
            "Пауза"
        } else {
            "Играть"
        },
        C::Play,
        options.playing,
        true,
        y,
    );
    button(
        u,
        &mut x,
        28.,
        ">",
        C::NextKey,
        false,
        layout.total_rows > 0,
        y,
    );
    button(
        u,
        &mut x,
        100.,
        "Ключ позы",
        C::KeyPose,
        false,
        options.can_key,
        y,
    );
    button(
        u,
        &mut x,
        92.,
        "Автоключ",
        C::AutoKey,
        options.auto_key,
        true,
        y,
    );
    let clip_name = active_clip
        .and_then(|i| project.clips.get(i))
        .map(|c| c.name.as_str())
        .unwrap_or("Новый клип");
    let clip_width = (width - 16. - 818.).max(96.);
    button(
        u,
        &mut x,
        clip_width,
        &format!("Клип: {clip_name}"),
        C::Clips,
        false,
        true,
        y,
    );
    u.label(x, y + 8., "Кадр", 10., p.muted);
    u.field(
        [x + 32., y, 68., 28.],
        &frame_label(current_time * project.fps as f64),
        action(C::CurrentFrame),
    );
    x += 106.;
    button(u, &mut x, 26., "−", C::ZoomOut, false, true, y);
    button(u, &mut x, 26., "+", C::ZoomIn, false, true, y);
    button(u, &mut x, 56., "Весь", C::Fit, false, true, y);
    button(
        u,
        &mut x,
        106.,
        "Выбранный",
        C::OnlySelected,
        state.only_selected,
        true,
        y,
    );

    if state.expanded {
        x = left + 8.;
        let y = top + 44.;
        button(
            u,
            &mut x,
            100.,
            "Диапазон",
            C::PreviewRange,
            options.preview_enabled,
            true,
            y,
        );
        for (label, value, control, label_width) in [
            ("С", options.preview_start, C::PreviewStart, 12.),
            ("По", options.preview_end, C::PreviewEnd, 20.),
        ] {
            u.label(x, y + 8., label, 10., p.muted);
            u.field(
                [x + label_width, y, 62., 28.],
                &frame_label(value),
                action(control),
            );
            x += label_width + 70.;
        }
        button(
            u,
            &mut x,
            58.,
            "Цикл",
            C::Loop,
            options.loop_enabled,
            true,
            y,
        );
        u.label(x, y + 8., "Скорость", 10., p.muted);
        u.field(
            [x + 56., y, 52., 28.],
            &format!("{:.2}", options.playback_speed),
            action(C::Speed),
        );
        x += 116.;
        button(
            u,
            &mut x,
            132.,
            "Соседние позы",
            C::ToggleGhosts,
            options.ghosts,
            true,
            y,
        );
        u.label(x, y + 8., "Шаг", 10., p.muted);
        u.field(
            [x + 27., y, 48., 28.],
            &options.ghost_step_frames.to_string(),
            action(C::EditGhostStep),
        );
        x += 83.;
        button(
            u,
            &mut x,
            110.,
            "Траектория",
            C::ToggleMotionPath,
            options.motion_path,
            selected_node.is_some(),
            y,
        );
        u.fitted_label(
            [x + 4., y + 9., (left + width - 14. - x).max(0.), 14.],
            "Предпросмотр · без изменения FPS",
            10.,
            p.muted,
        );
    }

    u.rect(layout.ruler, p.elevated);
    u.rect(layout.grid, p.bg);
    let selected: BTreeSet<_> = state.selection.iter().copied().map(key_identity).collect();
    for row in &layout.rows {
        let hovered = options
            .pointer
            .is_some_and(|point| ui::contains(row.rect, point));
        if row.node == selected_node && row.node.is_some() {
            u.surface(row.rect, p.selection);
        } else if hovered {
            u.surface(row.rect, p.hover);
        }
        u.rect(
            [row.rect[0], row.rect[1] + row.rect[3] - 1., row.rect[2], 1.],
            p.border,
        );
        if row.node.is_none() {
            u.quiet_button(
                [
                    row.rect[0],
                    row.rect[1],
                    layout.grid[0] - row.rect[0] - 6.,
                    ROW_HEIGHT,
                ],
                &row.label,
                action(C::SelectAll),
                false,
            );
        } else {
            u.fitted_label(
                [
                    row.rect[0] + 8.,
                    row.rect[1] + 6.,
                    layout.grid[0] - row.rect[0] - 18.,
                    16.,
                ],
                &row.label,
                11.,
                if row.locked { p.disabled } else { p.text },
            );
        }
    }

    let desired = (layout.view_span / (layout.ruler[2] / 80.).max(2.) as f64).max(1.);
    let magnitude = 10_f64.powf(desired.log10().floor());
    let step = [1., 2., 5., 10.]
        .into_iter()
        .map(|f| f * magnitude)
        .find(|&v| v >= desired)
        .unwrap_or(desired)
        .max(1.);
    let first = (layout.view_start / step).ceil() * step;
    let count = (layout.view_span / step).ceil().min(200.) as usize + 1;
    for index in 0..count {
        let frame = first + index as f64 * step;
        let x = layout.x_at_frame(frame);
        if x < layout.ruler[0] || x > layout.ruler[0] + layout.ruler[2] {
            continue;
        }
        u.rect(
            [
                x,
                layout.ruler[1] + 18.,
                1.,
                layout.grid[1] + layout.grid[3] - layout.ruler[1] - 18.,
            ],
            p.border,
        );
        let label_width = layout.ruler[0] + layout.ruler[2] - x - 4.;
        let label_width = if label_width.is_nan() {
            70.
        } else {
            label_width.clamp(0., 70.)
        };
        u.fitted_label(
            [x + 3., layout.ruler[1] + 4., label_width, 12.],
            &frame_label(frame),
            10.,
            p.muted,
        );
    }
    if options.preview_enabled {
        let start = layout
            .x_at_frame(options.preview_start)
            .clamp(layout.grid[0], layout.grid[0] + layout.grid[2]);
        let end = layout
            .x_at_frame(options.preview_end)
            .clamp(layout.grid[0], layout.grid[0] + layout.grid[2]);
        let mut shade = p.panel;
        shade[3] = 0.55;
        if start > layout.grid[0] {
            u.rect(
                [
                    layout.grid[0],
                    layout.grid[1],
                    start - layout.grid[0],
                    layout.grid[3],
                ],
                shade,
            );
        }
        if end < layout.grid[0] + layout.grid[2] {
            u.rect(
                [
                    end,
                    layout.grid[1],
                    layout.grid[0] + layout.grid[2] - end,
                    layout.grid[3],
                ],
                shade,
            );
        }
        for boundary in [options.preview_start, options.preview_end] {
            let x = layout.x_at_frame(boundary);
            if x >= layout.ruler[0] && x <= layout.ruler[0] + layout.ruler[2] {
                u.rect(
                    [
                        x,
                        layout.ruler[1] + 19.,
                        2.,
                        layout.grid[1] + layout.grid[3] - layout.ruler[1] - 19.,
                    ],
                    p.warn,
                );
            }
        }
    }
    for marker in &layout.markers {
        let count = marker
            .keys
            .iter()
            .filter(|key| selected.contains(&key_identity(**key)))
            .count();
        let hovered = options
            .pointer
            .is_some_and(|point| ui::contains(marker.rect, point));
        let locked = marker.node.is_some_and(|node| {
            layout
                .rows
                .iter()
                .any(|row| row.node == Some(node) && row.locked)
        });
        let color = if locked {
            p.disabled
        } else if count > 0 || hovered {
            p.accent
        } else {
            p.muted
        };
        let x = marker.rect[0] + marker.rect[2] * 0.5;
        let y = marker.rect[1] + marker.rect[3] * 0.5;
        let radius = if count > 0 || hovered { 6. } else { 5. };
        u.convex_polygon(
            &[
                [x, y - radius],
                [x + radius, y],
                [x, y + radius],
                [x - radius, y],
            ],
            color,
        );
        if count < marker.keys.len() && !hovered {
            let r = radius - 1.5;
            u.convex_polygon(&[[x, y - r], [x + r, y], [x, y + r], [x - r, y]], p.bg);
            if count > 0 {
                u.rect([x - 1., y - 1., 2., 2.], p.accent);
            }
        }
    }
    let cursor = layout.x_at_frame(current_time * project.fps as f64);
    if cursor >= layout.ruler[0] && cursor <= layout.ruler[0] + layout.ruler[2] {
        u.rect(
            [
                cursor,
                layout.ruler[1],
                1.5,
                layout.grid[1] + layout.grid[3] - layout.ruler[1],
            ],
            p.accent,
        );
        u.convex_polygon(
            &[
                [cursor - 5., layout.ruler[1]],
                [cursor + 5., layout.ruler[1]],
                [cursor, layout.ruler[1] + 5.],
            ],
            p.accent,
        );
    }
    if layout.markers.is_empty() {
        let label = if layout.total_rows > 0 {
            "Ключи вне масштаба — нажмите «Весь»."
        } else if active_clip.is_none() {
            "Создайте клип и запишите первую позу."
        } else if state.only_selected {
            "У выбранного объекта пока нет ключей."
        } else {
            "Запишите позу, чтобы появились ключи."
        };
        u.fitted_label(
            [
                layout.grid[0] + 10.,
                layout.grid[1] + 7.,
                layout.grid[2] - 20.,
                16.,
            ],
            label,
            11.,
            p.muted,
        );
    }
    if state.expanded {
        if layout.max_row_offset > 0 {
            let track_y = layout.grid[1] + ROW_HEIGHT;
            let track_h = layout.grid[3] - ROW_HEIGHT;
            let visible = layout.rows.len().saturating_sub(1).max(1);
            let thumb_h = (track_h * visible as f32 / layout.total_rows.max(1) as f32).max(12.);
            let ratio =
                state.row_offset.min(layout.max_row_offset) as f32 / layout.max_row_offset as f32;
            u.rect([left + width - 6., track_y, 2., track_h], p.elevated);
            u.rect(
                [
                    left + width - 6.,
                    track_y + ratio * (track_h - thumb_h),
                    2.,
                    thumb_h,
                ],
                p.muted,
            );
        }
        x = left + 8.;
        let y = top + 212.;
        let has_keys = !state.selection.is_empty();
        for (label, width, control, enabled) in [
            ("Копия", 68., C::Copy, has_keys),
            (
                "Вставить",
                90.,
                C::Paste,
                options.clipboard_available && active_clip.is_some(),
            ),
            ("Дубликат", 88., C::Duplicate, has_keys),
            ("Удалить", 82., C::Delete, has_keys),
            ("−1", 36., C::ShiftKeys(-1), has_keys),
            ("+1", 36., C::ShiftKeys(1), has_keys),
            ("Все", 46., C::SelectAll, layout.total_rows > 0),
        ] {
            button(u, &mut x, width, label, control, false, enabled, y);
        }
        u.label(x + 4., y + 9., "Каналы:", 10., p.muted);
        x += 58.;
        let interpolation = selected_interpolation(project, active_clip, &state.selection);
        for (index, label, width) in [(0, "Ступени", 80.), (1, "Линейно", 84.), (2, "Плавно", 76.)]
        {
            button(
                u,
                &mut x,
                width,
                label,
                C::Interpolation(index),
                interpolation == Some(index),
                has_keys,
                y,
            );
        }
        let suffix = format!("Ключей: {} · Shift — добавить", state.selection.len());
        u.fitted_label(
            [x + 4., y + 9., (left + width - x - 18.).max(0.), 15.],
            &suffix,
            10.,
            p.muted,
        );
    }
}

fn selected_interpolation(
    project: &Project,
    active_clip: Option<usize>,
    selection: &[KeyRef],
) -> Option<u8> {
    let clip = active_clip.and_then(|i| project.clips.get(i))?;
    let channels: BTreeSet<_> = selection
        .iter()
        .map(|key| (key.node, key.property))
        .collect();
    let mut result = None;
    for channel in clip
        .channels
        .iter()
        .filter(|c| channels.contains(&(c.node, c.property)))
    {
        let mode = match channel.interpolation {
            Interpolation::Step => 0,
            Interpolation::Linear => 1,
            Interpolation::CubicSpline
                if channel.keys.iter().all(|k| {
                    k.in_tangent
                        .as_ref()
                        .is_some_and(|v| v.iter().all(|&x| x == 0.))
                        && k.out_tangent
                            .as_ref()
                            .is_some_and(|v| v.iter().all(|&x| x == 0.))
                }) =>
            {
                2
            }
            Interpolation::CubicSpline => return None,
        };
        if result.is_some_and(|previous| previous != mode) {
            return None;
        }
        result = Some(mode);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Channel, Clip, Keyframe, Node};

    fn fixture() -> Project {
        let mut project = Project {
            fps: 24,
            duration: 4.,
            nodes: vec![Node::new(1, "Таз"), Node::new(2, "Кисть с длинным именем")],
            ..Project::default()
        };
        let mut clip = Clip::new("Проверка", 4.);
        for node in [1, 2] {
            for property in [Property::Translation, Property::Scale] {
                clip.channels.push(Channel {
                    node,
                    property,
                    interpolation: Interpolation::Linear,
                    keys: vec![
                        Keyframe::new(0.125, vec![0., 0., 0.]),
                        Keyframe::new(1.0125, vec![1., 0., 0.]),
                    ],
                });
            }
        }
        project.clips = vec![clip];
        project
    }

    #[test]
    fn view_mapping_and_zoom_preserve_fractional_source_frames() {
        let project = fixture();
        let mut state = TimelineState::default();
        state.fit(96.);
        let layout = TimelineLayout::new(&project, Some(0), Some(1), &state, 1100., 760.);
        let frame = 24.3;
        let ratio = (layout.x_at_frame(frame) - layout.grid[0]) / layout.grid[2];
        assert!((layout.frame_at(layout.x_at_frame(frame)) - frame).abs() < 1e-5);
        state.zoom(frame, 2., 96.);
        let zoomed = TimelineLayout::new(&project, Some(0), Some(1), &state, 1100., 760.);
        let next_ratio = (zoomed.x_at_frame(frame) - zoomed.grid[0]) / zoomed.grid[2];
        assert!((ratio - next_ratio).abs() < 1e-6);
        assert!(zoomed.visible_keys().iter().any(|key| key.time == 1.0125));
        state.pan(-1e6, 96.);
        assert_eq!(state.view_start, 0.);
        state.pan(1e6, 96.);
        assert_eq!(state.view_start, 48.);
    }

    #[test]
    fn huge_finite_durations_keep_the_supported_range_and_visible_keys() {
        for huge_active_clip in [false, true] {
            let mut project = fixture();
            project.fps = 240;
            if huge_active_clip {
                project.clips[0].duration = f64::MAX;
            } else {
                project.duration = f64::MAX;
            }
            project.validate().unwrap();
            let last_frame = total_frames(&project, Some(0));
            assert_eq!(last_frame, u32::MAX as f64);
            let mut state = TimelineState::default();
            state.fit(last_frame);
            assert_eq!(state.view_bounds(last_frame), (0., u32::MAX as f64));
            let layout = TimelineLayout::new(&project, Some(0), Some(1), &state, 1100., 760.);
            assert_eq!(layout.last_frame, u32::MAX as f64);
            assert_eq!(layout.visible_keys().len(), 8);
            assert!(layout
                .markers
                .iter()
                .all(|marker| marker.rect.iter().all(|value| value.is_finite())));
        }
    }

    #[test]
    fn marker_hits_take_priority_and_box_selection_deduplicates_summary() {
        let project = fixture();
        let state = TimelineState::default();
        let layout = TimelineLayout::new(&project, Some(0), Some(1), &state, 1100., 760.);
        let marker = &layout.markers[0];
        let point = [marker.rect[0] + 7., marker.rect[1] + 9.];
        assert!(
            matches!(layout.hit(point), Some(TimelineHit::Keys { keys, .. }) if keys.len() == 4)
        );
        assert_eq!(layout.visible_keys().len(), 8);
        assert_eq!(
            layout
                .keys_in_rect([
                    layout.grid[0] + layout.grid[2],
                    layout.grid[1] + layout.grid[3],
                    -layout.grid[2],
                    -layout.grid[3]
                ])
                .len(),
            8
        );
        assert!(matches!(
            layout.hit([layout.ruler[0] + 30., layout.ruler[1] + 6.]),
            Some(TimelineHit::Ruler(_))
        ));
        assert!(layout.hit([0., 0.]).is_none());
    }

    #[test]
    fn node_filter_and_compact_mode_keep_keys_reachable() {
        let project = fixture();
        let state = TimelineState {
            only_selected: true,
            expanded: false,
            ..Default::default()
        };
        let layout = TimelineLayout::new(&project, Some(0), Some(2), &state, 1100., 760.);
        assert_eq!(layout.rect[3], COMPACT_HEIGHT);
        assert_eq!(layout.rows.len(), 1);
        assert_eq!(layout.visible_keys().len(), 4);
        assert!(layout.visible_keys().iter().all(|key| key.node == 2));
    }

    #[test]
    fn controls_and_markers_fit_both_themes_and_dpi_scales() {
        let project = fixture();
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for scale in [1., 2.] {
                for expanded in [false, true] {
                    let state = TimelineState {
                        expanded,
                        ..Default::default()
                    };
                    let layout =
                        TimelineLayout::new(&project, Some(0), Some(1), &state, 1100., 760.);
                    let mut ui = Ui::with_theme(scale, theme, [0., 0.]);
                    draw_timeline(
                        &mut ui,
                        &layout,
                        &project,
                        Some(0),
                        Some(1),
                        0.,
                        &state,
                        &TimelineOptions::default(),
                        |_| Action::Timeline,
                    );
                    for hit in &ui.hits {
                        assert!(hit.rect[0] >= layout.rect[0]);
                        assert!(hit.rect[1] >= layout.rect[1]);
                        assert!(
                            hit.rect[0] + hit.rect[2] <= layout.rect[0] + layout.rect[2] + 0.1,
                            "{:?}",
                            hit.rect
                        );
                        assert!(hit.rect[1] + hit.rect[3] <= layout.rect[1] + layout.rect[3]);
                    }
                    for marker in &layout.markers {
                        assert!(marker.rect[0] >= layout.grid[0]);
                        assert!(marker.rect[0] + marker.rect[2] <= layout.grid[0] + layout.grid[2]);
                    }
                }
            }
        }
    }
}
