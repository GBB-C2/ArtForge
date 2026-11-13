use crate::{CoreError, NodeId, Project, Transform};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Property {
    Translation,
    Rotation,
    Scale,
    Weights,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}

/// A document key address, independent of channel order and list indices.
/// Editors should replace these addresses with the returned addresses after a
/// retime and clear them when changing the active clip or document.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyRef {
    pub node: NodeId,
    pub property: Property,
    pub time: f64,
}

/// Deliberate authoring operations. Ease is stored as an ordinary glTF-style
/// cubic channel with zero tangents; imported curves remain unchanged until the
/// user explicitly chooses a preset for their channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInterpolationPreset {
    Step,
    Linear,
    EaseInOut,
}

/// An application-local key clipboard. Times are relative to the earliest key,
/// in seconds; node/property targets and original interpolation are retained.
#[derive(Debug, Clone)]
pub struct KeyClipboard {
    pub(crate) channels: Vec<Channel>,
    duration: f64,
}
impl KeyClipboard {
    /// Number of channel-key records (one pose marker can contain TRS records).
    pub fn len(&self) -> usize {
        self.channels.iter().map(|channel| channel.keys.len()).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.channels.iter().all(|channel| channel.keys.is_empty())
    }
    pub fn duration(&self) -> f64 {
        self.duration
    }
}

const KEY_TIME_EPSILON: f64 = 1e-9;
type ResolvedKeys = BTreeMap<(NodeId, Property), BTreeSet<usize>>;

