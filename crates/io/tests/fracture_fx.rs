use poseforge_core::*;
use poseforge_io::{load_project,save_project,merge_project};

#[test]
fn schema_40_keeps_legacy_fx_and_new_gas_profiles_roundtrip_and_merge() {
    let mut e=Editor::new(Project::default());for kind in FxKind::ALL {e.create_fx(kind).unwrap();}
    let mut json=serde_json::to_value(&e.project).unwrap();json["version"]=40.into();
    for node in json["nodes"].as_array_mut().unwrap() {let f=node["fx"].as_object_mut().unwrap();for key in ["gas","gas_hollow","gas_domain","dissipation","buoyancy","quality","envelope"] {f.remove(key);}}
    let bytes=serde_json::to_vec(&json).unwrap();let dir=tempfile::tempdir().unwrap();let old=dir.path().join("old40.artforge");std::fs::write(&old,&bytes).unwrap();
    let mut loaded=load_project(&old).unwrap();assert_eq!(loaded,e.project);assert_eq!(std::fs::read(old).unwrap(),bytes);
    for (i,n) in loaded.nodes.iter_mut().enumerate() {let f=n.fx.as_mut().unwrap();*f=f.styled(1).unwrap();f.gas_hollow=true;f.gas_domain=3.;f.dissipation=0.25;f.buoyancy=0.7;
        f.quality=FxQuality::ALL[i%3];f.envelope.enabled=true;f.envelope.keys[1].size=0.4;f.envelope.keys[2].color=[0.2,0.7,1.];}
    let path=dir.path().join("gas41.artforge");save_project(&loaded,&path).unwrap();assert_eq!(load_project(&path).unwrap(),loaded);
    let mut merged=loaded.clone();merge_project(&mut merged,loaded.clone()).unwrap();merged.validate().unwrap();
    for n in &loaded.nodes {assert_eq!(merged.nodes.iter().filter(|other|other.fx==n.fx).count(),2);}
}

#[test]
fn fx_and_interior_slots_roundtrip_and_merge_remaps_material_references() {
    let mut editor=Editor::new(Project::default());
    for kind in FxKind::ALL {editor.create_fx(kind).unwrap();}
    let source=editor.project.next_id();let mut node=Node::new(source,"Fracture");node.mesh=Some(source+1);
    node.destruction=Some(DestructionSettings {interior_material:Some(source+2),shard_collision:true,..Default::default()});
    editor.project.nodes.push(node);editor.project.meshes.push(box_mesh(source+1,"Box",Vec3::ONE,None));
    editor.project.materials.push(Material {id:source+2,name:"Cut material".into(),in_library:false,..Default::default()});
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("fx.artforge");save_project(&editor.project,&path).unwrap();
    let loaded=load_project(&path).unwrap();assert_eq!(loaded,editor.project);
    let mut merged=loaded.clone();merge_project(&mut merged,loaded).unwrap();merged.validate().unwrap();
    let mut ids=std::collections::BTreeSet::new();
    for n in &merged.nodes {if let Some(d)=n.destruction {let id=d.interior_material.unwrap();assert!(d.shard_collision);assert_eq!(merged.material(id).unwrap().name,"Cut material");ids.insert(id);}}
    assert_eq!(ids.len(),2);assert_eq!(merged.nodes.iter().filter(|n|n.fx.is_some()).count(),8);
    let mut editor=Editor::new(merged);editor.remove_unused_materials().unwrap();
    assert!(ids.iter().all(|&id|editor.project.material(id).is_some()));
}

#[test]
fn schema_36_defaults_new_components_without_modifying_original_file() {
    let mut p=Project::default();let mut n=Node::new(1,"Old fracture");n.mesh=Some(2);n.destruction=Some(DestructionSettings::default());
    p.nodes.push(n);p.meshes.push(box_mesh(2,"Cube",Vec3::ONE,None));
    let mut json=serde_json::to_value(&p).unwrap();json["version"]=36.into();
    let component=json["nodes"][0]["destruction"].as_object_mut().unwrap();component.remove("interior_material");component.remove("shard_collision");
    json["nodes"][0].as_object_mut().unwrap().remove("fx");
    let bytes=serde_json::to_vec(&json).unwrap();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("old.artforge");std::fs::write(&path,&bytes).unwrap();
    let loaded=load_project(&path).unwrap();assert_eq!(loaded.version,SCHEMA_VERSION);
    assert!(loaded.nodes[0].fx.is_none());let d=loaded.nodes[0].destruction.unwrap();assert!(!d.shard_collision && d.interior_material.is_none());
    assert_eq!(std::fs::read(path).unwrap(),bytes);
}

