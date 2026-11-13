use poseforge_core::AlphaMode;
use poseforge_io::{import_model, import_obj, import_obj_with_progress, IoError};
use std::{fs, sync::atomic::AtomicBool};

#[test]
fn concave_negative_indices_keep_uvs_and_area() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("concave.obj");
    fs::write(&path,"v 0 0 0\nv 2 0 0\nv 2 2 0\nv 1 1 0\nv 0 2 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0.5 0.5\nvt 0 1\nf -5/1 -4/2 -3/3 -2/4 -1/5\n").unwrap();
    let result = import_model(path).unwrap();
    let mesh = &result.project.meshes[0];
    assert_eq!(mesh.vertices.len(), 5);
    assert_eq!(mesh.indices.len(), 9);
    assert_eq!(mesh.vertices[0].uv, [0., 1.]);
    let area: f32 = mesh
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let a = glam::Vec3::from_array(mesh.vertices[t[0] as usize].position);
            let b = glam::Vec3::from_array(mesh.vertices[t[1] as usize].position);
            let c = glam::Vec3::from_array(mesh.vertices[t[2] as usize].position);
            (b - a).cross(c - a).length() * 0.5
        })
        .sum();
    assert!((area - 3.).abs() < 1e-5);
    assert!(mesh.vertices.iter().all(|v| v.normal == [0., 0., 1.]));
    assert!(result.project.lights.is_empty());
}

#[test]
fn mtl_groups_maps_and_transparency_are_imported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("two.obj");
    fs::write(&path,"mtllib two.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nusemtl Red\nf 1 2 3\nusemtl Glass\nf 1 4 2\n").unwrap();
    fs::write(dir.path().join("two.mtl"),"newmtl Red\nKd 1 0.2 0.1\nPr 0.2\nPm 0.7\nmap_Kd albedo.png\nnewmtl Glass\nKd 0.1 0.2 0.8\nd 0.4\n").unwrap();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]))
        .save(dir.path().join("albedo.png"))
        .unwrap();
    let result = import_obj(path).unwrap();
    assert_eq!(result.report.meshes, 2);
    assert_eq!(result.report.materials, 2);
    assert_eq!(result.report.textures, 1);
    let red = &result.project.materials[0];
    assert_eq!(red.roughness, 0.2);
    assert_eq!(red.metallic, 0.7);
    assert!(red.base_color_texture.is_some());
    assert_eq!(result.project.materials[1].alpha_mode, AlphaMode::Blend);
    assert_eq!(result.project.materials[1].base_color[3], 0.4);
    result.project.validate().unwrap();
}

#[test]
fn missing_mtl_texture_is_reported_and_geometry_survives() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.obj");
    fs::write(
        &path,
        "mtllib material.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Skin\nf 1 2 3\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("material.mtl"),
        "newmtl Skin\nmap_Kd missing.png\nnorm missing.png\n",
    )
    .unwrap();
    let result = import_obj(path).unwrap();
    assert_eq!(result.report.missing_textures, ["missing.png"]);
    assert_eq!(result.project.meshes[0].indices.len(), 3);
    assert!(result.project.materials[0].base_color_texture.is_none());
    assert!(result.project.materials[0].normal_texture.is_none());
}

#[test]
fn malformed_indices_and_crossed_faces_fail() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.obj");
    for body in [
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 0 2 3\n",
        "v 0 0 0\nv 1 1 0\nv 0 1 0\nv 1 0 0\nf 1 2 3 4\n",
    ] {
        fs::write(&path, body).unwrap();
        assert!(import_obj(&path).is_err(), "{body}");
    }
}

#[test]
fn nonplanar_quad_at_reported_line_imports_without_flattening_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonplanar.obj");
    let prefix = "# exporter data\n".repeat(86_574);
    fs::write(
        &path,
        format!("{prefix}v 0 0 0\nv 1 0 0\nv 1 1 1\nv 0 1 0\nf 1 2 3 4\n"),
    )
    .unwrap();
    let result = import_obj(path).unwrap();
    let mesh = &result.project.meshes[0];
    assert_eq!(mesh.vertices.len(), 4);
    assert_eq!(mesh.indices.len(), 6);
    assert!(mesh.vertices.iter().any(|v| v.position == [1., 1., 1.]));
    assert_eq!(result.report.warnings.len(), 1);
    assert!(result.report.warnings[0].contains("автоматически триангулированы: 1"));
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let [a, b, c] =
            triangle.map(|i| glam::Vec3::from_array(mesh.vertices[i as usize].position));
        assert!((b - a).cross(c - a).length() > 0.5);
    }
    result.project.validate().unwrap();
}

