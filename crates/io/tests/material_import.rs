use poseforge_core::LightKind;
use poseforge_io::{import_gltf, load_texture};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn textured_fixture(dir: &Path) -> serde_json::Value {
    for name in ["skinned_triangle.bin", "checker.png"] {
        fs::copy(fixture(name), dir.join(name)).unwrap();
    }
    serde_json::from_slice(&fs::read(fixture("skinned_triangle.gltf")).unwrap()).unwrap()
}
fn write_gltf(dir: &Path, value: &serde_json::Value) -> PathBuf {
    let path = dir.join("material.gltf");
    fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

#[test]
fn all_pbr_maps_and_strengths_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = textured_fixture(dir.path());
    value["materials"][0]["pbrMetallicRoughness"]["metallicRoughnessTexture"] =
        serde_json::json!({"index":0});
    value["materials"][0]["normalTexture"] = serde_json::json!({"index":0,"scale":0.65});
    value["materials"][0]["occlusionTexture"] = serde_json::json!({"index":0,"strength":0.4});
    value["materials"][0]["emissiveTexture"] = serde_json::json!({"index":0});
    value["materials"][0]["emissiveFactor"] = serde_json::json!([0.2, 0.4, 0.6]);
    value["extensionsUsed"] = serde_json::json!(["KHR_materials_emissive_strength"]);
    value["extensionsRequired"] = serde_json::json!(["KHR_materials_emissive_strength"]);
    value["materials"][0]["extensions"] =
        serde_json::json!({"KHR_materials_emissive_strength":{"emissiveStrength":5.0}});
    let result = import_gltf(write_gltf(dir.path(), &value)).unwrap();
    let material = &result.project.materials[0];
    assert!(material.base_color_texture.is_some());
    assert_eq!(
        material.metallic_roughness_texture,
        material.base_color_texture
    );
    assert_eq!(material.normal_texture, material.base_color_texture);
    assert_eq!(material.occlusion_texture, material.base_color_texture);
    assert_eq!(material.emissive_texture, material.base_color_texture);
    assert_eq!(material.normal_scale, 0.65);
    assert_eq!(material.occlusion_strength, 0.4);
    assert_eq!(material.emissive, [1.0, 2.0, 3.0]);
    assert!(
        result.project.lights.is_empty(),
        "model-only imports must not duplicate studio lighting"
    );
}

#[test]
fn unsupported_uv_is_reported_instead_of_sampling_the_wrong_uv() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = textured_fixture(dir.path());
    value["materials"][0]["normalTexture"] = serde_json::json!({"index":0,"texCoord":1});
    let result = import_gltf(write_gltf(dir.path(), &value)).unwrap();
    assert!(result.project.materials[0].normal_texture.is_none());
    assert!(result
        .report
        .warnings
        .iter()
        .any(|v| v.contains("TEXCOORD_1") && v.contains("пропущена")));
}

#[test]
fn missing_data_map_uses_neutral_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = textured_fixture(dir.path());
    value["materials"][0]["normalTexture"] = serde_json::json!({"index":0});
    fs::remove_file(dir.path().join("checker.png")).unwrap();
    let result = import_gltf(write_gltf(dir.path(), &value)).unwrap();
    assert!(result.project.materials[0].base_color_texture.is_none());
    assert!(result.project.materials[0].normal_texture.is_none());
    assert_eq!(result.report.missing_textures, ["checker.png"]);
}

#[test]
fn oversized_gltf_texture_is_rejected_before_pixel_decode_not_hidden_as_missing() {
    let dir = tempfile::tempdir().unwrap();
    let value = textured_fixture(dir.path());
    image::RgbaImage::from_pixel(8193, 1, image::Rgba([128, 0, 0, 255]))
        .save(dir.path().join("checker.png"))
        .unwrap();
    assert!(matches!(
        import_gltf(write_gltf(dir.path(), &value)),
        Err(poseforge_io::IoError::Limit(_))
    ));
}

