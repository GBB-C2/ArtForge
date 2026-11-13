use poseforge_core::*;
use poseforge_io::{load_project,save_project,merge_project};
#[test]
fn text_and_cloth_lists_roundtrip_and_merge_keep_geometry_and_references() {
    let mut e=Editor::new(Project::default());let text=e.create_text(TextSettings {content:"Сцена 123".into(),..Default::default()}).unwrap();
    let mut cloth=Node::new(50,"Cloth");cloth.mesh=Some(51);cloth.physics=Some(PhysicsSettings::default());
    cloth.collision_targets.cloth=Some(vec![CollisionTarget {node:text,enabled:false}]);
    e.project.nodes.push(cloth);e.project.meshes.push(box_mesh(51,"Cloth",Vec3::splat(0.2),None));e.project.validate().unwrap();
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("text.artforge");save_project(&e.project,&path).unwrap();
    let loaded=load_project(&path).unwrap();assert_eq!(loaded,e.project);
    let mut p=Project::default();merge_project(&mut p,loaded).unwrap();p.validate().unwrap();
    let cloth=p.nodes.iter().find(|n|n.physics.is_some()).unwrap();let target=cloth.collision_targets.cloth.as_ref().unwrap()[0];
    assert!(!target.enabled);assert!(p.node(target.node).unwrap().text.is_some());
}
#[test]
fn schema_32_without_cloth_list_preserves_legacy_contacts_and_no_text() {
    let mut p=Project::default();let mut obstacle=Node::new(1,"Obstacle");obstacle.mesh=Some(3);
    obstacle.physics=Some(PhysicsSettings {kind:PhysicsKind::Collider,..Default::default()});
    let mut cloth=Node::new(2,"Cloth");cloth.mesh=Some(3);cloth.physics=Some(PhysicsSettings::default());
    p.nodes=vec![obstacle,cloth];p.meshes.push(box_mesh(3,"Mesh",Vec3::ONE,None));
    let mut value=serde_json::to_value(p).unwrap();value["version"]=32.into();
    for node in value["nodes"].as_array_mut().unwrap(){node.as_object_mut().unwrap().remove("text");node.as_object_mut().unwrap().remove("collision_targets");}
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("legacy.artforge");let bytes=serde_json::to_vec(&value).unwrap();std::fs::write(&path,&bytes).unwrap();
    let p=load_project(&path).unwrap();assert_eq!(p.version,SCHEMA_VERSION);assert_eq!(p.resolved_collision_targets(2,CollisionKind::Cloth),vec![CollisionTarget {node:1,enabled:true}]);
    assert!(p.nodes.iter().all(|n|n.text.is_none()));assert_eq!(std::fs::read(path).unwrap(),bytes);
}
#[test]
fn fonts_bevel_curve_flag_and_empty_parent_survive_save_and_merge() {
    let mut e=Editor::new(Project::default());let empty=e.create_empty().unwrap();
    let text=e.create_text(TextSettings {font:TextFont::Serif,bevel:0.015,bevel_segments:4,on_curve:true,..Default::default()}).unwrap();
    e.project.node_mut(text).unwrap().parent=Some(empty);
    let curve=e.project.next_id();let mut n=Node::new(curve,"Круг");n.curve=Some(CurvePreset::Circle.curve());e.project.nodes.push(n);
    e.project.node_mut(text).unwrap().curve_binding=Some(CurveBinding {curve,..Default::default()});
    let dir=tempfile::tempdir().unwrap();let file=dir.path().join("text-path.artforge");save_project(&e.project,&file).unwrap();let loaded=load_project(file).unwrap();assert_eq!(loaded,e.project);
    let mut p=Project::default();merge_project(&mut p,loaded).unwrap();p.validate().unwrap();
    let t=p.nodes.iter().find(|n|n.text.is_some()).unwrap();let settings=t.text.as_ref().unwrap();assert_eq!(settings.font,TextFont::Serif);assert_eq!(settings.bevel_segments,4);assert!(p.node(t.parent.unwrap()).unwrap().empty);assert!(p.node(t.curve_binding.unwrap().curve).unwrap().curve.is_some());assert_eq!(p.evaluated(None,0.).unwrap().text_meshes.len(),1);
}
#[test]
fn schema_33_text_keeps_original_font_geometry_and_has_no_implicit_bevel() {
    let mut e=Editor::new(Project::default());e.create_text(TextSettings::default()).unwrap();let mut value=serde_json::to_value(&e.project).unwrap();value["version"]=33.into();
    for node in value["nodes"].as_array_mut().unwrap() {
        node.as_object_mut().unwrap().remove("empty");
        if let Some(text)=node.get_mut("text").and_then(|t|t.as_object_mut()) {for field in ["font","bevel","bevel_segments","on_curve"] {text.remove(field);}}
    }
    let temp=tempfile::tempdir().unwrap();let file=temp.path().join("legacy.artforge");std::fs::write(&file,serde_json::to_vec(&value).unwrap()).unwrap();let loaded=load_project(file).unwrap();
    assert_eq!(loaded.nodes[0].text,Some(TextSettings::default()));assert!(!loaded.nodes[0].empty);assert_eq!(loaded.meshes,e.project.meshes);
}
