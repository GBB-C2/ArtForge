use poseforge_core::{FluidSettings, Project, RenderSettings, ToneMapping, SCHEMA_VERSION};
use poseforge_io::{load_project, merge_project, save_project};

fn authored_scene() -> Project {
    Project {
        render_settings: RenderSettings {
            tone_mapping: ToneMapping::Linear,
            antialiasing: 1,
            output_width: 2048,
            output_height: 1024,
            transparent_background: true,
            shadow_resolution: 2048,
            shadow_bias: 0.004,
            shadow_strength: 0.7,
            sky_color: [0.8, 1.1, 1.8],
            background_color: [0.1, 0.2, 0.3],
            ..RenderSettings::default()
        },
        fluid_settings: FluidSettings {
            gravity: 3.5,
            viscosity: 0.12,
            vorticity: 0.7,
            boundary_friction: 0.4,
            iterations: 8,
            max_substeps: 6,
            time_scale: 0.5,
            surface_cell_size: 0.03,
            surface_iso_density: 0.6,
            ..FluidSettings::default()
        },
        ..Project::default()
    }
}

#[test]
fn authored_scene_settings_survive_a_real_project_save_and_load() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("render-fluid.poseforge");
    let project = authored_scene();
    save_project(&project, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), project);
}

#[test]
fn previous_schema_settings_load_with_defaults_without_rewriting_files() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("previous.poseforge");
    for version in [1, 2, 3] {
        let mut value = serde_json::to_value(authored_scene()).unwrap();
        value["version"] = serde_json::json!(version);
        value["render_settings"] = serde_json::json!({"exposure": 1.25, "ambient_intensity": 0.4});
        value.as_object_mut().unwrap().remove("fluid_settings");
        let original = serde_json::to_vec(&value).unwrap();
        std::fs::write(&path, &original).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.render_settings.exposure, 1.25);
        assert_eq!(loaded.render_settings.ambient_intensity, 0.4);
        assert_eq!(loaded.render_settings.shadow_resolution, 1024);
        assert_eq!(loaded.render_settings.tone_mapping, ToneMapping::Aces);
        assert_eq!(loaded.fluid_settings, FluidSettings::default());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

#[test]
fn importing_a_model_preserves_destination_render_and_fluid_authoring_settings() {
    let mut destination = authored_scene();
    let expected_render = destination.render_settings;
    let expected_fluid = destination.fluid_settings;
    merge_project(&mut destination, Project::default()).unwrap();
    assert_eq!(destination.render_settings, expected_render);
    assert_eq!(destination.fluid_settings, expected_fluid);
    let before = destination.clone();
    let mut invalid_incoming = Project::default();
    invalid_incoming.fluid_settings.max_substeps = 0;
    assert!(merge_project(&mut destination, invalid_incoming).is_err());
    assert_eq!(destination, before);
}

#[test]
fn schema_four_defaults_and_layer_merge_preserve_source_identity() {
    use poseforge_core::{AnimationLayer, Clip, Node, Transform, Vec3};
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("schema4.artforge");
    let mut old = serde_json::to_value(Project::default()).unwrap();
    old["version"] = serde_json::json!(4);
    old.as_object_mut().unwrap().remove("animation_layers");
    old["render_settings"]
        .as_object_mut()
        .unwrap()
        .remove("atmosphere");
    let bytes = serde_json::to_vec(&old).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let migrated = load_project(&path).unwrap();
    assert_eq!(migrated.version, SCHEMA_VERSION);
    assert_eq!(migrated.render_settings.atmosphere.density, 0.);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    let mut incoming = Project::default();
    incoming.nodes.push(Node::new(1, "Animated"));
    let mut clip = Clip::new("Motion", 2.);
    clip.key_transform(1, 1., Transform::from_translation(Vec3::X));
    incoming.clips.push(clip);
    incoming.animation_layers.push(AnimationLayer {
        name: "Layer".into(),
        clip: 0,
        weight: 0.4,
        additive: true,
        enabled: true,
        solo: false,
    });
    let mut destination = incoming.clone();
    merge_project(&mut destination, incoming).unwrap();
    assert_eq!(destination.animation_layers[1].clip, 1);
    assert_ne!(
        destination.clips[0].channels[0].node,
        destination.clips[1].channels[0].node
    );
    destination.validate().unwrap();
}
