//! Scene-wide lighting resources: linear HDR radiance, filtered IBL and stackless triangle BVH.
use super::{graph::GpuGraph, pack_lights, GpuLight, MAX_LIGHTS};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use poseforge_core::{EnvironmentMap, Material, Project};
use std::{
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    sync::{Arc, atomic::{AtomicBool, Ordering}},
};
use wgpu::util::DeviceExt;

const MAX_TRIANGLES: usize = 1_000_000;
const ATLAS_SIZE: u32 = 256;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct RayNode {
    pub low: [f32; 4],
    pub high: [f32; 4],
    pub meta: [u32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct RayTriangle {
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub c: [f32; 4],
    pub na: [f32; 4],
    pub nb: [f32; 4],
    pub nc: [f32; 4],
    pub uv_y: [f32; 4],
    /// Packed linear RGBA8 for secondary rays; keeps one million triangles
    /// below the common 128 MiB storage-binding limit. Raster colors stay f32.
    /// The fourth element holds visibility flags: bit 0 means shadow-only.
    pub colors: [u32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RayMaterial {
    base: [f32; 4],
    properties: [f32; 4],
    emission: [f32; 4],
    uv: [f32; 4],
    rotation: [f32; 4],
    base_uv: [[f32; 4]; 2],
    sampling: [u32; 4],
    emission_uv: [[f32; 4]; 2],
    emission_sampling: [u32; 4],
    absorption: [f32; 4],
    optics: [f32; 4],
    graph: GpuGraph,
    npr: super::npr::GpuNpr,
    artistic: super::artistic::GpuArtistic,
    layers: [u32; 4], // next entry, force layer alpha, enabled, volumetric emission
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WorldUniform {
    counts: [u32; 4],
    environment: [f32; 4],
    sh: [[f32; 4]; 9],
    quality: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    lights: [GpuLight; MAX_LIGHTS],
    emitters: [[f32; 4]; super::emissive::MAX_SAMPLES],
    emission_meta: [u32; 4],
}
pub(super) struct World {
    pub uniform: wgpu::Buffer,
    nodes: wgpu::Buffer,
    triangles: wgpu::Buffer,
    materials: wgpu::Buffer,
    source: wgpu::TextureView,
    filtered: wgpu::TextureView,
    atlas: wgpu::TextureView,
    atlas_texture: Option<wgpu::Texture>,
    atlas_revisions: Vec<u64>,
    graph_atlas: wgpu::TextureView,
    graph_texture: Option<wgpu::Texture>,
    graph_edge: u32,
    graph_key: Vec<(u64, u64)>,
    pub graph_layers: BTreeMap<u64, u32>,
    sampler: wgpu::Sampler,
    filter_layout: wgpu::BindGroupLayout,
    filter_pipeline: wgpu::ComputePipeline,
    environment: Option<Arc<EnvironmentMap>>,
    sh: [[f32; 4]; 9],
    geometry_key: Option<u64>,
    preview: super::ray_preview::RayPreview,
    atlas_ids: Option<Vec<u64>>,
    material_key: Option<u64>,
    node_count: u32,
    emission_surfaces: Vec<super::emissive::Surface>,
    emission_key: Option<(u64,u64,usize)>,
    emitters: [[f32;4]; super::emissive::MAX_SAMPLES],
    emission_count: usize,
    emission_volume: bool,
    emission_ready: bool,
}
fn buffer<T: Pod>(device: &wgpu::Device, label: &str, data: &[T]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::STORAGE,
    })
}
fn texture(
    device: &wgpu::Device,
    name: &str,
    width: u32,
    height: u32,
    layers: u32,
    format: wgpu::TextureFormat,
    storage: bool,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(name),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | if storage {
                wgpu::TextureUsages::STORAGE_BINDING
            } else {
                wgpu::TextureUsages::empty()
            },
        view_formats: &[],
    })
}
fn array_view(texture: &wgpu::Texture) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}
fn upload(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    layer: u32,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
    data: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * bytes_per_pixel),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}
fn half(v: f32) -> u16 {
    let b = v.to_bits();
    let e = ((b >> 23) & 255) as i32 - 127 + 15;
    let m = b & 0x7fffff;
    if e <= 0 {
        if e < -10 {
            return 0;
        }
        return (((m | 0x800000) + (1u32 << (13 - e))) >> (14 - e)) as u16;
    }
    let rounded = m + 0x1000;
    ((e as u32 * 1024) + (rounded >> 13)).min(0x7bff) as u16
}
fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write(bytes);
    h.finish()
}

