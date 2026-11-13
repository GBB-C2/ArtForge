use poseforge_core::*;
use std::collections::BTreeSet;
fn scene() -> Editor {
    let mut mesh=PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap();
    let second=mesh.clone(); let offset=mesh.vertices.len() as u32;
    mesh.vertices.extend(second.vertices.iter().map(|v| { let mut v=*v;v.position[0]+=4.;v }));
    mesh.indices.extend(second.indices.iter().map(|i|*i+offset));
    mesh.topology.as_mut().unwrap().faces.extend(second.topology.unwrap().faces.iter().map(|f|f.iter().map(|i|*i+offset).collect()));
    mesh.topology.as_mut().unwrap().primitive=None;
    let mut a=Node::new(1,"A");a.mesh=Some(2);let mut b=a.clone();b.id=5;b.name="Instance".into();
    Editor::new(Project {nodes:vec![a,b],meshes:vec![mesh],materials:vec![Material{id:3,..Default::default()},Material{id:4,..Default::default()}],..Default::default()})
}
#[test]
fn selected_faces_connected_island_and_whole_mesh_have_distinct_scopes_and_undo() {
    for scope in [Some(BTreeSet::from([0])),Some((0..6).collect()),None] {
        let mut e=scene();let original=e.project.clone();
        let graph=MeshConnectivity::new(&e.project.meshes[0]);assert_eq!(graph.connected_faces(0),(0..6).collect());
        let result=e.assign_material_region(1,4,scope.as_ref()).unwrap();assert_ne!(result.mesh,2);
        let m=e.project.mesh(result.mesh).unwrap();
        for f in 0..12 {assert_eq!(m.face_material(f),Some(if scope.as_ref().is_none_or(|s|s.contains(&f)){4}else{3}));}
        assert_eq!(e.project.mesh(2).unwrap(),&original.meshes[0]);
        assert_eq!(e.undo_len(),1);assert!(e.undo());assert_eq!(e.project,original);assert!(e.redo());
        assert_eq!(e.project.node(1).unwrap().mesh,Some(result.mesh));
    }
}
#[test]
fn locked_and_invalid_regions_leave_the_document_and_history_untouched() {
    let mut e=scene(); e.project.node_mut(1).unwrap().locked=true;let original=e.project.clone();
    assert!(e.assign_material_region(1,4,None).is_err());assert_eq!(e.project,original);assert_eq!(e.undo_len(),0);
    e.project.node_mut(1).unwrap().locked=false;let original=e.project.clone();
    assert!(e.assign_material_region(1,4,Some(&BTreeSet::from([99]))).is_err());assert_eq!(e.project,original);
}
#[test]
fn painting_the_base_material_is_explicit_and_whole_drop_keeps_hidden_layers() {
    let mut e=scene();let layer=e.add_material_layer(1,None).unwrap();let original=e.project.clone();
    let result=e.assign_material_region(1,3,Some(&BTreeSet::from([0]))).unwrap();
    let mesh=e.project.mesh(result.mesh).unwrap();
    assert_eq!(mesh.topology.as_ref().unwrap().face_materials[0],Some(3));
    assert!(e.project.node(1).unwrap().material_layers[0].visible);
    assert!(e.undo());assert_eq!(e.project,original);
    e.assign_material_region(1,4,None).unwrap();
    let layers=&e.project.node(1).unwrap().material_layers;
    assert_eq!(layers.len(),2);
    assert_eq!(layers.iter().map(|l|l.material).collect::<Vec<_>>(),vec![3,layer]);
    assert!(layers.iter().all(|l|!l.visible));
    let after=e.project.clone();let count=e.undo_len();
    e.assign_material_region(1,4,None).unwrap();
    assert_eq!(e.project,after);assert_eq!(e.undo_len(),count);
    assert!(e.undo());assert_eq!(e.project,original);
}
fn shape_scene() -> Editor {
    let mut n=Node::new(1,"Quad");n.mesh=Some(2);
    let mut mesh=box_mesh(2,"Quad",Vec3::ONE,Some(3));
    mesh.vertices=[[-1.,-1.,0.],[2.,-1.,0.],[1.,1.,0.],[-1.,1.,0.]].into_iter().enumerate().map(|(i,p)|Vertex {position:p,normal:[0.,0.,1.],uv:[i as f32/4.,0.],..Default::default()}).collect();
    mesh.topology=Some(MeshTopology { loose_edges: Vec::new(),faces:vec![vec![0,1,2,3]].into(),face_materials:vec![Some(3)].into(),uv_seams:vec![],primitive:None,smooth_normals:false});
    mesh.indices=vec![0,1,2,0,2,3].into();
    Editor::new(Project {nodes:vec![n],meshes:vec![mesh],materials:vec![Material{id:3,..Default::default()}],..Default::default()})
}
#[test]
fn new_shapes_preserve_topology_uv_and_materials_and_are_atomic() {
    for edit in [ShapeEdit::Sphere{strength:0.5},ShapeEdit::Circle{axis:2,strength:0.5},ShapeEdit::Inflate{distance:0.2},ShapeEdit::Twist{axis:1,radians:0.4}] {
        let mut e=shape_scene();let original=e.project.clone();
        e.edit_mesh(1,MeshEdit::Shape {vertices:(0..4).collect(),edges:BTreeSet::new(),edit}).unwrap();
        let mesh=e.project.meshes.first().unwrap();assert_ne!(mesh.vertices,original.meshes[0].vertices);
        assert_eq!(mesh.face_vertices(0),original.meshes[0].face_vertices(0));assert_eq!(mesh.face_material(0),Some(3));
        assert_eq!(mesh.vertices.iter().map(|v|v.uv).collect::<Vec<_>>(),original.meshes[0].vertices.iter().map(|v|v.uv).collect::<Vec<_>>());
        e.project.validate().unwrap();assert_eq!(e.undo_len(),1);assert!(e.undo());assert_eq!(e.project,original);
    }
    let mut e=shape_scene();let original=e.project.clone();
    assert!(e.edit_mesh(1,MeshEdit::Shape{vertices:(0..4).collect(),edges:BTreeSet::new(),edit:ShapeEdit::Twist{axis:2,radians:1.}}).is_err());
    assert_eq!(e.project,original);assert_eq!(e.undo_len(),0);
}
