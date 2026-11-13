use poseforge_core::*;
use poseforge_io::{load_project,merge_project,save_project};

fn scene() -> Project {
    let mut p=Project::default();p.meshes.push(box_mesh(10,"Mesh",Vec3::splat(0.3),None));
    for id in 1..=3 {let mut n=Node::new(id,format!("Mesh {id}"));n.mesh=Some(10);p.nodes.push(n);}
    p.node_mut(1).unwrap().fluid=Some(FluidDomainSettings::default());
    p.node_mut(2).unwrap().destruction=Some(DestructionSettings::default());
    p
}
#[test]
fn collision_lists_roundtrip_with_disabled_entries_and_explicit_empty() {
    let mut p=scene();
    p.node_mut(1).unwrap().collision_targets.fluid=Some(vec![CollisionTarget {node:3,enabled:false}]);
    p.node_mut(2).unwrap().collision_targets.destruction=Some(vec![]);
    let temp=tempfile::tempdir().unwrap();let file=temp.path().join("targets.artforge");
    save_project(&p,&file).unwrap();assert_eq!(load_project(&file).unwrap(),p);
}
#[test]
fn schema_31_keeps_legacy_colliders_without_rewriting_input() {
    let mut p=scene();p.node_mut(3).unwrap().physics=Some(PhysicsSettings {kind:PhysicsKind::Collider,..Default::default()});
    let mut json=serde_json::to_value(&p).unwrap();json["version"]=31.into();
    for n in json["nodes"].as_array_mut().unwrap() {n.as_object_mut().unwrap().remove("collision_targets");}
    let bytes=serde_json::to_vec(&json).unwrap();let temp=tempfile::tempdir().unwrap();let file=temp.path().join("legacy.artforge");
    std::fs::write(&file,&bytes).unwrap();let loaded=load_project(&file).unwrap();
    assert_eq!(loaded.version,SCHEMA_VERSION);
    assert_eq!(loaded.resolved_collision_targets(1,CollisionKind::Fluid),vec![CollisionTarget {node:3,enabled:true}]);
    assert_eq!(std::fs::read(&file).unwrap(),bytes);
}
#[test]
fn merge_remaps_targets_to_imported_objects_not_matching_destination_ids() {
    let mut incoming=scene();incoming.node_mut(1).unwrap().collision_targets.fluid=Some(vec![CollisionTarget {node:3,enabled:true}]);
    let mut destination=scene();merge_project(&mut destination,incoming).unwrap();destination.validate().unwrap();
    let imported=destination.nodes.iter().find(|n|n.id>10&&n.fluid.is_some()).unwrap();
    let entry=imported.collision_targets.fluid.as_ref().unwrap()[0];
    assert!(entry.node>10);assert_ne!(entry.node,3);assert_eq!(destination.node(entry.node).unwrap().name,"Mesh 3");
    assert!(destination.node(1).unwrap().collision_targets.fluid.is_none());
}
