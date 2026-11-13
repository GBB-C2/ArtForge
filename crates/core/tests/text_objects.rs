use poseforge_core::*;
use std::collections::BTreeMap;
#[test]
fn cyrillic_text_has_outward_faces_closed_shells_and_open_counters() {
    let settings=TextSettings {content:"ОB8Ё".into(),..Default::default()};
    let mesh=settings.mesh(1,None).unwrap();mesh.validate_topology().unwrap();
    let point=|i:u32|Vec3::from_array(mesh.vertices[i as usize].position);
    let key=|p:Vec3|p.to_array().map(|v|(v as f64*1e6).round() as i64);
    let mut edges=BTreeMap::new();
    for t in mesh.indices.chunks_exact(3) {
        let n=(point(t[1])-point(t[0])).cross(point(t[2])-point(t[0]));
        assert!(n.length()>1e-10);assert!(n.dot(Vec3::from_array(mesh.vertices[t[0] as usize].normal))>0.);
        for i in 0..3 {let a=key(point(t[i]));let b=key(point(t[(i+1)%3]));*edges.entry((a.min(b),a.max(b))).or_insert(0)+=1;}
    }
    assert!(edges.values().all(|n|*n==2));
    let ring=TextSettings {content:"О".into(),depth:0.,..Default::default()}.mesh(2,None).unwrap();
    let cross=|a:Vec3,b:Vec3|a.x*b.y-a.y*b.x;
    assert!(!ring.indices.chunks_exact(3).any(|t| {
        let p=t.iter().map(|&i|Vec3::from_array(ring.vertices[i as usize].position)).collect::<Vec<_>>();
        cross(p[1]-p[0],-p[0])>=0.&&cross(p[2]-p[1],-p[1])>=0.&&cross(p[0]-p[2],-p[2])>=0.
    }));
}
#[test]
fn text_edit_detaches_shared_geometry_and_undo_conversion_restores_source() {
    let mut e=Editor::new(Project::default());let a=e.create_text(TextSettings::default()).unwrap();
    let b=e.project.duplicate_subtree(a).unwrap();let original=e.project.clone();
    let next=TextSettings {content:"ArtForge 3D".into(),depth:0.2,..Default::default()};
    e.set_text(a,next.clone()).unwrap();assert_eq!(e.project.node(a).unwrap().text,Some(next));
    assert_ne!(e.project.node(a).unwrap().mesh,e.project.node(b).unwrap().mesh);
    assert_eq!(e.project.node(b),original.node(b));assert!(e.editable_mesh(a).is_err());
    let before=e.project.clone();e.convert_text_to_mesh(a).unwrap();assert!(e.editable_mesh(a).is_ok());
    assert_eq!(e.project.meshes,before.meshes);e.undo();assert_eq!(e.project,before);e.undo();assert_eq!(e.project,original);
}
#[test]
fn invalid_text_is_atomic_and_multiline_and_zero_depth_are_supported() {
    let mut e=Editor::new(Project::default());let id=e.create_text(TextSettings::default()).unwrap();let before=e.project.clone();
    for content in ["","   ","🙂"] {assert!(e.set_text(id,TextSettings {content:content.into(),..Default::default()}).is_err());assert_eq!(e.project,before);}
    assert!(e.set_text(id,TextSettings {size:f32::NAN,..Default::default()}).is_err());assert_eq!(e.project,before);
    let mesh=TextSettings {content:"Привет\nМир".into(),depth:0.,..Default::default()}.mesh(3,None).unwrap();
    assert!(mesh.vertices.iter().all(|v|v.position[2]==0.));assert!(mesh.vertices.iter().any(|v|v.position[1]< -0.5));
    e.project.node_mut(id).unwrap().locked=true;assert!(e.convert_text_to_mesh(id).is_err());
}
