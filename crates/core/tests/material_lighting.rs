use poseforge_core::{
    Light, LightKind, Material, Project, RenderSettings, Texture, Vec3, MAX_ENABLED_LIGHTS,
};

fn material_project() -> Project {
    let mut project = Project::default();
    project.textures.push(Texture {
        id: 10,
        name: "Shared color and data image".into(),
        width: 1,
        height: 1,
        rgba8: vec![128, 128, 255, 255].into(),
    });
    project.materials.push(Material {
        id: 20,
        base_color_texture: Some(10),
        metallic_roughness_texture: Some(10),
        normal_texture: Some(10),
        occlusion_texture: Some(10),
        emissive_texture: Some(10),
        ..Material::default()
    });
    project
}

#[test]
fn every_material_map_requires_an_existing_texture() {
    let good = material_project();
    good.validate().unwrap();
    for slot in 0..5 {
        let mut bad = good.clone();
        let material = &mut bad.materials[0];
        match slot {
            0 => material.base_color_texture = Some(999),
            1 => material.metallic_roughness_texture = Some(999),
            2 => material.normal_texture = Some(999),
            3 => material.occlusion_texture = Some(999),
            _ => material.emissive_texture = Some(999),
        }
        assert!(bad.validate().is_err(), "slot {slot} must be validated");
    }
}

#[test]
fn invalid_material_factors_are_rejected_without_narrowing_gltf_scalars() {
    let mut project = material_project();
    project.materials[0].emissive = [4., 2., 1.];
    // glTF permits negative normal scaling and cutoff > 1 (fully masked).
    project.materials[0].normal_scale = -2.;
    project.materials[0].alpha_cutoff = 2.;
    project.validate().unwrap();
    let good = project.clone();
    for (normal, ao) in [(f32::INFINITY, 1.), (f32::NAN, 1.), (1., -0.1), (1., 1.1)] {
        project = good.clone();
        project.materials[0].normal_scale = normal;
        project.materials[0].occlusion_strength = ao;
        assert!(project.validate().is_err());
    }
}

#[test]
fn all_light_types_roundtrip_and_only_enabled_lights_consume_slots() {
    let mut project = Project {
        lights: vec![Light::default(); MAX_ENABLED_LIGHTS],
        ..Project::default()
    };
    project.lights[1].kind = LightKind::Point;
    project.lights[1].position = Vec3::new(1., 2., 3.);
    project.lights[1].range = 10.;
    project.lights[2].kind = LightKind::Spot;
    project.lights[2].inner_cone_angle = 0.2;
    project.lights[2].outer_cone_angle = 0.5;
    project.lights[3].kind = LightKind::Area;
    project.lights[3].area_size = [2., 0.5];
    project.lights.push(Light {
        enabled: false,
        ..Light::default()
    });
    project.validate().unwrap();
    let roundtrip: Project =
        serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert_eq!(roundtrip, project);
    project.lights.last_mut().unwrap().enabled = true;
    assert!(project.validate().is_err());
}

#[test]
fn invalid_lights_and_render_settings_are_rejected() {
    let good = Project::default();
    for light in [
        Light {
            position: Vec3::splat(f32::NAN),
            ..Light::default()
        },
        Light {
            direction: Vec3::ZERO,
            ..Light::default()
        },
        Light {
            range: -1.,
            ..Light::default()
        },
        Light {
            intensity: f32::INFINITY,
            ..Light::default()
        },
        Light {
            inner_cone_angle: 0.5,
            outer_cone_angle: 0.5,
            ..Light::default()
        },
        Light {
            outer_cone_angle: 2.,
            ..Light::default()
        },
    ] {
        let mut project = good.clone();
        project.lights = vec![light];
        assert!(project.validate().is_err());
    }
    for settings in [
        RenderSettings {
            exposure: f32::NAN,
            ambient_intensity: 1.,
            ..RenderSettings::default()
        },
        RenderSettings {
            exposure: 17.,
            ambient_intensity: 1.,
            ..RenderSettings::default()
        },
        RenderSettings {
            exposure: 0.,
            ambient_intensity: -1.,
            ..RenderSettings::default()
        },
        RenderSettings {
            exposure: 0.,
            ambient_intensity: 17.,
            ..RenderSettings::default()
        },
    ] {
        let mut project = good.clone();
        project.render_settings = settings;
        assert!(project.validate().is_err());
    }
    let mut dark = good;
    dark.lights.clear();
    dark.render_settings.ambient_intensity = 0.;
    dark.validate().unwrap();
}

