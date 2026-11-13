use poseforge_core::*;
use std::collections::BTreeSet;

fn scene() -> Project {
    let mut p=Project::default();
    p.meshes.push(box_mesh(10,"Cube",Vec3::splat(0.3),None));
    let group=Node::new(100,"Group");
    for id in 1..=3 {
        let mut n=Node::new(id,format!("Mesh {id}"));n.mesh=Some(10);n.parent=Some(100);
        if id==2 {n.physics=Some(PhysicsSettings {kind:PhysicsKind::Collider,..Default::default()});}
        p.nodes.push(n);
    }
    p.nodes.push(group);Editor::new(p).project
}
#[test]
fn explicit_empty_and_disabled_entries_override_legacy_membership_independently() {
    let mut p=scene();
    assert_eq!(p.resolved_collision_targets(1,CollisionKind::Fluid),vec![CollisionTarget {node:2,enabled:true}]);
    p.node_mut(1).unwrap().collision_targets.set(CollisionKind::Fluid,vec![]);
    assert!(p.resolved_collision_targets(1,CollisionKind::Fluid).is_empty());
    assert_eq!(p.resolved_collision_targets(1,CollisionKind::Destruction).len(),1);
    p.node_mut(1).unwrap().collision_targets.set(CollisionKind::Fluid,vec![CollisionTarget {node:3,enabled:false}]);
    p.validate().unwrap();
    assert_eq!(p.resolved_collision_targets(1,CollisionKind::Fluid),vec![CollisionTarget {node:3,enabled:false}]);
    assert!(p.node(3).unwrap().visible);
}
#[test]
fn deleting_obstacle_removes_references_in_the_same_undo_step() {
    let mut p=scene();
    p.node_mut(1).unwrap().collision_targets.fluid=Some(vec![CollisionTarget {node:2,enabled:true}]);
    let mut e=Editor::new(p.clone());
    e.delete_node(2).unwrap();
    assert_eq!(e.project.node(1).unwrap().collision_targets.fluid,Some(vec![]));
    assert_eq!(e.undo_len(),1);assert!(e.undo());assert_eq!(e.project,p);
    assert!(e.redo());assert!(e.project.node(2).is_none());
}
#[test]
fn removing_geometry_detaches_references_but_undo_restores_them() {
    let mut p=scene();p.node_mut(1).unwrap().collision_targets.destruction=Some(vec![CollisionTarget {node:3,enabled:true}]);
    let mut e=Editor::new(p.clone());
    e.transact("Remove geometry",|p|p.node_mut(3).unwrap().mesh=None).unwrap();
    assert!(e.project.node(1).unwrap().collision_targets.destruction.as_ref().unwrap().is_empty());
    assert!(e.undo());assert_eq!(e.project,p);
}
#[test]
fn subtree_duplication_remaps_internal_collision_references() {
    let mut p=scene();
    p.node_mut(1).unwrap().collision_targets.fluid=Some(vec![CollisionTarget {node:2,enabled:false},CollisionTarget {node:3,enabled:true}]);
    let root=p.duplicate_subtree(100).unwrap();
    p.validate().unwrap();
    let receiver=p.nodes.iter().find(|n|n.parent==Some(root)&&n.name.starts_with("Mesh 1")).unwrap();
    let targets=receiver.collision_targets.fluid.as_ref().unwrap();
    assert_eq!(targets.len(),2);
    for entry in targets {assert!(entry.node>100);assert_eq!(p.node(entry.node).unwrap().parent,Some(root));}
    assert!(!targets[0].enabled);assert!(targets[1].enabled);
    assert_eq!(p.node(1).unwrap().collision_targets.fluid.as_ref().unwrap()[0].node,2);
}
#[test]
fn invalid_references_are_rejected_atomically_and_own_shards_are_not_candidates() {
    let p=scene();let mut e=Editor::new(p.clone());
    for ids in [vec![1],vec![999],vec![2,2]] {
        assert!(e.transact("Invalid",|p|p.node_mut(1).unwrap().collision_targets.fluid=Some(ids.into_iter().map(|node|CollisionTarget {node,enabled:true}).collect())).is_err());
        assert_eq!(e.project,p);assert_eq!(e.undo_len(),0);
    }
    e.project.node_mut(1).unwrap().destruction=Some(DestructionSettings {result:Some(100),..Default::default()});
    assert!(!e.project.collision_target_allowed(1,2));
    assert_eq!(e.project.collision_candidate_ids(1),BTreeSet::new());
}
