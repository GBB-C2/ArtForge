use poseforge_core::*;
use std::collections::BTreeSet;

fn scene() -> Editor {
    let mut a=Node::new(1,"A");a.mesh=Some(2);
    let mut b=a.clone();b.id=5;b.name="B".into();
    Editor::new(Project {nodes:vec![a,b],meshes:vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap()],
        materials:vec![Material{id:3,..Default::default()},Material{id:4,in_library:true,..Default::default()}],..Default::default()})
}
#[test]
fn polygon_unassign_preserves_other_faces_instances_and_saved_memberships() {
    let mut e=scene();let original=e.project.clone();
    e.assign_material_region(1,4,Some(&BTreeSet::from([0,2]))).unwrap();
    let assigned=e.project.clone();
    let id=e.unassign_face_material(1,4,&BTreeSet::from([0,1])).unwrap();
    let mesh=e.project.mesh(id).unwrap();
    assert_eq!(mesh.face_material(0),Some(3));assert_eq!(mesh.face_material(2),Some(4));
    assert_eq!(e.project.mesh(2).unwrap(),original.mesh(2).unwrap());
    assert_eq!(mesh.vertices,original.mesh(2).unwrap().vertices);
    assert_eq!(mesh.indices,original.mesh(2).unwrap().indices);
    assert_eq!(e.undo_len(),2);
    let saved:Project=serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
    saved.validate().unwrap();assert_eq!(saved,e.project);
    assert!(e.undo());assert_eq!(e.project,assigned);assert!(e.redo());assert_eq!(e.project,saved);
}
#[test]
fn removing_the_base_from_a_subset_preserves_every_other_polygon() {
    let mut e=scene();let original=e.project.clone();
    let id=e.unassign_face_material(1,3,&BTreeSet::from([1,4])).unwrap();
    let mesh=e.project.mesh(id).unwrap();assert_ne!(id,2);
    for face in 0..6 {assert_eq!(mesh.face_material(face),if [1,4].contains(&face){None}else{Some(3)});}
    assert_eq!(e.project.mesh(2).unwrap(),original.mesh(2).unwrap());
    assert!(e.project.material(3).is_some());e.project.validate().unwrap();
    assert_eq!(e.undo_len(),1);assert!(e.undo());assert_eq!(e.project,original);
}
#[test]
fn locked_invalid_empty_and_unrelated_scopes_are_atomic() {
    let mut e=scene();let original=e.project.clone();
    for faces in [BTreeSet::new(),BTreeSet::from([99])] {
        assert!(e.unassign_face_material(1,3,&faces).is_err());assert_eq!(e.project,original);
    }
    assert!(e.unassign_face_material(1,99,&BTreeSet::from([0])).is_err());
    e.unassign_face_material(1,4,&BTreeSet::from([0])).unwrap();assert_eq!(e.project,original);assert_eq!(e.undo_len(),0);
    e.project.node_mut(1).unwrap().locked=true;let locked=e.project.clone();
    assert!(e.unassign_face_material(1,3,&BTreeSet::from([0])).is_err());assert_eq!(e.project,locked);assert_eq!(e.undo_len(),0);
}
