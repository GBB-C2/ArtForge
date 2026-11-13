//! Identical layout for primary surfaces and reflected/refracted ray hits.
use bytemuck::{Pod, Zeroable};
use poseforge_core::NprSettings;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuNpr {
    ramp: [f32; 4],
    shadow: [f32; 4],
    highlight: [f32; 4],
    rim: [f32; 4],
    ink: [f32; 4],
    palette_mid: [f32; 4],
    palette_light: [f32; 4],
    hatch: [f32; 4],
    style: [f32; 4],
}
impl From<NprSettings> for GpuNpr {
    fn from(n: NprSettings) -> Self {
        Self {
            ramp: [
                if n.enabled { 1. } else { 0. },
                n.bands as f32,
                n.threshold,
                n.softness,
            ],
            shadow: [n.shadow_color[0], n.shadow_color[1], n.shadow_color[2], 0.],
            highlight: [
                n.specular_size,
                n.specular_strength,
                n.rim_width,
                n.ink_width,
            ],
            rim: [
                n.rim_color[0],
                n.rim_color[1],
                n.rim_color[2],
                n.rim_strength,
            ],
            ink: [n.ink_color[0], n.ink_color[1], n.ink_color[2], 0.],
            palette_mid: [
                n.palette_mid[0],
                n.palette_mid[1],
                n.palette_mid[2],
                n.palette_strength,
            ],
            palette_light: [
                n.palette_light[0],
                n.palette_light[1],
                n.palette_light[2],
                n.palette_position,
            ],
            style: [n.light_wrap, n.light_color, n.halftone_strength, n.halftone_density],
            hatch: [
                n.hatch_strength,
                n.hatch_density,
                n.hatch_angle.to_radians(),
                n.hatch_cross,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_npr_style_has_a_padded_vec4_and_preserves_defaults() {
        let gpu=GpuNpr::from(NprSettings::default());assert_eq!(gpu.style,[0.,1.,0.,24.]);
        assert_eq!(std::mem::size_of::<GpuNpr>(),144);assert_eq!(std::mem::offset_of!(GpuNpr,style),128);
        let comic=GpuNpr::from(NprSettings::preset(3).unwrap());assert!(comic.style[2]>0.);
    }
}
