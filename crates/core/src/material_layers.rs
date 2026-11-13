//! Instance-owned surface overlays; library membership never changes assignments.
use crate::{CoreError, Editor, Material, NodeId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialLayer {
    pub material: u64,
    #[serde(default = "visible")]
    pub visible: bool,
}
fn visible() -> bool { true }

impl Editor {
    /// Copy a library surface or create a neutral default. Each new layer has
    /// independent settings, while its texture resources remain shared.
    pub fn add_material_layer(&mut self, node: NodeId, source: Option<u64>) -> Result<u64, CoreError> {
        let material = match source {
            Some(id) => self.project.material(id).cloned().ok_or_else(|| CoreError::Invalid("Материал удалён".into()))?,
            None => Material::default(),
        };
        self.insert_material_layer(node, material)
    }
    pub fn add_material_layer_preset(&mut self, node: NodeId, preset: crate::MaterialPreset) -> Result<u64, CoreError> {
        self.insert_material_layer(node, preset.create())
    }
    fn insert_material_layer(&mut self, node: NodeId, mut material: Material) -> Result<u64, CoreError> {
        let owner = self.project.node(node).ok_or_else(|| CoreError::Invalid("Выберите меш".into()))?;
        if owner.mesh.is_none() || self.project.node_locked(node) || owner.material_layers.len() >= 64 {
            return Err(CoreError::Invalid("Нужен незаблокированный меш; максимум 64 слоя".into()));
        }
        material.id = self.project.next_id();
        material.in_library = false;
        material.visible = true;
        let base = material.name.chars().take(110).collect::<String>();
        let mut name = base.clone();
        let mut suffix = 2;
        while self.project.materials.iter().any(|m| m.name == name) {
            name = format!("{base} {suffix}"); suffix += 1;
        }
        material.name = name;
        let id = material.id;
        self.transact("Добавить слой материала", |p| {
            p.materials.push(material);
            p.node_mut(node).unwrap().material_layers.push(MaterialLayer { material: id, visible: true });
        })?;
        Ok(id)
    }

    pub fn remove_material_layer(&mut self, node: NodeId, material: u64) -> Result<(), CoreError> {
        if self.project.node_locked(node) { return Err(CoreError::Invalid("Объект заблокирован".into())); }
        self.transact("Удалить слой материала", |p| {
            if let Some(n) = p.node_mut(node) { n.material_layers.retain(|l| l.material != material); }
        })
    }

    pub fn set_material_library(&mut self, material: u64, present: bool) -> Result<(), CoreError> {
        if self.project.material(material).is_none() { return Err(CoreError::Invalid("Материал удалён".into())); }
        self.transact(if present { "Добавить материал в библиотеку" } else { "Убрать материал из библиотеки" }, |p| {
            p.materials.iter_mut().find(|m| m.id == material).unwrap().in_library = present;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{box_mesh, Node, Project, Vec3};
    #[test]
    fn layers_are_local_library_is_explicit_and_undo_is_atomic() {
        let mut a = Node::new(1, "A"); a.mesh = Some(2);
        let mut b = a.clone(); b.id = 3;
        let project = Project { nodes: vec![a,b], meshes: vec![box_mesh(2, "Mesh", Vec3::ONE, None)], ..Default::default() };
        let mut e = Editor::new(project.clone());
        let bottom = e.add_material_layer(1, None).unwrap();
        assert!(!e.project.material(bottom).unwrap().in_library);
        let top = e.add_material_layer(1, Some(bottom)).unwrap();
        assert_ne!(bottom, top);
        assert!(e.project.node(3).unwrap().material_layers.is_empty());
        assert_eq!(e.project.node(1).unwrap().material_layers.len(), 2);
        e.set_material_library(top, true).unwrap();
        e.set_material_library(top, false).unwrap();
        assert_eq!(e.project.node(1).unwrap().material_layers[1].material, top);
        e.remove_unused_materials().unwrap();
        assert!(e.project.material(top).is_some());
        e.remove_material_layer(1, top).unwrap();
        assert_eq!(e.project.node(1).unwrap().material_layers.len(), 1);
        e.undo(); assert_eq!(e.project.node(1).unwrap().material_layers.len(), 2);
        e.project.validate().unwrap();
    }
}
