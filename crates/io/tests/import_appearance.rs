use poseforge_core::{
    TextureMagFilter, TextureMinFilter, TextureSampler, TextureWrap, SCHEMA_VERSION,
};
use poseforge_io::{import_gltf, load_project, save_project};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixture(dir: &Path) -> Value {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for file in ["skinned_triangle.bin", "checker.png"] {
        fs::copy(source.join(file), dir.join(file)).unwrap();
    }
    serde_json::from_slice(&fs::read(source.join("skinned_triangle.gltf")).unwrap()).unwrap()
}

fn write(dir: &Path, doc: &Value) -> PathBuf {
    let path = dir.join("appearance.gltf");
    fs::write(&path, serde_json::to_vec(doc).unwrap()).unwrap();
    path
}

fn append_view(dir: &Path, doc: &mut Value, data: &[u8], stride: Option<usize>) -> usize {
    let path = dir.join("skinned_triangle.bin");
    let mut buffer = fs::read(&path).unwrap();
    while !buffer.len().is_multiple_of(4) {
        buffer.push(0);
    }
    let offset = buffer.len();
    buffer.extend_from_slice(data);
    doc["buffers"][0]["byteLength"] = json!(buffer.len());
    fs::write(path, buffer).unwrap();
    let views = doc["bufferViews"].as_array_mut().unwrap();
    let index = views.len();
    let mut view = json!({"buffer":0, "byteOffset":offset, "byteLength":data.len()});
    if let Some(stride) = stride {
        view["byteStride"] = json!(stride);
    }
    views.push(view);
    index
}

fn colors(doc: &mut Value, accessor: Value) {
    let accessors = doc["accessors"].as_array_mut().unwrap();
    let index = accessors.len();
    accessors.push(accessor);
    doc["meshes"][0]["primitives"][0]["attributes"]["COLOR_0"] = json!(index);
}

#[test]
fn rgb_and_rgba_colors_import_normalized_and_survive_project_roundtrip() {
    for (component, dimensions, normalized, stride) in [
        (5126, "VEC3", false, 16),
        (5121, "VEC4", true, 4),
        (5123, "VEC4", true, 8),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut doc = fixture(dir.path());
        let mut data = Vec::new();
        let expected = if component == 5126 {
            [0.25, 0.5, 1., 1.]
        } else if component == 5121 {
            [0., 128. / 255., 1., 64. / 255.]
        } else {
            [0., 32768. / 65535., 1., 16384. / 65535.]
        };
        for _ in 0..3 {
            match component {
                5126 => {
                    for value in [0.25f32, 0.5, 1., 99.] {
                        data.extend(value.to_le_bytes());
                    }
                }
                5121 => data.extend([0, 128, 255, 64]),
                _ => {
                    for value in [0u16, 32768, 65535, 16384] {
                        data.extend(value.to_le_bytes());
                    }
                }
            }
        }
        let view = append_view(dir.path(), &mut doc, &data, Some(stride));
        colors(
            &mut doc,
            json!({"bufferView":view,"componentType":component,"type":dimensions,
            "normalized":normalized,"count":3}),
        );
        let imported = import_gltf(write(dir.path(), &doc)).unwrap();
        for vertex in &imported.project.meshes[0].vertices {
            assert_eq!(vertex.color, expected);
        }
        assert!(
            imported.report.warnings.is_empty(),
            "{:?}",
            imported.report.warnings
        );
        assert_eq!(imported.report.joints, 2);
        assert_eq!(imported.report.clips, 1);
        assert!(!imported.project.meshes[0].morph_targets.is_empty());
        let path = dir.path().join("saved.artforge");
        save_project(&imported.project, &path).unwrap();
        assert_eq!(load_project(path).unwrap(), imported.project);
    }
}

#[test]
fn sparse_rgb_colors_use_zero_rgb_and_opaque_alpha_for_unset_vertices() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = fixture(dir.path());
    let indices = append_view(dir.path(), &mut doc, &[1], None);
    let bytes: Vec<_> = [1f32, 0., 0.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let values = append_view(dir.path(), &mut doc, &bytes, None);
    colors(
        &mut doc,
        json!({"componentType":5126,"type":"VEC3","count":3,
        "sparse":{"count":1,"indices":{"bufferView":indices,"componentType":5121},
            "values":{"bufferView":values}}}),
    );
    let imported = import_gltf(write(dir.path(), &doc)).unwrap();
    let colors: Vec<_> = imported.project.meshes[0]
        .vertices
        .iter()
        .map(|v| v.color)
        .collect();
    assert_eq!(
        colors,
        [[0., 0., 0., 1.], [1., 0., 0., 1.], [0., 0., 0., 1.]]
    );
}

