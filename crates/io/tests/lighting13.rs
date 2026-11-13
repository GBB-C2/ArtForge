use poseforge_core::*;
use std::sync::Arc;
#[test]
fn hdr_and_ldr_import_keep_their_distinct_color_encodings() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("studio.hdr");
    let mut bytes = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 2\n".to_vec();
    bytes.extend([128, 64, 32, 132, 128, 128, 128, 129]);
    std::fs::write(&path, bytes).unwrap();
    let hdr = poseforge_io::load_environment(path).unwrap();
    assert_eq!(hdr.rgb[0], [8., 4., 2.]);
    assert_eq!(hdr.rgb[1], [1.; 3]);
    let path = directory.path().join("ordinary.png");
    image::RgbImage::from_pixel(1, 1, image::Rgb([128, 128, 128]))
        .save(&path)
        .unwrap();
    let ldr = poseforge_io::load_environment(path).unwrap();
    assert!((ldr.rgb[0][0] - 0.21586).abs() < 0.0001);
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/studio-softboxes.hdr");
    let generated = poseforge_io::load_environment(fixture).unwrap();
    assert_eq!([generated.width, generated.height], [256, 128]);
    assert!(generated.rgb.iter().flatten().any(|v| *v > 8.));
}
#[test]
fn schema_eight_migrates_and_hdri_graph_round_trip_without_source_overwrite() {
    let directory = tempfile::tempdir().unwrap();
    let old = directory.path().join("old.artforge");
    let mut json = serde_json::to_value(Project::default()).unwrap();
    json["version"] = 8.into();
    json.as_object_mut().unwrap().remove("environment_map");
    for name in [
        "hdri_enabled",
        "hdri_background",
        "hdri_strength",
        "hdri_rotation",
        "ray_reflections",
        "reflection_samples",
        "ray_shadows",
        "area_shadow_samples",
    ] {
        json["render_settings"]
            .as_object_mut()
            .unwrap()
            .remove(name);
    }
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&old, &bytes).unwrap();
    let mut project = poseforge_io::load_project(&old).unwrap();
    assert_eq!(project.version, SCHEMA_VERSION);
    assert!(!project.render_settings.ray_reflections && !project.render_settings.ray_shadows);
    assert_eq!(std::fs::read(old).unwrap(), bytes);
    project.environment_map = Some(Arc::new(EnvironmentMap {
        name: "Embedded".into(),
        width: 2,
        height: 1,
        rgb: vec![[2., 1., 0.1]; 2],
    }));
    project.materials.push(Material {
        id: 1,
        graph: Some(MaterialGraph {
            nodes: vec![MaterialNode::new(1, MaterialNodeKind::Noise, [60., 70.])],
            outputs: [Some(1), None, None, None, None, None, None, None],
            ..MaterialGraph::default()
        }),
        ..Material::default()
    });
    project.render_settings.ray_reflections = true;
    let path = directory.path().join("new.artforge");
    poseforge_io::save_project(&project, &path).unwrap();
    assert_eq!(poseforge_io::load_project(path).unwrap(), project);
    let mut editor = Editor::new(project);
    let identity = editor.project.environment_map.clone().unwrap();
    editor
        .transact("exposure", |p| p.render_settings.exposure = 1.)
        .unwrap();
    assert!(Arc::ptr_eq(
        &identity,
        editor.project.environment_map.as_ref().unwrap()
    ));
    editor.undo();
    assert!(Arc::ptr_eq(
        &identity,
        editor.project.environment_map.as_ref().unwrap()
    ));
}
