use crate::{
    animation::{evaluate, evaluate_base}, two_bone_transforms, Clip, CoreError, EvaluatedScene, Interpolation,
    KeyClipboard, KeyInterpolationPreset, KeyRef, NodeId, Project, Property, SavedPose, Transform,
};
use glam::{Quat, Vec3};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Scene,
    Pose,
    Animation,
    Render,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedResolution {
    Commit,
    Discard,
}
#[derive(Clone)]
struct Snapshot {
    project: Project,
    staged: BTreeMap<NodeId, Transform>,
    staged_weights: BTreeMap<NodeId, Vec<f32>>,
    time: f64,
    active_clip: Option<usize>,
}
struct HistoryEntry {
    label: String,
    snapshot: Snapshot,
}

/// Validated off-thread edit. Private fields prevent callers from changing the
/// candidate after validation or committing it against a different document.
pub struct PreparedProjectEdit {
    before: Project,
    after: Project,
    rest_pose: BTreeMap<NodeId, Transform>,
}
impl PreparedProjectEdit {
    pub fn new(before: Project, mut after: Project) -> Result<Self, CoreError> {
        for node in &mut after.nodes {
            if node.rest_transform.is_none() {
                node.rest_transform = Some(node.transform);
            }
        }
        after.validate()?;
        let rest_pose = after
            .nodes
            .iter()
            .map(|node| (node.id, node.rest_transform.unwrap_or(node.transform)))
            .collect();
        Ok(Self {
            before,
            after,
            rest_pose,
        })
    }
}

