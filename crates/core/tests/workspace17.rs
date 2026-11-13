use poseforge_core::*;
#[test]
fn legacy_graph_outputs_and_camera_lens_defaults_migrate_without_changing_glass() {
    let mut value = serde_json::to_value(Material::default()).unwrap();
    value.as_object_mut().unwrap().remove("geometric_thickness");
    for key in ["favorite", "category", "tags"] {
        value.as_object_mut().unwrap().remove(key);
    }
    let old: Material = serde_json::from_value(value).unwrap();
    assert!(!old.geometric_thickness);
    assert!(!old.favorite);
    let graph:MaterialGraph=serde_json::from_value(serde_json::json!({"enabled":true,"nodes":[],"outputs":[null,null,null,null],"output_position":[0,0]})).unwrap();
    assert_eq!(graph.outputs, [None; 8]);
    graph.compile().unwrap();
    let lens: CameraLens = serde_json::from_str("{}").unwrap();
    assert!(!lens.depth_of_field.enabled);
    assert!(lens.thirds);
}
#[test]
fn node_surface_sockets_classify_render_pass_and_validate_independent_images() {
    let mut p = Project::default();
    let mut g = MaterialGraph::default();
    g.nodes
        .push(MaterialNode::new(1, MaterialNodeKind::Value, [0., 0.]));
    g.outputs[5] = Some(1);
    p.materials.push(Material {
        id: 1,
        graph: Some(g),
        ..Material::default()
    });
    assert!(p.materials[0].is_transparent());
    assert!(p.materials[0].uses_transmission());
    p.materials[0].graph.as_mut().unwrap().outputs[5] = None;
    p.materials[0].graph.as_mut().unwrap().outputs[4] = Some(1);
    assert!(!p.materials[0].uses_transmission());
    assert_eq!(p.materials[0].alpha_shader_mode(), -2.);
    let mut image = MaterialNode::new(2, MaterialNodeKind::Texture, [100., 0.]);
    image.texture = Some(400);
    p.materials[0].graph.as_mut().unwrap().nodes.push(image);
    assert!(p.validate().is_err());
    p.textures.push(Texture {
        id: 400,
        name: "normal".into(),
        width: 1,
        height: 1,
        rgba8: vec![128, 128, 255, 255].into(),
    });
    p.validate().unwrap();
    let restored: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    assert_eq!(restored, p);
}
#[test]
fn saved_views_reject_invalid_focus_and_keep_the_active_camera_independent() {
    let mut p = Project::default();
    p.saved_views.push(SavedView {
        name: "Detail".into(),
        camera: Camera::default(),
    });
    p.validate().unwrap();
    p.saved_views[0].camera.depth_of_field.f_stop = 0.;
    assert!(p.validate().is_err());
    p.saved_views[0].camera.depth_of_field.f_stop = 1.4;
    let restored: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    assert_eq!(restored, p);
    assert_eq!(restored.active_camera, None);
}

#[test]
fn collections_preserve_parentage_visibility_locks_and_duplicate_membership() {
    let mut p = Project::default();
    let a = Node::new(1, "parent");
    let mut b = Node::new(2, "child");
    b.parent = Some(1);
    let c = Node::new(3, "other");
    p.nodes = vec![a, b, c];
    p.collections.push(Collection {
        id: 10,
        name: "Assembly".into(),
        members: vec![1],
        visible: false,
        locked: true,
        ..Default::default()
    });
    p.validate().unwrap();
    assert!(!p.node_visible(2, false));
    assert!(p.node_visible(2, true));
    assert!(p.node_locked(2));
    assert_eq!(
        p.visible_node_ids(false),
        std::collections::BTreeSet::from([3])
    );
    assert_eq!(p.node_collection(2), Some(10));
    let saved = serde_json::to_string(&p).unwrap();
    let read: Project = serde_json::from_str(&saved).unwrap();
    assert_eq!(p, read);
    p.collections[0].locked = false;
    p.collections[0].visible = true;
    let copy = p.duplicate_subtree(1).unwrap();
    assert!(p.collections[0].members.contains(&copy));
    p.validate().unwrap();
}
