use poseforge_core::*;
use poseforge_io::{load_project,save_project};
#[test]
fn helper_shapes_fields_and_primitives_roundtrip_in_project_files() {
    let mut e=Editor::new(Project::default());for shape in EmptyShape::ALL {e.create_empty_shape(shape).unwrap();}for kind in ForceFieldKind::ALL {e.create_force_field(kind).unwrap();}
    for (_,kind) in PrimitiveKind::MESH_MENU {let id=e.project.next_id();let mut n=Node::new(id,"Primitive");n.mesh=Some(id+1);e.project.meshes.push(PrimitiveSettings::new(kind).mesh(id+1,None).unwrap());e.project.nodes.push(n);}
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("helpers.artforge");save_project(&e.project,&path).unwrap();assert_eq!(load_project(&path).unwrap(),e.project);
    let bytes=std::fs::read(&path).unwrap();e.project.nodes.iter_mut().find(|n|n.force_field.is_some()).unwrap().force_field.as_mut().unwrap().radius=-1.;assert!(save_project(&e.project,&path).is_err());assert_eq!(std::fs::read(path).unwrap(),bytes);
}
#[test]
fn schema_35_empties_keep_legacy_axes_and_add_no_fields_without_rewriting_source() {
    let mut e=Editor::new(Project::default());e.create_empty().unwrap();let mut value=serde_json::to_value(&e.project).unwrap();value["version"]=35.into();
    for n in value["nodes"].as_array_mut().unwrap() {for key in ["empty_shape","empty_size","force_field"] {n.as_object_mut().unwrap().remove(key);}}
    let bytes=serde_json::to_vec(&value).unwrap();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("old.artforge");std::fs::write(&path,&bytes).unwrap();
    let loaded=load_project(&path).unwrap();assert_eq!(loaded.version,SCHEMA_VERSION);assert_eq!(loaded.nodes[0].empty_shape,EmptyShape::Axes);assert_eq!(loaded.nodes[0].empty_size,1.);assert!(loaded.nodes[0].force_field.is_none());assert_eq!(std::fs::read(path).unwrap(),bytes);
}
