use poseforge_core::*;

#[test]
fn merging_remaps_scene_order_material_order_and_folder_lights() {
    let mut node = Node::new(1, "A");
    node.mesh = Some(2);
    let mut incoming = Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, Some(3))
            .unwrap()],
        materials: vec![Material {
            id: 3,
            ..Default::default()
        }],
        collections: vec![Collection {
            id: 4,
            members: vec![1],
            light_members: vec![0],
            ..Default::default()
        }],
        ..Default::default()
    };
    incoming.outliner.order = vec![
        SceneItem::Folder(4),
        SceneItem::Node(1),
        SceneItem::Light(0),
    ];
    incoming.outliner.materials.insert(1, vec![3]);
    let mut destination = incoming.clone();
    let imported = poseforge_io::merge_project(&mut destination, incoming).unwrap();
    let node = imported[0];
    let folder = destination.node_collection(node).unwrap();
    let material = destination
        .mesh(destination.node(node).unwrap().mesh.unwrap())
        .unwrap()
        .material
        .unwrap();
    assert_ne!(node, 1);
    assert_ne!(folder, 4);
    assert_ne!(material, 3);
    assert_eq!(destination.light_collection(1), Some(folder));
    assert!(destination.outliner.order.contains(&SceneItem::Node(node)));
    assert!(destination.outliner.order.contains(&SceneItem::Light(1)));
    assert_eq!(destination.outliner.materials[&node], vec![material]);
    let restored: Project =
        serde_json::from_slice(&serde_json::to_vec(&destination).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, destination);
}

#[test]
fn merge_remaps_nested_folder_parents_and_object_bindings() {
    let mut target=Node::new(1,"Empty"); target.empty=true;
    let mut camera=Node::new(2,"Camera"); camera.camera=Some(CameraLens::default());
    camera.curve_binding=Some(CurveBinding {curve:1,..Default::default()});
    let incoming=Project {nodes:vec![target,camera],collections:vec![
        Collection{id:3,..Default::default()},Collection{id:4,parent:Some(3),members:vec![2],..Default::default()}],..Default::default()};
    let mut destination=incoming.clone(); let roots=poseforge_io::merge_project(&mut destination,incoming).unwrap();
    let camera=destination.node(roots[1]).unwrap(); assert_eq!(camera.curve_binding.unwrap().curve,roots[0]);
    let folder=destination.node_collection(camera.id).unwrap(); let chain=destination.collection_chain(folder);
    assert_eq!(chain.len(),2); assert_ne!(chain[0],4); assert_ne!(chain[1],3); destination.validate().unwrap();
}
