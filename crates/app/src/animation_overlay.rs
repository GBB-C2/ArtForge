//! Bounded authoring aids: keyed skeleton ghosts and a selected-node motion path.
//!
//! The cache contains world-space samples only. Camera/DPI/theme changes never
//! resample animation or change the project; the current pose is drawn later.
use crate::ui::{Theme, Ui};
use glam::{Mat4, Vec2, Vec3, Vec4};
use poseforge_core::{Channel, Clip, Interpolation, Node, NodeId, Project, Property};
use poseforge_render::View;
use std::cell::RefCell;
use std::collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

const MAX_SCENE_NODES: usize = 16_384;
const MAX_RELEVANT_NODES: usize = 1_024;
const MAX_GHOST_JOINTS: usize = 256;
const MAX_RELEVANT_KEYS: usize = 200_000;
const MAX_PATH_SAMPLES: usize = 65;

pub(crate) struct AnimationOverlay {
    pub(crate) ghosts: bool,
    pub(crate) path: bool,
    pub(crate) ghost_step_frames: u32,
    cache: RefCell<Cache>,
}
impl Default for AnimationOverlay {
    fn default() -> Self {
        Self {
            ghosts: false,
            path: false,
            ghost_step_frames: 5,
            cache: RefCell::new(Cache::default()),
        }
    }
}

#[derive(Default)]
struct Cache {
    signature: Option<u64>,
    source: Option<Source>,
    path: Option<PathSamples>,
    ghosts: Option<GhostSamples>,
    source_builds: usize,
    path_builds: usize,
    ghost_builds: usize,
}
struct Source {
    project: Project,
    selected: NodeId,
    selected_locked: bool,
    path_ancestors: BTreeSet<NodeId>,
    joints: Vec<(NodeId, bool)>,
    edges: Vec<(NodeId, NodeId, bool)>,
}
struct PathSamples {
    range: [u32; 2],
    points: Vec<(f64, Option<Vec3>)>,
    breaks: Vec<bool>,
}
struct GhostSamples {
    frames: [Option<u32>; 2],
    poses: [GhostPose; 2],
}
#[derive(Default)]
struct GhostPose {
    joints: Vec<(Vec3, bool)>,
    segments: Vec<(Vec3, Vec3, bool)>,
}
struct Inputs<'a> {
    signature: u64,
    selected: NodeId,
    selected_locked: bool,
    fps: u32,
    duration: f64,
    nodes: Vec<&'a Node>,
    channels: Vec<&'a Channel>,
    path_ancestors: BTreeSet<NodeId>,
    joints: Vec<(NodeId, bool)>,
}