impl World {
    pub fn entries() -> Vec<wgpu::BindGroupLayoutEntry> {
        let mut entries = Vec::new();
        for binding in 4..=7 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 4 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage { read_only: true }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }
        for binding in [8, 9, 11, 12] {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: if binding == 8 {
                        wgpu::TextureViewDimension::D2
                    } else {
                        wgpu::TextureViewDimension::D2Array
                    },
                    multisampled: false,
                },
                count: None,
            });
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 10,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        entries
    }
    pub fn bindings(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![
            wgpu::BindGroupEntry {
                binding: 4,
                resource: self.uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: self.nodes.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: self.triangles.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: self.materials.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(&self.source),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(&self.filtered),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::TextureView(&self.atlas),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(&self.graph_atlas),
            },
        ]
    }
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HDRI and ray settings"),
            size: std::mem::size_of::<WorldUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let source = texture(
            device,
            "empty HDRI",
            1,
            1,
            1,
            wgpu::TextureFormat::Rgba16Float,
            false,
        );
        upload(
            queue,
            &source,
            0,
            1,
            1,
            8,
            bytemuck::cast_slice(&[0u16, 0, 0, 0x3c00]),
        );
        let filtered = texture(
            device,
            "empty filtered HDRI",
            1,
            1,
            8,
            wgpu::TextureFormat::Rgba16Float,
            true,
        );
        let atlas = texture(
            device,
            "empty ray color atlas",
            1,
            1,
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            false,
        );
        upload(queue, &atlas, 0, 1, 1, 4, &[255; 4]);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HDR panorama repeat / pole clamp"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let filter_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GGX environment filter"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GGX environment convolution"),
            source: wgpu::ShaderSource::Wgsl(include_str!("environment.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&filter_layout],
            push_constant_ranges: &[],
        });
        let filter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GGX IBL preparation"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            uniform,
            nodes: buffer(device, "empty BVH", &[RayNode::zeroed()]),
            triangles: buffer(device, "empty triangles", &[RayTriangle::zeroed()]),
            materials: buffer(device, "empty ray materials", &[RayMaterial::zeroed()]),
            source: source.create_view(&Default::default()),
            filtered: array_view(&filtered),
            atlas: array_view(&atlas),
            atlas_texture: None,
            atlas_revisions: Vec::new(),
            graph_atlas: array_view(&atlas),
            graph_texture: None,
            graph_edge: 0,
            graph_key: Vec::new(),
            graph_layers: BTreeMap::new(),
            sampler,
            filter_layout,
            filter_pipeline,
            environment: None,
            sh: [[0.; 4]; 9],
            geometry_key: None,
            preview: Default::default(),
            atlas_ids: None,
            material_key: None,
            node_count: 0,
            emission_surfaces: Vec::new(),
            emission_key: None,
            emitters: [[0.;4];super::emissive::MAX_SAMPLES],
            emission_count: 0,
            emission_volume: false,
            emission_ready: false,
        }
    }
    pub fn has_emissive_volume(&self) -> bool { self.emission_ready && self.emission_volume }
    pub fn pending(&self) -> bool { self.preview.pending() }
    pub fn invalidate(&mut self) {
        self.preview.cancel();
        self.graph_key.clear();
        self.geometry_key = None;
        self.emission_key = None;
        self.emission_ready = false;
        self.atlas_ids = None;
        self.material_key = None;
    }
    fn prepare_environment(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        map: &Arc<EnvironmentMap>,
    ) {
        let source = texture(
            device,
            "linear HDR panorama",
            map.width,
            map.height,
            1,
            wgpu::TextureFormat::Rgba16Float,
            false,
        );
        let pixels: Vec<u16> = map
            .rgb
            .iter()
            .flat_map(|p| [half(p[0]), half(p[1]), half(p[2]), 0x3c00])
            .collect();
        upload(
            queue,
            &source,
            0,
            map.width,
            map.height,
            8,
            bytemuck::cast_slice(&pixels),
        );
        self.source = source.create_view(&Default::default());
        let filtered = texture(
            device,
            "GGX roughness layers",
            256,
            128,
            8,
            wgpu::TextureFormat::Rgba16Float,
            true,
        );
        self.filtered = array_view(&filtered);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GGX prefilter inputs"),
            layout: &self.filter_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.filtered),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("prefilter HDRI once"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.filter_pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(32, 16, 8);
        }
        queue.submit(Some(encoder.finish()));
        self.sh = irradiance(map);
        self.environment = Some(map.clone());
    }
    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        export: bool,
        enable_rays: bool,
        interactive: bool,
    ) -> Result<bool, String> {
        let mut changed = false;
        if let Some(map) = &project.environment_map {
            if self
                .environment
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(map, old))
            {
                map.validate().map_err(|e| e.to_string())?;
                self.prepare_environment(device, queue, map);
                changed = true;
            }
        } else {
            self.environment = None;
        }
        let graph_ids: std::collections::BTreeSet<_> = project
            .materials
            .iter()
            .filter_map(|m| m.graph.as_ref())
            .filter(|g| g.enabled)
            .flat_map(|g| g.nodes.iter().filter_map(|n| n.texture))
            .collect();
        let graph_key: Vec<_> = graph_ids
            .iter()
            .filter_map(|id| {
                project
                    .textures
                    .iter()
                    .find(|t| t.id == *id)
                    .map(|t| (*id, t.rgba8.revision()))
            })
            .collect();
        if graph_key != self.graph_key {
            if graph_key.len() + 1 > device.limits().max_texture_array_layers as usize {
                return Err("Слишком много изображений в узлах".into());
            }
            let edge = project
                .textures
                .iter()
                .filter(|t| graph_ids.contains(&t.id))
                .map(|t| t.width.max(t.height))
                .max()
                .unwrap_or(1)
                .min(1024)
                .min(device.limits().max_texture_dimension_2d);
            if (graph_key.len() as u64 + 1) * edge as u64 * edge as u64 * 4 > 512 * 1024 * 1024 {
                return Err("Изображения узлов превышают бюджет 512 МиБ; уменьшите карты".into());
            }
            let rebuild = self.graph_texture.is_none() || self.graph_edge != edge
                || graph_key.iter().map(|k| k.0).ne(self.graph_key.iter().map(|k| k.0));
            if rebuild {
                let atlas=texture(device,"Node image atlas",edge,edge,graph_key.len() as u32+1,wgpu::TextureFormat::Rgba8Unorm,false);
                upload(queue,&atlas,0,edge,edge,4,&vec![255;edge as usize*edge as usize*4]);
                self.graph_atlas=array_view(&atlas); self.graph_texture=Some(atlas); self.graph_edge=edge;
                self.graph_layers=graph_key.iter().enumerate().map(|(i,(id,_))| (*id,i as u32+1)).collect(); changed=true;
            }
            for (i,(id,revision)) in graph_key.iter().enumerate() {
                if !rebuild && self.graph_key.get(i)==Some(&(*id,*revision)) { continue; }
                let t=project.textures.iter().find(|t| t.id==*id).ok_or("Изображение узла удалено")?;
                let image=image::RgbaImage::from_raw(t.width,t.height,t.rgba8.to_vec()).ok_or("Некорректное изображение узла")?;
                let scaled=image::imageops::resize(&image,edge,edge,image::imageops::FilterType::Triangle);
                upload(queue,self.graph_texture.as_ref().unwrap(),i as u32+1,edge,edge,4,scaled.as_raw());
            }
            self.graph_key = graph_key;
        }
        let settings = project.render_settings;
        let has_emission = project.materials.iter().any(|m| m.visible &&
            (m.emissive.iter().any(|v| *v > 0.) || m.graph.as_ref().is_some_and(|g| g.enabled && g.outputs[3].is_some())));
        let rays = enable_rays && (has_emission || settings.ray_reflections
            || (settings.ray_shadows && settings.shadows)
            || project
                .materials
                .iter()
                .any(|m| m.uses_transmission() && m.geometric_thickness));
        let mut geometry_ready = false;
        self.emission_ready = false;
        if rays {
            let ids: Vec<u64> = project
                .materials
                .iter()
                .flat_map(|m| [m.base_color_texture, m.emissive_texture].into_iter().flatten())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            if ids.len() + 1 > device.limits().max_texture_array_layers as usize {
                return Err("Слишком много карт цвета и свечения для лучевых эффектов: не более 255".into());
            }
            let rebuild = self.atlas_ids.as_ref() != Some(&ids) || self.atlas_texture.is_none();
            if rebuild {
                let atlas = texture(device,"ray base colors",ATLAS_SIZE,ATLAS_SIZE,ids.len() as u32+1,wgpu::TextureFormat::Rgba8UnormSrgb,false);
                upload(queue,&atlas,0,ATLAS_SIZE,ATLAS_SIZE,4,&vec![255;(ATLAS_SIZE*ATLAS_SIZE*4) as usize]);
                self.atlas = array_view(&atlas); self.atlas_texture=Some(atlas); self.atlas_ids=Some(ids.clone());
                self.atlas_revisions.clear(); changed=true;
            }
            let mut revisions=Vec::with_capacity(ids.len());
            for (i,id) in ids.iter().enumerate() {
                let image=project.textures.iter().find(|t| t.id==*id).ok_or("Не найдена карта цвета отражения")?;
                let revision=image.rgba8.revision(); revisions.push(revision);
                if !rebuild && self.atlas_revisions.get(i)==Some(&revision) { continue; }
                let rgba=image::RgbaImage::from_raw(image.width,image.height,image.rgba8.to_vec()).ok_or("Неверная карта цвета")?;
                let scaled=image::imageops::resize(&rgba,ATLAS_SIZE,ATLAS_SIZE,image::imageops::FilterType::Triangle);
                upload(queue,self.atlas_texture.as_ref().unwrap(),i as u32+1,ATLAS_SIZE,ATLAS_SIZE,4,scaled.as_raw());
            }
            self.atlas_revisions=revisions;
            let fallback = Material::default();
            let mut materials: Vec<_> = std::iter::once(&fallback)
                .chain(project.materials.iter())
                .map(|m| -> Result<RayMaterial, String> {
                    Ok(RayMaterial {
                        base: m.base_color,
                        properties: [
                            m.metallic,
                            m.roughness,
                            m.alpha_shader_mode(),
                            m.transmission,
                        ],
                        emission: [m.emissive[0], m.emissive[1], m.emissive[2], m.ior],
                        uv: [m.uv_scale[0], m.uv_scale[1], m.uv_offset[0], m.uv_offset[1]],
                        base_uv: m.texture_uvs[0].rows(),
                        emission_uv: m.texture_uvs[4].rows(),
                        emission_sampling: {
                            use poseforge_core::{TextureMagFilter, TextureWrap};
                            let wrap = |mode| match mode { TextureWrap::Repeat=>0, TextureWrap::MirroredRepeat=>1, TextureWrap::ClampToEdge=>2 };
                            let s = m.texture_samplers[4];
                            [wrap(s.wrap_s),wrap(s.wrap_t),u32::from(s.mag_filter==TextureMagFilter::Nearest),
                                m.emissive_texture.and_then(|id| ids.iter().position(|v|*v==id)).map_or(0,|i|i as u32+1)]
                        },
                        sampling: {
                            use poseforge_core::{TextureMagFilter, TextureWrap};
                            let wrap = |mode| match mode {
                                TextureWrap::Repeat => 0,
                                TextureWrap::MirroredRepeat => 1,
                                TextureWrap::ClampToEdge => 2,
                            };
                            let s = m.texture_samplers[0];
                            [
                                wrap(s.wrap_s),
                                wrap(s.wrap_t),
                                u32::from(s.mag_filter == TextureMagFilter::Nearest),
                                0,
                            ]
                        },
                        rotation: [
                            m.uv_rotation.to_radians().cos(),
                            m.uv_rotation.to_radians().sin(),
                            m.base_color_texture
                                .and_then(|id| ids.iter().position(|v| *v == id))
                                .map_or(0., |i| i as f32 + 1.),
                            if m.double_sided { 1. } else { 0. },
                        ],
                        absorption: {
                            let c = m.attenuation_color.unwrap_or([
                                m.base_color[0],
                                m.base_color[1],
                                m.base_color[2],
                            ]);
                            [
                                c[0],
                                c[1],
                                c[2],
                                m.thickness / m.attenuation_distance.max(0.001),
                            ]
                        },
                        optics: [
                            m.thickness,
                            m.attenuation_distance,
                            if m.geometric_thickness { 1. } else { 0. },
                            0.,
                        ],
                        graph: GpuGraph::new(m, &self.graph_layers)?,
                        npr: m.npr.into(),
                        artistic: m.artistic.into(),
                        layers: [0, 0, u32::from(m.visible), u32::from(m.emission_volume)],
                    })
                })
                .collect::<Result<_, _>>()?;
            for (_, chain) in layer_chains(project) {
                let start = materials.len();
                for (i, source) in chain.iter().copied().enumerate() {
                    let mut entry = materials[source];
                    entry.layers[0] = if i + 1 < chain.len() { (start + i + 1) as u32 } else { 0 };
                    entry.layers[1] = u32::from(i > 0);
                    materials.push(entry);
                }
            }
            let key = hash_bytes(bytemuck::cast_slice(&materials));
            if self.material_key != Some(key) {
                if std::mem::size_of_val(materials.as_slice())
                    > device.limits().max_storage_buffer_binding_size as usize
                {
                    return Err("Материалы превышают буфер трассировки GPU".into());
                }
                self.materials = buffer(device, "ray materials", &materials);
                self.material_key = Some(key);
                changed = true;
            }
            let key = geometry_key(project, globals, export);
            if self.geometry_key != Some(key) {
                let geometry = if export {
                    self.preview.cancel();
                    let (nodes,triangles) = build_geometry(project, globals, export)?;
                    let emission = super::emissive::prepare(&triangles,&AtomicBool::new(false))?;
                    Some((nodes,triangles,emission))
                } else {
                    self.preview.update(key, project, globals)?
                };
                if let Some((nodes, triangles, emission)) = geometry {
                    if std::mem::size_of_val(triangles.as_slice())
                        > device.limits().max_storage_buffer_binding_size as usize
                    {
                        return Err("Геометрия превышает буфер трассировки GPU".into());
                    }
                    self.node_count = nodes.len() as u32;
                    let empty_nodes = [RayNode::zeroed()];
                    let empty_triangles = [RayTriangle::zeroed()];
                    self.nodes = buffer(
                        device,
                        "stackless BVH",
                        if nodes.is_empty() {
                            &empty_nodes
                        } else {
                            &nodes
                        },
                    );
                    self.triangles = buffer(
                        device,
                        "world triangles",
                        if triangles.is_empty() {
                            &empty_triangles
                        } else {
                            &triangles
                        },
                    );
                    self.emission_surfaces = emission;
                    self.emission_key = None;
                    self.geometry_key = Some(key);
                    changed = true;
                }
            } else if self.preview.pending() {
                self.preview.cancel();
            }
            geometry_ready = self.geometry_key == Some(key);
            if geometry_ready {
                let budget = super::emissive::sample_budget(export,interactive);
                let emission_key = (self.material_key.unwrap_or(0),key,budget);
                if self.emission_key != Some(emission_key) {
                    let (samples,count) = super::emissive::pack(&self.emission_surfaces,
                        |index| emission_energy(&materials,index,false),budget);
                    self.emitters = samples;
                    self.emission_count = count;
                    self.emission_volume = self.emission_surfaces.iter().any(|s| emission_energy(&materials,s.material,true)>0.);
                    self.emission_key = Some(emission_key);
                }
                self.emission_ready = true;
            }
        } else {
            self.preview.cancel();
        }
        let (lights, count) = pack_lights(project, export)?;
        let uniform = WorldUniform {
            counts: [
                if geometry_ready { self.node_count } else { 0 },
                count as u32,
                u32::from(geometry_ready && settings.ray_shadows && settings.shadows),
                u32::from(geometry_ready && settings.ray_reflections),
            ],
            environment: [
                if self.environment.is_some() && settings.hdri_enabled {
                    settings.hdri_strength
                } else {
                    0.
                },
                settings.hdri_rotation.to_radians().cos(),
                settings.hdri_rotation.to_radians().sin(),
                if self.environment.is_some() && settings.hdri_enabled && settings.hdri_background {
                    1.
                } else {
                    0.
                },
            ],
            sh: self.sh,
            quality: [
                settings.reflection_samples as f32,
                settings.area_shadow_samples as f32,
                0.0002,
                settings.shadow_strength,
            ],
            sky: [
                settings.sky_color[0],
                settings.sky_color[1],
                settings.sky_color[2],
                settings.ambient_intensity,
            ],
            ground: [
                settings.ground_color[0],
                settings.ground_color[1],
                settings.ground_color[2],
                if self.environment.is_some() && settings.hdri_enabled {
                    1.
                } else {
                    0.
                },
            ],
            lights,
            emitters: self.emitters,
            emission_meta: [if geometry_ready {self.emission_count as u32} else {0},
                u32::from(settings.shadows),0,0],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        Ok(changed)
    }
}

