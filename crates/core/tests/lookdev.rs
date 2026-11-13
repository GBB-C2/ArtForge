use poseforge_core::*;

#[test]
fn temperature_is_finite_unit_luminance_and_preserves_disabled_tint() {
    let warm = temperature_rgb(2500.);
    let cool = temperature_rgb(10000.);
    assert!(warm[0] > warm[2] * 3. && cool[2] > cool[0]);
    for t in (1000..=12000).step_by(17) {
        let c = temperature_rgb(t as f32);
        assert!(c.iter().all(|v| v.is_finite() && *v >= 0.));
        assert!((c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722 - 1.).abs() < 1e-5);
    }
    let mut light = Light {
        color: [0.2, 0.7, 0.9],
        temperature_kelvin: 2500.,
        ..Light::default()
    };
    assert_eq!(light.effective_color(), light.color);
    light.use_temperature = true;
    assert_eq!(
        light.effective_color(),
        std::array::from_fn(|i| light.color[i] * warm[i])
    );
}

#[test]
fn rectangle_roll_keeps_an_orthonormal_frame_and_swaps_axes_at_ninety_degrees() {
    for direction in [Vec3::Y, Vec3::NEG_Z, Vec3::new(0.4, -0.8, 0.2).normalize()] {
        let mut light = Light {
            direction,
            ..Light::default()
        };
        let (t, b) = light.area_basis();
        light.area_rotation = 90.;
        let (rt, rb) = light.area_basis();
        assert!(rt.distance(b) < 1e-5 && rb.distance(-t) < 1e-5);
        assert!(rt.dot(rb).abs() < 1e-5 && rt.dot(direction).abs() < 1e-5);
        assert!((rt.cross(rb).dot(direction) - 1.).abs() < 1e-5);
    }
}

#[test]
fn lookdev_ranges_are_validated_and_presets_preserve_texture_mapping() {
    let mut p = Project::default();
    p.materials.push(Material {
        id: 1,
        uv_scale: [3., 2.],
        uv_offset: [0.2, -0.3],
        uv_rotation: 25.,
        ..Material::default()
    });
    for preset in MaterialPreset::ALL {
        preset.apply(&mut p.materials[0]);
        assert_eq!(p.materials[0].uv_scale, [3., 2.]);
        assert_eq!(p.materials[0].uv_offset, [0.2, -0.3]);
        assert_eq!(p.materials[0].uv_rotation, 25.);
        p.validate().unwrap();
    }
    let good = p.clone();
    for value in [0., -1., 1001., f32::NAN] {
        p = good.clone();
        p.materials[0].uv_scale[0] = value;
        assert!(p.validate().is_err());
    }
    p = good.clone();
    p.render_settings.ambient_occlusion = 2.1;
    assert!(p.validate().is_err());
    p = good.clone();
    p.render_settings.ambient_occlusion_radius = 0.;
    assert!(p.validate().is_err());
    for value in [999., 12001., f32::NAN] {
        p = good.clone();
        p.lights.push(Light {
            temperature_kelvin: value,
            ..Light::default()
        });
        assert!(p.validate().is_err());
    }
}
