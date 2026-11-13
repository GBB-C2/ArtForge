use poseforge_core::{Material, MaterialPreset, Project};

#[test]
fn fiber_presets_are_distinct_and_reset_without_losing_authored_absorption() {
    let brushed = MaterialPreset::BrushedMetal.create();
    let satin = MaterialPreset::Satin.create();
    let velvet = MaterialPreset::Velvet.create();
    assert!(brushed.anisotropy > 0.7 && brushed.metallic == 1.);
    assert!(satin.anisotropy > 0.5 && satin.sheen_weight > 0.);
    assert!(velvet.sheen_weight == 1. && velvet.sheen_roughness > satin.sheen_roughness);
    let mut authored = satin;
    authored.attenuation_color = Some([0.2, 0.6, 1.]);
    authored.anisotropy_rotation = 73.;
    let color = authored.base_color;
    MaterialPreset::Matte.apply(&mut authored);
    assert_eq!(authored.anisotropy, 0.);
    assert_eq!(authored.anisotropy_rotation, 0.);
    assert_eq!(authored.sheen_weight, 0.);
    assert_eq!(authored.base_color, color);
    assert_eq!(authored.attenuation_color, Some([0.2, 0.6, 1.]));
}

#[test]
fn fiber_and_absorption_values_reject_nonfinite_and_out_of_range_data() {
    let invalid = [
        Material {
            anisotropy: 0.96,
            ..Material::default()
        },
        Material {
            anisotropy: f32::NAN,
            ..Material::default()
        },
        Material {
            anisotropy_rotation: 181.,
            ..Material::default()
        },
        Material {
            anisotropy_rotation: f32::INFINITY,
            ..Material::default()
        },
        Material {
            sheen_weight: -0.1,
            ..Material::default()
        },
        Material {
            sheen_weight: f32::NAN,
            ..Material::default()
        },
        Material {
            sheen_roughness: 0.069,
            ..Material::default()
        },
        Material {
            sheen_roughness: f32::INFINITY,
            ..Material::default()
        },
        Material {
            attenuation_color: Some([1.1, 0., 0.]),
            ..Material::default()
        },
        Material {
            attenuation_color: Some([0., f32::NAN, 0.]),
            ..Material::default()
        },
    ];
    for material in invalid {
        let project = Project {
            materials: vec![material],
            ..Project::default()
        };
        assert!(project.validate().is_err());
    }
    for rotation in [-180., 180.] {
        let project = Project {
            materials: vec![Material {
                anisotropy: 0.95,
                anisotropy_rotation: rotation,
                sheen_weight: 1.,
                sheen_roughness: 0.07,
                attenuation_color: Some([0., 0.5, 1.]),
                ..Material::default()
            }],
            ..Project::default()
        };
        project.validate().unwrap();
    }
}