fn key_time_conflicts(keys: &[Keyframe], time: f64) -> bool {
    let next = keys.partition_point(|key| key.time < time);
    keys.get(next)
        .is_some_and(|key| (key.time - time).abs() < KEY_TIME_EPSILON)
        || next
            .checked_sub(1)
            .and_then(|index| keys.get(index))
            .is_some_and(|key| (key.time - time).abs() < KEY_TIME_EPSILON)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Keyframe {
    pub time: f64,
    pub value: Vec<f32>,
    pub in_tangent: Option<Vec<f32>>,
    pub out_tangent: Option<Vec<f32>>,
}
impl Keyframe {
    pub fn new(time: f64, value: Vec<f32>) -> Self {
        Self {
            time,
            value,
            in_tangent: None,
            out_tangent: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Channel {
    pub node: NodeId,
    pub property: Property,
    pub interpolation: Interpolation,
    pub keys: Vec<Keyframe>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Clip {
    pub name: String,
    pub duration: f64,
    pub channels: Vec<Channel>,
}
impl Clip {
    pub fn new(name: impl Into<String>, duration: f64) -> Self {
        Self {
            name: name.into(),
            duration,
            channels: vec![],
        }
    }
    pub fn validate(&self, project: &Project) -> Result<(), CoreError> {
        if !self.duration.is_finite() || self.duration < 0. {
            return Err(CoreError::Invalid("длительность клипа".into()));
        }
        let mut targets = BTreeSet::new();
        for c in &self.channels {
            let node = project.node(c.node).ok_or(CoreError::MissingNode(c.node))?;
            if !targets.insert((c.node, c.property)) {
                return Err(CoreError::Invalid("два канала одной цели в клипе".into()));
            }
            let len = match c.property {
                Property::Translation | Property::Scale => 3,
                Property::Rotation => 4,
                Property::Weights => node
                    .mesh
                    .and_then(|id| project.mesh(id))
                    .map(|m| m.morph_targets.len())
                    .unwrap_or(node.morph_weights.len()),
            };
            if c.keys.is_empty() || len == 0 {
                return Err(CoreError::Invalid("пустой канал анимации".into()));
            }
            let mut previous = -1.;
            for k in &c.keys {
                if !k.time.is_finite()
                    || k.time < 0.
                    || k.time > self.duration + 1e-8
                    || k.time <= previous
                    || k.value.len() != len
                    || k.value.iter().any(|v| !v.is_finite())
                {
                    return Err(CoreError::Invalid("время, размер или данные ключа".into()));
                }
                previous = k.time;
                if c.property == Property::Rotation
                    && (Quat::from_array(k.value[..4].try_into().unwrap()).length_squared() - 1.)
                        .abs()
                        > 0.01
                {
                    return Err(CoreError::Invalid("ключ вращения не нормализован".into()));
                }
                if c.interpolation == Interpolation::CubicSpline {
                    for tangent in [&k.in_tangent, &k.out_tangent] {
                        if tangent
                            .as_ref()
                            .map(|t| t.len() != len || t.iter().any(|v| !v.is_finite()))
                            .unwrap_or(true)
                        {
                            return Err(CoreError::Invalid(
                                "кубический канал требует исходные касательные".into(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn key_transform(&mut self, node: NodeId, time: f64, t: Transform) {
        for (property, value) in [
            (Property::Translation, t.translation.to_array().to_vec()),
            (Property::Rotation, t.rotation.to_array().to_vec()),
            (Property::Scale, t.scale.to_array().to_vec()),
        ] {
            if !self
                .channels
                .iter()
                .any(|c| c.node == node && c.property == property)
            {
                self.channels.push(Channel {
                    node,
                    property,
                    interpolation: Interpolation::Linear,
                    keys: vec![],
                });
            }
            let c = self
                .channels
                .iter_mut()
                .find(|c| c.node == node && c.property == property)
                .unwrap();
            let mut key = Keyframe::new(time, value);
            if c.interpolation == Interpolation::CubicSpline {
                key.in_tangent = Some(vec![0.; key.value.len()]);
                key.out_tangent = Some(vec![0.; key.value.len()]);
            }
            if let Some(old) = c.keys.iter_mut().find(|k| (k.time - time).abs() < 1e-9) {
                *old = key;
            } else {
                c.keys.push(key);
                c.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
            }
        }
        self.duration = self.duration.max(time);
    }
    pub fn key_times(&self, node: Option<NodeId>) -> Vec<f64> {
        let mut times: Vec<_> = self
            .channels
            .iter()
            .filter(|c| node.map(|id| c.node == id).unwrap_or(true))
            .flat_map(|c| c.keys.iter().map(|k| k.time))
            .collect();
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        times
    }

    /// Resolve the complete selection before mutating anything. Near-coincident
    /// imported keys must not be guessed from an ambiguous screen marker.
    pub(crate) fn resolve_key_refs(
        &self,
        references: &[KeyRef],
    ) -> Result<ResolvedKeys, CoreError> {
        if references.is_empty() {
            return Err(CoreError::Invalid(
                "сначала выделите ключи на шкале времени".into(),
            ));
        }
        let mut channels = BTreeMap::new();
        for channel in &self.channels {
            if channels
                .insert((channel.node, channel.property), channel)
                .is_some()
            {
                return Err(CoreError::Invalid(
                    "неоднозначная цель канала анимации".into(),
                ));
            }
            if channel.keys.iter().any(|key| !key.time.is_finite())
                || channel
                    .keys
                    .windows(2)
                    .any(|pair| pair[0].time >= pair[1].time)
            {
                return Err(CoreError::Invalid(
                    "ключи канала должны иметь возрастающее конечное время".into(),
                ));
            }
        }
        let mut resolved = ResolvedKeys::new();
        for reference in references {
            if !reference.time.is_finite() || reference.time < 0. {
                return Err(CoreError::Invalid(
                    "некорректное время выделенного ключа".into(),
                ));
            }
            let target = (reference.node, reference.property);
            let channel = channels.get(&target).ok_or_else(|| {
                CoreError::Invalid(
                    "выделенный канал больше не существует; выделите ключи заново".into(),
                )
            })?;
            // At most two neighboring keys on each side are enough to reject
            // ambiguity. Never scan a whole channel for every selected key.
            let insertion = channel
                .keys
                .partition_point(|key| key.time < reference.time);
            let start = insertion.saturating_sub(2);
            let end = insertion.saturating_add(2).min(channel.keys.len());
            let mut matches = channel.keys[start..end]
                .iter()
                .enumerate()
                .filter(|(_, key)| (key.time - reference.time).abs() < KEY_TIME_EPSILON);
            let (index, _) = matches.next().ok_or_else(|| {
                CoreError::Invalid(
                    "выделенный ключ больше не существует; выделите ключи заново".into(),
                )
            })?;
            if matches.next().is_some() {
                return Err(CoreError::Invalid(
                    "в канале несколько почти совпадающих ключей".into(),
                ));
            }
            resolved.entry(target).or_default().insert(start + index);
        }
        Ok(resolved)
    }

    /// Produce a non-mutating retime preview from a captured start clip. This
    /// preserves off-grid times, interpolation and every original tangent. A
    /// caller can install successive previews within one editor transaction.
    pub fn retimed_keys(
        &self,
        references: &[KeyRef],
        delta_seconds: f64,
        duplicate: bool,
    ) -> Result<(Self, Vec<KeyRef>), CoreError> {
        if !delta_seconds.is_finite() {
            return Err(CoreError::Invalid(
                "сдвиг ключей должен быть конечным".into(),
            ));
        }
        let resolved = self.resolve_key_refs(references)?;
        let mut candidate = self.clone();
        let mut destinations = Vec::new();
        for channel in &mut candidate.channels {
            let Some(indices) = resolved.get(&(channel.node, channel.property)) else {
                continue;
            };
            let mut moved = Vec::with_capacity(indices.len());
            let mut retained = Vec::with_capacity(channel.keys.len());
            for (index, key) in channel.keys.iter().enumerate() {
                if indices.contains(&index) {
                    let mut shifted = key.clone();
                    shifted.time += delta_seconds;
                    if !shifted.time.is_finite() || shifted.time < 0. {
                        return Err(CoreError::Invalid(
                            "нельзя переместить ключи до начала анимации".into(),
                        ));
                    }
                    moved.push(shifted);
                }
                if duplicate || !indices.contains(&index) {
                    retained.push(key.clone());
                }
            }
            if moved
                .windows(2)
                .any(|pair| (pair[1].time - pair[0].time).abs() < KEY_TIME_EPSILON)
            {
                return Err(CoreError::Invalid(
                    "сдвиг слишком велик для различимых времён ключей".into(),
                ));
            }
            for key in &moved {
                if key_time_conflicts(&retained, key.time) {
                    return Err(CoreError::Invalid(
                        "в целевом кадре уже есть ключ; существующие ключи не перезаписаны".into(),
                    ));
                }
                destinations.push(KeyRef {
                    node: channel.node,
                    property: channel.property,
                    time: key.time,
                });
                candidate.duration = candidate.duration.max(key.time);
            }
            retained.extend(moved);
            retained.sort_by(|a, b| a.time.total_cmp(&b.time));
            channel.keys = retained;
        }
        Ok((candidate, destinations))
    }

    pub(crate) fn copied_keys(&self, references: &[KeyRef]) -> Result<KeyClipboard, CoreError> {
        let resolved = self.resolve_key_refs(references)?;
        let origin = self
            .channels
            .iter()
            .filter_map(|channel| {
                resolved
                    .get(&(channel.node, channel.property))
                    .map(|indices| {
                        indices
                            .iter()
                            .map(|index| channel.keys[*index].time)
                            .fold(f64::INFINITY, f64::min)
                    })
            })
            .fold(f64::INFINITY, f64::min);
        let mut duration: f64 = 0.;
        let channels = self
            .channels
            .iter()
            .filter_map(|channel| {
                resolved
                    .get(&(channel.node, channel.property))
                    .map(|indices| {
                        let mut copied = channel.clone();
                        copied.keys = indices
                            .iter()
                            .map(|index| {
                                let mut key = channel.keys[*index].clone();
                                key.time -= origin;
                                duration = duration.max(key.time);
                                key
                            })
                            .collect();
                        copied
                    })
            })
            .collect();
        Ok(KeyClipboard { channels, duration })
    }

    pub(crate) fn pasted_keys(
        &self,
        clipboard: &KeyClipboard,
        destination_seconds: f64,
    ) -> Result<(Self, Vec<KeyRef>), CoreError> {
        if clipboard.is_empty() || !destination_seconds.is_finite() || destination_seconds < 0. {
            return Err(CoreError::Invalid(
                "нет скопированных ключей или неверное время вставки".into(),
            ));
        }
        let mut candidate = self.clone();
        let mut inserted = Vec::new();
        for source in &clipboard.channels {
            let existing = candidate.channels.iter().position(|channel| {
                channel.node == source.node && channel.property == source.property
            });
            if let Some(index) = existing {
                if candidate.channels[index].interpolation != source.interpolation {
                    return Err(CoreError::Invalid(
                        "интерполяция исходного и целевого канала различается; вставьте в новый клип или сначала выберите одинаковую интерполяцию".into(),
                    ));
                }
            }
            let mut shifted = source.clone();
            for key in &mut shifted.keys {
                key.time += destination_seconds;
                if !key.time.is_finite() {
                    return Err(CoreError::Invalid(
                        "время вставки ключа слишком велико".into(),
                    ));
                }
                if existing.is_some_and(|index| {
                    key_time_conflicts(&candidate.channels[index].keys, key.time)
                }) {
                    return Err(CoreError::Invalid(
                        "в целевом кадре уже есть ключ; существующие ключи не перезаписаны".into(),
                    ));
                }
                candidate.duration = candidate.duration.max(key.time);
                inserted.push(KeyRef {
                    node: source.node,
                    property: source.property,
                    time: key.time,
                });
            }
            if shifted
                .keys
                .windows(2)
                .any(|pair| (pair[1].time - pair[0].time).abs() < KEY_TIME_EPSILON)
            {
                return Err(CoreError::Invalid(
                    "время вставки слишком велико для различимых ключей".into(),
                ));
            }
            if let Some(index) = existing {
                candidate.channels[index].keys.extend(shifted.keys);
                candidate.channels[index]
                    .keys
                    .sort_by(|a, b| a.time.total_cmp(&b.time));
            } else {
                candidate.channels.push(shifted);
            }
        }
        Ok((candidate, inserted))
    }
}
impl Channel {
    /// Evaluate without mutable playback state. Cubic tangent values are multiplied
    /// by the interval duration, exactly as specified by glTF 2.0.
    pub fn sample(&self, time: f64) -> Option<Vec<f32>> {
        if !time.is_finite() {
            return None;
        }
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        if time <= first.time {
            return Some(first.value.clone());
        }
        if time >= last.time {
            return Some(last.value.clone());
        }
        let right = self.keys.partition_point(|key| key.time <= time);
        let a = &self.keys[right - 1];
        let b = &self.keys[right];
        let dt = (b.time - a.time) as f32;
        let u = ((time - a.time) / (b.time - a.time)) as f32;
        let mut value = match self.interpolation {
            Interpolation::Step => a.value.clone(),
            Interpolation::Linear if self.property == Property::Rotation => {
                let qa = Quat::from_array(a.value.as_slice().try_into().ok()?);
                let qb = Quat::from_array(b.value.as_slice().try_into().ok()?);
                qa.slerp(qb, u).normalize().to_array().to_vec()
            }
            Interpolation::Linear => a
                .value
                .iter()
                .zip(&b.value)
                .map(|(&a, &b)| a + (b - a) * u)
                .collect(),
            Interpolation::CubicSpline => {
                let outgoing = a.out_tangent.as_ref()?;
                let incoming = b.in_tangent.as_ref()?;
                let u2 = u * u;
                let u3 = u2 * u;
                (0..a.value.len())
                    .map(|i| {
                        (2. * u3 - 3. * u2 + 1.) * a.value[i]
                            + (u3 - 2. * u2 + u) * dt * outgoing[i]
                            + (-2. * u3 + 3. * u2) * b.value[i]
                            + (u3 - u2) * dt * incoming[i]
                    })
                    .collect()
            }
        };
        if self.property == Property::Rotation {
            let q = Quat::from_array(value.as_slice().try_into().ok()?);
            // A malformed curve can pass through a zero quaternion; preserve a
            // finite neighboring orientation rather than emit NaN into skinning.
            value = if q.length_squared() > 1e-12 {
                q.normalize().to_array().to_vec()
            } else {
                a.value.clone()
            };
        }
        Some(value)
    }
}
#[derive(Debug, Clone)]
pub struct EvaluatedScene {
    pub modifier_meshes: BTreeMap<NodeId, crate::Mesh>,
    pub text_meshes: BTreeMap<NodeId, crate::Mesh>,
    pub lights: Vec<crate::Light>,
    pub local: BTreeMap<NodeId, Transform>,
    pub global: BTreeMap<NodeId, Mat4>,
    pub morph_weights: BTreeMap<NodeId, Vec<f32>>,
}
pub fn evaluate(
    project: &Project,
    clip: Option<usize>,
    time: f64,
    overrides: &BTreeMap<NodeId, Transform>,
) -> Result<EvaluatedScene, CoreError> {
    let mut scene = evaluate_base(project, clip, time, overrides)?;
    scene.modifier_meshes = project.evaluated_modifier_meshes(&std::sync::atomic::AtomicBool::new(false))?;
    Ok(scene)
}
/// Transform evaluation without expensive procedural geometry; native previews
/// fill modifier_meshes from worker-owned results, while exports use evaluate.
pub fn evaluate_base(
    project: &Project,
    clip: Option<usize>,
    time: f64,
    overrides: &BTreeMap<NodeId, Transform>,
) -> Result<EvaluatedScene, CoreError> {
    if !time.is_finite() {
        return Err(CoreError::Invalid("время должно быть конечным".into()));
    }
    let mut local: BTreeMap<_, _> = project.nodes.iter().map(|n| (n.id, n.transform)).collect();
    let mut morph_weights: BTreeMap<_, _> = project
        .nodes
        .iter()
        .map(|n| (n.id, n.morph_weights.clone()))
        .collect();
    if let Some(index) = clip {
        let clip = project
            .clips
            .get(index)
            .ok_or_else(|| CoreError::Invalid("клип не найден".into()))?;
        for channel in &clip.channels {
            let Some(value) = channel.sample(time) else {
                continue;
            };
            let transform = local
                .get_mut(&channel.node)
                .ok_or(CoreError::MissingNode(channel.node))?;
            match channel.property {
                Property::Translation if value.len() == 3 => {
                    transform.translation = Vec3::from_slice(&value)
                }
                Property::Scale if value.len() == 3 => transform.scale = Vec3::from_slice(&value),
                Property::Rotation if value.len() == 4 => {
                    transform.rotation =
                        Quat::from_array(value.as_slice().try_into().unwrap()).normalize()
                }
                Property::Weights => {
                    morph_weights.insert(channel.node, value);
                }
                _ => return Err(CoreError::Invalid("размер значения канала".into())),
            }
        }
    }
    if clip.is_some() {
        crate::studio::apply_layers(project, time, &mut local, &mut morph_weights)?;
    }
    for (&id, &t) in overrides {
        if !local.contains_key(&id) {
            return Err(CoreError::MissingNode(id));
        }
        local.insert(id, t);
    }
    if local.values().any(|t| !t.is_valid()) {
        return Err(CoreError::Invalid(
            "анимация образует вырожденную трансформацию".into(),
        ));
    }
    let global = project.curve_globals_for(&local, time)?;
    let lights = project.evaluated_lights(&global, time)?;
    let text_meshes=project.evaluated_text_meshes(&global,&local,time)?;
    Ok(EvaluatedScene {
        modifier_meshes: BTreeMap::new(),
        text_meshes,
        lights,
        local,
        global,
        morph_weights,
    })
}