#[test]
fn warped_concave_ngon_preserves_uvs_winding_and_single_warning() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("warped.obj");
    let header = "v 0 0 0\nv 2 0 0.2\nv 2 2 0\nv 1 1 0.1\nv 0 2 -0.1\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0.5 0.5\nvt 0 1\n";
    for face in ["f 1/1 2/2 3/3 4/4 5/5", "f 5/5 4/4 3/3 2/2 1/1"] {
        fs::write(&path, format!("{header}{face}\n{face}\n")).unwrap();
        let result = import_obj(&path).unwrap();
        let mesh = &result.project.meshes[0];
        assert_eq!(mesh.indices.len(), 18);
        assert_eq!(result.report.warnings.len(), 1);
        assert!(result.report.warnings[0].contains("триангулированы: 2"));
        let mut projected_area = 0.;
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] =
                triangle.map(|i| glam::Vec3::from_array(mesh.vertices[i as usize].position));
            let z = (b - a).cross(c - a).z;
            assert!(if face.starts_with("f 1") {
                z > 0.
            } else {
                z < 0.
            });
            projected_area += z.abs() * 0.5;
        }
        assert!((projected_area - 6.).abs() < 1e-5);
        for v in &mesh.vertices {
            assert!((v.uv[0] - v.position[0] * 0.5).abs() < 1e-6);
            assert!((v.uv[1] - (1. - v.position[1] * 0.5)).abs() < 1e-6);
        }
    }
}

#[test]
fn cancellation_precedes_any_file_access() {
    assert!(matches!(
        import_obj_with_progress("absent.obj", &AtomicBool::new(true), |_, _| {}),
        Err(IoError::Cancelled)
    ));
}

#[test]
fn obj_normal_map_flips_green_without_changing_a_shared_albedo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("normal.obj");
    fs::write(
        &path,
        "mtllib material.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Normal\nf 1 2 3\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("material.mtl"),
        "newmtl Normal\nmap_Kd image.png\nnorm image.png\n",
    )
    .unwrap();
    image::RgbaImage::from_pixel(1, 1, image::Rgba([128, 192, 255, 255]))
        .save(dir.path().join("image.png"))
        .unwrap();
    let p = import_obj(path).unwrap().project;
    let m = &p.materials[0];
    assert_ne!(m.base_color_texture, m.normal_texture);
    assert_eq!(
        p.textures
            .iter()
            .find(|t| Some(t.id) == m.base_color_texture)
            .unwrap()
            .rgba8,
        [128, 192, 255, 255]
    );
    assert_eq!(
        p.textures
            .iter()
            .find(|t| Some(t.id) == m.normal_texture)
            .unwrap()
            .rgba8,
        [128, 63, 255, 255]
    );
}

#[test]
fn smoothing_groups_merge_only_compatible_missing_normals() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("smooth.obj");
    let vertices = "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\n";
    fs::write(&path, format!("{vertices}s 1\nf 1 2 3\nf 1 4 2\n")).unwrap();
    let smooth = import_obj(&path).unwrap().project;
    assert_eq!(smooth.meshes[0].vertices.len(), 4);
    let normal = glam::Vec3::from_array(smooth.meshes[0].vertices[0].normal);
    assert!(normal.dot(glam::Vec3::new(0., 1., 1.).normalize()) > 0.9999);
    fs::write(&path, format!("{vertices}s off\nf 1 2 3\nf 1 4 2\n")).unwrap();
    let flat = import_obj(&path).unwrap().project;
    assert_eq!(flat.meshes[0].vertices.len(), 6);
}

#[test]
fn utf8_bom_preserves_the_first_vertex_and_first_mtl_material() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bom.obj");
    fs::write(
        &path,
        "\u{feff}v 0 0 0\nv 1 0 0\nv 0 1 0\nmtllib bom.mtl\nusemtl Red\nf 1 2 3\n",
    )
    .unwrap();
    fs::write(dir.path().join("bom.mtl"), "\u{feff}newmtl Red\nKd 1 0 0\n").unwrap();
    let result = import_obj(path).unwrap();
    assert_eq!(result.report.vertices, 3);
    assert_eq!(result.project.materials[0].base_color, [1., 0., 0., 1.]);
    assert!(result.report.warnings.is_empty());
}