impl AnimationOverlay {
    /// Useful on document replacement; content hashing also catches in-place
    /// key edits, Undo, ancestor transforms and visibility changes automatically.
    pub(crate) fn invalidate(&self) {
        *self.cache.borrow_mut() = Cache::default();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &self,
        ui: &mut Ui,
        project: &Project,
        clip: Option<usize>,
        selected: Option<NodeId>,
        rig_root: Option<NodeId>,
        time: f64,
        frame_range: [u32; 2],
        view: &View,
        viewport: [f32; 4],
    ) {
        if (!self.ghosts && !self.path)
            || !viewport.iter().all(|v| v.is_finite())
            || viewport[2] <= 0.
            || viewport[3] <= 0.
            || !ui.scale.is_finite()
            || ui.scale <= 0.
            || !time.is_finite()
        {
            return;
        }
        let matrix = view.view_projection();
        if !matrix.is_finite() {
            return;
        }
        let inputs = match prepare(project, clip, selected, rig_root, self.ghosts) {
            Ok(inputs) => inputs,
            Err(message) => {
                legend(ui, viewport, message);
                return;
            }
        };
        let max_frame = (inputs.duration * inputs.fps as f64)
            .floor()
            .clamp(0., u32::MAX as f64) as u32;
        let range = [
            frame_range[0].min(frame_range[1]).min(max_frame),
            frame_range[0].max(frame_range[1]).min(max_frame),
        ];
        let frame = (time.max(0.) * inputs.fps as f64)
            .round()
            .clamp(0., max_frame as f64) as u32;
        let step = self.ghost_step_frames.clamp(1, 120);
        // Ghosts use the surrounding clip frames, even when the visible path
        // range is zoomed. Missing samples at either clip endpoint stay absent.
        let ghost_frames = [
            frame.checked_sub(step),
            frame.checked_add(step).filter(|f| *f <= max_frame),
        ];
        let mut cache = self.cache.borrow_mut();
        if cache.signature != Some(inputs.signature) {
            cache.signature = Some(inputs.signature);
            cache.source = Some(inputs.into_source());
            cache.path = None;
            cache.ghosts = None;
            cache.source_builds += 1;
        }
        if self.path && cache.path.as_ref().is_none_or(|p| p.range != range) {
            cache.path = cache.source.as_ref().map(|s| sample_path(s, range));
            cache.path_builds += 1;
        }
        if self.ghosts
            && cache
                .ghosts
                .as_ref()
                .is_none_or(|g| g.frames != ghost_frames)
        {
            cache.ghosts = cache.source.as_ref().map(|s| GhostSamples {
                frames: ghost_frames,
                poses: ghost_frames.map(|f| f.map(|f| sample_ghost(s, f)).unwrap_or_default()),
            });
            cache.ghost_builds += 1;
        }
        let start = ui.rects.len();
        let (past, future) = colors(ui.theme);
        if self.path {
            if let (Some(path), Some(source)) = (&cache.path, &cache.source) {
                for (index, pair) in path.points.windows(2).enumerate() {
                    if path.breaks[index] {
                        continue;
                    }
                    if let (Some(a), Some(b)) = (pair[0].1, pair[1].1) {
                        let tint = if pair[1].0 <= frame as f64 {
                            past
                        } else {
                            future
                        };
                        draw_segment(
                            ui,
                            matrix,
                            viewport,
                            a,
                            b,
                            1.6,
                            tint,
                            source.selected_locked,
                        );
                    }
                }
                // Sample dots make timing and coarse bounded sampling visible.
                for &(sample_frame, point) in &path.points {
                    let Some(point) = point else { continue };
                    let tint = if sample_frame <= frame as f64 {
                        past
                    } else {
                        future
                    };
                    draw_point(
                        ui,
                        matrix,
                        viewport,
                        point,
                        1.7,
                        tint,
                        source.selected_locked,
                    );
                }
            }
        }
        if self.ghosts {
            if let Some(ghosts) = &cache.ghosts {
                for (pose, mut tint) in ghosts.poses.iter().zip([past, future]) {
                    tint[3] *= 0.55;
                    for &(a, b, locked) in &pose.segments {
                        draw_segment(ui, matrix, viewport, a, b, 1.25, tint, locked);
                    }
                    for &(point, locked) in &pose.joints {
                        draw_point(ui, matrix, viewport, point, 2., tint, locked);
                    }
                }
            }
        }
        clip_rectangles(ui, start, viewport);
        let label = match (self.ghosts, self.path) {
            (true, true) => format!("Позы −{step} / +{step} к. · траектория"),
            (true, false) => format!("Позы −{step} / +{step} кадров"),
            (false, true) => format!(
                "Траектория · {} отсчётов",
                cache.path.as_ref().map_or(0, |p| p.points.len())
            ),
            _ => String::new(),
        };
        drop(cache);
        legend(ui, viewport, &format!("{label} · до / после"));
    }
}

