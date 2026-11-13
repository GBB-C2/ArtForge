//! Explicit hardware/software-adapter test: cargo test -p poseforge-render -- --ignored
use poseforge_core::Project;
use poseforge_render::{Renderer, View};

#[path = "gpu/import_appearance.rs"]
mod import_appearance;

#[path = "gpu/emissive_lighting.rs"]
mod emissive_lighting;
#[path = "gpu/fx.rs"]
mod fx;
#[path = "gpu/artistic.rs"]
mod artistic;

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn npr_bands_textures_alpha_lights_and_pbr_toggle() {
    use poseforge_core::{AlphaMode, Light, LightKind, MaterialPreset};
    let (mut p, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.render_settings.shadows = false;
    p.lights.push(Light {
        direction: [0., 0., -1.].into(),
        intensity: 0.4,
        color: [1.; 3],
        ..Light::default()
    });
    let pbr = material_picture(&mut renderer, &p, &view, "18-pbr-before");
    // Enabling the mode alone does not overwrite any PBR data.
    p.materials[0].npr.enabled = true;
    p.materials[0].npr.specular_strength = 0.;
    p.materials[0].npr.rim_strength = 0.;
    p.materials[0].npr.bands = 2;
    p.materials[0].npr.softness = 0.;
    let shadow = material_picture(&mut renderer, &p, &view, "18-npr-shadow");
    p.lights[0].intensity = 0.8;
    let lit = material_picture(&mut renderer, &p, &view, "18-npr-lit");
    assert!(lit.get_pixel(80, 80)[0] > shadow.get_pixel(80, 80)[0] + 40);
    p.materials[0].npr.threshold = 0.9;
    let threshold = material_picture(&mut renderer, &p, &view, "18-npr-threshold");
    assert!(threshold.get_pixel(80, 80)[0] < lit.get_pixel(80, 80)[0]);
    p.materials[0].npr.threshold = 0.5;
    p.materials[0].base_color_texture = Some(10);
    let texture = material_picture(&mut renderer, &p, &view, "18-npr-texture");
    assert!(texture.get_pixel(80, 80)[0] > texture.get_pixel(80, 80)[2] + 40);
    p.materials[0].normal_texture = Some(13);
    let normal = material_picture(&mut renderer, &p, &view, "18-npr-normal");
    assert!(rgb_energy(&normal) < rgb_energy(&texture));
    p.materials[0].normal_texture = None;
    p.materials[0].base_color_texture = None;
    p.materials[0].alpha_mode = AlphaMode::Blend;
    p.materials[0].base_color[3] = 0.25;
    let alpha = material_picture(&mut renderer, &p, &view, "18-npr-alpha");
    assert!((63..=65).contains(&alpha.get_pixel(80, 80)[3]));
    p.materials[0].alpha_mode = AlphaMode::Mask;
    let mask = material_picture(&mut renderer, &p, &view, "18-npr-mask");
    assert_eq!(mask.get_pixel(80, 80)[3], 0);
    p.materials[0].alpha_mode = AlphaMode::Opaque;
    p.materials[0].base_color[3] = 1.;
    p.materials[0].npr.enabled = false;
    p.lights[0].intensity = 0.4;
    let restored = material_picture(&mut renderer, &p, &view, "18-pbr-restored");
    assert_images_equal(
        &pbr,
        &restored,
        "PBR toggle must preserve the previous image",
    );
    p.materials[0].npr.enabled = true;
    p.materials[0].npr.specular_strength = 1.;
    let highlight = material_picture(&mut renderer, &p, &view, "18-npr-highlight");
    assert!(rgb_energy(&highlight) > rgb_energy(&shadow));
    p.materials[0].npr.specular_strength = 0.;
    for vertex in &mut p.meshes[0].vertices {
        vertex.normal = glam::Vec3::new(0.9, 0., 0.25).normalize().to_array();
    }
    p.render_settings.ambient_intensity = 0.5;
    let edge_off = material_picture(&mut renderer, &p, &view, "18-npr-edge-off");
    p.materials[0].npr.rim_width = 0.8;
    p.materials[0].npr.rim_strength = 1.;
    let edge_on = material_picture(&mut renderer, &p, &view, "18-npr-edge-on");
    assert!(rgb_energy(&edge_on) > rgb_energy(&edge_off));
    p.materials[0].npr.rim_strength = 0.;
    p.materials[0].npr.ink_width = 0.5;
    let ink = material_picture(&mut renderer, &p, &view, "18-npr-ink");
    assert!(rgb_energy(&ink) < rgb_energy(&edge_off));
    for vertex in &mut p.meshes[0].vertices {
        vertex.normal = [0., 0., 1.];
    }
    for (index, preset) in [
        MaterialPreset::Toon,
        MaterialPreset::Anime,
        MaterialPreset::Ink,
    ]
    .into_iter()
    .enumerate()
    {
        preset.apply(&mut p.materials[0]);
        for kind in [
            LightKind::Directional,
            LightKind::Point,
            LightKind::Spot,
            LightKind::Area,
        ] {
            p.lights[0].kind = kind;
            p.lights[0].position = [0., 0., 2.].into();
            p.lights[0].intensity = 3.;
            p.lights[0].area_size = [2., 2.];
            let image = material_picture(
                &mut renderer,
                &p,
                &view,
                &format!("18-npr-{index}-{kind:?}"),
            );
            assert!(rgb_energy(&image) > 1000);
        }
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn npr_reflection_updates_with_the_authored_shadow_palette() {
    use poseforge_core::{box_mesh, Material, MaterialPreset, Node, Vec3};
    let (mut p, view) = material_scene();
    p.materials[0].base_color = [1.; 4];
    p.materials[0].metallic = 1.;
    p.materials[0].roughness = 0.045;
    let mut reflected = Material {
        id: 20,
        base_color: [1.; 4],
        ..MaterialPreset::Toon.create()
    };
    reflected.npr.bands = 2;
    reflected.npr.threshold = 0.9;
    reflected.npr.shadow_color = [0.6, 0.01, 0.01];
    reflected.npr.rim_strength = 0.;
    reflected.npr.specular_strength = 0.;
    p.materials.push(reflected);
    p.meshes
        .push(box_mesh(21, "NPR behind camera", Vec3::splat(2.), Some(20)));
    let mut node = Node::new(22, "NPR reflected object");
    node.mesh = Some(21);
    node.transform.translation = Vec3::new(0., 0., 5.);
    p.nodes.push(node);
    p.render_settings.ambient_intensity = 0.4;
    p.render_settings.sky_color = [1.; 3];
    p.render_settings.ground_color = [1.; 3];
    p.render_settings.ray_reflections = true;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let red = material_picture(&mut renderer, &p, &view, "18-npr-reflected-red");
    let pixel = red.get_pixel(80, 80);
    assert!(pixel[0] > pixel[2] + 40, "{pixel:?}");
    p.materials[1].npr.shadow_color = [0.01, 0.01, 0.6];
    let blue = material_picture(&mut renderer, &p, &view, "18-npr-reflected-blue");
    let pixel = blue.get_pixel(80, 80);
    assert!(pixel[2] > pixel[0] + 40, "{pixel:?}");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn geometry_reflection_sees_offscreen_emission_and_tracks_visibility() {
    use poseforge_core::{box_mesh, Material, Node, Vec3};
    let (mut p, view) = material_scene();
    p.materials[0].base_color = [1.; 4];
    p.materials[0].metallic = 1.;
    p.materials[0].roughness = 0.045;
    p.materials.push(Material {
        id: 20,
        emissive: [3., 0.02, 0.01],
        ..Material::default()
    });
    p.meshes
        .push(box_mesh(21, "Behind camera", Vec3::splat(2.), Some(20)));
    let mut node = Node::new(22, "Only in mirror");
    node.mesh = Some(21);
    node.transform.translation = Vec3::new(0., 0., 5.);
    p.nodes.push(node);
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let off = material_picture(&mut renderer, &p, &view, "rays-reflection-off");
    p.render_settings.ray_reflections = true;
    let on = material_picture(&mut renderer, &p, &view, "rays-reflection-on");
    let pixel = on.get_pixel(80, 80);
    assert!(
        pixel[0] > pixel[2] + 80,
        "offscreen red geometry must be reflected: {pixel:?}"
    );
    assert!(rgb_energy(&on) > rgb_energy(&off) + 10000);
    assert!(renderer.frame_stats().culled >= 1);
    p.materials[1].transmission = 1.;
    p.materials[1].geometric_thickness = true;
    let glass = material_picture(&mut renderer, &p, &view, "17-reflected-glass");
    assert!(
        glass.get_pixel(80, 80)[0] > glass.get_pixel(80, 80)[2] + 60,
        "transmissive geometry must remain visible in reflections"
    );
    p.materials[1].alpha_mode = poseforge_core::AlphaMode::Blend;
    p.materials[1].base_color[3] = 0.5;
    let blend = material_picture(&mut renderer, &p, &view, "17-reflected-blended-glass");
    assert!(blend.get_pixel(80, 80)[0] > blend.get_pixel(80, 80)[2] + 40);

    p.nodes.last_mut().unwrap().render_visible = false;
    let hidden = material_picture(&mut renderer, &p, &view, "rays-reflection-hidden");
    assert_images_equal(&hidden, &off, "hidden reflected object must disappear");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn hdri_linear_radiance_rotation_strength_and_transparent_alpha() {
    use poseforge_core::EnvironmentMap;
    let (mut p, view) = material_scene();
    p.render_settings.ambient_intensity = 1.;
    let rgb = (0..32)
        .flat_map(|_| {
            (0..64).map(|x| {
                if x < 32 {
                    [3., 0.04, 0.01]
                } else {
                    [0.01, 0.04, 3.]
                }
            })
        })
        .collect();
    p.environment_map = Some(std::sync::Arc::new(EnvironmentMap {
        name: "Procedural HDR".into(),
        width: 64,
        height: 32,
        rgb,
    }));
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let first = material_picture(&mut renderer, &p, &view, "hdri-zero");
    p.render_settings.hdri_rotation = 180.;
    let rotated = material_picture(&mut renderer, &p, &view, "hdri-rotated");
    assert!(first
        .pixels()
        .zip(rotated.pixels())
        .any(|(a, b)| a[0].abs_diff(b[0]) > 30));
    assert!(first
        .pixels()
        .zip(rotated.pixels())
        .all(|(a, b)| a[3] == b[3]));
    p.render_settings.hdri_strength = 0.;
    let dark = material_picture(&mut renderer, &p, &view, "hdri-black");
    assert_eq!(rgb_energy(&dark), 0);
    p.render_settings.hdri_enabled = false;
    let fallback = material_picture(&mut renderer, &p, &view, "hdri-disabled");
    assert!(rgb_energy(&fallback) > 10000);
    p.environment_map = None;
    assert_images_equal(
        &fallback,
        &material_picture(&mut renderer, &p, &view, "hdri-removed"),
        "disabled panorama must retain analytic environment",
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn material_graph_procedural_connections_reach_color_and_emission() {
    use poseforge_core::{MaterialGraph, MaterialNode, MaterialNodeKind};
    let (mut p, view) = material_scene();
    let mut graph = MaterialGraph::default();
    let mut red = MaterialNode::new(1, MaterialNodeKind::Color, [0., 0.]);
    red.value = [1., 0., 0., 1.];
    let mut blue = MaterialNode::new(2, MaterialNodeKind::Color, [0., 200.]);
    blue.value = [0., 0., 1., 1.];
    let mut checker = MaterialNode::new(3, MaterialNodeKind::Checker, [220., 0.]);
    checker.inputs = [Some(1), Some(2), None];
    graph.nodes = vec![checker, blue, red];
    graph.outputs[3] = Some(3);
    p.materials[0].graph = Some(graph);
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let image = material_picture(&mut renderer, &p, &view, "graph-checker");
    let reds = image.pixels().filter(|p| p[0] > 100 && p[2] < 20).count();
    let blues = image.pixels().filter(|p| p[2] > 100 && p[0] < 20).count();
    assert!(reds > 100 && blues > 100);
    p.materials[0].graph.as_mut().unwrap().enabled = false;
    let off = material_picture(&mut renderer, &p, &view, "graph-off");
    assert_eq!(rgb_energy(&off), 0);
    p.materials[0].graph.as_mut().unwrap().enabled = true;
    assert_images_equal(
        &image,
        &material_picture(&mut renderer, &p, &view, "graph-reset"),
        "graph toggle preserves nodes and appearance",
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn ray_area_shadows_use_emitter_segments_and_follow_cast_flags() {
    let (mut p, view) = local_shadow_scene();
    p.lights[0].kind = poseforge_core::LightKind::Area;
    p.lights[0].area_size = [2., 2.];
    p.lights[0].intensity = 10.;
    p.render_settings.ray_shadows = true;
    p.render_settings.area_shadow_samples = 16;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.lights[0].casts_shadow = false;
    let clear = material_picture(&mut renderer, &p, &view, "rays-shadow-clear");
    p.lights[0].casts_shadow = true;
    let shadow = material_picture(&mut renderer, &p, &view, "rays-shadow-16");
    assert!(darkened_pixels(&shadow, &clear) > 100);
    p.render_settings.area_shadow_samples = 64;
    let fine = material_picture(&mut renderer, &p, &view, "rays-shadow-64");
    assert!(darkened_pixels(&fine, &clear) > 100);
    p.nodes[1].transform.translation.x += 1.;
    let moved = material_picture(&mut renderer, &p, &view, "rays-shadow-moved");
    assert!(
        fine.pixels()
            .zip(moved.pixels())
            .filter(|(a, b)| a[0].abs_diff(b[0]) > 10)
            .count()
            > 100
    );
    p.nodes[1].transform.translation.x -= 1.;
    p.render_settings.area_shadow_samples = 16;
    p.lights[0].casts_shadow = false;
    assert_images_equal(
        &clear,
        &material_picture(&mut renderer, &p, &view, "rays-shadow-reset"),
        "cast flag must disable traced shadows",
    );
    p.lights[0].casts_shadow = true;
    p.materials[1].base_color = [1.; 4];
    p.materials[1].transmission = 1.;
    p.materials[1].attenuation_color = Some([1.; 3]);
    let neutral = material_picture(&mut renderer, &p, &view, "rays-glass-neutral");
    p.materials[1].attenuation_color = Some([0.95, 0.06, 0.02]);
    let tinted = material_picture(&mut renderer, &p, &view, "rays-glass-tinted");
    let colored_floor = clear
        .pixels()
        .zip(neutral.pixels())
        .zip(tinted.pixels())
        .filter(|((base, a), b)| {
            base[0].abs_diff(base[1]) < 5
                && base[1].abs_diff(base[2]) < 5
                && a[3] == 255
                && b[3] == 255
                && a[2] > b[2].saturating_add(15)
                && b[0] > b[2].saturating_add(15)
        })
        .count();
    assert!(
        colored_floor > 30,
        "colored glass must attenuate light on receiver: {colored_floor}"
    );
}

fn viewport_picture(
    renderer: &mut Renderer,
    project: &Project,
    view: &View,
    name: &str,
) -> image::RgbaImage {
    let directory = std::env::var_os("POSEFORGE_RENDER_QA_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("artforge-viewport-qa"));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join(format!("viewport-{name}.png"));
    renderer
        .capture_viewport_png(
            project,
            &project.global_transforms().unwrap(),
            view,
            &[],
            &[],
            &[],
            &file,
            160,
            160,
        )
        .unwrap();
    image::open(file).unwrap().into_rgba8()
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn checkerboard_is_viewport_only_dpi_aware_exposure_independent_and_not_in_png_alpha() {
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let mut project = Project::default();
    project.render_settings.transparent_background = true;
    let view = View {
        viewport: [12., 12., 132., 132.],
        ..View::default()
    };
    renderer.set_editor_appearance([0.01, 0.01, 0.01, 1.], true);
    let first = viewport_picture(&mut renderer, &project, &view, "checker-1x");
    assert_ne!(first.get_pixel(18, 18), first.get_pixel(30, 18));
    assert_eq!(first.get_pixel(18, 18), first.get_pixel(42, 18));
    assert_eq!(first.get_pixel(2, 2), first.get_pixel(158, 158));
    assert_ne!(first.get_pixel(2, 2), first.get_pixel(18, 18));
    assert!(first.pixels().all(|p| p[3] == 255));
    project.render_settings.exposure = 6.;
    assert_eq!(
        first,
        viewport_picture(&mut renderer, &project, &view, "checker-exposure")
    );
    renderer.set_editor_scale(2.);
    let double = viewport_picture(&mut renderer, &project, &view, "checker-2x");
    assert_eq!(double.get_pixel(18, 18), double.get_pixel(30, 18));
    assert_ne!(double.get_pixel(18, 18), double.get_pixel(42, 18));
    let alpha = material_picture(&mut renderer, &project, &view, "checker-export-alpha");
    assert!(
        alpha.pixels().all(|p| p[3] == 0),
        "checker never becomes artwork"
    );
    project.render_settings.transparent_background = false;
    renderer.set_editor_appearance([0.01, 0.01, 0.01, 1.], false);
    let opaque = viewport_picture(&mut renderer, &project, &view, "solid-background");
    assert!(opaque.pixels().all(|p| p == opaque.get_pixel(0, 0)));
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn helper_grid_does_not_occlude_meshes_or_change_lighting_and_shadows_below_zero() {
    let (mut project, mut view) = local_shadow_scene();
    for node in &mut project.nodes {
        node.transform.translation.y -= 2.;
    }
    view.target.y -= 2.;
    project.render_settings.transparent_background = false;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    renderer.set_editor_appearance([0.032, 0.043, 0.055, 1.], false);
    let without_grid = viewport_picture(&mut renderer, &project, &view, "below-grid-off");
    let alpha = material_picture(&mut renderer, &project, &view, "below-grid-alpha");
    renderer.set_editor_appearance([0.032, 0.043, 0.055, 1.], true);
    let with_grid = viewport_picture(&mut renderer, &project, &view, "below-grid-on");
    let object_pixels = alpha.pixels().filter(|p| p[3] == 255).count();
    assert!(
        object_pixels > 300,
        "authored meshes below zero must remain visible"
    );
    for ((a, b), mask) in without_grid
        .pixels()
        .zip(with_grid.pixels())
        .zip(alpha.pixels())
    {
        if mask[3] == 255 {
            assert_eq!(a, b, "helper cannot affect an opaque scene surface");
        }
    }
    project.render_settings.shadows = false;
    let lit = viewport_picture(&mut renderer, &project, &view, "below-grid-no-shadow");
    assert!(
        darkened_pixels(&with_grid, &lit) > 20,
        "physical caster shadows pass through the helper plane"
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn offscreen_skinning_alpha_and_dimensions() {
    let mut renderer = pollster::block_on(Renderer::headless(640, 480)).expect("GPU adapter");
    let project = Project::demo();
    project.validate().unwrap();
    let view = View {
        effects: Default::default(),
        depth_of_field: Default::default(),
        eye: project.camera.eye,
        target: project.camera.target,
        up: project.camera.up,
        fov_y: project.camera.fov_y_radians,
        viewport: [0., 0., 640., 480.],
        orthographic: None,
    };
    let directory = std::env::var_os("POSEFORGE_RENDER_QA_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/render-smoke")
        });
    std::fs::create_dir_all(&directory).unwrap();
    let first = directory.join("pose-0.png");
    let second = directory.join("pose-1.png");
    let clear = directory.join("transparent.png");
    let frame0 = project.evaluated(Some(0), 0.).unwrap();
    let frame1 = project.evaluated(Some(0), 1.).unwrap();
    renderer
        .export_png(
            &project,
            &frame0.global,
            &view,
            &[],
            &first,
            640,
            480,
            false,
        )
        .unwrap();
    renderer
        .export_png(
            &project,
            &frame1.global,
            &view,
            &[],
            &second,
            640,
            480,
            false,
        )
        .unwrap();
    renderer
        .export_png(&project, &frame1.global, &view, &[], &clear, 640, 480, true)
        .unwrap();
    let image0 = image::open(&first).unwrap().into_rgba8();
    let image1 = image::open(&second).unwrap().into_rgba8();
    let image_alpha = image::open(&clear).unwrap().into_rgba8();
    assert_eq!(image0.dimensions(), (640, 480));
    assert!(
        image0
            .pixels()
            .filter(|p| p[1] > p[0].saturating_add(25) && p[2] > 80)
            .count()
            > 500,
        "teal mannequin rendered"
    );
    assert!(
        image0
            .pixels()
            .zip(image1.pixels())
            .filter(|(a, b)| a != b)
            .count()
            > 200,
        "animated GPU skin changes visible pixels"
    );
    assert!(
        image_alpha.pixels().any(|p| p[3] == 0),
        "transparent backdrop"
    );
    assert!(
        image_alpha.pixels().any(|p| p[3] == 255),
        "opaque mesh retained"
    );
    println!(
        "Adapter: {}; verified renders: {}",
        renderer.adapter_name,
        directory.display()
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn imported_reordered_skin_and_morph_reach_the_gpu() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/skinned_triangle.glb");
    let mut project = poseforge_io::import_gltf(fixture).unwrap().project;
    let mut renderer = pollster::block_on(Renderer::headless(320, 320)).unwrap();
    let view = View {
        effects: Default::default(),
        depth_of_field: Default::default(),
        eye: project.camera.eye,
        target: project.camera.target,
        up: project.camera.up,
        fov_y: project.camera.fov_y_radians,
        viewport: [0., 0., 320., 320.],
        orthographic: None,
    };
    let directory = std::env::var_os("POSEFORGE_RENDER_QA_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/render-smoke")
        });
    std::fs::create_dir_all(&directory).unwrap();
    let mut pictures = vec![];
    for (index, time) in [0., 0.5].into_iter().enumerate() {
        let evaluated = project.evaluated(Some(0), time).unwrap();
        for node in &mut project.nodes {
            if let Some(w) = evaluated.morph_weights.get(&node.id) {
                node.morph_weights = w.clone();
            }
        }
        let path = directory.join(format!("imported-{index}.png"));
        renderer
            .export_png(
                &project,
                &evaluated.global,
                &view,
                &[],
                &path,
                320,
                320,
                true,
            )
            .unwrap();
        pictures.push(image::open(path).unwrap().into_rgba8());
    }
    assert!(
        pictures[0].pixels().filter(|p| p[3] > 200).count() > 100,
        "imported skin triangle visible at bind pose"
    );
    assert!(
        pictures[0]
            .pixels()
            .zip(pictures[1].pixels())
            .filter(|(a, b)| a != b)
            .count()
            > 100,
        "imported translation and morph animation changes output"
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn large_skin_palettes_resize_and_unbind_without_stale_deformation() {
    use poseforge_core::{Mat4, Node, Skin, Vec3};
    let (mut project, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let baseline = material_picture(&mut renderer, &project, &view, "223-unbound");
    for count in [2usize, 257, 1024, 1] {
        project.nodes.truncate(1);
        for i in 0..count {
            let mut joint = Node::new(100 + i as u64, "Joint");
            joint.is_joint = true;
            project.nodes.push(joint);
        }
        project.nodes[0].skin = Some(Skin {
            joints: (100..100 + count as u64).collect(),
            inverse_bind_matrices: vec![Mat4::IDENTITY; count],
        });
        for v in &mut project.meshes[0].vertices {
            v.joints = [(count - 1) as u32, u32::MAX, u32::MAX, u32::MAX];
            v.weights = [1., 0., 0., 0.];
        }
        let bind = material_picture(&mut renderer, &project, &view, &format!("223-bind-{count}"));
        assert_images_equal(
            &baseline,
            &bind,
            "dynamic skin palette preserves the bind pose",
        );
        project.nodes.last_mut().unwrap().transform.translation = Vec3::X * 0.3;
        let moved = material_picture(&mut renderer, &project, &view, &format!("223-move-{count}"));
        assert!(
            baseline
                .pixels()
                .zip(moved.pixels())
                .filter(|(a, b)| a != b)
                .count()
                > 100
        );
        // Preserve authored weights to check the explicit skin-enabled flag.
        project.nodes[0].skin = None;
        let unbound = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("223-unbind-{count}"),
        );
        assert_images_equal(
            &baseline,
            &unbound,
            "removed skin must not use the old palette",
        );
    }
}

/// Small authored triangle keeps material/light assertions independent of demo colors.
fn material_scene() -> (Project, View) {
    use glam::Vec3;
    use poseforge_core::{Material, Mesh, Node, Texture, Vertex};
    let mut project = Project::default();
    project.lights.clear();
    project.render_settings.ambient_intensity = 0.;
    project.materials.push(Material {
        id: 3,
        base_color: [0.6, 0.6, 0.6, 1.],
        roughness: 0.7,
        ..Material::default()
    });
    project.meshes.push(Mesh {
        id: 2,
        name: "PBR test triangle".into(),
        vertices: [
            ([-1., -1., 0.], [0., 1.]),
            ([1., -1., 0.], [1., 1.]),
            ([0., 1., 0.], [0.5, 0.]),
        ]
        .into_iter()
        .map(|(position, uv)| Vertex {
            position,
            uv,
            normal: [0., 0., 1.],
            ..Vertex::default()
        })
        .collect(),
        indices: vec![0, 1, 2].into(),
        material: Some(3),
        topology: None,
        morph_targets: vec![],
    });
    let mut node = Node::new(1, "PBR test");
    node.mesh = Some(2);
    project.nodes.push(node);
    for (id, name, rgba) in [
        (10, "red", [255, 0, 0, 255]),
        (11, "blue", [0, 0, 255, 255]),
        (12, "occluded", [0, 0, 0, 255]),
        (13, "tilted normal", [255, 128, 128, 255]),
        (14, "rough dielectric", [0, 255, 0, 255]),
    ] {
        project.textures.push(Texture {
            id,
            name: name.into(),
            width: 1,
            height: 1,
            rgba8: rgba.to_vec().into(),
        });
    }
    let view = View {
        effects: Default::default(),
        depth_of_field: Default::default(),
        eye: Vec3::new(0., 0., 3.),
        target: Vec3::ZERO,
        up: Vec3::Y,
        fov_y: 45f32.to_radians(),
        viewport: [0., 0., 160., 160.],
        orthographic: None,
    };
    (project, view)
}
fn material_picture(
    renderer: &mut Renderer,
    project: &Project,
    view: &View,
    name: &str,
) -> image::RgbaImage {
    material_picture_with_particles(renderer, project, view, name, &[])
}
fn material_picture_with_particles(
    renderer: &mut Renderer,
    project: &Project,
    view: &View,
    name: &str,
    particles: &[glam::Vec3],
) -> image::RgbaImage {
    project.validate().unwrap();
    let directory = std::env::var_os("POSEFORGE_RENDER_QA_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/render-smoke")
        });
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join(format!("pbr-{name}.png"));
    renderer
        .export_png(
            project,
            &project.global_transforms().unwrap(),
            view,
            particles,
            &file,
            160,
            160,
            true,
        )
        .unwrap();
    image::open(file).unwrap().into_rgba8()
}
fn assert_images_equal(a: &image::RgbaImage, b: &image::RgbaImage, message: &str) {
    assert_eq!(a.dimensions(), b.dimensions(), "{message}: dimensions");
    let changed = a.pixels().zip(b.pixels()).filter(|(a, b)| a != b).count();
    assert!(changed == 0, "{message}: {changed} pixels differ");
}

fn rgb_energy(picture: &image::RgbaImage) -> u64 {
    picture
        .pixels()
        .map(|p| u64::from(p[0]) + u64::from(p[1]) + u64::from(p[2]))
        .sum()
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn translucent_foreground_tints_particles_behind_it() {
    let (mut project, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.render_settings.tone_mapping = poseforge_core::ToneMapping::Linear;
    project.materials[0].base_color = [0., 0., 0., 0.5];
    project.materials[0].emissive = [1., 0., 0.];
    project.materials[0].alpha_mode = poseforge_core::AlphaMode::Blend;
    let picture = material_picture_with_particles(
        &mut renderer,
        &project,
        &view,
        "translucent-foreground-particle",
        &[glam::Vec3::new(0., 0., -0.15)],
    );
    let pixel = picture.get_pixel(80, 80);
    assert_eq!(
        pixel[3], 255,
        "opaque particle is visible through the transparent foreground"
    );
    assert!(
        pixel[0] > pixel[2],
        "red translucent surface must tint the blue particle behind it: {pixel:?}"
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn rough_white_metal_preserves_energy_under_white_environment() {
    let (mut project, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.render_settings.tone_mapping = poseforge_core::ToneMapping::Linear;
    project.render_settings.ambient_intensity = 1.;
    project.render_settings.sky_color = [1.; 3];
    project.render_settings.ground_color = [1.; 3];
    project.materials[0].base_color = [1.; 4];
    project.materials[0].metallic = 1.;
    for (index, roughness) in [0.1, 0.5, 1.].into_iter().enumerate() {
        project.materials[0].roughness = roughness;
        let picture = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("white-metal-{index}"),
        );
        assert!(
            picture.get_pixel(80, 80).0[..3]
                .iter()
                .all(|channel| *channel >= 253),
            "a perfectly reflecting metal must not lose energy as roughness increases"
        );
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn hdr_transparency_is_composited_before_tone_mapping() {
    use poseforge_core::{AlphaMode, Material, Node, ToneMapping};
    let (mut project, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.materials[0].base_color = [0., 0., 0., 1.];
    let mut foreground = project.meshes[0].clone();
    foreground.id = 5;
    foreground.material = Some(6);
    project.meshes.push(foreground);
    project.materials.push(Material {
        id: 6,
        base_color: [0., 0., 0., 0.25],
        emissive: [4.; 3],
        alpha_mode: AlphaMode::Blend,
        ..Material::default()
    });
    let mut node = Node::new(4, "HDR translucent foreground");
    node.mesh = Some(5);
    node.transform.translation.z = 0.1;
    project.nodes.push(node);
    for (index, curve) in [
        ToneMapping::Aces,
        ToneMapping::Reinhard,
        ToneMapping::Linear,
    ]
    .into_iter()
    .enumerate()
    {
        project.render_settings.tone_mapping = curve;
        project.materials[1].base_color[3] = 0.25;
        project.materials[1].emissive = [4.; 3];
        let blended = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("hdr-blend-{index}"),
        );
        project.materials[1].base_color[3] = 1.;
        project.materials[1].emissive = [1.; 3];
        let equivalent = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("hdr-opaque-{index}"),
        );
        for channel in 0..4 {
            assert!(
                blended.get_pixel(80, 80)[channel].abs_diff(equivalent.get_pixel(80, 80)[channel])
                    <= 1,
                "0.25 * HDR4 over black must equal HDR1 before the display curve"
            );
        }
    }
    // Straight-alpha PNG retains the foreground's displayed color, independent of opacity.
    project.nodes[0].render_visible = false;
    project.materials[1].base_color[3] = 0.25;
    project.materials[1].emissive = [0.5; 3];
    let translucent = material_picture(&mut renderer, &project, &view, "hdr-straight-alpha");
    project.materials[1].base_color[3] = 1.;
    let opaque = material_picture(&mut renderer, &project, &view, "hdr-straight-opaque");
    assert!((63..=65).contains(&translucent.get_pixel(80, 80)[3]));
    for channel in 0..3 {
        assert!(
            translucent.get_pixel(80, 80)[channel].abs_diff(opaque.get_pixel(80, 80)[channel]) <= 2,
            "transparent PNG unpremultiplication preserves scene color"
        );
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn minified_color_texture_uses_linear_light_mipmaps() {
    use poseforge_core::{Texture, ToneMapping};
    let (mut project, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.render_settings.tone_mapping = ToneMapping::Linear;
    project.materials[0].emissive = [1.; 3];
    project.materials[0].emissive_texture = Some(30);
    let mut pixels = vec![];
    for y in 0..32 {
        for x in 0..32 {
            let value = if (x + y) % 2 == 0 { 0 } else { 255 };
            pixels.extend_from_slice(&[value, value, value, 0]);
        }
    }
    project.textures.push(Texture {
        id: 30,
        name: "minified checker".into(),
        width: 32,
        height: 32,
        rgba8: pixels.into(),
    });
    for vertex in &mut project.meshes[0].vertices {
        vertex.uv[0] *= 128.;
        vertex.uv[1] *= 128.;
    }
    // Emission ignores the source image's alpha, including lower mip levels.
    let picture = material_picture(&mut renderer, &project, &view, "linear-mip-checker");
    for y in 60..100 {
        for x in 70..90 {
            let pixel = picture.get_pixel(x, y);
            assert_eq!(pixel[3], 255);
            assert!((186..=190).contains(&pixel[0]),
                "averaging black and white in linear light should encode near sRGB188, got {pixel:?}");
        }
    }
    // Opaque base color also ignores alpha. Changing alpha mode must rebuild the
    // semantic binding, then changing back must recover the original color chain.
    project.materials[0].emissive = [0.; 3];
    project.materials[0].emissive_texture = None;
    project.materials[0].base_color_texture = Some(30);
    project.materials[0].base_color = [1.; 4];
    project.render_settings.ambient_intensity = 1.;
    project.render_settings.sky_color = [1.; 3];
    project.render_settings.ground_color = [1.; 3];
    let opaque = material_picture(&mut renderer, &project, &view, "mip-opaque-ignores-alpha");
    assert!(opaque.get_pixel(80, 80)[0] > 170);
    project.materials[0].alpha_mode = poseforge_core::AlphaMode::Mask;
    let masked = material_picture(&mut renderer, &project, &view, "mip-mask-honors-alpha");
    assert!(masked.pixels().all(|pixel| pixel[3] == 0));
    project.materials[0].alpha_mode = poseforge_core::AlphaMode::Opaque;
    let restored = material_picture(&mut renderer, &project, &view, "mip-opaque-mode-restored");
    assert_images_equal(
        &opaque,
        &restored,
        "switching alpha mode restores opaque semantic binding",
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn material_maps_lights_exposure_and_live_binding_changes_reach_gpu() {
    use glam::Vec3;
    use poseforge_core::{Light, LightKind};
    let (mut project, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    let dark = material_picture(&mut renderer, &project, &view, "unlit");
    assert_eq!(
        rgb_energy(&dark),
        0,
        "no hidden default lamp with empty lights and no ambient"
    );
    project.lights.push(Light {
        direction: -Vec3::Z,
        intensity: 1.,
        color: [1.; 3],
        ..Light::default()
    });
    let lit = material_picture(&mut renderer, &project, &view, "directional");
    assert!(rgb_energy(&lit) > 100_000);
    project.render_settings.exposure = -2.;
    let dim = material_picture(&mut renderer, &project, &view, "exposure");
    assert!(rgb_energy(&dim) < rgb_energy(&lit));
    project.render_settings.exposure = 0.;
    project.lights.push(Light {
        kind: LightKind::Point,
        position: Vec3::new(0., 0., 2.),
        intensity: 4.,
        color: [0.1, 0.2, 1.],
        ..Light::default()
    });
    let two = material_picture(&mut renderer, &project, &view, "two-lights");
    assert!(
        rgb_energy(&two) > rgb_energy(&lit),
        "second light contributes"
    );
    project.lights[1].enabled = false;
    assert_images_equal(
        &(material_picture(&mut renderer, &project, &view, "disabled")),
        &(lit),
        "images must match exactly",
    );
    project.materials[0].base_color_texture = Some(10);
    let red = material_picture(&mut renderer, &project, &view, "base-red");
    project.materials[0].base_color_texture = Some(11);
    let blue = material_picture(&mut renderer, &project, &view, "base-blue");
    assert_ne!(
        red, blue,
        "slot reassignment changes binding without full invalidation"
    );
    project.materials[0].base_color_texture = None;
    project.materials[0].normal_texture = Some(13);
    let normal = material_picture(&mut renderer, &project, &view, "normal");
    assert!(
        rgb_energy(&normal) < rgb_energy(&lit) / 2,
        "normal map turns surface away from lamp"
    );
    project.materials[0].normal_texture = None;
    project.materials[0].metallic = 1.;
    let metallic = material_picture(&mut renderer, &project, &view, "metallic");
    project.materials[0].metallic_roughness_texture = Some(14);
    let dielectric = material_picture(&mut renderer, &project, &view, "metallic-map");
    assert_ne!(
        metallic, dielectric,
        "linear metallic-roughness data affects BRDF"
    );
    project.materials[0].metallic = 0.;
    project.materials[0].metallic_roughness_texture = None;
    project.lights.clear();
    project.render_settings.ambient_intensity = 1.;
    let ambient = material_picture(&mut renderer, &project, &view, "ambient");
    project.materials[0].occlusion_texture = Some(12);
    assert_eq!(
        rgb_energy(&material_picture(&mut renderer, &project, &view, "ao")),
        0
    );
    project.materials[0].occlusion_strength = 0.;
    assert_images_equal(
        &(material_picture(&mut renderer, &project, &view, "ao-disabled")),
        &(ambient),
        "images must match exactly",
    );
    project.render_settings.ambient_intensity = 0.;
    project.materials[0].emissive = [1.; 3];
    project.materials[0].emissive_texture = Some(10);
    let emission = material_picture(&mut renderer, &project, &view, "emissive");
    assert!(emission
        .pixels()
        .any(|p| p[0] > 100 && p[1] == 0 && p[2] == 0));
    project.materials[0].emissive_texture = None;
    project.materials[0].emissive = [f32::MAX; 3];
    project.render_settings.exposure = 16.;
    let hdr = material_picture(&mut renderer, &project, &view, "extreme-emission");
    assert!(
        hdr.pixels()
            .filter(|p| p[3] == 255)
            .all(|p| p[0] > 250 && p[1] > 250 && p[2] > 250),
        "finite HDR overflow saturates to white rather than producing NaN/black"
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn point_range_spot_direction_and_material_backfaces_reach_gpu() {
    use glam::Vec3;
    use poseforge_core::{Light, LightKind};
    let (mut project, mut view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.lights.push(Light {
        kind: LightKind::Point,
        position: Vec3::new(0., 0., 2.),
        intensity: 4.,
        range: 1.,
        color: [1.; 3],
        ..Light::default()
    });
    assert_eq!(
        rgb_energy(&material_picture(
            &mut renderer,
            &project,
            &view,
            "point-out-of-range"
        )),
        0
    );
    project.lights[0].range = 0.;
    assert!(
        rgb_energy(&material_picture(
            &mut renderer,
            &project,
            &view,
            "point-unlimited"
        )) > 100_000
    );
    project.lights[0].kind = LightKind::Spot;
    project.lights[0].direction = Vec3::Z;
    assert_eq!(
        rgb_energy(&material_picture(
            &mut renderer,
            &project,
            &view,
            "spot-away"
        )),
        0
    );
    project.lights[0].direction = -Vec3::Z;
    assert!(
        rgb_energy(&material_picture(
            &mut renderer,
            &project,
            &view,
            "spot-towards"
        )) > 100_000
    );
    project.nodes[0].transform.scale.x = -1.;
    let mirrored = material_picture(&mut renderer, &project, &view, "mirrored-front");
    assert!(mirrored.pixels().filter(|p| p[3] > 200).count() > 100);
    view.eye.z = -3.;
    let culled = material_picture(&mut renderer, &project, &view, "back-culled");
    assert!(culled.pixels().all(|p| p[3] == 0));
    project.materials[0].double_sided = true;
    let two_sided = material_picture(&mut renderer, &project, &view, "back-visible");
    assert!(two_sided.pixels().filter(|p| p[3] > 200).count() > 100);
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn tone_mapping_colored_environment_and_typed_texture_sampling_reach_gpu() {
    use poseforge_core::{Texture, ToneMapping};
    let (mut project, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.materials[0].emissive = [0.3, 0.6, 1.];
    let aces = material_picture(&mut renderer, &project, &view, "tone-aces");
    project.render_settings.tone_mapping = ToneMapping::Reinhard;
    let reinhard = material_picture(&mut renderer, &project, &view, "tone-reinhard");
    project.render_settings.tone_mapping = ToneMapping::Linear;
    let linear = material_picture(&mut renderer, &project, &view, "tone-linear");
    assert_ne!(aces, reinhard);
    assert_ne!(linear, reinhard);

    project.textures.push(Texture {
        id: 20,
        name: "gray sRGB".into(),
        width: 1,
        height: 1,
        rgba8: vec![128, 128, 128, 255].into(),
    });
    project.materials[0].emissive = [1.; 3];
    project.materials[0].emissive_texture = Some(20);
    let gray = material_picture(&mut renderer, &project, &view, "typed-srgb-gray");
    let center = gray.get_pixel(80, 80);
    assert!(
        (126..=130).contains(&center[0]),
        "sRGB decode followed by linear tone/output must roundtrip gray128: {center:?}"
    );
    project.materials[0].emissive = [0.; 3];
    project.materials[0].emissive_texture = None;
    project.render_settings.ambient_intensity = 1.;
    project.render_settings.sky_color = [1., 0., 0.];
    project.render_settings.ground_color = [1., 0., 0.];
    let red = material_picture(&mut renderer, &project, &view, "environment-red");
    project.render_settings.sky_color = [0., 0., 1.];
    project.render_settings.ground_color = [0., 0., 1.];
    let blue = material_picture(&mut renderer, &project, &view, "environment-blue");
    assert!(red.get_pixel(80, 80)[0] > 100 && red.get_pixel(80, 80)[2] == 0);
    assert!(blue.get_pixel(80, 80)[2] > 100 && blue.get_pixel(80, 80)[0] == 0);
    project.materials[0].occlusion_texture = Some(20);
    let ao = material_picture(&mut renderer, &project, &view, "typed-linear-gray");
    assert!(
        ao.get_pixel(80, 80)[2] as f32 > blue.get_pixel(80, 80)[2] as f32 * 0.68,
        "AO reads128 as linear data, not as an sRGB-decoded0.216"
    );
    println!(
        "Texture fallback={}, adapter={}",
        renderer.uses_compatible_texture_uploads(),
        renderer.adapter_name
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn directional_shadows_visibility_alpha_masks_and_antialiasing_reach_gpu() {
    use glam::Vec3;
    use poseforge_core::{box_mesh, AlphaMode, Material, Node};
    let mut project = Project::default();
    project.render_settings.ambient_intensity = 0.2;
    project.render_settings.shadow_strength = 1.;
    // Project the shadow toward the camera so its receiver is visible beside
    // the occluder, independent of changes to the project's default lamp.
    project.lights[0].direction = Vec3::new(0.8, -1., 0.4).normalize();
    project.materials.push(Material {
        id: 1,
        base_color: [0.7, 0.7, 0.7, 1.],
        roughness: 0.8,
        ..Material::default()
    });
    project.materials.push(Material {
        id: 2,
        base_color: [0.5, 0.25, 0.1, 1.],
        roughness: 0.6,
        ..Material::default()
    });
    project
        .meshes
        .push(box_mesh(3, "Receiver", Vec3::new(5., 0.1, 5.), Some(1)));
    project
        .meshes
        .push(box_mesh(4, "Caster", Vec3::splat(0.8), Some(2)));
    let mut receiver = Node::new(5, "Receiver");
    receiver.mesh = Some(3);
    let mut caster = Node::new(6, "Caster");
    caster.mesh = Some(4);
    caster.transform.translation.y = 0.8;
    project.nodes.extend([receiver, caster]);
    let view = View {
        eye: Vec3::new(3., 3., 5.),
        target: Vec3::new(0., 0.3, 0.),
        viewport: [0., 0., 160., 160.],
        ..View::default()
    };
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.render_settings.shadows = false;
    let clear = material_picture(&mut renderer, &project, &view, "shadow-off");
    project.render_settings.shadows = true;
    let shadow = material_picture(&mut renderer, &project, &view, "shadow-on");
    assert!(
        shadow
            .pixels()
            .zip(clear.pixels())
            .filter(|(a, b)| a[0].saturating_add(12) < b[0])
            .count()
            > 40,
        "a real occluder darkens the authored receiver"
    );
    assert!(rgb_energy(&shadow) < rgb_energy(&clear));
    project.render_settings.shadow_strength = 0.;
    assert_images_equal(
        &(material_picture(&mut renderer, &project, &view, "shadow-zero")),
        &(clear),
        "images must match exactly",
    );
    project.render_settings.shadow_strength = 1.;
    for resolution in [512, 2048] {
        project.render_settings.shadow_resolution = resolution;
        let result = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("shadow-{resolution}"),
        );
        assert!(rgb_energy(&result) < rgb_energy(&clear));
    }
    project.nodes[1].render_visible = false;
    project.render_settings.shadows = false;
    let hidden_clear = material_picture(&mut renderer, &project, &view, "hidden-caster-clear");
    project.render_settings.shadows = true;
    let hidden_shadow = material_picture(&mut renderer, &project, &view, "hidden-caster-shadow");
    assert_images_equal(
        &(hidden_shadow),
        &(hidden_clear),
        "hidden objects do not cast shadows",
    );
    project.nodes[1].render_visible = true;
    project.materials[1].alpha_mode = AlphaMode::Mask;
    project.materials[1].base_color[3] = 0.;
    assert_images_equal(
        &(material_picture(&mut renderer, &project, &view, "masked-caster")),
        &(hidden_clear),
        "transparent alpha-mask texels do not cast opaque shadows",
    );
    project.materials[1].alpha_mode = AlphaMode::Blend;
    assert_images_equal(
        &(material_picture(&mut renderer, &project, &view, "blend-caster")),
        &(hidden_clear),
        "alpha-blend material is excluded from opaque shadow casting",
    );
    project.render_settings.antialiasing = 1;
    let single = material_picture(&mut renderer, &project, &view, "aa-one");
    assert!(single.pixels().all(|p| p[3] == 0 || p[3] == 255));
    project.render_settings.antialiasing = 4;
    let multi = material_picture(&mut renderer, &project, &view, "aa-four");
    if renderer.effective_antialiasing(4) == 4 {
        assert!(multi.pixels().any(|p| p[3] > 0 && p[3] < 255));
    }
    println!(
        "MSAA requested4/effective{}, adapter={}",
        renderer.effective_antialiasing(4),
        renderer.adapter_name
    );
}

fn local_shadow_scene() -> (Project, View) {
    use glam::Vec3;
    use poseforge_core::{box_mesh, Light, LightKind, Material, Node};
    let mut project = Project::default();
    project.render_settings.ambient_intensity = 0.1;
    project.render_settings.shadow_strength = 1.;
    project.render_settings.shadow_resolution = 512;
    project.lights = vec![Light {
        kind: LightKind::Point,
        position: Vec3::new(-1.5, 3., -2.),
        direction: Vec3::new(1.5, -3., 2.).normalize(),
        intensity: 35.,
        range: 12.,
        inner_cone_angle: 0.55,
        outer_cone_angle: 0.9,
        ..Light::default()
    }];
    project.materials = vec![
        Material {
            id: 1,
            base_color: [0.65, 0.65, 0.65, 1.],
            roughness: 0.8,
            ..Material::default()
        },
        Material {
            id: 2,
            base_color: [0.3, 0.1, 0.05, 1.],
            roughness: 0.6,
            ..Material::default()
        },
    ];
    project.meshes = vec![
        box_mesh(3, "Receiver", Vec3::new(5., 0.1, 5.), Some(1)),
        box_mesh(4, "Caster", Vec3::splat(0.65), Some(2)),
    ];
    let mut receiver = Node::new(5, "Receiver");
    receiver.mesh = Some(3);
    let mut caster = Node::new(6, "Caster");
    caster.mesh = Some(4);
    caster.transform.translation.y = 0.9;
    project.nodes = vec![receiver, caster];
    let view = View {
        eye: Vec3::new(3., 3., 5.),
        target: Vec3::new(0., 0.3, 0.),
        viewport: [0., 0., 160., 160.],
        ..View::default()
    };
    (project, view)
}

fn darkened_pixels(on: &image::RgbaImage, off: &image::RgbaImage) -> usize {
    on.pixels()
        .zip(off.pixels())
        .filter(|(a, b)| {
            a.0[..3].iter().map(|v| u32::from(*v)).sum::<u32>() + 24
                < b.0[..3].iter().map(|v| u32::from(*v)).sum::<u32>()
        })
        .count()
}

fn shadow_centroid(on: &image::RgbaImage, off: &image::RgbaImage) -> (f64, f64) {
    let mut weight = 0.;
    let mut x = 0.;
    let mut y = 0.;
    for (px, py, pixel) in off.enumerate_pixels() {
        let other = on.get_pixel(px, py);
        let difference = pixel.0[..3].iter().map(|v| i32::from(*v)).sum::<i32>()
            - other.0[..3].iter().map(|v| i32::from(*v)).sum::<i32>();
        if difference > 24 {
            let w = f64::from(difference);
            weight += w;
            x += f64::from(px) * w;
            y += f64::from(py) * w;
        }
    }
    assert!(weight > 0., "a shadow mask must contain darkened pixels");
    (x / weight, y / weight)
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn point_and_spot_shadows_follow_light_motion_cast_flags_and_masked_casters() {
    use glam::Vec3;
    use poseforge_core::{AlphaMode, LightKind};
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    for kind in [LightKind::Point, LightKind::Spot] {
        let (mut project, view) = local_shadow_scene();
        project.lights[0].kind = kind;
        let prefix = if kind == LightKind::Point {
            "point"
        } else {
            "spot"
        };
        project.render_settings.shadows = false;
        let clear = material_picture(&mut renderer, &project, &view, &format!("{prefix}-clear"));
        project.render_settings.shadows = true;
        let on = material_picture(&mut renderer, &project, &view, &format!("{prefix}-shadow"));
        assert!(
            darkened_pixels(&on, &clear) > 30,
            "{prefix} must shadow the receiver"
        );
        let stats = renderer.shadow_stats();
        assert_eq!(stats.shadow_lights, 1);
        assert_eq!(stats.layers, if kind == LightKind::Point { 6 } else { 1 });
        assert_eq!(stats.effective_resolution, 512);
        let first = shadow_centroid(&on, &clear);
        project.lights[0].casts_shadow = false;
        assert_images_equal(
            &(material_picture(
                &mut renderer,
                &project,
                &view,
                &format!("{prefix}-cast-off"),
            )),
            &(clear),
            "Cast Shadow must leave direct illumination intact",
        );
        assert_eq!(renderer.shadow_stats().layers, 0);
        project.lights[0].casts_shadow = true;
        project.lights[0].position = Vec3::new(1.8, 3., -2.);
        project.lights[0].direction = (-project.lights[0].position).normalize();
        project.render_settings.shadows = false;
        let moved_clear = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("{prefix}-moved-clear"),
        );
        project.render_settings.shadows = true;
        let moved = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("{prefix}-moved-shadow"),
        );
        assert!(darkened_pixels(&moved, &moved_clear) > 30);
        let next = shadow_centroid(&moved, &moved_clear);
        assert!(
            (first.0 - next.0).hypot(first.1 - next.1) > 2.,
            "shadow position must follow {prefix} motion"
        );
        project.materials[1].alpha_mode = AlphaMode::Mask;
        project.materials[1].base_color[3] = 0.;
        project.render_settings.shadows = false;
        let mask_clear = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("{prefix}-cutout-clear"),
        );
        project.render_settings.shadows = true;
        for radius in [0., 1., 4.] {
            project.render_settings.shadow_filter_radius = radius;
            let mask = material_picture(
                &mut renderer,
                &project,
                &view,
                &format!("{prefix}-cutout-filter-{radius}"),
            );
            assert_images_equal(
                &mask,
                &mask_clear,
                &format!("fully transparent {prefix} cutout cannot block light at filter radius {radius}"),
            );
        }
        println!(
            "{prefix}: darkened={}, moved-centroid={:.2}px; layers={}, adapter={}",
            darkened_pixels(&on, &clear),
            (first.0 - next.0).hypot(first.1 - next.1),
            stats.layers,
            renderer.adapter_name
        );
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn multiple_light_shadows_keep_packed_indices_and_budget_after_light_changes() {
    use glam::Vec3;
    use poseforge_core::{Light, LightKind};
    let (mut project, view) = local_shadow_scene();
    let mut point = project.lights[0].clone();
    point.intensity = 25.;
    let spot = Light {
        kind: LightKind::Spot,
        position: Vec3::new(1.8, 3., -2.),
        direction: Vec3::new(-1.8, -3., 2.).normalize(),
        intensity: 30.,
        range: 12.,
        inner_cone_angle: 0.55,
        outer_cone_angle: 0.9,
        ..Light::default()
    };
    project.lights = vec![
        Light {
            enabled: false,
            ..Light::default()
        },
        Light {
            casts_shadow: false,
            intensity: 0.25,
            ..Light::default()
        },
        point,
        spot,
    ];
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    project.render_settings.shadows = false;
    let clear = material_picture(&mut renderer, &project, &view, "multi-clear");
    project.render_settings.shadows = true;
    project.lights[3].casts_shadow = false;
    let point_only = material_picture(&mut renderer, &project, &view, "multi-point-shadow");
    assert!(darkened_pixels(&point_only, &clear) > 20);
    project.lights[2].casts_shadow = false;
    project.lights[3].casts_shadow = true;
    let spot_only = material_picture(&mut renderer, &project, &view, "multi-spot-shadow");
    assert!(darkened_pixels(&spot_only, &clear) > 20);
    project.lights[2].casts_shadow = true;
    let all = material_picture(&mut renderer, &project, &view, "multi-both-shadows");
    assert!(rgb_energy(&all) < rgb_energy(&point_only));
    assert!(rgb_energy(&all) < rgb_energy(&spot_only));
    assert_eq!(renderer.shadow_stats().shadow_lights, 2);
    assert_eq!(renderer.shadow_stats().layers, 7);
    project.lights[2].enabled = false;
    let disabled = material_picture(&mut renderer, &project, &view, "multi-point-disabled");
    assert_eq!(renderer.shadow_stats().layers, 1);
    project.lights.remove(2);
    assert_images_equal(
        &(disabled),
        &(material_picture(&mut renderer, &project, &view, "multi-point-removed")),
        "removing a disabled light must not shift a different shadow onto the spot",
    );
    // All eight sources still cast; only map resolution adapts to the budget.
    project.lights = vec![
        Light {
            kind: LightKind::Point,
            position: Vec3::new(-1.5, 3., -2.),
            intensity: 3.,
            range: 12.,
            ..Light::default()
        };
        8
    ];
    project.render_settings.shadow_resolution = 2048;
    material_picture(&mut renderer, &project, &view, "eight-point-shadows");
    let stats = renderer.shadow_stats();
    assert_eq!(stats.shadow_lights, 8);
    assert_eq!(stats.layers, 48);
    assert_eq!(stats.requested_resolution, 2048);
    assert_eq!(stats.effective_resolution, 512);
    assert!(stats.bytes <= 128 * 1024 * 1024);
    println!(
        "Shadow budget: {:?}, adapter={}",
        stats, renderer.adapter_name
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn point_shadow_depth_cameras_cover_all_six_world_directions() {
    use glam::Vec3;
    use poseforge_core::{box_mesh, Light, LightKind};
    let (mut project, mut view) = local_shadow_scene();
    project.lights = vec![Light {
        kind: LightKind::Point,
        position: Vec3::ZERO,
        intensity: 35.,
        range: 10.,
        ..Light::default()
    }];
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    let directions = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::X, Vec3::Y),
    ];
    for (face, (axis, tangent, up)) in directions.into_iter().enumerate() {
        let size = Vec3::splat(2.) - axis.abs() * 1.95;
        project.meshes[0] = box_mesh(3, "Receiver", size, Some(1));
        project.meshes[1] = box_mesh(4, "Caster", Vec3::splat(0.2), Some(2));
        project.nodes[0].transform.translation = axis * 3.;
        project.nodes[1].transform.translation = axis * 1.5;
        view.eye = axis * 0.4 + tangent * 1.;
        view.target = axis * 3.;
        view.up = up;
        renderer.invalidate_assets();
        project.render_settings.shadows = false;
        project.nodes[1].render_visible = false;
        let receiver_only = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("point-face-{face}-receiver"),
        );
        project.render_settings.shadows = true;
        let receiver_shadow = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("point-face-{face}-receiver-shadow"),
        );
        assert_images_equal(
            &receiver_only,
            &receiver_shadow,
            &format!("point face {face}: a plane without occluders must remain illuminated"),
        );
        project.nodes[1].render_visible = true;
        project.render_settings.shadows = false;
        let clear = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("point-face-{face}-clear"),
        );
        project.render_settings.shadows = true;
        let on = material_picture(
            &mut renderer,
            &project,
            &view,
            &format!("point-face-{face}-shadow"),
        );
        let receiver_pixels: Vec<_> = clear
            .pixels()
            .zip(receiver_only.pixels())
            .zip(on.pixels())
            .filter(|((clear, receiver), _)| receiver[3] > 0 && clear == receiver)
            .map(|((clear, _), shadow)| (clear, shadow))
            .collect();
        let visible = receiver_pixels.len();
        let darkened = receiver_pixels
            .iter()
            .filter(|(clear, shadow)| {
                shadow.0[..3].iter().map(|v| u32::from(*v)).sum::<u32>() + 24
                    < clear.0[..3].iter().map(|v| u32::from(*v)).sum::<u32>()
            })
            .count();
        assert!(
            darkened > 20 && darkened < visible / 2,
            "point face {face}: shadow must be localized, darkened={darkened}, visible={visible}"
        );
        assert_eq!(renderer.shadow_stats().layers, 6);
        println!("Point face {face}: {darkened}/{visible} visible receiver pixels darkened");
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn dielectric_ior_and_coating_change_the_brdf_without_stale_material_cache() {
    use glam::Vec3;
    use poseforge_core::Light;
    let (mut project, view) = material_scene();
    project.render_settings.shadows = false;
    project.lights = vec![Light {
        direction: Vec3::NEG_Z,
        intensity: 1.,
        ..Light::default()
    }];
    project.materials[0].base_color = [0.12, 0.24, 0.4, 1.];
    project.materials[0].roughness = 0.4;
    project.materials[0].ior = 1.;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).expect("GPU adapter");
    let no_interface = material_picture(&mut renderer, &project, &view, "ior-one");
    project.materials[0].ior = 2.;
    let reflected = material_picture(&mut renderer, &project, &view, "ior-two");
    assert!(rgb_energy(&reflected) > rgb_energy(&no_interface));
    project.materials[0].ior = 1.5;
    let plain = material_picture(&mut renderer, &project, &view, "coat-none");
    project.materials[0].coat_weight = 1.;
    project.materials[0].coat_roughness = 0.08;
    let polished = material_picture(&mut renderer, &project, &view, "coat-polished");
    assert_ne!(plain, polished);
    project.materials[0].coat_roughness = 0.7;
    let rough = material_picture(&mut renderer, &project, &view, "coat-rough");
    assert_ne!(polished, rough);
    project.materials[0].coat_weight = 0.;
    assert_images_equal(
        &(plain),
        &(material_picture(&mut renderer, &project, &view, "coat-removed")),
        "images must match exactly",
    );
    project.materials[0].metallic = 1.;
    project.materials[0].ior = 1.;
    let metal = material_picture(&mut renderer, &project, &view, "metal-ior-one");
    project.materials[0].ior = 3.;
    assert_images_equal(
        &(metal),
        &(material_picture(&mut renderer, &project, &view, "metal-ior-three")),
        "dielectric IOR must not change a fully metallic base layer",
    );
    println!(
        "IOR energy {} -> {}; coating live update verified, adapter={}",
        rgb_energy(&no_interface),
        rgb_energy(&reflected),
        renderer.adapter_name
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn rectangular_area_lights_integrate_size_orientation_and_live_changes() {
    use glam::Vec3;
    use poseforge_core::{Light, LightKind, ToneMapping};
    let (mut project, view) = material_scene();
    project.render_settings.tone_mapping = ToneMapping::Linear;
    project.materials[0].roughness = 0.8;
    project.lights = vec![Light {
        kind: LightKind::Area,
        position: Vec3::new(0., 0., 2.),
        direction: Vec3::NEG_Z,
        area_size: [0.5, 0.5],
        color: [1., 1., 1.],
        intensity: 2.,
        casts_shadow: false,
        ..Light::default()
    }];
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let small = material_picture(&mut renderer, &project, &view, "area-small");
    project.lights[0].area_size = [2., 2.];
    let large = material_picture(&mut renderer, &project, &view, "area-large");
    assert!(
        rgb_energy(&large) > rgb_energy(&small) * 2,
        "radiance held constant: larger emitter delivers more light"
    );
    project.lights[0].direction = Vec3::Z;
    let back = material_picture(&mut renderer, &project, &view, "area-back");
    assert_eq!(
        rgb_energy(&back),
        0,
        "one-sided emitter must not illuminate behind itself"
    );
    project.lights[0].direction = Vec3::NEG_Z;
    project.lights[0].range = 0.5;
    let ranged = material_picture(&mut renderer, &project, &view, "area-out-of-range");
    assert_eq!(rgb_energy(&ranged), 0);
    project.lights[0].range = 0.;
    project.lights[0].area_size = [0.5, 0.5];
    let restored = material_picture(&mut renderer, &project, &view, "area-restored");
    assert_images_equal(
        &small,
        &restored,
        "area dimensions must update without stale bindings",
    );
    project.lights[0].casts_shadow = true;
    material_picture(&mut renderer, &project, &view, "area-shadow-allocation");
    assert_eq!(
        renderer.shadow_stats().layers,
        6,
        "area visibility uses six center-origin faces"
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn minified_normal_variance_broadens_specular_instead_of_becoming_polished() {
    use glam::Vec3;
    use poseforge_core::{Light, Texture, ToneMapping};
    let (mut project, view) = material_scene();
    project.render_settings.tone_mapping = ToneMapping::Linear;
    project.materials[0].base_color = [0.8, 0.8, 0.8, 1.];
    project.materials[0].metallic = 1.;
    project.materials[0].roughness = 0.05;
    project.lights = vec![Light {
        direction: Vec3::new(0.4, 0., -1.).normalize(),
        color: [1., 1., 1.],
        intensity: 0.25,
        casts_shadow: false,
        ..Light::default()
    }];
    for vertex in &mut project.meshes[0].vertices {
        vertex.uv[0] *= 256.;
        vertex.uv[1] *= 256.;
    }
    let mut pixels = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            // +/-45-degree normals average to +Z but have substantial unresolved variance.
            pixels.extend_from_slice(if (x + y) % 2 == 0 {
                &[218, 128, 218, 0]
            } else {
                &[37, 128, 218, 0]
            });
        }
    }
    project.textures.push(Texture {
        id: 50,
        name: "Normal variance".into(),
        width: 64,
        height: 64,
        rgba8: pixels.into(),
    });
    project.materials[0].normal_texture = Some(50);
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let filtered = material_picture(&mut renderer, &project, &view, "normal-variance-filtered");
    project.materials[0].normal_texture = None;
    project.materials[0].roughness = (0.05_f32.powi(4) + 0.25).powf(0.25);
    let rough_reference =
        material_picture(&mut renderer, &project, &view, "normal-variance-reference");
    let center_a = filtered.get_pixel(80, 80);
    let center_b = rough_reference.get_pixel(80, 80);
    assert!(
        (i32::from(center_a[0]) - i32::from(center_b[0])).abs() <= 3,
        "unresolved normal distribution should match broadened lobe: {center_a:?} vs {center_b:?}"
    );
    project.materials[0].roughness = 0.05;
    let polished = material_picture(&mut renderer, &project, &view, "normal-variance-polished");
    assert_ne!(
        filtered, polished,
        "minification must not erase authored rough microstructure"
    );
    project.materials[0].normal_texture = Some(50);
    project.materials[0].normal_scale = 0.;
    let disabled = material_picture(&mut renderer, &project, &view, "normal-variance-zero-scale");
    assert_images_equal(
        &polished,
        &disabled,
        "normal scale zero disables normal detail and variance",
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn dielectric_roughness_changes_diffuse_response_without_a_specular_interface() {
    use glam::Vec3;
    use poseforge_core::{Light, ToneMapping};
    let (mut project, view) = material_scene();
    project.render_settings.tone_mapping = ToneMapping::Linear;
    project.materials[0].ior = 1.;
    project.materials[0].roughness = 0.05;
    project.lights = vec![Light {
        direction: Vec3::NEG_Z,
        color: [1., 1., 1.],
        intensity: 1.,
        casts_shadow: false,
        ..Light::default()
    }];
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let smooth = material_picture(&mut renderer, &project, &view, "diffuse-smooth");
    project.materials[0].roughness = 1.;
    let rough = material_picture(&mut renderer, &project, &view, "diffuse-rough");
    let linear = |byte: u8| {
        let value = byte as f32 / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let ratio = linear(rough.get_pixel(80, 80)[0]) / linear(smooth.get_pixel(80, 80)[0]);
    assert!(
        (0.62..0.73).contains(&ratio),
        "normal-incidence diffuse normalization remains active without Fresnel specular: {ratio}"
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn glass_transmission_has_fresnel_coverage_and_thickness_absorption() {
    let (mut p, v) = material_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.materials[0].base_color = [1.; 4];
    p.materials[0].transmission = 1.;
    p.materials[0].ior = 1.5;
    let clear = material_picture(&mut r, &p, &v, "glass-clear-alpha");
    let a = clear.get_pixel(80, 80)[3];
    assert!((6..20).contains(&a), "Fresnel coverage {a}");
    p.materials[0].base_color = [0.2, 0.8, 1., 1.];
    p.materials[0].thickness = 0.1;
    let thin = material_picture(&mut r, &p, &v, "glass-thin-alpha");
    p.materials[0].thickness = 2.;
    let thick = material_picture(&mut r, &p, &v, "glass-thick-alpha");
    assert!(thick.get_pixel(80, 80)[3] > thin.get_pixel(80, 80)[3] + 30);
    assert_eq!(
        thick.get_pixel(0, 0)[3],
        0,
        "background remains transparent"
    );
}
#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn refractive_ior_moves_background_and_gradient_ignores_exposure() {
    use poseforge_core::BackgroundMode;
    let (mut p, v) = material_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    r.set_editor_appearance([0.; 4], false);
    p.render_settings.background_mode = BackgroundMode::Linear;
    p.render_settings.background_color = [0.01, 0.02, 0.7];
    p.render_settings.background_secondary = [0.9, 0.2, 0.01];
    p.render_settings.background_angle = 0.;
    p.materials[0].transmission = 1.;
    p.materials[0].base_color = [1.; 4];
    p.materials[0].roughness = 0.05;
    p.materials[0].ior = 1.;
    p.materials[0].thickness = 1.;
    p.nodes[0].transform.rotation = glam::Quat::from_rotation_y(0.55);
    let straight = viewport_picture(&mut r, &p, &v, "ior-one-gradient");
    p.materials[0].ior = 1.5;
    let refracted = viewport_picture(&mut r, &p, &v, "ior-glass-gradient");
    assert!(
        straight
            .pixels()
            .zip(refracted.pixels())
            .filter(|(a, b)| a != b)
            .count()
            > 300
    );
    assert_ne!(straight.get_pixel(10, 10), straight.get_pixel(150, 10));
    p.nodes.clear();
    let first = viewport_picture(&mut r, &p, &v, "gradient-only");
    p.render_settings.exposure = 8.;
    assert_eq!(first, viewport_picture(&mut r, &p, &v, "gradient-ev8"));
    p.render_settings.background_mode = BackgroundMode::Radial;
    let radial = viewport_picture(&mut r, &p, &v, "radial");
    assert_ne!(radial.get_pixel(80, 80), radial.get_pixel(10, 10));
}
#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn glass_casts_partial_shadow_between_clear_and_opaque() {
    let (mut p, v) = local_shadow_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.materials[1].base_color = [1.; 4];
    p.render_settings.shadow_strength = 1.;
    let deficit = |a: &image::RgbaImage, b: &image::RgbaImage| -> u64 {
        a.pixels()
            .zip(b.pixels())
            .map(|(a, b)| {
                b.0[..3]
                    .iter()
                    .zip(&a.0[..3])
                    .map(|(b, a)| u64::from(b.saturating_sub(*a)))
                    .sum::<u64>()
            })
            .sum()
    };
    p.render_settings.shadows = false;
    let off = material_picture(&mut r, &p, &v, "opaque-shadow-off");
    p.render_settings.shadows = true;
    let on = material_picture(&mut r, &p, &v, "opaque-shadow-on");
    let opaque = deficit(&on, &off);
    p.materials[1].transmission = 0.75;
    p.render_settings.shadows = false;
    let off = material_picture(&mut r, &p, &v, "glass-shadow-off");
    p.render_settings.shadows = true;
    let on = material_picture(&mut r, &p, &v, "glass-shadow-on");
    let partial = deficit(&on, &off);
    assert!(
        partial > 100 && partial < opaque,
        "partial {partial}, opaque {opaque}"
    );
}
#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn volumetric_light_adds_scattering_and_density_zero_preserves_clear_frame() {
    let mut p = Project {
        lights: vec![poseforge_core::Light::default()],
        ..Default::default()
    };
    let v = View {
        viewport: [0., 0., 160., 160.],
        ..View::default()
    };
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let clear = material_picture(&mut r, &p, &v, "volume-zero");
    assert!(clear.pixels().all(|p| p[3] == 0));
    p.render_settings.atmosphere.density = 0.08;
    for light in &mut p.lights { light.volume.enabled = true; }
    let fog = material_picture(&mut r, &p, &v, "volume-lit");
    assert!(fog.pixels().all(|p| p[3] > 0));
    assert!(rgb_energy(&fog) > 1000);
    p.lights.clear();
    let dark = material_picture(&mut r, &p, &v, "volume-unlit");
    assert!(rgb_energy(&dark) < rgb_energy(&fog));
    p.render_settings.atmosphere.density = 0.;
    assert_eq!(clear, material_picture(&mut r, &p, &v, "volume-reset"));
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn anisotropic_highlight_rotates_with_uv_frame_and_disabled_rotation_is_inert() {
    use poseforge_core::{Light, LightKind, Vertex};
    let (mut p, v) = material_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.render_settings.tone_mapping = poseforge_core::ToneMapping::Linear;
    p.render_settings.shadows = false;
    p.meshes[0].vertices = [
        ([-1., -1., 0.], [0., 0.]),
        ([1., -1., 0.], [1., 0.]),
        ([1., 1., 0.], [1., 1.]),
        ([-1., 1., 0.], [0., 1.]),
    ]
    .into_iter()
    .map(|(position, uv)| Vertex {
        position,
        uv,
        normal: [0., 0., 1.],
        ..Vertex::default()
    })
    .collect();
    p.meshes[0].indices = vec![0, 1, 2, 0, 2, 3].into();
    p.lights = vec![Light {
        kind: LightKind::Point,
        position: glam::Vec3::new(0., 0., 3.),
        intensity: 0.8,
        range: 0.,
        ..Light::default()
    }];
    p.materials[0].metallic = 1.;
    p.materials[0].roughness = 0.4;
    let iso = material_picture(&mut r, &p, &v, "aniso-disabled-zero");
    p.materials[0].anisotropy_rotation = 90.;
    assert_images_equal(
        &iso,
        &material_picture(&mut r, &p, &v, "aniso-disabled-90"),
        "disabled direction",
    );
    p.materials[0].anisotropy = 0.9;
    p.materials[0].anisotropy_rotation = 0.;
    let horizontal = material_picture(&mut r, &p, &v, "aniso-horizontal");
    p.materials[0].anisotropy_rotation = 90.;
    let vertical = material_picture(&mut r, &p, &v, "aniso-vertical");
    let moments = |im: &image::RgbaImage| -> (f64, f64) {
        im.enumerate_pixels().filter(|(_, _, p)| p[3] > 250).fold(
            (0., 0.),
            |(xx, yy), (x, y, p)| {
                let w = f64::from(p[0]).powi(3);
                (
                    xx + (x as f64 - 79.5).powi(2) * w,
                    yy + (y as f64 - 79.5).powi(2) * w,
                )
            },
        )
    };
    let (hx, hy) = moments(&horizontal);
    let (vx, vy) = moments(&vertical);
    assert!(
        hx > hy * 2. && vy > vx * 2.,
        "ellipse moments {hx}/{hy}, {vx}/{vy}"
    );
    let mirrored = p.clone();
    for vertex in &mut p.meshes[0].vertices {
        vertex.uv[0] = 1. - vertex.uv[0];
    }
    assert_images_equal(
        &vertical,
        &material_picture(&mut r, &p, &v, "aniso-mirrored"),
        "90-degree mirrored UV frame",
    );
    p = mirrored;
    for vertex in &mut p.meshes[0].vertices {
        vertex.uv = [0., 0.];
    }
    let missing_uv = material_picture(&mut r, &p, &v, "aniso-no-uv");
    assert_images_equal(&vertical, &missing_uv, "planar model-X fallback");
    p.nodes[0].transform.scale = glam::Vec3::new(0.75, 1.2, 0.5);
    let scaled = material_picture(&mut r, &p, &v, "aniso-scaled-no-uv");
    assert!(rgb_energy(&scaled) > 1000 && scaled.get_pixel(80, 80)[3] == 255);
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn fiber_sheen_changes_grazing_light_without_emission_or_metal_glass_leakage() {
    use poseforge_core::Light;
    let (mut p, mut v) = material_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.materials[0].base_color = [0.1, 0.02, 0.03, 1.];
    p.materials[0].sheen_roughness = 0.8;
    p.lights = vec![Light {
        direction: glam::Vec3::new(-0.95, 0., -0.312).normalize(),
        intensity: 2.,
        ..Light::default()
    }];
    v.eye = glam::Vec3::new(3., 0., 1.);
    let plain = material_picture(&mut r, &p, &v, "fiber-plain");
    p.materials[0].sheen_weight = 1.;
    let fiber = material_picture(&mut r, &p, &v, "fiber-grazing");
    assert!(
        rgb_energy(&fiber) > rgb_energy(&plain) * 11 / 10,
        "sheen must reach direct light"
    );
    p.lights.clear();
    assert_eq!(
        rgb_energy(&material_picture(&mut r, &p, &v, "fiber-unlit")),
        0,
        "fiber is not emission"
    );
    p.render_settings.ambient_intensity = 1.;
    for glass in [false, true] {
        p.materials[0].metallic = if glass { 0. } else { 1. };
        p.materials[0].transmission = if glass { 1. } else { 0. };
        p.materials[0].sheen_weight = 0.;
        let off = material_picture(&mut r, &p, &v, &format!("fiber-off-{glass}"));
        p.materials[0].sheen_weight = 1.;
        assert_images_equal(
            &off,
            &material_picture(&mut r, &p, &v, &format!("fiber-suppressed-{glass}")),
            "fiber only on opaque dielectric substrate",
        );
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn white_fiber_environment_keeps_energy_at_normal_and_grazing_angles() {
    let (mut p, mut v) = material_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.render_settings.tone_mapping = poseforge_core::ToneMapping::Linear;
    p.render_settings.ambient_intensity = 0.25;
    p.render_settings.sky_color = [1.; 3];
    p.render_settings.ground_color = [1.; 3];
    p.materials[0].base_color = [1.; 4];
    p.materials[0].sheen_weight = 1.;
    // A quarter-intensity furnace leaves headroom so clipping cannot conceal extra energy.
    for (angle, eye) in [glam::Vec3::new(0., 0., 3.), glam::Vec3::new(3., 0., 0.3)]
        .into_iter()
        .enumerate()
    {
        v.eye = eye;
        for (index, rough) in [0.07, 0.3, 0.8, 1.].into_iter().enumerate() {
            p.materials[0].sheen_roughness = rough;
            let im = material_picture(&mut r, &p, &v, &format!("fiber-furnace-{angle}-{index}"));
            let center = im.get_pixel(80, 80);
            assert!(
                center[3] == 255 && center.0[..3].iter().all(|v| (135..=139).contains(v)),
                "furnace {center:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn independent_absorption_changes_transmission_and_shadow_without_recoloring_surface() {
    let (mut p, v) = material_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    r.set_editor_appearance([1.; 4], false);
    p.render_settings.background_mode = poseforge_core::BackgroundMode::Solid;
    p.render_settings.background_color = [1.; 3];
    p.materials[0].base_color = [1.; 4];
    p.materials[0].transmission = 1.;
    p.materials[0].thickness = 1.;
    p.materials[0].attenuation_distance = 1.;
    p.materials[0].attenuation_color = Some([0.04, 0.7, 1.]);
    let blue = viewport_picture(&mut r, &p, &v, "absorption-blue");
    p.materials[0].attenuation_color = Some([1., 0.3, 0.02]);
    let red = viewport_picture(&mut r, &p, &v, "absorption-red");
    let b = blue.get_pixel(80, 80);
    let a = red.get_pixel(80, 80);
    assert!(
        b[2] > b[0] + 40 && a[0] > a[2] + 40,
        "transmitted tint {b:?} {a:?}"
    );
    p.materials[0].transmission = 0.;
    let opaque = material_picture(&mut r, &p, &v, "absorption-opaque-override");
    p.materials[0].attenuation_color = None;
    assert_images_equal(
        &opaque,
        &material_picture(&mut r, &p, &v, "absorption-opaque-inherit"),
        "opaque surface ignores absorption",
    );
    p.materials[0].transmission = 1.;
    p.materials[0].base_color = [0.4, 0.7, 0.9, 1.];
    let inherit = material_picture(&mut r, &p, &v, "absorption-inherit");
    p.materials[0].attenuation_color = Some([0.4, 0.7, 0.9]);
    assert_images_equal(
        &inherit,
        &material_picture(&mut r, &p, &v, "absorption-same-override"),
        "legacy fallback",
    );
    let (mut p, v) = local_shadow_scene();
    p.materials[1].base_color = [1.; 4];
    p.materials[1].transmission = 1.;
    p.materials[1].thickness = 1.;
    p.materials[1].attenuation_color = Some([1.; 3]);
    let clear = material_picture(&mut r, &p, &v, "absorption-clear-shadow");
    p.materials[1].attenuation_color = Some([0.02; 3]);
    let dark = material_picture(&mut r, &p, &v, "absorption-dark-shadow");
    // Only count the receiver's outer half: the glass object's own tint is irrelevant.
    let energy = |im: &image::RgbaImage| -> u64 {
        im.enumerate_pixels()
            .filter(|(x, y, _)| *x < 60 && *y > 65)
            .map(|(_, _, p)| p.0[..3].iter().map(|v| *v as u64).sum::<u64>())
            .sum()
    };
    assert!(
        energy(&clear) > energy(&dark) + 100,
        "absorption reaches transparent shadow coverage"
    );
}

fn rough_refraction_scene() -> (Project, View) {
    use poseforge_core::{Material, Mesh, Node, Texture, Vertex};
    let (mut p, v) = material_scene();
    p.render_settings.antialiasing = 1;
    p.render_settings.shadows = false;
    p.render_settings.tone_mapping = poseforge_core::ToneMapping::Linear;
    p.materials[0].base_color = [1.; 4];
    p.materials[0].transmission = 1.;
    p.materials[0].roughness = 0.8;
    p.materials[0].thickness = 1.;
    let quad = |id, material, extent: f32, z: f32| Mesh {
        id,
        name: "Refraction plane".into(),
        material: Some(material),
        vertices: [
            ([-extent, -extent, z], [0., 0.]),
            ([extent, -extent, z], [1., 0.]),
            ([extent, extent, z], [1., 1.]),
            ([-extent, extent, z], [0., 1.]),
        ]
        .into_iter()
        .map(|(position, uv)| Vertex {
            position,
            uv,
            normal: [0., 0., 1.],
            ..Vertex::default()
        })
        .collect(),
        indices: vec![0, 1, 2, 0, 2, 3].into(),
        topology: None,
        morph_targets: vec![],
    };
    p.meshes[0] = quad(2, 3, 0.98, 0.);
    p.materials.push(Material {
        id: 20,
        base_color: [0., 0., 0., 1.],
        emissive: [1.; 3],
        emissive_texture: Some(25),
        ..Material::default()
    });
    p.meshes.push(quad(21, 20, 2., -1.));
    let mut background = Node::new(22, "Checker wall");
    background.mesh = Some(21);
    p.nodes.push(background);
    let mut rgba8 = vec![];
    for y in 0..64 {
        for x in 0..64 {
            let c = if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 };
            rgba8.extend([c, c, c, 255]);
        }
    }
    p.textures.push(Texture {
        id: 25,
        name: "Refraction checker".into(),
        width: 64,
        height: 64,
        rgba8: rgba8.into(),
    });
    (p, v)
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn rough_glass_rejects_foreground_color_in_every_filter_tap() {
    use poseforge_core::{Material, Node};
    let (mut p, v) = rough_refraction_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.materials.push(Material {
        id: 30,
        base_color: [0., 0., 0., 1.],
        emissive: [1., 0., 0.],
        ..Material::default()
    });
    let mut foreground = p.meshes[0].clone();
    foreground.id = 31;
    foreground.material = Some(30);
    for vertex in &mut foreground.vertices {
        vertex.position[0] *= 0.18;
        vertex.position[2] = 0.5;
    }
    p.meshes.push(foreground);
    let mut node = Node::new(32, "Opaque foreground");
    node.mesh = Some(31);
    p.nodes.push(node);
    for aa in [1, 4] {
        p.render_settings.antialiasing = aa;
        // Non-axis-aligned subpixel edges also exercise the MSAA color/depth boundary.
        p.nodes[2].transform.translation.x = if aa == 4 { 0.073 } else { 0. };
        p.nodes[2].transform.rotation =
            glam::Quat::from_rotation_z(if aa == 4 { 0.19 } else { 0. });
        p.materials[2].emissive = [1., 0., 0.];
        p.nodes[0].visible = false;
        let reference =
            material_picture(&mut r, &p, &v, &format!("refraction-foreground-mask-{aa}"));
        p.nodes[0].visible = true;
        let red = material_picture(&mut r, &p, &v, &format!("refraction-foreground-red-{aa}"));
        p.materials[2].emissive = [0., 1., 0.];
        let green = material_picture(&mut r, &p, &v, &format!("refraction-foreground-green-{aa}"));
        let mut maximum = 0;
        let mut checked = 0;
        for y in 25..135 {
            for x in 25..135 {
                let near_front = (y - 2..=y + 2).any(|yy| {
                    (x - 2..=x + 2).any(|xx| {
                        let p = reference.get_pixel(xx, yy);
                        p[0] > p[1].saturating_add(10)
                    })
                });
                if near_front {
                    continue;
                }
                checked += 1;
                for c in 0..3 {
                    maximum =
                        maximum.max(red.get_pixel(x, y)[c].abs_diff(green.get_pixel(x, y)[c]));
                }
            }
        }
        assert!(checked > 5000);
        assert!(
            maximum <= 1,
            "foreground color leaked through rough glass: {maximum}, MSAA {aa}"
        );
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn transmission_blur_follows_thickness_ior_and_image_scale() {
    let (mut p, v) = rough_refraction_scene();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let variance = |im: &image::RgbaImage| -> f64 {
        let values: Vec<_> = (45..115)
            .flat_map(|y| (45..115).map(move |x| f64::from(im.get_pixel(x, y)[0])))
            .collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
    };
    p.materials[0].thickness = 0.08;
    let thin = material_picture(&mut r, &p, &v, "refraction-thin-rough");
    p.materials[0].thickness = 1.;
    let thick = material_picture(&mut r, &p, &v, "refraction-thick-rough");
    assert!(
        variance(&thick) < variance(&thin) * 0.65,
        "thickness blur: thin {}, thick {}",
        variance(&thin),
        variance(&thick)
    );
    let directory = std::path::PathBuf::from(
        std::env::var_os("POSEFORGE_RENDER_QA_OUTPUT")
            .unwrap_or_else(|| std::env::temp_dir().into_os_string()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("refraction-scale-320.png");
    let mut doubled = v.clone();
    doubled.viewport = [0., 0., 320., 320.];
    r.export_png(
        &p,
        &p.global_transforms().unwrap(),
        &doubled,
        &[],
        &file,
        320,
        320,
        true,
    )
    .unwrap();
    let large = image::open(file).unwrap().into_rgba8();
    let down = image::imageops::resize(&large, 160, 160, image::imageops::FilterType::Triangle);
    let ratio = variance(&down) / variance(&thick);
    assert!(
        (0.7..1.3).contains(&ratio),
        "resolution-invariant blur variance ratio {ratio}"
    );
    p.materials[0].ior = 1.;
    let matched = material_picture(&mut r, &p, &v, "refraction-ior-one-rough");
    p.materials[0].roughness = 0.05;
    assert_images_equal(
        &matched,
        &material_picture(&mut r, &p, &v, "refraction-ior-one-smooth"),
        "index matched material must not bend or blur light",
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn frustum_culling_reduces_draws_without_changing_pixels_or_leaking_unused_glass_passes() {
    use poseforge_core::{AlphaMode, Material, Node, Vec3};
    let (mut project, view) = material_scene();
    project.render_settings.shadows = false;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let baseline = material_picture(&mut renderer, &project, &view, "cull-baseline");
    let base = renderer.frame_stats();
    assert_eq!(base.scene_draws, 1);
    assert!(!base.depth_prepass);
    project.materials.push(Material {
        id: 900,
        transmission: 1.,
        alpha_mode: AlphaMode::Blend,
        ..Material::default()
    });
    for index in 0..32 {
        let mut n = Node::new(1000 + index, "Outside");
        n.mesh = Some(2);
        n.transform.translation = Vec3::new(100. + index as f32 * 2., 0., 0.);
        project.nodes.push(n);
    }
    let actual = material_picture(&mut renderer, &project, &view, "cull-outside");
    assert_eq!(baseline, actual);
    let stats = renderer.frame_stats();
    assert_eq!(stats.candidates, 33);
    assert_eq!(stats.culled, 32);
    assert_eq!(stats.scene_draws, 1);
    assert!(!stats.depth_prepass && !stats.refraction_copy);
    project.nodes.last_mut().unwrap().transform.translation = Vec3::new(0.1, 0., 0.);
    let _ = material_picture(&mut renderer, &project, &view, "cull-enter");
    assert_eq!(renderer.frame_stats().scene_draws, 2);
    project.render_settings.atmosphere.density = 0.03;
    for light in &mut project.lights { light.volume.enabled = true; }
    let _ = material_picture(&mut renderer, &project, &view, "cull-fog");
    assert!(renderer.frame_stats().depth_prepass);
    assert_eq!(renderer.frame_stats().depth_draws, 2);
}
#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn shadow_face_culling_keeps_real_shadows_and_skips_unrelated_cubemap_faces() {
    let (project, view) = local_shadow_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let shadow = material_picture(&mut renderer, &project, &view, "face-cull-shadow");
    let stats = renderer.frame_stats();
    assert!(stats.shadow_draws > 0);
    assert!(stats.shadow_draws < project.nodes.iter().filter(|n| n.mesh.is_some()).count() * 6);
    let mut lit = project.clone();
    lit.render_settings.shadows = false;
    let picture = material_picture(&mut renderer, &lit, &view, "face-cull-lit");
    assert!(darkened_pixels(&shadow, &picture) > 20);
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn contact_obscurance_darkens_nearby_ambient_only_and_preserves_alpha() {
    let (mut p, mut view) = local_shadow_scene();
    p.lights.clear();
    p.render_settings.ambient_intensity = 1.;
    p.render_settings.sky_color = [0.6; 3];
    p.render_settings.ground_color = [0.6; 3];
    p.render_settings.ambient_occlusion_radius = 0.65;
    p.nodes[1].transform.translation.y = 0.75;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    for ortho in [false, true] {
        view.orthographic = ortho.then_some(2.7);
        p.render_settings.ambient_occlusion = 0.;
        let off = material_picture(&mut renderer, &p, &view, &format!("contact-off-{ortho}"));
        assert!(!renderer.frame_stats().depth_prepass);
        p.render_settings.ambient_occlusion = 1.5;
        let on = material_picture(&mut renderer, &p, &view, &format!("contact-on-{ortho}"));
        assert!(renderer.frame_stats().depth_prepass && !renderer.frame_stats().refraction_copy);
        assert!(
            darkened_pixels(&on, &off) > 25,
            "contact needs visible ambient occlusion"
        );
        assert!(on.pixels().zip(off.pixels()).all(|(a, b)| a[3] == b[3]));
        p.render_settings.ambient_occlusion = 0.;
        assert_eq!(
            off,
            material_picture(&mut renderer, &p, &view, &format!("contact-reset-{ortho}"))
        );
    }
    let (mut direct, view) = local_shadow_scene();
    direct.render_settings.ambient_intensity = 0.;
    direct.materials[0].emissive = [0.3, 0.1, 0.];
    let off = material_picture(&mut renderer, &direct, &view, "contact-direct-off");
    direct.render_settings.ambient_occlusion = 2.;
    assert_eq!(
        off,
        material_picture(&mut renderer, &direct, &view, "contact-direct-on"),
        "direct light and emission must not be darkened"
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn mapped_uv_matches_baked_uv_in_color_cutout_and_shadow_passes() {
    use poseforge_core::{AlphaMode, Texture};
    let (mut p, view) = local_shadow_scene();
    let mut pixels = Vec::new();
    for y in 0..32 {
        for x in 0..32 {
            pixels.extend_from_slice(&[
                if x < 16 { 230 } else { 25 },
                if y < 16 { 180 } else { 40 },
                30,
                if (x / 8 + y / 8) % 2 == 0 { 255 } else { 0 },
            ]);
        }
    }
    p.textures.push(Texture {
        id: 90,
        name: "UV test".into(),
        width: 32,
        height: 32,
        rgba8: pixels.into(),
    });
    p.materials[1].base_color_texture = Some(90);
    p.materials[1].alpha_mode = AlphaMode::Mask;
    p.materials[1].uv_scale = [1.7, 0.8];
    p.materials[1].uv_offset = [0.15, -0.2];
    p.materials[1].uv_rotation = 37.;
    p.materials[1].texture_uvs[0] = poseforge_core::TextureUvTransform {
        offset: [0.1, 0.8],
        rotation: -0.4,
        scale: [0.7, -1.3],
    };
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let mapped = material_picture(&mut renderer, &p, &view, "uv-mapped");
    let mut baked = p.clone();
    let m = &p.materials[1];
    let (sin, cos) = m.uv_rotation.to_radians().sin_cos();
    for vertex in &mut baked.meshes[1].vertices {
        let u = vertex.uv[0] * m.uv_scale[0];
        let v = vertex.uv[1] * m.uv_scale[1];
        vertex.uv = m.texture_uvs[0].transform([
            u * cos - v * sin + m.uv_offset[0],
            u * sin + v * cos + m.uv_offset[1],
        ]);
    }
    baked.materials[1].uv_scale = [1.; 2];
    baked.materials[1].uv_offset = [0.; 2];
    baked.materials[1].uv_rotation = 0.;
    baked.materials[1].texture_uvs = Default::default();
    // A separate renderer loads the edited immutable mesh resource.
    let mut other = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let baked = material_picture(&mut other, &baked, &view, "uv-baked");
    let severe = mapped
        .pixels()
        .zip(baked.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 2))
        .count();
    assert!(
        severe < 12,
        "shader and authored UV transform differ at {severe} pixels"
    );
    p.materials[1].uv_scale = [1.; 2];
    p.materials[1].uv_offset = [0.; 2];
    p.materials[1].uv_rotation = 0.;
    p.materials[1].texture_uvs = Default::default();
    assert_ne!(
        mapped,
        material_picture(&mut renderer, &p, &view, "uv-reset")
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn area_roll_and_temperature_change_actual_illumination_and_reset() {
    use poseforge_core::{Light, LightKind, Vec3};
    let (mut p, view) = material_scene();
    p.materials[0].roughness = 0.16;
    p.lights = vec![Light {
        kind: LightKind::Area,
        position: Vec3::new(0.2, 0.4, 2.),
        direction: Vec3::NEG_Z,
        area_size: [2., 0.2],
        intensity: 18.,
        color: [1.; 3],
        ..Light::default()
    }];
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let horizontal = material_picture(&mut renderer, &p, &view, "light-roll-0");
    p.lights[0].area_rotation = 90.;
    let vertical = material_picture(&mut renderer, &p, &view, "light-roll-90");
    assert!(
        horizontal
            .pixels()
            .zip(vertical.pixels())
            .filter(|(a, b)| a != b)
            .count()
            > 200
    );
    p.lights[0].use_temperature = true;
    p.lights[0].temperature_kelvin = 2500.;
    let warm = material_picture(&mut renderer, &p, &view, "light-warm");
    p.lights[0].temperature_kelvin = 10000.;
    let cool = material_picture(&mut renderer, &p, &view, "light-cool");
    let balance = |image: &image::RgbaImage| {
        image
            .pixels()
            .map(|v| i64::from(v[0]) - i64::from(v[2]))
            .sum::<i64>()
    };
    assert!(balance(&warm) > balance(&cool) + 5000);
    p.lights[0].use_temperature = false;
    assert_eq!(
        vertical,
        material_picture(&mut renderer, &p, &view, "light-temperature-off")
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn instanced_ui_preserves_alpha_order_and_reuses_label_and_vertex_storage() {
    use poseforge_render::{UiRect, UiText};
    let directory = std::env::var_os("POSEFORGE_RENDER_QA_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("ui-instancing16.png");
    let p = Project::default();
    let mut renderer = pollster::block_on(Renderer::headless(160, 128)).unwrap();
    renderer.set_editor_appearance([0., 0., 0., 1.], false);
    let view = View {
        viewport: [0., 0., 160., 128.],
        ..View::default()
    };
    let rects = [
        UiRect {
            triangle: None,
            segment: None,
            depth: None,
            rect: [10., 10., 70., 60.],
            color: [1., 0., 0., 1.],
        },
        UiRect {
            triangle: None,
            segment: None,
            depth: None,
            rect: [30., 30., 70., 50.],
            color: [0., 1., 0., 0.5],
        },
    ];
    let mut texts = vec![UiText {
        weight: 400,
        x: 10.,
        y: 90.,
        text: "Материал".into(),
        size: 14.,
        color: [1.; 4],
    }];
    let capture = |r: &mut Renderer, texts: &[UiText]| {
        r.capture_viewport_png(
            &p,
            &Default::default(),
            &view,
            &rects,
            texts,
            &[],
            &path,
            160,
            128,
        )
        .unwrap();
        image::open(&path).unwrap().into_rgba8()
    };
    let first = capture(&mut renderer, &texts);
    let first_stats = renderer.frame_stats();
    assert_eq!(first_stats.ui_text_layouts, 1);
    assert_eq!(first_stats.ui_buffer_allocations, 1);
    assert_eq!(first_stats.ui_upload_bytes, 160);
    assert_eq!(first_stats.ui_rectangles, 2);
    let red = first.get_pixel(15, 15);
    assert!(red[0] > 250 && red[1] < 3 && red[2] < 3);
    let overlap = first.get_pixel(40, 40);
    assert!(
        (185..=190).contains(&overlap[0]) && (185..=190).contains(&overlap[1]) && overlap[2] < 3
    );
    let second = capture(&mut renderer, &texts);
    assert_eq!(first, second);
    assert_eq!(renderer.frame_stats().ui_text_layouts, 0);
    assert_eq!(renderer.frame_stats().ui_buffer_allocations, 0);
    texts[0].text = "Стекло".into();
    let changed = capture(&mut renderer, &texts);
    assert_ne!(second, changed);
    assert_eq!(renderer.frame_stats().ui_text_layouts, 1);
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn workspace17_independent_node_images_opacity_geometric_glass_and_camera_focus() {
    use glam::Vec3;
    use poseforge_core::*;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let (mut p, mut view) = material_scene();
    p.textures.extend([
        Texture {
            id: 100,
            name: "red node".into(),
            width: 1,
            height: 1,
            rgba8: vec![240, 40, 5, 255].into(),
        },
        Texture {
            id: 101,
            name: "blue node".into(),
            width: 1,
            height: 1,
            rgba8: vec![5, 40, 240, 255].into(),
        },
    ]);
    let mut image = MaterialNode::new(1, MaterialNodeKind::Texture, [0., 0.]);
    image.texture = Some(100);
    let mut opacity = MaterialNode::new(2, MaterialNodeKind::Value, [0., 100.]);
    opacity.value[0] = 0.2;
    let mut graph = MaterialGraph {
        nodes: vec![image, opacity],
        ..Default::default()
    };
    graph.outputs[3] = Some(1);
    p.materials[0].graph = Some(graph);
    let red = material_picture(&mut renderer, &p, &view, "17-node-red");
    p.materials[0].graph.as_mut().unwrap().nodes[0].texture = Some(101);
    let blue = material_picture(&mut renderer, &p, &view, "17-node-blue");
    assert!(red.get_pixel(80, 80)[0] > red.get_pixel(80, 80)[2] + 80);
    assert!(blue.get_pixel(80, 80)[2] > blue.get_pixel(80, 80)[0] + 80);
    p.materials[0].graph.as_mut().unwrap().outputs[4] = Some(2);
    let alpha = material_picture(&mut renderer, &p, &view, "17-node-opacity");
    assert!((49..=53).contains(&alpha.get_pixel(80, 80)[3]));
    p.materials[0].graph = None;
    p.materials[0].base_color = [0.05, 1., 1., 1.];
    p.materials[0].transmission = 1.;
    p.materials[0].ior = 1.;
    p.materials[0].thickness = 0.001;
    p.materials[0].geometric_thickness = true;
    p.meshes[0] = box_mesh(2, "Closed glass", Vec3::new(1., 1., 0.1), Some(3));
    let thin = material_picture(&mut renderer, &p, &view, "17-glass-thin-geometry");
    for v in &mut p.meshes[0].vertices {
        v.position[2] *= 8.;
    }
    let thick = material_picture(&mut renderer, &p, &view, "17-glass-thick-geometry");
    assert!(
        thick.get_pixel(80, 80)[3] > thin.get_pixel(80, 80)[3] + 20,
        "actual volume must determine absorption: {:?} {:?}",
        thin.get_pixel(80, 80),
        thick.get_pixel(80, 80)
    );
    p.materials[0].geometric_thickness = false;
    let sheet = material_picture(&mut renderer, &p, &view, "17-glass-authored-thickness");
    assert!(sheet.get_pixel(80, 80)[3] < thin.get_pixel(80, 80)[3]);
    // Resolve a high contrast wall at the focus plane, then deliberately defocus it.
    let (mut p, _) = rough_refraction_scene();
    p.nodes.remove(0);
    p.render_settings.antialiasing = 1;
    view.depth_of_field = DepthOfField {
        enabled: true,
        focus_distance: 4.,
        f_stop: 0.7,
    };
    let focused = material_picture(&mut renderer, &p, &view, "17-dof-focus");
    view.depth_of_field.enabled = false;
    let pinhole = material_picture(&mut renderer, &p, &view, "17-dof-disabled");
    assert_images_equal(&focused, &pinhole, "the chosen plane remains sharp");
    view.depth_of_field = DepthOfField {
        enabled: true,
        focus_distance: 0.15,
        f_stop: 0.7,
    };
    let blurred = material_picture(&mut renderer, &p, &view, "17-dof-blurred");
    let contrast = |im: &image::RgbaImage| -> u64 {
        (35..125)
            .flat_map(|y| (35..124).map(move |x| (x, y)))
            .map(|(x, y)| im.get_pixel(x, y)[0].abs_diff(im.get_pixel(x + 1, y)[0]) as u64)
            .sum()
    };
    assert!(
        contrast(&blurred) < contrast(&pinhole) * 3 / 4,
        "defocus must reduce wall contrast"
    );
    view.orthographic = Some(1.5);
    let ortho = material_picture(&mut renderer, &p, &view, "17-dof-ortho");
    view.depth_of_field.enabled = false;
    assert_images_equal(
        &ortho,
        &material_picture(&mut renderer, &p, &view, "17-dof-ortho-disabled"),
        "orthographic camera has no lens blur",
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn workspace17_ramp_uv_bump_normal_and_collection_visibility() {
    use poseforge_core::*;
    let (mut p, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let uv = MaterialNode::new(1, MaterialNodeKind::Coordinates, [0., 0.]);
    let mut noise = MaterialNode::new(2, MaterialNodeKind::Noise, [0., 0.]);
    noise.inputs[0] = Some(1);
    noise.value[0] = 5.;
    let mut ramp = MaterialNode::new(3, MaterialNodeKind::ColorRamp, [0., 0.]);
    ramp.inputs[0] = Some(2);
    ramp.value = [1., 0., 0., 1.];
    ramp.secondary = [0., 0., 1., 1.];
    let mut bump = MaterialNode::new(4, MaterialNodeKind::Bump, [0., 0.]);
    bump.inputs[0] = Some(2);
    bump.value[0] = 0.3;
    let mut normal = MaterialNode::new(5, MaterialNodeKind::NormalMap, [0., 0.]);
    normal.inputs[0] = Some(6);
    let mut color = MaterialNode::new(6, MaterialNodeKind::Color, [0., 0.]);
    color.value = [0.9, 0.5, 0.8, 1.];
    let mut graph = MaterialGraph {
        nodes: vec![uv, noise, ramp, bump, normal, color],
        ..Default::default()
    };
    graph.outputs[3] = Some(3);
    p.materials[0].graph = Some(graph);
    let first = material_picture(&mut renderer, &p, &view, "17-ramp-uv");
    assert!(first.pixels().any(|p| p[0] as u16 > p[2] as u16 + 30));
    assert!(first.pixels().any(|p| p[2] as u16 > p[0] as u16 + 30));
    p.materials[0].graph.as_mut().unwrap().nodes[0].value[2] = 0.37;
    let shifted = material_picture(&mut renderer, &p, &view, "17-ramp-shifted");
    assert_ne!(first, shifted);
    p.materials[0].graph.as_mut().unwrap().outputs[3] = None;
    p.lights.push(Light {
        direction: Vec3::new(-0.5, -0.4, -1.).normalize(),
        intensity: 3.,
        casts_shadow: false,
        ..Default::default()
    });
    let flat = material_picture(&mut renderer, &p, &view, "17-flat");
    p.materials[0].graph.as_mut().unwrap().outputs[7] = Some(4);
    let bumped = material_picture(&mut renderer, &p, &view, "17-bump");
    assert_ne!(flat, bumped);
    p.materials[0].graph.as_mut().unwrap().outputs[7] = Some(5);
    let mapped = material_picture(&mut renderer, &p, &view, "17-normal-map");
    assert_ne!(flat, mapped);
    assert_ne!(bumped, mapped);
    p.collections.push(Collection {
        id: 100,
        name: "Hidden render".into(),
        members: vec![p.nodes[0].id],
        render_visible: false,
        ..Default::default()
    });
    let hidden = material_picture(&mut renderer, &p, &view, "17-hidden-collection");
    assert_eq!(rgb_energy(&hidden), 0);
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn npr22_contours_change_opaque_silhouette_without_changing_alpha_or_flat_interior() {
    let (mut p, mut view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.materials[0].emissive = [0.6; 3];
    p.render_settings.shadows = false;
    for ortho in [false, true] {
        view.orthographic = ortho.then_some(1.5);
        p.render_settings.npr_contours.width = 0.;
        let plain = material_picture(&mut renderer, &p, &view, "22-contour-off");
        p.render_settings.npr_contours.width = 3.;
        let ink = material_picture(&mut renderer, &p, &view, "22-contour-on");
        assert!(rgb_energy(&ink) < rgb_energy(&plain));
        assert_eq!(ink.get_pixel(80, 80), plain.get_pixel(80, 80));
        assert!(plain.pixels().zip(ink.pixels()).all(|(a, b)| a[3] == b[3]));
    }
}
#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn npr22_palette_hatching_and_pbr_restoration_reach_gpu() {
    use poseforge_core::Light;
    let (mut p, view) = material_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    p.lights.push(Light {
        direction: [0., 0., -1.].into(),
        intensity: 0.4,
        casts_shadow: false,
        ..Default::default()
    });
    let pbr = material_picture(&mut renderer, &p, &view, "22-pbr");
    p.materials[0].npr.enabled = true;
    p.materials[0].npr.specular_strength = 0.;
    p.materials[0].npr.rim_strength = 0.;
    p.materials[0].npr.threshold = 0.7;
    let plain = material_picture(&mut renderer, &p, &view, "22-npr");
    p.materials[0].npr.palette_strength = 1.;
    p.materials[0].npr.palette_mid = [0.1, 0.8, 0.2];
    let palette = material_picture(&mut renderer, &p, &view, "22-palette");
    assert_ne!(plain, palette);
    p.materials[0].npr.hatch_strength = 1.;
    p.materials[0].npr.hatch_cross = 1.;
    p.materials[0].npr.hatch_density = 8.;
    let hatch = material_picture(&mut renderer, &p, &view, "22-hatching");
    assert!(rgb_energy(&hatch) < rgb_energy(&palette));
    p.materials[0].npr.enabled = false;
    assert_images_equal(
        &pbr,
        &material_picture(&mut renderer, &p, &view, "22-pbr-restored"),
        "NPR parameters leave PBR unchanged",
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn edit_components_hide_behind_surfaces_and_xray_restores_them() {
    use glam::Vec3;
    use poseforge_render::UiRect;
    let (mut project, mut view) = material_scene();
    project.render_settings.shadows = false;
    let globals = project.global_transforms().unwrap();
    let mut renderer = pollster::block_on(Renderer::headless(320, 320)).unwrap();
    renderer.set_editor_appearance([0., 0., 0., 1.], false);
    let path = std::env::temp_dir().join("artforge-edit-depth226.png");
    for samples in [1, 4] {
        project.render_settings.antialiasing = samples;
        for ortho in [None, Some(1.25)] {
            view.orthographic = ortho;
            for scale in [1., 2.] {
                let size = (160. * scale) as u32;
                view.viewport = [0., 0., size as f32, size as f32];
                let depth = |z| {
                    let clip = view.view_projection() * Vec3::new(0., 0., z).extend(1.);
                    clip.z / clip.w
                };
                let front = depth(0.1);
                let back = depth(-0.2);
                let mut rects = vec![
                    UiRect {
                        triangle: None,
                        rect: [77., 77., 6., 6.].map(|v| v * scale),
                        color: [0., 1., 0., 1.],
                        segment: None,
                        depth: Some([front; 2]),
                    },
                    UiRect {
                        triangle: None,
                        rect: [77., 77., 6., 6.].map(|v| v * scale),
                        color: [1., 0., 0., 1.],
                        segment: None,
                        depth: Some([back; 2]),
                    },
                    UiRect {
                        triangle: None,
                        rect: [19., 78., 122., 4.].map(|v| v * scale),
                        color: [0., 0., 1., 1.],
                        segment: Some([20., 80., 140., 80., 2.].map(|v| v * scale)),
                        depth: Some([back; 2]),
                    },
                ];
                let capture = |renderer: &mut Renderer, rects: &[UiRect]| {
                    renderer
                        .capture_viewport_png(
                            &project,
                            &globals,
                            &view,
                            rects,
                            &[],
                            &[],
                            &path,
                            size,
                            size,
                        )
                        .unwrap();
                    image::open(&path).unwrap().into_rgba8()
                };
                let solid = capture(&mut renderer, &rects);
                assert!(renderer.frame_stats().depth_prepass);
                let center = solid.get_pixel((80. * scale) as u32, (80. * scale) as u32);
                assert!(
                    center[1] > 250 && center[0] < 3 && center[2] < 3,
                    "{center:?}"
                );
                let outside = solid.get_pixel((24. * scale) as u32, (80. * scale) as u32);
                assert!(
                    outside[2] > 250,
                    "unoccluded edge must remain visible: {outside:?}"
                );
                for rect in &mut rects {
                    rect.depth = None;
                }
                let xray = capture(&mut renderer, &rects);
                let center = xray.get_pixel((80. * scale) as u32, (80. * scale) as u32);
                assert!(
                    center[2] > 250 && center[1] < 3,
                    "X-Ray must reveal the rear edge: {center:?}"
                );
            }
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn selected_face_fill_blends_only_triangle_interior_and_respects_scene_depth() {
    use poseforge_render::UiRect;
    let (mut project, mut view) = material_scene();
    project.materials[0].emissive = [0.2, 0.2, 0.2];
    project.render_settings.shadows = false;
    let globals = project.global_transforms().unwrap();
    let mut renderer = pollster::block_on(Renderer::headless(320, 320)).unwrap();
    let path = std::env::temp_dir().join("artforge-face-fill26.png");
    for scale in [1., 2.] {
        let size = (160. * scale) as u32;
        view.viewport = [0., 0., size as f32, size as f32];
        let depth = |z| {
            let p = view.view_projection() * glam::Vec3::new(0., 0., z).extend(1.);
            p.z / p.w
        };
        let triangle = |z: f32, test: bool| UiRect {
            rect: [32., 32., 96., 96.].map(|v| v * scale),
            color: [1., 0., 0., 0.25],
            segment: None,
            depth: test.then_some([depth(z); 2]),
            triangle: Some(
                [[32., 32.], [128., 32.], [80., 128.]]
                    .map(|[x, y]| [x * scale, y * scale, depth(z)]),
            ),
        };
        let capture = |renderer: &mut Renderer, rects: &[UiRect]| {
            renderer
                .capture_viewport_png(
                    &project,
                    &globals,
                    &view,
                    rects,
                    &[],
                    &[],
                    &path,
                    size,
                    size,
                )
                .unwrap();
            image::open(&path).unwrap().into_rgba8()
        };
        let original = capture(&mut renderer, &[]);
        let hidden = capture(&mut renderer, &[triangle(-0.2, true)]);
        let front = capture(&mut renderer, &[triangle(0.1, true)]);
        let xray = capture(&mut renderer, &[triangle(-0.2, false)]);
        let center = (80. * scale) as u32;
        let base = original.get_pixel(center, center);
        let fill = front.get_pixel(center, center);
        assert_eq!(hidden.get_pixel(center, center), base);
        assert!(
            fill[0] > base[0] && fill[0] < 250 && fill[1] < base[1],
            "translucent fill: {fill:?} on {base:?}"
        );
        assert_eq!(xray.get_pixel(center, center), fill);
        let outside = ((34. * scale) as u32, (120. * scale) as u32);
        assert_eq!(
            front.get_pixel(outside.0, outside.1),
            original.get_pixel(outside.0, outside.1)
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn polygon_materials_render_and_reassign_without_changing_authoring_mesh() {
    use poseforge_core::{Editor, MeshTopology, Vertex};
    use std::collections::BTreeSet;
    let (mut p, v) = material_scene();
    p.materials[0].emissive = [1., 0., 0.];
    let mut blue = p.materials[0].clone();
    blue.id = 4;
    blue.emissive = [0., 0., 1.];
    p.materials.push(blue);
    let m = &mut p.meshes[0];
    m.vertices = [
        [-1., -1., 0.],
        [0., -1., 0.],
        [0., 1., 0.],
        [-1., 1., 0.],
        [1., -1., 0.],
        [1., 1., 0.],
    ]
    .into_iter()
    .map(|position| Vertex {
        position,
        normal: [0., 0., 1.],
        ..Vertex::default()
    })
    .collect();
    m.topology = Some(MeshTopology {
        faces: vec![vec![0, 1, 2, 3], vec![1, 4, 5, 2]].into(),
        face_materials: Default::default(),
        uv_seams: vec![],
        smooth_normals: false,
        primitive: None,
    });
    m.rebuild_polygon_indices().unwrap();
    let mut e = Editor::new(p);
    e.assign_face_material(2, &[1], &BTreeSet::from([1]), 4)
        .unwrap();
    let authored = e.project.clone();
    let mut r = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let im = material_picture(&mut r, &e.project, &v, "24-two-material-quads");
    let left = im.get_pixel(48, 80);
    let right = im.get_pixel(112, 80);
    assert!(
        left[0] > left[2] + 80 && right[2] > right[0] + 80,
        "left {left:?}, right {right:?}"
    );
    assert_eq!(e.project, authored);
    // Reusing the renderer must invalidate just the changed material batch.
    e.assign_face_material(2, &[1], &BTreeSet::from([0]), 4)
        .unwrap();
    let im = material_picture(&mut r, &e.project, &v, "24-reassigned-quads");
    assert!(im.get_pixel(48, 80)[2] > im.get_pixel(48, 80)[0] + 80);
    e.undo();
    let im = material_picture(&mut r, &e.project, &v, "24-undo-quads");
    assert!(im.get_pixel(48, 80)[0] > im.get_pixel(48, 80)[2] + 80);

    e.project.materials[1].alpha_mode = poseforge_core::AlphaMode::Blend;
    e.project.materials[1].base_color[3] = 0.25;
    let im = material_picture(&mut r, &e.project, &v, "24-face-alpha");
    assert_eq!(im.get_pixel(48, 80)[3], 255);
    assert!((63..=65).contains(&im.get_pixel(112, 80)[3]));
    let mut joint = poseforge_core::Node::new(5, "Joint");
    joint.is_joint = true;
    joint.transform.translation.x = 0.5;
    e.project.nodes.push(joint);
    e.project.nodes[0].skin = Some(poseforge_core::Skin {
        joints: vec![5],
        inverse_bind_matrices: vec![glam::Mat4::IDENTITY],
    });
    for vertex in &mut e.project.meshes[0].vertices {
        vertex.weights = [1., 0., 0., 0.];
    }
    let im = material_picture(&mut r, &e.project, &v, "24-skinned-materials");
    assert!(im.get_pixel(80, 80)[0] > im.get_pixel(80, 80)[2] + 80);
    assert_eq!(im.get_pixel(24, 80)[3], 0);
    let len = e.project.meshes[0].vertices.len();
    e.project.meshes[0]
        .morph_targets
        .push(poseforge_core::MorphTarget {
            name: "Lift".into(),
            positions: vec![[0., 0.5, 0.]; len].into(),
            normals: Default::default(),
        });
    e.project.nodes[0].morph_weights = vec![1.];
    let im = material_picture(&mut r, &e.project, &v, "24-skinned-morphed-materials");
    assert_eq!(im.get_pixel(80, 130)[3], 0);
    assert!(im.get_pixel(80, 80)[0] > im.get_pixel(80, 80)[2] + 80);
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn object_selection_shines_through_occluders_without_intersections_or_export() {
    let (mut project, mut view) = material_scene();
    project.render_settings.shadows = false;
    project.materials[0].emissive = [0.2; 3];
    view.viewport = [0., 0., 160., 160.];
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    renderer.set_editor_appearance([0.02, 0.02, 0.02, 1.], false);
    let path = std::env::temp_dir().join("artforge-selection27.png");
    let capture = |r: &mut Renderer, p: &Project| {
        r.capture_viewport_png(
            p,
            &p.global_transforms().unwrap(),
            &view,
            &[],
            &[],
            &[],
            &path,
            160,
            160,
        )
        .unwrap();
        image::open(&path).unwrap().into_rgba8()
    };
    let base = capture(&mut renderer, &project);
    let export = material_picture(&mut renderer, &project, &view, "27-unselected");
    renderer.set_editor_selection(&[1], [0.2, 0.6, 0.9, 1.]);
    let outlined = capture(&mut renderer, &project);
    assert_eq!(
        outlined.get_pixel(80, 80),
        base.get_pixel(80, 80),
        "interior stays unchanged"
    );
    let changed = base
        .pixels()
        .zip(outlined.pixels())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 100 && changed < 2000,
        "silhouette pixels: {changed}"
    );
    let accent = base
        .pixels()
        .zip(outlined.pixels())
        .find(|(a, b)| a != b)
        .unwrap()
        .1;
    for (actual, expected) in accent.0[..3].iter().zip([51u8, 153, 230]) {
        assert!(
            actual.abs_diff(expected) <= 2,
            "selection uses the UI accent in sRGB: {accent:?}"
        );
    }
    assert_eq!(
        export,
        material_picture(&mut renderer, &project, &view, "27-selected-export")
    );
    let mut occluder = project.nodes[0].clone();
    occluder.id = 99;
    occluder.transform.translation.z = 0.1;
    project.nodes.push(occluder);
    let hidden = capture(&mut renderer, &project);
    renderer.set_editor_selection(&[], [0., 1., 0., 1.]);
    assert_ne!(
        hidden,
        capture(&mut renderer, &project),
        "the selected silhouette stays visible through an occluder"
    );
    assert_eq!(
        hidden.get_pixel(80, 80),
        capture(&mut renderer, &project).get_pixel(80, 80),
        "the interior remains unchanged"
    );
    // A narrow foreground object crosses the middle of the selected surface.
    // It must not cut the silhouette or draw a new line across its interior.
    project.nodes.last_mut().unwrap().transform.scale.x = 0.25;
    let partial_base = capture(&mut renderer, &project);
    renderer.set_editor_selection(&[1], [0.2, 0.6, 0.9, 1.]);
    let partial_selected = capture(&mut renderer, &project);
    for y in 65..95 {
        for x in 68..92 {
            assert_eq!(
                partial_base.get_pixel(x, y),
                partial_selected.get_pixel(x, y),
                "no contour on the occluder intersection at {x},{y}"
            );
        }
    }
    assert_ne!(
        partial_base, partial_selected,
        "outer silhouette remains visible"
    );
    // Multiple selected meshes form one mask without internal intersection edges.
    renderer.set_editor_selection(&[1, 99], [0.2, 0.6, 0.9, 1.]);
    let union = capture(&mut renderer, &project);
    for y in 65..95 {
        for x in 68..92 {
            assert_eq!(partial_base.get_pixel(x, y), union.get_pixel(x, y));
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn camera_effects_change_color_preserve_png_alpha_and_restore_when_disabled() {
    let (mut project, mut view) = material_scene();
    project.materials[0].emissive = [0.4, 0.2, 0.1];
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let before = material_picture(&mut renderer, &project, &view, "42-effects-off");
    view.effects = poseforge_core::CameraEffects { enabled: true, exposure: 0.5, contrast: 1.2,
        saturation: 0.2, temperature: -0.5, vignette: 0.8, grain: 0.08, aberration: 4., ..Default::default() };
    let after = material_picture(&mut renderer, &project, &view, "42-effects-on");
    assert!(before.pixels().zip(after.pixels()).filter(|(a,b)| a[3] > 0 && a.0[..3] != b.0[..3]).count() > 100);
    assert!(before.pixels().zip(after.pixels()).all(|(a,b)| a[3] == b[3]));
    let authored=view.effects;
    for field in 4..=10 {view.effects.toggle(field);}
    let muted=material_picture(&mut renderer,&project,&view,"camera-effect-list-muted");
    assert_images_equal(&before,&muted,"muted rows preserve the source image and alpha");
    for field in 4..=10 {view.effects.toggle(field);}
    assert_eq!(view.effects.uniforms(),authored.uniforms());
    view.effects.enabled = false;
    let restored = material_picture(&mut renderer, &project, &view, "42-effects-restored");
    assert_images_equal(&before, &restored, "disabled effects preserve the source image");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn changed_video_pixels_reach_existing_material_bindings_and_restore_exactly() {
    std::thread::Builder::new().stack_size(poseforge_render::RENDERER_STACK_BYTES).spawn(|| {
        let (mut p,view) = material_scene();
        p.materials[0].emissive = [1.;3]; p.materials[0].emissive_texture = Some(10);
        let mut renderer = pollster::block_on(Renderer::headless(160,160)).unwrap();
        let original = p.textures.iter().find(|t|t.id==10).unwrap().rgba8.clone();
        let red = material_picture(&mut renderer,&p,&view,"4726-video-red");
        p.textures.iter_mut().find(|t|t.id==10).unwrap().rgba8 = vec![0,0,255,255].into();
        let blue = material_picture(&mut renderer,&p,&view,"4726-video-blue");
        assert!(i16::from(red.get_pixel(80,80)[0])-i16::from(red.get_pixel(80,80)[2])>40);
        assert!(i16::from(blue.get_pixel(80,80)[2])-i16::from(blue.get_pixel(80,80)[0])>40);
        p.textures.iter_mut().find(|t|t.id==10).unwrap().rgba8 = original;
        let restored = material_picture(&mut renderer,&p,&view,"4726-video-restored");
        assert_images_equal(&red,&restored,"restoring a video frame restores its pixels without new bindings");
    }).unwrap().join().unwrap();
}
