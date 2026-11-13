use poseforge_io::{import_gltf, import_obj, load_texture, IoError};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixture(dir: &Path) -> Value {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::copy(
        source.join("skinned_triangle.bin"),
        dir.join("skinned_triangle.bin"),
    )
    .unwrap();
    serde_json::from_slice(&fs::read(source.join("skinned_triangle.gltf")).unwrap()).unwrap()
}
fn write(dir: &Path, value: &Value) -> PathBuf {
    let path = dir.join("textured.gltf");
    fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}
fn pixels() -> image::RgbaImage {
    image::RgbaImage::from_fn(4, 4, |x, y| {
        image::Rgba([x as u8 * 40, y as u8 * 50, 80, 255])
    })
}
fn uri_path(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    format!(
        "file://{}",
        path.replace('%', "%25")
            .replace(' ', "%20")
            .replace('#', "%23")
    )
}

#[test]
fn moved_gltf_maps_load_actual_pixels_and_keep_skin_animation_and_transforms() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = fixture(dir.path());
    fs::create_dir(dir.path().join("textures")).unwrap();
    pixels()
        .save(dir.path().join("textures/body color.png"))
        .unwrap();
    value["images"][0]["name"] = json!("Albedo without a file extension");
    value["materials"][0]["normalTexture"] = json!({"index":0});
    value["materials"][0]["emissiveTexture"] = json!({"index":0});
    for uri in [
        "old/textures/body%20color.png",
        "C:\\Old Export\\body color.png",
        "body%20color.png",
    ] {
        value["images"][0]["uri"] = json!(uri);
        let result = import_gltf(write(dir.path(), &value)).unwrap();
        assert_eq!(
            result.project.textures[0].rgba8.as_ref(),
            pixels().as_raw().as_slice()
        );
        assert_eq!(result.report.textures, 1);
        assert!(result.report.missing_textures.is_empty());
        assert_eq!(result.report.joints, 2);
        assert_eq!(result.report.clips, 1);
        assert!(result.project.nodes.iter().any(|n| n.skin.is_some()));
        let mat = &result.project.materials[0];
        assert_eq!(mat.base_color_texture, mat.normal_texture);
        assert_eq!(mat.base_color_texture, mat.emissive_texture);
        assert!(result
            .report
            .warnings
            .iter()
            .any(|w| w.contains("новому пути")));
    }
}

#[test]
fn authored_absolute_and_file_uri_images_win_over_nearby_copies() {
    let dir = tempfile::tempdir().unwrap();
    let assets = tempfile::tempdir().unwrap();
    let map = assets.path().join("body # 100%20.png");
    pixels().save(&map).unwrap();
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]))
        .save(dir.path().join(map.file_name().unwrap()))
        .unwrap();
    let mut value = fixture(dir.path());
    for uri in [map.to_string_lossy().into_owned(), uri_path(&map)] {
        // Literal % is encoded in a glTF URI. file: already encodes it above.
        let uri = if uri.starts_with("file:") {
            uri
        } else {
            uri.replace('%', "%25")
        };
        value["images"][0]["uri"] = json!(uri);
        let result = import_gltf(write(dir.path(), &value)).unwrap();
        assert_eq!(
            result.project.textures[0].rgba8.as_ref(),
            pixels().as_raw().as_slice()
        );
        assert!(result.report.missing_textures.is_empty());
        assert!(result.report.warnings.is_empty());
    }
}

#[test]
fn external_tga_and_bmp_load_in_obj_gltf_and_manual_assignment_with_exact_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = fixture(dir.path());
    for ext in ["TGA", "bmp"] {
        let name = format!("цвет карты.{ext}");
        let map = dir.path().join(&name);
        pixels().save(&map).unwrap();
        let expected = pixels().into_raw();
        assert_eq!(
            load_texture(&map).unwrap().rgba8.as_ref(),
            expected.as_slice()
        );
        value["images"][0] = json!({"name":"Skin map", "uri": name.replace(' ', "%20")});
        let result = import_gltf(write(dir.path(), &value)).unwrap();
        assert_eq!(
            result.project.textures[0].rgba8.as_ref(),
            expected.as_slice()
        );
        assert_eq!(result.project.textures[0].name, "Skin map");
        assert!(result.report.missing_textures.is_empty());
        fs::write(
            dir.path().join("skin.mtl"),
            format!("newmtl Skin\nmap_Kd \"{name}\"\n"),
        )
        .unwrap();
        let obj = dir.path().join("skin.obj");
        fs::write(
            &obj,
            "mtllib skin.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Skin\nf 1 2 3\n",
        )
        .unwrap();
        let result = import_obj(obj).unwrap();
        assert_eq!(
            result.project.textures[0].rgba8.as_ref(),
            expected.as_slice()
        );
        assert!(result.report.missing_textures.is_empty());
    }
}