#[test]
fn schema_37_fx_migrate_without_turning_on_new_lights_or_rewriting_source() {
    let mut editor=Editor::new(Project::default());for kind in FxKind::ALL {editor.create_fx(kind).unwrap();}
    let mut json=serde_json::to_value(&editor.project).unwrap();json["version"]=37.into();
    for node in json["nodes"].as_array_mut().unwrap() {
        let fx=node["fx"].as_object_mut().unwrap();
        for key in ["width","height","softness","drift","smoke","flicker","illuminate"] {fx.remove(key);}
    }
    let bytes=serde_json::to_vec(&json).unwrap();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("fx37.artforge");
    std::fs::write(&path,&bytes).unwrap();let loaded=load_project(&path).unwrap();assert_eq!(loaded.version,SCHEMA_VERSION);
    for (old,node) in editor.project.nodes.iter().zip(&loaded.nodes) {
        assert_eq!(node.fx.unwrap(),FxSettings {illuminate:false,..old.fx.unwrap()});
    }
    assert_eq!(std::fs::read(path).unwrap(),bytes);loaded.validate().unwrap();
}

#[test]
fn customized_fx_shape_smoke_and_light_roundtrip_exactly() {
    let mut editor=Editor::new(Project::default());let id=editor.create_fx(FxKind::Fire).unwrap();
    let node=editor.project.node_mut(id).unwrap();node.transform.translation=Vec3::new(2.,3.,4.);
    node.fx=Some(FxSettings {width:0.3,height:0.7,softness:0.6,drift:[-0.4,0.2],smoke:0.8,flicker:0.65,illuminate:true,..FxSettings::default()});
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("fx38.artforge");save_project(&editor.project,&path).unwrap();
    assert_eq!(load_project(path).unwrap(),editor.project);
}

#[test]
fn fx_sources_obstacles_and_field_settings_roundtrip_and_merge_remap_ids() {
    let mut e=Editor::new(Project::default());let id=e.create_fx(FxKind::Steam).unwrap();
    for (node_id,name) in [(10,"Emitter"),(20,"Obstacle")] {
        let mut n=Node::new(node_id,name);n.mesh=Some(node_id+1);e.project.nodes.push(n);e.project.meshes.push(box_mesh(node_id+1,name,Vec3::ONE,None));
    }
    let field=e.create_force_field(ForceFieldKind::Wind).unwrap();e.project.node_mut(field).unwrap().force_field.as_mut().unwrap().fx=false;
    e.set_fx_source(id,Some(10)).unwrap();let n=e.project.node_mut(id).unwrap();
    n.collision_targets.fx=Some(vec![CollisionTarget{node:20,enabled:false}]);let f=n.fx.as_mut().unwrap();f.mesh_collision=true;f.field_influence=0.7;f.source_radius=0.4;
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("sources39.artforge");save_project(&e.project,&path).unwrap();
    let loaded=load_project(&path).unwrap();assert_eq!(loaded,e.project);
    let mut merged=loaded.clone();merge_project(&mut merged,loaded).unwrap();merged.validate().unwrap();
    let copy=merged.nodes.iter().find(|n|n.fx.is_some() && n.id!=id).unwrap();let source=copy.fx.unwrap().source.unwrap();
    assert_ne!(source,10);assert_eq!(merged.node(source).unwrap().name,"Emitter");
    let obstacle=merged.resolved_collision_targets(copy.id,CollisionKind::Fx)[0];assert_ne!(obstacle.node,20);assert!(!obstacle.enabled);
    assert!(merged.nodes.iter().filter_map(|n|n.force_field).all(|f|!f.fx));
}

