use super::*;
use poseforge_core::{Light, LightKind, Vec3};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use std::time::Instant;

fn demanding_scene(width: u32, height: u32) -> (Project, View) {
    let (mut project, mut view) = local_shadow_scene();
    let settings = &mut project.render_settings;
    settings.antialiasing = 4;
    settings.reflection_samples = 16;
    settings.area_shadow_samples = 64;
    settings.shadow_resolution = 2048;
    settings.atmosphere.density = 0.065;
    settings.atmosphere.height = 16.;
    settings.atmosphere.distance = 500.;
    settings.atmosphere.anisotropy = 0.65;
    let original = project.lights[0].clone();
    project.lights = (0..8)
        .map(|i| {
            let mut light = original.clone();
            let angle = i as f32 * std::f32::consts::TAU / 8.;
            light.kind = match i % 4 {
                0 => LightKind::Directional,
                1 => LightKind::Area,
                2 => LightKind::Spot,
                _ => LightKind::Point,
            };
            light.position = Vec3::new(3. * angle.cos(), 2.5, 3. * angle.sin());
            light.direction = -light.position.normalize();
            light.intensity = if light.kind == LightKind::Directional {
                1.2
            } else {
                12.
            };
            light.range = 8.;
            light.volume.enabled = true;
            light
        })
        .collect();
    view.effects.enabled = true;
    view.effects.aberration = 8.;
    view.viewport = [0., 0., width as f32, height as f32];
    (project, view)
}