fn emission_energy(materials: &[RayMaterial], mut index: usize, volume: bool) -> f32 {
    let mut energy = 0_f32;
    for _ in 0..65 {
        let Some(m) = materials.get(index) else { break; };
        if m.layers[2] != 0 && (!volume || m.layers[3] != 0) {
            let graph = m.graph.meta[0] > 0 && m.graph.outputs[0][3] >= 0;
            energy = energy.max(m.emission[..3].iter().copied().fold(0_f32,f32::max).max(if graph {1.} else {0.}));
        }
        if m.layers[0] == 0 { break; }
        index = m.layers[0] as usize;
    }
    energy
}
fn geometry_key(project: &Project, globals: &BTreeMap<u64, Mat4>, export: bool) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    export.hash(&mut h);
    for c in &project.collections {
        c.id.hash(&mut h);
        c.visible.hash(&mut h);
        c.render_visible.hash(&mut h);
        c.members.hash(&mut h);
    }
    // Camera, curve and helper motion must not cancel an otherwise current BVH.
    // Mesh transforms and the skin matrices actually used by geometry are below.
    let meshes: HashMap<_,_> = project.meshes.iter().map(|m|(m.id,m)).collect();
    for material in &project.materials {
        material.id.hash(&mut h);
        material.visible.hash(&mut h);
    }
    for node in &project.nodes {
        node.id.hash(&mut h);
        node.mesh.hash(&mut h);
        node.visible.hash(&mut h);
        node.render_visible.hash(&mut h);
        node.shadow_only.hash(&mut h);
        node.parent.hash(&mut h);
        for layer in &node.material_layers { layer.material.hash(&mut h); layer.visible.hash(&mut h); }
        h.write(bytemuck::cast_slice(&node.morph_weights));
        if let Some(mesh) = node.mesh.and_then(|id| meshes.get(&id)) {
            globals.get(&node.id).map(|m| m.to_cols_array().map(f32::to_bits)).hash(&mut h);
            node.skin.is_some().hash(&mut h);
            if let Some(skin) = &node.skin {
                skin.joints.hash(&mut h);
                for joint in &skin.joints {
                    globals.get(joint).map(|m| m.to_cols_array().map(f32::to_bits)).hash(&mut h);
                }
                for matrix in &skin.inverse_bind_matrices {
                    h.write(bytemuck::cast_slice(&matrix.to_cols_array()));
                }
            }
            mesh.id.hash(&mut h);
            mesh.material.hash(&mut h);
            mesh.vertices.len().hash(&mut h);
            mesh.indices.len().hash(&mut h);
            mesh.vertices.revision().hash(&mut h);
            mesh.indices.revision().hash(&mut h);
            for target in &mesh.morph_targets {
                target.positions.revision().hash(&mut h);
                target.normals.revision().hash(&mut h);
            }
        }
    }
    h.finish()
}
fn pack_color(color: [f32; 4]) -> u32 {
    color.into_iter().enumerate().fold(0, |packed, (i, c)| {
        packed | (((c.clamp(0., 1.) * 255.).round() as u32) << (8 * i))
    })
}