#[test]
fn schema_38_gains_safe_source_defaults_without_rewriting_original() {
    let mut e=Editor::new(Project::default());e.create_fx(FxKind::Fire).unwrap();e.create_force_field(ForceFieldKind::Wind).unwrap();
    let mut json=serde_json::to_value(&e.project).unwrap();json["version"]=38.into();
    for n in json["nodes"].as_array_mut().unwrap() {
        if let Some(f)=n["fx"].as_object_mut() {for name in ["source","source_radius","use_fields","field_influence","mesh_collision"] {f.remove(name);}}
        if let Some(f)=n["force_field"].as_object_mut() {f.remove("fx");}
        if let Some(c)=n.get_mut("collision_targets").and_then(serde_json::Value::as_object_mut) {c.remove("fx");}
    }
    let bytes=serde_json::to_vec(&json).unwrap();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("old38.artforge");std::fs::write(&path,&bytes).unwrap();
    let loaded=load_project(&path).unwrap();assert_eq!(loaded.version,SCHEMA_VERSION);assert_eq!(std::fs::read(&path).unwrap(),bytes);
    let fx=loaded.nodes.iter().find(|n|n.fx.is_some()).unwrap();let settings=fx.fx.unwrap();
    assert!(settings.source.is_none() && !settings.mesh_collision && settings.use_fields);assert_eq!(settings.source_radius,0.25);
    assert!(loaded.resolved_collision_targets(fx.id,CollisionKind::Fx).is_empty());
    assert!(loaded.nodes.iter().filter_map(|n|n.force_field).all(|f|f.fx));
}

#[test]
fn schema_39_migrates_modes_in_memory_and_new_modes_roundtrip_for_all_four_kinds() {
    let mut e=Editor::new(Project::default());for kind in FxKind::ALL {e.create_fx(kind).unwrap();}
    let mut json=serde_json::to_value(&e.project).unwrap();json["version"]=39.into();
    for n in json["nodes"].as_array_mut().unwrap() {for key in ["cloud","burst","start","duration","looped"] {n["fx"].as_object_mut().unwrap().remove(key);}}
    let bytes=serde_json::to_vec(&json).unwrap();let dir=tempfile::tempdir().unwrap();let old=dir.path().join("fx39.artforge");std::fs::write(&old,&bytes).unwrap();
    let mut loaded=load_project(&old).unwrap();assert_eq!(loaded,e.project);assert_eq!(std::fs::read(&old).unwrap(),bytes);
    for (i,node) in loaded.nodes.iter_mut().enumerate() {let f=node.fx.as_mut().unwrap();f.cloud=true;f.burst=true;f.start=i as f64*1.25;f.duration=2.5;f.looped=i%2==0;}
    let path=dir.path().join("modes40.artforge");save_project(&loaded,&path).unwrap();assert_eq!(load_project(path).unwrap(),loaded);
    let mut merged=loaded.clone();merge_project(&mut merged,loaded).unwrap();merged.validate().unwrap();
    for kind in FxKind::ALL {assert_eq!(merged.nodes.iter().filter_map(|n|n.fx).filter(|f|f.kind==kind && f.cloud && f.burst).count(),2);}
}

#[test]
fn schema_41_preserves_old_fx_and_combustion_roundtrips_and_merges() {
    let mut e=Editor::new(Project::default());for kind in FxKind::ALL {e.create_fx(kind).unwrap();}
    let mut json=serde_json::to_value(&e.project).unwrap();json["version"]=41.into();
    for n in json["nodes"].as_array_mut().unwrap() {n["fx"].as_object_mut().unwrap().remove("pyro");}
    let bytes=serde_json::to_vec(&json).unwrap();let dir=tempfile::tempdir().unwrap();let old=dir.path().join("old41.artforge");std::fs::write(&old,&bytes).unwrap();
    let loaded=load_project(&old).unwrap();assert_eq!(loaded,e.project);assert_eq!(std::fs::read(old).unwrap(),bytes);assert!(loaded.nodes.iter().filter_map(|n|n.fx).all(|f|f.pyro.is_none()));
    let mut e=Editor::new(Project::default());for kind in FxKind::ALL {let id=e.create_pyro(kind).unwrap();let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();
        *f=f.styled(2).unwrap();let p=f.pyro.as_mut().unwrap();p.smoke_color=[0.21,0.32,0.43];p.self_shadow=false;p.temperature_color=false;p.anisotropy=-0.3;p.burst_seconds=0.08;}
    let path=dir.path().join("pyro42.artforge");save_project(&e.project,&path).unwrap();let loaded=load_project(&path).unwrap();assert_eq!(loaded,e.project);
    let mut merged=loaded.clone();merge_project(&mut merged,loaded).unwrap();merged.validate().unwrap();
    for n in e.project.nodes {assert_eq!(merged.nodes.iter().filter(|other|other.fx==n.fx).count(),2);}
}