#[test]
#[ignore = "manual volume frame benchmark on a graphics adapter"]
fn maximum_volume_frame_benchmark() {
    let (w, h) = (512, 288);
    let (scene, view) = demanding_scene(w, h);
    scene.validate().unwrap();
    let globals = scene.global_transforms().unwrap();
    let mut renderer = pollster::block_on(Renderer::headless(w, h)).unwrap();
    renderer.set_editor_shading(3);
    let directory = std::env::var_os("POSEFORGE_RENDER_QA_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("artforge-volume-qa"));
    std::fs::create_dir_all(&directory).unwrap();
    for (name, interactive) in [("export", false), ("viewport", true)] {
        renderer.set_interactive_preview(interactive);
        let path = directory.join(format!("{name}.png"));
        let mut samples = Vec::new();
        for n in 0..4 {
            let started = Instant::now();
            if interactive {
                renderer
                    .capture_viewport_png(&scene, &globals, &view, &[], &[], &[], &path, w, h)
                    .unwrap();
                samples.push(started.elapsed());
            } else {
                let rgba = renderer.render_rgba(&scene, &globals, &view, w, h).unwrap();
                samples.push(started.elapsed());
                if n == 3 {
                    image::save_buffer(&path, &rgba, w, h, image::ColorType::Rgba8).unwrap();
                }
            }
        }
        samples.remove(0);
        samples.sort();
        println!(
            "{name} {w}x{h} median_ms={:.1}",
            samples[1].as_secs_f64() * 1000.
        );
    }
}

#[test]
#[ignore = "requires a graphics adapter"]
fn tiled_volume_is_seamless_and_cancellation_preserves_the_previous_png() {
    let (w, h) = (291, 151);
    let (mut scene, mut view) = local_shadow_scene();
    scene.render_settings.atmosphere.density = 0.045;
    scene.lights[0].volume.enabled = true;
    view.effects.enabled = true;
    view.effects.aberration = 8.;
    let globals = scene.global_transforms().unwrap();
    let mut renderer = pollster::block_on(Renderer::headless(w, h)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("frame.png");
    let export =
        |r: &mut Renderer, s: &Project| r.export_png(s, &globals, &view, &[], &path, w, h, true);
    export(&mut renderer, &scene).unwrap();
    assert_eq!(renderer.frame_stats().volume_tiles, 2);
    let first = image::open(&path).unwrap().to_rgba8();
    assert!(first.pixels().any(|p| p[3] == 255));
    assert!(first.pixels().any(|p| p[3] > 0 && p[3] < 255));
    let mut dark = scene.lights[0].clone();
    dark.intensity = 0.;
    dark.casts_shadow = false;
    scene.lights.extend([dark.clone(), dark]);
    export(&mut renderer, &scene).unwrap();
    assert_eq!(renderer.frame_stats().volume_tiles, 6);
    assert!(
        image::open(&path).unwrap().to_rgba8() == first,
        "tile boundaries and PNG alpha"
    );
    let previous = std::fs::read(&path).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let count = Arc::new(AtomicU32::new(0));
    let stop = cancel.clone();
    let seen = count.clone();
    renderer.set_fx_cancel(cancel.clone());
    renderer.set_volume_progress(move |done, total| {
        assert_eq!((done, total), (1, 6));
        seen.fetch_add(1, Ordering::Relaxed);
        stop.store(true, Ordering::Relaxed);
    });
    assert!(export(&mut renderer, &scene).unwrap_err().contains("отмен"));
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert!(std::fs::read(&path).unwrap() == previous);
    cancel.store(false, Ordering::Relaxed);
    renderer.set_volume_progress(|_, _| {});
    export(&mut renderer, &scene).unwrap();
    assert!(image::open(&path).unwrap().to_rgba8() == first, "retry");
    scene.render_settings.atmosphere.density = 0.;
    export(&mut renderer, &scene).unwrap();
    assert_eq!(renderer.frame_stats().volume_tiles, 0);
    assert!(image::open(path)
        .unwrap()
        .to_rgba8()
        .pixels()
        .any(|p| p[3] == 0));
}

#[test]
#[ignore = "requires a graphics adapter"]
fn navigation_reuses_shadow_maps_and_scene_edits_refresh_them() {
    let (mut scene, mut view) = local_shadow_scene();
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let draw = |r: &mut Renderer, s: &Project, v: &View| {
        r.render_rgba(s, &s.global_transforms().unwrap(), v, 160, 160)
            .unwrap()
    };
    let first = draw(&mut renderer, &scene, &view);
    assert!(renderer.frame_stats().shadow_draws > 0);
    assert!(first == draw(&mut renderer, &scene, &view));
    assert_eq!(renderer.frame_stats().shadow_draws, 0);
    view.eye.x += 0.6;
    scene.lights[0].color = [0.3, 0.9, 0.7];
    assert!(first != draw(&mut renderer, &scene, &view));
    assert_eq!(renderer.frame_stats().shadow_draws, 0);
    scene.nodes[1].transform.translation.x += 0.5;
    let moved = draw(&mut renderer, &scene, &view);
    assert!(renderer.frame_stats().shadow_draws > 0);
    scene.lights[0].position.x += 2.;
    assert!(moved != draw(&mut renderer, &scene, &view));
    assert!(renderer.frame_stats().shadow_draws > 0);
    scene.materials[1].alpha_mode = poseforge_core::AlphaMode::Mask;
    scene.materials[1].base_color[3] = 0.;
    draw(&mut renderer, &scene, &view);
    assert!(renderer.frame_stats().shadow_draws > 0);
    scene.meshes[0].vertices[0].position[1] += 0.2;
    draw(&mut renderer, &scene, &view);
    assert!(renderer.frame_stats().shadow_draws > 0);
    scene.materials[1].base_color[3] = 1.;
    scene.materials[1].base_color_texture = Some(50);
    scene.textures.push(poseforge_core::Texture {
        id: 50,
        name: "Cutout".into(),
        width: 1,
        height: 1,
        rgba8: vec![255; 4].into(),
    });
    let opaque = draw(&mut renderer, &scene, &view);
    scene.textures[0].rgba8[3] = 0;
    let edited = draw(&mut renderer, &scene, &view);
    assert!(opaque != edited);
    assert!(renderer.frame_stats().shadow_draws > 0);
    scene.render_settings.shadows = false;
    draw(&mut renderer, &scene, &view);
    scene.render_settings.shadows = true;
    assert!(edited == draw(&mut renderer, &scene, &view));
    assert!(renderer.frame_stats().shadow_draws > 0);
}

#[test]
#[ignore = "requires a graphics adapter"]
fn directional_fog_matches_the_height_integral_and_navigation_restores_full_resolution() {
    let (w, h) = (95, 71);
    let mut scene = Project::default();
    scene.render_settings.shadows = false;
    scene.render_settings.atmosphere.density = 0.12;
    scene.render_settings.atmosphere.height = 1.3;
    scene.render_settings.atmosphere.distance = 8.;
    scene.lights = vec![Light::default()];
    scene.lights[0].volume.enabled = true;
    let mut view = View {
        viewport: [0., 0., w as f32, h as f32],
        ..View::default()
    };
    let globals = scene.global_transforms().unwrap();
    let mut renderer = pollster::block_on(Renderer::headless(w, h)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fog.png");
    renderer.set_interactive_preview(true);
    for (start, end, ortho) in [
        (-1., -1., false),
        (-1., 2., false),
        (2., -1., true),
        (1., 1., true),
    ] {
        view.eye = Vec3::new(0., start, 2.);
        view.target = Vec3::new(0., end, -1.);
        view.orthographic = ortho.then_some(2.);
        renderer
            .export_png(&scene, &globals, &view, &[], &path, w, h, true)
            .unwrap();
        assert_eq!(renderer.frame_stats().volume_stride, 1);
        let alpha = image::open(&path)
            .unwrap()
            .to_rgba8()
            .get_pixel(w / 2, h / 2)[3];
        let direction = (view.target - view.eye).normalize().y;
        let origin = start + if ortho { 0.01 * direction } else { 0. };
        let step = 8. / 2048.;
        let optical_depth: f32 = (0..2048)
            .map(|i| {
                let y = origin + direction * (i as f32 + 0.5) * step;
                0.12 * step * (-y.max(0.) / 1.3).exp()
            })
            .sum();
        let expected = (255. * (1. - (-optical_depth).exp())).round() as u8;
        assert!(
            alpha.abs_diff(expected) <= 1,
            "measured {alpha}, reference {expected}"
        );
    }
    view.viewport = [7.5, 8., 73., 53.];
    renderer.set_editor_shading(3);
    for (moving, stride) in [(false, 1), (true, 2), (false, 1)] {
        renderer.set_interactive_preview(moving);
        renderer
            .capture_viewport_png(&scene, &globals, &view, &[], &[], &[], &path, w, h)
            .unwrap();
        assert_eq!(renderer.frame_stats().volume_stride, stride);
        assert_eq!(renderer.frame_stats().volume_tiles, 1);
    }
    let still = image::open(&path).unwrap().to_rgba8();
    scene.render_settings.atmosphere.density = 0.;
    renderer
        .capture_viewport_png(&scene, &globals, &view, &[], &[], &[], &path, w, h)
        .unwrap();
    let clear = image::open(&path).unwrap().to_rgba8();
    for (x, y) in [(0, 0), (6, 30), (81, 20), (35, 61), (94, 70)] {
        assert_eq!(still.get_pixel(x, y), clear.get_pixel(x, y));
    }
    assert_ne!(still.get_pixel(44, 34), clear.get_pixel(44, 34));
}

#[test]
#[ignore = "slow full-HD stress test on a graphics adapter"]
fn full_hd_maximum_volume_exports_all_tiles() {
    check_volume_export(1920, 1080);
}

#[test]
#[ignore = "slow maximum-resolution stress test on a graphics adapter"]
fn maximum_project_resolution_completes_with_eight_volume_lights() {
    check_volume_export(4096, 4096);
}

fn check_volume_export(w: u32, h: u32) {
    let (scene, view) = demanding_scene(w, h);
    let globals = scene.global_transforms().unwrap();
    let mut renderer = pollster::block_on(Renderer::headless(w, h)).unwrap();
    let seen = Arc::new(AtomicU32::new(0));
    let completed = seen.clone();
    let total = w.div_ceil(128) * h.div_ceil(128);
    renderer.set_volume_progress(move |done, count| {
        assert_eq!(count, total);
        assert_eq!(completed.fetch_add(1, Ordering::Relaxed) + 1, done);
    });
    let out = renderer.render_rgba(&scene, &globals, &view, w, h).unwrap();
    assert_eq!(out.len(), (w * h * 4) as usize);
    assert_eq!(seen.load(Ordering::Relaxed), total);
    assert_eq!(renderer.frame_stats().volume_stride, 1);
    assert!(out.chunks_exact(4).any(|p| p != &out[..4]));
}
