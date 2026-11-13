use poseforge_core::{TextureMagFilter, TextureMinFilter, TextureSampler, TextureWrap};

pub(super) fn descriptor(
    settings: TextureSampler,
    anisotropy: u16,
) -> wgpu::SamplerDescriptor<'static> {
    use wgpu::FilterMode::{Linear, Nearest};
    let address = |wrap| match wrap {
        TextureWrap::Repeat => wgpu::AddressMode::Repeat,
        TextureWrap::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        TextureWrap::ClampToEdge => wgpu::AddressMode::ClampToEdge,
    };
    let mag_filter = match settings.mag_filter {
        TextureMagFilter::Nearest => Nearest,
        TextureMagFilter::Linear => Linear,
    };
    let (min_filter, mipmap_filter, mips) = match settings.min_filter {
        TextureMinFilter::Nearest => (Nearest, Nearest, false),
        TextureMinFilter::Linear => (Linear, Nearest, false),
        TextureMinFilter::NearestMipmapNearest => (Nearest, Nearest, true),
        TextureMinFilter::LinearMipmapNearest => (Linear, Nearest, true),
        TextureMinFilter::NearestMipmapLinear => (Nearest, Linear, true),
        TextureMinFilter::LinearMipmapLinear => (Linear, Linear, true),
    };
    wgpu::SamplerDescriptor {
        label: Some("material slot sampler"),
        address_mode_u: address(settings.wrap_s),
        address_mode_v: address(settings.wrap_t),
        mag_filter,
        min_filter,
        mipmap_filter,
        // NEAREST and LINEAR explicitly disable mip selection in glTF.
        lod_max_clamp: if mips { 32. } else { 0. },
        anisotropy_clamp: if mag_filter == Linear && min_filter == Linear && mipmap_filter == Linear
        {
            anisotropy
        } else {
            1
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_wrap_and_lod_match_authored_sampling() {
        use wgpu::FilterMode::{Linear as L, Nearest as N};
        use TextureMinFilter::*;
        for (mode, min, mip, max_lod) in [
            (Nearest, N, N, 0.),
            (Linear, L, N, 0.),
            (NearestMipmapNearest, N, N, 32.),
            (LinearMipmapNearest, L, N, 32.),
            (NearestMipmapLinear, N, L, 32.),
            (LinearMipmapLinear, L, L, 32.),
        ] {
            let s = descriptor(
                TextureSampler {
                    min_filter: mode,
                    mag_filter: TextureMagFilter::Nearest,
                    wrap_s: TextureWrap::ClampToEdge,
                    wrap_t: TextureWrap::MirroredRepeat,
                },
                16,
            );
            assert_eq!((s.min_filter, s.mipmap_filter), (min, mip));
            assert_eq!(s.mag_filter, N);
            assert_eq!(s.lod_max_clamp, max_lod);
            assert_eq!(s.anisotropy_clamp, 1);
            assert_eq!(s.address_mode_u, wgpu::AddressMode::ClampToEdge);
            assert_eq!(s.address_mode_v, wgpu::AddressMode::MirrorRepeat);
        }
        let s = descriptor(TextureSampler::default(), 16);
        assert_eq!(s.anisotropy_clamp, 16);
        assert_eq!(s.address_mode_u, wgpu::AddressMode::Repeat);
    }
}
