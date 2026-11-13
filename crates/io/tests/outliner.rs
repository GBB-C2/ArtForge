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
