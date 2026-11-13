use poseforge_core::*;
use std::collections::BTreeMap;
fn layered() -> Project {
    let mut p = Project::default();
    p.nodes.push(Node::new(1, "Object"));
    let mut base = Clip::new("Base", 2.);
    base.key_transform(1, 0., Transform::default());
    base.key_transform(1, 1., Transform::from_translation(Vec3::X * 2.));
    let mut second = Clip::new("Layer", 2.);
    second.key_transform(1, 0., Transform::from_translation(Vec3::X));
    second.key_transform(1, 1., Transform::from_translation(Vec3::X * 5.));
    p.clips = vec![base, second];
    p.animation_layers.push(AnimationLayer {
        name: "Layer".into(),
        clip: 1,
        weight: 0.5,
        additive: false,
        enabled: true,
        solo: false,
    });
    p
}
fn x(p: &Project) -> f32 {
    evaluate(p, Some(0), 1., &BTreeMap::new()).unwrap().local[&1]
        .translation
        .x
}
#[test]
fn layer_blending_solo_disable_and_pose_overrides_are_deterministic() {
    let mut p = layered();
    p.validate().unwrap();
    assert_eq!(x(&p), 3.5);
    p.animation_layers[0].additive = true;
    assert_eq!(x(&p), 4.);
    p.animation_layers[0].enabled = false;
    assert_eq!(x(&p), 2.);
    let l = AnimationLayer {
        name: "Solo".into(),
        clip: 1,
        weight: 1.,
        additive: false,
        enabled: true,
        solo: true,
    };
    p.animation_layers.push(l);
    assert_eq!(x(&p), 5.);
    assert_eq!(
        evaluate(&p, None, 1., &BTreeMap::new()).unwrap().local[&1].translation,
        Vec3::ZERO
    );
    let overrides = BTreeMap::from([(1, Transform::from_translation(Vec3::X * 9.))]);
    assert_eq!(
        evaluate(&p, Some(0), 1., &overrides).unwrap().local[&1]
            .translation
            .x,
        9.
    );
}
#[test]
fn studio_fields_roundtrip_and_reject_invalid_data() {
    let mut p = layered();
    p.render_settings.background_mode = BackgroundMode::Radial;
    p.render_settings.atmosphere.density = 0.08;
    p.materials.push(MaterialPreset::Glass.create());
    let id = p.materials[0].id;
    p.meshes.push(physics_mesh(5, id, true));
    p.nodes[0].mesh = Some(5);
    p.nodes[0].physics = Some(PhysicsSettings {
        kind: PhysicsKind::Muscle,
        ..Default::default()
    });
    p.validate().unwrap();
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(p, serde_json::from_str::<Project>(&json).unwrap());
    p.animation_layers[0].clip = 99;
    assert!(p.validate().is_err());
    p.animation_layers[0].clip = 1;
    p.nodes[0].physics.as_mut().unwrap().activation = f32::NAN;
    assert!(p.validate().is_err());
    let mut render = RenderSettings::default();
    render.atmosphere.density = -1.;
    assert!(render.validate().is_err());
    let mut fluid = FluidSettings::default();
    fluid.source_size[0] = 0.;
    assert!(fluid.validate().is_err());
}
#[test]
fn cloth_and_muscle_have_outward_faces_and_finite_topology() {
    for muscle in [false, true] {
        let m = physics_mesh(1, 2, muscle);
        assert!(m.vertices.len() < 8192);
        assert!(m.indices.iter().all(|i| (*i as usize) < m.vertices.len()));
        for t in m.indices.as_chunks::<3>().0 {
            let a = &m.vertices[t[0] as usize];
            let b = &m.vertices[t[1] as usize];
            let c = &m.vertices[t[2] as usize];
            let face = (Vec3::from_array(b.position) - Vec3::from_array(a.position))
                .cross(Vec3::from_array(c.position) - Vec3::from_array(a.position));
            assert!(face.dot(Vec3::from_array(a.normal)) >= -1e-7);
        }
    }
}
