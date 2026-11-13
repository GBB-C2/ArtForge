use poseforge_core::{MaterialPreset, Project, SCHEMA_VERSION};
use poseforge_io::{load_project,save_project};

#[test]
fn artistic_schema_42_migration_does_not_rewrite_or_restyle_old_materials() {
    let p=Project { materials:vec![MaterialPreset::Plastic.create()],..Project::default() };
    let mut json=serde_json::to_value(&p).unwrap(); json["version"]=42.into();
    json["materials"][0].as_object_mut().unwrap().remove("artistic");
    let bytes=serde_json::to_vec(&json).unwrap();
    let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("old.artforge");
    std::fs::write(&path,&bytes).unwrap(); let loaded=load_project(&path).unwrap();
    assert_eq!(loaded.version,SCHEMA_VERSION); assert_eq!(loaded.materials,p.materials);
    assert_eq!(std::fs::read(&path).unwrap(),bytes);
}
#[test]
fn artistic_materials_roundtrip_and_invalid_save_keeps_previous_file() {
    let mut p=Project::default();
    for (i,preset) in MaterialPreset::ALL[13..].iter().enumerate() {
        let mut m=preset.create(); m.id=100+i as u64; m.artistic.seed=117;
        p.materials.push(m);
    }
    let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("Лепка и акварель.artforge");
    save_project(&p,&path).unwrap(); assert_eq!(load_project(&path).unwrap(),p);
    let bytes=std::fs::read(&path).unwrap(); p.materials[0].artistic.scale=0.;
    assert!(save_project(&p,&path).is_err()); assert_eq!(std::fs::read(&path).unwrap(),bytes);
}
#[test]
fn artistic_gallery_loads_with_thirteen_editable_presets() {
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/artistic-materials.artforge");
    let p=load_project(path).unwrap(); p.validate().unwrap();
    assert_eq!(p.materials.iter().filter(|m|m.artistic.enabled).count(),13);
}
