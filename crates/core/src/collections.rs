//! Collections organize visibility without changing transform or skin hierarchies.
use crate::{CoreError, NodeId, Project};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Collection {
    pub id: u64,
    pub name: String,
    pub parent: Option<u64>,
    pub members: Vec<NodeId>,
    pub light_members: Vec<usize>,
    pub visible: bool,
    pub render_visible: bool,
    pub locked: bool,
}
impl Default for Collection {
    fn default() -> Self {
        Self {
            id: 0,
            name: "Коллекция".into(),
            parent: None,
            members: vec![],
            light_members: vec![],
            visible: true,
            render_visible: true,
            locked: false,
        }
    }
}
impl Project {
    pub fn collection_chain(&self, id: u64) -> Vec<u64> {
        let mut result = Vec::new();
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            if result.contains(&id) { break; }
            let Some(folder) = self.collections.iter().find(|c| c.id == id) else { break; };
            result.push(id); cursor = folder.parent;
        }
        result
    }
    pub fn collection_visible(&self, id: u64, export: bool) -> bool {
        self.collection_chain(id).iter().all(|id| self.collections.iter().find(|c| c.id == *id)
            .is_some_and(|c| if export { c.render_visible } else { c.visible }))
    }
    pub fn collection_locked(&self, id: u64) -> bool {
        self.collections.iter().all(|c| c.id != id) || self.collection_chain(id).iter()
            .any(|id| self.collections.iter().find(|c| c.id == *id).is_some_and(|c| c.locked))
    }
    pub fn remove_collection(&mut self, id: u64) {
        let Some(folder) = self.collections.iter().find(|c| c.id == id).cloned() else { return; };
        self.collections.retain(|c| c.id != id);
        for child in &mut self.collections {
            if child.parent == Some(id) { child.parent = folder.parent; }
            if Some(child.id) == folder.parent {
                child.members.extend(folder.members.iter().copied());
                child.light_members.extend(folder.light_members.iter().copied());
            }
        }
    }
    pub fn node_collection(&self, id: NodeId) -> Option<u64> {
        let mut cursor = Some(id);
        let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return None;
            }
            if let Some(c) = self.collections.iter().find(|c| c.members.contains(&id)) {
                return Some(c.id);
            }
            cursor = self.node(id).and_then(|n| n.parent);
        }
        None
    }
    pub fn node_visible(&self, id: NodeId, export: bool) -> bool {
        self.node_access(id, export, false)
    }
    pub fn node_selectable(&self, id: NodeId) -> bool {
        self.node_access(id, false, true)
    }
    fn node_access(&self, id: NodeId, export: bool, check_lock: bool) -> bool {
        let mut current = Some(id);
        let mut visited = BTreeSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return false;
            }
            let Some(node) = self.node(id) else {
                return false;
            };
            if !(if export {
                node.render_visible
            } else {
                node.visible
            }) || check_lock && node.locked
            {
                return false;
            }
            if self
                .collections
                .iter()
                .filter(|c| c.members.contains(&id))
                .any(|c| {
                    !self.collection_visible(c.id, export) || check_lock && self.collection_locked(c.id)
                })
            {
                return false;
            }
            current = node.parent;
        }
        true
    }
    pub fn selection_roots(&self, ids: &BTreeSet<NodeId>) -> Vec<NodeId> {
        ids.iter()
            .copied()
            .filter(|id| {
                let mut current = self.node(*id).and_then(|n| n.parent);
                let mut visited = BTreeSet::new();
                while let Some(parent) = current {
                    if ids.contains(&parent) || !visited.insert(parent) {
                        return false;
                    }
                    current = self.node(parent).and_then(|n| n.parent);
                }
                true
            })
            .collect()
    }
    pub(crate) fn validate_collections(&self) -> Result<(), CoreError> {
        if self.collections.len() > 256 {
            return Err(CoreError::Invalid("не более 256 коллекций".into()));
        }
        let mut ids = BTreeSet::new();
        let mut assigned = BTreeSet::new();
        let mut assigned_lights = BTreeSet::new();
        for c in &self.collections {
            let mut seen = BTreeSet::from([c.id]);
            let mut parent = c.parent;
            while let Some(id) = parent {
                if !seen.insert(id) { return Err(CoreError::Invalid("Цикл вложенных папок".into())); }
                parent = self.collections.iter().find(|folder| folder.id == id)
                    .ok_or_else(|| CoreError::Invalid("Родительская папка отсутствует".into()))?.parent;
            }
            if c.light_members
                .iter()
                .any(|i| *i >= self.lights.len() || !assigned_lights.insert(*i))
            {
                return Err(CoreError::Invalid(
                    "источник света папки отсутствует или указан дважды".into(),
                ));
            }
            if !ids.insert(c.id)
                || c.name.trim().is_empty()
                || c.name.chars().count() > 64
                || c.name.chars().any(char::is_control)
            {
                return Err(CoreError::Invalid("имя или ID коллекции".into()));
            }
            for id in &c.members {
                if self.node(*id).is_none() || !assigned.insert(*id) {
                    return Err(CoreError::Invalid(
                        "объект коллекции отсутствует или находится в двух коллекциях".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}
impl Project {
    /// Resolve a whole hierarchy once, avoiding one linear node search per draw.
    pub fn visible_node_ids(&self, export: bool) -> BTreeSet<NodeId> {
        self.accessible_nodes(export, false)
    }
    pub fn selectable_node_ids(&self) -> BTreeSet<NodeId> {
        self.accessible_nodes(false, true)
    }
    fn accessible_nodes(&self, export: bool, lock: bool) -> BTreeSet<NodeId> {
        use std::collections::BTreeMap;
        let nodes: BTreeMap<_, _> = self.nodes.iter().map(|n| (n.id, n)).collect();
        let blocked: BTreeSet<_> = self
            .collections
            .iter()
            .filter(|c| !self.collection_visible(c.id, export) || lock && self.collection_locked(c.id))
            .flat_map(|c| c.members.iter().copied())
            .collect();
        let mut result = BTreeMap::<u64, bool>::new();
        for n in &self.nodes {
            let mut path = vec![];
            let mut cursor = Some(n.id);
            let mut seen = BTreeSet::new();
            let mut visible = true;
            while let Some(id) = cursor {
                if let Some(value) = result.get(&id) {
                    visible = *value;
                    break;
                }
                if !seen.insert(id) {
                    visible = false;
                    break;
                }
                path.push(id);
                let Some(node) = nodes.get(&id) else {
                    visible = false;
                    break;
                };
                if !(if export {
                    node.render_visible
                } else {
                    node.visible
                }) || lock && node.locked
                    || blocked.contains(&id)
                {
                    visible = false;
                    break;
                }
                cursor = node.parent;
            }
            for id in path {
                result.insert(id, visible);
            }
        }
        result
            .into_iter()
            .filter_map(|(id, visible)| visible.then_some(id))
            .collect()
    }
    pub fn collection_membership(&self) -> std::collections::BTreeMap<NodeId, u64> {
        use std::collections::BTreeMap;
        let nodes: BTreeMap<_, _> = self.nodes.iter().map(|n| (n.id, n.parent)).collect();
        let assigned: BTreeMap<_, _> = self
            .collections
            .iter()
            .flat_map(|c| c.members.iter().map(|id| (*id, c.id)))
            .collect();
        let mut resolved = BTreeMap::<u64, Option<u64>>::new();
        for n in &self.nodes {
            let mut path = vec![];
            let mut cursor = Some(n.id);
            let mut value = None;
            let mut seen = BTreeSet::new();
            while let Some(id) = cursor {
                if let Some(found) = resolved.get(&id) {
                    value = *found;
                    break;
                }
                if let Some(found) = assigned.get(&id) {
                    value = Some(*found);
                    path.push(id);
                    break;
                }
                if !seen.insert(id) {
                    break;
                }
                path.push(id);
                cursor = nodes.get(&id).copied().flatten();
            }
            for id in path {
                resolved.insert(id, value);
            }
        }
        resolved
            .into_iter()
            .filter_map(|(id, c)| c.map(|c| (id, c)))
            .collect()
    }
    pub fn descendants_of(&self, roots: &BTreeSet<NodeId>) -> BTreeSet<NodeId> {
        let parents: std::collections::BTreeMap<_, _> =
            self.nodes.iter().map(|n| (n.id, n.parent)).collect();
        let mut cache: std::collections::BTreeMap<u64, bool> =
            roots.iter().map(|id| (*id, true)).collect();
        for n in &self.nodes {
            let mut path = vec![];
            let mut cursor = Some(n.id);
            let mut seen = BTreeSet::new();
            let mut included = false;
            while let Some(id) = cursor {
                if let Some(value) = cache.get(&id) {
                    included = *value;
                    break;
                }
                if !seen.insert(id) {
                    break;
                }
                path.push(id);
                cursor = parents.get(&id).copied().flatten();
            }
            for id in path {
                cache.insert(id, included);
            }
        }
        cache
            .into_iter()
            .filter_map(|(id, v)| v.then_some(id))
            .collect()
    }
}
impl Project {
    pub fn node_locked(&self, id: NodeId) -> bool {
        let mut cursor = Some(id);
        let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return true;
            }
            let Some(n) = self.node(id) else {
                return true;
            };
            if n.locked
                || self
                    .collections
                    .iter()
                    .any(|c| c.members.contains(&id) && self.collection_locked(c.id))
            {
                return true;
            }
            cursor = n.parent;
        }
        false
    }
}