#[test]
fn smooth_normals_cross_uv_and_material_seams_but_keep_distinct_uvs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("seam.obj");
    let vertices = "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nvt 0 0\nvt 1 1\ns 1\n";
    for split in ["", "usemtl Other\n"] {
        fs::write(
            &path,
            format!("{vertices}f 1/1 2/1 3/1\n{split}f 1/2 4/2 2/2\n"),
        )
        .unwrap();
        let result = import_obj(&path).unwrap();
        let shared: Vec<_> = result
            .project
            .meshes
            .iter()
            .flat_map(|m| m.vertices.iter())
            .filter(|v| v.position == [0.; 3])
            .collect();
        assert_eq!(shared.len(), 2);
        assert_ne!(shared[0].uv, shared[1].uv);
        let expected = glam::Vec3::new(0., 1., 1.).normalize();
        for v in shared {
            assert!(glam::Vec3::from_array(v.normal).dot(expected) > 0.9999);
        }
    }
}

#[test]
fn cancellation_during_geometry_does_not_finish_or_deliver_a_partial_model() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cancel.obj");
    fs::write(
        &path,
        format!("v 0 0 0\nv 1 0 0\nv 0 1 0\n{}", "f 1 2 3\n".repeat(5000)),
    )
    .unwrap();
    let cancel = AtomicBool::new(false);
    let mut finished = false;
    let result = import_obj_with_progress(&path, &cancel, |p, phase| {
        if phase == "Геометрия OBJ" && p > 0.05 {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        finished |= p >= 1.;
    });
    assert!(matches!(result, Err(IoError::Cancelled)));
    assert!(!finished);
    assert!(
        import_obj(path).is_ok(),
        "cancellation does not leave a poisoned importer"
    );
}

#[test]
fn repeated_closing_and_adjacent_corners_at_line_93428_are_repaired() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("repeated.obj");
    let prefix = "# exporter data\n".repeat(93_417);
    let header =
        "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\ns 1\n";
    for face in [
        "f 1/1/1 2/2/1 3/3/1 4/4/1 1/1/1",
        "f 1/1/1 2/2/1 2/2/1 3/3/1 4/4/1",
    ] {
        let body = format!("{prefix}{header}{face}\n");
        assert_eq!(body.lines().nth(93_427).unwrap(), face);
        fs::write(&path, body).unwrap();
        let result = import_obj(&path).unwrap();
        let mesh = &result.project.meshes[0];
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
        for v in &mesh.vertices {
            assert_eq!(v.normal, [0., 0., 1.]);
            assert_eq!(v.uv, [v.position[0], 1. - v.position[1]]);
        }
        assert_eq!(result.report.warnings.len(), 1);
        assert!(result.report.warnings[0].contains("повторяющиеся"));
    }
}

#[test]
fn damaged_face_at_line_93428_is_reported_without_losing_other_groups() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crossing.obj");
    let prefix = "# exporter data\n".repeat(93_418);
    let header = "v 0 0 0\nv 3 3 0\nv 0 3 0\nv 2 0 0\nvt 0 0\nvt 1 1\nvt 0 1\nvt 1 0\ng Damaged\n";
    fs::write(
        &path,
        format!("{prefix}{header}f 1/1 2/2 3/3 4/4\ng Kept\nf 1/1 4/4 3/3\n"),
    )
    .unwrap();
    let result = import_obj(&path).unwrap();
    let mesh = &result.project.meshes[0];
    assert_eq!(result.report.meshes, 1);
    assert_eq!(mesh.name, "Kept");
    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices.len(), 3);
    assert_eq!(result.report.warnings.len(), 1);
    assert!(result.report.warnings[0].contains("пропущено повреждённых граней: 1"));
    assert!(result.report.warnings[0].contains("строка 93428"));
    assert!(result.report.warnings[0].contains("отверстия"));
    result.project.validate().unwrap();
    // A completely unusable file must not commit an empty replacement scene.
    fs::write(&path, format!("{header}f 1/1 2/2 3/3 4/4\n")).unwrap();
    assert!(import_obj(path)
        .unwrap_err()
        .to_string()
        .contains("нет пригодных граней"));
}

