//! Fixed 256 KiB occupancy atlas; completed masks upload once, never per animation frame.
use poseforge_simulation::fx_collision::{MASK_CELLS, MASK_EDGE};
pub(super) struct Atlas { texture: wgpu::Texture, pub view: wgpu::TextureView, key: Option<u64>,density:wgpu::Texture,pub density_view:wgpu::TextureView,pub sampler:wgpu::Sampler,
    gas_keys:[Option<(u64,u64,u32)>;poseforge_core::MAX_FX] }
impl Atlas {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture=device.create_texture(&wgpu::TextureDescriptor {
            label:Some("FX selected obstacle occupancy"),
            size:wgpu::Extent3d {width:MASK_EDGE as u32,height:MASK_EDGE as u32,depth_or_array_layers:(MASK_EDGE*poseforge_core::MAX_FX) as u32},
            mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D3,format:wgpu::TextureFormat::R8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
        });
        let view=texture.create_view(&Default::default());
        let density=device.create_texture(&wgpu::TextureDescriptor {
            label:Some("FX smoke, temperature, flame and fuel atlas"),size:wgpu::Extent3d{width:32,height:32,depth_or_array_layers:256},
            mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D3,format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
        });
        let density_view=density.create_view(&Default::default());
        let sampler=device.create_sampler(&wgpu::SamplerDescriptor{label:Some("FX gas interpolation"),mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,..Default::default()});
        Self{texture,view,key:None,density,density_view,sampler,gas_keys:[None;poseforge_core::MAX_FX]}
    }
    pub fn sync(&mut self,queue:&wgpu::Queue,state:&super::fx_interactions::State) {
        let Some((key,data))=state.masks() else{self.key=None;return;};
        if self.key==Some(key) {return;}
        debug_assert_eq!(data.len(),MASK_CELLS*poseforge_core::MAX_FX);
        queue.write_texture(wgpu::TexelCopyTextureInfo{texture:&self.texture,mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
            data,wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(MASK_EDGE as u32),rows_per_image:Some(MASK_EDGE as u32)},
            wgpu::Extent3d{width:MASK_EDGE as u32,height:MASK_EDGE as u32,depth_or_array_layers:(MASK_EDGE*poseforge_core::MAX_FX) as u32});
        self.key=Some(key);
    }
    pub fn sync_gas(&mut self,queue:&wgpu::Queue,state:&super::fx_gas::State) {
        let (key,images)=state.images();let mut present=[false;poseforge_core::MAX_FX];
        for (&id,frame) in images {
            let slot=frame.slot;if slot>=poseforge_core::MAX_FX {continue;}present[slot]=true;
            let tag=Some((key,id,frame.tick));if self.gas_keys[slot]==tag {continue;}
            queue.write_texture(wgpu::TexelCopyTextureInfo{texture:&self.density,mip_level:0,origin:wgpu::Origin3d{x:0,y:0,z:slot as u32*32},aspect:wgpu::TextureAspect::All},
                &frame.pixels,wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(128),rows_per_image:Some(32)},wgpu::Extent3d{width:32,height:32,depth_or_array_layers:32});
            self.gas_keys[slot]=tag;
        }
        for (i,found) in present.into_iter().enumerate() {if !found {self.gas_keys[i]=None;}}
    }

}
