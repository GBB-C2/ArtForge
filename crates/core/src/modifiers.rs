//! Ordered, instance-owned procedural geometry. Source buffers are never mutated.
use crate::{CoreError, Editor, Mesh, NodeId, Project};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
};

pub const MAX_MODIFIERS: usize = 32;
pub const MAX_MODIFIER_VERTICES: usize = 262_144;
pub const MAX_MODIFIER_TRIANGLES: usize = 524_288;
pub const MAX_MODIFIER_SCENE_VERTICES: usize = 1_048_576;
const CACHE_BYTES: usize = 96 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierKind {
    Mirror,
    Array,
    Subdivision,
    Solidify,
    Bevel,
}
impl ModifierKind {
    pub const ALL: [Self; 5] = [
        Self::Mirror,
        Self::Array,
        Self::Subdivision,
        Self::Solidify,
        Self::Bevel,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Mirror => "Зеркало",
            Self::Array => "Массив",
            Self::Subdivision => "Подразделение",
            Self::Solidify => "Толщина",
            Self::Bevel => "Фаска",
        }
    }
    pub fn settings(self) -> ModifierSettings {
        match self {
            Self::Bevel => ModifierSettings::Bevel {
                width: 0.05,
                segments: 3,
                profile: 0.5,
                angle: 0.5235988,
            },
            Self::Mirror => ModifierSettings::Mirror { axis: 0, plane: 0. },
            Self::Array => ModifierSettings::Array {
                count: 2,
                offset: [2., 0., 0.],
            },
            Self::Subdivision => ModifierSettings::Subdivision {
                levels: 1,
                smooth: true,
            },
            Self::Solidify => ModifierSettings::Solidify {
                thickness: 0.1,
                offset: 0.,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ModifierSettings {
    Mirror {
        axis: u8,
        plane: f32,
    },
    Array {
        count: u16,
        offset: [f32; 3],
    },
    RadialArray {
        count: u16,
        axis: u8,
        radius: f32,
        angle: f32,
        rotate: bool,
    },
    CurveArray {
        count: u16,
        curve: Option<NodeId>,
        spacing: f32,
        start: f32,
        roll: f32,
        follow: bool,
    },
    Bevel {
        width: f32,
        segments: u32,
        profile: f32,
        angle: f32,
    },
    Subdivision {
        levels: u8,
        smooth: bool,
    },
    Solidify {
        thickness: f32,
        offset: f32,
    },
}
impl ModifierSettings {
    pub fn kind(self) -> ModifierKind {
        match self {
            Self::Mirror { .. } => ModifierKind::Mirror,
            Self::Array { .. } | Self::RadialArray { .. } | Self::CurveArray { .. } => {
                ModifierKind::Array
            }
            Self::Bevel { .. } => ModifierKind::Bevel,
            Self::Subdivision { .. } => ModifierKind::Subdivision,
            Self::Solidify { .. } => ModifierKind::Solidify,
        }
    }
    pub fn validate(self) -> Result<(), CoreError> {
        let finite = |v: f32| v.is_finite() && v.abs() <= 10_000.;
        let valid = match self {
            Self::Mirror { axis, plane } => axis < 3 && finite(plane),
            Self::Array { count, offset } => {
                (1..=128).contains(&count) && offset.into_iter().all(finite)
            }
            Self::RadialArray {
                count,
                axis,
                radius,
                angle,
                ..
            } => {
                (1..=128).contains(&count)
                    && axis < 3
                    && finite(radius)
                    && radius >= 0.
                    && angle.is_finite()
                    && angle.abs() <= std::f32::consts::TAU
            }
            Self::CurveArray {
                count,
                curve,
                spacing,
                start,
                roll,
                ..
            } => {
                (1..=128).contains(&count)
                    && curve != Some(0)
                    && finite(spacing)
                    && spacing > 0.
                    && finite(start)
                    && roll.is_finite()
                    && roll.abs() <= std::f32::consts::TAU
            }
            Self::Bevel {
                width,
                segments,
                profile,
                angle,
            } => {
                finite(width)
                    && (0. ..=1000.).contains(&width)
                    && (1..=32).contains(&segments)
                    && profile.is_finite()
                    && (0. ..=1.).contains(&profile)
                    && angle.is_finite()
                    && (0. ..=std::f32::consts::PI).contains(&angle)
            }
            Self::Subdivision { levels, .. } => (1..=4).contains(&levels),
            Self::Solidify { thickness, offset } => {
                finite(thickness)
                    && thickness >= 0.
                    && offset.is_finite()
                    && (-1. ..=1.).contains(&offset)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(error("Недопустимые параметры модификатора"))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshModifier {
    /// Stable within its owner's stack; independent of the displayed position.
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub settings: ModifierSettings,
}
impl MeshModifier {
    pub fn new(id: u64, kind: ModifierKind) -> Self {
        Self {
            id,
            name: kind.label().into(),
            enabled: true,
            settings: kind.settings(),
        }
    }
}
fn error(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
pub(crate) fn cancelled(cancel: &AtomicBool) -> Result<(), CoreError> {
    if cancel.load(Ordering::Relaxed) {
        Err(error("Расчёт модификаторов отменён"))
    } else {
        Ok(())
    }
}
pub(crate) fn geometry_budget(vertices: usize, triangles: usize) -> Result<(), CoreError> {
    if vertices > MAX_MODIFIER_VERTICES || triangles > MAX_MODIFIER_TRIANGLES {
        Err(error("Стек превышает предел 262 144 вершин / 524 288 треугольников. Уменьшите число копий или уровней."))
    } else {
        Ok(())
    }
}

/// Conservative O(stack length) preflight. No exponentially growing allocation.
pub fn check_modifier_budget(mesh: &Mesh, stack: &[MeshModifier]) -> Result<(), CoreError> {
    output_budget(mesh, stack).map(|_| ())
}
fn output_budget(mesh: &Mesh, stack: &[MeshModifier]) -> Result<(usize, usize), CoreError> {
    let (mut v, mut t, mut f) = (
        mesh.vertices.len(),
        mesh.indices.len() / 3,
        mesh.face_count(),
    );
    for m in stack.iter().filter(|m| m.enabled) {
        m.settings.validate()?;
        match m.settings {
            ModifierSettings::Mirror { .. }
            | ModifierSettings::Array { .. }
            | ModifierSettings::RadialArray { .. }
            | ModifierSettings::CurveArray { .. } => {
                let count = match m.settings {
                    ModifierSettings::Array { count, .. }
                    | ModifierSettings::RadialArray { count, .. }
                    | ModifierSettings::CurveArray { count, .. } => count as usize,
                    _ => 2,
                };
                v = v.saturating_mul(count);
                t = t.saturating_mul(count);
                f = f.saturating_mul(count);
            }
            ModifierSettings::Bevel {
                segments, width, ..
            } if width > 0. => {
                let corners = t.saturating_add(f.saturating_mul(2));
                v = v.saturating_add(corners.saturating_mul(3 + segments as usize));
                t = t.saturating_add(corners.saturating_mul(3 + segments as usize * 2));
                f = t;
            }
            ModifierSettings::Subdivision { levels, .. } => {
                for _ in 0..levels {
                    let corners = t.saturating_add(f.saturating_mul(2));
                    v = corners.saturating_mul(4);
                    t = corners.saturating_mul(2);
                    f = corners;
                    geometry_budget(v, t)?;
                }
            }
            ModifierSettings::Solidify { thickness, .. } if thickness > 0. => {
                let corners = t.saturating_add(f.saturating_mul(2));
                v = v.saturating_mul(2);
                t = t
                    .saturating_mul(2)
                    .saturating_add(corners.saturating_mul(2));
                f = f.saturating_mul(2).saturating_add(corners);
            }
            _ => {}
        }
        geometry_budget(v, t)?;
    }
    Ok((v, t))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModifierKey {
    source: [u64; 5],
    seams: Vec<[u32; 2]>,
    wires: Vec<[u32; 2]>,
    material: Option<u64>,
    smooth: bool,
    settings: Vec<ModifierSettings>,
    frames: Vec<(u64, Vec<[f32; 16]>)>,
}
pub fn modifier_key(mesh: &Mesh, stack: &[MeshModifier]) -> ModifierKey {
    ModifierKey {
        frames: vec![],
        source: [
            mesh.id,
            mesh.vertices.revision(),
            mesh.indices.revision(),
            mesh.topology.as_ref().map_or(0, |t| t.faces.revision()),
            mesh.topology
                .as_ref()
                .map_or(0, |t| t.face_materials.revision()),
        ],
        seams: mesh
            .topology
            .as_ref()
            .map_or_else(Vec::new, |t| t.uv_seams.clone()),
        wires: mesh
            .topology
            .as_ref()
            .map_or_else(Vec::new, |t| t.loose_edges.clone()),
        material: mesh.material,
        smooth: mesh.smooth_normals_enabled(),
        settings: stack
            .iter()
            .filter(|m| m.enabled)
            .map(|m| m.settings)
            .collect(),
    }
}
#[derive(Clone)]
pub struct PreparedModifiers {
    pub key: ModifierKey,
    mesh: Mesh,
    stack: Vec<MeshModifier>,
    frames: BTreeMap<u64, Vec<crate::Mat4>>,
}
impl PreparedModifiers {
    pub fn cached(&self) -> Option<Result<Mesh, String>> {
        cached_key(&self.key)
    }
    pub fn evaluate(&self, cancel: &AtomicBool) -> Result<Mesh, CoreError> {
        evaluate_prepared(
            &self.mesh,
            &self.stack,
            self.key.clone(),
            &self.frames,
            cancel,
        )
    }
}
struct CacheEntry {
    key: ModifierKey,
    mesh: Result<Mesh, String>,
    bytes: usize,
}
static CACHE: OnceLock<Mutex<VecDeque<CacheEntry>>> = OnceLock::new();
fn cache() -> &'static Mutex<VecDeque<CacheEntry>> {
    CACHE.get_or_init(Default::default)
}
/// A cache miss never blocks on geometry construction. Used by the native UI.
pub fn cached_modifier_mesh(mesh: &Mesh, stack: &[MeshModifier]) -> Option<Result<Mesh, String>> {
    if !stack.iter().any(|m| m.enabled) {
        return Some(Ok(mesh.clone()));
    }
    cached_key(&modifier_key(mesh, stack))
}
fn cached_key(key: &ModifierKey) -> Option<Result<Mesh, String>> {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    let at = cache.iter().position(|entry| &entry.key == key)?;
    let entry = cache.remove(at).unwrap();
    let result = entry.mesh.clone();
    cache.push_back(entry);
    Some(result)
}
pub fn evaluate_modifier_mesh(
    mesh: &Mesh,
    stack: &[MeshModifier],
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    evaluate_prepared(
        mesh,
        stack,
        modifier_key(mesh, stack),
        &BTreeMap::new(),
        cancel,
    )
}
fn evaluate_prepared(
    mesh: &Mesh,
    stack: &[MeshModifier],
    key: ModifierKey,
    frames: &BTreeMap<u64, Vec<crate::Mat4>>,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    cancelled(cancel)?;
    if let Some(result) = cached_key(&key) {
        return result.map_err(|s| error(&s));
    }
    let result = (|| {
        check_modifier_budget(mesh, stack)?;
        mesh.validate_topology()?;
        if !mesh.morph_targets.is_empty() {
            return Err(error("Сначала преобразуйте morph-геометрию в обычный меш"));
        }
        let mut result = mesh.clone();
        let mut groups = vec![0; mesh.vertices.len()];
        for m in stack.iter().filter(|m| m.enabled) {
            cancelled(cancel)?;
            result = match m.settings {
                ModifierSettings::CurveArray { .. } => crate::advanced_modifiers::copies(
                    &result,
                    frames
                        .get(&m.id)
                        .ok_or_else(|| error("Выберите кривую для массива"))?,
                    &mut groups,
                    cancel,
                ),
                _ => crate::modifier_geometry::apply(&result, m.settings, &mut groups, cancel),
            }
            .map_err(|e| error(&format!("{}: {e}", m.name)))?;
        }
        Ok(result)
    })();
    // Cancelled or stale work cannot poison a future request.
    cancelled(cancel)?;
    let bytes = (key.seams.len() + key.wires.len()) * 8
        + key.frames.iter().map(|(_, f)| f.len() * 64).sum::<usize>()
        + result.as_ref().map_or(0, |m| {
            m.vertices.len() * std::mem::size_of::<crate::Vertex>()
                + m.indices.len() * 4
                + m.face_count() * 48
                + m.topology
                    .as_ref()
                    .map_or(0, |t| (t.uv_seams.len() + t.loose_edges.len()) * 8)
        });
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(existing) = cache.iter().find(|e| e.key == key) {
        return existing.mesh.clone().map_err(|s| error(&s));
    }
    while !cache.is_empty()
        && (cache.len() >= 64 || cache.iter().map(|e| e.bytes).sum::<usize>() + bytes > CACHE_BYTES)
    {
        cache.pop_front();
    }
    if bytes <= CACHE_BYTES {
        cache.push_back(CacheEntry {
            key,
            mesh: result
                .as_ref()
                .map(Clone::clone)
                .map_err(ToString::to_string),
            bytes,
        });
    }
    result
}

impl Project {
    /// Resolve path references and animated transforms before handing immutable
    /// geometry to a worker. The key includes every sampled placement frame.
    pub fn prepare_modifiers(
        &self,
        id: NodeId,
        globals: &BTreeMap<NodeId, crate::Mat4>,
    ) -> Result<PreparedModifiers, CoreError> {
        let node = self.node(id).ok_or(CoreError::MissingNode(id))?;
        let mesh = node
            .mesh
            .and_then(|m| self.mesh(m))
            .ok_or_else(|| error("Модификатору нужен меш"))?;
        let mut frames = BTreeMap::new();
        for m in node.modifiers.iter().filter(|m| m.enabled) {
            if let ModifierSettings::CurveArray {
                count,
                curve: Some(target),
                spacing,
                start,
                roll,
                follow,
            } = m.settings
            {
                let curve = self
                    .node(target)
                    .and_then(|n| n.curve.as_ref())
                    .ok_or_else(|| error("Кривая массива отсутствует"))?;
                let owner = *globals.get(&id).ok_or(CoreError::MissingNode(id))?;
                let target = *globals.get(&target).ok_or(CoreError::MissingNode(target))?;
                let path = crate::curve_binding::Path::new(curve, owner.inverse() * target)?;
                let samples = (0..count)
                    .map(|i| {
                        let frame = path.text_frame(start + spacing * i as f32);
                        let placement = if follow {
                            frame
                        } else {
                            crate::Mat4::from_translation(frame.w_axis.truncate())
                        };
                        placement * crate::Mat4::from_rotation_x(roll * i as f32)
                    })
                    .collect::<Vec<_>>();
                frames.insert(m.id, samples);
            }
        }
        let mut key = modifier_key(mesh, &node.modifiers);
        key.frames = frames
            .iter()
            .map(|(id, v)| (*id, v.iter().map(|m| m.to_cols_array()).collect()))
            .collect();
        Ok(PreparedModifiers {
            key,
            mesh: mesh.clone(),
            stack: node.modifiers.clone(),
            frames,
        })
    }
    pub fn modifier_supported(&self, id: NodeId) -> bool {
        self.node(id).is_some_and(|n| {
            n.mesh
                .and_then(|id| self.mesh(id))
                .is_some_and(|m| m.morph_targets.is_empty())
                && n.skin.is_none()
                && n.physics.is_none()
                && n.fluid.is_none()
                && n.destruction.is_none()
                && n.text.is_none()
                && n.curve.is_none()
                && n.fx.is_none()
                && !n.is_joint
        })
    }
    pub(crate) fn validate_modifiers(&self) -> Result<(), CoreError> {
        let mut scene_vertices = 0usize;
        let mut scene_triangles = 0usize;
        for node in &self.nodes {
            if node.modifiers.is_empty() {
                continue;
            }
            if !self.modifier_supported(node.id) {
                return Err(error(
                    "Модификаторы требуют обычный меш без skin, morph, текста, кривой и симуляции",
                ));
            }
            if node.modifiers.len() > MAX_MODIFIERS {
                return Err(error("Не более 32 модификаторов на объект"));
            }
            if node.modifiers.iter().any(|m| m.enabled) {
                let (v, t) =
                    output_budget(self.mesh(node.mesh.unwrap()).unwrap(), &node.modifiers)?;
                scene_vertices = scene_vertices.saturating_add(v);
                scene_triangles = scene_triangles.saturating_add(t);
                if scene_vertices > MAX_MODIFIER_SCENE_VERTICES
                    || scene_triangles > MAX_MODIFIER_SCENE_VERTICES * 2
                {
                    return Err(error("Суммарный результат модификаторов превышает предел сцены: 1 048 576 вершин / 2 097 152 треугольника"));
                }
            }
            let mut ids = BTreeSet::new();
            for m in &node.modifiers {
                if m.id == 0
                    || !ids.insert(m.id)
                    || m.name.trim().is_empty()
                    || m.name.chars().count() > 80
                    || m.name.chars().any(char::is_control)
                {
                    return Err(error("Недопустимое имя или повторный ID модификатора"));
                }
                m.settings.validate()?;
                if let ModifierSettings::CurveArray {
                    curve: Some(target),
                    ..
                } = m.settings
                {
                    if !self.node(target).is_some_and(|n| n.curve.is_some()) {
                        return Err(error("Массив ссылается на отсутствующую кривую"));
                    }
                }
            }
        }
        Ok(())
    }
    pub fn evaluated_modifier_meshes(
        &self,
        cancel: &AtomicBool,
    ) -> Result<BTreeMap<NodeId, Mesh>, CoreError> {
        self.evaluated_modifier_meshes_at(&self.global_transforms()?, cancel)
    }
    pub fn evaluated_modifier_meshes_at(
        &self,
        globals: &BTreeMap<NodeId, crate::Mat4>,
        cancel: &AtomicBool,
    ) -> Result<BTreeMap<NodeId, Mesh>, CoreError> {
        self.nodes
            .iter()
            .filter(|n| n.modifiers.iter().any(|m| m.enabled))
            .map(|n| {
                cancelled(cancel)?;
                Ok((
                    n.id,
                    self.prepare_modifiers(n.id, globals)?.evaluate(cancel)?,
                ))
            })
            .collect()
    }
}

impl Editor {
    pub fn apply_modifiers(&mut self, id: NodeId) -> Result<(), CoreError> {
        let node = self.modifier_owner(id)?;
        if node.modifiers.is_empty() {
            return Ok(());
        }
        let input = self
            .project
            .prepare_modifiers(id, &self.evaluated_base()?.global)?;
        let key = input.key.clone();
        let mesh = input.evaluate(&AtomicBool::new(false))?;
        self.apply_evaluated_modifiers(id, &key, mesh)
    }
    /// A completed preview can be baked even after its global cache entry was
    /// evicted. Verify its complete source key before an atomic instance edit.
    pub fn apply_evaluated_modifiers(
        &mut self,
        id: NodeId,
        expected: &ModifierKey,
        mut mesh: Mesh,
    ) -> Result<(), CoreError> {
        let node = self.modifier_owner(id)?;
        let original = node.mesh.unwrap();
        if &self
            .project
            .prepare_modifiers(id, &self.evaluated_base()?.global)?
            .key
            != expected
        {
            return Err(error(
                "Исходный меш или стек изменился; дождитесь нового расчёта",
            ));
        }
        if node.modifiers.is_empty() {
            return Ok(());
        }
        mesh.id = if self
            .project
            .nodes
            .iter()
            .any(|n| n.id != id && n.mesh == Some(original))
        {
            self.project.next_id()
        } else {
            original
        };
        self.transact(
            "Применить стек модификаторов",
            |p| {
                let next = mesh.id;
                if let Some(slot) = p.meshes.iter_mut().find(|m| m.id == next) {
                    *slot = mesh;
                } else {
                    p.meshes.push(mesh);
                }
                let node = p.node_mut(id).unwrap();
                node.mesh = Some(next);
                node.modifiers.clear();
            },
        )
    }
    fn modifier_owner(&self, id: NodeId) -> Result<&crate::Node, CoreError> {
        if self.has_staged() {
            return Err(CoreError::UncommittedPose);
        }
        if self.project.node_locked(id) {
            return Err(error("Объект или его папка заблокированы"));
        }
        if !self.project.modifier_supported(id) {
            return Err(error(
                "Выберите обычный меш без skin, morph, текста, кривой и симуляции",
            ));
        }
        self.project.node(id).ok_or(CoreError::MissingNode(id))
    }
    fn check_stack(&self, id: NodeId, stack: &[MeshModifier]) -> Result<(), CoreError> {
        let node = self.modifier_owner(id)?;
        if stack.len() > MAX_MODIFIERS {
            return Err(error("Не более 32 модификаторов на объект"));
        }
        check_modifier_budget(self.project.mesh(node.mesh.unwrap()).unwrap(), stack)
    }
    pub fn add_modifier(&mut self, node: NodeId, kind: ModifierKind) -> Result<u64, CoreError> {
        let mut stack = self.modifier_owner(node)?.modifiers.clone();
        let id = next_id(&stack)?;
        stack.push(MeshModifier::new(id, kind));
        self.check_stack(node, &stack)?;
        self.transact("Добавить модификатор", |p| {
            p.node_mut(node).unwrap().modifiers = stack
        })?;
        Ok(id)
    }
    pub fn update_modifier(
        &mut self,
        node: NodeId,
        modifier: MeshModifier,
    ) -> Result<(), CoreError> {
        let mut stack = self.modifier_owner(node)?.modifiers.clone();
        let index = stack
            .iter()
            .position(|m| m.id == modifier.id)
            .ok_or_else(|| error("Модификатор удалён"))?;
        stack[index] = modifier;
        self.check_stack(node, &stack)?;
        self.transact("Изменить модификатор", |p| {
            p.node_mut(node).unwrap().modifiers = stack
        })
    }
    pub fn remove_modifier(&mut self, node: NodeId, id: u64) -> Result<(), CoreError> {
        self.modifier_owner(node)?;
        self.transact("Удалить модификатор", |p| {
            p.node_mut(node).unwrap().modifiers.retain(|m| m.id != id)
        })
    }
    /// `before` identifies a stable destination row; None appends. Move is atomic across owners.
    pub fn transfer_modifier(
        &mut self,
        source: NodeId,
        id: u64,
        target: NodeId,
        before: Option<u64>,
        copy: bool,
    ) -> Result<u64, CoreError> {
        let original = self.modifier_owner(source)?.modifiers.clone();
        let mut from = original.clone();
        let index = from
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| error("Модификатор удалён"))?;
        let mut modifier = from[index].clone();
        let mut into = self.modifier_owner(target)?.modifiers.clone();
        if !copy && source == target && before == Some(id) {
            return Ok(id);
        }
        if copy || source != target {
            modifier.id = next_id(&into)?;
        }
        if !copy {
            from.remove(index);
        }
        if source == target {
            into = from.clone();
        }
        let at = match before {
            Some(id) => into
                .iter()
                .position(|m| m.id == id)
                .ok_or_else(|| error("Место вставки изменилось"))?,
            None => into.len(),
        };
        let inserted = modifier.id;
        into.insert(at, modifier);
        self.check_stack(target, &into)?;
        if source != target && !copy {
            self.check_stack(source, &from)?;
        }
        self.transact(
            if copy {
                "Копировать модификатор"
            } else {
                "Переместить модификатор"
            },
            |p| {
                if source != target && !copy {
                    p.node_mut(source).unwrap().modifiers = from;
                }
                p.node_mut(target).unwrap().modifiers = into;
            },
        )?;
        Ok(inserted)
    }
}
fn next_id(stack: &[MeshModifier]) -> Result<u64, CoreError> {
    stack
        .iter()
        .map(|m| m.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| error("Исчерпаны ID модификаторов"))
}
