use poseforge_core::*;
fn fixture()->(Editor,u64,u64) {
    let mut e=Editor::new(Project::default());let text=e.create_text(TextSettings {content:"Текст".into(),on_curve:true,..Default::default()}).unwrap();
    let curve=e.project.next_id();let mut n=Node::new(curve,"Путь");n.curve=Some(CurveData {points:vec![[0.,0.,0.],[4.,0.,0.]],interpolation:CurveInterpolation::Polyline,..Default::default()});e.project.nodes.push(n);
    e.project.node_mut(text).unwrap().curve_binding=Some(CurveBinding {curve,..Default::default()});(e,text,curve)
}
#[test]
fn fonts_and_bevel_change_real_geometry_and_preserve_materials() {
    let mut e=Editor::new(Project::default());let id=e.create_text(TextSettings::default()).unwrap();let original=e.project.clone();
    let mut buffers=vec![];
    for font in TextFont::ALL {
        let settings=TextSettings {font,content:"ОB8Ё".into(),bevel:0.02,bevel_segments:4,..Default::default()};
        let plain=TextSettings {bevel:0.,..settings.clone()}.mesh(1,None).unwrap();
        e.set_text(id,settings).unwrap();let mesh=e.project.mesh(e.project.node(id).unwrap().mesh.unwrap()).unwrap();mesh.validate_topology().unwrap();
        assert!(mesh.vertices.len()>plain.vertices.len());assert!(mesh.vertices.iter().any(|v|v.normal[2]>0.1&&v.normal[2]<0.99));
        assert_eq!(e.project.materials,original.materials);buffers.push(mesh.vertices.clone());
    }
    assert_ne!(buffers[0],buffers[1]);assert_ne!(buffers[0],buffers[2]);
    let before=e.project.clone();let bad=TextSettings {bevel:f32::NAN,..Default::default()};assert!(e.set_text(id,bad).is_err());assert_eq!(e.project,before);
}
#[test]
fn text_uses_x_baseline_on_path_and_reuses_immutable_geometry() {
    let (mut e,id,curve)=fixture();let before=e.project.clone();let a=e.evaluated().unwrap();let b=e.evaluated().unwrap();
    assert_eq!(e.project,before);assert!(a.text_meshes[&id].vertices.shares_storage(&b.text_meshes[&id].vertices));
    let positions:Vec<_>=a.text_meshes[&id].vertices.iter().map(|v|a.global[&id].transform_point3(Vec3::from_array(v.position))).collect();
    assert!(positions.iter().map(|p|p.x).fold(f32::INFINITY,f32::min).abs()<1e-4);
    assert!(positions.iter().map(|p|p.x).fold(f32::NEG_INFINITY,f32::max)>1.);
    e.project.node_mut(curve).unwrap().curve=Some(CurvePreset::Circle.curve());
    let curved=e.evaluated().unwrap();assert_ne!(curved.text_meshes[&id].vertices,a.text_meshes[&id].vertices);
    assert!(curved.text_meshes[&id].vertices.iter().all(|v|Vec3::from_array(v.position).is_finite()&&Vec3::from_array(v.normal).is_normalized()));
    e.project.node_mut(id).unwrap().text.as_mut().unwrap().on_curve=false;assert!(e.evaluated().unwrap().text_meshes.is_empty());
}
#[test]
fn linked_text_instances_animate_independently_and_conversion_is_reversible() {
    let (mut e,a,_)=fixture();let b=e.project.duplicate_subtree(a).unwrap();
    e.project.node_mut(b).unwrap().curve_binding.as_mut().unwrap().position=0.5;
    e.project.node_mut(a).unwrap().curve_binding.as_mut().unwrap().speed=0.25;
    let before=e.project.clone();let frame=e.evaluated().unwrap();let later=e.project.evaluated(None,1.).unwrap();
    let world=|f:&EvaluatedScene,id|f.global[&id].transform_point3(Vec3::from_array(f.text_meshes[&id].vertices[0].position));
    assert!((world(&later,a)-world(&frame,a)-Vec3::X).length()<1e-4);assert!((world(&frame,b)-world(&frame,a)-Vec3::X*2.).length()<1e-4);
    let mut sampled=e.project.clone();sampled.apply_text_meshes(&frame.text_meshes);assert_ne!(sampled.node(a).unwrap().mesh,sampled.node(b).unwrap().mesh);assert_eq!(e.project,before);
    e.convert_text_to_mesh(a).unwrap();assert!(e.project.node(a).unwrap().text.is_none());assert_eq!(e.project.mesh(e.project.node(a).unwrap().mesh.unwrap()).unwrap().vertices,frame.text_meshes[&a].vertices);
    assert_eq!(e.project.mesh(e.project.node(b).unwrap().mesh.unwrap()).unwrap().vertices,before.mesh(before.node(b).unwrap().mesh.unwrap()).unwrap().vertices);
    e.undo();assert_eq!(e.project,before);
}
#[test]
fn open_path_extends_long_text_without_collapsing_end_vertices() {
    let (mut e,id,path)=fixture();e.project.node_mut(path).unwrap().curve.as_mut().unwrap().points=vec![[0.,0.,0.],[0.1,0.,0.]];
    e.project.node_mut(id).unwrap().curve_binding.as_mut().unwrap().position=1.;let scene=e.evaluated().unwrap();let mesh=&scene.text_meshes[&id];
    let world=|i:u32|scene.global[&id].transform_point3(Vec3::from_array(mesh.vertices[i as usize].position));
    assert!(mesh.vertices.iter().any(|v|scene.global[&id].transform_point3(Vec3::from_array(v.position)).x>1.));
    for t in mesh.indices.chunks_exact(3) {assert!((world(t[1])-world(t[0])).cross(world(t[2])-world(t[0])).length()>1e-10);}
}
#[test]
fn empty_is_a_nonrendering_parent_and_can_follow_a_curve() {
    let (mut e,child,path)=fixture();let id=e.create_empty().unwrap();assert!(e.project.node(id).unwrap().empty);assert!(e.project.node(id).unwrap().mesh.is_none());
    e.project.node_mut(child).unwrap().curve_binding=None;e.project.node_mut(child).unwrap().parent=Some(id);
    e.project.node_mut(id).unwrap().curve_binding=Some(CurveBinding {curve:path,position:0.5,..Default::default()});
    e.project.validate().unwrap();let frame=e.evaluated().unwrap();assert_eq!(frame.global[&id],frame.global[&child]);assert!(e.editable_mesh(id).is_err());
    let clone=e.project.duplicate_subtree(id).unwrap();assert!(e.project.node(clone).unwrap().empty);
}
#[test]
fn physics_cannot_silently_disable_text_path_deformation() {
    let (mut e,id,_)=fixture();let before=e.project.clone();
    assert!(e.transact("Cloth",|p|p.node_mut(id).unwrap().physics=Some(PhysicsSettings {kind:PhysicsKind::Cloth,..Default::default()})).is_err());assert_eq!(e.project,before);
    e.convert_text_to_mesh(id).unwrap();e.transact("Cloth",|p|p.node_mut(id).unwrap().physics=Some(PhysicsSettings {kind:PhysicsKind::Cloth,..Default::default()})).unwrap();
}