#[test]
fn invalid_colors_are_rejected_without_silently_clamping() {
    for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        let dir = tempfile::tempdir().unwrap();
        let mut doc = fixture(dir.path());
        let data: Vec<_> = [invalid, 1., 1., 1.]
            .repeat(3)
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let view = append_view(dir.path(), &mut doc, &data, None);
        colors(
            &mut doc,
            json!({"bufferView":view,"componentType":5126,"type":"VEC4","count":3}),
        );
        assert!(import_gltf(write(dir.path(), &doc))
            .unwrap_err()
            .to_string()
            .contains("COLOR_0"));
    }
    let dir = tempfile::tempdir().unwrap();
    let mut doc = fixture(dir.path());
    let view = append_view(dir.path(), &mut doc, &[255; 8], None);
    colors(
        &mut doc,
        json!({"bufferView":view,"componentType":5121,"normalized":true,"type":"VEC4","count":2}),
    );
    assert!(import_gltf(write(dir.path(), &doc))
        .unwrap_err()
        .to_string()
        .contains("не совпадает"));
}

#[test]
fn per_slot_samplers_share_pixels_but_preserve_all_filters_and_wrap_modes() {
    use TextureMinFilter::*;
    for (filter, expected) in [
        (9728, Nearest),
        (9729, Linear),
        (9984, NearestMipmapNearest),
        (9985, LinearMipmapNearest),
        (9986, NearestMipmapLinear),
        (9987, LinearMipmapLinear),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut doc = fixture(dir.path());
        doc["samplers"] =
            json!([{}, {"wrapS":33071,"wrapT":33648,"minFilter":filter,"magFilter":9728}]);
        doc["textures"] = json!([{"source":0,"sampler":0},{"source":0,"sampler":1}]);
        doc["materials"][0]["normalTexture"] = json!({"index":1});
        doc["materials"][0]["pbrMetallicRoughness"]["metallicRoughnessTexture"] =
            json!({"index":1});
        doc["materials"][0]["occlusionTexture"] = json!({"index":0});
        doc["materials"][0]["emissiveTexture"] = json!({"index":1});
        let imported = import_gltf(write(dir.path(), &doc)).unwrap();
        assert_eq!(imported.project.textures.len(), 1);
        let material = &imported.project.materials[0];
        assert_eq!(material.base_color_texture, material.normal_texture);
        assert_eq!(material.texture_samplers[0], TextureSampler::default());
        assert_eq!(material.texture_samplers[3], TextureSampler::default());
        for index in [1, 2, 4] {
            assert_eq!(
                material.texture_samplers[index],
                TextureSampler {
                    wrap_s: TextureWrap::ClampToEdge,
                    wrap_t: TextureWrap::MirroredRepeat,
                    mag_filter: TextureMagFilter::Nearest,
                    min_filter: expected,
                }
            );
        }
        assert!(
            imported.report.warnings.is_empty(),
            "{:?}",
            imported.report.warnings
        );
        let path = dir.path().join("samplers.artforge");
        save_project(&imported.project, &path).unwrap();
        assert_eq!(load_project(path).unwrap(), imported.project);
    }
}

#[test]
fn legacy_projects_and_uncolored_imports_default_to_white_and_repeat() {
    let dir = tempfile::tempdir().unwrap();
    let doc = fixture(dir.path());
    let imported = import_gltf(write(dir.path(), &doc)).unwrap();
    assert!(imported.project.meshes[0]
        .vertices
        .iter()
        .all(|v| v.color == [1.; 4]));
    let mut legacy = serde_json::to_value(&imported.project).unwrap();
    legacy["version"] = json!(19);
    for mesh in legacy["meshes"].as_array_mut().unwrap() {
        for vertex in mesh["vertices"].as_array_mut().unwrap() {
            vertex.as_object_mut().unwrap().remove("color");
        }
    }
    for material in legacy["materials"].as_array_mut().unwrap() {
        material.as_object_mut().unwrap().remove("texture_samplers");
    }
    let path = dir.path().join("legacy.artforge");
    let original = serde_json::to_vec(&legacy).unwrap();
    fs::write(&path, &original).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert_eq!(loaded, imported.project);
    assert_eq!(fs::read(path).unwrap(), original);
}