fn prepare<'a>(
    project: &'a Project,
    clip: Option<usize>,
    selected: Option<NodeId>,
    rig_root: Option<NodeId>,
    ghosts: bool,
) -> Result<Inputs<'a>, &'static str> {
    let selected = selected.ok_or("Выберите объект для подсказок движения")?;
    let clip_index = clip.ok_or("Для подсказок движения нужен активный клип")?;
    let clip = project.clips.get(clip_index).ok_or("Клип недоступен")?;
    if project.fps == 0 || !clip.duration.is_finite() || clip.duration < 0. {
        return Err("Некорректные параметры клипа");
    }
    if project.nodes.len() > MAX_SCENE_NODES || clip.channels.len() > MAX_SCENE_NODES * 4 {
        return Err("Подсказки: превышен бюджет сцены");
    }
    let nodes: BTreeMap<_, _> = project.nodes.iter().map(|n| (n.id, n)).collect();
    let ancestor_chain = ancestors(selected, &nodes)?;
    let selected_locked = ancestor_chain.iter().any(|id| nodes[id].locked);
    if ancestor_chain.iter().any(|id| !nodes[id].visible) {
        return Err("Выбранная ветвь скрыта");
    }
    let scope = rig_root
        .filter(|id| ancestor_chain.contains(id))
        .unwrap_or_else(|| {
            ancestor_chain
                .iter()
                .find(|id| !nodes[id].is_joint)
                .copied()
                .unwrap_or_else(|| *ancestor_chain.last().unwrap())
        });
    let path_ancestors: BTreeSet<_> = ancestor_chain.iter().copied().collect();
    let mut required = path_ancestors.clone();
    let mut joints = Vec::new();
    if ghosts {
        let scope_ancestors = ancestors(scope, &nodes)?;
        let mut children: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        for node in &project.nodes {
            if let Some(parent) = node.parent {
                children.entry(parent).or_default().push(node.id);
            }
        }
        let mut pending = vec![(scope, scope_ancestors.iter().any(|id| nodes[id].locked))];
        let mut visited = BTreeSet::new();
        while let Some((id, parent_locked)) = pending.pop() {
            if !visited.insert(id) {
                return Err("Цикл в иерархии объекта");
            }
            if visited.len() > MAX_RELEVANT_NODES {
                return Err("Подсказки: слишком большая ветвь");
            }
            let node = nodes.get(&id).ok_or("Узел иерархии недоступен")?;
            if !node.visible {
                continue;
            }
            let locked = parent_locked || node.locked;
            if node.is_joint {
                joints.push((id, locked));
                if joints.len() > MAX_GHOST_JOINTS {
                    return Err("Подсказки: предел 256 суставов");
                }
                for ancestor in ancestors(id, &nodes)? {
                    required.insert(ancestor);
                }
            }
            if let Some(children) = children.get(&id) {
                pending.extend(children.iter().map(|id| (*id, locked)));
            }
        }
    }
    if required.len() > MAX_RELEVANT_NODES {
        return Err("Подсказки: слишком длинная иерархия");
    }
    joints.sort_by_key(|joint| joint.0);
    let selected_nodes: Vec<_> = required
        .iter()
        .filter_map(|id| nodes.get(id).copied())
        .collect();
    let mut channels = Vec::new();
    let mut key_count = 0usize;
    for channel in &clip.channels {
        if !required.contains(&channel.node) || channel.property == Property::Weights {
            continue;
        }
        key_count = key_count.saturating_add(channel.keys.len());
        if key_count > MAX_RELEVANT_KEYS {
            return Err("Подсказки: предел 200 000 ключей");
        }
        let components = if channel.property == Property::Rotation {
            4
        } else {
            3
        };
        let mut previous = -1.;
        if channel.keys.is_empty()
            || channel.keys.iter().any(|key| {
                let invalid = !key.time.is_finite()
                    || key.time <= previous
                    || key.time < 0.
                    || key.time > clip.duration + 1e-8
                    || key.value.len() != components
                    || key.value.iter().any(|v| !v.is_finite())
                    || [&key.in_tangent, &key.out_tangent].iter().any(|tangent| {
                        tangent.as_ref().map_or(
                            channel.interpolation == Interpolation::CubicSpline,
                            |values| {
                                values.len() != components || values.iter().any(|v| !v.is_finite())
                            },
                        )
                    });
                previous = key.time;
                invalid
            })
        {
            return Err("Некорректный канал трансформации");
        }
        channels.push(channel);
    }
    let mut hash = DefaultHasher::new();
    selected.hash(&mut hash);
    scope.hash(&mut hash);
    ghosts.hash(&mut hash);
    project.fps.hash(&mut hash);
    clip_index.hash(&mut hash);
    clip.duration.to_bits().hash(&mut hash);
    for node in &selected_nodes {
        node.id.hash(&mut hash);
        node.parent.hash(&mut hash);
        node.visible.hash(&mut hash);
        node.locked.hash(&mut hash);
        node.is_joint.hash(&mut hash);
        for value in node
            .transform
            .translation
            .to_array()
            .into_iter()
            .chain(node.transform.rotation.to_array())
            .chain(node.transform.scale.to_array())
        {
            value.to_bits().hash(&mut hash);
        }
    }
    joints.hash(&mut hash);
    for channel in &channels {
        channel.node.hash(&mut hash);
        (channel.property as u8).hash(&mut hash);
        (channel.interpolation as u8).hash(&mut hash);
        channel.keys.len().hash(&mut hash);
        for key in &channel.keys {
            key.time.to_bits().hash(&mut hash);
            hash_values(&mut hash, &key.value);
            for tangent in [&key.in_tangent, &key.out_tangent] {
                tangent.is_some().hash(&mut hash);
                if let Some(values) = tangent {
                    hash_values(&mut hash, values);
                }
            }
        }
    }
    Ok(Inputs {
        signature: hash.finish(),
        selected,
        selected_locked,
        fps: project.fps,
        duration: clip.duration,
        nodes: selected_nodes,
        channels,
        path_ancestors,
        joints,
    })
}

