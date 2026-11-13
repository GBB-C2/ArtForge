use poseforge_core::{
    box_mesh, Light, LightKind, Material, Node, Project, RenderSettings, Texture, Vec3,
    MAX_ENABLED_LIGHTS, SCHEMA_VERSION,
};
use poseforge_io::{load_project, merge_project, save_project};

#[test]
fn bundled_starter_matches_the_embedded_meshes_and_loads_with_its_skin() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/starter-character.artforge");
    let project = load_project(path).unwrap();
    project.validate().unwrap();
    assert_eq!(project.meshes, Project::demo().meshes);
    for id in [20, 21, 22] {
        assert_eq!(project.node(id).unwrap().skin.as_ref().unwrap().joints.len(), 17);
    }
}

fn textured_scene() -> Project {
    let mut project = Project::default();
    project.textures.push(Texture {
        id: 10,
        name: "Texture".into(),
        width: 1,
        height: 1,
        rgba8: vec![127, 192, 255, 255].into(),
    });
    project.materials.push(Material {
        id: 20,
        base_color_texture: Some(10),
        metallic_roughness_texture: Some(10),
        normal_texture: Some(10),
        occlusion_texture: Some(10),
        emissive_texture: Some(10),
        normal_scale: 0.7,
        occlusion_strength: 0.5,
        emissive: [2., 1., 0.],
        emission_volume: true,
        ior: 1.8,
        coat_weight: 0.65,
        coat_roughness: 0.2,
        ..Material::default()
    });
    project
        .meshes
        .push(box_mesh(30, "Cube", Vec3::splat(0.5), Some(20)));
    let mut node = Node::new(1, "Cube");
    node.mesh = Some(30);
    project.nodes.push(node);
    project.lights[0] = Light {
        kind: LightKind::Spot,
        position: Vec3::new(2., 4., 3.),
        direction: Vec3::new(-1., -1., -1.).normalize(),
        range: 15.,
        inner_cone_angle: 0.2,
        outer_cone_angle: 0.6,
        casts_shadow: false,
        ..Light::default()
    };
    project.render_settings = RenderSettings {
        exposure: 1.5,
        ambient_intensity: 0.4,
        shadow_filter_radius: 2.25,
        ..RenderSettings::default()
    };
    project.validate().unwrap();
    project
}

#[test]
fn pbr_maps_lights_and_render_settings_survive_self_contained_save() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("materials-and-lights.poseforge");
    let project = textured_scene();
    save_project(&project, &path).unwrap();
    assert_eq!(load_project(path).unwrap(), project);
}

#[test]
fn earlier_schema_fields_migrate_without_rewriting_source_or_changing_lighting() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("legacy.poseforge");
    for version in [1, 2, 3] {
        let mut data = serde_json::to_value(textured_scene()).unwrap();
        data["version"] = serde_json::json!(version);
        data.as_object_mut().unwrap().remove("render_settings");
        for material in data["materials"].as_array_mut().unwrap() {
            for key in [
                "metallic_roughness_texture",
                "normal_texture",
                "normal_scale",
                "occlusion_texture",
                "occlusion_strength",
                "emissive_texture",
                "ior",
                "coat_weight",
                "coat_roughness",
            ] {
                material.as_object_mut().unwrap().remove(key);
            }
        }
        for light in data["lights"].as_array_mut().unwrap() {
            for key in [
                "kind",
                "position",
                "range",
                "area_size",
                "inner_cone_angle",
                "outer_cone_angle",
                "enabled",
                "casts_shadow",
            ] {
                light.as_object_mut().unwrap().remove(key);
            }
        }
        let bytes = serde_json::to_vec(&data).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.render_settings, RenderSettings::default());
        assert_eq!(loaded.lights[0].kind, LightKind::Directional);
        assert_eq!(
            loaded.lights[0].direction,
            textured_scene().lights[0].direction
        );
        assert_eq!(loaded.materials[0].base_color_texture, Some(10));
        assert_eq!(loaded.materials[0].normal_texture, None);
        assert_eq!(loaded.materials[0].normal_scale, 1.);
        assert_eq!(loaded.materials[0].occlusion_strength, 1.);
        assert_eq!(loaded.materials[0].ior, 1.5);
        assert_eq!(loaded.materials[0].coat_weight, 0.);
        assert_eq!(loaded.materials[0].coat_roughness, 0.1);
        assert!(loaded.lights[0].casts_shadow);
        assert_eq!(loaded.lights[0].area_size, [1., 1.]);
        assert_eq!(loaded.render_settings.shadow_filter_radius, 1.);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn repeated_import_remaps_all_maps_and_preserves_destination_render_settings() {
    let incoming = textured_scene();
    let mut destination = Project::default();
    let settings = destination.render_settings;
    merge_project(&mut destination, incoming.clone()).unwrap();
    merge_project(&mut destination, incoming).unwrap();
    assert_eq!(destination.materials.len(), 2);
    assert_ne!(
        destination.materials[0].base_color_texture,
        destination.materials[1].base_color_texture
    );
    for material in &destination.materials {
        for slot in [
            material.metallic_roughness_texture,
            material.normal_texture,
            material.occlusion_texture,
            material.emissive_texture,
        ] {
            assert_eq!(slot, material.base_color_texture);
            assert!(destination
                .textures
                .iter()
                .any(|texture| Some(texture.id) == slot));
        }
    }
    assert_eq!(destination.lights.len(), 3);
    assert_eq!(destination.lights[1].kind, LightKind::Spot);
    assert_eq!(destination.render_settings, settings);
    destination.validate().unwrap();
}

