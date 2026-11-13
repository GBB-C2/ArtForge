//! Per-receiver collision membership; None preserves legacy scene colliders.
use crate::{CoreError, NodeId, PhysicsKind, Project};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionKind { Fluid, Destruction, Cloth, Fx }
impl CollisionKind {pub fn limit(self)->usize {if self==Self::Fx {8}else{64}}}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollisionTarget {
    pub node: NodeId,
    #[serde(default = "enabled")]
    pub enabled: bool,
}
fn enabled() -> bool { true }

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CollisionTargets {
    pub fluid: Option<Vec<CollisionTarget>>,
    pub destruction: Option<Vec<CollisionTarget>>,
    pub cloth: Option<Vec<CollisionTarget>>,
    pub fx: Option<Vec<CollisionTarget>>,
}
impl CollisionTargets {
    pub fn is_legacy(&self) -> bool { self.fluid.is_none() && self.destruction.is_none() && self.cloth.is_none() && self.fx.is_none() }
    pub fn get(&self, kind: CollisionKind) -> Option<&[CollisionTarget]> {
        match kind { CollisionKind::Fluid => self.fluid.as_deref(), CollisionKind::Destruction => self.destruction.as_deref(), CollisionKind::Cloth => self.cloth.as_deref(), CollisionKind::Fx=>self.fx.as_deref() }
    }
    pub fn set(&mut self, kind: CollisionKind, entries: Vec<CollisionTarget>) {
        let list = match kind { CollisionKind::Fluid => &mut self.fluid, CollisionKind::Destruction => &mut self.destruction, CollisionKind::Cloth => &mut self.cloth, CollisionKind::Fx=>&mut self.fx };
        *list = Some(entries);
    }
    pub fn remap(&mut self, ids: &BTreeMap<NodeId,NodeId>) {
        for list in [&mut self.fluid,&mut self.destruction,&mut self.cloth,&mut self.fx].into_iter().flatten() {
            for entry in list { entry.node = ids.get(&entry.node).copied().unwrap_or(entry.node); }
        }
    }
    pub fn remove(&mut self, ids: &BTreeSet<NodeId>) {
        for list in [&mut self.fluid,&mut self.destruction,&mut self.cloth,&mut self.fx].into_iter().flatten() {
            list.retain(|entry| !ids.contains(&entry.node));
        }
    }
}
impl Project {
    pub fn collision_candidate_ids(&self, owner: NodeId) -> BTreeSet<NodeId> {
        let Some(node) = self.node(owner) else { return BTreeSet::new(); };
        let excluded = node.destruction.and_then(|d| d.result).map_or_else(BTreeSet::new,
            |root| self.descendants_of(&BTreeSet::from([root])));
        self.nodes.iter().filter(|n| n.id != owner && n.mesh.is_some() && !n.is_joint && n.camera.is_none()
            && node.fx.and_then(|f|f.source)!=Some(n.id) && !excluded.contains(&n.id)).map(|n| n.id).collect()
    }
    pub fn collision_target_allowed(&self, owner: NodeId, target: NodeId) -> bool {
        if owner == target || self.node(target).is_none_or(|n| n.mesh.is_none() || n.is_joint || n.camera.is_some()) { return false; }
        let Some(node) = self.node(owner) else { return false; };
        if node.fx.and_then(|f|f.source)==Some(target) {return false;}
        if let Some(result) = node.destruction.and_then(|d| d.result) {
            let mut cursor = Some(target);
            let mut seen = BTreeSet::new();
            while let Some(id) = cursor.filter(|id| seen.insert(*id)) {
                if id == result { return false; }
                cursor = self.node(id).and_then(|n| n.parent);
            }
        }
        true
    }
    pub fn resolved_collision_targets(&self, owner: NodeId, kind: CollisionKind) -> Vec<CollisionTarget> {
        let Some(node) = self.node(owner) else { return Vec::new(); };
        if let Some(entries) = node.collision_targets.get(kind) { return entries.to_vec(); }
        if kind==CollisionKind::Fx {return Vec::new();}
        let candidates = self.collision_candidate_ids(owner);
        self.nodes.iter().filter(|n| n.physics.is_some_and(|p| p.kind == PhysicsKind::Collider) && candidates.contains(&n.id))
            .map(|n| CollisionTarget { node:n.id, enabled:n.physics.unwrap().visible }).collect()
    }
    pub(crate) fn validate_collision_targets(&self) -> Result<(),CoreError> {
        for node in &self.nodes {
            if node.collision_targets.is_legacy() { continue; }
            let candidates = self.collision_candidate_ids(node.id);
            for kind in [CollisionKind::Fluid,CollisionKind::Destruction,CollisionKind::Cloth,CollisionKind::Fx] {
                let Some(entries)=node.collision_targets.get(kind) else{continue;};
                if entries.len() > kind.limit() { return Err(CoreError::Invalid(format!("Столкновения: не более {} мешей в списке",kind.limit()))); }
                let mut seen = BTreeSet::new();
                for entry in entries {
                    if !seen.insert(entry.node) || !candidates.contains(&entry.node) {
                        return Err(CoreError::Invalid("Столкновения: повтор, отсутствующий меш или ссылка на собственную геометрию".into()));
                    }
                }
            }
        }
        Ok(())
    }
}