/// Identical ordering is used by material packing and BVH indices.
fn layer_chains(project: &Project) -> Vec<(u64, Vec<usize>)> {
    let indices: HashMap<_, _> = project.materials.iter().enumerate().map(|(i,m)| (m.id,if m.visible { i+1 } else { 0 })).collect();
    let meshes: HashMap<_,_> = project.meshes.iter().map(|m|(m.id,m)).collect();
    project.nodes.iter().filter_map(|node| {
        if node.material_layers.is_empty() { return None; }
        let mesh = node.mesh.and_then(|id| meshes.get(&id))?;
        let layers: Vec<_> = node.material_layers.iter().filter(|l| l.visible)
            .filter_map(|l| indices.get(&l.material).copied().filter(|i|*i!=0)).collect();
        if layers.is_empty() { return None; }
        let base = mesh.material.and_then(|id|indices.get(&id).copied()).unwrap_or(0);
        Some((node.id,std::iter::once(base).chain(layers).collect()))
    }).collect()
}

pub(super) fn build_geometry(
    project: &Project,
    globals: &BTreeMap<u64, Mat4>,
    export: bool,
) -> Result<(Vec<RayNode>, Vec<RayTriangle>), String> {
    build_geometry_cancellable(project, globals, export, &AtomicBool::new(false))
}
pub(super) fn build_geometry_cancellable(project: &Project, globals: &BTreeMap<u64, Mat4>, export: bool,
    cancel: &AtomicBool) -> Result<(Vec<RayNode>, Vec<RayTriangle>), String> {
    let check = || if cancel.load(Ordering::Relaxed) { Err("Подготовка геометрии отменена".to_string()) } else { Ok(()) };
    check()?;
    project.global_transforms().map_err(|e| e.to_string())?;
    let material_ids: HashMap<_, _> = project
        .materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id, if m.visible { i as f32 + 1. } else { 0. }))
        .collect();
    let mut layer_indices = HashMap::new();
    let mut next = project.materials.len() + 1;
    for (node, chain) in layer_chains(project) { layer_indices.insert(node, next as f32); next += chain.len(); }
    let mut triangles = Vec::new();
    let visible_ids = project.visible_node_ids(export);
    for (object_index, node) in project.nodes.iter().enumerate() {
        check()?;
        if !visible_ids.contains(&node.id) {
            continue;
        }
        let Some(mesh) = node.mesh.and_then(|id| project.mesh(id)) else { continue; };
        if triangles.len() + mesh.indices.len() / 3 > MAX_TRIANGLES {
            return Err("Трассировка поддерживает до 1 000 000 видимых треугольников. Отключите лучевые эффекты или упростите сцену.".into());
        }
        let model = globals.get(&node.id).copied().unwrap_or(Mat4::IDENTITY);
        let volume = closed_volume_sign_cancellable(mesh, cancel)? * model.determinant().signum();
        let normal_model = if model.determinant().abs() > 1e-8 { model.inverse().transpose() } else { Mat4::IDENTITY };
        let palette: Vec<Mat4> = node
            .skin
            .as_ref()
            .map(|skin| {
                skin.joints
                    .iter()
                    .zip(&skin.inverse_bind_matrices)
                    .map(|(id, bind)| {
                        model.inverse() * globals.get(id).copied().unwrap_or(Mat4::IDENTITY) * *bind
                    })
                    .collect()
            })
            .unwrap_or_default();
        let vertices: Vec<_> = mesh
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if i % 1024 == 0 { check()?; }
                let mut p = Vec3::from(v.position);
                let mut n = Vec3::from(v.normal);
                for (m, &w) in mesh.morph_targets.iter().zip(&node.morph_weights) {
                    p += Vec3::from(m.positions[i]) * w;
                    if !m.normals.is_empty() {
                        n += Vec3::from(m.normals[i]) * w;
                    }
                }
                let mut skin = Mat4::IDENTITY;
                if !palette.is_empty() && v.weights.iter().sum::<f32>() > 0.0001 {
                    skin = Mat4::ZERO;
                    for (&joint, &weight) in v.joints.iter().zip(&v.weights) {
                        skin += palette
                            .get(joint as usize)
                            .copied()
                            .unwrap_or(Mat4::IDENTITY)
                            * weight;
                    }
                }
                let matrix = model * skin;
                let position = matrix.transform_point3(p);
                let normal = if palette.is_empty() {
                    normal_model.transform_vector3(n).normalize_or_zero()
                } else if matrix.determinant().abs() > 1e-6 {
                    matrix
                        .inverse()
                        .transpose()
                        .transform_vector3(n)
                        .normalize_or_zero()
                } else {
                    normal_model.transform_vector3(n)
                        .normalize_or_zero()
                };
                Ok((position, normal, v.uv, v.color))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let material = layer_indices.get(&node.id).copied().unwrap_or_else(|| mesh.material
            .and_then(|id| material_ids.get(&id).copied()).unwrap_or(0.));
        for (face_index, face) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
            if face_index % 1024 == 0 { check()?; }
            let a = vertices[face[0] as usize];
            let b = vertices[face[1] as usize];
            let c = vertices[face[2] as usize];
            if !a.0.is_finite() || !b.0.is_finite() || !c.0.is_finite() {
                return Err("Не конечные координаты трассировки".into());
            }
            if (b.0 - a.0).cross(c.0 - a.0).length_squared() < 1e-20 {
                continue;
            }
            triangles.push(RayTriangle {
                a: a.0.extend(material).to_array(),
                b: b.0.extend(object_index as f32 + 1.).to_array(),
                c: c.0.extend(volume).to_array(),
                na: a.1.extend(a.2[0]).to_array(),
                nb: b.1.extend(b.2[0]).to_array(),
                nc: c.1.extend(c.2[0]).to_array(),
                uv_y: [a.2[1], b.2[1], c.2[1], 0.],
                colors: [pack_color(a.3), pack_color(b.3), pack_color(c.3), u32::from(node.shadow_only)],
            });
        }
    }
    let mut nodes = Vec::new();
    // Partition in place: leaf ranges already form the final traversal order.
    // Avoid duplicating up to 128 MiB of triangle data for an ordered copy.
    if !triangles.is_empty() {
        build_bvh(&mut triangles, &mut nodes, 0, cancel)?;
    }
    Ok((nodes, triangles))
}
// Weld positions across UV/normal seams. Only closed, consistently oriented
// meshes use geometric absorption; open surfaces retain authored thickness.
fn closed_volume_sign_cancellable(mesh: &poseforge_core::Mesh, cancel: &AtomicBool) -> Result<f32, String> {
    type Point = [i64; 3];
    let points: Vec<Point> = mesh
        .vertices
        .iter()
        .map(|v| v.position.map(|x| (x as f64 * 100000.).round() as i64))
        .collect();
    let mut edges: HashMap<(Point, Point), (u32, i32)> = HashMap::new();
    let mut volume = 0f64;
    for (index, face) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
        if index % 1024 == 0 && cancel.load(Ordering::Relaxed) { return Err("Подготовка геометрии отменена".into()); }
        let a = points[face[0] as usize];
        let b = points[face[1] as usize];
        let c = points[face[2] as usize];
        if a == b || b == c || c == a {
            continue;
        }
        for (x, y) in [(a, b), (b, c), (c, a)] {
            let key = if x < y { (x, y) } else { (y, x) };
            let entry = edges.entry(key).or_default();
            entry.0 += 1;
            entry.1 += if x < y { 1 } else { -1 };
        }
        let a = Vec3::from(mesh.vertices[face[0] as usize].position).as_dvec3();
        let b = Vec3::from(mesh.vertices[face[1] as usize].position).as_dvec3();
        let c = Vec3::from(mesh.vertices[face[2] as usize].position).as_dvec3();
        volume += a.dot(b.cross(c));
    }
    if edges.is_empty() || edges.values().any(|&(n, w)| n != 2 || w != 0) || volume.abs() < 1e-15 {
        Ok(0.)
    } else {
        Ok(volume.signum() as f32)
    }
}
fn point(p: [f32; 4]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}
fn build_bvh(
    triangles: &mut [RayTriangle],
    nodes: &mut Vec<RayNode>,
    offset: usize,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) { return Err("Подготовка геометрии отменена".into()); }
    let index = nodes.len();
    nodes.push(RayNode::zeroed());
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for (i, t) in triangles.iter().enumerate() {
        if i % 1024 == 0 && cancel.load(Ordering::Relaxed) { return Err("Подготовка геометрии отменена".into()); }
        for p in [t.a, t.b, t.c] {
            low = low.min(point(p));
            high = high.max(point(p));
        }
    }
    let first = offset as u32;
    let count = if triangles.len() <= 4 {
        triangles.len() as u32
    } else {
        let extent = high - low;
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };
        let middle = triangles.len() / 2;
        triangles.select_nth_unstable_by(middle, |a, b| {
            ((a.a[axis] + a.b[axis] + a.c[axis]) / 3.)
                .total_cmp(&((b.a[axis] + b.b[axis] + b.c[axis]) / 3.))
        });
        let (a, b) = triangles.split_at_mut(middle);
        build_bvh(a, nodes, offset, cancel)?;
        build_bvh(b, nodes, offset + middle, cancel)?;
        0
    };
    nodes[index] = RayNode {
        low: (low - Vec3::splat(1e-5)).extend(0.).to_array(),
        high: (high + Vec3::splat(1e-5)).extend(0.).to_array(),
        meta: [nodes.len() as u32, first, count, 0],
    };
    Ok(())
}
fn basis(n: Vec3) -> [f32; 9] {
    [
        0.282095,
        0.488603 * n.y,
        0.488603 * n.z,
        0.488603 * n.x,
        1.092548 * n.x * n.y,
        1.092548 * n.y * n.z,
        0.315392 * (3. * n.z * n.z - 1.),
        1.092548 * n.x * n.z,
        0.546274 * (n.x * n.x - n.y * n.y),
    ]
}
fn irradiance(map: &EnvironmentMap) -> [[f32; 4]; 9] {
    let mut sh = [[0.; 4]; 9];
    let width = map.width.min(256);
    let height = map.height.min(128);
    let mut weight_sum = 0.;
    for y in 0..height {
        let theta = (y as f32 + 0.5) / height as f32 * std::f32::consts::PI;
        let weight = theta.sin();
        for x in 0..width {
            let phi = ((x as f32 + 0.5) / width as f32 - 0.5) * std::f32::consts::TAU;
            let n = Vec3::new(
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            );
            let b = basis(n);
            let px = (((x as f32 + 0.5) / width as f32) * map.width as f32) as u32;
            let py = (((y as f32 + 0.5) / height as f32) * map.height as f32) as u32;
            let color =
                map.rgb[(py.min(map.height - 1) * map.width + px.min(map.width - 1)) as usize];
            for i in 0..9 {
                for (channel, value) in color.iter().enumerate() {
                    sh[i][channel] += value * b[i] * weight;
                }
            }
            weight_sum += weight;
        }
    }
    for (i, coefficient) in sh.iter_mut().enumerate() {
        let convolution = if i == 0 {
            1.
        } else if i < 4 {
            2. / 3.
        } else {
            0.25
        };
        for value in coefficient.iter_mut().take(3) {
            *value *= 4. * std::f32::consts::PI / weight_sum * convolution;
        }
    }
    sh
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{box_mesh, MorphTarget, Node};

    #[test]
    fn constant_hdr_irradiance_is_direction_independent_and_preserves_energy() {
        let map = EnvironmentMap {
            name: "Constant".into(),
            width: 64,
            height: 32,
            rgb: vec![[4., 2., 0.25]; 64 * 32],
        };
        let sh = irradiance(&map);
        for n in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::Y, Vec3::ONE.normalize()] {
            let b = basis(n);
            for (channel, expected) in [4., 2., 0.25].into_iter().enumerate() {
                let value: f32 = (0..9).map(|i| sh[i][channel] * b[i]).sum();
                assert!((value - expected).abs() < 0.01, "{value} != {expected}");
            }
        }
        for (f, h) in [
            (0., 0),
            (1., 0x3c00),
            (4., 0x4400),
            (0.5, 0x3800),
            (2f32.powi(-24), 1),
            (65504., 0x7bff),
        ] {
            assert_eq!(half(f), h);
        }
        assert_eq!(std::mem::size_of::<RayTriangle>(), 128);
        assert_eq!(std::mem::size_of::<RayNode>(), 48);
    }
    #[test]
    fn bvh_contains_transformed_morph_geometry_and_partitions_every_triangle_once() {
        let mut p = Project::default();
        let mut mesh = box_mesh(1, "box", Vec3::ONE, None);
        mesh.morph_targets.push(MorphTarget {
            name: "up".into(),
            positions: vec![[0., 2., 0.]; mesh.vertices.len()].into(),
            normals: vec![].into(),
        });
        p.meshes.push(mesh);
        let mut n = Node::new(2, "instance");
        n.mesh = Some(1);
        n.morph_weights = vec![0.5];
        n.transform.translation = Vec3::new(3., 4., 5.);
        p.nodes.push(n);
        let globals = p.global_transforms().unwrap();
        let (nodes, triangles) = build_geometry(&p, &globals, false).unwrap();
        assert_eq!(triangles.len(), 12);
        assert_eq!(nodes[0].meta[0] as usize, nodes.len());
        let mut visits = vec![0; triangles.len()];
        for (i, node) in nodes.iter().enumerate() {
            assert!(node.meta[0] as usize > i && node.meta[0] as usize <= nodes.len());
            for t in node.meta[1]..node.meta[1] + node.meta[2] {
                visits[t as usize] += 1;
                for vertex in [
                    triangles[t as usize].a,
                    triangles[t as usize].b,
                    triangles[t as usize].c,
                ] {
                    let v = point(vertex);
                    assert!(v.cmpge(point(node.low)).all() && v.cmple(point(node.high)).all());
                }
            }
        }
        assert!(visits.iter().all(|&n| n == 1));
        assert!((point(nodes[0].low) - Vec3::new(2., 4., 4.)).length() < 0.0001);
        assert!((point(nodes[0].high) - Vec3::new(4., 6., 6.)).length() < 0.0001);
    }
    #[test]
    fn bvh_visibility_honors_parent_and_render_channel_and_invalidates_key() {
        let mut p = Project::default();
        p.meshes.push(box_mesh(1, "box", Vec3::ONE, None));
        let mut parent = Node::new(2, "parent");
        parent.visible = false;
        p.nodes.push(parent);
        let mut child = Node::new(3, "child");
        child.mesh = Some(1);
        child.parent = Some(2);
        p.nodes.push(child);
        let globals = p.global_transforms().unwrap();
        let old = geometry_key(&p, &globals, false);
        assert!(build_geometry(&p, &globals, false).unwrap().1.is_empty());
        assert_eq!(build_geometry(&p, &globals, true).unwrap().1.len(), 12);
        p.nodes[0].visible = true;
        assert_ne!(geometry_key(&p, &globals, false), old);
        assert_eq!(build_geometry(&p, &globals, false).unwrap().1.len(), 12);
        p.nodes[1].render_visible = false;
        assert!(build_geometry(&p, &globals, true).unwrap().1.is_empty());
        p.nodes[0].parent = Some(3);
        assert!(build_geometry(&p, &globals, false).is_err());
    }
}

