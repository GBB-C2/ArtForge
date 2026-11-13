//! Minimal native renderer. The window and all interface geometry are drawn through wgpu.
//! Mesh skinning runs on the GPU. Morph deltas are currently combined on the CPU.
mod material_parts;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use glyphon::{
    Cache, Color, FontSystem, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use poseforge_core::{AlphaMode, LightKind, Material, Project, ToneMapping, Vertex};
mod gpu_timer;
mod graph;
mod mipmap;
mod npr;
mod artistic;
mod sampler;
mod shadow;
mod text_cache;
mod world;
mod emissive;
mod fx;
mod fx_geometry;
mod fx_interactions;
mod fx_mask;
mod fx_gas;
mod shaders;
mod material_texture;
mod ray_preview;
mod readback;
use mipmap::MipSemantic;
const TEXTURE_SEMANTICS: [MipSemantic; 5] = [
    MipSemantic::ColorSrgb,
    MipSemantic::LinearData,
    MipSemantic::Normal,
    MipSemantic::LinearData,
    MipSemantic::ColorSrgbOpaque,
];
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::Arc,
};
use wgpu::util::DeviceExt;
use winit::window::Window;

#[derive(Clone, Debug)]
pub struct UiRect {
    /// UV crop in the cached interface image: origin U/V, width/height.
    pub image_uv: Option<[f32;4]>,
    /// Projected triangle vertices (physical x/y and NDC depth).
    pub triangle: Option<[[f32; 3]; 3]>,
    pub rect: [f32; 4],
    pub color: [f32; 4],
    /// Physical-pixel endpoints and width; rect becomes its clipping box.
    pub segment: Option<[f32; 5]>,
    /// Optional NDC depths at the segment endpoints (equal for a vertex marker).
    /// Compared per fragment against scene depth; None is ordinary UI/X-Ray.
    pub depth: Option<[f32; 2]>,
}
#[derive(Clone, Debug)]
pub struct UiText {
    pub weight: u16,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size: f32,
    pub color: [f32; 4],
}
#[derive(Clone, Debug)]
pub struct View {
    pub effects: poseforge_core::CameraEffects,
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    /// Physical x, y, width, height of the scene viewport.
    pub viewport: [f32; 4],
    pub orthographic: Option<f32>,
    pub depth_of_field: poseforge_core::DepthOfField,
}
impl Default for View {
    fn default() -> Self {
        Self {
            effects: Default::default(),
            eye: Vec3::new(4., 2.8, 6.),
            target: Vec3::new(0., 1., 0.),
            up: Vec3::Y,
            fov_y: 45f32.to_radians(),
            viewport: [0., 0., 1280., 800.],
            orthographic: None,
            depth_of_field: Default::default(),
        }
    }
}
impl View {
    pub fn view_projection(&self) -> Mat4 {
        let aspect = (self.viewport[2] / self.viewport[3].max(1.)).max(0.01);
        let projection = if let Some(h) = self.orthographic {
            let h = h.max(0.01);
            Mat4::orthographic_rh(-h * aspect, h * aspect, -h, h, 0.01, 1000.)
        } else {
            Mat4::perspective_rh(self.fov_y, aspect, 0.01, 1000.)
        };
        projection * Mat4::look_at_rh(self.eye, self.target, self.up)
    }
    pub fn project(&self, p: Vec3) -> Option<[f32; 2]> {
        let p = self.view_projection() * p.extend(1.);
        if p.w <= 0. || p.z < 0. {
            return None;
        }
        let n = p.truncate() / p.w;
        Some([
            self.viewport[0] + (n.x + 1.) * 0.5 * self.viewport[2],
            self.viewport[1] + (1. - n.y) * 0.5 * self.viewport[3],
        ])
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    joints: [u32; 4],
    weights: [f32; 4],
    color: [f32; 4],
}
impl From<Vertex> for GpuVertex {
    fn from(v: Vertex) -> Self {
        Self {
            position: v.position,
            normal: v.normal,
            uv: v.uv,
            joints: v.joints,
            weights: v.weights,
            color: v.color,
        }
    }
}
const VERTEX_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32x4,4=>Float32x4,5=>Float32x4];
const MAX_LIGHTS: usize = 8;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuLight {
    position_kind: [f32; 4],
    direction_range: [f32; 4],
    color_intensity: [f32; 4],
    cone: [f32; 4],
    shape: [f32; 4],
}
fn pack_lights(project: &Project, export: bool) -> Result<([GpuLight; MAX_LIGHTS], usize), String> {
    let mut packed = [GpuLight::zeroed(); MAX_LIGHTS];
    let mut count = 0;
    for (_, light) in project
        .lights
        .iter()
        .enumerate()
        .filter(|(i, _)| project.light_visible(*i, export))
    {
        if count == MAX_LIGHTS {
            return Err("Одновременно можно включить не более 8 источников света".into());
        }
        let kind = match light.kind {
            LightKind::Directional => 0.,
            LightKind::Point => 1.,
            LightKind::Spot => 2.,
            LightKind::Area => 3.,
        };
        let color = light.effective_color();
        packed[count] = GpuLight {
            position_kind: light.position.extend(kind).to_array(),
            direction_range: light
                .direction
                .normalize_or_zero()
                .extend(light.range)
                .to_array(),
            // Saturate only extreme HDR inputs before shader arithmetic; finite f32
            // values may still overflow when color, power and attenuation multiply.
            color_intensity: [
                color[0].min(1e8),
                color[1].min(1e8),
                color[2].min(1e8),
                light.intensity.min(1e8),
            ],
            shape: [
                light.area_rotation.to_radians().cos(),
                light.area_rotation.to_radians().sin(),
                0.,
                if light.casts_shadow { 1. } else { 0. },
            ],
            cone: [
                light.inner_cone_angle.cos(),
                light.outer_cone_angle.cos(),
                light.area_size[0],
                light.area_size[1],
            ],
        };
        count += 1;
    }
    Ok((packed, count))
}
fn pack_volume_lights(project: &Project, export: bool) -> [[f32; 4]; MAX_LIGHTS] {
    let mut result = [[0.; 4]; MAX_LIGHTS];
    for (out, (_, light)) in result.iter_mut().zip(project.lights.iter().enumerate()
        .filter(|(i, _)| project.light_visible(*i, export))) {
        *out = [light.volume.tint[0], light.volume.tint[1], light.volume.tint[2],
            if light.volume.enabled { light.volume.strength } else { 0. }];
    }
    result
}
fn scene_pipeline_index(blend: bool, double_sided: bool, mirrored: bool) -> usize {
    usize::from(blend) * 4 + usize::from(double_sided) * 2 + usize::from(mirrored)
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawUniform {
    model: [[f32; 4]; 4],
    normal_model: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    base_color: [f32; 4],
    params: [f32; 4],
    emissive: [f32; 4],
    maps: [f32; 4],
    surface: [f32; 4], // IOR, coat weight, coat roughness, reserved
    environment: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    lights: [GpuLight; MAX_LIGHTS],
    transmission: [f32; 4],
    viewport: [f32; 4],
    background: [f32; 4],
    secondary: [f32; 4],
    gradient: [f32; 4],
    anisotropy: [f32; 4], // strength, cos(rotation), sin(rotation), reserved
    fiber: [f32; 4],      // sheen weight, perceptual roughness, reserved
    absorption: [f32; 4], // linear color, explicit override
    uv_transform: [f32; 4],
    uv_rotation: [f32; 4],
    map_uvs: [[f32; 4]; 10],
    ambient_occlusion: [f32; 4],
    inverse_view_proj: [[f32; 4]; 4],
    graph: graph::GpuGraph,
    npr: npr::GpuNpr,
    artistic: artistic::GpuArtistic,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UiVertex {
    image: [f32;4],
    rect: [f32; 4],
    basis: [f32; 4],
    color: [f32; 4],
    clip: [f32; 4],
    depth: [f32; 4],
}
impl UiVertex {
    fn from_rect(r: &UiRect, width: f32, height: f32) -> Self {
        let [x, y, w, h] = r.rect;
        let (origin, u, v, feather) = if let Some([a, b, c]) = r.triangle {
            (
                [a[0], a[1]],
                [b[0] - a[0], b[1] - a[1]],
                [c[0] - a[0], c[1] - a[1]],
                0.,
            )
        } else if let Some([ax, ay, bx, by, stroke]) = r.segment {
            let d = glam::Vec2::new(bx - ax, by - ay);
            let normal = glam::Vec2::new(-d.y, d.x).normalize_or_zero();
            let span = stroke + 2.;
            (
                [ax - normal.x * span * 0.5, ay - normal.y * span * 0.5],
                [d.x, d.y],
                [normal.x * span, normal.y * span],
                span,
            )
        } else {
            ([x, y], [w, 0.], [0., h], 0.)
        };
        Self {
            image: r.image_uv.unwrap_or([0.;4]),
            rect: [
                origin[0] / width * 2. - 1.,
                1. - origin[1] / height * 2.,
                u[0] / width * 2.,
                -u[1] / height * 2.,
            ],
            basis: [
                v[0] / width * 2.,
                -v[1] / height * 2.,
                feather,
                f32::from(r.triangle.is_some()),
            ],
            color: r.color,
            clip: [x, y, x + w, y + h],
            depth: if let Some([a, b, c]) = r.triangle {
                [a[2], b[2], f32::from(r.depth.is_some()), c[2]]
            } else {
                r.depth.map_or([0.; 4], |[a, b]| [a, b, 1., 0.])
            },
        }
    }
}
#[cfg(test)]
mod ui_geometry_tests {
    use super::*;
    #[test]
    fn segment_instances_preserve_physical_endpoints_width_and_clip() {
        for (width, height) in [(800., 600.), (1600., 1200.)] {
            let r = UiRect {
                image_uv: None,
                triangle: None,
                rect: [18., 28., 84., 44.],
                color: [1.; 4],
                segment: Some([20., 30., 100., 70., 2.]),
                depth: None,
            };
            let v = UiVertex::from_rect(&r, width, height);
            let to_pixels = |p: [f32; 2]| [(p[0] + 1.) * width * 0.5, (1. - p[1]) * height * 0.5];
            let a = to_pixels([v.rect[0] + v.basis[0] * 0.5, v.rect[1] + v.basis[1] * 0.5]);
            let b = to_pixels([
                v.rect[0] + v.rect[2] + v.basis[0] * 0.5,
                v.rect[1] + v.rect[3] + v.basis[1] * 0.5,
            ]);
            for (actual, expected) in a.into_iter().chain(b).zip([20., 30., 100., 70.]) {
                assert!((actual - expected).abs() < 0.0002);
            }
            assert_eq!(v.clip, [18., 28., 102., 72.]);
            assert_eq!(v.basis[2], 4.);
        }
        assert_eq!(std::mem::size_of::<UiVertex>(), 96);
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParticleCamera {
    view_proj: [[f32; 4]; 4],
    right: [f32; 4],
    up: [f32; 4],
}
struct Drawable {
    mesh_id: u64,
    vertex_revision: u64,
    vertex_count: usize,
    index_revision: u64,
    topology_revision: Option<u64>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    wire_indices: wgpu::Buffer,
    wire_count: u32,
    count: u32,
    uniform: wgpu::Buffer,
    joints: wgpu::Buffer,
    bind: wgpu::BindGroup,
    morph_weights: Vec<f32>,
    bounds: shadow::MeshBounds,
}

fn wire_indices<'a>(faces: impl Iterator<Item = &'a [u32]>) -> Vec<u32> {
    let mut edges = std::collections::BTreeSet::new();
    for face in faces {
        if face.len() < 2 { continue; }
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            if a != b { edges.insert((a.min(b), a.max(b))); }
        }
    }
    edges.into_iter().flat_map(|(a, b)| [a, b]).collect()
}
fn skin_palette_bytes(joints: usize, limits: &wgpu::Limits) -> Result<u64, String> {
    if joints > poseforge_core::MAX_SKIN_JOINTS {
        return Err("Skin превышает лимит 65 536 суставов".into());
    }
    let bytes = joints.max(1) as u64 * std::mem::size_of::<Mat4>() as u64;
    if bytes > limits.max_buffer_size || bytes > u64::from(limits.max_storage_buffer_binding_size) {
        return Err("Палитра суставов превышает лимит буфера GPU".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod skin_palette_tests {
    use super::*;

    #[test]
    fn palette_sizes_cover_large_rigs_and_check_both_device_limits() {
        let limits = wgpu::Limits::default();
        for (joints, bytes) in [
            (0, 64),
            (1, 64),
            (257, 16_448),
            (1024, 65_536),
            (65_536, 4_194_304),
        ] {
            assert_eq!(skin_palette_bytes(joints, &limits).unwrap(), bytes);
        }
        assert!(skin_palette_bytes(65_537, &limits).is_err());
        let mut small = limits.clone();
        small.max_storage_buffer_binding_size = 16_384;
        assert!(skin_palette_bytes(257, &small).is_err());
        small = limits;
        small.max_buffer_size = 16_384;
        assert!(skin_palette_bytes(257, &small).is_err());
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MaterialKey([Option<u64>; 5], bool, [poseforge_core::TextureSampler; 5]);
impl MaterialKey {
    fn from_material(material: &Material) -> Self {
        Self(
            [
                material.base_color_texture,
                material.metallic_roughness_texture,
                material.normal_texture,
                material.occlusion_texture,
                material.emissive_texture,
            ],
            material.alpha_mode != AlphaMode::Opaque,
            material.texture_samplers,
        )
    }
    fn semantic(self, index: usize) -> MipSemantic {
        if index == 0 && !self.1 {
            MipSemantic::ColorSrgbOpaque
        } else {
            TEXTURE_SEMANTICS[index]
        }
    }
}
struct FrameTargets {
    width: u32,
    height: u32,
    samples: u32,
    depth: wgpu::TextureView,
    color: Option<wgpu::TextureView>,
    hdr: wgpu::TextureView,
    hdr_texture: wgpu::Texture,
    refraction_texture: wgpu::Texture,
    refraction_bind: wgpu::BindGroup,
    grid_bind: wgpu::BindGroup,
    depth_single: wgpu::TextureView,
    dof_depth: wgpu::TextureView,
    depth_color: wgpu::TextureView,
    dof_color: wgpu::TextureView,
    selection_color: wgpu::TextureView,
    selection_z: wgpu::TextureView,
    ui_bind: wgpu::BindGroup,
    post_bind: wgpu::BindGroup,
}

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    // exposure, tone map, background mode (solid/alpha/checker), explicit sRGB encode
    params: [f32; 4],
    background: [f32; 4],
    viewport: [f32; 4],
    checker: [f32; 4], // cell pixels, dark, light, unused
    secondary: [f32; 4],
    gradient: [f32; 4],
    inverse_view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    volume: [f32; 4],
    volume_color: [f32; 4],
    volume_lights: [[f32; 4]; MAX_LIGHTS],
    light_meta: [f32; 4],
    lights: [GpuLight; MAX_LIGHTS],
    dof: [f32; 4],
    forward: [f32; 4],
    contour: [f32; 4],
    contour_color: [f32; 4],
    selection: [f32; 4], // display-linear accent and outline radius; zero disables
    camera_grade: [f32; 4],
    camera_lens: [f32; 4],
    fx_meta: [f32;4],
    fx: [fx::GpuFx; poseforge_core::MAX_FX],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuShadowLight {
    view_proj: [[[f32; 4]; 4]; 6],
    // First array layer, face count (0/1/6), perspective near and far.
    params: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowUniform {
    lights: [GpuShadowLight; MAX_LIGHTS],
    // Strength, receiver bias, inverse map dimension, PCF footprint in texels.
    params: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowCameraUniform {
    view_proj: [[f32; 4]; 4],
}
struct ShadowPass {
    camera: wgpu::Buffer,
    bind: wgpu::BindGroup,
}
struct ShadowMap {
    sampling: wgpu::TextureView,
    resolution: u32,
    depth: Vec<wgpu::TextureView>,
    bind: wgpu::BindGroup,
}
/// Actual depth allocation from the most recently rendered frame. The document's
/// requested setting is retained even if the bounded depth pool reduces it.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShadowStats {
    pub requested_resolution: u32,
    pub effective_resolution: u32,
    pub shadow_lights: u32,
    pub layers: u32,
    pub bytes: u64,
}

const EXPORT_BACKGROUND: [f32; 4] = [0.032, 0.043, 0.055, 1.];

/// Preferences belong to the editor, not to the rendered project.
#[derive(Clone, Copy)]
struct EditorAppearance {
    background: [f32; 4],
    show_grid: bool,
}
impl Default for EditorAppearance {
    fn default() -> Self {
        Self {
            background: EXPORT_BACKGROUND,
            show_grid: true,
        }
    }
}
impl EditorAppearance {
    fn new(background: [f32; 4], show_grid: bool) -> Self {
        let mut safe = EXPORT_BACKGROUND;
        for (channel, value) in safe.iter_mut().zip(background) {
            if value.is_finite() {
                *channel = value.clamp(0., 1.);
            }
        }
        // The editor surface is opaque; transparency is an explicit export option.
        safe[3] = 1.;
        Self {
            background: safe,
            show_grid,
        }
    }
    fn background(
        self,
        export: bool,
        transparent: bool,
        project_background: [f32; 3],
    ) -> wgpu::Color {
        if transparent {
            return wgpu::Color::TRANSPARENT;
        }
        let [r, g, b, a] = if export {
            [
                project_background[0],
                project_background[1],
                project_background[2],
                1.,
            ]
        } else {
            self.background
        };
        wgpu::Color {
            r: r as f64,
            g: g as f64,
            b: b as f64,
            a: a as f64,
        }
    }
    fn grid_visible(self, export: bool, transparent: bool) -> bool {
        !export && !transparent && self.show_grid
    }
}

/// Artwork draw counts and UI preparation work for the last frame.
/// Artwork counts exclude the editor grid/UI and GPU initialization clears.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameStats {
    pub ui_text_layouts: usize,
    pub ui_rectangles: usize,
    pub ui_upload_bytes: usize,
    pub ui_buffer_allocations: usize,
    pub candidates: usize,
    pub culled: usize,
    pub scene_draws: usize,
    pub shadow_draws: usize,
    pub depth_draws: usize,
    pub depth_prepass: bool,
    pub refraction_copy: bool,
}
fn surface_retry_pending(failures: u8) -> bool { (1..=3).contains(&failures) }

/// Stack reservation for native GPU initialization workers (including shader compilation).
pub const RENDERER_STACK_BYTES:usize=shaders::STACK_BYTES;
/// Parse and validate the exact built-in shaders without a window or graphics adapter.
pub fn validate_shaders()->Result<(),String> {shaders::sources().map(|_|())}

async fn startup_checkpoint(device:&wgpu::Device,adapter:&str,stage:&str)->Result<(),String> {
    if let Some(error)=device.pop_error_scope().await {
        return Err(format!("Графика · {stage} · {adapter}: {error}"));
    }
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    eprintln!("GPU: {stage} — готово");Ok(())
}

/// Native surface is acquired on the window thread before driver/pipeline work.
pub struct RendererStartup {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    width: u32,
    height: u32,
}
impl RendererStartup {
    pub async fn finish(self) -> Result<Renderer,String> {
        Renderer::initialize(self.instance,Some(self.surface),self.width,self.height).await
    }
}

pub struct Renderer {
    world: world::World,
    gpu_timer: gpu_timer::GpuTimer,
    frame_stats: FrameStats,
    pub adapter_name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    scene_pipelines: Vec<[wgpu::RenderPipeline; 8]>,
    wire_pipelines: Vec<wgpu::RenderPipeline>,
    editor_shading: u8,
    interactive_preview: bool,
    surface_failures: u8,
    effect_time: f64,
    destruction_times: std::collections::BTreeMap<u64,f64>,
    fx_state: fx_interactions::State,
    fx_masks: fx_mask::Atlas,
    fx_gas: fx_gas::State,
    fx_cancel: Arc<std::sync::atomic::AtomicBool>,
    msaa4_supported: bool,
    shadow_pipelines: [wgpu::RenderPipeline; 4],
    depth_pipelines: [wgpu::RenderPipeline; 4],
    refraction_layout: wgpu::BindGroupLayout,
    shadow_uniform: wgpu::Buffer,
    shadow_pass_layout: wgpu::BindGroupLayout,
    shadow_passes: Vec<ShadowPass>,
    shadow_stats: ShadowStats,
    shadow_layout: wgpu::BindGroupLayout,
    shadow_sampler: wgpu::Sampler,
    shadow_map: ShadowMap,
    post_pipeline: wgpu::RenderPipeline,
    post_layout: wgpu::BindGroupLayout,
    post_uniform: wgpu::Buffer,
    ui_pipeline: wgpu::RenderPipeline,
    ui_image: wgpu::TextureView,
    ui_image_sampler: wgpu::Sampler,
    ui_buffer: Option<(wgpu::Buffer, usize)>,
    text_cache: text_cache::TextCache,
    particle_pipelines: Vec<wgpu::RenderPipeline>,
    draw_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    draws: HashMap<u64, Drawable>,
    material_parts: material_parts::MaterialParts,
    textures: HashMap<(u64, MipSemantic), material_texture::CachedTexture>,
    material_bindings: HashMap<MaterialKey, wgpu::BindGroup>,
    material_sampler: wgpu::Sampler,
    material_samplers: HashMap<poseforge_core::TextureSampler, wgpu::Sampler>,
    material_anisotropy: u16,
    white: wgpu::TextureView,
    flat_normal: wgpu::TextureView,
    sheen_albedo: wgpu::TextureView,
    default_material: wgpu::BindGroup,
    floor: Drawable,
    particle_uniform: wgpu::Buffer,
    particle_bind: wgpu::BindGroup,
    font_system: FontSystem,
    swash: SwashCache,
    text_viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    targets: Option<FrameTargets>,
    editor_appearance: EditorAppearance,
    editor_selection: Vec<u64>,
    selection_accent: [f32; 4],
    editor_scale: f32,
}
impl Renderer {
    pub fn gpu_milliseconds(&self) -> Option<f64> {
        self.gpu_timer.milliseconds
    }
    /// Refine CPU ray geometry and retry a transient presentation failure.
    pub fn preview_pending(&self) -> bool { self.world.pending() || self.fx_state.pending() || self.fx_gas.pending() || surface_retry_pending(self.surface_failures) }
    pub fn fx_status(&self)->Option<&str> {self.fx_state.status().or_else(||self.fx_gas.status())}
    pub fn fx_blocked(&self)->bool {self.fx_state.blocked() || self.fx_gas.blocked()}
    pub fn fx_cache_bytes(&self)->usize {self.fx_gas.memory()}
    pub fn fx_bounds(&self,id:u64)->Option<(Vec3,Vec3)> {let g=self.fx_state.guides(id)?;Some((Vec3::from_slice(&g.low[..3]),Vec3::from_slice(&g.high[..3])))}
    pub fn clear_fx_cache(&mut self) {self.fx_gas.clear();}
    /// Share cancellation with FX preparation and GPU frame readback.
    pub fn set_fx_cancel(&mut self,cancel:Arc<std::sync::atomic::AtomicBool>) {self.fx_cancel=cancel;}
    pub fn frame_stats(&self) -> FrameStats {
        self.frame_stats
    }
    pub fn prepare_window(window: Arc<Window>) -> Result<RendererStartup, String> {
        let size = window.inner_size();
        eprintln!("GPU: создание контекста и поверхности окна");
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = instance.create_surface(window).map_err(|e| e.to_string())?;
        Ok(RendererStartup {instance,surface,width:size.width.max(1),height:size.height.max(1)})
    }
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        Self::prepare_window(window)?.finish().await
    }
    pub async fn headless(width: u32, height: u32) -> Result<Self, String> {
        Self::initialize(
            wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default()),
            None,
            width.max(1),
            height.max(1),
        )
        .await
    }
    async fn initialize(
        instance: wgpu::Instance,
        surface: Option<wgpu::Surface<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        eprintln!("GPU: проверка встроенных шейдеров без видеокарты");
        let sources=shaders::sources()?;
        eprintln!("GPU: выбор адаптера");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Не найден совместимый графический адаптер")?;
        let adapter_info = adapter.get_info();
        let adapter_name = format!("{} ({:?})", adapter_info.name, adapter_info.backend);
        eprintln!("GPU: {adapter_name} · создание устройства");
        let anisotropy = if adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING)
        {
            16
        } else {
            1
        };
        let timer_features = if adapter.features().contains(gpu_timer::GpuTimer::features()) {
            gpu_timer::GpuTimer::features()
        } else {
            wgpu::Features::empty()
        };
        let cache_features = adapter.features() & wgpu::Features::PIPELINE_CACHE;
        let limits = wgpu::Limits::default().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Poseforge GPU"),
                    required_features: timer_features | cache_features,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        // Share compiled shader programs between the MSAA, blend and culling
        // variants. Drivers without pipeline-cache support retain their defaults.
        let pipeline_cache = (!cache_features.is_empty()).then(|| {
            // SAFETY: no externally supplied cache bytes are deserialized.
            unsafe {
                device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                    label: Some("ArtForge startup pipelines"),
                    data: None,
                    fallback: true,
                })
            }
        });
        eprintln!("GPU: устройство готово · создание ресурсов");
        let (format, alpha_mode) = if let Some(s) = &surface {
            let caps = s.get_capabilities(&adapter);
            (
                caps.formats
                    .iter()
                    .find(|f| f.is_srgb())
                    .copied()
                    .unwrap_or(caps.formats[0]),
                caps.alpha_modes[0],
            )
        } else {
            (
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::CompositeAlphaMode::Opaque,
            )
        };
        let supports4 = |format| {
            adapter
                .get_texture_format_features(format)
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4)
        };
        let msaa4_supported = supports4(SCENE_FORMAT)
            && supports4(wgpu::TextureFormat::Depth32Float)
            && adapter
                .get_texture_format_features(SCENE_FORMAT)
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE);
        let sample_counts: &[u32] = if msaa4_supported { &[1, 4] } else { &[1] };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        if let Some(s) = &surface {
            s.configure(&device, &config);
        }
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("draw layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let mut texture_entries: Vec<_> = (0..5)
            .chain(std::iter::once(6))
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();
        texture_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        texture_entries.extend((7..12).map(|binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        }));
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PBR material maps"),
            entries: &texture_entries,
        });
        let shadow_pass_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow camera and node images"),
                entries: &{
                    let mut entries = vec![wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }];
                    entries.extend(
                        world::World::entries()
                            .into_iter()
                            .filter(|e| matches!(e.binding, 10 | 12)),
                    );
                    entries
                },
            });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("all-light shadow sampling"),
            entries: &{
                let mut entries = vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                ];
                entries.extend(world::World::entries());
                entries
            },
        });
        let shadow_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("all-light shadow transforms"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow comparison"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let world = world::World::new(&device, &queue);
        let shadow_map = Self::make_shadow_map(
            &device,
            &shadow_layout,
            &shadow_uniform,
            &shadow_sampler,
            1,
            2,
            &world,
        );
        startup_checkpoint(&device,&adapter_name,"ресурсы сцены").await?;
        eprintln!("GPU: компиляция шейдера сцены");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh skinning + PBR"),
            source: wgpu::ShaderSource::Wgsl(
                sources.scene.as_str().into(),
            ),
        });
        startup_checkpoint(&device,&adapter_name,"шейдер сцены").await?;
        // Resolve/composite geometry in linear HDR before applying any display curve.
        let post_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("display transform settings"),
            size: std::mem::size_of::<PostUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR display transform"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D3, multisampled: false }, count: None,
                },
                wgpu::BindGroupLayoutEntry {binding:6,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Texture{sample_type:wgpu::TextureSampleType::Float{filterable:true},view_dimension:wgpu::TextureViewDimension::D3,multisampled:false},count:None},
                wgpu::BindGroupLayoutEntry {binding:7,visibility:wgpu::ShaderStages::FRAGMENT,ty:wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),count:None},
            ],
        });
        let fx_masks = fx_mask::Atlas::new(&device);
        let refraction_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("opaque scene refraction snapshot"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        startup_checkpoint(&device,&adapter_name,"ресурсы постобработки").await?;
        eprintln!("GPU: компиляция шейдера постобработки и FX");
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HDR display transform"),
            source: wgpu::ShaderSource::Wgsl(
                sources.post.as_str().into(),
            ),
        });
        startup_checkpoint(&device,&adapter_name,"шейдер постобработки и FX").await?;
        eprintln!("GPU: создание конвейеров рендера");
        let post_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR display transform"),
            bind_group_layouts: &[&post_layout, &shadow_layout],
            push_constant_ranges: &[],
        });
        let post_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR tone mapping after transparency and MSAA resolve"),
            layout: Some(&post_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: pipeline_cache.as_ref(),
        });
        eprintln!("GPU: конвейер постобработки — готово");
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &draw_layout,
                &texture_layout,
                &shadow_layout,
                &refraction_layout,
            ],
            push_constant_ranges: &[],
        });
        let make_scene = |samples: u32, blend: bool, double_sided: bool, mirrored: bool, wire: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if blend {
                    "transparent mesh"
                } else {
                    "opaque mesh"
                }),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GpuVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &VERTEX_ATTRS,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if wire { "fs_wire" } else { "fs_main" }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SCENE_FORMAT,
                        blend: if blend {
                            Some(wgpu::BlendState::ALPHA_BLENDING)
                        } else {
                            None
                        },
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: if wire { wgpu::PrimitiveTopology::LineList } else { wgpu::PrimitiveTopology::TriangleList },
                    cull_mode: if double_sided {
                        None
                    } else {
                        Some(wgpu::Face::Back)
                    },
                    front_face: if mirrored {
                        wgpu::FrontFace::Cw
                    } else {
                        wgpu::FrontFace::Ccw
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: !blend,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                multiview: None,
                cache: pipeline_cache.as_ref(),
            })
        };
        let scene_pipelines = sample_counts
            .iter()
            .map(|&samples| {
                std::array::from_fn(|index| {
                    make_scene(samples, index & 4 != 0, index & 2 != 0, index & 1 != 0, false)
                })
            })
            .collect();
        eprintln!("GPU: конвейеры материалов — готово");
        let wire_pipelines = sample_counts.iter().map(|&samples| make_scene(samples, false, true, false, true)).collect();
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow caster layout"),
                bind_group_layouts: &[&draw_layout, &texture_layout, &shadow_pass_layout],
                push_constant_ranges: &[],
            });
        let shadow_pipelines = std::array::from_fn(|index| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("all-light shadow caster"),
                layout: Some(&shadow_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_shadow"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GpuVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &VERTEX_ATTRS,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_shadow"),
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: if index & 2 != 0 {
                        None
                    } else {
                        Some(wgpu::Face::Back)
                    },
                    front_face: if index & 1 != 0 {
                        wgpu::FrontFace::Cw
                    } else {
                        wgpu::FrontFace::Ccw
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.,
                        clamp: 0.,
                    },
                }),
                multisample: Default::default(),
                multiview: None,
                cache: pipeline_cache.as_ref(),
            })
        });
        let depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow caster layout"),
                bind_group_layouts: &[&draw_layout, &texture_layout, &shadow_layout],
                push_constant_ranges: &[],
            });
        let depth_pipelines = std::array::from_fn(|index| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("all-light shadow caster"),
                layout: Some(&depth_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GpuVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &VERTEX_ATTRS,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_depth"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: if index & 2 != 0 {
                        None
                    } else {
                        Some(wgpu::Face::Back)
                    },
                    front_face: if index & 1 != 0 {
                        wgpu::FrontFace::Cw
                    } else {
                        wgpu::FrontFace::Ccw
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: 0.,
                        clamp: 0.,
                    },
                }),
                multisample: Default::default(),
                multiview: None,
                cache: pipeline_cache.as_ref(),
            })
        });
        startup_checkpoint(&device,&adapter_name,"конвейеры сцены, теней и FX").await?;
        eprintln!("GPU: создание интерфейса и частиц");
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("custom UI"),
            source: wgpu::ShaderSource::Wgsl(sources.ui.into()),
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI rectangles"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![5=>Float32x4,0=>Float32x4,1=>Float32x4,2=>Float32x4,3=>Float32x4,4=>Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: pipeline_cache.as_ref(),
        });
        let particle_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let particle_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let particle_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &particle_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: particle_uniform.as_entire_binding(),
            }],
        });
        let particle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("diagnostic fluid particles"),
            source: wgpu::ShaderSource::Wgsl(sources.particles.into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&particle_layout],
            push_constant_ranges: &[],
        });
        let particle_pipelines = sample_counts
            .iter()
            .map(|&samples| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("fluid diagnostics"),
                    layout: Some(&pl),
                    vertex: wgpu::VertexState {
                        module: &particle_shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: 12,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![0=>Float32x3],
                        }],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &particle_shader,
                        entry_point: Some("fs_main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: SCENE_FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::LessEqual,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: samples,
                        ..Default::default()
                    },
                    multiview: None,
                    cache: pipeline_cache.as_ref(),
                })
            })
            .collect();
        let white = Self::make_texture(
            &device,
            &queue,
            1,
            1,
            &[255, 255, 255, 255],
            MipSemantic::LinearData,
        );
        let flat_normal = Self::make_texture(
            &device,
            &queue,
            1,
            1,
            &[128, 128, 255, 255],
            MipSemantic::Normal,
        );
        let material_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material trilinear anisotropic repeat"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: anisotropy,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let sheen_albedo = Self::make_texture(
            &device,
            &queue,
            64,
            64,
            include_bytes!("sheen_albedo.rgba8"),
            MipSemantic::LinearData,
        );
        let default_material = Self::make_material_binding(
            &device,
            &texture_layout,
            &material_sampler,
            [&material_sampler; 5],
            [&white, &white, &flat_normal, &white, &white],
            &sheen_albedo,
        );
        let fv: Vec<GpuVertex> = [
            [-50., -0.005, -50.],
            [-50., -0.005, 50.],
            [50., -0.005, 50.],
            [50., -0.005, -50.],
        ]
        .into_iter()
        .map(|p| GpuVertex {
            position: p,
            normal: [0., 1., 0.],
            uv: [0., 0.],
            joints: [0; 4],
            weights: [0.; 4],
            color: [1.; 4],
        })
        .collect();
        let floor = Self::make_draw(
            &device,
            &draw_layout,
            u64::MAX,
            &fv,
            &[0, 1, 2, 0, 2, 3],
            &[0, 1, 1, 2, 2, 3, 3, 0],
            64,
        );
        startup_checkpoint(&device,&adapter_name,"интерфейс и частицы").await?;
        eprintln!("GPU: подготовка шрифтов");
        let font_system = text_cache::font_system();
        let swash = SwashCache::new();
        let cache = Cache::new(&device);
        let text_viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let gpu_timer = gpu_timer::GpuTimer::new(&device);
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!("Графика · шрифты и завершение подготовки · {adapter_name}: {error}"));
        }
        let ui_image_sampler=device.create_sampler(&wgpu::SamplerDescriptor {
            label:Some("interface image sampling"),mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,..Default::default()
        });
        eprintln!("GPU: подготовка завершена");
        Ok(Self {
            gpu_timer,
            world,
            frame_stats: FrameStats::default(),
            adapter_name,
            device,
            queue,
            surface,
            config,
            scene_pipelines,
            wire_pipelines,
            editor_shading: 3,
            interactive_preview: false,
            surface_failures: 0,
            effect_time: 0.,
            destruction_times: Default::default(),
            fx_state: Default::default(),
            fx_masks,
            fx_gas:Default::default(),
            fx_cancel:Arc::new(std::sync::atomic::AtomicBool::new(false)),
            msaa4_supported,
            shadow_pipelines,
            depth_pipelines,
            refraction_layout,
            shadow_uniform,
            shadow_pass_layout,
            shadow_passes: Vec::new(),
            shadow_stats: ShadowStats::default(),
            shadow_layout,
            shadow_sampler,
            shadow_map,
            post_pipeline,
            post_layout,
            post_uniform,
            ui_image: white.clone(),
            ui_image_sampler,
            ui_pipeline,
            ui_buffer: None,
            text_cache: text_cache::TextCache::default(),
            particle_pipelines,
            draw_layout,
            texture_layout,
            draws: HashMap::new(),
            material_parts: Default::default(),
            textures: HashMap::new(),
            material_bindings: HashMap::new(),
            material_sampler,
            material_samplers: HashMap::new(),
            material_anisotropy: anisotropy,
            white,
            flat_normal,
            sheen_albedo,
            default_material,
            floor,
            particle_uniform,
            particle_bind,
            font_system,
            swash,
            text_viewport,
            atlas,
            text_renderer,
            targets: None,
            editor_appearance: EditorAppearance::default(),
            editor_selection: Vec::new(),
            selection_accent: [0.2, 0.65, 1., 1.],
            editor_scale: 1.,
        })
    }
    /// Set the interactive viewport background (linear RGB) and helper grid.
    /// PNG exports use the project's background and omit UI and the helper grid.
    /// Authored scene meshes retain their visibility and physical shadows.
    pub fn set_editor_shading(&mut self, mode: u8) { self.editor_shading = mode.min(3); }
    pub fn set_interactive_preview(&mut self, active: bool) { self.interactive_preview = active; }
    /// Upload once; interface redraws use a single textured quad, not pixel geometry.
    pub fn set_ui_image(&mut self,image:&poseforge_core::Texture)->Result<(),String> {
        let max=self.device.limits().max_texture_dimension_2d.min(4096);
        if image.width==0 || image.height==0 || image.width>max || image.height>max
            || image.rgba8.len()!=image.width as usize*image.height as usize*4 {return Err("Invalid interface image".into());}
        self.ui_image=Self::make_texture(&self.device,&self.queue,image.width,image.height,&image.rgba8,MipSemantic::ColorSrgb);
        self.targets=None;Ok(())
    }
    /// Transient animation clock: advancing volumes never invalidates scene geometry.
    pub fn set_effect_time(&mut self, time: f64) { self.effect_time = if time.is_finite() {time.max(0.)}else{0.}; }
    pub fn set_destruction_times(&mut self, times: std::collections::BTreeMap<u64,f64>) { self.destruction_times=times; }
    pub fn set_editor_appearance(&mut self, background: [f32; 4], show_grid: bool) {
        self.editor_appearance = EditorAppearance::new(background, show_grid);
    }
    /// Keep the display-only checker squares constant in logical pixels.
    /// Selection is viewport-only and never changes an exported image.
    pub fn set_editor_selection(&mut self, nodes: &[u64], accent: [f32; 4]) {
        self.editor_selection.clear();
        self.editor_selection.extend_from_slice(nodes);
        self.selection_accent = accent.map(|v| {
            let v = if v.is_finite() { v.clamp(0., 1.) } else { 0. };
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
    }
    pub fn set_editor_scale(&mut self, scale: f32) {
        self.editor_scale = if scale.is_finite() {
            scale.clamp(0.5, 8.)
        } else {
            1.
        };
    }
    /// The requested setting remains in the document; this reports the actual
    /// hardware-supported sample count used by both the viewport and PNG export.
    pub fn effective_antialiasing(&self, requested: u32) -> u32 {
        if requested >= 4 && self.msaa4_supported {
            4
        } else {
            1
        }
    }
    pub fn shadow_stats(&self) -> ShadowStats {
        self.shadow_stats
    }
    /// Each semantic has an independently filtered, explicitly typed image chain.
    /// This path also works on adapters without reinterpretable texture views.
    pub fn uses_compatible_texture_uploads(&self) -> bool {
        true
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface_failures = 0;
        if let Some(s) = &self.surface {
            s.configure(&self.device, &self.config);
        }
    }
    pub fn invalidate_assets(&mut self) {
        self.world.invalidate();
        self.draws.clear();
        self.material_bindings.clear();
        self.textures.clear();
    }
    fn make_texture(
        device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32,
        rgba: &[u8], semantic: MipSemantic,
    ) -> wgpu::TextureView {
        material_texture::create(device, queue, width, height, rgba, semantic).1
    }
    fn make_shadow_map(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        resolution: u32,
        layers: u32,
        world: &world::World,
    ) -> ShadowMap {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bounded all-light shadow depth array"),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let sampling = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(layers),
            ..Default::default()
        });
        let depth = (0..layers)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let bind = Self::make_world_bind(device, layout, uniform, sampler, &sampling, world);
        ShadowMap {
            sampling,
            resolution,
            depth,
            bind,
        }
    }
    fn make_world_bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        sampling: &wgpu::TextureView,
        world: &world::World,
    ) -> wgpu::BindGroup {
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(sampling),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ];
        entries.extend(world.bindings());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadows / HDRI / ray geometry"),
            layout,
            entries: &entries,
        })
    }
    fn make_material_binding(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        samplers: [&wgpu::Sampler; 5],
        maps: [&wgpu::TextureView; 5],
        sheen_albedo: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let mut entries: Vec<_> = maps
            .iter()
            .enumerate()
            .map(|(binding, view)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 5,
            resource: wgpu::BindingResource::Sampler(sampler),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 6,
            resource: wgpu::BindingResource::TextureView(sheen_albedo),
        });
        entries.extend(
            samplers
                .iter()
                .enumerate()
                .map(|(index, sampler)| wgpu::BindGroupEntry {
                    binding: 7 + index as u32,
                    resource: wgpu::BindingResource::Sampler(sampler),
                }),
        );
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cached material bindings"),
            layout,
            entries: &entries,
        })
    }
    fn make_draw(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        mesh_id: u64,
        vertices: &[GpuVertex],
        indices: &[u32],
        lines: &[u32],
        joint_bytes: u64,
    ) -> Drawable {
        let bounds = shadow::MeshBounds::new(vertices);
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let wire_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("portable wire edges"), contents: bytemuck::cast_slice(lines), usage: wgpu::BufferUsages::INDEX,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("object uniform"),
            size: std::mem::size_of::<DrawUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let joints = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("joint matrix palette"),
            size: joint_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: joints.as_entire_binding(),
                },
            ],
        });
        Drawable {
            mesh_id,
            vertex_revision: 0,
            vertex_count: vertices.size() as usize / std::mem::size_of::<GpuVertex>(),
            index_revision: 0,
            topology_revision: None,
            vertices,
            indices: indices_buffer,
            wire_indices,
            wire_count: lines.len() as u32,
            count: indices.len() as u32,
            uniform,
            joints,
            bind,
            morph_weights: vec![],
            bounds,
        }
    }
    fn sync_scene(
        &mut self,
        project: &Project,
        ray_project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        export: bool,
        transparent: bool,
    ) -> Result<(), String> {
        self.fx_state.update_with_cancel(project,globals,export,&self.fx_cancel)?;
        self.fx_masks.sync(&self.queue,&self.fx_state);
        self.fx_gas.update(self.fx_state.gas(),project,self.effect_time,export,&self.fx_cancel)?;
        self.fx_masks.sync_gas(&self.queue,&self.fx_gas);
        if self
            .world
            .sync(&self.device, &self.queue, ray_project, globals, export, export || self.editor_shading >= 2, self.interactive_preview)?
        {
            self.shadow_passes.clear();
            self.shadow_map.bind = Self::make_world_bind(
                &self.device,
                &self.shadow_layout,
                &self.shadow_uniform,
                &self.shadow_sampler,
                &self.shadow_map.sampling,
                &self.world,
            );
        }
        let programs: HashMap<_, _> = project
            .materials
            .iter()
            .map(|m| graph::GpuGraph::new(m, &self.world.graph_layers).map(|g| (m.id, g)))
            .collect::<Result<_, _>>()?;
        // Upload only referenced semantic chains. One source image can be used as
        // both color and data, but their filtered mip pixels must not share storage.
        let needed: std::collections::HashSet<_> = project
            .materials
            .iter()
            .flat_map(|material| {
                let key = MaterialKey::from_material(material);
                key.0
                    .into_iter()
                    .enumerate()
                    .filter_map(move |(index, id)| id.map(|id| (id, key.semantic(index))))
            })
            .collect();
        self.textures.retain(|key, _| needed.contains(key));
        for (id, semantic) in needed {
            let Some(texture) = project.textures.iter().find(|texture| texture.id == id) else { continue; };
            if self.textures.get(&(id,semantic)).is_some_and(|cached| cached.matches(texture)) { continue; }
            let expected = u64::from(texture.width) * u64::from(texture.height) * 4;
            if texture.width == 0 || texture.height == 0 || expected != texture.rgba8.len() as u64 {
                return Err(format!("Некорректные пиксели текстуры {}", texture.name));
            }
            if texture.width > self.device.limits().max_texture_dimension_2d
                || texture.height > self.device.limits().max_texture_dimension_2d
            {
                return Err(format!("Текстура {} превышает лимит GPU", texture.name));
            }
            let replaced = if let Some(cached) = self.textures.get_mut(&(id, semantic)) {
                cached.update(&self.device, &self.queue, texture, semantic)
            } else {
                self.textures.insert((id, semantic), material_texture::CachedTexture::new(&self.device, &self.queue, texture, semantic));
                true
            };
            if replaced { self.material_bindings.retain(|key,_| !key.0.contains(&Some(id))); }
        }
        // Scalars are uniforms. Pixel edits upload into existing images; only a
        // change of dimensions/source/semantic requires new material bindings.
        let live_keys: std::collections::HashSet<_> = project
            .materials
            .iter()
            .map(MaterialKey::from_material)
            .collect();
        self.material_bindings
            .retain(|key, _| live_keys.contains(key));
        let live_samplers: std::collections::HashSet<_> =
            live_keys.iter().flat_map(|key| key.2).collect();
        self.material_samplers
            .retain(|key, _| live_samplers.contains(key));
        for settings in live_samplers {
            self.material_samplers.entry(settings).or_insert_with(|| {
                self.device
                    .create_sampler(&sampler::descriptor(settings, self.material_anisotropy))
            });
        }
        for key in live_keys {
            if self.material_bindings.contains_key(&key) {
                continue;
            }
            let image = |index: usize| {
                key.0[index].and_then(|id| self.textures.get(&(id, key.semantic(index)))).map(|cached| &cached.view)
            };
            let maps = [
                image(0).unwrap_or(&self.white),
                image(1).unwrap_or(&self.white),
                image(2).unwrap_or(&self.flat_normal),
                image(3).unwrap_or(&self.white),
                image(4).unwrap_or(&self.white),
            ];
            let binding = Self::make_material_binding(
                &self.device,
                &self.texture_layout,
                &self.material_sampler,
                key.2.map(|settings| &self.material_samplers[&settings]),
                maps,
                &self.sheen_albedo,
            );
            self.material_bindings.insert(key, binding);
        }
        let (mut lights, mut light_count) = pack_lights(project, export)?;
        fx::append_scene_lights(project,globals,export,self.effect_time,&mut lights,&mut light_count,&self.fx_state,&self.fx_gas);
        let settings = project.render_settings;
        let refraction_msaa = if self.effective_antialiasing(settings.antialiasing) > 1 {
            1.
        } else {
            0.
        };
        let background =
            if settings.background_mode == poseforge_core::BackgroundMode::Theme && !export {
                [
                    self.editor_appearance.background[0],
                    self.editor_appearance.background[1],
                    self.editor_appearance.background[2],
                ]
            } else {
                settings.background_color
            };
        let mode = match settings.background_mode {
            poseforge_core::BackgroundMode::Linear => 2.,
            poseforge_core::BackgroundMode::Radial => 3.,
            _ => 0.,
        };
        let angle = settings.background_angle.to_radians();
        let editor_shading = self.editor_shading;
        let editor_background = self.editor_appearance.background;
        let wire_color = if editor_background[0] + editor_background[1] + editor_background[2] > 0.6 {
            [0.025, 0.045, 0.08, 1.]
        } else { [0.35, 0.5, 0.7, 1.] };
        let make_uniform = |model: Mat4, material: &Material, grid: bool| {
            let cutoff = material.alpha_shader_mode();
            DrawUniform {
                model: model.to_cols_array_2d(),
                normal_model: model.inverse().transpose().to_cols_array_2d(),
                view_proj: view.view_projection().to_cols_array_2d(),
                eye: view.eye.extend(1.).to_array(),
                base_color: if !export && !grid && editor_shading == 0 { wire_color } else { material.base_color },
                params: [
                    material.metallic,
                    material.roughness,
                    cutoff,
                    if grid { 1. } else if !export && editor_shading == 1 { 2. } else { 0. },
                ],
                emissive: [
                    material.emissive[0].min(1e8),
                    material.emissive[1].min(1e8),
                    material.emissive[2].min(1e8),
                    0.,
                ],
                maps: [
                    material.normal_scale,
                    material.occlusion_strength,
                    if material.normal_texture.is_some() {
                        1.
                    } else {
                        0.
                    },
                    if material.normal_y_flip { 1. } else { 0. },
                ],
                surface: [
                    material.ior,
                    material.coat_weight,
                    material.coat_roughness,
                    material.transmission,
                ],
                environment: [
                    project.render_settings.exposure.exp2(),
                    project.render_settings.ambient_intensity,
                    light_count as f32,
                    match project.render_settings.tone_mapping {
                        ToneMapping::Aces => 0.,
                        ToneMapping::Reinhard => 1.,
                        ToneMapping::Linear => 2.,
                    },
                ],
                sky: [
                    project.render_settings.sky_color[0],
                    project.render_settings.sky_color[1],
                    project.render_settings.sky_color[2],
                    0.,
                ],
                ground: [
                    project.render_settings.ground_color[0],
                    project.render_settings.ground_color[1],
                    project.render_settings.ground_color[2],
                    0.,
                ],
                lights,
                transmission: [
                    material.thickness,
                    material.attenuation_distance,
                    if material.geometric_thickness { 1. } else { 0. },
                    0.,
                ],
                anisotropy: [
                    material.anisotropy,
                    material.anisotropy_rotation.to_radians().cos(),
                    material.anisotropy_rotation.to_radians().sin(),
                    0.,
                ],
                fiber: [material.sheen_weight, material.sheen_roughness, 0., 0.],
                absorption: material
                    .attenuation_color
                    .map_or([0.; 4], |c| [c[0], c[1], c[2], 1.]),
                uv_transform: [
                    material.uv_scale[0],
                    material.uv_scale[1],
                    material.uv_offset[0],
                    material.uv_offset[1],
                ],
                uv_rotation: [
                    material.uv_rotation.to_radians().cos(),
                    material.uv_rotation.to_radians().sin(),
                    0.,
                    0.,
                ],
                map_uvs: std::array::from_fn(|i| material.texture_uvs[i / 2].rows()[i % 2]),
                ambient_occlusion: [
                    settings.ambient_occlusion,
                    settings.ambient_occlusion_radius,
                    0.,
                    0.,
                ],
                inverse_view_proj: view.view_projection().inverse().to_cols_array_2d(),
                graph: if grid {
                    graph::GpuGraph::default()
                } else {
                    programs.get(&material.id).copied().unwrap_or_default()
                },
                npr: material.npr.into(),
                artistic: material.artistic.into(),
                viewport: view.viewport,
                background: [background[0], background[1], background[2], 0.],
                secondary: [
                    settings.background_secondary[0],
                    settings.background_secondary[1],
                    settings.background_secondary[2],
                    mode,
                ],
                gradient: [
                    angle.cos(),
                    angle.sin(),
                    if transparent { 1. } else { 0. },
                    refraction_msaa,
                ],
            }
        };
        self.queue.write_buffer(
            &self.floor.uniform,
            0,
            bytemuck::bytes_of(&make_uniform(
                Mat4::IDENTITY,
                &Material {
                    base_color: [0.12, 0.15, 0.18, 1.],
                    roughness: 0.9,
                    ..Material::default()
                },
                true,
            )),
        );
        let live_nodes: std::collections::HashSet<_> = project
            .nodes
            .iter()
            .filter(|n| n.mesh.is_some())
            .map(|n| n.id)
            .collect();
        self.draws.retain(|id, _| live_nodes.contains(id));
        let fallback_material = Material::default();
        for (object_index, node) in project.nodes.iter().enumerate() {
            let Some(mesh) = node.mesh.and_then(|id| project.mesh(id)) else {
                continue;
            };
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            if mesh.vertices.len() as u64 * std::mem::size_of::<GpuVertex>() as u64
                > self.device.limits().max_buffer_size
            {
                return Err("Геометрия превышает лимит GPU".into());
            }
            if mesh.indices.len() as u64 * 8 > self.device.limits().max_buffer_size {
                return Err("Каркас сетки превышает лимит буфера GPU".into());
            }
            let joint_bytes = skin_palette_bytes(
                node.skin.as_ref().map_or(0, |skin| skin.joints.len()),
                &self.device.limits(),
            )?;
            let replace = self
                .draws
                .get(&node.id)
                .map(|d| {
                    d.mesh_id != mesh.id
                        || d.index_revision != mesh.indices.revision()
                        || d.topology_revision != mesh.topology.as_ref().map(|t| t.faces.revision())
                        || d.vertex_count != mesh.vertices.len()
                        || d.joints.size() != joint_bytes
                })
                .unwrap_or(true);
            if replace {
                let vertices: Vec<_> = mesh.vertices.iter().copied().map(GpuVertex::from).collect();
                self.draws.insert(
                    node.id,
                    Self::make_draw(
                        &self.device,
                        &self.draw_layout,
                        mesh.id,
                        &vertices,
                        &mesh.indices,
                        &wire_indices((0..mesh.face_count()).filter_map(|face| mesh.face_vertices(face))),
                        joint_bytes,
                    ),
                );
            }
            let draw = self.draws.get_mut(&node.id).unwrap();
            let geometry_changed = draw.vertex_revision != mesh.vertices.revision();
            if !replace && geometry_changed && mesh.morph_targets.is_empty() {
                let vertices: Vec<_> = mesh.vertices.iter().copied().map(GpuVertex::from).collect();
                self.queue
                    .write_buffer(&draw.vertices, 0, bytemuck::cast_slice(&vertices));
                draw.bounds = shadow::MeshBounds::new(&vertices);
            }
            draw.vertex_revision = mesh.vertices.revision();
            draw.index_revision = mesh.indices.revision();
            draw.topology_revision = mesh.topology.as_ref().map(|t| t.faces.revision());
            if !mesh.morph_targets.is_empty()
                && (replace || geometry_changed || draw.morph_weights != node.morph_weights)
            {
                let mut vertices: Vec<_> =
                    mesh.vertices.iter().copied().map(GpuVertex::from).collect();
                for (target, &w) in mesh.morph_targets.iter().zip(&node.morph_weights) {
                    if w.abs() < 1e-7 {
                        continue;
                    }
                    for (i, v) in vertices.iter_mut().enumerate() {
                        for axis in 0..3 {
                            v.position[axis] += target.positions[i][axis] * w;
                            if !target.normals.is_empty() {
                                v.normal[axis] += target.normals[i][axis] * w;
                            }
                        }
                    }
                }
                self.queue
                    .write_buffer(&draw.vertices, 0, bytemuck::cast_slice(&vertices));
                draw.bounds = shadow::MeshBounds::new(&vertices);
                draw.morph_weights = node.morph_weights.clone();
            }
            let model = globals.get(&node.id).copied().unwrap_or(Mat4::IDENTITY);
            if let Some(skin) = &node.skin {
                let inv_model = model.inverse();
                let matrices: Vec<_> = skin
                    .joints
                    .iter()
                    .zip(&skin.inverse_bind_matrices)
                    .map(|(id, ibm)| {
                        (inv_model * globals.get(id).copied().unwrap_or(Mat4::IDENTITY) * *ibm)
                            .to_cols_array_2d()
                    })
                    .collect();
                self.queue
                    .write_buffer(&draw.joints, 0, bytemuck::cast_slice(&matrices));
            }
            let material = mesh
                .material
                .and_then(|id| project.material(id)).filter(|m| m.visible)
                .unwrap_or(&fallback_material);
            let mut uniform = make_uniform(model, material, false);
            if let Some(cutoff) = self.material_parts.layer_mask(node.id) { uniform.params[2] = cutoff; }
            uniform.maps[3] = if node.skin.is_some() { 1. } else { 0. };
            uniform.transmission[3] = self.material_parts.ray_object_id(node.id).unwrap_or(object_index as f32 + 1.);
            self.queue
                .write_buffer(&draw.uniform, 0, bytemuck::bytes_of(&uniform));
        }
        let forward = (view.target - view.eye).normalize();
        let right = forward.cross(view.up).normalize();
        let up = right.cross(forward).normalize();
        self.queue.write_buffer(
            &self.particle_uniform,
            0,
            bytemuck::bytes_of(&ParticleCamera {
                view_proj: view.view_projection().to_cols_array_2d(),
                right: right.extend(0.).to_array(),
                up: up.extend(0.).to_array(),
            }),
        );
        Ok(())
    }
    pub fn render(
        &mut self,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        rectangles: &[UiRect],
        texts: &[UiText],
        particles: &[Vec3],
    ) -> Result<(), String> {
        let surface = self
            .surface
            .as_ref()
            .ok_or("Headless renderer: use export_png")?;
        let frame = match surface.get_current_texture() {
            Ok(f) => { self.surface_failures=0; f },
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface_failures=self.surface_failures.saturating_add(1);
                if !surface_retry_pending(self.surface_failures) {
                    return Err("Не удалось восстановить поверхность окна после трёх попыток".into());
                }
                surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                self.surface_failures=self.surface_failures.saturating_add(1);
                return if surface_retry_pending(self.surface_failures) {Ok(())}
                    else {Err("GPU не предоставил кадр после трёх повторов; повторите действие или измените размер окна".into())};
            }
            Err(e) => { self.surface_failures=0; return Err(e.to_string()); },
        };
        let target = frame.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("editor frame"),
            });
        let timer_slot = self
            .gpu_timer
            .begin(&self.device, &self.queue, &mut encoder);
        self.render_to(
            &mut encoder,
            &target,
            self.config.width,
            self.config.height,
            project,
            globals,
            view,
            rectangles,
            texts,
            particles,
            project.render_settings.transparent_background,
            false,
        )?;
        self.gpu_timer.end(&mut encoder, timer_slot);
        self.queue.submit(Some(encoder.finish()));
        self.gpu_timer.submitted(timer_slot);
        frame.present();
        self.atlas.trim();
        Ok(())
    }
    fn render_shadows(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        visible: &[&poseforge_core::Node],
        show_floor: bool,
        export: bool,
    ) {
        let settings = project.render_settings;
        let world_bounds: BTreeMap<_, _> = visible
            .iter()
            .filter_map(|node| {
                self.draws
                    .get(&node.id)
                    .and_then(|d| d.bounds.world(node, globals))
                    .map(|b| (node.id, b))
            })
            .collect();
        let mut bounds = None;
        for node in visible {
            shadow::merge(&mut bounds, world_bounds.get(&node.id).copied());
        }
        let requested = match settings.shadow_resolution {
            512 | 1024 | 2048 => settings.shadow_resolution,
            _ => 1024,
        };
        let candidates: Vec<_> = project
            .lights
            .iter()
            .enumerate()
            .filter(|(i, _)| project.light_visible(*i, export))
            .map(|(_, light)| light)
            .enumerate()
            .filter(|(_, l)| settings.shadows && settings.shadow_strength > 0. && l.casts_shadow)
            .take(MAX_LIGHTS)
            .collect();
        let potential_layers = if bounds.is_some() {
            candidates
                .iter()
                .map(|(_, l)| shadow::face_count(l) as u32)
                .sum()
        } else {
            0
        };
        let resolution = shadow::effective_resolution(
            requested,
            potential_layers,
            self.device.limits().max_texture_dimension_2d,
        );
        let mut uniform = ShadowUniform::zeroed();
        let mut matrices = Vec::new();
        let mut light_count = 0;
        if let Some(bounds) = bounds {
            for (index, light) in candidates {
                if let Some(cameras) = shadow::light_cameras(bounds, light, resolution, show_floor)
                {
                    let light_uniform = &mut uniform.lights[index];
                    light_uniform.params = [
                        matrices.len() as f32,
                        cameras.matrices.len() as f32,
                        cameras.near,
                        cameras.far,
                    ];
                    for (face, matrix) in cameras.matrices.iter().enumerate() {
                        light_uniform.view_proj[face] = matrix.to_cols_array_2d();
                    }
                    matrices.extend(cameras.matrices);
                    light_count += 1;
                }
            }
        }
        let layers = matrices.len() as u32;
        let allocated_layers = shadow::allocation_layers(layers);
        let resolution = if layers == 0 { 1 } else { resolution };
        if self.shadow_map.resolution != resolution
            || self.shadow_map.depth.len() != allocated_layers as usize
        {
            self.shadow_map = Self::make_shadow_map(
                &self.device,
                &self.shadow_layout,
                &self.shadow_uniform,
                &self.shadow_sampler,
                resolution,
                allocated_layers,
                &self.world,
            );
        }
        self.shadow_stats = ShadowStats {
            requested_resolution: requested,
            effective_resolution: resolution,
            shadow_lights: light_count,
            layers,
            bytes: u64::from(resolution) * u64::from(resolution) * u64::from(allocated_layers) * 4,
        };
        uniform.params = [
            if layers == 0 {
                0.
            } else {
                settings.shadow_strength
            },
            settings.shadow_bias,
            1. / resolution as f32,
            settings.shadow_filter_radius,
        ];
        self.queue
            .write_buffer(&self.shadow_uniform, 0, bytemuck::bytes_of(&uniform));
        // Each encoded pass references a distinct uniform allocation. Rewriting
        // one shared buffer between encode calls would make every pass use the
        // last matrix once the command buffer reaches the GPU.
        while self.shadow_passes.len() < matrices.len() {
            let camera = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("per-face shadow camera"),
                size: std::mem::size_of::<ShadowCameraUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow camera, no depth attachment feedback"),
                layout: &self.shadow_pass_layout,
                entries: &{
                    let mut entries = vec![wgpu::BindGroupEntry {
                        binding: 3,
                        resource: camera.as_entire_binding(),
                    }];
                    entries.extend(
                        self.world
                            .bindings()
                            .into_iter()
                            .filter(|e| matches!(e.binding, 10 | 12)),
                    );
                    entries
                },
            });
            self.shadow_passes.push(ShadowPass { camera, bind });
        }
        for (layer, matrix) in matrices.iter().enumerate() {
            self.queue.write_buffer(
                &self.shadow_passes[layer].camera,
                0,
                bytemuck::bytes_of(&ShadowCameraUniform {
                    view_proj: matrix.to_cols_array_2d(),
                }),
            );
        }
        for layer in 0..allocated_layers as usize {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("light shadow depth face"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_map.depth[layer],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if layer >= matrices.len() {
                continue;
            }
            pass.set_bind_group(2, &self.shadow_passes[layer].bind, &[]);
            for node in visible {
                if world_bounds
                    .get(&node.id)
                    .is_some_and(|b| !b.intersects_clip(matrices[layer]))
                {
                    continue;
                }
                let material = node
                    .mesh
                    .and_then(|id| project.mesh(id))
                    .and_then(|mesh| mesh.material)
                    .and_then(|id| project.material(id)).filter(|m| m.visible);
                let draw = &self.draws[&node.id];
                let mirrored = globals
                    .get(&node.id)
                    .is_some_and(|model| model.determinant() < 0.);
                let pipeline = usize::from(material.is_some_and(|m| m.double_sided)) * 2
                    + usize::from(mirrored);
                pass.set_pipeline(&self.shadow_pipelines[pipeline]);
                let binding = material
                    .map(MaterialKey::from_material)
                    .and_then(|key| self.material_bindings.get(&key))
                    .unwrap_or(&self.default_material);
                pass.set_bind_group(0, &draw.bind, &[]);
                pass.set_bind_group(1, binding, &[]);
                pass.set_vertex_buffer(0, draw.vertices.slice(..));
                pass.set_index_buffer(draw.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.count, 0, 0..1);
                self.frame_stats.shadow_draws += 1;
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn render_to(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        rectangles: &[UiRect],
        texts: &[UiText],
        particles: &[Vec3],
        transparent: bool,
        export: bool,
    ) -> Result<(), String> {
        let mut aimed = std::borrow::Cow::Borrowed(project);
        if project.lights.iter().any(|l| l.target.is_some() || l.curve_binding.is_some()) {
            let lights = project.evaluated_lights(globals, 0.).map_err(|e| e.to_string())?;
            if lights != project.lights { aimed.to_mut().lights = lights; }
        }
        let project = aimed.as_ref();
        let expanded = self.material_parts.scene(project, globals)?;
        let (project, globals) = expanded
            .as_ref()
            .map_or((project, globals), |(p, g)| (p.as_ref(), g.as_ref()));
        let selected = self.material_parts.selection_parts(&self.editor_selection);
        let selection_visible = !export
            && project
                .nodes
                .iter()
                .any(|node| node.mesh.is_some() && selected.contains(&node.id));
        let ray_project = self.material_parts.ray_scene();
        self.sync_scene(project, ray_project.as_deref().unwrap_or(project), globals, view, export, transparent)?;
        let samples = self.effective_antialiasing(project.render_settings.antialiasing);
        let quality_index = usize::from(samples == 4);
        if self
            .targets
            .as_ref()
            .map(|t| (t.width, t.height, t.samples))
            != Some((width, height, samples))
        {
            let create = |format| {
                self.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("cached multisample attachment"),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: samples,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let hdr_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("linear HDR scene resolve"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SCENE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let hdr = hdr_texture.create_view(&Default::default());
            let single = |format, usage, label| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            let refraction_texture = single(
                SCENE_FORMAT,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                "refraction snapshot",
            );
            let refract_view = refraction_texture.create_view(&Default::default());
            let depth_single = single(
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                "opaque depth for refraction and fog",
            )
            .create_view(&Default::default());
            let dof_depth = single(
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                "camera depth including glass",
            )
            .create_view(&Default::default());
            // Portable sampled depth: GL cannot translate raw depth texture loads.
            // Keep the depth attachment for visibility and pack the same f32 z losslessly into universally renderable RGBA8.
            let depth_color = single(
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                "opaque sampled depth",
            )
            .create_view(&Default::default());
            let dof_color = single(
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                "complete sampled depth",
            )
            .create_view(&Default::default());
            let selection_color = single(
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                "complete selected silhouette",
            )
            .create_view(&Default::default());
            let selection_z = single(
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                "independent selected depth",
            )
            .create_view(&Default::default());
            let ui_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("edit components scene depth"),
                layout: &self.ui_pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {binding:0,resource:wgpu::BindingResource::TextureView(&dof_color)},
                    wgpu::BindGroupEntry {binding:1,resource:wgpu::BindingResource::TextureView(&self.ui_image)},
                    wgpu::BindGroupEntry {binding:2,resource:wgpu::BindingResource::Sampler(&self.ui_image_sampler)},
                ],
            });
            let refraction_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene refraction"),
                layout: &self.refraction_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&refract_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&depth_color),
                    },
                ],
            });
            let grid_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("grid mask including transparent surfaces"),
                layout: &self.refraction_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&refract_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&dof_color) },
                ],
            });
            let post_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("linear HDR resolved frame"),
                layout: &self.post_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&hdr),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.post_uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&depth_color),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&dof_color),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&selection_color),
                    },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.fx_masks.view) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.fx_masks.density_view) },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&self.fx_masks.sampler) },
                ],
            });
            self.targets = Some(FrameTargets {
                width,
                height,
                samples,
                depth: create(wgpu::TextureFormat::Depth32Float),
                color: (samples > 1).then(|| create(SCENE_FORMAT)),
                hdr,
                hdr_texture,
                refraction_texture,
                refraction_bind,
                grid_bind,
                depth_single,
                dof_depth,
                depth_color,
                dof_color,
                selection_color,
                selection_z,
                ui_bind,
                post_bind,
            });
        }
        let targets = self.targets.as_ref().unwrap();
        let depth_view = targets.depth.clone();
        let color_view = targets.color.clone();
        let hdr_view = targets.hdr.clone();
        let post_bind = targets.post_bind.clone();
        let settings = project.render_settings;
        let mut atmosphere = settings.atmosphere;
        let volume_lights = pack_volume_lights(project, export);
        let (mut fx_volumes,mut fx_count)=fx::pack_scene(project,globals,export,self.effect_time,&self.fx_state,&self.fx_gas);
        fx::append_destruction_dust(project,globals,export,self.effect_time,&self.destruction_times,&mut fx_volumes,&mut fx_count);
        if !volume_lights.iter().any(|v| v[3] > 0.) && !self.world.has_emissive_volume() { atmosphere.density = 0.; }
        let (mut lights, mut light_count) = pack_lights(project, export)?;
        fx::append_scene_lights(project,globals,export,self.effect_time,&mut lights,&mut light_count,&self.fx_state,&self.fx_gas);
        let mode = match settings.background_mode {
            poseforge_core::BackgroundMode::Linear => 2.,
            poseforge_core::BackgroundMode::Radial => 3.,
            _ => 0.,
        };
        let angle = settings.background_angle.to_radians();
        let background = self.editor_appearance.background(
            export || settings.background_mode != poseforge_core::BackgroundMode::Theme,
            transparent && export,
            project.render_settings.background_color,
        );
        self.queue.write_buffer(
            &self.post_uniform,
            0,
            bytemuck::bytes_of(&PostUniform {
                params: [
                    project.render_settings.exposure.exp2(),
                    match project.render_settings.tone_mapping {
                        ToneMapping::Aces => 0.,
                        ToneMapping::Reinhard => 1.,
                        ToneMapping::Linear => 2.,
                    },
                    if !transparent {
                        0.
                    } else if export {
                        1.
                    } else {
                        2.
                    },
                    if self.config.format.is_srgb() { 0. } else { 1. },
                ],
                background: [
                    background.r as f32,
                    background.g as f32,
                    background.b as f32,
                    background.a as f32,
                ],
                viewport: view.viewport,
                secondary: [
                    settings.background_secondary[0],
                    settings.background_secondary[1],
                    settings.background_secondary[2],
                    mode,
                ],
                gradient: [angle.cos(), angle.sin(), 0., 0.],
                inverse_view_proj: view.view_projection().inverse().to_cols_array_2d(),
                eye: view.eye.extend(1.).to_array(),
                volume: [
                    atmosphere.density,
                    atmosphere.height,
                    atmosphere.distance,
                    atmosphere.anisotropy,
                ],
                volume_color: [
                    atmosphere.color[0],
                    atmosphere.color[1],
                    atmosphere.color[2],
                    0.,
                ],
                volume_lights,
                light_meta: [
                    light_count as f32,
                    if export { 64. } else { 48. },
                    if view.orthographic.is_some() { 1. } else { 0. },
                    0.,
                ],
                dof: [
                    view.depth_of_field.focus_distance,
                    if view.orthographic.is_some() {
                        0.
                    } else {
                        view.depth_of_field
                            .pixel_coefficient(view.fov_y, view.viewport[3])
                    },
                    0.,
                    0.,
                ],
                camera_grade: view.effects.uniforms().0,
                camera_lens: view.effects.uniforms().1,
                fx_meta: [fx_count as f32, fx::ray_steps(project,export,self.interactive_preview,self.effect_time) as f32,fx::shadow_quality(project,!export && self.interactive_preview),
                    (0..light_count).max_by(|&a,&b|lights[a].color_intensity[3].total_cmp(&lights[b].color_intensity[3])).unwrap_or(0) as f32],
                fx: fx_volumes,
                contour: [
                    settings.npr_contours.width,
                    settings.npr_contours.depth_threshold,
                    settings.npr_contours.normal_angle.to_radians().cos(),
                    0.,
                ],
                contour_color: [
                    settings.npr_contours.color[0],
                    settings.npr_contours.color[1],
                    settings.npr_contours.color[2],
                    0.,
                ],
                selection: [
                    self.selection_accent[0],
                    self.selection_accent[1],
                    self.selection_accent[2],
                    if selection_visible {
                        2. * self.editor_scale
                    } else {
                        0.
                    },
                ],
                forward: (view.target - view.eye).normalize().extend(0.).to_array(),
                lights,
                checker: if self.editor_appearance.background[0] > 0.3 {
                    [12. * self.editor_scale, 0.55, 0.72, 0.]
                } else {
                    [12. * self.editor_scale, 0.045, 0.085, 0.]
                },
            }),
        );
        let particle_data: Vec<[f32; 3]> = particles.iter().map(|p| p.to_array()).collect();
        let particle_buffer = (!particles.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("fluid positions"),
                    contents: bytemuck::cast_slice(&particle_data),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        self.frame_stats = FrameStats::default();
        let visible_ids = project.visible_node_ids(export);
        let mut visible: Vec<_> = project
            .nodes
            .iter()
            .filter(|n| visible_ids.contains(&n.id))
            .filter(|n| self.draws.contains_key(&n.id))
            .collect();
        visible.sort_by(|a, b| {
            let blend = |n: &&poseforge_core::Node| {
                n.mesh
                    .and_then(|id| project.mesh(id))
                    .and_then(|m| m.material)
                    .and_then(|id| project.material(id)).filter(|m| m.visible)
                    .map(|m| m.is_transparent())
                    .unwrap_or(false)
            };
            blend(a).cmp(&blend(b)).then_with(|| {
                let da = globals
                    .get(&a.id)
                    .unwrap_or(&Mat4::IDENTITY)
                    .w_axis
                    .truncate()
                    .distance_squared(view.eye);
                let db = globals
                    .get(&b.id)
                    .unwrap_or(&Mat4::IDENTITY)
                    .w_axis
                    .truncate()
                    .distance_squared(view.eye);
                db.total_cmp(&da)
            })
        });
        self.render_shadows(
            encoder, project, globals, &visible,
            false, // The helper grid must never change shadow bounds or receive shadows.
            export,
        );
        // Shadow-only objects still contributed to shadow bounds and maps above.
        // Exclude them from camera depth, transmission, outline and color passes.
        visible.retain(|node| !node.shadow_only);
        self.frame_stats.candidates = visible.len();
        let clip = view.view_projection();
        visible.retain(|node| {
            self.draws
                .get(&node.id)
                .and_then(|d| d.bounds.world(node, globals))
                .is_none_or(|b| b.intersects_clip(clip))
        });
        self.frame_stats.culled = self.frame_stats.candidates - visible.len();
        let transmission = visible.iter().any(|node| {
            node.mesh
                .and_then(|id| project.mesh(id))
                .and_then(|m| m.material)
                .and_then(|id| project.material(id)).filter(|m| m.visible)
                .is_some_and(|m| m.uses_transmission())
        });
        let grid_visible = self.editor_appearance.grid_visible(export, transparent);
        let needs_depth = settings.npr_contours.width > 0.
            || transmission
            || atmosphere.density > 0.
            || fx_count > 0
            || project.render_settings.ambient_occlusion > 0.;
        let transparent_objects = visible.iter().any(|n| {
            n.mesh
                .and_then(|id| project.mesh(id))
                .and_then(|m| m.material)
                .and_then(|id| project.material(id)).filter(|m| m.visible)
                .is_some_and(|m| m.is_transparent())
        });
        for (needed, for_dof) in [
            (needs_depth, false),
            (
                (view.depth_of_field.enabled && view.orthographic.is_none())
                    || rectangles.iter().any(|r| r.depth.is_some())
                    || selection_visible || grid_visible,
                true,
            ),
        ] {
            if !needed {
                continue;
            }
            self.frame_stats.depth_prepass = true;
            let targets = self.targets.as_ref().unwrap();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("portable depth prepass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if for_dof {
                        &targets.dof_color
                    } else {
                        &targets.depth_color
                    },
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.,
                            g: 0.,
                            b: 128. / 255.,
                            a: 63. / 255.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: if for_dof {
                        &targets.dof_depth
                    } else {
                        &targets.depth_single
                    },
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let [x, y, w, h] = view.viewport;
            pass.set_viewport(x, y, w, h, 0., 1.);
            pass.set_bind_group(2, &self.shadow_map.bind, &[]);
            for node in &visible {
                let material = node
                    .mesh
                    .and_then(|id| project.mesh(id))
                    .and_then(|m| m.material)
                    .and_then(|id| project.material(id)).filter(|m| m.visible);
                if !for_dof && material.is_some_and(|m| m.is_transparent()) {
                    continue;
                }
                let draw = &self.draws[&node.id];
                let mirrored = globals.get(&node.id).is_some_and(|m| m.determinant() < 0.);
                let index = usize::from(material.is_some_and(|m| m.double_sided)) * 2
                    + usize::from(mirrored);
                pass.set_pipeline(&self.depth_pipelines[index]);
                let binding = material
                    .map(MaterialKey::from_material)
                    .and_then(|k| self.material_bindings.get(&k))
                    .unwrap_or(&self.default_material);
                pass.set_bind_group(0, &draw.bind, &[]);
                pass.set_bind_group(1, binding, &[]);
                pass.set_vertex_buffer(0, draw.vertices.slice(..));
                pass.set_index_buffer(draw.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.count, 0, 0..1);
                self.frame_stats.depth_draws += 1;
            }
        }
        if selection_visible {
            let targets = self.targets.as_ref().unwrap();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("complete object selection without occluder cut edges"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.selection_color,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.,
                            g: 0.,
                            b: 128. / 255.,
                            a: 63. / 255.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.selection_z,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let [x, y, w, h] = view.viewport;
            pass.set_viewport(x, y, w, h, 0., 1.);
            pass.set_bind_group(2, &self.shadow_map.bind, &[]);
            for node in visible.iter().filter(|n| selected.contains(&n.id)) {
                let material = node
                    .mesh
                    .and_then(|id| project.mesh(id))
                    .and_then(|m| m.material)
                    .and_then(|id| project.material(id)).filter(|m| m.visible);
                let draw = &self.draws[&node.id];
                let mirrored = globals.get(&node.id).is_some_and(|m| m.determinant() < 0.);
                let index = usize::from(material.is_some_and(|m| m.double_sided)) * 2
                    + usize::from(mirrored);
                pass.set_pipeline(&self.depth_pipelines[index]);
                let binding = material
                    .map(MaterialKey::from_material)
                    .and_then(|k| self.material_bindings.get(&k))
                    .unwrap_or(&self.default_material);
                pass.set_bind_group(0, &draw.bind, &[]);
                pass.set_bind_group(1, binding, &[]);
                pass.set_vertex_buffer(0, draw.vertices.slice(..));
                pass.set_index_buffer(draw.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..draw.count, 0, 0..1);
                self.frame_stats.depth_draws += 1;
            }
        }
        for transparent_pass in [false, true] {
            if transparent_pass && !transparent_objects {
                continue;
            }
            let targets = self.targets.as_ref().unwrap();
            if transparent_pass && transmission {
                self.frame_stats.refraction_copy = true;
                encoder.copy_texture_to_texture(
                    targets.hdr_texture.as_image_copy(),
                    targets.refraction_texture.as_image_copy(),
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref().unwrap_or(&hdr_view),
                    resolve_target: (samples > 1).then_some(&hdr_view),
                    ops: wgpu::Operations {
                        load: if transparent_pass {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: if transparent_pass {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(1.)
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let [x, y, w, h] = view.viewport;
            let x = x.clamp(0., width.saturating_sub(1) as f32);
            let y = y.clamp(0., height.saturating_sub(1) as f32);
            let w = w.max(1.).min(width as f32 - x);
            let h = h.max(1.).min(height as f32 - y);
            pass.set_viewport(x, y, w, h, 0., 1.);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            pass.set_pipeline(&self.scene_pipelines[quality_index][0]);
            pass.set_bind_group(2, &self.shadow_map.bind, &[]);
            pass.set_bind_group(3, &targets.refraction_bind, &[]);
            if let Some(buffer) = particle_buffer.as_ref().filter(|_| !transparent_pass) {
                pass.set_pipeline(&self.particle_pipelines[quality_index]);
                pass.set_bind_group(0, &self.particle_bind, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..particles.len() as u32);
            }
            for node in &visible {
                let draw = &self.draws[&node.id];
                let material = node
                    .mesh
                    .and_then(|id| project.mesh(id))
                    .and_then(|m| m.material)
                    .and_then(|id| project.material(id)).filter(|m| m.visible);
                if material.is_some_and(|m| m.is_transparent()) != transparent_pass {
                    continue;
                }
                let mirrored = globals
                    .get(&node.id)
                    .is_some_and(|model| model.determinant() < 0.);
                let pipeline = scene_pipeline_index(
                    material.is_some_and(|m| m.is_transparent()),
                    material.is_some_and(|m| m.double_sided),
                    mirrored,
                );
                let wire = !export && self.editor_shading == 0;
                pass.set_pipeline(if wire { &self.wire_pipelines[quality_index] }
                    else { &self.scene_pipelines[quality_index][pipeline] });
                let binding = material
                    .map(MaterialKey::from_material)
                    .and_then(|key| self.material_bindings.get(&key))
                    .unwrap_or(&self.default_material);
                pass.set_bind_group(0, &draw.bind, &[]);
                pass.set_bind_group(1, binding, &[]);
                pass.set_vertex_buffer(0, draw.vertices.slice(..));
                pass.set_index_buffer(if wire { draw.wire_indices.slice(..) } else { draw.indices.slice(..) }, wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..if wire { draw.wire_count } else { draw.count }, 0, 0..1);
                self.frame_stats.scene_draws += 1;
            }
        }
        if grid_visible {
            let targets = self.targets.as_ref().unwrap();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("editor grid after physical captures"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.hdr,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.dof_depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None, occlusion_query_set: None,
            });
            let [x, y, w, h] = view.viewport;
            let x = x.clamp(0., width.saturating_sub(1) as f32);
            let y = y.clamp(0., height.saturating_sub(1) as f32);
            let w = w.max(1.).min(width as f32 - x);
            let h = h.max(1.).min(height as f32 - y);
            pass.set_viewport(x, y, w, h, 0., 1.);
            pass.set_scissor_rect(x as u32, y as u32, w as u32, h as u32);
            pass.set_pipeline(&self.scene_pipelines[0][6]);
            pass.set_bind_group(0, &self.floor.bind, &[]);
            pass.set_bind_group(1, &self.default_material, &[]);
            pass.set_bind_group(2, &self.shadow_map.bind, &[]);
            pass.set_bind_group(3, &targets.grid_bind, &[]);
            pass.set_vertex_buffer(0, self.floor.vertices.slice(..));
            pass.set_index_buffer(self.floor.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.floor.count, 0, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("display transform"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.post_pipeline);
            pass.set_bind_group(0, &post_bind, &[]);
            pass.set_bind_group(1, &self.shadow_map.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        if rectangles.is_empty() && texts.is_empty() {
            return Ok(());
        }
        let ui: Vec<_> = rectangles
            .iter()
            .map(|r| UiVertex::from_rect(r, width as f32, height as f32))
            .collect();
        self.frame_stats.ui_rectangles = ui.len();
        let bytes = bytemuck::cast_slice(&ui);
        self.frame_stats.ui_upload_bytes = bytes.len();
        if !bytes.is_empty() {
            if self
                .ui_buffer
                .as_ref()
                .is_none_or(|(_, capacity)| *capacity < bytes.len())
            {
                let capacity = bytes.len().max(1024).next_power_of_two();
                self.ui_buffer = Some((
                    self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Reusable UI instances"),
                        size: capacity as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    capacity,
                ));
                self.frame_stats.ui_buffer_allocations = 1;
            }
            self.queue
                .write_buffer(&self.ui_buffer.as_ref().unwrap().0, 0, bytes);
        }
        let (keys, shaped) = self.text_cache.prepare(&mut self.font_system, texts, width);
        self.frame_stats.ui_text_layouts = shaped;
        self.text_viewport
            .update(&self.queue, Resolution { width, height });
        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.text_viewport,
                texts.iter().zip(&keys).map(|(text, key)| TextArea {
                    buffer: self.text_cache.get(key),
                    left: text.x,
                    top: text.y,
                    scale: 1.,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width as i32,
                        bottom: height as i32,
                    },
                    default_color: Color::rgba(
                        (text.color[0] * 255.) as u8,
                        (text.color[1] * 255.) as u8,
                        (text.color[2] * 255.) as u8,
                        (text.color[3] * 255.) as u8,
                    ),
                    custom_glyphs: &[],
                }),
                &mut self.swash,
            )
            .map_err(|e| e.to_string())?;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("custom interface overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if !ui.is_empty() {
            let (buffer, _) = self.ui_buffer.as_ref().unwrap();
            pass.set_pipeline(&self.ui_pipeline);
            pass.set_bind_group(0, &self.targets.as_ref().unwrap().ui_bind, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..6, 0..ui.len() as u32);
        }
        self.text_renderer
            .render(&self.atlas, &self.text_viewport, &mut pass)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn export_png(
        &mut self,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        particles: &[Vec3],
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
        transparent: bool,
    ) -> Result<(), String> {
        self.capture_png(
            project,
            globals,
            view,
            &[],
            &[],
            particles,
            path.as_ref(),
            width,
            height,
            transparent,
            true,
        )
    }
    /// Capture the exact editor scene/UI render path, including glyphon text.
    /// The checkerboard is opaque display feedback, never exported artwork data.
    #[allow(clippy::too_many_arguments)]
    pub fn capture_viewport_png(
        &mut self,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        rectangles: &[UiRect],
        texts: &[UiText],
        particles: &[Vec3],
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.capture_png(
            project,
            globals,
            view,
            rectangles,
            texts,
            particles,
            path.as_ref(),
            width,
            height,
            project.render_settings.transparent_background,
            false,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn capture_png(
        &mut self,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        rectangles: &[UiRect],
        texts: &[UiText],
        particles: &[Vec3],
        path: &Path,
        width: u32,
        height: u32,
        transparent: bool,
        export: bool,
    ) -> Result<(), String> {
        let rgba = self.capture_pixels(
            project,
            globals,
            view,
            rectangles,
            texts,
            particles,
            width,
            height,
            transparent,
            export,
        )?;
        image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8)
            .map_err(|e| e.to_string())
    }
    /// Read the same artwork pipeline into sRGB RGBA pixels without a temporary file.
    /// This is used by the material-sphere preview worker.
    pub fn render_rgba(
        &mut self,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        self.capture_pixels(
            project,
            globals,
            view,
            &[],
            &[],
            &[],
            width,
            height,
            false,
            true,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn capture_pixels(
        &mut self,
        project: &Project,
        globals: &BTreeMap<u64, Mat4>,
        view: &View,
        rectangles: &[UiRect],
        texts: &[UiText],
        particles: &[Vec3],
        width: u32,
        height: u32,
        transparent: bool,
        export: bool,
    ) -> Result<Vec<u8>, String> {
        let max = self.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(format!("Размер изображения должен быть от 1 до {max}"));
        }
        let row_bytes = width * 4;
        let padded = row_bytes.div_ceil(256) * 256;
        let buffer_size = padded as u64 * height as u64;
        if buffer_size > self.device.limits().max_buffer_size {
            return Err("Изображение превышает лимит буфера GPU".into());
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("final image"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PNG readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("export PNG"),
            });
        let mut view = view.clone();
        if export {
            view.viewport = [0., 0., width as f32, height as f32];
        }
        self.render_to(
            &mut encoder,
            &target,
            width,
            height,
            project,
            globals,
            &view,
            rectangles,
            texts,
            particles,
            transparent,
            export,
        )?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        output
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        if let Err(error) = readback::wait_for_mapping(&rx, &self.fx_cancel, readback::TIMEOUT, || {
            self.device.poll(wgpu::Maintain::Poll);
        }) {
            output.unmap();
            return Err(error);
        }
        let mapped = output.slice(..).get_mapped_range();
        let mut rgba = Vec::with_capacity((row_bytes * height) as usize);
        for row in mapped.chunks(padded as usize) {
            rgba.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        output.unmap();
        if matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for p in rgba.chunks_mut(4) {
                p.swap(0, 2);
            }
        }
        // Alpha blending produces premultiplied RGB over a clear background; PNG stores straight alpha.
        if transparent && export {
            for p in rgba.chunks_mut(4) {
                if p[3] > 0 && p[3] < 255 {
                    let a = p[3] as f32 / 255.;
                    for c in &mut p[..3] {
                        let s = *c as f32 / 255.;
                        let linear = if s <= 0.04045 {
                            s / 12.92
                        } else {
                            ((s + 0.055) / 1.055).powf(2.4)
                        };
                        let l = (linear / a).clamp(0., 1.);
                        *c = (255.
                            * if l <= 0.0031308 {
                                12.92 * l
                            } else {
                                1.055 * l.powf(1. / 2.4) - 0.055
                            })
                        .round() as u8;
                    }
                }
            }
        }
        Ok(rgba)
    }
}

#[cfg(test)]
mod appearance_tests {
    use super::*;

    #[test]
    fn gpu_uniform_layout_matches_wgsl_vec4_and_light_array_strides() {
        assert_eq!(MAX_LIGHTS, poseforge_core::MAX_ENABLED_LIGHTS);
        assert_eq!(std::mem::size_of::<GpuLight>(), 80);
        assert_eq!(std::mem::offset_of!(DrawUniform, eye), 192);
        assert_eq!(std::mem::offset_of!(DrawUniform, environment), 288);
        assert_eq!(std::mem::offset_of!(DrawUniform, lights), 336);
        assert_eq!(std::mem::offset_of!(DrawUniform, anisotropy), 1056);
        assert_eq!(std::mem::offset_of!(DrawUniform, map_uvs), 1136);
        assert_eq!(std::mem::offset_of!(DrawUniform, artistic), 3616);
        assert_eq!(std::mem::size_of::<DrawUniform>(), 3680);
        assert_eq!(std::mem::offset_of!(ShadowUniform, params), 3200);
        assert_eq!(std::mem::size_of::<ShadowUniform>(), 3216);
    }

    #[test]
    fn lights_have_explicit_empty_disabled_and_overflow_behavior() {
        let mut project = Project::default();
        project.lights.clear();
        assert_eq!(pack_lights(&project, false).unwrap().1, 0);
        let disabled = poseforge_core::Light {
            enabled: false,
            ..Default::default()
        };
        project.lights = vec![disabled; 12];
        assert_eq!(pack_lights(&project, false).unwrap().1, 0);
        let spot = poseforge_core::Light {
            kind: LightKind::Spot,
            position: Vec3::new(1., 2., 3.),
            direction: Vec3::new(0., -2., 0.),
            range: 5.,
            inner_cone_angle: 0.2,
            outer_cone_angle: 0.5,
            ..poseforge_core::Light::default()
        };
        project.lights.push(spot);
        let (lights, count) = pack_lights(&project, false).unwrap();
        assert_eq!(count, 1);
        assert_eq!(lights[0].position_kind, [1., 2., 3., 2.]);
        assert_eq!(lights[0].direction_range, [0., -1., 0., 5.]);
        assert!((lights[0].cone[0] - 0.2f32.cos()).abs() < 1e-6);
        assert!((lights[0].cone[1] - 0.5f32.cos()).abs() < 1e-6);
        project
            .lights
            .extend(vec![poseforge_core::Light::default(); 8]);
        assert!(
            pack_lights(&project, false).is_err(),
            "never silently discard an enabled light"
        );
    }

    #[test]
    fn folder_light_visibility_uses_the_same_packing_in_viewport_and_export() {
        let mut p = Project::default();
        p.collections.push(poseforge_core::Collection {
            id: 10,
            light_members: vec![0],
            visible: false,
            ..Default::default()
        });
        assert_eq!(pack_lights(&p, false).unwrap().1, 0);
        assert_eq!(pack_lights(&p, true).unwrap().1, 1);
        p.collections[0].render_visible = false;
        assert_eq!(pack_lights(&p, true).unwrap().1, 0);
    }

    #[test]
    fn editor_preferences_never_change_export_background_or_floor() {
        let appearance = EditorAppearance::new([0.7, 0.8, 0.9, 1.], false);
        let defaults = EditorAppearance::default();
        assert_eq!(
            appearance.background(true, false, [0.032, 0.043, 0.055]),
            defaults.background(true, false, [0.032, 0.043, 0.055])
        );
        assert!(!appearance.grid_visible(true, false));
        assert!(!appearance.grid_visible(false, false));
        assert_ne!(
            appearance.background(false, false, [0.032, 0.043, 0.055]),
            defaults.background(false, false, [0.032, 0.043, 0.055])
        );
        let project_background = [0.1, 0.2, 0.3];
        let export_color = appearance.background(true, false, project_background);
        assert_eq!(export_color.r, project_background[0] as f64);
        assert_eq!(export_color.g, project_background[1] as f64);
        assert_eq!(export_color.b, project_background[2] as f64);
        assert_eq!(
            appearance.background(false, false, project_background),
            appearance.background(false, false, [0.; 3])
        );
    }

    #[test]
    fn transparent_exports_have_no_background_or_floor() {
        let appearance = EditorAppearance::new([0.7, 0.8, 0.9, 1.], true);
        assert_eq!(
            appearance.background(true, true, [0.032, 0.043, 0.055]),
            wgpu::Color::TRANSPARENT
        );
        assert!(!appearance.grid_visible(true, true));
    }

    #[test]
    fn invalid_editor_colors_are_finite_and_opaque() {
        let appearance = EditorAppearance::new([f32::NAN, -1., 3., 0.], true);
        assert_eq!(appearance.background, [EXPORT_BACKGROUND[0], 0., 1., 1.]);
    }
}

#[cfg(test)]
mod viewport33_tests {
    use super::*;
    #[test]
    fn post_uniform_preserves_wgsl_alignment_with_per_source_volume_colors() {
        assert_eq!(std::mem::offset_of!(PostUniform, volume_lights), 208);
        assert_eq!(std::mem::offset_of!(PostUniform, light_meta), 336);
        assert_eq!(std::mem::offset_of!(PostUniform, lights), 352);
        assert_eq!(std::mem::offset_of!(PostUniform, camera_grade), 1072);
        assert_eq!(std::mem::offset_of!(PostUniform, camera_lens), 1088);
        assert_eq!(std::mem::offset_of!(PostUniform, fx_meta), 1104);
        assert_eq!(std::mem::offset_of!(PostUniform, fx), 1120);
        assert_eq!(std::mem::size_of::<fx::GpuFx>(), 1760);
        assert_eq!(std::mem::size_of::<PostUniform>(), 15200);
        assert!(std::mem::size_of::<PostUniform>()<=16384);
    }
    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn volume_slots_follow_the_same_visibility_filter_as_lighting_and_shadows() {
        let mut p = Project::default();
        p.lights = vec![poseforge_core::Light::default(); 3];
        p.lights[0].enabled = false;
        p.lights[1].volume = poseforge_core::VolumeLight { enabled: true, strength: 2., tint: [1., 0., 0.] };
        p.lights[2].volume = poseforge_core::VolumeLight { enabled: true, strength: 0.5, tint: [0., 0., 1.] };
        assert_eq!(pack_lights(&p, false).unwrap().1, 2);
        let packed = pack_volume_lights(&p, false);
        assert_eq!(packed[0], [1., 0., 0., 2.]);
        assert_eq!(packed[1], [0., 0., 1., 0.5]);
        assert_eq!(packed[2], [0.; 4]);
        p.lights[1].volume.enabled = false;
        assert_eq!(pack_volume_lights(&p, false)[0][3], 0.);
        assert_eq!(pack_lights(&p, false).unwrap().1, 2);
    }
    #[test]
    fn wire_edges_are_unique_and_ignore_degenerate_edges() {
        let edges = wire_indices([0, 1, 2, 0, 2, 3, 3, 3, 3].chunks_exact(3));
        assert_eq!(edges, [0, 1, 0, 2, 0, 3, 1, 2, 2, 3]);
    }
    #[test]
    fn polygon_wireframe_never_exposes_render_diagonals() {
        let quad = [0, 1, 2, 3];
        let edges = wire_indices(std::iter::once(quad.as_slice()));
        assert_eq!(edges, [0, 1, 0, 3, 1, 2, 2, 3]);
        let ngon = [0, 1, 2, 3, 4];
        assert_eq!(wire_indices(std::iter::once(ngon.as_slice())).len(), 10);
    }
}