fn ancestors(id: NodeId, nodes: &BTreeMap<NodeId, &Node>) -> Result<Vec<NodeId>, &'static str> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut cursor = Some(id);
    while let Some(id) = cursor {
        if result.len() >= MAX_RELEVANT_NODES || !seen.insert(id) {
            return Err("Подсказки: цикл или слишком длинная иерархия");
        }
        let node = nodes.get(&id).ok_or("Узел иерархии недоступен")?;
        result.push(id);
        cursor = node.parent;
    }
    Ok(result)
}
fn hash_values(hash: &mut DefaultHasher, values: &[f32]) {
    values.len().hash(hash);
    for value in values {
        value.to_bits().hash(hash);
    }
}
impl Inputs<'_> {
    fn into_source(self) -> Source {
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .map(|source| {
                let mut node = Node::new(source.id, "");
                node.parent = source.parent;
                node.transform = source.transform;
                node.is_joint = source.is_joint;
                node.visible = source.visible;
                node.locked = source.locked;
                node
            })
            .collect();
        let by_id: BTreeMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();
        let joint_ids: BTreeSet<_> = self.joints.iter().map(|j| j.0).collect();
        let mut edges = Vec::new();
        for &(id, locked) in &self.joints {
            let mut parent = by_id[&id].parent;
            while let Some(parent_id) = parent {
                let Some(node) = by_id.get(&parent_id) else {
                    break;
                };
                if node.is_joint {
                    if joint_ids.contains(&parent_id) {
                        edges.push((parent_id, id, locked || node.locked));
                    }
                    break;
                }
                parent = node.parent;
            }
        }
        let project = Project {
            nodes,
            fps: self.fps,
            duration: self.duration,
            clips: vec![Clip {
                name: String::new(),
                duration: self.duration,
                channels: self.channels.into_iter().cloned().collect(),
            }],
            ..Project::default()
        };
        Source {
            project,
            selected: self.selected,
            selected_locked: self.selected_locked,
            path_ancestors: self.path_ancestors,
            joints: self.joints,
            edges,
        }
    }
}
fn sample_path(source: &Source, range: [u32; 2]) -> PathSamples {
    let count = ((range[1] as u64 - range[0] as u64 + 1).min(MAX_PATH_SAMPLES as u64)) as usize;
    let points: Vec<_> = (0..count)
        .map(|i| {
            let frame = range[0] as f64
                + (range[1] - range[0]) as f64 * i as f64 / (count.saturating_sub(1).max(1)) as f64;
            let point = source
                .project
                .evaluated(Some(0), frame / source.project.fps as f64)
                .ok()
                .and_then(|e| e.global.get(&source.selected).map(|m| m.w_axis.truncate()))
                .filter(|p| p.is_finite());
            (frame, point)
        })
        .collect();
    let breaks = points
        .windows(2)
        .map(|pair| {
            let start = pair[0].0 / source.project.fps as f64;
            let end = pair[1].0 / source.project.fps as f64;
            source.project.clips[0].channels.iter().any(|channel| {
                channel.interpolation == Interpolation::Step
                    && source.path_ancestors.contains(&channel.node)
                    && channel
                        .keys
                        .get(channel.keys.partition_point(|k| k.time <= start))
                        .is_some_and(|key| key.time <= end)
            })
        })
        .collect();
    PathSamples {
        range,
        points,
        breaks,
    }
}
fn sample_ghost(source: &Source, frame: u32) -> GhostPose {
    let Ok(evaluated) = source
        .project
        .evaluated(Some(0), frame as f64 / source.project.fps as f64)
    else {
        return GhostPose::default();
    };
    let position = |id: &NodeId| {
        evaluated
            .global
            .get(id)
            .map(|m| m.w_axis.truncate())
            .filter(|p| p.is_finite())
    };
    GhostPose {
        joints: source
            .joints
            .iter()
            .filter_map(|(id, locked)| position(id).map(|p| (p, *locked)))
            .collect(),
        segments: source
            .edges
            .iter()
            .filter_map(|(a, b, locked)| Some((position(a)?, position(b)?, *locked)))
            .collect(),
    }
}

