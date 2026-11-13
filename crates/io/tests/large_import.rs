//! Exercise the former 256 MiB boundary with valid models, not only constants.
use poseforge_io::{import_gltf, import_gltf_with_progress, import_obj, IoError};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path, sync::atomic::AtomicBool};

const OLD_LIMIT: u64 = 256 * 1024 * 1024;
const MODEL_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

fn triangle(buffer: Value) -> Value {
    json!({
        "asset":{"version":"2.0"},
        "scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
        "buffers":[buffer],"bufferViews":[{"buffer":0,"byteLength":36}],
        "accessors":[{"bufferView":0,"componentType":5126,"type":"VEC3",
            "count":3,"min":[0,0,0],"max":[1,1,0]}]
    })
}

fn positions() -> Vec<u8> {
    [0_f32, 0., 0., 1., 0., 0., 0., 1., 0.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect()
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

#[test]
fn valid_glb_external_gltf_and_obj_above_256_mib_import() {
    let dir = tempfile::tempdir().unwrap();
    let bin_len = OLD_LIMIT + 4;
    let mut doc = serde_json::to_vec(&triangle(json!({"byteLength":bin_len}))).unwrap();
    while !doc.len().is_multiple_of(4) {
        doc.push(b' ');
    }
    let total = 12 + 8 + doc.len() as u64 + 8 + bin_len;
    let path = dir.path().join("large.glb");
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(b"glTF").unwrap();
    file.write_all(&2_u32.to_le_bytes()).unwrap();
    file.write_all(&(total as u32).to_le_bytes()).unwrap();
    file.write_all(&(doc.len() as u32).to_le_bytes()).unwrap();
    file.write_all(b"JSON").unwrap();
    file.write_all(&doc).unwrap();
    file.write_all(&(bin_len as u32).to_le_bytes()).unwrap();
    file.write_all(b"BIN\0").unwrap();
    file.write_all(&positions()).unwrap();
    file.set_len(total).unwrap();
    drop(file);
    let result = import_gltf(&path).unwrap();
    assert_eq!(result.report.vertices, 3);
    result.project.validate().unwrap();
    drop(result);
    fs::remove_file(path).unwrap();

    let path = dir.path().join("large.gltf");
    let bin = dir.path().join("large.bin");
    let mut file = fs::File::create(&bin).unwrap();
    file.write_all(&positions()).unwrap();
    file.set_len(bin_len).unwrap();
    drop(file);
    write_json(
        &path,
        &triangle(json!({"uri":"large.bin","byteLength":bin_len})),
    );
    let result = import_gltf(&path).unwrap();
    assert_eq!(result.project.meshes[0].indices, [0, 1, 2]);
    drop(result);
    fs::remove_file(bin).unwrap();

    let path = dir.path().join("large.obj");
    let mut file = fs::File::create(&path).unwrap();
    // Valid bounded comment lines exercise the text reader and parser as well.
    let mut line = vec![b' '; 1024 * 1024];
    line[0] = b'#';
    *line.last_mut().unwrap() = b'\n';
    for _ in 0..256 {
        file.write_all(&line).unwrap();
    }
    file.write_all(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n")
        .unwrap();
    drop(file);
    assert!(fs::metadata(&path).unwrap().len() > OLD_LIMIT);
    let result = import_obj(path).unwrap();
    assert_eq!(result.report.vertices, 3);
    result.project.validate().unwrap();
}

#[test]
fn oversized_models_and_actual_aggregate_buffers_fail_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    for ext in ["glb", "obj"] {
        let path = dir.path().join(format!("oversized.{ext}"));
        fs::File::create(&path)
            .unwrap()
            .set_len(MODEL_LIMIT + 1)
            .unwrap();
        let error = if ext == "glb" {
            import_gltf(&path)
        } else {
            import_obj(&path)
        }
        .unwrap_err()
        .to_string();
        assert!(error.contains("2 ГиБ"), "{error}");
        fs::remove_file(path).unwrap();
    }
    // A repeated external resource counts twice, even if JSON claims 36 bytes.
    fs::File::create(dir.path().join("huge.bin"))
        .unwrap()
        .set_len(MODEL_LIMIT / 2 + 1)
        .unwrap();
    let mut doc = triangle(json!({"uri":"huge.bin","byteLength":36}));
    doc["buffers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"uri":"huge.bin","byteLength":36}));
    let path = dir.path().join("aggregate.gltf");
    write_json(&path, &doc);
    assert!(import_gltf(&path)
        .unwrap_err()
        .to_string()
        .contains("Суммарные буферы"));
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        import_gltf_with_progress(path, &cancelled, |_, _| {}),
        Err(IoError::Cancelled)
    ));
}

#[test]
fn accessor_and_expansion_limits_precede_payload_reads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preflight.gltf");
    let base = triangle(json!({"byteLength":36,"uri":"intentionally-missing.bin"}));
    let mut cases = Vec::new();
    let mut value = base.clone();
    value["accessors"][0]["count"] = json!(2_000_001);
    cases.push((value, "2 миллиона"));
    let mut value = base.clone();
    value["accessors"]
        .as_array_mut()
        .unwrap()
        .push(json!({"bufferView":0,"componentType":5125,"type":"SCALAR","count":12_000_003}));
    value["meshes"][0]["primitives"][0]["indices"] = json!(1);
    cases.push((value, "4 миллиона"));
    let mut value = base.clone();
    value["meshes"][0]["primitives"][0]["targets"] = json!(vec![json!({"POSITION":0}); 65]);
    cases.push((value, "64 morph"));
    let mut value = base;
    value["accessors"]
        .as_array_mut()
        .unwrap()
        .push(json!({"bufferView":0,"componentType":5126,"type":"VEC3","count":4}));
    value["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(1);
    cases.push((value, "не совпадает"));
    for (value, message) in cases {
        write_json(&path, &value);
        let error = import_gltf(&path).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert!(
            !matches!(error, IoError::File(_)),
            "preflight must run before buffer IO"
        );
    }
}

#[test]
#[ignore = "allocates about 400 MiB; run explicitly with one test thread"]
fn decoded_textures_over_256_mib_fit_for_gltf_and_obj_without_losing_maps() {
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::from_pixel(4096, 4096, image::Rgba([60, 120, 180, 255]))
        .save(dir.path().join("map0.png"))
        .unwrap();
    for i in 1..5 {
        fs::hard_link(
            dir.path().join("map0.png"),
            dir.path().join(format!("map{i}.png")),
        )
        .unwrap();
    }
    fs::write(dir.path().join("mesh.bin"), positions()).unwrap();
    let mut doc = triangle(json!({"uri":"mesh.bin","byteLength":36}));
    doc["images"] = json!((0..5)
        .map(|i| json!({"uri":format!("map{i}.png")}))
        .collect::<Vec<_>>());
    doc["textures"] = json!((0..5).map(|i| json!({"source":i})).collect::<Vec<_>>());
    doc["materials"] = json!((0..5)
        .map(|i| json!({"pbrMetallicRoughness":{"baseColorTexture":{"index":i}}}))
        .collect::<Vec<_>>());
    doc["meshes"][0]["primitives"][0]["material"] = json!(0);
    write_json(&dir.path().join("mesh.gltf"), &doc);
    let mut obj = String::from("mtllib mesh.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\n");
    let mut mtl = String::new();
    for i in 0..5 {
        obj.push_str(&format!("usemtl m{i}\nf 1 2 3\n"));
        mtl.push_str(&format!("newmtl m{i}\nmap_Kd map{i}.png\n"));
    }
    fs::write(dir.path().join("mesh.obj"), obj).unwrap();
    fs::write(dir.path().join("mesh.mtl"), mtl).unwrap();
    for gltf in [true, false] {
        let imported = if gltf {
            import_gltf(dir.path().join("mesh.gltf")).unwrap()
        } else {
            import_obj(dir.path().join("mesh.obj")).unwrap()
        };
        assert_eq!(imported.project.textures.len(), 5);
        let bytes: usize = imported
            .project
            .textures
            .iter()
            .map(|t| t.rgba8.len())
            .sum();
        assert!(bytes <= 256 * 1024 * 1024);
        assert!(imported.report.missing_textures.is_empty());
        assert!(imported
            .report
            .warnings
            .iter()
            .any(|w| w.contains("бюджета текстур")));
        assert_eq!(
            imported
                .project
                .materials
                .iter()
                .filter(|m| m.base_color_texture.is_some())
                .count(),
            5
        );
        imported.project.validate().unwrap();
        eprintln!(
            "{}: original 320 MiB, imported {} MiB, all 5 maps assigned",
            if gltf { "glTF" } else { "OBJ" },
            bytes / (1024 * 1024)
        );
    }
}