#[test]
fn invalid_faces_have_bounded_diagnostics_and_parse_errors_remain_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.obj");
    let header = "v 0 0 0\nv 3 3 0\nv 0 3 0\nv 2 0 0\n";
    fs::write(
        &path,
        format!("{header}{}f 1 4 3\n", "f 1 2 3 4\n".repeat(1000)),
    )
    .unwrap();
    let result = import_obj(&path).unwrap();
    assert_eq!(result.report.warnings.len(), 1);
    let warning = &result.report.warnings[0];
    assert!(warning.contains("граней: 1000"));
    assert_eq!(warning.matches("строка ").count(), 8);
    assert!(warning.len() < 2048);
    assert_eq!(result.project.meshes[0].indices.len(), 3);
    fs::write(&path, format!("{header}f 1 4 3\nf 1 2 99\n")).unwrap();
    assert!(import_obj(path)
        .unwrap_err()
        .to_string()
        .contains("индекс OBJ"));
}

#[test]
fn narrow_valid_faces_and_alternate_nonplanar_projection_preserve_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("projection.obj");
    for header in [
        // Old f32 tolerance treated opposite edges as touching.
        "v 0 0 0\nv 1 0 0\nv 1 0.0000001 0\nv 0 0.0000001 0\n",
        // XY (dominant normal axis) crosses, but YZ is simple.
        "v 0 0 0\nv 3 3 0\nv 0 3 0.1\nv 2 0 0.2\n",
    ] {
        fs::write(&path, format!("{header}f 1 2 3 4\n")).unwrap();
        let result = import_obj(&path).unwrap();
        let mesh = &result.project.meshes[0];
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
        assert!(!result
            .report
            .warnings
            .iter()
            .any(|w| w.contains("пропущено")));
        for t in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = t.map(|i| glam::Vec3::from_array(mesh.vertices[i as usize].position));
            assert!((b - a).cross(c - a).length() > 0.);
        }
    }
}

#[test]
fn nonadjacent_touching_hole_bridges_are_not_filled_with_an_unsafe_fan() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("touch.obj");
    fs::write(&path,"v 0 0 0\nv 4 0 0\nv 4 4 0\nv 0 4 0\nv 1 1 0\nv 1 2 0\nv 2 2 0\nv 2 1 0\nf 1 2 3 4 1 5 6 7 8 5\nf 5 8 7\n").unwrap();
    let result = import_obj(path).unwrap();
    assert_eq!(result.project.meshes[0].indices.len(), 3);
    assert!(result
        .report
        .warnings
        .iter()
        .any(|w| w.contains("пропущено повреждённых граней: 1")));
}

const RESOURCE_TRIANGLE: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Surface\nf 1 2 3\n";

fn png(path: &std::path::Path, color: [u8; 4]) {
    image::RgbaImage::from_pixel(1, 1, image::Rgba(color))
        .save(path)
        .unwrap();
}

#[test]
fn absolute_mtl_and_texture_paths_take_priority_over_same_named_local_copies() {
    let dir = tempfile::tempdir().unwrap();
    let asset = tempfile::tempdir().unwrap();
    let mtl = asset.path().join("body material.mtl");
    let map = asset.path().join("body color.png");
    png(&map, [21, 42, 63, 255]);
    png(&dir.path().join("body color.png"), [255, 0, 0, 255]);
    fs::write(
        &mtl,
        format!("newmtl Surface\nKd 0.2 0.3 0.4\nmap_Kd {}\n", map.display()),
    )
    .unwrap();
    fs::write(
        dir.path().join("body material.mtl"),
        "newmtl Surface\nKd 1 0 0\n",
    )
    .unwrap();
    let path = dir.path().join("model.obj");
    fs::write(
        &path,
        format!("mtllib \"{}\"\n{RESOURCE_TRIANGLE}", mtl.display()),
    )
    .unwrap();
    let result = import_obj(path).unwrap();
    assert_eq!(result.project.materials[0].base_color, [0.2, 0.3, 0.4, 1.]);
    assert_eq!(result.project.textures[0].rgba8, [21, 42, 63, 255]);
    assert!(result.report.warnings.is_empty());
    assert!(result.report.missing_textures.is_empty());
}

#[test]
fn moved_windows_paths_find_mtl_and_maps_next_to_obj_without_modifying_sources() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("materials")).unwrap();
    fs::create_dir(dir.path().join("textures")).unwrap();
    let mtl = dir.path().join("materials/body material.mtl");
    let body="newmtl Surface\nmap_Kd D:\\Old Project\\Textures\\body color.png\nmap_Ke D:\\Old Project\\Textures\\body color.png\n";
    fs::write(&mtl, body).unwrap();
    png(
        &dir.path().join("textures/body color.png"),
        [12, 34, 56, 255],
    );
    let path = dir.path().join("model.obj");
    let source =
        format!("mtllib C:\\Old Project\\materials\\body material.mtl\n{RESOURCE_TRIANGLE}");
    fs::write(&path, &source).unwrap();
    let result = import_obj(&path).unwrap();
    assert_eq!(result.report.textures, 1);
    assert_eq!(result.project.textures[0].rgba8, [12, 34, 56, 255]);
    let material = &result.project.materials[0];
    assert_eq!(material.base_color_texture, material.emissive_texture);
    assert!(result.report.missing_textures.is_empty());
    assert_eq!(result.report.warnings.len(), 2);
    assert!(result
        .report
        .warnings
        .iter()
        .all(|w| w.contains("найден по новому пути")));
    assert_eq!(fs::read_to_string(path).unwrap(), source);
    assert_eq!(fs::read_to_string(mtl).unwrap(), body);
}