fn colors(theme: Theme) -> ([f32; 4], [f32; 4]) {
    match theme {
        Theme::Dark => ([1., 0.70, 0.30, 0.78], [0.28, 0.84, 1., 0.80]),
        Theme::Light => ([0.64, 0.32, 0.03, 0.82], [0.03, 0.40, 0.64, 0.86]),
    }
}
fn legend(ui: &mut Ui, viewport: [f32; 4], text: &str) {
    if viewport[2] < 60. || viewport[3] < 40. {
        return;
    }
    let rect = [
        viewport[0] + 8.,
        viewport[1] + viewport[3] - 27.,
        (viewport[2] - 16.).min(360.),
        21.,
    ];
    let mut background = ui.palette().panel;
    background[3] = 0.86;
    ui.surface(rect, background);
    ui.fitted_label(
        [rect[0] + 6., rect[1] + 4., rect[2] - 12., 14.],
        text,
        10.,
        ui.palette().muted,
    );
}
fn screen(viewport: [f32; 4], clip: Vec4) -> Option<Vec2> {
    if !clip.is_finite() || clip.w <= 1e-7 {
        return None;
    }
    let p = clip.truncate() / clip.w;
    let result = Vec2::new(
        viewport[0] + (p.x + 1.) * viewport[2] * 0.5,
        viewport[1] + (1. - p.y) * viewport[3] * 0.5,
    );
    result.is_finite().then_some(result)
}
fn project_segment(matrix: Mat4, viewport: [f32; 4], a: Vec3, b: Vec3) -> Option<(Vec2, Vec2)> {
    let a = matrix * a.extend(1.);
    let b = matrix * b.extend(1.);
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let mut start = 0f32;
    let mut end = 1f32;
    for (fa, fb) in [(a.z, b.z), (a.w - a.z, b.w - b.z), (a.w - 1e-6, b.w - 1e-6)] {
        if fa < 0. && fb < 0. {
            return None;
        }
        if (fa < 0.) != (fb < 0.) {
            let t = fa / (fa - fb);
            if fa < 0. {
                start = start.max(t);
            } else {
                end = end.min(t);
            }
        }
    }
    if start > end {
        return None;
    }
    clip_segment(
        screen(viewport, a.lerp(b, start))?,
        screen(viewport, a.lerp(b, end))?,
        viewport,
    )
}
fn clip_segment(a: Vec2, b: Vec2, r: [f32; 4]) -> Option<(Vec2, Vec2)> {
    let delta = b - a;
    let mut start = 0f32;
    let mut end = 1f32;
    for (p, q) in [
        (-delta.x, a.x - r[0]),
        (delta.x, r[0] + r[2] - a.x),
        (-delta.y, a.y - r[1]),
        (delta.y, r[1] + r[3] - a.y),
    ] {
        if p.abs() < 1e-8 {
            if q < 0. {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0. {
            start = start.max(t);
        } else {
            end = end.min(t);
        }
    }
    (start <= end).then(|| (a + delta * start, a + delta * end))
}
#[allow(clippy::too_many_arguments)]
fn draw_segment(
    ui: &mut Ui,
    matrix: Mat4,
    viewport: [f32; 4],
    a: Vec3,
    b: Vec3,
    width: f32,
    mut tint: [f32; 4],
    locked: bool,
) {
    let Some((a, b)) = project_segment(matrix, viewport, a, b) else {
        return;
    };
    if locked {
        tint[3] *= 0.4;
    }
    ui.smooth_segment(a.to_array(), b.to_array(), width, tint);
}
#[allow(clippy::too_many_arguments)]
fn draw_point(
    ui: &mut Ui,
    matrix: Mat4,
    viewport: [f32; 4],
    point: Vec3,
    radius: f32,
    mut tint: [f32; 4],
    locked: bool,
) {
    let clip = matrix * point.extend(1.);
    if clip.z < 0. || clip.z > clip.w {
        return;
    }
    let Some(center) = screen(viewport, clip) else {
        return;
    };
    if center.x < viewport[0] - radius
        || center.x > viewport[0] + viewport[2] + radius
        || center.y < viewport[1] - radius
        || center.y > viewport[1] + viewport[3] + radius
    {
        return;
    }
    if locked {
        tint[3] *= 0.4;
    }
    let points: Vec<_> = (0..12)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 12.;
            (center + Vec2::new(angle.cos(), angle.sin()) * radius).to_array()
        })
        .collect();
    ui.convex_polygon(&points, tint);
}
fn clip_rectangles(ui: &mut Ui, start: usize, viewport: [f32; 4]) {
    let r = viewport.map(|v| v * ui.scale);
    let mut index = 0;
    ui.rects.retain_mut(|primitive| {
        index += 1;
        if index <= start {
            return true;
        }
        let [x, y, w, h] = primitive.rect;
        let left = x.max(r[0]);
        let top = y.max(r[1]);
        let right = (x + w).min(r[0] + r[2]);
        let bottom = (y + h).min(r[1] + r[3]);
        if right <= left || bottom <= top {
            return false;
        }
        primitive.rect = [left, top, right - left, bottom - top];
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Keyframe, Quat};

    fn fixture() -> Project {
        let mut root = Node::new(1, "Character");
        root.transform.translation = Vec3::new(-0.5, 0., 0.);
        let mut pelvis = Node::new(2, "Pelvis");
        pelvis.parent = Some(1);
        pelvis.is_joint = true;
        let mut helper = Node::new(3, "Helper");
        helper.parent = Some(2);
        helper.transform.translation = Vec3::Y;
        let mut tip = Node::new(4, "Tip");
        tip.parent = Some(3);
        tip.is_joint = true;
        tip.transform.translation = Vec3::Y;
        Project {
            fps: 24,
            duration: 2.,
            nodes: vec![root, pelvis, helper, tip],
            clips: vec![Clip {
                name: "Move".into(),
                duration: 2.,
                channels: vec![Channel {
                    node: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    keys: vec![
                        Keyframe::new(0., vec![-0.5, 0., 0.]),
                        Keyframe::new(2., vec![0.5, 0., 0.]),
                    ],
                }],
            }],
            ..Project::default()
        }
    }
    fn view() -> View {
        View {
            eye: Vec3::new(3., 2.8, 6.),
            target: Vec3::Y,
            viewport: [20., 30., 480., 340.],
            ..View::default()
        }
    }
    fn enabled() -> AnimationOverlay {
        AnimationOverlay {
            ghosts: true,
            path: true,
            ..Default::default()
        }
    }
    fn draw(overlay: &AnimationOverlay, project: &Project, time: f64) -> Ui {
        let mut ui = Ui::new(1.);
        overlay.draw(
            &mut ui,
            project,
            Some(0),
            Some(4),
            Some(1),
            time,
            [0, 48],
            &view(),
            [20., 30., 480., 340.],
        );
        ui
    }

    #[test]
    fn parent_motion_and_helper_nodes_are_sampled_without_project_mutation() {
        let mut project = fixture();
        project.clips[0].channels.push(Channel {
            node: 2,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            keys: vec![
                Keyframe::new(0., Quat::IDENTITY.to_array().to_vec()),
                Keyframe::new(
                    2.,
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
                        .to_array()
                        .to_vec(),
                ),
            ],
        });
        let before = project.clone();
        let overlay = enabled();
        draw(&overlay, &project, 1.);
        let cache = overlay.cache.borrow();
        let path = cache.path.as_ref().unwrap();
        assert_eq!(path.points.len(), 49);
        assert!(path.points[0]
            .1
            .unwrap()
            .abs_diff_eq(Vec3::new(-0.5, 2., 0.), 1e-5));
        assert!(path.points[48]
            .1
            .unwrap()
            .abs_diff_eq(Vec3::new(-1.5, 0., 0.), 1e-5));
        assert_eq!(cache.source.as_ref().unwrap().edges, vec![(2, 4, false)]);
        assert_eq!(cache.ghosts.as_ref().unwrap().poses[0].segments.len(), 1);
        assert_eq!(project, before);
    }

    #[test]
    fn in_place_key_edits_and_undo_invalidate_world_samples() {
        let mut project = fixture();
        let before = project.clone();
        let overlay = enabled();
        draw(&overlay, &project, 1.);
        let original = overlay
            .cache
            .borrow()
            .path
            .as_ref()
            .unwrap()
            .points
            .last()
            .unwrap()
            .1;
        draw(&overlay, &project, 1.);
        assert_eq!(overlay.cache.borrow().source_builds, 1);
        project.clips[0].channels[0].keys[1].value[0] = 1.5;
        draw(&overlay, &project, 1.);
        assert_eq!(overlay.cache.borrow().source_builds, 2);
        assert_ne!(
            overlay
                .cache
                .borrow()
                .path
                .as_ref()
                .unwrap()
                .points
                .last()
                .unwrap()
                .1,
            original
        );
        project = before;
        draw(&overlay, &project, 1.);
        assert_eq!(overlay.cache.borrow().source_builds, 3);
        assert_eq!(
            overlay
                .cache
                .borrow()
                .path
                .as_ref()
                .unwrap()
                .points
                .last()
                .unwrap()
                .1,
            original
        );
    }

    #[test]
    fn camera_time_fps_clip_and_range_have_separate_cache_lifetimes() {
        let mut project = fixture();
        let overlay = enabled();
        draw(&overlay, &project, 1.);
        draw(&overlay, &project, 1.2);
        assert_eq!(overlay.cache.borrow().path_builds, 1);
        assert_eq!(overlay.cache.borrow().ghost_builds, 2);
        let mut changed_view = view();
        changed_view.eye = Vec3::new(-4., 3., 5.);
        let mut ui = Ui::new(2.);
        overlay.draw(
            &mut ui,
            &project,
            Some(0),
            Some(4),
            Some(1),
            1.2,
            [0, 48],
            &changed_view,
            [20., 30., 480., 340.],
        );
        assert_eq!(overlay.cache.borrow().path_builds, 1);
        assert_eq!(overlay.cache.borrow().ghost_builds, 2);
        overlay.draw(
            &mut ui,
            &project,
            Some(0),
            Some(4),
            Some(1),
            1.2,
            [12, 36],
            &changed_view,
            [20., 30., 480., 340.],
        );
        assert_eq!(overlay.cache.borrow().path_builds, 2);
        assert_eq!(overlay.cache.borrow().ghost_builds, 2);
        project.fps = 30;
        draw(&overlay, &project, 1.2);
        assert_eq!(overlay.cache.borrow().source_builds, 2);
        assert_eq!(overlay.cache.borrow().path_builds, 3);
        project.clips.push(project.clips[0].clone());
        overlay.draw(
            &mut ui,
            &project,
            Some(1),
            Some(4),
            Some(1),
            1.2,
            [0, 48],
            &view(),
            [20., 30., 480., 340.],
        );
        assert_eq!(overlay.cache.borrow().source_builds, 3);
        overlay.invalidate();
        assert!(overlay.cache.borrow().source.is_none());
        assert!(overlay.ghosts && overlay.path);
    }

    #[test]
    fn hidden_branches_and_inherited_locks_match_editor_semantics() {
        let mut project = fixture();
        project.nodes[2].visible = false;
        let inputs = prepare(&project, Some(0), Some(1), Some(1), true).unwrap();
        assert_eq!(inputs.joints, vec![(2, false)]);
        assert!(prepare(&project, Some(0), Some(4), Some(1), true).is_err());
        project.nodes[2].visible = true;
        project.nodes[0].locked = true;
        let source = prepare(&project, Some(0), Some(4), Some(1), true)
            .unwrap()
            .into_source();
        assert!(source.selected_locked);
        assert!(source.joints.iter().all(|joint| joint.1));
        assert!(source.edges.iter().all(|edge| edge.2));
    }

    #[test]
    fn ghosts_do_not_repeat_current_pose_at_clip_endpoints() {
        let project = fixture();
        let overlay = enabled();
        draw(&overlay, &project, 0.);
        assert_eq!(
            overlay.cache.borrow().ghosts.as_ref().unwrap().frames,
            [None, Some(5)]
        );
        assert!(overlay.cache.borrow().ghosts.as_ref().unwrap().poses[0]
            .joints
            .is_empty());
        draw(&overlay, &project, 2.);
        assert_eq!(
            overlay.cache.borrow().ghosts.as_ref().unwrap().frames,
            [Some(43), None]
        );
        assert!(overlay.cache.borrow().ghosts.as_ref().unwrap().poses[1]
            .joints
            .is_empty());
    }

    #[test]
    fn step_motion_never_draws_a_fictitious_interpolated_bridge() {
        let mut project = fixture();
        project.clips[0].channels[0].interpolation = Interpolation::Step;
        project.clips[0].channels[0].keys[1].time = 1.;
        let source = prepare(&project, Some(0), Some(4), Some(1), true)
            .unwrap()
            .into_source();
        let path = sample_path(&source, [0, 48]);
        assert_eq!(path.breaks.iter().filter(|jump| **jump).count(), 1);
        assert!(path.breaks[23]);
        assert!(!path.breaks[22]);
        assert!(!path.breaks[24]);
    }

    #[test]
    fn overlay_geometry_stays_finite_clipped_and_noninteractive_in_both_themes_and_dpi() {
        let mut project = fixture();
        project.clips[0].channels[0].keys[0].value[0] = -100.;
        project.clips[0].channels[0].keys[1].value[0] = 100.;
        let overlay = enabled();
        let viewport = [20., 30., 480., 340.];
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 2.] {
                for orthographic in [None, Some(3.)] {
                    let mut ui = Ui::with_theme(scale, theme, [-1., -1.]);
                    let mut v = view();
                    v.orthographic = orthographic;
                    overlay.draw(
                        &mut ui,
                        &project,
                        Some(0),
                        Some(4),
                        Some(1),
                        1.,
                        [0, 48],
                        &v,
                        viewport,
                    );
                    assert!(!ui.rects.is_empty());
                    assert!(ui.hits.is_empty());
                    for primitive in &ui.rects {
                        assert!(primitive
                            .rect
                            .iter()
                            .chain(&primitive.color)
                            .all(|v| v.is_finite()));
                        assert!(primitive.rect[0] >= viewport[0] * scale - 1e-4);
                        assert!(primitive.rect[1] >= viewport[1] * scale - 1e-4);
                        assert!(
                            primitive.rect[0] + primitive.rect[2]
                                <= (viewport[0] + viewport[2]) * scale + 1e-4
                        );
                        assert!(
                            primitive.rect[1] + primitive.rect[3]
                                <= (viewport[1] + viewport[3]) * scale + 1e-4
                        );
                    }
                }
            }
        }
        let v = View {
            eye: Vec3::new(0., 0., 5.),
            target: Vec3::ZERO,
            ..view()
        };
        assert!(project_segment(
            v.view_projection(),
            viewport,
            Vec3::new(0., 0., 4.999),
            Vec3::ZERO
        )
        .is_some());
        assert!(project_segment(
            v.view_projection(),
            viewport,
            Vec3::new(0., 0., 6.),
            Vec3::new(0., 1., 7.)
        )
        .is_none());
    }

    #[test]
    fn sampling_and_input_budgets_are_explicit_and_bounded() {
        let mut project = fixture();
        project.clips[0].duration = 10_000.;
        let overlay = enabled();
        let mut ui = Ui::new(1.);
        overlay.draw(
            &mut ui,
            &project,
            Some(0),
            Some(4),
            Some(1),
            1.,
            [0, u32::MAX],
            &view(),
            [20., 30., 480., 340.],
        );
        assert_eq!(
            overlay.cache.borrow().path.as_ref().unwrap().points.len(),
            MAX_PATH_SAMPLES
        );
        for id in 10..(10 + MAX_GHOST_JOINTS as u64) {
            let mut joint = Node::new(id, "Joint");
            joint.parent = Some(1);
            joint.is_joint = true;
            project.nodes.push(joint);
        }
        assert_eq!(
            prepare(&project, Some(0), Some(1), Some(1), true).err(),
            Some("Подсказки: предел 256 суставов")
        );
        // A path can still be enabled alone for the same large character.
        assert!(prepare(&project, Some(0), Some(4), Some(1), false).is_ok());
    }

    #[test]
    fn malformed_transform_keys_are_rejected_before_sampling() {
        let mut project = fixture();
        project.clips[0].channels[0].keys[1].value = vec![0.; 2];
        assert!(prepare(&project, Some(0), Some(4), Some(1), true).is_err());
        project.clips[0].channels[0].keys[1].value = vec![0., f32::NAN, 0.];
        assert!(prepare(&project, Some(0), Some(4), Some(1), true).is_err());
        project.clips[0].channels[0].keys[1].value = vec![0.; 3];
        project.clips[0].channels[0].keys[1].time = 0.;
        assert!(prepare(&project, Some(0), Some(4), Some(1), true).is_err());
    }
}
