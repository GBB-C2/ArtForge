//! Saved presentation order. Organizing the outliner never reparents a rig.
use crate::{AssetId, NodeId, Project};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SceneItem {
    Node(NodeId),
    Light(usize),
    Folder(u64),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Outliner {
    pub order: Vec<SceneItem>,
    /// Per-instance presentation order; it does not change polygon assignments.
    pub materials: BTreeMap<NodeId, Vec<AssetId>>,
}

impl Project {
    pub fn clean_outliner(&mut self) {
        if self.outliner.order.is_empty() && self.outliner.materials.is_empty() {
            return;
        }
        let nodes: BTreeSet<_> = self.nodes.iter().map(|n| n.id).collect();
        let materials: BTreeSet<_> = self.materials.iter().map(|m| m.id).collect();
        let folders: BTreeSet<_> = self.collections.iter().map(|c| c.id).collect();
        let mut seen = BTreeSet::new();
        self.outliner.order.retain(|item| {
            let exists = match item {
                SceneItem::Node(id) => nodes.contains(id),
                SceneItem::Light(index) => *index < self.lights.len(),
                SceneItem::Folder(id) => folders.contains(id),
            };
            exists && seen.insert(*item)
        });
        self.outliner.materials.retain(|id, order| {
            let mut seen = BTreeSet::new();
            order.retain(|m| materials.contains(m) && seen.insert(*m));
            nodes.contains(id) && !order.is_empty()
        });
    }

    pub fn light_collection(&self, index: usize) -> Option<u64> {
        self.collections
            .iter()
            .find(|c| c.light_members.contains(&index))
            .map(|c| c.id)
    }

    pub fn light_visible(&self, index: usize, export: bool) -> bool {
        self.lights.get(index).is_some_and(|l| l.enabled)
            && self
                .collections
                .iter()
                .filter(|c| c.light_members.contains(&index))
                .all(|c| self.collection_visible(c.id, export))
    }

    pub fn light_locked(&self, index: usize) -> bool {
        index >= self.lights.len()
            || self
                .collections
                .iter()
                .any(|c| c.light_members.contains(&index) && self.collection_locked(c.id))
    }

    /// Array-backed light references must move with their light after deletion.
    pub fn remove_light(&mut self, index: usize) {
        if index >= self.lights.len() {
            return;
        }
        self.lights.remove(index);
        for c in &mut self.collections {
            c.light_members.retain(|i| *i != index);
            for i in &mut c.light_members {
                if *i > index {
                    *i -= 1;
                }
            }
        }
        self.outliner
            .order
            .retain(|item| *item != SceneItem::Light(index));
        for item in &mut self.outliner.order {
            if let SceneItem::Light(i) = item {
                if *i > index {
                    *i -= 1;
                }
            }
        }
    }

    pub fn clear_light_organization(&mut self) {
        for c in &mut self.collections {
            c.light_members.clear();
        }
        self.outliner
            .order
            .retain(|item| !matches!(item, SceneItem::Light(_)));
    }
}