#[test]
fn embedded_gltf_png_has_the_same_pixels_as_external_texture() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = textured_fixture(dir.path());
    let expected = import_gltf(write_gltf(dir.path(), &value))
        .unwrap()
        .project
        .textures[0]
        .rgba8
        .clone();
    let png = fs::read(dir.path().join("checker.png")).unwrap();
    let bin_path = dir.path().join("skinned_triangle.bin");
    let mut buffer = fs::read(&bin_path).unwrap();
    while !buffer.len().is_multiple_of(4) {
        buffer.push(0);
    }
    let offset = buffer.len();
    buffer.extend_from_slice(&png);
    fs::write(&bin_path, &buffer).unwrap();
    value["buffers"][0]["byteLength"] = serde_json::json!(buffer.len());
    let views = value["bufferViews"].as_array_mut().unwrap();
    let index = views.len();
    views.push(serde_json::json!({"buffer":0,"byteOffset":offset,"byteLength":png.len()}));
    value["images"][0] = serde_json::json!({"bufferView":index,"mimeType":"image/png"});
    fs::remove_file(dir.path().join("checker.png")).unwrap();
    let result = import_gltf(write_gltf(dir.path(), &value)).unwrap();
    assert_eq!(result.project.textures[0].rgba8, expected);
    assert!(result.report.missing_textures.is_empty());
}

#[test]
fn required_and_optional_texture_transforms_survive_all_five_slots_glb_and_save() {
    use poseforge_core::{Project, TextureUvTransform};
    for required in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut value = textured_fixture(dir.path());
        value["extensionsUsed"] = serde_json::json!(["KHR_texture_transform"]);
        if required {
            value["extensionsRequired"] = serde_json::json!(["KHR_texture_transform"]);
        }
        let mappings: [TextureUvTransform; 5] = std::array::from_fn(|i| TextureUvTransform {
            offset: [i as f32 * 0.17, -0.3],
            rotation: i as f32 * 0.4,
            scale: [0.5 + i as f32, if i == 2 { -1. } else { 2. }],
        });
        let paths = [
            "/materials/0/pbrMetallicRoughness/baseColorTexture",
            "/materials/0/pbrMetallicRoughness/metallicRoughnessTexture",
            "/materials/0/normalTexture",
            "/materials/0/occlusionTexture",
            "/materials/0/emissiveTexture",
        ];
        value["materials"][0]["pbrMetallicRoughness"]["metallicRoughnessTexture"] =
            serde_json::json!({});
        value["materials"][0]["normalTexture"] = serde_json::json!({});
        value["materials"][0]["occlusionTexture"] = serde_json::json!({});
        value["materials"][0]["emissiveTexture"] = serde_json::json!({});
        for (path, mapping) in paths.iter().zip(mappings) {
            // The extension override must take priority over textureInfo.texCoord.
            *value.pointer_mut(path).unwrap() = serde_json::json!({"index":0,"texCoord":1,"extensions":{"KHR_texture_transform":{"offset":mapping.offset,"rotation":mapping.rotation,"scale":mapping.scale,"texCoord":0}}});
        }
        value["materials"][0]["normalTexture"]["scale"] = serde_json::json!(0.65);
        value["materials"][0]["occlusionTexture"]["strength"] = serde_json::json!(0.4);
        let path = write_gltf(dir.path(), &value);
        let imported = import_gltf(&path).unwrap();
        let material = &imported.project.materials[0];
        assert_eq!(material.texture_uvs, mappings);
        assert_eq!(material.normal_scale, 0.65);
        assert_eq!(material.occlusion_strength, 0.4);
        assert!(
            material.base_color_texture.is_some()
                && material.metallic_roughness_texture.is_some()
                && material.normal_texture.is_some()
                && material.occlusion_texture.is_some()
                && material.emissive_texture.is_some()
        );
        assert!(
            imported.report.warnings.is_empty(),
            "{:?}",
            imported.report.warnings
        );
        let mut merged = Project::default();
        poseforge_io::merge_project(&mut merged, imported.project).unwrap();
        let saved = dir.path().join("transformed.poseforge");
        poseforge_io::save_project(&merged, &saved).unwrap();
        assert_eq!(
            poseforge_io::load_project(saved).unwrap().materials[0].texture_uvs,
            mappings
        );

        // Exercise the binary container, including a required extension.
        value["buffers"][0].as_object_mut().unwrap().remove("uri");
        let mut json = serde_json::to_vec(&value).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut bin = fs::read(dir.path().join("skinned_triangle.bin")).unwrap();
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let mut glb = b"glTF".to_vec();
        glb.extend(2u32.to_le_bytes());
        glb.extend((28u32 + json.len() as u32 + bin.len() as u32).to_le_bytes());
        glb.extend((json.len() as u32).to_le_bytes());
        glb.extend(b"JSON");
        glb.extend(json);
        glb.extend((bin.len() as u32).to_le_bytes());
        glb.extend(b"BIN\0");
        glb.extend(bin);
        let path = dir.path().join("transformed.glb");
        fs::write(&path, glb).unwrap();
        assert_eq!(
            import_gltf(path).unwrap().project.materials[0].texture_uvs,
            mappings
        );
    }
}

