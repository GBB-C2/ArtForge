//! Reuse GPU images and bindings when only pixels change (video and painting).
use super::mipmap::{self, MipLevel, MipSemantic};
use poseforge_core::Texture;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp { revision: u64, width: u32, height: u32 }
impl Stamp {
    fn of(source: &Texture) -> Self {
        Self { revision: source.rgba8.revision(), width: source.width, height: source.height }
    }
    fn same_extent(self, other: Self) -> bool { self.width == other.width && self.height == other.height }
}
pub(super) struct CachedTexture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    stamp: Stamp,
}
impl CachedTexture {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, source: &Texture, semantic: MipSemantic) -> Self {
        let (texture, view) = create(device, queue, source.width, source.height, &source.rgba8, semantic);
        Self { texture, view, stamp: Stamp::of(source) }
    }
    pub fn matches(&self, source: &Texture) -> bool { self.stamp == Stamp::of(source) }
    /// True only when image dimensions changed and material bindings need replacing.
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, source: &Texture, semantic: MipSemantic) -> bool {
        let stamp = Stamp::of(source);
        if self.stamp == stamp { return false; }
        if !self.stamp.same_extent(stamp) {
            *self = Self::new(device, queue, source, semantic);
            return true;
        }
        let levels = mipmap::generate(source.width, source.height, &source.rgba8, semantic);
        upload(queue, &self.texture, &levels);
        self.stamp = stamp;
        false
    }
}
pub(super) fn create(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32,
    rgba: &[u8], semantic: MipSemantic) -> (wgpu::Texture, wgpu::TextureView) {
        let levels = mipmap::generate(width, height, rgba, semantic);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("semantic material mip chain"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if matches!(
                semantic,
                MipSemantic::ColorSrgb | MipSemantic::ColorSrgbOpaque
            ) {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload(queue, &texture, &levels);
        let view = texture.create_view(&Default::default());
        (texture, view)
}
fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, levels: &[MipLevel]) {
        for (level, data) in levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(data.width * 4),
                    rows_per_image: Some(data.height),
                },
                wgpu::Extent3d {
                    width: data.width,
                    height: data.height,
                    depth_or_array_layers: 1,
                },
            );
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shape_change_invalidates_a_stamp_without_changing_shared_pixels() {
        let mut texture = Texture { id: 1, name: "Frame".into(), width: 2, height: 3, rgba8: vec![255;24].into() };
        let old = Stamp::of(&texture);
        texture.width = 3; texture.height = 2;
        let resized = Stamp::of(&texture);
        assert_eq!(old.revision, resized.revision);
        assert_ne!(old, resized); assert!(!old.same_extent(resized));
        texture.rgba8[0] = 0;
        let edited = Stamp::of(&texture);
        assert_ne!(edited, resized); assert!(edited.same_extent(resized));
    }
    #[test]
    #[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
    fn video_pixels_reuse_gpu_texture_and_bindings_but_resize_replaces_them() {
        std::thread::Builder::new().stack_size(crate::RENDERER_STACK_BYTES).spawn(|| {
            let mut renderer = pollster::block_on(crate::Renderer::headless(16,16)).unwrap();
            let mut p = poseforge_core::Project::default();
            p.materials.push(poseforge_core::Material { id: 1, base_color_texture: Some(2), ..Default::default() });
            p.textures.push(Texture { id: 2, name: "Frame".into(), width: 2, height: 3, rgba8: vec![255;24].into() });
            let global = p.global_transforms().unwrap(); let view = crate::View::default();
            let key = crate::MaterialKey::from_material(&p.materials[0]); let semantic = key.semantic(0);
            renderer.device.push_error_scope(wgpu::ErrorFilter::Validation);
            renderer.sync_scene(&p,&p,&global,&view,false,false).unwrap();
            let image = renderer.textures[&(2,semantic)].texture.clone();
            let binding = renderer.material_bindings[&key].clone();
            for _ in 0..4 {
                p.textures[0].rgba8[0] = p.textures[0].rgba8[0].wrapping_add(1);
                renderer.sync_scene(&p,&p,&global,&view,false,false).unwrap();
                assert_eq!(renderer.textures[&(2,semantic)].texture,image);
                assert_eq!(renderer.material_bindings[&key],binding);
            }
            // Same pixel count and revision, different aspect ratio: the old cache missed this.
            p.textures[0].width = 3; p.textures[0].height = 2;
            renderer.sync_scene(&p,&p,&global,&view,false,false).unwrap();
            assert_ne!(renderer.textures[&(2,semantic)].texture,image);
            assert_ne!(renderer.material_bindings[&key],binding);
            assert_eq!(renderer.textures[&(2,semantic)].texture.width(),3);
            renderer.device.poll(wgpu::Maintain::Wait);
            assert!(pollster::block_on(renderer.device.pop_error_scope()).is_none());
        }).unwrap().join().unwrap();
    }
}
