use poseforge_core::Project;
use poseforge_io::{
    import_gltf, import_gltf_with_progress, load_project, merge_project, save_project, IoError,
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn gltf_and_glb_preserve_skin_order_morph_and_animation() {
    for name in ["skinned_triangle.gltf", "skinned_triangle.glb"] {
        let imported = import_gltf(fixture(name)).unwrap();
        let p = imported.project;
        assert_eq!(imported.report.joints, 2);
        assert_eq!(imported.report.clips, 1);
        assert!(imported.report.missing_textures.is_empty());
        assert_eq!(p.node(3).unwrap().skin.as_ref().unwrap().joints, vec![2, 1]);
        assert_eq!(p.meshes[0].morph_targets.len(), 1);
        assert_eq!(p.clips[0].channels[0].keys[1].value, vec![0., 2., 0.]);
        assert_eq!(p.textures[0].rgba8.len(), 16);
        p.validate().unwrap();
    }
}

#[test]
fn save_roundtrip_unicode_and_failed_save_keeps_previous_file() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = dir.path().join("проект.poseforge");
    let mut p = import_gltf(fixture("skinned_triangle.glb"))
        .unwrap()
        .project;
    p.title = "Персонаж — тест".into();
    save_project(&p, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), p);
    let old = std::fs::read(&path).unwrap();
    p.nodes[0].transform.translation.x = f32::NAN;
    assert!(save_project(&p, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn importing_same_character_twice_remaps_skin_and_channels() {
    let imported = import_gltf(fixture("skinned_triangle.glb"))
        .unwrap()
        .project;
    let mut p = Project::default();
    let first = merge_project(&mut p, imported.clone()).unwrap();
    let second = merge_project(&mut p, imported).unwrap();
    assert_ne!(first, second);
    assert_eq!(p.nodes.len(), 6);
    assert_eq!(p.clips.len(), 2);
    let skin_a = p
        .nodes
        .iter()
        .filter_map(|n| n.skin.as_ref())
        .next()
        .unwrap()
        .clone();
    let skin_b = p
        .nodes
        .iter()
        .filter_map(|n| n.skin.as_ref())
        .nth(1)
        .unwrap()
        .clone();
    assert!(skin_a.joints.iter().all(|j| !skin_b.joints.contains(j)));
    assert_ne!(p.clips[0].channels[0].node, p.clips[1].channels[0].node);
    let original = p.node(skin_a.joints[0]).unwrap().transform;
    p.node_mut(skin_b.joints[0])
        .unwrap()
        .transform
        .translation
        .x = 9.;
    assert_eq!(p.node(skin_a.joints[0]).unwrap().transform, original);
    p.validate().unwrap();
}

fn copy_fixture(dir: &Path) -> serde_json::Value {
    std::fs::copy(
        fixture("skinned_triangle.bin"),
        dir.join("skinned_triangle.bin"),
    )
    .unwrap();
    serde_json::from_slice(&std::fs::read(fixture("skinned_triangle.gltf")).unwrap()).unwrap()
}

#[test]
fn missing_texture_is_reported_and_does_not_destroy_geometry() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let json = copy_fixture(dir.path());
    let path = dir.path().join("test.gltf");
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let result = import_gltf(path).unwrap();
    assert_eq!(result.report.missing_textures, vec!["checker.png"]);
    assert_eq!(result.project.meshes[0].indices, vec![0, 1, 2]);
    assert!(result.project.textures.is_empty());
    assert_eq!(result.report.textures, 0);
    assert!(result.project.materials[0].base_color_texture.is_none());
}

#[test]
fn unsupported_required_extension_is_not_silently_ignored() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let mut json = copy_fixture(dir.path());
    json["extensionsUsed"] = serde_json::json!(["KHR_draco_mesh_compression"]);
    json["extensionsRequired"] = serde_json::json!(["KHR_draco_mesh_compression"]);
    let path = dir.path().join("required.gltf");
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(import_gltf(path).is_err());
}

