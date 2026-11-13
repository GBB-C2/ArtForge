//! Four vec4s, matching Artistic in artistic.wgsl. No per-material textures.
use bytemuck::{Pod, Zeroable};
use poseforge_core::ArtisticSettings;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuArtistic {
    params: [f32; 4],
    detail: [f32; 4],
    paint: [f32; 4],
    finish: [f32; 4],
}
impl From<ArtisticSettings> for GpuArtistic {
    fn from(s: ArtisticSettings) -> Self {
        // Inactive data is exactly zero, including in reflected material buffers.
        if !s.enabled { return Self::zeroed(); }
        Self { params: [s.style.shader_id() as f32, s.scale, s.bump, s.grain],
            detail: [s.variation, s.tool_marks, s.cracks, s.subsurface],
            paint: [s.pigment, s.paper, s.edge_darkening, s.wash],
            finish: [s.dry_brush, s.seed as f32, 0., 0.] }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn artistic_gpu_layout_and_disabled_fast_path() {
        assert_eq!(std::mem::size_of::<GpuArtistic>(), 64);
        assert_eq!(std::mem::offset_of!(GpuArtistic, paint), 32);
        assert_eq!(std::mem::offset_of!(GpuArtistic, finish), 48);
        let off=GpuArtistic::from(ArtisticSettings::default());
        assert!(bytemuck::bytes_of(&off).iter().all(|v| *v==0));
        for style in poseforge_core::ArtisticStyle::ALL {
            let mut s=ArtisticSettings::for_style(style); s.seed=65535;
            let gpu=GpuArtistic::from(s);
            assert_eq!(gpu.params[0],style.shader_id() as f32);
            assert_eq!(gpu.finish[1],65535.);
            assert_eq!(gpu.detail[3],s.subsurface);
        }
    }
}
