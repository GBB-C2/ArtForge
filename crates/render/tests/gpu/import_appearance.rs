use super::*;

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn vertex_colors_multiply_base_and_alpha() {
    use poseforge_core::{AlphaMode, ToneMapping};
    let (mut p, view) = material_scene();
    p.render_settings.ambient_intensity = 1.;
    p.render_settings.sky_color = [1.; 3];
    p.render_settings.ground_color = [1.; 3];
    p.render_settings.tone_mapping = ToneMapping::Linear;
    p.materials[0].base_color = [1.; 4];
    // Isolate diffuse/vertex color from the white dielectric environment reflection.
    p.materials[0].ior = 1.;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let white = material_picture(&mut renderer, &p, &view, "import-white");
    for vertex in &mut p.meshes[0].vertices {
        vertex.color = [1., 0., 0., 0.25];
    }
    p.materials[0].alpha_mode = AlphaMode::Blend;
    let colored = material_picture(&mut renderer, &p, &view, "import-red-alpha");
    let pixel = colored.get_pixel(80, 80);
    assert!(pixel[0] > 80 && pixel[1] < 5 && pixel[2] < 5, "{pixel:?}");
    assert!((63..=65).contains(&pixel[3]), "{pixel:?}");
    p.materials[0].alpha_mode = AlphaMode::Mask;
    let mask = material_picture(&mut renderer, &p, &view, "import-vertex-mask");
    assert_eq!(mask.get_pixel(80, 80)[3], 0);
    p.materials[0].alpha_mode = AlphaMode::Opaque;
    let opaque = material_picture(&mut renderer, &p, &view, "import-vertex-opaque");
    assert_eq!(opaque.get_pixel(80, 80)[3], 255);
    for vertex in &mut p.meshes[0].vertices {
        vertex.color = [1.; 4];
    }
    let restored = material_picture(&mut renderer, &p, &view, "import-white-restored");
    assert_images_equal(
        &white,
        &restored,
        "white COLOR_0 must preserve legacy rendering",
    );
    for (vertex, color) in
        p.meshes[0]
            .vertices
            .iter_mut()
            .zip([[1., 0., 0., 1.], [0., 1., 0., 1.], [0., 0., 1., 1.]])
    {
        vertex.color = color;
    }
    let gradient = material_picture(&mut renderer, &p, &view, "import-rgb-interpolated");
    assert!(gradient.get_pixel(80, 80).0[..3].iter().all(|c| *c > 30));
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn per_slot_wrap_and_filter_update_without_image_duplication() {
    use poseforge_core::{
        TextureMagFilter, TextureMinFilter, TextureSampler, TextureWrap, ToneMapping,
    };
    let (mut p, view) = material_scene();
    p.render_settings.ambient_intensity = 1.;
    p.render_settings.sky_color = [1.; 3];
    p.render_settings.ground_color = [1.; 3];
    p.render_settings.tone_mapping = ToneMapping::Linear;
    p.materials[0].base_color = [1.; 4];
    // Isolate diffuse/vertex color from the white dielectric environment reflection.
    p.materials[0].ior = 1.;
    p.materials[0].base_color_texture = Some(10);
    p.textures[0].width = 2;
    p.textures[0].rgba8 = vec![255, 0, 0, 255, 0, 0, 255, 255].into();
    let nearest = TextureSampler {
        mag_filter: TextureMagFilter::Nearest,
        min_filter: TextureMinFilter::Nearest,
        ..Default::default()
    };
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    for (uv, wrap, red) in [
        (1.25, TextureWrap::Repeat, true),
        (1.25, TextureWrap::ClampToEdge, false),
        (1.25, TextureWrap::MirroredRepeat, false),
        (-0.25, TextureWrap::Repeat, false),
        (-0.25, TextureWrap::MirroredRepeat, true),
        (-0.25, TextureWrap::ClampToEdge, true),
    ] {
        for vertex in &mut p.meshes[0].vertices {
            vertex.uv = [uv, 0.5];
        }
        p.materials[0].texture_samplers[0] = TextureSampler {
            wrap_s: wrap,
            ..nearest
        };
        let picture = material_picture(
            &mut renderer,
            &p,
            &view,
            &format!("import-wrap-{uv}-{wrap:?}"),
        );
        let pixel = picture.get_pixel(80, 80);
        let (a, b) = if red { (0, 2) } else { (2, 0) };
        assert!(
            pixel[a] > 80 && pixel[b] < 5,
            "uv={uv}, wrap={wrap:?}: {pixel:?}"
        );
    }
    for vertex in &mut p.meshes[0].vertices {
        vertex.uv = [0.5, 0.5];
    }
    p.materials[0].texture_samplers[0] = nearest;
    let sharp = material_picture(&mut renderer, &p, &view, "import-nearest");
    p.materials[0].texture_samplers[0] = TextureSampler::default();
    let linear = material_picture(&mut renderer, &p, &view, "import-linear");
    assert!(sharp.get_pixel(80, 80)[0] < 5);
    assert!(linear.get_pixel(80, 80)[0] > 50 && linear.get_pixel(80, 80)[2] > 50);
    for vertex in &mut p.meshes[0].vertices {
        vertex.uv = [1.25, 0.5];
    }
    p.materials[0].texture_samplers[0] = nearest;
    p.materials[0].emissive_texture = Some(10);
    p.materials[0].emissive = [0., 0., 1.];
    p.materials[0].texture_samplers[4] = TextureSampler {
        wrap_s: TextureWrap::ClampToEdge,
        ..nearest
    };
    let mixed = material_picture(&mut renderer, &p, &view, "import-shared-image-samplers");
    assert!(mixed.get_pixel(80, 80)[0] > 80 && mixed.get_pixel(80, 80)[2] > 80);
    assert_eq!(
        p.materials[0].base_color_texture,
        p.materials[0].emissive_texture
    );
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn vertex_colors_and_sampler_reach_reflections() {
    use poseforge_core::{box_mesh, Material, Node, Vec3};
    let (mut p, view) = material_scene();
    p.materials[0].base_color = [1.; 4];
    // Isolate diffuse/vertex color from the white dielectric environment reflection.
    p.materials[0].ior = 1.;
    p.materials[0].metallic = 1.;
    p.materials[0].roughness = 0.045;
    p.materials.push(Material {
        id: 20,
        base_color: [1.; 4],
        roughness: 1.,
        ..Default::default()
    });
    let mut mesh = box_mesh(21, "Reflected color", Vec3::splat(2.), Some(20));
    for vertex in &mut mesh.vertices {
        vertex.color = [1., 0., 0., 1.];
    }
    p.meshes.push(mesh);
    let mut node = Node::new(22, "Behind camera");
    node.mesh = Some(21);
    node.transform.translation = Vec3::new(0., 0., 5.);
    p.nodes.push(node);
    p.render_settings.ambient_intensity = 1.;
    p.render_settings.sky_color = [1.; 3];
    p.render_settings.ground_color = [1.; 3];
    p.render_settings.ray_reflections = true;
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    let red = material_picture(&mut renderer, &p, &view, "import-reflected-red");
    let pixel = red.get_pixel(80, 80);
    assert!(pixel[0] as i32 > pixel[2] as i32 + 40, "{pixel:?}");
    for vertex in &mut p.meshes[1].vertices {
        vertex.color = [0., 0., 1., 1.];
    }
    let blue = material_picture(&mut renderer, &p, &view, "import-reflected-blue");
    let pixel = blue.get_pixel(80, 80);
    assert!(pixel[2] as i32 > pixel[0] as i32 + 40, "{pixel:?}");
    // The ray atlas has its own sampling path. Verify an actual image there,
    // including a live wrap change on the same source pixels.
    use poseforge_core::{TextureMagFilter, TextureMinFilter, TextureSampler, TextureWrap};
    for vertex in &mut p.meshes[1].vertices {
        vertex.color = [1.; 4];
        vertex.uv = [1.25, 0.5];
    }
    p.textures[0].width = 2;
    p.textures[0].rgba8 = vec![255, 0, 0, 255, 0, 0, 255, 255].into();
    p.materials[1].base_color_texture = Some(10);
    p.materials[1].texture_samplers[0] = TextureSampler {
        mag_filter: TextureMagFilter::Nearest,
        min_filter: TextureMinFilter::Nearest,
        ..Default::default()
    };
    renderer.invalidate_assets();
    let repeated = material_picture(&mut renderer, &p, &view, "import-reflected-repeat");
    let pixel = repeated.get_pixel(80, 80);
    assert!(pixel[0] as i32 > pixel[2] as i32 + 40, "{pixel:?}");
    p.materials[1].texture_samplers[0].wrap_s = TextureWrap::ClampToEdge;
    let clamped = material_picture(&mut renderer, &p, &view, "import-reflected-clamp");
    let pixel = clamped.get_pixel(80, 80);
    assert!(pixel[2] as i32 > pixel[0] as i32 + 40, "{pixel:?}");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn vertex_alpha_mask_removes_color_depth_and_raster_or_ray_shadow() {
    use poseforge_core::AlphaMode;
    let (mut p, view) = local_shadow_scene();
    p.materials[1].alpha_mode = AlphaMode::Mask;
    for vertex in &mut p.meshes[1].vertices {
        vertex.color[3] = 0.;
    }
    let mut renderer = pollster::block_on(Renderer::headless(160, 160)).unwrap();
    for rays in [false, true] {
        p.render_settings.ray_shadows = rays;
        p.nodes[1].visible = true;
        let masked = material_picture(
            &mut renderer,
            &p,
            &view,
            &format!("import-mask-shadow-{rays}"),
        );
        p.nodes[1].visible = false;
        let hidden = material_picture(
            &mut renderer,
            &p,
            &view,
            &format!("import-hidden-shadow-{rays}"),
        );
        assert_images_equal(
            &masked,
            &hidden,
            "zero vertex alpha must remove color, depth and shadows",
        );
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn replacing_frame_pixels_with_the_same_texture_id_refreshes_live_bindings() {
    let (mut p,view)=material_scene();p.render_settings.ambient_intensity=1.;p.render_settings.sky_color=[1.;3];p.render_settings.ground_color=[1.;3];p.render_settings.tone_mapping=poseforge_core::ToneMapping::Linear;
    p.materials[0].base_color=[1.;4];p.materials[0].ior=1.;p.materials[0].base_color_texture=Some(10);
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let red=material_picture(&mut renderer,&p,&view,"video-frame-red");
    let t=p.textures.iter_mut().find(|t|t.id==10).unwrap();t.rgba8=[0,0,255,255].repeat((t.width*t.height) as usize).into();
    let blue=material_picture(&mut renderer,&p,&view,"video-frame-blue");
    assert!(red.get_pixel(80,80)[0]>red.get_pixel(80,80)[2]+40);
    assert!(blue.get_pixel(80,80)[2]>blue.get_pixel(80,80)[0]+40);
}
#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn npr_halftone_changes_visible_shading_without_changing_authored_base_color() {
    let (mut p,view)=material_scene();p.render_settings.ambient_intensity=0.4;p.render_settings.sky_color=[1.;3];p.render_settings.ground_color=[1.;3];
    p.materials[0].npr.enabled=true;p.materials[0].npr.rim_strength=0.;p.materials[0].npr.specular_strength=0.;
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let plain=material_picture(&mut renderer,&p,&view,"npr-no-dots");let base=p.materials[0].base_color;
    p.materials[0].npr.halftone_strength=1.;p.materials[0].npr.halftone_density=12.;
    let dots=material_picture(&mut renderer,&p,&view,"npr-dots");assert!(rgb_energy(&dots)<rgb_energy(&plain));assert_eq!(p.materials[0].base_color,base);
}