#[test]
fn missing_absolute_assets_keep_geometry_with_explicit_fallbacks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.obj");
    fs::write(
        &path,
        format!("mtllib Z:/missing/export.mtl\n{RESOURCE_TRIANGLE}"),
    )
    .unwrap();
    let result = import_obj(&path).unwrap();
    assert_eq!(result.project.meshes[0].indices.len(), 3);
    assert!(result
        .report
        .warnings
        .iter()
        .any(|w| w.contains("Z:/missing/export.mtl")));
    fs::write(dir.path().join("surface.mtl"),"newmtl Surface\nmap_Kd Z:/missing/albedo.png\nmap_Ke Z:/missing/emission.png\nnorm Z:/missing/normal.png\n").unwrap();
    fs::write(&path, format!("mtllib surface.mtl\n{RESOURCE_TRIANGLE}")).unwrap();
    let result = import_obj(&path).unwrap();
    assert_eq!(result.project.meshes[0].indices.len(), 3);
    assert_eq!(result.report.missing_textures.len(), 3);
    assert_eq!(result.report.textures, 0);
    assert!(result.project.textures.is_empty());
    assert_eq!(result.report.textures, 0);
    assert!(result.project.materials[0].base_color_texture.is_none());
    assert!(result.project.materials[0].emissive_texture.is_none());
    assert!(result.project.materials[0].normal_texture.is_none());
}

#[test]
fn quoted_multiple_libraries_spaces_hashes_and_apostrophes_preserve_materials() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first #1.mtl");
    let second = dir.path().join("O'Brien second.mtl");
    let map = dir.path().join("color #1.png");
    png(&map, [13, 23, 33, 255]);
    fs::write(
        &first,
        "newmtl Surface\nKd 0.5 0.6 0.7 # comment\nmap_Kd \"color #1.png\" # comment\n",
    )
    .unwrap();
    fs::write(&second, "newmtl Other\nKd 0.1 0.2 0.3\n").unwrap();
    let path = dir.path().join("quoted.obj");
    fs::write(
        &path,
        format!(
            "mtllib \"{}\" \"{}\" # comment\n{RESOURCE_TRIANGLE}usemtl Other\nf 1 3 2\n",
            first.display(),
            second.display()
        ),
    )
    .unwrap();
    let result = import_obj(&path).unwrap();
    assert_eq!(result.report.materials, 2);
    assert_eq!(result.project.materials[0].base_color, [0.5, 0.6, 0.7, 1.]);
    assert_eq!(result.project.materials[1].base_color, [0.1, 0.2, 0.3, 1.]);
    assert_eq!(result.project.textures[0].rgba8, [13, 23, 33, 255]);
    assert!(result.report.warnings.is_empty());
    // An apostrophe inside an unquoted filename is not a quote delimiter.
    fs::write(
        &path,
        "mtllib O'Brien second.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Other\nf 1 2 3\n",
    )
    .unwrap();
    assert!(import_obj(path).unwrap().report.warnings.is_empty());
}

