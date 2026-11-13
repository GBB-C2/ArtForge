use poseforge_core::*;

#[test]
fn effect_list_imports_old_values_without_changing_the_image() {
    let old:CameraEffects=serde_json::from_str(r#"{"enabled":true,"contrast":1.3,"vignette":0.7,"grain":0.08}"#).unwrap();
    assert!(old.list.is_none());
    assert_eq!((4..=10).filter(|&f|old.contains(f)).collect::<Vec<_>>(),vec![5,8,9]);
    assert_eq!(old.uniforms(),([0.,1.3,1.,0.],[0.7,0.08,0.,1.]));
    let mut edited=old;edited.toggle(8);
    assert_eq!(edited.vignette,0.7);assert_eq!(edited.uniforms().1,[0.,0.08,0.,1.]);
    edited.toggle(8);assert_eq!(edited.uniforms(),old.uniforms());
    let json=serde_json::to_string(&edited).unwrap();
    assert_eq!(serde_json::from_str::<CameraEffects>(&json).unwrap(),edited);
}

#[test]
fn effect_rows_keep_neutral_and_muted_values_and_reject_invalid_masks() {
    let mut effects=CameraEffects::default();
    for field in 4..=10 {
        effects.add(field).unwrap();assert!(effects.contains(field) && effects.effect_enabled(field));
        let before=effects;effects.add(field).unwrap();assert_eq!(effects,before);
        let stored=effects.field(field).unwrap().0;effects.toggle(field);
        assert_eq!(effects.field(field).unwrap().0,stored);
        effects.set(field,CameraEffects::default().field(field).unwrap().0).unwrap();
        assert!(effects.contains(field) && !effects.effect_enabled(field));
    }
    effects.validate().unwrap();assert_eq!(effects.uniforms(),([0.,1.,1.,0.],[0.,0.,0.,1.]));
    for field in 4..=10 {effects.remove(field);assert!(!effects.contains(field));}
    assert_eq!(effects.effect_list().present,0);effects.validate().unwrap();
    let before=effects;assert!(effects.add(99).is_err());assert_eq!(effects,before);
    for list in [CameraEffectList{present:128,enabled:0,..Default::default()},CameraEffectList{present:1,enabled:2,..Default::default()}] {
        effects.list=Some(list);assert!(effects.validate().is_err());
    }
}

#[test]
fn camera_effect_order_appends_moves_removes_and_survives_save() {
    let mut effects = CameraEffects::default();
    for field in [9, 4, 8] { effects.add(field).unwrap(); }
    assert_eq!(effects.ordered_fields().collect::<Vec<_>>(), [9, 4, 8]);
    effects.toggle(4);
    let values = effects.uniforms();
    assert!(effects.move_before(8, Some(9)).unwrap());
    assert_eq!(effects.ordered_fields().collect::<Vec<_>>(), [8, 9, 4]);
    assert_eq!(effects.uniforms(), values);
    assert!(!effects.move_before(8, Some(9)).unwrap());
    let before = effects;
    assert!(effects.move_before(7, None).is_err());
    assert!(effects.move_before(8, Some(7)).is_err());
    assert_eq!(effects, before);
    effects.remove(9); effects.add(9).unwrap();
    assert_eq!(effects.ordered_fields().collect::<Vec<_>>(), [8, 4, 9]);
    effects.validate().unwrap();
    let loaded: CameraEffects = serde_json::from_slice(&serde_json::to_vec(&effects).unwrap()).unwrap();
    assert_eq!(loaded, effects);
    let old: CameraEffects = serde_json::from_str(r#"{"list":{"present":49,"enabled":33}}"#).unwrap();
    old.validate().unwrap();
    assert_eq!(old.ordered_fields().collect::<Vec<_>>(), [4, 8, 9]);
    for order in [[4,4,0,0,0,0,0], [3,0,0,0,0,0,0], [7,0,0,0,0,0,0]] {
        let invalid = CameraEffects { list: Some(CameraEffectList { order, ..old.list.unwrap() }), ..old };
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn missing_effects_preserve_legacy_camera_appearance() {
    let mut json = serde_json::to_value(Camera::default()).unwrap();
    json.as_object_mut().unwrap().remove("effects");
    let camera: Camera = serde_json::from_value(json).unwrap();
    assert_eq!(camera.effects, CameraEffects::default());
    let lens: CameraLens = serde_json::from_str("{}").unwrap();
    assert_eq!(lens.effects.uniforms(), ([0., 1., 1., 0.], [0.; 4]));
}
#[test]
fn camera_effect_parameters_are_bounded_and_keep_disabled_values() {
    let mut effects = CameraEffects::default();
    for field in 4..=10 {
        let (_, low, high, _) = effects.field(field).unwrap();
        effects.set(field, low).unwrap(); effects.set(field, high).unwrap(); effects.validate().unwrap();
        let before = effects;
        for bad in [low - 1., high + 1., f32::NAN, f32::INFINITY] {
            assert!(effects.set(field, bad).is_err()); assert_eq!(effects, before);
        }
    }
    assert_eq!(effects.uniforms(), ([0., 1., 1., 0.], [0.; 4]));
    effects.enabled = true;
    assert_eq!(effects.uniforms().1, [1., 0.3, 8., 1.]);
}
#[test]
fn output_camera_uses_its_own_effects_and_undo_restores_them() {
    let mut project = Project::default();
    let mut node = Node::new(1, "Camera"); node.camera = Some(CameraLens::default());
    project.nodes.push(node); project.active_camera = Some(1);
    let mut editor = Editor::new(project);
    let original = editor.project.clone();
    editor.transact("Effects", |p| {
        let effects = &mut p.node_mut(1).unwrap().camera.as_mut().unwrap().effects;
        effects.enabled = true; effects.vignette = 0.75; effects.exposure = 1.;
    }).unwrap();
    let camera = editor.project.render_camera(&editor.project.global_transforms().unwrap());
    assert!(camera.effects.enabled); assert_eq!(camera.effects.vignette, 0.75);
    assert_eq!(CameraLens::from_camera(&camera).effects, camera.effects);
    assert!(editor.undo()); assert_eq!(editor.project, original);
}
