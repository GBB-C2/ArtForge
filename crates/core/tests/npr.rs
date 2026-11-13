use poseforge_core::{box_mesh, Editor, Material, MaterialPreset, Node, NprSettings, Project};

#[test]
fn npr_creation_is_isolated_and_undoable_and_presets_keep_authored_data() {
    let mut p = Project::default();
    p.materials.push(Material {
        id: 1,
        ..Material::default()
    });
    p.meshes
        .push(box_mesh(2, "Shared", [1., 1., 1.].into(), Some(1)));
    for id in [3, 4] {
        let mut node = Node::new(id, "Instance");
        node.mesh = Some(2);
        p.nodes.push(node);
    }
    let mut editor = Editor::new(p.clone());
    let p = editor.project.clone();
    let created = editor
        .create_material(2, &[3], MaterialPreset::Anime)
        .unwrap();
    assert_ne!(created.mesh, 2);
    assert!(
        editor
            .project
            .material(created.material)
            .unwrap()
            .npr
            .enabled
    );
    assert_eq!(editor.project.node(4).unwrap().mesh, Some(2));
    assert_eq!(editor.project.mesh(2).unwrap().material, Some(1));
    editor.undo();
    assert_eq!(editor.project, p);
    editor.redo();
    assert!(
        editor
            .project
            .material(created.material)
            .unwrap()
            .npr
            .enabled
    );

    let mut m = Material {
        id: 99,
        name: "Authored".into(),
        base_color: [0.1, 0.3, 0.6, 0.7],
        emissive: [2., 1., 0.],
        base_color_texture: Some(100),
        normal_texture: Some(101),
        uv_scale: [2., 3.],
        ..Material::default()
    };
    let original = m.clone();
    for preset in [
        MaterialPreset::Toon,
        MaterialPreset::Anime,
        MaterialPreset::Ink,
    ] {
        preset.apply(&mut m);
        assert!(m.npr.enabled && m.npr.valid());
        assert_eq!(m.base_color, original.base_color);
        assert_eq!(m.emissive, original.emissive);
        assert_eq!(m.base_color_texture, original.base_color_texture);
        assert_eq!(m.normal_texture, original.normal_texture);
        assert_eq!(m.uv_scale, original.uv_scale);
        assert_eq!(m.name, original.name);
        assert_eq!(m.id, original.id);
    }
    m.npr.threshold = 0.37;
    MaterialPreset::Matte.apply(&mut m);
    assert!(!m.npr.enabled);
    assert_eq!(m.npr.threshold, 0.37);
}

#[test]
fn npr_validation_rejects_bad_data_even_when_disabled() {
    for npr in [
        NprSettings {
            bands: 1,
            ..NprSettings::default()
        },
        NprSettings {
            bands: 9,
            ..NprSettings::default()
        },
        NprSettings {
            threshold: f32::NAN,
            ..NprSettings::default()
        },
        NprSettings {
            softness: -0.01,
            ..NprSettings::default()
        },
        NprSettings {
            specular_size: 0.,
            ..NprSettings::default()
        },
        NprSettings {
            specular_strength: f32::INFINITY,
            ..NprSettings::default()
        },
        NprSettings {
            rim_width: 1.1,
            ..NprSettings::default()
        },
        NprSettings {
            rim_strength: -1.,
            ..NprSettings::default()
        },
        NprSettings {
            ink_width: 0.6,
            ..NprSettings::default()
        },
        NprSettings {
            shadow_color: [0., f32::NAN, 0.],
            ..NprSettings::default()
        },
        NprSettings {
            rim_color: [2., 0., 0.],
            ..NprSettings::default()
        },
        NprSettings {
            ink_color: [-0.1, 0., 0.],
            ..NprSettings::default()
        },
    ] {
        let p = Project {
            materials: vec![Material {
                npr,
                ..Material::default()
            }],
            ..Project::default()
        };
        assert!(p.validate().is_err(), "{npr:?}");
    }
}

#[test]
fn npr_does_not_refract_but_keeps_alpha_and_pbr_settings_for_toggle() {
    let mut m = MaterialPreset::Glass.create();
    m.npr.enabled = true;
    assert!(!m.uses_transmission());
    assert!(!m.is_transparent());
    assert_eq!(m.transmission, 1.);
    m.alpha_mode = poseforge_core::AlphaMode::Blend;
    assert!(m.is_transparent());
    m.npr.enabled = false;
    assert!(m.uses_transmission());
}

#[test]
fn palette_hatching_and_contours_validate_and_default_to_disabled() {
    let n = NprSettings::default();
    assert_eq!(n.palette_strength, 0.);
    assert_eq!(n.hatch_strength, 0.);
    for n in [
        NprSettings {
            palette_position: 0.,
            ..n
        },
        NprSettings {
            palette_strength: 1.1,
            ..n
        },
        NprSettings {
            palette_mid: [f32::NAN, 0., 0.],
            ..n
        },
        NprSettings {
            hatch_density: 0.,
            ..n
        },
        NprSettings {
            hatch_angle: f32::INFINITY,
            ..n
        },
        NprSettings {
            hatch_cross: -1.,
            ..n
        },
    ] {
        assert!(!n.valid());
    }
    let c = poseforge_core::NprContours::default();
    assert_eq!(c.width, 0.);
    assert!(c.valid());
    for c in [
        poseforge_core::NprContours { width: 9., ..c },
        poseforge_core::NprContours {
            depth_threshold: 0.,
            ..c
        },
        poseforge_core::NprContours {
            normal_angle: f32::NAN,
            ..c
        },
    ] {
        let mut p = Project::default();
        p.render_settings.npr_contours = c;
        assert!(p.validate().is_err());
    }
}
