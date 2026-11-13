use poseforge_core::*;

#[test]
fn light_deletion_remaps_folder_membership_order_and_undo() {
    let mut p = Project::default();
    p.lights.extend([
        Light {
            name: "B".into(),
            ..Default::default()
        },
        Light {
            name: "C".into(),
            ..Default::default()
        },
    ]);
    p.collections.push(Collection {
        id: 10,
        light_members: vec![1, 2],
        ..Default::default()
    });
    p.outliner.order = vec![
        SceneItem::Light(2),
        SceneItem::Folder(10),
        SceneItem::Light(1),
        SceneItem::Light(0),
    ];
    let before = p.clone();
    let mut e = Editor::new(p);
    e.transact("Remove", |p| p.remove_light(1)).unwrap();
    assert_eq!(e.project.lights[1].name, "C");
    assert_eq!(e.project.collections[0].light_members, vec![1]);
    assert_eq!(
        e.project.outliner.order,
        vec![
            SceneItem::Light(1),
            SceneItem::Folder(10),
            SceneItem::Light(0)
        ]
    );
    e.undo();
    assert_eq!(e.project, before);
}

#[test]
fn folder_light_visibility_export_and_lock_are_independent() {
    let mut p = Project::default();
    p.collections.push(Collection {
        id: 10,
        light_members: vec![0],
        visible: false,
        locked: true,
        ..Default::default()
    });
    assert!(!p.light_visible(0, false));
    assert!(p.light_visible(0, true));
    assert!(p.light_locked(0));
    p.collections[0].render_visible = false;
    assert!(!p.light_visible(0, true));
    p.collections[0].visible = true;
    p.lights[0].enabled = false;
    assert!(!p.light_visible(0, false));
    p.collections[0].light_members.push(0);
    assert!(p.validate().is_err());
}

#[test]
fn replacing_one_material_preserves_other_faces_and_isolates_shared_instances() {
    let mut mesh = PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(10, Some(20))
        .unwrap();
    mesh.topology.as_mut().unwrap().face_materials =
        vec![Some(21), None, Some(21), None, None, None].into();
    let mut one = Node::new(1, "A");
    one.mesh = Some(10);
    let mut two = one.clone();
    two.id = 2;
    let mut e = Editor::new(Project {
        nodes: vec![one, two],
        meshes: vec![mesh],
        materials: (20..23)
            .map(|id| Material {
                id,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    e.project.outliner.materials.insert(1, vec![21, 20]);
    let before = e.project.clone();
    let output = e.replace_assigned_material(10, &[1], 21, 22).unwrap();
    assert_ne!(output.mesh, 10);
    assert_eq!(e.project.node(2).unwrap().mesh, Some(10));
    let edited = e.project.mesh(output.mesh).unwrap();
    assert_eq!(edited.face_material(0), Some(22));
    assert_eq!(edited.face_material(1), Some(20));
    assert!(edited.vertices.shares_storage(&before.meshes[0].vertices));
    assert_eq!(e.project.outliner.materials[&1], vec![22, 20]);
    assert_eq!(e.project.mesh(10), before.mesh(10));
    assert_eq!(e.undo_len(), 1);
    e.undo();
    assert_eq!(e.project, before);
}

#[test]
fn deleted_nodes_clean_presentation_metadata_without_changing_other_selection() {
    let mut e = Editor::new(Project {
        nodes: vec![Node::new(1, "A"), Node::new(2, "B")],
        ..Default::default()
    });
    e.project.outliner.order = vec![SceneItem::Node(2), SceneItem::Node(1)];
    e.selected = Some(1);
    let before = e.project.clone();
    e.delete_node(2).unwrap();
    assert_eq!(e.selected, Some(1));
    assert_eq!(e.project.outliner.order, vec![SceneItem::Node(1)]);
    e.undo();
    assert_eq!(e.project, before);
    assert_eq!(e.selected, Some(1));
}