#[test]
fn too_many_imported_lights_leave_destination_untouched() {
    let mut destination = textured_scene();
    destination.lights = vec![Light::default(); MAX_ENABLED_LIGHTS];
    let before = destination.clone();
    assert!(merge_project(&mut destination, textured_scene()).is_err());
    assert_eq!(destination, before);
    let mut incoming = textured_scene();
    incoming.lights[0].enabled = false;
    merge_project(&mut destination, incoming).unwrap();
    assert_eq!(destination.lights.len(), MAX_ENABLED_LIGHTS + 1);
    destination.validate().unwrap();
}

#[test]
fn duplicate_subtree_keeps_shared_material_and_all_map_references_valid() {
    let mut project = textured_scene();
    let materials = project.materials.clone();
    let textures = project.textures.clone();
    let root = project.duplicate_subtree(1).unwrap();
    assert_eq!(
        project.node(root).unwrap().mesh,
        project.node(1).unwrap().mesh
    );
    assert_eq!(project.materials, materials);
    assert_eq!(project.textures, textures);
    project.validate().unwrap();
}

#[test]
fn material_studio_example_opens_with_all_maps_lights_and_original_animation() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let studio = load_project(examples.join("material-studio.poseforge")).unwrap();
    let legacy = load_project(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project_v1.poseforge")).unwrap();
    assert_eq!(studio.version, SCHEMA_VERSION);
    assert_eq!(studio.nodes, legacy.nodes);
    assert_eq!(studio.meshes, legacy.meshes);
    assert_eq!(studio.clips, legacy.clips);
    assert_eq!(studio.textures.len(), 5);
    let body = studio.material(200).unwrap();
    let slots = [
        body.base_color_texture,
        body.metallic_roughness_texture,
        body.normal_texture,
        body.occlusion_texture,
        body.emissive_texture,
    ];
    assert!(slots.iter().all(Option::is_some));
    assert_eq!(
        studio
            .lights
            .iter()
            .map(|light| light.kind)
            .collect::<Vec<_>>(),
        vec![LightKind::Directional, LightKind::Point, LightKind::Spot]
    );
    assert_eq!(studio.rig_profiles.len(), 1);
    assert_eq!(studio.rig_profiles[0].assignments.len(), 17);
}

#[test]
fn rectangular_emitters_survive_save_load_and_merge() {
    let mut scene = textured_scene();
    scene.lights[0].kind = LightKind::Area;
    scene.lights[0].area_size = [3.2, 0.6];
    scene.lights[0].intensity = 4.5;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("area-light.poseforge");
    save_project(&scene, &path).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.lights, scene.lights);
    let mut destination = Project::default();
    merge_project(&mut destination, loaded).unwrap();
    assert!(destination.lights.contains(&scene.lights[0]));
}

#[test]
fn schema_34_emission_migrates_without_enabling_fog_or_rewriting_file() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("emission-v34.artforge");
    let mut source = serde_json::to_value(textured_scene()).unwrap();
    source["version"] = 34.into();
    source["materials"][0].as_object_mut().unwrap().remove("emission_volume");
    let bytes = serde_json::to_vec(&source).unwrap();
    std::fs::write(&path,&bytes).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version,SCHEMA_VERSION);
    assert_eq!(loaded.materials[0].emissive,[2.,1.,0.]);
    assert!(!loaded.materials[0].emission_volume);
    assert_eq!(std::fs::read(path).unwrap(),bytes);
}