#[cfg(test)]
mod material_visibility_tests {
    use super::*;
    use poseforge_core::{box_mesh, Node};
    #[test]
    fn shadow_only_flags_are_per_instance_and_respect_both_visibility_channels() {
        let mut p = Project::default();
        p.meshes.push(poseforge_core::box_mesh(3, "Shared cube", Vec3::ONE, None));
        let mut caster = poseforge_core::Node::new(1, "Caster");
        caster.mesh = Some(3);
        let mut visible = poseforge_core::Node::new(2, "Visible");
        visible.mesh = Some(3); visible.transform.translation = Vec3::X * 3.;
        p.nodes = vec![caster, visible];
        let globals = p.global_transforms().unwrap();
        let original_key = geometry_key(&p, &globals, false);
        assert!(build_geometry(&p, &globals, false).unwrap().1.iter().all(|t| t.colors[3] == 0));
        p.nodes[0].shadow_only = true;
        assert_ne!(geometry_key(&p, &globals, false), original_key);
        for export in [false, true] {
            let triangles = build_geometry(&p, &globals, export).unwrap().1;
            assert_eq!(triangles.len(), 24);
            assert_eq!(triangles.iter().filter(|t| t.colors[3] & 1 != 0).count(), 12);
        }
        p.nodes[0].visible = false;
        assert_eq!(build_geometry(&p, &globals, false).unwrap().1.len(), 12);
        assert_eq!(build_geometry(&p, &globals, true).unwrap().1.len(), 24);
        p.nodes[0].render_visible = false;
        assert_eq!(build_geometry(&p, &globals, true).unwrap().1.len(), 12);
    }
    #[test]
    fn hidden_materials_keep_geometry_and_resolve_to_the_implicit_default() {
        let mut p = Project::default();
        p.materials.push(Material { id: 3, ..Default::default() });
        p.meshes.push(poseforge_core::box_mesh(2, "Cube", Vec3::ONE, Some(3)));
        let mut n = poseforge_core::Node::new(1, "Cube");
        n.mesh = Some(2);
        p.nodes.push(n);
        let globals = p.global_transforms().unwrap();
        let key = geometry_key(&p, &globals, false);
        assert_eq!(build_geometry(&p, &globals, false).unwrap().1.len(), 12);
        p.materials[0].visible = false;
        assert_ne!(geometry_key(&p, &globals, false), key);
        for export in [false, true] {
            let triangles = build_geometry(&p, &globals, export).unwrap().1;
            assert_eq!(triangles.len(), 12);
            assert!(triangles.iter().all(|t| t.a[3] == 0.));
        }
        assert_eq!(p.materials.len(), 1, "the fallback is not a project material slot");
        p.materials.push(Material { id: 4, ..Default::default() });
        p.nodes[0].material_layers.push(poseforge_core::MaterialLayer { material: 4, visible: true });
        assert_eq!(layer_chains(&p), vec![(1, vec![0, 2])]);
        p.nodes[0].material_layers[0].visible = false;
        assert!(layer_chains(&p).is_empty());
        assert!(build_geometry(&p, &globals, false).unwrap().1.iter().all(|t| t.a[3] == 0.));
    }
    #[test]
    fn canceled_ray_geometry_never_returns_a_partial_bvh() {
        let mut p = Project::default();
        p.meshes.push(box_mesh(2, "Box", Vec3::ONE, None));
        let mut node = Node::new(1, "Box"); node.mesh = Some(2); p.nodes.push(node);
        let globals = p.global_transforms().unwrap();
        assert!(build_geometry_cancellable(&p, &globals, false, &AtomicBool::new(true)).is_err());
        let (nodes, triangles) = build_geometry_cancellable(&p, &globals, false, &AtomicBool::new(false)).unwrap();
        assert!(!nodes.is_empty()); assert_eq!(triangles.len(), 12);
    }
    #[test]
    fn unskinned_ray_normals_use_the_inverse_transpose_of_nonuniform_scale() {
        let mut p = Project::default();
        let mut mesh = box_mesh(2, "Box", Vec3::ONE, None);
        let normal = Vec3::ONE.normalize();
        for v in &mut mesh.vertices { v.normal = normal.to_array(); }
        p.meshes.push(mesh);
        let mut node = Node::new(1, "Box"); node.mesh = Some(2); node.transform.scale = Vec3::new(2., 1., 0.5); p.nodes.push(node);
        let globals = p.global_transforms().unwrap();
        let (_, triangles) = build_geometry(&p, &globals, false).unwrap();
        let expected = globals[&1].inverse().transpose().transform_vector3(normal).normalize();
        for triangle in triangles {
            for normal in [triangle.na, triangle.nb, triangle.nc] {
                assert!(point(normal).distance(expected) < 1e-6);
            }
        }
    }

}