#[test]
fn cancellation_happens_before_reading_and_truncated_buffer_is_rejected() {
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        import_gltf_with_progress("does-not-exist.glb", &cancelled, |_, _| {}),
        Err(IoError::Cancelled)
    ));
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let json = copy_fixture(dir.path());
    let path = dir.path().join("truncated.gltf");
    std::fs::write(dir.path().join("skinned_triangle.bin"), [0u8; 4]).unwrap();
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(import_gltf(path).is_err());
}

#[test]
fn future_schema_and_invalid_json_are_rejected() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = dir.path().join("future.poseforge");
    let p = Project {
        version: 999,
        ..Default::default()
    };
    std::fs::write(&path, serde_json::to_vec(&p).unwrap()).unwrap();
    assert!(load_project(&path).is_err());
    std::fs::write(&path, b"{broken").unwrap();
    assert!(load_project(path).is_err());
}

#[test]
fn cubic_animation_keeps_original_tangents_and_evaluation() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let mut json = copy_fixture(dir.path());
    let bin_path = dir.path().join("skinned_triangle.bin");
    let mut bin = std::fs::read(&bin_path).unwrap();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let offset = bin.len();
    for value in [
        0f32, 0., 0., 0., 1., 0., 0., 4., 0., 0., 0., 0., 0., 2., 0., 0., 0., 0.,
    ] {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    let views = json["bufferViews"].as_array_mut().unwrap();
    let view = views.len();
    views.push(serde_json::json!({"buffer":0,"byteOffset":offset,"byteLength":72}));
    let accessors = json["accessors"].as_array_mut().unwrap();
    let accessor = accessors.len();
    accessors
        .push(serde_json::json!({"bufferView":view,"componentType":5126,"type":"VEC3","count":6}));
    json["animations"][0]["samplers"][0]["interpolation"] = serde_json::json!("CUBICSPLINE");
    json["animations"][0]["samplers"][0]["output"] = serde_json::json!(accessor);
    json["buffers"][0]["byteLength"] = serde_json::json!(bin.len());
    std::fs::write(bin_path, bin).unwrap();
    let path = dir.path().join("cubic.gltf");
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let p = import_gltf(path).unwrap().project;
    let c = &p.clips[0].channels[0];
    assert_eq!(
        c.keys[0].out_tangent.as_deref(),
        Some([0., 4., 0.].as_slice())
    );
    assert!((c.sample(0.5).unwrap()[1] - 2.).abs() < 1e-6);
}

#[test]
fn duplicate_influences_across_sets_are_combined_without_losing_the_skeleton() {
    let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let mut json = copy_fixture(dir.path());
    let bin_path = dir.path().join("skinned_triangle.bin");
    let mut bin = std::fs::read(&bin_path).unwrap();
    let weight_view = json["accessors"][4]["bufferView"].as_u64().unwrap() as usize;
    let offset = json["bufferViews"][weight_view]["byteOffset"]
        .as_u64()
        .unwrap() as usize;
    for i in 0..12 {
        bin[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&0.25f32.to_le_bytes());
    }
    // Repeated influences describe the same two bones, not eight distinct ones.
    json["meshes"][0]["primitives"][0]["attributes"]["JOINTS_1"] = serde_json::json!(3);
    json["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_1"] = serde_json::json!(4);
    std::fs::write(bin_path, bin).unwrap();
    let path = dir.path().join("weights.gltf");
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let result = import_gltf(path).unwrap();
    assert_eq!(result.report.joints, 2);
    assert!(!result.report.warnings.iter().any(|w| w.contains("влияния")));
    for vertex in &result.project.meshes[0].vertices {
        assert!(vertex.weights.iter().filter(|&&w| w > 0.).count() <= 2);
        assert!((vertex.weights.iter().sum::<f32>() - 1.).abs() < 1e-6);
    }
}