#[test]
fn texture_transform_can_select_an_alternate_uv_set_and_rejects_a_missing_set() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = textured_fixture(dir.path());
    value["extensionsUsed"] = serde_json::json!(["KHR_texture_transform"]);
    value["extensionsRequired"] = serde_json::json!(["KHR_texture_transform"]);
    value["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["extensions"] =
        serde_json::json!({"KHR_texture_transform":{"texCoord":1,"scale":[-1,0]}});
    assert!(import_gltf(write_gltf(dir.path(), &value))
        .unwrap_err()
        .to_string()
        .contains("TEXCOORD_1"));
    let uv = [[0.2f32, 0.8], [0.6, 0.1], [0.9, 0.3]];
    let bin_path = dir.path().join("skinned_triangle.bin");
    let mut buffer = fs::read(&bin_path).unwrap();
    let offset = buffer.len();
    for v in uv.iter().flatten() {
        buffer.extend(v.to_le_bytes());
    }
    fs::write(&bin_path, &buffer).unwrap();
    value["buffers"][0]["byteLength"] = serde_json::json!(buffer.len());
    let views = value["bufferViews"].as_array_mut().unwrap();
    let view = views.len();
    views.push(serde_json::json!({"buffer":0,"byteOffset":offset,"byteLength":24}));
    let accessors = value["accessors"].as_array_mut().unwrap();
    let accessor = accessors.len();
    accessors
        .push(serde_json::json!({"bufferView":view,"componentType":5126,"type":"VEC2","count":3}));
    value["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_1"] = serde_json::json!(accessor);
    let imported = import_gltf(write_gltf(dir.path(), &value)).unwrap();
    assert_eq!(
        imported.project.meshes[0]
            .vertices
            .iter()
            .map(|v| v.uv)
            .collect::<Vec<_>>(),
        uv
    );
    assert!(imported.project.materials[0].base_color_texture.is_some());
    assert_eq!(
        imported.project.materials[0].texture_uvs[0].scale,
        [-1., 0.]
    );
}

#[test]
fn malformed_normal_and_occlusion_transforms_fail_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = textured_fixture(dir.path());
    value["extensionsUsed"] = serde_json::json!(["KHR_texture_transform"]);
    value["extensionsRequired"] = serde_json::json!(["KHR_texture_transform"]);
    for slot in ["normalTexture", "occlusionTexture"] {
        for bad in [
            serde_json::json!({"scale":[1]}),
            serde_json::json!({"texCoord":-1}),
            serde_json::json!({"rotation":"invalid"}),
        ] {
            value["materials"][0][slot] =
                serde_json::json!({"index":0,"extensions":{"KHR_texture_transform":bad}});
            let error = import_gltf(write_gltf(dir.path(), &value)).unwrap_err();
            assert!(
                error.to_string().contains("KHR_texture_transform"),
                "{error}"
            );
        }
        value["materials"][0].as_object_mut().unwrap().remove(slot);
    }
}