#[cfg(test)]
mod geometry_key_regressions {
    use super::*;
    use poseforge_core::{box_mesh,Node,Skin};
    #[test]
    fn artistic_ray_material_layout_matches_the_shared_shader() {
        assert_eq!(std::mem::offset_of!(RayMaterial, artistic),2448);
        assert_eq!(std::mem::offset_of!(RayMaterial, layers),2512);
        assert_eq!(std::mem::size_of::<RayMaterial>(),2528);
    }

    fn scene()->Project {
        let mut p=Project::default();p.meshes.push(box_mesh(2,"Mesh",Vec3::ONE,None));
        let mut node=Node::new(1,"Mesh");node.mesh=Some(2);p.nodes.push(node);
        p.nodes.push(Node::new(10,"Camera or helper"));p
    }
    #[test]
    fn helper_motion_does_not_cancel_mesh_bvh_but_inherited_motion_does() {
        let mut p=scene();let first=p.global_transforms().unwrap();
        let key=geometry_key(&p,&first,false);
        p.node_mut(10).unwrap().transform.translation=Vec3::X;
        let moved=p.global_transforms().unwrap();
        assert_eq!(geometry_key(&p,&moved,false),key);
        let a=build_geometry(&p,&first,false).unwrap().1;let b=build_geometry(&p,&moved,false).unwrap().1;
        assert_eq!(bytemuck::cast_slice::<_,u8>(&a),bytemuck::cast_slice::<_,u8>(&b));
        p.node_mut(1).unwrap().parent=Some(10);
        assert_ne!(geometry_key(&p,&p.global_transforms().unwrap(),false),key);
    }
    #[test]
    fn skin_binding_and_joint_motion_invalidate_ray_geometry() {
        let mut p=scene();p.node_mut(10).unwrap().is_joint=true;
        for v in &mut p.meshes[0].vertices {v.joints=[0;4];v.weights=[1.,0.,0.,0.];}
        p.node_mut(1).unwrap().skin=Some(Skin{joints:vec![10],inverse_bind_matrices:vec![Mat4::IDENTITY]});
        let globals=p.global_transforms().unwrap();let key=geometry_key(&p,&globals,false);
        let before=build_geometry(&p,&globals,false).unwrap().1;
        p.node_mut(1).unwrap().skin.as_mut().unwrap().inverse_bind_matrices[0]=Mat4::from_translation(Vec3::X);
        let rebound=geometry_key(&p,&globals,false);assert_ne!(rebound,key);
        let after=build_geometry(&p,&globals,false).unwrap().1;
        let min_x=|triangles:&[RayTriangle]|triangles.iter().flat_map(|t|[t.a[0],t.b[0],t.c[0]]).fold(f32::INFINITY,f32::min);
        assert!((min_x(&after)-min_x(&before)-1.).abs()<1e-5);
        p.node_mut(10).unwrap().transform.translation=Vec3::Y;
        assert_ne!(geometry_key(&p,&p.global_transforms().unwrap(),false),rebound);
    }
}
