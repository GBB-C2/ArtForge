use poseforge_core::{Editor, FluidSettings, Project, RenderSettings, ToneMapping};

#[test]
fn old_and_partial_settings_receive_defaults_without_losing_authored_values() {
    let mut value = serde_json::to_value(Project::default()).unwrap();
    value.as_object_mut().unwrap().remove("fluid_settings");
    value["render_settings"] = serde_json::json!({"exposure": 2.5, "ambient_intensity": 0.3});
    let project: Project = serde_json::from_value(value.clone()).unwrap();
    project.validate().unwrap();
    assert_eq!(project.fluid_settings, FluidSettings::default());
    assert_eq!(project.render_settings.exposure, 2.5);
    assert_eq!(project.render_settings.ambient_intensity, 0.3);
    assert_eq!(project.render_settings.tone_mapping, ToneMapping::Aces);
    assert_eq!(project.render_settings.antialiasing, 4);
    assert_eq!(project.render_settings.output_width, 1920);
    assert_eq!(project.render_settings.output_height, 1080);
    assert!(!project.render_settings.transparent_background);
    assert!(project.render_settings.shadows);
    value["fluid_settings"] = serde_json::json!({"viscosity": 0.15});
    value["render_settings"]["antialiasing"] = serde_json::json!(1);
    let partial: Project = serde_json::from_value(value).unwrap();
    assert_eq!(partial.fluid_settings.viscosity, 0.15);
    assert_eq!(partial.fluid_settings.gravity, 9.81);
    assert_eq!(partial.fluid_settings.max_substeps, 4);
    assert_eq!(partial.render_settings.antialiasing, 1);
    partial.validate().unwrap();
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn render_and_fluid_settings_roundtrip_all_authored_fields() {
    let mut project = Project::default();
    project.render_settings = RenderSettings {
        exposure: -1.5,
        ambient_intensity: 0.6,
        antialiasing: 1,
        output_width: 2048,
        output_height: 1152,
        transparent_background: true,
        tone_mapping: ToneMapping::Reinhard,
        shadows: false,
        shadow_resolution: 2048,
        shadow_bias: 0.004,
        shadow_strength: 0.6,
        shadow_filter_radius: 2.5,
        sky_color: [0.3, 0.6, 1.2],
        ground_color: [0.3, 0.2, 0.1],
        background_color: [0.2, 0.15, 0.4],
        ..RenderSettings::default()
    };
    project.fluid_settings = FluidSettings {
        gravity: 4.9,
        viscosity: 0.12,
        vorticity: 0.8,
        boundary_friction: 0.4,
        iterations: 8,
        max_substeps: 6,
        time_scale: 0.5,
        surface_cell_size: 0.03,
        surface_iso_density: 0.7,
        ..FluidSettings::default()
    };
    for tone_mapping in [
        ToneMapping::Aces,
        ToneMapping::Reinhard,
        ToneMapping::Linear,
    ] {
        project.render_settings.tone_mapping = tone_mapping;
        project.validate().unwrap();
        let serialized = serde_json::to_vec(&project).unwrap();
        let loaded: Project = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(loaded, project);
    }
    let invalid_enum = serde_json::json!({"tone_mapping": "UnknownFutureMode"});
    assert!(serde_json::from_value::<RenderSettings>(invalid_enum).is_err());
}

#[test]
fn unsafe_render_settings_are_rejected_even_when_the_feature_is_disabled() {
    let good = RenderSettings::default();
    for settings in [
        RenderSettings {
            output_width: 63,
            ..good
        },
        RenderSettings {
            output_height: 4097,
            ..good
        },
        RenderSettings {
            antialiasing: 2,
            ..good
        },
        RenderSettings {
            shadow_resolution: 4096,
            ..good
        },
        RenderSettings {
            shadows: false,
            shadow_bias: f32::NAN,
            ..good
        },
        RenderSettings {
            shadow_bias: -0.001,
            ..good
        },
        RenderSettings {
            shadow_strength: 1.01,
            ..good
        },
        RenderSettings {
            sky_color: [f32::INFINITY, 0., 0.],
            ..good
        },
        RenderSettings {
            ground_color: [-0.1, 0., 0.],
            ..good
        },
        RenderSettings {
            background_color: [1.01, 0., 0.],
            ..good
        },
    ] {
        let project = Project {
            render_settings: settings,
            ..Project::default()
        };
        assert!(
            project.validate().is_err(),
            "invalid render settings: {settings:?}"
        );
    }
    RenderSettings {
        antialiasing: 1,
        shadow_resolution: 512,
        shadow_bias: 0.02,
        shadow_strength: 1.,
        sky_color: [16.; 3],
        ground_color: [0.; 3],
        background_color: [1.; 3],
        ..good
    }
    .validate()
    .unwrap();
}

#[test]
fn unsafe_fluid_settings_are_rejected_before_runtime_allocation() {
    let good = FluidSettings::default();
    for settings in [
        FluidSettings {
            gravity: f32::NAN,
            ..good
        },
        FluidSettings {
            gravity: -1.,
            ..good
        },
        FluidSettings {
            viscosity: 0.21,
            ..good
        },
        FluidSettings {
            vorticity: f32::INFINITY,
            ..good
        },
        FluidSettings {
            vorticity: 5.1,
            ..good
        },
        FluidSettings {
            boundary_friction: 1.1,
            ..good
        },
        FluidSettings {
            iterations: 0,
            ..good
        },
        FluidSettings {
            iterations: 17,
            ..good
        },
        FluidSettings {
            max_substeps: 0,
            ..good
        },
        FluidSettings {
            max_substeps: 9,
            ..good
        },
        FluidSettings {
            time_scale: 0.,
            ..good
        },
        FluidSettings {
            surface_cell_size: 0.015,
            ..good
        },
        FluidSettings {
            surface_iso_density: 1.21,
            ..good
        },
    ] {
        let project = Project {
            fluid_settings: settings,
            ..Project::default()
        };
        assert!(
            project.validate().is_err(),
            "invalid fluid settings: {settings:?}"
        );
    }
    FluidSettings {
        gravity: 30.,
        viscosity: 0.2,
        vorticity: 5.,
        boundary_friction: 1.,
        iterations: 16,
        max_substeps: 8,
        time_scale: 2.,
        surface_cell_size: 0.025,
        surface_iso_density: 1.2,
        ..FluidSettings::default()
    }
    .validate()
    .unwrap();
}

#[test]
fn settings_gestures_are_atomic_cancelable_and_reversible() {
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    editor.begin_edit("Lighting and fluid preview");
    for exposure in [0.5, 1., 1.5] {
        editor
            .transact("Preview", |p| {
                p.render_settings.exposure = exposure;
                p.render_settings.shadow_resolution = 2048;
                p.fluid_settings.vorticity = exposure;
            })
            .unwrap();
    }
    assert_eq!(editor.undo_len(), 0);
    editor.commit_edit().unwrap();
    let committed = editor.project.clone();
    assert_eq!(editor.undo_len(), 1);
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    assert!(editor.redo());
    assert_eq!(editor.project, committed);
    editor.begin_edit("Cancelled preview");
    editor
        .transact("Preview", |p| p.fluid_settings.gravity = 0.)
        .unwrap();
    editor.cancel_edit();
    assert_eq!(editor.project, committed);
    assert_eq!(editor.undo_len(), 1);
    assert!(editor
        .transact("Invalid combination", |p| {
            p.render_settings.exposure = 3.;
            p.fluid_settings.max_substeps = 0;
        })
        .is_err());
    assert_eq!(editor.project, committed);
    assert_eq!(editor.undo_len(), 1);
}

#[test]
fn per_source_volume_settings_reject_non_finite_and_out_of_range_values() {
    let good = Project::default();
    for strength in [f32::NAN, f32::INFINITY, -0.1, 10.01] {
        let mut p = good.clone();
        p.lights[0].volume.strength = strength;
        assert!(p.validate().is_err());
    }
    for tint in [f32::NAN, -0.1, 1.01] {
        let mut p = good.clone();
        p.lights[0].volume.tint[1] = tint;
        assert!(p.validate().is_err());
    }
}