#[test]
fn legacy_material_and_light_json_receive_compatible_defaults() {
    let material: Material = serde_json::from_value(serde_json::json!({
        "id": 1, "name": "Legacy", "base_color": [0.2, 0.3, 0.4, 1.],
        "metallic": 0.5, "roughness": 0.8, "emissive": [0., 0., 0.],
        "base_color_texture": null
    }))
    .unwrap();
    assert_eq!(material.metallic_roughness_texture, None);
    assert_eq!(material.normal_texture, None);
    assert_eq!(material.occlusion_texture, None);
    assert_eq!(material.emissive_texture, None);
    assert_eq!(material.normal_scale, 1.);
    assert_eq!(material.occlusion_strength, 1.);
    assert_eq!(material.ior, 1.5);
    assert_eq!(material.coat_weight, 0.);
    assert_eq!(material.coat_roughness, 0.1);
    let light: Light = serde_json::from_value(serde_json::json!({
        "name": "Legacy light", "direction": [0., -1., 0.],
        "color": [1., 0.9, 0.8], "intensity": 4.
    }))
    .unwrap();
    assert_eq!(light.kind, LightKind::Directional);
    assert_eq!(light.position, Vec3::new(0., 3., 0.));
    assert_eq!(light.range, 0.);
    assert_eq!(light.area_size, [1., 1.]);
    assert!(light.enabled);
    assert!(light.casts_shadow);
    let settings: RenderSettings = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(settings, RenderSettings::default());
}

#[test]
fn area_lights_reject_degenerate_or_nonfinite_emitter_dimensions() {
    for size in [
        [0., 1.],
        [-1., 1.],
        [1., f32::NAN],
        [f32::INFINITY, 1.],
        [1., 10001.],
    ] {
        let mut project = Project::default();
        project.lights[0].kind = LightKind::Area;
        project.lights[0].area_size = size;
        assert!(project.validate().is_err(), "invalid emitter {size:?}");
    }
    let mut project = Project::default();
    project.lights[0].kind = LightKind::Area;
    project.lights[0].area_size = [0.0001, 10000.];
    project.validate().unwrap();
}

#[test]
fn layered_material_factors_and_shadow_filter_reject_nonfinite_or_invalid_values() {
    for bad in [f32::NAN, f32::INFINITY, -0.1, 3.1] {
        let mut project = material_project();
        project.materials[0].ior = bad;
        assert!(project.validate().is_err());
    }
    for bad in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        let mut project = material_project();
        project.materials[0].coat_weight = bad;
        assert!(project.validate().is_err());
        project.materials[0].coat_weight = 0.;
        project.materials[0].coat_roughness = bad;
        assert!(project.validate().is_err());
    }
    let mut project = material_project();
    project.materials[0].coat_roughness = 0.029;
    assert!(project.validate().is_err());
    for radius in [f32::NAN, f32::INFINITY, -0.01, 4.01] {
        let settings = RenderSettings {
            shadows: false,
            shadow_filter_radius: radius,
            ..RenderSettings::default()
        };
        assert!(settings.validate().is_err());
    }
    for (ior, coat, roughness) in [(1., 0., 0.03), (3., 1., 1.)] {
        let mut project = material_project();
        let material = &mut project.materials[0];
        material.ior = ior;
        material.coat_weight = coat;
        material.coat_roughness = roughness;
        project.validate().unwrap();
    }
}
