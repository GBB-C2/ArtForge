use poseforge_core::*;

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