#[test]
fn schema14_materials_migrate_with_identity_per_map_uv_without_rewriting_source() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = poseforge_core::Project::default();
    p.materials.push(poseforge_core::Material {
        id: 1,
        ..Default::default()
    });
    let mut value = serde_json::to_value(p).unwrap();
    value["version"] = serde_json::json!(14);
    value["materials"][0]
        .as_object_mut()
        .unwrap()
        .remove("texture_uvs");
    let source = serde_json::to_vec(&value).unwrap();
    let path = dir.path().join("old.poseforge");
    fs::write(&path, &source).unwrap();
    let loaded = poseforge_io::load_project(&path).unwrap();
    assert_eq!(loaded.version, poseforge_core::SCHEMA_VERSION);
    assert!(loaded.materials[0]
        .texture_uvs
        .iter()
        .all(|uv| *uv == Default::default()));
    assert_eq!(fs::read(path).unwrap(), source);
}

#[test]
fn required_punctual_lights_import_parent_transform_and_properties() {
    let dir = tempfile::tempdir().unwrap();
    let value = serde_json::json!({
        "asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
        "extensionsUsed":["KHR_lights_punctual"],"extensionsRequired":["KHR_lights_punctual"],
        "extensions":{"KHR_lights_punctual":{"lights":[{"name":"Spot","type":"spot","color":[1.0,0.5,0.25],"intensity":25.0,"range":12.0,"spot":{"innerConeAngle":0.2,"outerConeAngle":0.6}}]}},
        "nodes":[{"translation":[2.0,0.0,0.0],"children":[1]},{"translation":[0.0,3.0,0.0],"extensions":{"KHR_lights_punctual":{"light":0}}}]
    });
    let result = import_gltf(write_gltf(dir.path(), &value)).unwrap();
    assert_eq!(result.report.lights, 1);
    let light = &result.project.lights[0];
    assert_eq!(light.kind, LightKind::Spot);
    assert_eq!(light.position.to_array(), [2.0, 3.0, 0.0]);
    assert_eq!(light.direction.to_array(), [0.0, 0.0, -1.0]);
    assert_eq!(light.intensity, 25.0);
    assert_eq!(light.range, 12.0);
    assert_eq!(light.inner_cone_angle, 0.2);
    assert_eq!(light.outer_cone_angle, 0.6);
}

#[test]
fn direct_texture_loader_checks_format_and_dimensions_before_commit() {
    let texture = load_texture(fixture("checker.png")).unwrap();
    assert_eq!(texture.id, 0);
    assert_eq!(texture.width, 2);
    assert_eq!(texture.height, 2);
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake.png");
    fs::write(&fake, b"not a PNG").unwrap();
    assert!(load_texture(fake).is_err());
    let large = dir.path().join("wide.png");
    image::RgbaImage::new(8193, 1).save(&large).unwrap();
    assert!(load_texture(large).is_err());
}

#[test]
fn unsupported_layer_extensions_are_explicit_and_preserve_the_base_material() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = textured_fixture(dir.path());
    value["extensionsUsed"] = serde_json::json!(["KHR_materials_ior", "KHR_materials_clearcoat"]);
    value["materials"][0]["extensions"] = serde_json::json!({
        "KHR_materials_ior": {"ior": 1.8},
        "KHR_materials_clearcoat": {"clearcoatFactor": 0.9, "clearcoatRoughnessFactor": 0.2}
    });
    let result = import_gltf(write_gltf(dir.path(), &value)).unwrap();
    let material = &result.project.materials[0];
    assert!(material.base_color_texture.is_some());
    assert_eq!(material.ior, 1.5);
    assert_eq!(material.coat_weight, 0.);
    for extension in ["KHR_materials_ior", "KHR_materials_clearcoat"] {
        assert!(result
            .report
            .warnings
            .iter()
            .any(|warning| warning.contains(extension)));
    }
}