pub struct Editor {
    pub project: Project,
    pub selected: Option<NodeId>,
    pub time: f64,
    pub playing: bool,
    pub auto_key: bool,
    pub active_clip: Option<usize>,
    pub mode: EditorMode,
    pub render_animated: bool,
    pub status: String,
    pub modified: bool,
    staged: BTreeMap<NodeId, Transform>,
    staged_weights: BTreeMap<NodeId, Vec<f32>>,
    rest_pose: BTreeMap<NodeId, Transform>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    gesture: Option<(String, Snapshot)>,
    clipboard: Option<Transform>,
}
impl Editor {
    pub fn new(mut project: Project) -> Self {
        for node in &mut project.nodes {
            if node.rest_transform.is_none() {
                node.rest_transform = Some(node.transform);
            }
        }
        let rest_pose = project
            .nodes
            .iter()
            .map(|n| (n.id, n.rest_transform.unwrap_or(n.transform)))
            .collect();
        Self {
            active_clip: if project.clips.is_empty() {
                None
            } else {
                Some(0)
            },
            project,
            selected: None,
            time: 0.,
            playing: false,
            auto_key: false,
            mode: EditorMode::Pose,
            render_animated: false,
            status: "Готово".into(),
            modified: false,
            staged: BTreeMap::new(),
            staged_weights: BTreeMap::new(),
            rest_pose,
            undo: vec![],
            redo: vec![],
            gesture: None,
            clipboard: None,
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            project: self.project.clone(),
            staged: self.staged.clone(),
            staged_weights: self.staged_weights.clone(),
            time: self.time,
            active_clip: self.active_clip,
        }
    }
    fn restore(&mut self, s: Snapshot) {
        self.project = s.project;
        self.staged = s.staged;
        self.staged_weights = s.staged_weights;
        self.time = s.time;
        self.active_clip = s.active_clip;
        self.rest_pose = self
            .project
            .nodes
            .iter()
            .map(|n| (n.id, n.rest_transform.unwrap_or(n.transform)))
            .collect();
        if self
            .selected
            .is_some_and(|id| self.project.node(id).is_none())
        {
            self.selected = None;
        }
    }
    fn record(&mut self, label: &str, before: Snapshot) {
        if self.gesture.is_some() {
            return;
        }
        if before.project == self.project && before.staged == self.staged && before.staged_weights == self.staged_weights {
            return;
        }
        self.undo.push(HistoryEntry {
            label: label.into(),
            snapshot: before,
        });
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.modified = true;
        self.status = label.into();
    }
    pub fn transact(
        &mut self,
        label: &str,
        operation: impl FnOnce(&mut Project),
    ) -> Result<(), CoreError> {
        let before = self.snapshot();
        operation(&mut self.project);
        // Deleting an obstacle or removing a curve's geometry also detaches
        // its collision references, within the same reversible edit.
        let live_meshes: BTreeSet<_> = self.project.nodes.iter().filter(|n| n.mesh.is_some()).map(|n| n.id).collect();
        let lost_meshes: BTreeSet<_> = before.project.nodes.iter().filter(|node| node.mesh.is_some()
            && !live_meshes.contains(&node.id)).map(|n| n.id).collect();
        if !lost_meshes.is_empty() {
            for node in &mut self.project.nodes { node.collision_targets.remove(&lost_meshes); }
        }
        let detached_fx:Vec<_>=self.project.nodes.iter().filter(|n|n.fx.and_then(|f|f.source).is_some_and(|source|
            before.project.node(source).is_some_and(|s|s.mesh.is_some() || s.curve.is_some())
            && self.project.node(source).is_none_or(|s|s.mesh.is_none() && s.curve.is_none()))).map(|n|n.id).collect();
        if !detached_fx.is_empty() {
            let sampled=match evaluate_base(&before.project,self.evaluated_clip(),self.time,&before.staged) {
                Ok(scene)=>scene,Err(error)=>{self.restore(before);return Err(error);}
            };
            for id in detached_fx {
                let Some(local)=sampled.local.get(&id).copied() else {self.restore(before);return Err(CoreError::MissingNode(id));};
                if let Err(error)=self.project.detach_fx_source(id,&sampled.global,local) {self.restore(before);return Err(error);}
                self.project.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=false;
            }
        }
        let removed_curves:BTreeSet<_>=before.project.nodes.iter().filter(|n|n.curve.is_some() && self.project.node(n.id).is_none_or(|n|n.curve.is_none())).map(|n|n.id).collect();
        for node in &mut self.project.nodes {for m in &mut node.modifiers {
            if let crate::ModifierSettings::CurveArray {curve:Some(target),..}=m.settings {
                if removed_curves.contains(&target) {if let crate::ModifierSettings::CurveArray {curve,..}=&mut m.settings {*curve=None;}m.enabled=false;}
            }
        }}
        let lost_curve = |binding: Option<crate::CurveBinding>| binding.is_some_and(|b|
            before.project.node(b.curve).is_some() && (self.project.node(b.curve).is_none()
                || before.project.node(b.curve).is_some_and(|n| n.curve.is_some())
                    && self.project.node(b.curve).is_some_and(|n| n.curve.is_none())));
        let detached: Vec<_> = self.project.nodes.iter().filter(|n| lost_curve(n.curve_binding)).map(|n| n.id).collect();
        let detached_lights: Vec<_> = self.project.lights.iter().enumerate()
            .filter(|(_, l)| lost_curve(l.curve_binding)).map(|(i, _)| i).collect();
        if !detached.is_empty() || !detached_lights.is_empty() {
            let sampled = match evaluate_base(&before.project, self.evaluated_clip(), self.time, &before.staged) {
                Ok(scene) => scene,
                Err(error) => { self.restore(before); return Err(error); }
            };
            for id in detached {
                let Some(local) = sampled.local.get(&id).copied() else {
                    self.restore(before); return Err(CoreError::MissingNode(id));
                };
                if let Err(error) = self.project.detach_curve_node(id, &sampled.global, local) {
                    self.restore(before); return Err(error);
                }
            }
            for index in detached_lights {
                match before.project.evaluate_light(&self.project.lights[index], &sampled.global, self.time) {
                    Ok(light) => self.project.lights[index] = light,
                    Err(error) => { self.restore(before); return Err(error); }
                }
            }
        }
        let missing_targets = self.project.lights.iter().any(|l| {
            l.target
                .is_some_and(|t| self.project.node(t.node).is_none())
        });
        if missing_targets {
            let globals = evaluate_base(
                &before.project,
                self.evaluated_clip(),
                self.time,
                &before.staged,
            )
            .map(|e| e.global)
            .unwrap_or_default();
            let nodes: BTreeSet<_> = self.project.nodes.iter().map(|n| n.id).collect();
            for light in &mut self.project.lights {
                if light.target.is_some_and(|t| !nodes.contains(&t.node)) {
                    // Resolve aiming in world space, then store the direction in
                    // the remaining path frame (if only the aim target vanished).
                    let direction = before.project.evaluate_light(light, &globals, self.time)
                        .map(|l| l.direction).unwrap_or(light.direction);
                    light.direction = light.curve_binding
                        .and_then(|b| before.project.curve_binding_frame(b, &globals, self.time, true).ok())
                        .map_or(direction, |frame| frame.inverse().transform_vector3(direction).normalize_or_zero());
                    light.target = None;
                }
            }
        }
        self.project.clean_outliner();
        if let Err(e) = self.project.validate() {
            self.restore(before);
            return Err(e);
        }
        self.staged.retain(|id, _| self.project.node(*id).is_some());
        self.staged_weights.retain(|id, w| self.project.node(*id).and_then(|n| n.mesh).and_then(|m| self.project.mesh(m)).is_some_and(|m| m.morph_targets.len() == w.len()));
        for n in &mut self.project.nodes {
            if n.rest_transform.is_none() {
                n.rest_transform = Some(n.transform);
            }
        }
        self.rest_pose = self
            .project
            .nodes
            .iter()
            .map(|n| (n.id, n.rest_transform.unwrap_or(n.transform)))
            .collect();
        self.record(label, before);
        Ok(())
    }
    /// Publish a prepared result without revalidating every vertex on the UI
    /// thread. A stale result or active edit is rejected without touching history.
    pub fn commit_prepared(
        &mut self,
        label: &str,
        edit: PreparedProjectEdit,
    ) -> Result<(), CoreError> {
        if self.gesture.is_some() || self.has_staged() {
            return Err(CoreError::Invalid(
                "Завершите текущее редактирование перед импортом".into(),
            ));
        }
        if self.project != edit.before {
            return Err(CoreError::Invalid(
                "Сцена изменилась во время импорта; повторите импорт".into(),
            ));
        }
        let before = self.snapshot();
        self.project = edit.after;
        self.rest_pose = edit.rest_pose;
        self.record(label, before);
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if self.gesture.is_some() {
            self.cancel_edit();
            return true;
        }
        if let Some(h) = self.undo.pop() {
            let now = self.snapshot();
            self.restore(h.snapshot);
            self.redo.push(HistoryEntry {
                label: h.label.clone(),
                snapshot: now,
            });
            self.status = format!("Отмена: {}", h.label);
            self.modified = true;
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if self.gesture.is_some() {
            return false;
        }
        if let Some(h) = self.redo.pop() {
            let now = self.snapshot();
            self.restore(h.snapshot);
            self.undo.push(HistoryEntry {
                label: h.label.clone(),
                snapshot: now,
            });
            self.status = format!("Повтор: {}", h.label);
            self.modified = true;
            true
        } else {
            false
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn begin_edit(&mut self, label: &str) {
        if self.gesture.is_none() {
            self.gesture = Some((label.into(), self.snapshot()));
        }
    }
    pub fn has_active_edit(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn commit_edit(&mut self) -> Result<(), CoreError> {
        if let Some((label, before)) = self.gesture.take() {
            if let Err(e) = self.project.validate() {
                self.restore(before);
                return Err(e);
            }
            self.record(&label, before);
        }
        Ok(())
    }
    pub fn cancel_edit(&mut self) {
        if let Some((_, before)) = self.gesture.take() {
            self.restore(before);
            self.status = "Действие отменено".into();
        }
    }
    pub fn evaluated_clip(&self) -> Option<usize> {
        if self.mode == EditorMode::Animation
            || self.playing
            || (self.mode == EditorMode::Render && self.render_animated)
        {
            self.active_clip
        } else {
            None
        }
    }
    pub fn evaluated(&self) -> Result<EvaluatedScene, CoreError> {
        let mut scene = evaluate(&self.project, self.evaluated_clip(), self.time, &self.staged)?;
        scene.morph_weights.extend(self.staged_weights.clone());
        Ok(scene)
    }
    pub fn evaluated_base(&self) -> Result<EvaluatedScene, CoreError> {
        let mut scene = crate::animation::evaluate_base(&self.project, self.evaluated_clip(), self.time, &self.staged)?;
        scene.morph_weights.extend(self.staged_weights.clone());
        Ok(scene)
    }
    pub fn has_staged(&self) -> bool {
        !self.staged.is_empty() || !self.staged_weights.is_empty()
    }
    pub fn set_time(&mut self, time: f64) -> Result<(), CoreError> {
        if !time.is_finite() {
            return Err(CoreError::Invalid("время".into()));
        }
        if self.has_staged() && (time - self.time).abs() > 1e-9 {
            self.playing = false;
            return Err(CoreError::UncommittedPose);
        }
        self.time = time.clamp(0., self.project.duration);
        Ok(())
    }
    pub fn advance(&mut self, seconds: f64) -> Result<(), CoreError> {
        if self.playing && self.project.duration > 0. {
            self.set_time((self.time + seconds.max(0.)).rem_euclid(self.project.duration))?;
        }
        Ok(())
    }
    /// Move to a frame using the project's time grid without retiming its keys.
    pub fn set_frame(&mut self, frame: u32) -> Result<(), CoreError> {
        if !(1..=240).contains(&self.project.fps) {
            return Err(CoreError::Invalid("частота кадров".into()));
        }
        self.set_time(frame as f64 / self.project.fps as f64)
    }
    fn require_animation_command(&self, allow_staged: bool) -> Result<(), CoreError> {
        if self.gesture.is_some() {
            return Err(CoreError::Invalid(
                "завершите перетаскивание перед изменением ключей или клипов".into(),
            ));
        }
        if !allow_staged && self.has_staged() {
            return Err(CoreError::UncommittedPose);
        }
        self.project.validate()?;
        if !self.time.is_finite() || self.time < 0. {
            return Err(CoreError::Invalid("текущее время".into()));
        }
        Ok(())
    }
    fn active_clip_index(&self) -> Result<usize, CoreError> {
        self.active_clip
            .filter(|index| *index < self.project.clips.len())
            .ok_or_else(|| CoreError::Invalid("сначала выберите или создайте клип".into()))
    }
    fn clip_name(&self, name: &str, except: Option<usize>) -> Result<String, CoreError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
            return Err(CoreError::Invalid(
                "имя клипа должно содержать от 1 до 128 символов без управляющих знаков".into(),
            ));
        }
        if self.project.clips.iter().enumerate().any(|(index, clip)| {
            Some(index) != except && clip.name.to_lowercase() == name.to_lowercase()
        }) {
            return Err(CoreError::Invalid(
                "клип с таким именем уже существует".into(),
            ));
        }
        Ok(name.to_owned())
    }
    fn checked_key_nodes(&self, nodes: &[NodeId]) -> Result<BTreeSet<NodeId>, CoreError> {
        if nodes.is_empty() {
            return Err(CoreError::Invalid(
                "сначала выберите объект или персонажа".into(),
            ));
        }
        let ids: BTreeSet<_> = nodes.iter().copied().collect();
        for id in &ids {
            let node = self.project.node(*id).ok_or(CoreError::MissingNode(*id))?;
            if node.locked {
                return Err(CoreError::Invalid(format!(
                    "объект «{}» заблокирован",
                    node.name
                )));
            }
        }
        Ok(ids)
    }
    fn finish_animation_command(&mut self, before: Snapshot, label: &str) -> Result<(), CoreError> {
        if let Err(error) = self.project.validate() {
            self.restore(before);
            return Err(error);
        }
        self.playing = false;
        self.record(label, before);
        Ok(())
    }
    /// Create and activate an empty clip. UI mode changes remain the caller's choice.
    pub fn create_clip(&mut self, name: &str) -> Result<usize, CoreError> {
        self.require_animation_command(false)?;
        let name = self.clip_name(name, None)?;
        let before = self.snapshot();
        let index = self.project.clips.len();
        self.project
            .clips
            .push(Clip::new(name, self.project.duration));
        self.active_clip = Some(index);
        self.time = 0.;
        self.finish_animation_command(before, "Создать клип")?;
        Ok(index)
    }
    pub fn rename_active_clip(&mut self, name: &str) -> Result<(), CoreError> {
        self.require_animation_command(false)?;
        let index = self.active_clip_index()?;
        let name = self.clip_name(name, Some(index))?;
        let before = self.snapshot();
        self.project.clips[index].name = name;
        self.finish_animation_command(before, "Переименовать клип")
    }
    /// Duplicate keys and their original interpolation/tangents without resampling.
    pub fn duplicate_active_clip(&mut self, name: &str) -> Result<usize, CoreError> {
        self.require_animation_command(false)?;
        let source = self.active_clip_index()?;
        let name = self.clip_name(name, None)?;
        let mut clip = self.project.clips[source].clone();
        clip.name = name;
        let before = self.snapshot();
        let index = self.project.clips.len();
        self.time = self.time.min(clip.duration);
        self.project.duration = self.project.duration.max(clip.duration);
        self.project.clips.push(clip);
        self.active_clip = Some(index);
        self.finish_animation_command(before, "Дублировать клип")?;
        Ok(index)
    }
    /// Key an explicit scope in one transaction and evaluate the visible pose once.
    /// Staged changes outside this scope remain pending and are never discarded.
    pub fn key_nodes(&mut self, nodes: &[NodeId]) -> Result<(), CoreError> {
        self.require_animation_command(true)?;
        let ids = self.checked_key_nodes(nodes)?;
        let evaluated = self.evaluated_base()?;
        let transforms: Vec<_> = ids
            .iter()
            .map(|id| {
                evaluated
                    .local
                    .get(id)
                    .copied()
                    .map(|transform| (*id, transform))
                    .ok_or(CoreError::MissingNode(*id))
            })
            .collect::<Result<_, _>>()?;
        let before = self.snapshot();
        let index = self.ensure_clip();
        for (id, transform) in transforms {
            self.project.clips[index].key_transform(id, self.time, transform);
            self.staged.remove(&id);
        }
        self.project.duration = self.project.duration.max(self.time);
        self.finish_animation_command(before, "Записать ключи персонажа")
    }
    /// Shift all scoped channel keys at the cursor by an exact number of frames.
    /// Any destination collision rejects the complete edit; existing keys survive.
    /// The count is channel-key records (a transform normally has three records).
    pub fn shift_current_keys(
        &mut self,
        delta_frames: i32,
        nodes: &[NodeId],
    ) -> Result<usize, CoreError> {
        self.require_animation_command(false)?;
        let ids = self.checked_key_nodes(nodes)?;
        let index = self.active_clip_index()?;
        if delta_frames == 0 {
            return Ok(0);
        }
        let destination = self.time + delta_frames as f64 / self.project.fps as f64;
        if !destination.is_finite() || destination < 0. {
            return Err(CoreError::Invalid(
                "нельзя переместить ключи до начала анимации".into(),
            ));
        }
        let mut count = 0;
        for channel in &self.project.clips[index].channels {
            if !ids.contains(&channel.node) {
                continue;
            }
            let source_count = channel
                .keys
                .iter()
                .filter(|key| (key.time - self.time).abs() < 1e-9)
                .count();
            if source_count > 1 {
                return Err(CoreError::Invalid(
                    "в канале несколько почти совпадающих ключей".into(),
                ));
            }
            if source_count == 1 {
                if channel.keys.iter().any(|key| {
                    (key.time - self.time).abs() >= 1e-9 && (key.time - destination).abs() < 1e-9
                }) {
                    return Err(CoreError::Invalid(
                        "в целевом кадре уже есть ключ; выберите свободный кадр".into(),
                    ));
                }
                count += 1;
            }
        }
        if count == 0 {
            return Ok(0);
        }
        let before = self.snapshot();
        for channel in &mut self.project.clips[index].channels {
            if ids.contains(&channel.node) {
                for key in &mut channel.keys {
                    if (key.time - self.time).abs() < 1e-9 {
                        key.time = destination;
                    }
                }
                channel.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
            }
        }
        self.project.clips[index].duration = self.project.clips[index].duration.max(destination);
        self.project.duration = self.project.duration.max(destination);
        self.time = destination;
        self.finish_animation_command(before, "Сдвинуть ключи")?;
        Ok(count)
    }
    /// Delete scoped keys at the cursor and remove channels that become empty.
    pub fn delete_current_keys(&mut self, nodes: &[NodeId]) -> Result<usize, CoreError> {
        self.require_animation_command(false)?;
        let ids = self.checked_key_nodes(nodes)?;
        let index = self.active_clip_index()?;
        let before = self.snapshot();
        let mut count = 0;
        for channel in &mut self.project.clips[index].channels {
            if ids.contains(&channel.node) {
                let old_len = channel.keys.len();
                channel
                    .keys
                    .retain(|key| (key.time - self.time).abs() >= 1e-9);
                count += old_len - channel.keys.len();
            }
        }
        self.project.clips[index]
            .channels
            .retain(|channel| !channel.keys.is_empty());
        self.finish_animation_command(before, "Удалить ключи")?;
        Ok(count)
    }

    /// A locked ancestor protects its descendant animation just as it protects
    /// viewport transforms. All targets are checked before any channel changes.
    fn checked_animation_targets(
        &self,
        targets: impl IntoIterator<Item = NodeId>,
    ) -> Result<(), CoreError> {
        let ids: BTreeSet<_> = targets.into_iter().collect();
        for id in ids {
            let mut current = Some(id);
            let mut visited = BTreeSet::new();
            while let Some(node_id) = current {
                if !visited.insert(node_id) {
                    return Err(CoreError::Invalid(
                        "цикл в родителях выбранного объекта".into(),
                    ));
                }
                let node = self
                    .project
                    .node(node_id)
                    .ok_or(CoreError::MissingNode(node_id))?;
                if node.locked {
                    return Err(CoreError::Invalid(format!(
                        "объект или родитель «{}» заблокирован",
                        node.name
                    )));
                }
                current = node.parent;
            }
        }
        Ok(())
    }

    /// Copy committed key records without resampling their curves. The returned
    /// times start at zero, preserving all relative second offsets and tangents.
    /// This also validates a selection before beginning an interactive retime.
    pub fn copy_keys(&self, references: &[KeyRef]) -> Result<KeyClipboard, CoreError> {
        self.require_animation_command(false)?;
        let index = self.active_clip_index()?;
        self.checked_animation_targets(references.iter().map(|reference| reference.node))?;
        self.project.clips[index].copied_keys(references)
    }

    fn retime_selected_keys(
        &mut self,
        references: &[KeyRef],
        delta_frames: i32,
        duplicate: bool,
    ) -> Result<Vec<KeyRef>, CoreError> {
        self.require_animation_command(false)?;
        let index = self.active_clip_index()?;
        self.checked_animation_targets(references.iter().map(|reference| reference.node))?;
        let (clip, references) = self.project.clips[index].retimed_keys(
            references,
            delta_frames as f64 / self.project.fps as f64,
            duplicate,
        )?;
        if delta_frames == 0 && !duplicate {
            return Ok(references);
        }
        let before = self.snapshot();
        self.project.duration = self.project.duration.max(clip.duration);
        self.project.clips[index] = clip;
        self.finish_animation_command(
            before,
            if duplicate {
                "Дублировать выделенные ключи"
            } else {
                "Переместить выделенные ключи"
            },
        )?;
        Ok(references)
    }

    /// Move selected keys by a frame delta; the playhead stays where it was.
    /// Occupied destinations reject the complete edit, without overwriting.
    pub fn move_keys(
        &mut self,
        references: &[KeyRef],
        delta_frames: i32,
    ) -> Result<Vec<KeyRef>, CoreError> {
        self.retime_selected_keys(references, delta_frames, false)
    }

    pub fn duplicate_keys(
        &mut self,
        references: &[KeyRef],
        delta_frames: i32,
    ) -> Result<Vec<KeyRef>, CoreError> {
        self.retime_selected_keys(references, delta_frames, true)
    }

    pub fn delete_keys(&mut self, references: &[KeyRef]) -> Result<usize, CoreError> {
        self.require_animation_command(false)?;
        let index = self.active_clip_index()?;
        self.checked_animation_targets(references.iter().map(|reference| reference.node))?;
        let resolved = self.project.clips[index].resolve_key_refs(references)?;
        let count = resolved.values().map(BTreeSet::len).sum();
        let before = self.snapshot();
        for channel in &mut self.project.clips[index].channels {
            if let Some(indices) = resolved.get(&(channel.node, channel.property)) {
                let mut index = 0;
                channel.keys.retain(|_| {
                    let keep = !indices.contains(&index);
                    index += 1;
                    keep
                });
            }
        }
        self.project.clips[index]
            .channels
            .retain(|channel| !channel.keys.is_empty());
        self.finish_animation_command(before, "Удалить выделенные ключи")?;
        Ok(count)
    }

    /// Paste into the original node/property targets of the active clip. The
    /// earliest copied key lands on destination_frame; relative times remain in
    /// seconds even when the project FPS changed after copying.
    pub fn paste_keys(
        &mut self,
        clipboard: &KeyClipboard,
        destination_frame: u32,
    ) -> Result<Vec<KeyRef>, CoreError> {
        self.require_animation_command(false)?;
        let index = self.active_clip_index()?;
        self.checked_animation_targets(clipboard.channels.iter().map(|channel| channel.node))?;
        let copied = Clip {
            name: "Скопированные ключи".into(),
            duration: clipboard.duration(),
            channels: clipboard.channels.clone(),
        };
        copied.validate(&self.project)?;
        let (clip, references) = self.project.clips[index].pasted_keys(
            clipboard,
            destination_frame as f64 / self.project.fps as f64,
        )?;
        let before = self.snapshot();
        self.project.duration = self.project.duration.max(clip.duration);
        self.project.clips[index] = clip;
        self.finish_animation_command(before, "Вставить ключи")?;
        Ok(references)
    }

    /// Interpolation belongs to the complete referenced channels, not individual
    /// key segments. Only this explicit command may replace imported tangents.
    pub fn set_key_interpolation(
        &mut self,
        references: &[KeyRef],
        preset: KeyInterpolationPreset,
    ) -> Result<usize, CoreError> {
        self.require_animation_command(false)?;
        let index = self.active_clip_index()?;
        self.checked_animation_targets(references.iter().map(|reference| reference.node))?;
        let resolved = self.project.clips[index].resolve_key_refs(references)?;
        let before = self.snapshot();
        let mut changed = 0;
        for channel in &mut self.project.clips[index].channels {
            if !resolved.contains_key(&(channel.node, channel.property)) {
                continue;
            }
            let old = channel.clone();
            channel.interpolation = match preset {
                KeyInterpolationPreset::Step => Interpolation::Step,
                KeyInterpolationPreset::Linear => Interpolation::Linear,
                KeyInterpolationPreset::EaseInOut => Interpolation::CubicSpline,
            };
            let mut previous_rotation: Option<Quat> = None;
            for key in &mut channel.keys {
                if preset == KeyInterpolationPreset::EaseInOut {
                    // Zero-tangent quaternion Hermite curves must use consistent
                    // hemispheres, or antipodal equivalent keys pass through 0.
                    if channel.property == Property::Rotation {
                        let mut rotation =
                            Quat::from_array(key.value.as_slice().try_into().unwrap());
                        if previous_rotation.is_some_and(|previous| previous.dot(rotation) < 0.) {
                            rotation = -rotation;
                            key.value = rotation.to_array().to_vec();
                        }
                        previous_rotation = Some(rotation);
                    }
                    key.in_tangent = Some(vec![0.; key.value.len()]);
                    key.out_tangent = Some(vec![0.; key.value.len()]);
                } else {
                    key.in_tangent = None;
                    key.out_tangent = None;
                }
            }
            changed += usize::from(*channel != old);
        }
        self.finish_animation_command(before, "Изменить интерполяцию каналов")?;
        Ok(changed)
    }
    fn ensure_clip(&mut self) -> usize {
        if let Some(i) = self.active_clip {
            if i < self.project.clips.len() {
                return i;
            }
        }
        self.project
            .clips
            .push(Clip::new("Движение", self.project.duration));
        let i = self.project.clips.len() - 1;
        self.active_clip = Some(i);
        i
    }
    pub fn set_transform(&mut self, id: NodeId, transform: Transform) -> Result<(), CoreError> {
        if self.mode != EditorMode::Animation && self.has_staged() {
            return Err(CoreError::UncommittedPose);
        }
        if self.project.node_locked(id) {
            return Err(CoreError::Invalid(
                "объект или коллекция заблокированы".into(),
            ));
        }
        if !transform.is_valid() {
            return Err(CoreError::Invalid("трансформация".into()));
        }
        if self
            .project
            .node(id)
            .ok_or(CoreError::MissingNode(id))?
            .locked
        {
            return Err(CoreError::Invalid("объект заблокирован".into()));
        }
        let before = if self.gesture.is_none() {
            Some(self.snapshot())
        } else {
            None
        };
        if self.mode == EditorMode::Animation {
            if self.auto_key {
                let index = self.ensure_clip();
                self.project.clips[index].key_transform(id, self.time, transform);
                self.staged.remove(&id);
            } else {
                self.staged.insert(id, transform);
            }
        } else {
            self.project.node_mut(id).unwrap().transform = transform;
        }
        if let Some(before) = before {
            self.record("Изменение позы", before);
        }
        Ok(())
    }
    pub(crate) fn change_morph_weight(&mut self, id: NodeId, index: usize, value: f32) -> Result<(), CoreError> {
        if self.mode != EditorMode::Animation && self.has_staged() { return Err(CoreError::UncommittedPose); }
        let n = self.project.node(id).ok_or(CoreError::MissingNode(id))?;
        if self.project.node_locked(id) { return Err(CoreError::Invalid("Объект заблокирован".into())); }
        let count = n.mesh.and_then(|m| self.project.mesh(m)).map_or(0, |m| m.morph_targets.len());
        if index >= count || !value.is_finite() || !(0. ..=1.).contains(&value) { return Err(CoreError::Invalid("Вес морфа: 0–1".into())); }
        let mut weights = self.evaluated_base()?.morph_weights.get(&id).cloned().unwrap_or(n.morph_weights.clone());
        weights.resize(count, 0.); weights[index] = value;
        let before = self.snapshot();
        if self.mode == EditorMode::Animation {
            if self.auto_key {
                let clip = self.ensure_clip();
                crate::morph::key_weights(&mut self.project.clips[clip], id, self.time, weights);
                self.staged_weights.remove(&id);
            } else { self.staged_weights.insert(id, weights); }
        } else { self.project.node_mut(id).unwrap().morph_weights = weights; }
        self.record("Вес морфа", before); Ok(())
    }
    pub fn key_morph_weights(&mut self, id: NodeId) -> Result<(), CoreError> {
        if self.project.node_locked(id) { return Err(CoreError::Invalid("Объект заблокирован".into())); }
        let n = self.project.node(id).ok_or(CoreError::MissingNode(id))?;
        let count = n.mesh.and_then(|m| self.project.mesh(m)).map_or(0, |m| m.morph_targets.len());
        if count == 0 { return Err(CoreError::Invalid("Нет морфов для ключа".into())); }
        let mut weights = self.evaluated_base()?.morph_weights.get(&id).cloned().unwrap_or(n.morph_weights.clone());
        weights.resize(count, 0.);
        let before = self.snapshot(); let index = self.ensure_clip();
        crate::morph::key_weights(&mut self.project.clips[index], id, self.time, weights);
        self.project.duration = self.project.duration.max(self.time);
        self.staged_weights.remove(&id); self.record("Ключ весов морфов", before); Ok(())
    }
    pub fn key_transform(&mut self, id: NodeId) -> Result<(), CoreError> {
        let transform = *self
            .evaluated_base()?
            .local
            .get(&id)
            .ok_or(CoreError::MissingNode(id))?;
        let before = self.snapshot();
        let index = self.ensure_clip();
        self.project.clips[index].key_transform(id, self.time, transform);
        self.staged.remove(&id);
        self.project.duration = self.project.duration.max(self.time);
        self.record("Поставить ключ", before);
        Ok(())
    }
    pub fn key_selected(&mut self) -> Result<(), CoreError> {
        let id = self
            .selected
            .ok_or_else(|| CoreError::Invalid("сначала выберите объект или сустав".into()))?;
        if self.staged_weights.contains_key(&id) {
            let before = self.snapshot();
            // The ordinary key command also records visible, staged morphs.
            let weights = self.staged_weights[&id].clone();
            let transform = *self.evaluated_base()?.local.get(&id).ok_or(CoreError::MissingNode(id))?;
            let index = self.ensure_clip();
            self.project.clips[index].key_transform(id,self.time,transform);
            crate::morph::key_weights(&mut self.project.clips[index],id,self.time,weights);
            self.staged.remove(&id); self.staged_weights.remove(&id); self.project.duration=self.project.duration.max(self.time);
            self.record("Поставить ключ позы и морфов",before); Ok(())
        } else { self.key_transform(id) }
    }
    pub fn resolve_staged(&mut self, resolution: StagedResolution) -> Result<(), CoreError> {
        if !self.has_staged() {
            return Ok(());
        }
        let before = self.snapshot();
        if resolution == StagedResolution::Commit {
            let index = self.ensure_clip();
            for (&id, &transform) in &self.staged {
                self.project.clips[index].key_transform(id, self.time, transform);
            }
        }
        if resolution == StagedResolution::Commit {
            let index = self.ensure_clip();
            for (&id, weights) in &self.staged_weights {
                crate::morph::key_weights(&mut self.project.clips[index], id, self.time, weights.clone());
            }
            self.project.duration = self.project.duration.max(self.time);
        }
        self.staged.clear();
        self.staged_weights.clear();
        self.record(
            if resolution == StagedResolution::Commit {
                "Записать позу в ключи"
            } else {
                "Отбросить правку позы"
            },
            before,
        );
        Ok(())
    }
    pub fn reset_selected(&mut self) -> Result<(), CoreError> {
        let id = self
            .selected
            .ok_or_else(|| CoreError::Invalid("сначала выберите объект или сустав".into()))?;
        let transform = *self.rest_pose.get(&id).ok_or(CoreError::MissingNode(id))?;
        self.set_transform(id, transform)
    }
    pub fn reset_all(&mut self) -> Result<(), CoreError> {
        self.begin_edit("Сбросить позу");
        let entries: Vec<_> = self
            .rest_pose
            .iter()
            .filter(|(id, _)| {
                self.project
                    .node(**id)
                    .is_some_and(|n| n.is_joint && !n.locked)
            })
            .map(|(&id, &t)| (id, t))
            .collect();
        for (id, t) in entries {
            if let Err(e) = self.set_transform(id, t) {
                self.cancel_edit();
                return Err(e);
            }
        }
        self.commit_edit()
    }
    pub fn apply_ik(
        &mut self,
        root: NodeId,
        middle: NodeId,
        end: NodeId,
        target: Vec3,
        pole: Vec3,
    ) -> Result<(), CoreError> {
        let pose = self.evaluated_base()?;
        let transforms =
            two_bone_transforms(&self.project, &pose.local, root, middle, end, target, pole)?;
        let own_gesture = self.gesture.is_none();
        if own_gesture {
            self.begin_edit("IK конечности");
        }
        for (id, t) in transforms {
            if let Err(e) = self.set_transform(id, t) {
                if own_gesture {
                    self.cancel_edit();
                }
                return Err(e);
            }
        }
        if own_gesture {
            self.commit_edit()?;
        }
        Ok(())
    }
    pub fn copy_selected_pose(&mut self) -> Result<(), CoreError> {
        let id = self
            .selected
            .ok_or_else(|| CoreError::Invalid("нет выделения".into()))?;
        self.clipboard = self.evaluated_base()?.local.get(&id).copied();
        Ok(())
    }
    pub fn paste_selected_pose(&mut self) -> Result<(), CoreError> {
        let id = self
            .selected
            .ok_or_else(|| CoreError::Invalid("нет выделения".into()))?;
        let t = self
            .clipboard
            .ok_or_else(|| CoreError::Invalid("буфер поз пуст".into()))?;
        self.set_transform(id, t)
    }
    pub fn save_pose(&mut self, name: &str) -> Result<(), CoreError> {
        let transforms = self
            .evaluated_base()?
            .local
            .into_iter()
            .filter(|(id, _)| self.project.node(*id).is_some_and(|n| n.is_joint))
            .collect();
        self.transact("Сохранить позу", |p| {
            p.poses.push(SavedPose {
                name: name.into(),
                transforms,
            })
        })
    }
    pub fn load_pose(&mut self, index: usize) -> Result<(), CoreError> {
        let pose = self
            .project
            .poses
            .get(index)
            .ok_or_else(|| CoreError::Invalid("поза не найдена".into()))?
            .clone();
        self.begin_edit("Применить позу");
        for (id, t) in pose.transforms {
            if self.project.node(id).is_some() {
                if let Err(e) = self.set_transform(id, t) {
                    self.cancel_edit();
                    return Err(e);
                }
            }
        }
        self.commit_edit()
    }
    /// Mirror local pose across the X plane using explicit symmetric pairs.
    pub fn mirror_pose(&mut self, pairs: &[(NodeId, NodeId)]) -> Result<(), CoreError> {
        let local = self.evaluated_base()?.local;
        let mirror = |t: Transform| {
            let q = t.rotation;
            Transform {
                translation: Vec3::new(-t.translation.x, t.translation.y, t.translation.z),
                rotation: Quat::from_xyzw(q.x, -q.y, -q.z, q.w).normalize(),
                scale: t.scale,
            }
        };
        let mut seen = BTreeSet::new();
        for &(a, b) in pairs {
            if !seen.insert(a)
                || !seen.insert(b)
                || !local.contains_key(&a)
                || !local.contains_key(&b)
            {
                return Err(CoreError::Invalid("некорректные пары симметрии".into()));
            }
        }
        self.begin_edit("Зеркальная поза");
        for &(a, b) in pairs {
            for (id, t) in [(a, mirror(local[&b])), (b, mirror(local[&a]))] {
                if let Err(e) = self.set_transform(id, t) {
                    self.cancel_edit();
                    return Err(e);
                }
            }
        }
        self.commit_edit()
    }
    pub fn delete_selected(&mut self) -> Result<(), CoreError> {
        let id = self
            .selected
            .ok_or_else(|| CoreError::Invalid("нет выделения".into()))?;
        self.delete_node(id)
    }
    pub fn delete_node(&mut self, id: NodeId) -> Result<(), CoreError> {
        if self.project.node(id).is_none() {
            return Err(CoreError::MissingNode(id));
        }
        let mut ids = BTreeSet::from([id]);
        loop {
            let before = ids.len();
            for n in &self.project.nodes {
                if ids.contains(&n.id) {
                    if let Some(root) = n.destruction.and_then(|d| d.result) { ids.insert(root); }
                }
                if n.parent.is_some_and(|p| ids.contains(&p)) {
                    ids.insert(n.id);
                }
            }
            if before == ids.len() {
                break;
            }
        }
        // Referenced joints may not be deleted independently from a skinned mesh.
        if self
            .project
            .nodes
            .iter()
            .filter(|n| !ids.contains(&n.id))
            .any(|n| {
                n.skin
                    .as_ref()
                    .is_some_and(|s| s.joints.iter().any(|j| ids.contains(j)))
            })
        {
            return Err(CoreError::Invalid(
                "сустав используется skin; удалите весь персонаж".into(),
            ));
        }
        if ids.iter().any(|id| self.project.node_locked(*id)) {
            return Err(CoreError::Invalid(
                "объект или коллекция заблокированы".into(),
            ));
        }
        self.transact("Удалить объект", |p| {
            p.nodes.retain(|n| !ids.contains(&n.id));
            for node in &mut p.nodes {
                if let Some(settings) = &mut node.destruction {
                    if settings.result.is_some_and(|root| ids.contains(&root)) {
                        settings.result = None;
                        node.visible = settings.source_visible; node.render_visible = settings.source_render_visible;
                    }
                }
            }
            for collection in &mut p.collections {
                collection.members.retain(|id| !ids.contains(id));
            }
            if p.active_camera.is_some_and(|id| ids.contains(&id)) {
                p.active_camera = None;
            }
            for c in &mut p.clips {
                c.channels.retain(|c| !ids.contains(&c.node));
            }
            for pose in &mut p.poses {
                pose.transforms.retain(|id, _| !ids.contains(id));
            }
            p.rig_profiles.retain_mut(|profile| {
                if profile.root.is_some_and(|root| ids.contains(&root)) {
                    return false;
                }
                profile.assignments.retain(|_, id| !ids.contains(id));
                !profile.assignments.is_empty()
            });
        })?;
        if self.selected.is_some_and(|id| ids.contains(&id)) {
            self.selected = None;
        }
        Ok(())
    }
}