fn dds(fourcc: &[u8; 4]) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    bytes[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (8, 0x1007),
        (12, 4),
        (16, 4),
        (76, 32),
        (80, 4),
        (108, 0x1000),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[84..88].copy_from_slice(fourcc);
    if fourcc == b"DXT3" {
        bytes.extend([255; 8]);
    }
    if fourcc == b"DXT5" {
        bytes.extend([255, 0, 0, 0, 0, 0, 0, 0]);
    }
    bytes.extend([0, 248, 0, 0, 0, 0, 0, 0]); // red RGB565 endpoint, all indices zero
    bytes
}

#[test]
fn dds_bc1_bc2_bc3_decode_pixels_and_unsupported_bc7_stays_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let map = dir.path().join("albedo.dds");
    for variant in [b"DXT1", b"DXT3", b"DXT5"] {
        fs::write(&map, dds(variant)).unwrap();
        let texture = load_texture(&map).unwrap();
        assert_eq!((texture.width, texture.height), (4, 4));
        assert!(texture
            .rgba8
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [255, 0, 0, 255]));
        let mut value = fixture(dir.path());
        value["images"][0] = json!({"uri":"albedo.dds"});
        let result = import_gltf(write(dir.path(), &value)).unwrap();
        assert_eq!(result.project.textures[0].rgba8, texture.rgba8);
        assert!(result.report.missing_textures.is_empty());
    }
    let mut unsupported = dds(b"DX10");
    unsupported.truncate(128);
    unsupported.extend([98u32, 3, 0, 1, 0].into_iter().flat_map(u32::to_le_bytes));
    fs::write(&map, unsupported).unwrap();
    assert!(load_texture(&map).unwrap_err().to_string().contains("98"));
}

#[test]
fn missing_broken_and_remote_images_preserve_authored_color_and_skeleton_without_pink_assets() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = fixture(dir.path());
    pixels().save(dir.path().join("remote.png")).unwrap();
    fs::write(dir.path().join("broken.png"), b"broken image").unwrap();
    value["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"] = json!([0.3, 0.4, 0.5, 0.8]);
    value["materials"][0]["normalTexture"] = json!({"index":0});
    for uri in [
        "missing.png",
        "broken.png",
        "https://example.invalid/remote.png",
        "file://server/share/remote.png",
        "\\\\server\\share\\remote.png",
        "%2F%2Fserver/remote.png",
    ] {
        value["images"][0]["uri"] = json!(uri);
        let result = import_gltf(write(dir.path(), &value)).unwrap();
        assert_eq!(result.report.joints, 2);
        assert_eq!(result.report.clips, 1);
        assert_eq!(result.report.meshes, 1);
        assert_eq!(result.report.textures, 0);
        assert_eq!(result.report.missing_textures, [uri]);
        assert!(result.project.textures.is_empty());
        assert_eq!(result.project.materials[0].base_color, [0.3, 0.4, 0.5, 0.8]);
        assert!(result.project.materials[0].base_color_texture.is_none());
        assert!(result.project.materials[0].normal_texture.is_none());
        assert!(result
            .report
            .warnings
            .iter()
            .any(|w| w.contains("числовые параметры")));
    }
}

#[test]
fn relocated_image_file_and_tga_dimensions_keep_allocation_limits() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = fixture(dir.path());
    fs::create_dir(dir.path().join("textures")).unwrap();
    fs::File::create(dir.path().join("textures/oversized.png"))
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    value["images"][0]["uri"] = json!("old/oversized.png");
    assert!(matches!(
        import_gltf(write(dir.path(), &value)),
        Err(IoError::Limit(_))
    ));
    let mut tga = vec![0; 18];
    tga[2] = 2;
    tga[12..14].copy_from_slice(&8193u16.to_le_bytes());
    tga[14..16].copy_from_slice(&1u16.to_le_bytes());
    tga[16] = 24;
    fs::write(dir.path().join("wide.tga"), tga).unwrap();
    assert!(matches!(
        load_texture(dir.path().join("wide.tga")),
        Err(IoError::Limit(_))
    ));
}