#[test]
fn multiple_unquoted_libraries_and_relative_parent_backslashes_still_work() {
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("model");
    fs::create_dir(&model).unwrap();
    fs::write(
        dir.path().join("a.mtl"),
        "newmtl Surface\nmap_Kd color.png\n",
    )
    .unwrap();
    fs::write(dir.path().join("b.mtl"), "newmtl Other\nKd 0 1 0\n").unwrap();
    png(&dir.path().join("color.png"), [1, 2, 3, 255]);
    let path = model.join("relative.obj");
    fs::write(
        &path,
        format!("mtllib ..\\a.mtl ..\\b.mtl\n{RESOURCE_TRIANGLE}usemtl Other\nf 1 3 2\n"),
    )
    .unwrap();
    let result = import_obj(path).unwrap();
    assert_eq!(result.report.materials, 2);
    assert_eq!(result.project.textures[0].rgba8, [1, 2, 3, 255]);
    assert!(result.report.warnings.is_empty());
    let materials = dir.path().join("materials");
    fs::create_dir(&materials).unwrap();
    fs::copy(dir.path().join("a.mtl"), materials.join("a.mtl")).unwrap();
    fs::copy(dir.path().join("b.mtl"), materials.join("b.mtl")).unwrap();
    png(&materials.join("color.png"), [1, 2, 3, 255]);
    let path = dir.path().join("two_paths.obj");
    fs::write(
        &path,
        format!(
            "mtllib materials/a.mtl materials/b.mtl\n{RESOURCE_TRIANGLE}usemtl Other\nf 1 3 2\n"
        ),
    )
    .unwrap();
    let result = import_obj(path).unwrap();
    assert_eq!(result.report.materials, 2);
    assert!(result.report.warnings.is_empty());
}

#[test]
fn local_file_uris_decode_spaces_without_treating_plain_percent_as_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let map = dir.path().join("color # map.png");
    png(&map, [7, 8, 9, 255]);
    let absolute = map.to_string_lossy().replace('\\', "/");
    let uri_path = if absolute.starts_with('/') {
        absolute
    } else {
        format!("/{absolute}")
    };
    let uri_path = uri_path.replace(' ', "%20").replace('#', "%23");
    let mtl = dir.path().join("surface.mtl");
    let path = dir.path().join("uri.obj");
    fs::write(&path, format!("mtllib surface.mtl\n{RESOURCE_TRIANGLE}")).unwrap();
    for authority in ["", "localhost"] {
        fs::write(
            &mtl,
            format!("newmtl Surface\nmap_Kd file://{authority}{uri_path}\n"),
        )
        .unwrap();
        let result = import_obj(&path).unwrap();
        assert_eq!(result.project.textures[0].rgba8, [7, 8, 9, 255]);
        assert!(result.report.warnings.is_empty());
    }
    png(&dir.path().join("100%20.png"), [70, 80, 90, 255]);
    fs::write(&mtl, "newmtl Surface\nmap_Kd 100%20.png\n").unwrap();
    assert_eq!(
        import_obj(path).unwrap().project.textures[0].rgba8,
        [70, 80, 90, 255]
    );
}

#[test]
fn network_references_are_not_fetched_or_rebased_and_never_abort_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("network.obj");
    // These tempting local copies must not turn a URL into an unrelated asset.
    fs::write(dir.path().join("remote.mtl"), "newmtl Surface\nKd 1 0 0\n").unwrap();
    png(&dir.path().join("remote.png"), [255, 0, 0, 255]);
    for remote in [
        "https://example.invalid/remote",
        "\\\\server\\share\\remote",
        "file://server/share/remote",
    ] {
        fs::write(&path, format!("mtllib {remote}.mtl\n{RESOURCE_TRIANGLE}")).unwrap();
        let result = import_obj(&path).unwrap();
        assert_eq!(result.project.meshes[0].indices.len(), 3);
        assert_ne!(result.project.materials[0].base_color, [1., 0., 0., 1.]);
        assert!(result.report.warnings.iter().any(|w| w.contains("сетев")));
        fs::write(
            dir.path().join("surface.mtl"),
            format!("newmtl Surface\nmap_Kd {remote}.png\n"),
        )
        .unwrap();
        fs::write(&path, format!("mtllib surface.mtl\n{RESOURCE_TRIANGLE}")).unwrap();
        let result = import_obj(&path).unwrap();
        assert_eq!(result.report.missing_textures.len(), 1);
        assert!(result.project.textures.is_empty());
        assert_eq!(result.report.textures, 0);
        assert!(result.project.materials[0].base_color_texture.is_none());
    }
}

#[test]
fn optional_resource_fallback_does_not_hide_texture_size_limits() {
    let dir = tempfile::tempdir().unwrap();
    let map = dir.path().join("oversized.png");
    fs::File::create(&map)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    fs::write(
        dir.path().join("surface.mtl"),
        format!("newmtl Surface\nmap_Kd {}\n", map.display()),
    )
    .unwrap();
    let path = dir.path().join("model.obj");
    fs::write(&path, format!("mtllib surface.mtl\n{RESOURCE_TRIANGLE}")).unwrap();
    let error = import_obj(path).unwrap_err();
    assert!(matches!(error, IoError::Limit(_)), "{error}");
    assert!(error.to_string().contains("64 МиБ"));
}
