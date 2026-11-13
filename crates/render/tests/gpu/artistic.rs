//! Run explicitly with a real graphics adapter; not part of CPU-only CI.
use super::*;
use poseforge_core::{ArtisticSettings,ArtisticStyle};

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn artistic_toggle_restores_original_and_pigment_keeps_opaque_coverage() {
    let (mut p,view)=material_scene();
    p.render_settings.ambient_intensity=1.;
    p.render_settings.sky_color=[0.7;3]; p.render_settings.ground_color=[0.7;3];
    p.materials[0].base_color=[0.08,0.2,0.45,1.];
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let baseline=material_picture(&mut renderer,&p,&view,"artistic-off");
    p.materials[0].artistic=ArtisticSettings::for_style(ArtisticStyle::Watercolor);
    let painted=material_picture(&mut renderer,&p,&view,"artistic-watercolor");
    assert_ne!(baseline.as_raw(),painted.as_raw());
    assert_eq!(painted.get_pixel(80,80)[3],255);
    p.materials[0].artistic.seed=321;
    let changed=material_picture(&mut renderer,&p,&view,"artistic-watercolor-seed");
    assert_ne!(painted.as_raw(),changed.as_raw());
    p.materials[0].artistic.enabled=false;
    let restored=material_picture(&mut renderer,&p,&view,"artistic-off-restored");
    assert_images_equal(&baseline,&restored,"disabled style must restore PBR");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn artistic_all_presets_render_and_preserve_alpha_coverage() {
    let (mut p,view)=material_scene();
    p.render_settings.ambient_intensity=1.;
    p.render_settings.sky_color=[0.6;3]; p.render_settings.ground_color=[0.3;3];
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    for (index,preset) in poseforge_core::MaterialPreset::ALL[13..].iter().enumerate() {
        p.materials[0]=poseforge_core::Material { id:3,..preset.create() };
        let frame=material_picture(&mut renderer,&p,&view,&format!("artistic-preset-{index}"));
        assert_eq!(frame.get_pixel(80,80)[3],255);
        assert!(rgb_energy(&frame)>0);
    }
}
