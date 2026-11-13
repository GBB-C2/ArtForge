//! Editable shape targets, instance isolation and animation channel maintenance.
use crate::{Channel, Clip, CoreError, Editor, Interpolation, Keyframe, Mesh, MorphTarget, Node, Property, Vec3};
fn invalid(message: &str) -> CoreError { CoreError::Invalid(message.into()) }

pub fn morph_compatible(base: &Mesh, target: &Mesh) -> bool {
    base.vertices.len() == target.vertices.len() && base.indices == target.indices
        && match (&base.topology, &target.topology) { (Some(a), Some(b)) => a.faces == b.faces, _ => true }
}
impl Editor {
    fn morph_mesh(&self, node: u64) -> Result<&Mesh, CoreError> {
        self.authoring_unlocked(node)?;
        if self.project.node_locked(node) { return Err(invalid("Объект заблокирован")); }
        let n = self.project.node(node).ok_or(CoreError::MissingNode(node))?;
        if n.fluid.is_some() || n.physics.is_some() || n.curve.is_some() || n.destruction.is_some() {
            return Err(invalid("Для морфов нужен обычный меш без активной физики или кривой"));
        }
        n.mesh.and_then(|id| self.project.mesh(id)).ok_or_else(|| invalid("Выберите меш"))
    }
    fn commit_morph_mesh(&mut self, node: u64, mut mesh: Mesh, label: &str,
        change: impl FnOnce(&mut crate::Project)) -> Result<(), CoreError> {
        if self.project.nodes.iter().filter(|n| n.mesh == Some(mesh.id)).count() > 1 { mesh.id = self.project.next_id(); }
        if let Some(topology) = &mut mesh.topology { topology.primitive = None; }
        self.transact(label, move |p| {
            p.node_mut(node).unwrap().mesh = Some(mesh.id);
            if let Some(old) = p.meshes.iter_mut().find(|m| m.id == mesh.id) { *old = mesh; } else { p.meshes.push(mesh); }
            change(p);
        })
    }
    pub fn create_morph(&mut self, node: u64) -> Result<usize, CoreError> {
        let mut mesh = self.morph_mesh(node)?.clone();
        let index = mesh.morph_targets.len();
        if index >= 64 || mesh.vertices.len() > 65536 { return Err(invalid("Создание морфов: до 64 форм и 65536 вершин")); }
        mesh.morph_targets.push(MorphTarget { name: format!("Форма {}", index + 1),
            positions: vec![[0.; 3]; mesh.vertices.len()].into(), normals: Default::default() });
        self.commit_morph_mesh(node, mesh, "Создать морф", |p| resize_weights(p, node, index + 1, None))?;
        Ok(index)
    }
    pub fn capture_morph(&mut self, node: u64, target: u64, replace: Option<usize>) -> Result<usize, CoreError> {
        if node == target { return Err(invalid("Выберите другой меш как форму")); }
        let mut mesh = self.morph_mesh(node)?.clone();
        let target_node = self.project.node(target).ok_or(CoreError::MissingNode(target))?;
        let other = target_node.mesh.and_then(|id| self.project.mesh(id)).ok_or_else(|| invalid("У формы нет меша"))?;
        if !morph_compatible(&mesh, other) { return Err(invalid("Форма должна сохранять число и порядок вершин, рёбер и граней исходного меша")); }
        if !other.morph_targets.is_empty() || target_node.skin.is_some() || target_node.fluid.is_some() || target_node.physics.is_some() || target_node.curve.is_some() || target_node.destruction.is_some() {
            return Err(invalid("Сначала создайте обычную копию формы без скелета и других деформаций"));
        }
        let count = mesh.morph_targets.len();
        let index = replace.unwrap_or(count);
        if index > count || (replace.is_some() && index == count) || (replace.is_none() && count >= 64) || mesh.vertices.len() > 65536 {
            return Err(invalid("Некорректный индекс или превышен предел морфов"));
        }
        let target = MorphTarget {
            name: replace.map_or_else(|| target_node.name.clone(), |i| mesh.morph_targets[i].name.clone()),
            positions: mesh.vertices.iter().zip(&other.vertices).map(|(a,b)| (Vec3::from_array(b.position)-Vec3::from_array(a.position)).to_array()).collect(),
            normals: mesh.vertices.iter().zip(&other.vertices).map(|(a,b)| (Vec3::from_array(b.normal)-Vec3::from_array(a.normal)).to_array()).collect(),
        };
        if index == count { mesh.morph_targets.push(target); } else { mesh.morph_targets[index] = target; }
        let count = mesh.morph_targets.len();
        self.commit_morph_mesh(node, mesh, "Записать форму морфа", |p| resize_weights(p, node, count, None))?;
        Ok(index)
    }
    pub fn rename_morph(&mut self, node: u64, index: usize, name: &str) -> Result<(), CoreError> {
        let name = name.trim(); if name.is_empty() || name.chars().count() > 80 { return Err(invalid("Имя формы: 1–80 символов")); }
        let mut mesh = self.morph_mesh(node)?.clone();
        mesh.morph_targets.get_mut(index).ok_or_else(|| invalid("Форма удалена"))?.name = name.into();
        self.commit_morph_mesh(node, mesh, "Переименовать морф", |_| {})
    }
    pub fn delete_morph(&mut self, node: u64, index: usize) -> Result<(), CoreError> {
        let mut mesh = self.morph_mesh(node)?.clone();
        if index >= mesh.morph_targets.len() { return Err(invalid("Форма удалена")); }
        mesh.morph_targets.remove(index); let count = mesh.morph_targets.len();
        self.commit_morph_mesh(node, mesh, "Удалить морф", |p| resize_weights(p, node, count, Some(index)))
    }
    pub fn set_morph_weight(&mut self, node: u64, index: usize, value: f32) -> Result<(), CoreError> {
        self.change_morph_weight(node, index, value)
    }
    /// A regular, unskinned copy can be edited with the existing vertex tools.
    /// Capture reads local vertices; moving this object aside does not enter the delta.
    pub fn copy_morph_shape(&mut self, node: u64, index: Option<usize>) -> Result<u64, CoreError> {
        let mut mesh = self.morph_mesh(node)?.clone();
        if mesh.vertices.len() > 65536 { return Err(invalid("Редактируемая форма: до 65536 вершин")); }
        let source = self.project.node(node).unwrap().clone();
        let weights = if let Some(index) = index {
            if index >= mesh.morph_targets.len() { return Err(invalid("Форма удалена")); }
            (0..mesh.morph_targets.len()).map(|i| if i == index { 1. } else { 0. }).collect::<Vec<_>>()
        } else { self.evaluated()?.morph_weights.get(&node).cloned().unwrap_or(source.morph_weights.clone()) };
        for (i, vertex) in mesh.vertices.iter_mut().enumerate() {
            for (target, weight) in mesh.morph_targets.iter().zip(&weights) {
                vertex.position = (Vec3::from_array(vertex.position) + Vec3::from_array(target.positions[i]) * *weight).to_array();
                if let Some(delta) = target.normals.get(i) { vertex.normal = (Vec3::from_array(vertex.normal) + Vec3::from_array(*delta) * *weight).to_array(); }
            }
            vertex.normal = Vec3::from_array(vertex.normal).try_normalize().unwrap_or(Vec3::Y).to_array();
        }
        mesh.morph_targets.clear(); if let Some(t) = &mut mesh.topology { t.primitive = None; }
        let min = mesh.vertices.iter().map(|v| v.position[0]).fold(f32::INFINITY, f32::min);
        let max = mesh.vertices.iter().map(|v| v.position[0]).fold(f32::NEG_INFINITY, f32::max);
        mesh.id = self.project.next_id(); mesh.name = format!("{} · форма", source.name);
        let id = mesh.id + 1; let mut copy = Node::new(id, &mesh.name);
        copy.mesh = Some(mesh.id); copy.parent = source.parent; copy.transform = source.transform;
        copy.transform.translation.x += (max - min).max(0.1) * source.transform.scale.x.abs().max(0.1) * 1.3;
        copy.material_layers = source.material_layers.clone();
        self.transact("Копия для морфа", |p| { p.meshes.push(mesh); p.nodes.push(copy); })?;
        Ok(id)
    }
}
pub(crate) fn key_weights(clip: &mut Clip, node: u64, time: f64, weights: Vec<f32>) {
    let count = weights.len(); clip.duration = clip.duration.max(time);
    let channel = if let Some(i) = clip.channels.iter().position(|c| c.node == node && c.property == Property::Weights) { &mut clip.channels[i] }
        else { clip.channels.push(Channel { node, property: Property::Weights, interpolation: Interpolation::Linear, keys: vec![] }); clip.channels.last_mut().unwrap() };
    if let Some(key) = channel.keys.iter_mut().find(|k| (k.time-time).abs() < 1e-6) { key.value = weights; }
    else {
        let mut key = Keyframe::new(time, weights);
        if channel.interpolation == Interpolation::CubicSpline { key.in_tangent = Some(vec![0.;count]); key.out_tangent = Some(vec![0.;count]); }
        channel.keys.push(key); channel.keys.sort_by(|a,b| a.time.total_cmp(&b.time));
    }
}

fn resize_weights(project: &mut crate::Project, node: u64, count: usize, remove: Option<usize>) {
    let edit = |values: &mut Vec<f32>| { if let Some(i) = remove { if i < values.len() { values.remove(i); } } values.resize(count, 0.); };
    edit(&mut project.node_mut(node).unwrap().morph_weights);
    for clip in &mut project.clips {
        if count == 0 { clip.channels.retain(|c| !(c.node == node && c.property == Property::Weights)); }
        for channel in clip.channels.iter_mut().filter(|c| c.node == node && c.property == Property::Weights) {
            for key in &mut channel.keys {
                edit(&mut key.value);
                if let Some(v) = &mut key.in_tangent { edit(v); }
                if let Some(v) = &mut key.out_tangent { edit(v); }
            }
        }
    }
}
