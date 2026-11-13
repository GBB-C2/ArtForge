use crate::animation::{Clip, EvaluatedScene};
use crate::humanoid::RigProfile;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub type NodeId = u64;
pub type AssetId = u64;
pub const SCHEMA_VERSION: u32 = 43;
/// Every enabled light is evaluated by the realtime renderer. Disabled lights
/// remain in the document without consuming a GPU light slot.
pub const MAX_ENABLED_LIGHTS: usize = 8;
/// Full u16 joint-index range used by glTF. GPU palettes are sized per skin.
pub const MAX_SKIN_JOINTS: usize = 65_536;
/// Bound expanded palettes across mesh instances (64 MiB of matrix data).
pub const MAX_SKIN_MATRICES: usize = 1_048_576;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Недопустимые данные: {0}")]
    Invalid(String),
    #[error("Объект {0} не найден")]
    MissingNode(NodeId),
    #[error("Обнаружен цикл иерархии у объекта {0}")]
    HierarchyCycle(NodeId),
    #[error("Есть незаписанная поза: поставьте ключ, отбросьте изменение или останьтесь на текущем времени")]
    UncommittedPose,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}
impl Transform {
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::default()
        }
    }
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
    pub fn from_matrix(matrix: Mat4) -> Self {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Self {
            translation,
            rotation: rotation.normalize(),
            scale,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.translation.is_finite()
            && self.rotation.is_finite()
            && self.scale.is_finite()
            && (self.rotation.length_squared() - 1.0).abs() < 0.01
            && self.scale.abs().min_element() > 1e-7
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    #[serde(default)]
    pub empty: bool,
    #[serde(default)]
    pub empty_shape: crate::EmptyShape,
    #[serde(default = "crate::default_empty_size")]
    pub empty_size: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_field: Option<crate::ForceFieldSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx: Option<crate::FxSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<crate::TextSettings>,
    #[serde(default, skip_serializing_if = "crate::CollisionTargets::is_legacy")]
    pub collision_targets: crate::CollisionTargets,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_binding: Option<crate::CurveBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<crate::CurveData>,
    /// Bottom-to-top surface overlays, owned by this instance. The mesh's face materials remain the foundation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_layers: Vec<crate::MaterialLayer>,
    /// A liquid domain uses this node's local mesh bounds as its container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluid: Option<FluidDomainSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destruction: Option<crate::DestructionSettings>,
    #[serde(default)]
    pub physics: Option<crate::PhysicsSettings>,
    pub id: NodeId,
    pub name: String,
    pub parent: Option<NodeId>,
    pub transform: Transform,
    #[serde(default)]
    pub rest_transform: Option<Transform>,
    pub mesh: Option<AssetId>,
    /// A scene camera shares the node's stable ID, hierarchy and transform.
    #[serde(default)]
    pub camera: Option<crate::camera::CameraLens>,
    pub skin: Option<Skin>,
    #[serde(default)]
    pub morph_weights: Vec<f32>,
    pub visible: bool,
    #[serde(default = "yes")]
    pub render_visible: bool,
    /// Keep this mesh in shadow passes while hiding its surface from view and secondary rays.
    #[serde(default)]
    pub shadow_only: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub is_joint: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone_display: Option<crate::BoneDisplay>,
    /// Optional local endpoint for display and automatic weights of authored bones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone_tip: Option<Vec3>,
}
fn yes() -> bool {
    true
}
impl Node {
    pub fn new(id: NodeId, name: impl Into<String>) -> Self {
        Self {
            curve_binding: None,
            empty: false,
            empty_shape: Default::default(), empty_size: 1., force_field: None, fx: None,
            text: None,
            collision_targets: Default::default(),
            id,
            curve: None,
            name: name.into(),
            parent: None,
            transform: Transform::default(),
            rest_transform: None,
            material_layers: Vec::new(),
            mesh: None,
            camera: None,
            physics: None,
            fluid: None,
            destruction: None,
            skin: None,
            morph_weights: vec![],
            visible: true,
            render_visible: true,
            shadow_only: false,
            locked: false,
            is_joint: false,
            bone_display: None,
            bone_tip: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skin {
    pub joints: Vec<NodeId>,
    pub inverse_bind_matrices: Vec<Mat4>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Linear RGBA multiplier (glTF COLOR_0); white for legacy/uncolored meshes.
    #[serde(default = "default_vertex_color")]
    pub color: [f32; 4],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}
fn default_vertex_color() -> [f32; 4] {
    [1.; 4]
}
impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            normal: [0., 1., 0.],
            uv: [0.; 2],
            color: default_vertex_color(),
            joints: [0; 4],
            weights: [0.; 4],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MorphTarget {
    pub name: String,
    pub positions: crate::SharedBuffer<[f32; 3]>,
    pub normals: crate::SharedBuffer<[f32; 3]>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mesh {
    pub id: AssetId,
    pub name: String,
    pub vertices: crate::SharedBuffer<Vertex>,
    pub indices: crate::SharedBuffer<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<crate::MeshTopology>,
    pub material: Option<AssetId>,
    #[serde(default)]
    pub morph_targets: Vec<MorphTarget>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Material {
    /// Legacy resources remain in the library; newly created object layers opt in explicitly.
    #[serde(default = "yes")]
    pub in_library: bool,
    /// Hides every surface using this shared material, including shadows and reflections.
    #[serde(default = "yes")]
    pub visible: bool,
    /// Disabled by default, preserving the rendering of older projects.
    #[serde(default)]
    pub npr: crate::NprSettings,
    /// Opt-in: old projects keep their exact PBR/NPR response.
    #[serde(default)]
    pub artistic: crate::ArtisticSettings,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub graph: Option<crate::MaterialGraph>,
    /// Shared UV0 mapping: offset + rotation * (uv * scale), in every render pass.
    #[serde(default = "default_uv_scale")]
    pub uv_scale: [f32; 2],
    #[serde(default)]
    pub uv_offset: [f32; 2],
    #[serde(default)]
    pub uv_rotation: f32,
    /// Imported per-map UV mapping after the shared editor transform:
    /// base color, metallic/roughness, normal, occlusion, emission.
    #[serde(default)]
    pub texture_uvs: [crate::TextureUvTransform; 5],
    /// Same slot order as texture_uvs; independent even for a shared image.
    #[serde(default)]
    pub texture_samplers: [crate::TextureSampler; 5],
    /// Tangent-space GGX elongation. UV0 defines the direction when available.
    #[serde(default)]
    pub anisotropy: f32,
    /// Rotation in degrees around the shading normal.
    #[serde(default)]
    pub anisotropy_rotation: f32,
    #[serde(default)]
    pub sheen_weight: f32,
    #[serde(default = "default_sheen_roughness")]
    pub sheen_roughness: f32,
    /// None preserves the legacy absorption from the sampled base color.
    #[serde(default)]
    pub attenuation_color: Option<[f32; 3]>,
    #[serde(default)]
    pub transmission: f32,
    #[serde(default = "one")]
    pub thickness: f32,
    /// Measure absorption along closed mesh volumes; open surfaces use thickness.
    #[serde(default)]
    pub geometric_thickness: bool,
    #[serde(default = "one")]
    pub attenuation_distance: f32,
    pub id: AssetId,
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// Dielectric index of refraction. Conductors use metallic base-color F0.
    #[serde(default = "default_ior")]
    pub ior: f32,
    /// Weight of the separate dielectric top coat; zero preserves the base layer.
    #[serde(default)]
    pub coat_weight: f32,
    #[serde(default = "default_coat_roughness")]
    pub coat_roughness: f32,
    pub emissive: [f32; 3],
    /// Scatter this material's emitted radiance in the shared atmosphere.
    #[serde(default)]
    pub emission_volume: bool,
    pub base_color_texture: Option<AssetId>,
    /// PBR maps use TEXCOORD_0. Color/emissive images are sampled as sRGB;
    /// metallic-roughness (G/B), normal and occlusion (R) maps are linear.
    #[serde(default)]
    pub metallic_roughness_texture: Option<AssetId>,
    #[serde(default)]
    pub normal_texture: Option<AssetId>,
    #[serde(default = "one")]
    pub normal_scale: f32,
    #[serde(default)]
    pub normal_y_flip: bool,
    #[serde(default)]
    pub occlusion_texture: Option<AssetId>,
    #[serde(default = "one")]
    pub occlusion_strength: f32,
    #[serde(default)]
    pub emissive_texture: Option<AssetId>,
    #[serde(default)]
    pub alpha_mode: AlphaMode,
    #[serde(default = "default_cutoff")]
    pub alpha_cutoff: f32,
    #[serde(default)]
    pub double_sided: bool,
}
fn default_uv_scale() -> [f32; 2] {
    [1.; 2]
}
fn default_cutoff() -> f32 {
    0.5
}
fn default_ior() -> f32 {
    1.5
}
fn default_coat_roughness() -> f32 {
    0.1
}
fn default_sheen_roughness() -> f32 {
    0.5
}
fn one() -> f32 {
    1.
}
impl Default for Material {
    fn default() -> Self {
        Self {
            visible: true,
            in_library: false,
            npr: crate::NprSettings::default(),
            artistic: crate::ArtisticSettings::default(),
            favorite: false,
            category: String::new(),
            tags: Vec::new(),
            graph: None,
            id: 0,
            name: "Материал".into(),
            uv_scale: [1.; 2],
            uv_offset: [0.; 2],
            uv_rotation: 0.,
            texture_uvs: [crate::TextureUvTransform::default(); 5],
            texture_samplers: [crate::TextureSampler::default(); 5],
            anisotropy: 0.,
            anisotropy_rotation: 0.,
            sheen_weight: 0.,
            sheen_roughness: default_sheen_roughness(),
            attenuation_color: None,
            transmission: 0.,
            thickness: 1.,
            geometric_thickness: true,
            attenuation_distance: 1.,
            base_color: [0.72, 0.74, 0.78, 1.],
            metallic: 0.,
            roughness: 0.6,
            ior: default_ior(),
            coat_weight: 0.,
            coat_roughness: default_coat_roughness(),
            emissive: [0.; 3],
            emission_volume: false,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.,
            normal_y_flip: false,
            occlusion_texture: None,
            occlusion_strength: 1.,
            emissive_texture: None,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Texture {
    pub id: AssetId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba8: crate::SharedBuffer<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Camera {
    #[serde(default)]
    pub effects: crate::CameraEffects,
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y_radians: f32,
    pub near: f32,
    pub far: f32,
    pub orthographic: bool,
    pub ortho_size: f32,
    #[serde(default)]
    pub depth_of_field: crate::DepthOfField,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            effects: crate::CameraEffects::default(),
            eye: Vec3::new(4., 2.8, 6.),
            target: Vec3::new(0., 1., 0.),
            up: Vec3::Y,
            fov_y_radians: 45f32.to_radians(),
            near: 0.01,
            far: 1000.,
            orthographic: false,
            ortho_size: 3.,
            depth_of_field: crate::DepthOfField::default(),
        }
    }
}
impl Camera {
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        let projection = if self.orthographic {
            let h = self.ortho_size.max(0.01);
            Mat4::orthographic_rh(-h * aspect, h * aspect, -h, h, self.near, self.far)
        } else {
            Mat4::perspective_rh(self.fov_y_radians, aspect.max(0.01), self.near, self.far)
        };
        projection * Mat4::look_at_rh(self.eye, self.target, self.up)
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum LightKind {
    #[default]
    Directional,
    Point,
    Spot,
    /// One-sided rectangular emitter; intensity is emitted radiance.
    Area,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Light {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_binding: Option<crate::CurveBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<LightTarget>,
    pub name: String,
    #[serde(default)]
    pub kind: LightKind,
    #[serde(default = "default_light_position")]
    pub position: Vec3,
    /// Light direction in world coordinates, or in the path frame when bound.
    pub direction: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    /// Zero means unlimited range. Positive values smoothly fade local lights.
    #[serde(default)]
    pub range: f32,
    /// Width and height of an area emitter, in world units. Ignored by other kinds.
    #[serde(default = "default_area_size")]
    pub area_size: [f32; 2],
    #[serde(default)]
    pub area_rotation: f32,
    #[serde(default)]
    pub use_temperature: bool,
    #[serde(default = "default_temperature")]
    pub temperature_kelvin: f32,
    #[serde(default)]
    pub inner_cone_angle: f32,
    #[serde(default = "default_outer_cone")]
    pub outer_cone_angle: f32,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Independent of illumination: disabling this keeps the light contribution.
    #[serde(default = "yes")]
    pub casts_shadow: bool,
    #[serde(default)]
    pub volume: VolumeLight,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LightTarget {
    pub node: NodeId,
    pub local_center: Vec3,
}

/// Independent scattering contribution of one source in the shared atmosphere.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VolumeLight {
    pub enabled: bool,
    pub strength: f32,
    pub tint: [f32; 3],
}
impl Default for VolumeLight {
    fn default() -> Self {
        Self { enabled: false, strength: 1., tint: [1.; 3] }
    }
}

impl Light {
    pub fn evaluated_direction(&self, globals: &BTreeMap<NodeId, Mat4>) -> Vec3 {
        self.target
            .and_then(|target| {
                (globals
                    .get(&target.node)?
                    .transform_point3(target.local_center)
                    - self.position)
                    .try_normalize()
            })
            .unwrap_or(self.direction)
    }
}

fn default_light_position() -> Vec3 {
    Vec3::new(0., 3., 0.)
}
fn default_temperature() -> f32 {
    6500.
}
fn default_area_size() -> [f32; 2] {
    [1., 1.]
}
fn default_outer_cone() -> f32 {
    std::f32::consts::FRAC_PI_4
}
impl Default for Light {
    fn default() -> Self {
        Self {
            curve_binding: None,
            name: "Основной свет".into(),
            target: None,
            kind: LightKind::Directional,
            position: default_light_position(),
            direction: Vec3::new(-0.5, -1., -0.4).normalize(),
            color: [1., 0.95, 0.88],
            intensity: 3.,
            range: 0.,
            area_size: default_area_size(),
            area_rotation: 0.,
            use_temperature: false,
            temperature_kelvin: default_temperature(),
            inner_cone_angle: 0.,
            outer_cone_angle: default_outer_cone(),
            enabled: true,
            casts_shadow: true,
            volume: VolumeLight::default(),
        }
    }
}

/// Output transfer from scene-linear HDR lighting to display-referred color.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ToneMapping {
    #[default]
    Aces,
    Reinhard,
    Linear,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RenderSettings {
    pub npr_contours: crate::NprContours,
    pub hdri_enabled: bool,
    pub hdri_background: bool,
    pub hdri_strength: f32,
    pub hdri_rotation: f32,
    pub ray_reflections: bool,
    pub reflection_samples: u32,
    pub ray_shadows: bool,
    pub area_shadow_samples: u32,
    pub background_mode: crate::BackgroundMode,
    pub background_secondary: [f32; 3],
    pub background_angle: f32,
    pub atmosphere: crate::Atmosphere,
    /// Exposure compensation in stops (EV), applied before tone mapping.
    pub exposure: f32,
    /// Multiplier for the renderer's hemispherical ambient illumination.
    pub ambient_intensity: f32,
    /// Scene sample count. UI/text remain single-sampled after scene resolution.
    pub antialiasing: u32,
    /// Authored output size in pixels; independent of the editor viewport.
    pub output_width: u32,
    pub output_height: u32,
    pub transparent_background: bool,
    pub tone_mapping: ToneMapping,
    /// Enabled sun, point and spot lights with casts_shadow can cast shadows.
    pub shadows: bool,
    pub shadow_resolution: u32,
    /// Receiver depth offset in normalized shadow-map depth coordinates.
    pub shadow_bias: f32,
    pub shadow_strength: f32,
    /// PCF sampling radius in shadow-map texels, not physical emitter size.
    pub shadow_filter_radius: f32,
    /// Screen-space ambient obscurance. Zero preserves legacy rendering.
    pub ambient_occlusion: f32,
    pub ambient_occlusion_radius: f32,
    /// Scene-linear RGB values for the upper/lower ambient hemisphere.
    pub sky_color: [f32; 3],
    pub ground_color: [f32; 3],
    /// Scene-linear RGB clear color for nontransparent render output.
    pub background_color: [f32; 3],
}
impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            npr_contours: crate::NprContours::default(),
            hdri_enabled: true,
            hdri_background: true,
            hdri_strength: 1.,
            hdri_rotation: 0.,
            ray_reflections: false,
            reflection_samples: 4,
            ray_shadows: false,
            area_shadow_samples: 16,
            background_mode: crate::BackgroundMode::Theme,
            background_secondary: [0.18, 0.24, 0.34],
            background_angle: 90.,
            atmosphere: crate::Atmosphere::default(),
            exposure: 0.,
            ambient_intensity: 1.,
            antialiasing: 4,
            output_width: 1920,
            output_height: 1080,
            transparent_background: false,
            tone_mapping: ToneMapping::Aces,
            shadows: true,
            shadow_resolution: 1024,
            shadow_bias: 0.001,
            shadow_strength: 0.85,
            shadow_filter_radius: 1.,
            ambient_occlusion: 0.,
            ambient_occlusion_radius: 0.35,
            sky_color: [0.28; 3],
            ground_color: [0.12; 3],
            background_color: [0.032, 0.043, 0.055],
        }
    }
}
impl RenderSettings {
    pub fn validate(&self) -> Result<(), CoreError> {
        self.atmosphere.validate()?;
        if !self.npr_contours.valid() {
            return Err(CoreError::Invalid(
                "Параметры экранного контура вне диапазона".into(),
            ));
        }
        if !self.hdri_strength.is_finite()
            || !(0.0..=32.0).contains(&self.hdri_strength)
            || !self.hdri_rotation.is_finite()
            || !(-180.0..=180.0).contains(&self.hdri_rotation)
            || !matches!(self.reflection_samples, 1 | 4 | 8 | 16)
            || !matches!(self.area_shadow_samples, 4 | 16 | 64)
        {
            return Err(CoreError::Invalid(
                "HDRI или число выборок трассировки вне диапазона".into(),
            ));
        }

        if !self.background_angle.is_finite()
            || self
                .background_secondary
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(CoreError::Invalid("цвет или угол фона".into()));
        }
        let finite_range =
            |value: f32, min: f32, max: f32| value.is_finite() && (min..=max).contains(&value);
        if !finite_range(self.exposure, -16., 16.) || !finite_range(self.ambient_intensity, 0., 16.)
        {
            return Err(CoreError::Invalid(
                "экспозиция должна быть от −16 до 16 EV, фоновый свет — от 0 до 16".into(),
            ));
        }
        if !(64..=4096).contains(&self.output_width) || !(64..=4096).contains(&self.output_height) {
            return Err(CoreError::Invalid(
                "размер изображения должен быть от 64 до 4096 пикселей".into(),
            ));
        }
        if !matches!(self.antialiasing, 1 | 4) {
            return Err(CoreError::Invalid(
                "сглаживание должно быть 1× или 4×".into(),
            ));
        }
        if !finite_range(self.ambient_occlusion, 0., 2.)
            || !finite_range(self.ambient_occlusion_radius, 0.01, 10.)
        {
            return Err(CoreError::Invalid(
                "контактное затенение: сила 0–2, радиус 0,01–10 м".into(),
            ));
        }
        if !matches!(self.shadow_resolution, 512 | 1024 | 2048)
            || !finite_range(self.shadow_bias, 0., 0.02)
            || !finite_range(self.shadow_strength, 0., 1.)
            || !finite_range(self.shadow_filter_radius, 0., 4.)
        {
            return Err(CoreError::Invalid(
                "параметры теней вне допустимого диапазона".into(),
            ));
        }
        if self
            .sky_color
            .iter()
            .chain(&self.ground_color)
            .any(|v| !finite_range(*v, 0., 16.))
            || self
                .background_color
                .iter()
                .any(|v| !finite_range(*v, 0., 1.))
        {
            return Err(CoreError::Invalid(
                "цвет окружения или фона вне допустимого диапазона".into(),
            ));
        }
        Ok(())
    }
}

/// Authoring parameters for the deterministic fluid preview. Runtime particles,
/// caches, playback state and solver-specific kernels are deliberately not stored
/// in a project. Applications map these values to their simulation backend.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FluidSettings {
    pub source_kind: u8,
    pub source_center: [f32; 3],
    pub source_size: [f32; 3],
    /// Downward acceleration magnitude in meters per second squared (Y is up).
    pub gravity: f32,
    pub viscosity: f32,
    pub vorticity: f32,
    pub boundary_friction: f32,
    pub iterations: u32,
    pub max_substeps: u32,
    pub time_scale: f32,
    /// Grid spacing in meters for a captured surface, not a simulation timestep.
    pub surface_cell_size: f32,
    /// Isosurface threshold relative to the fluid's rest density.
    pub surface_iso_density: f32,
}

/// Object-owned liquid setup. The original mesh remains the editable container;
/// its material shades a transient surface in the viewport and exported frames.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FluidDomainSettings {
    pub visible: bool,
    pub simulation: FluidSettings,
    /// Fraction of the container height initially filled, from its lower Y face.
    pub fill: f32,
    pub enabled: bool,
    /// Collide with visible meshes in the receiver list; legacy lists use PhysicsKind::Collider.
    pub mesh_collision: bool,
}
impl Default for FluidDomainSettings {
    fn default() -> Self {
        Self {
            simulation: FluidSettings::default(),
            visible: true,
            fill: 1.,
            enabled: true,
            mesh_collision: true,
        }
    }
}
impl FluidDomainSettings {
    pub fn validate(&self) -> Result<(), CoreError> {
        self.simulation.validate()?;
        if !self.fill.is_finite() || !(0.1..=1.).contains(&self.fill) {
            return Err(CoreError::Invalid("заполнение контейнера: 10–100%".into()));
        }
        Ok(())
    }
}
impl Default for FluidSettings {
    fn default() -> Self {
        Self {
            source_kind: 0,
            source_center: [-0.36, 0.73, 0.],
            source_size: [0.88, 0.56, 0.4],
            gravity: 9.81,
            viscosity: 0.04,
            vorticity: 0.1,
            boundary_friction: 0.05,
            iterations: 4,
            max_substeps: 4,
            time_scale: 1.,
            surface_cell_size: 0.045,
            surface_iso_density: 0.5,
        }
    }
}
impl FluidSettings {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.source_kind > 2
            || self
                .source_center
                .iter()
                .any(|x| !x.is_finite() || x.abs() > 100.)
            || self
                .source_size
                .iter()
                .any(|x| !x.is_finite() || !(0.08..=10.0).contains(x))
        {
            return Err(CoreError::Invalid("область источника жидкости".into()));
        }
        for (name, value, min, max) in [
            ("гравитация", self.gravity, 0., 30.),
            ("вязкость", self.viscosity, 0., 0.2),
            ("вихри", self.vorticity, 0., 5.),
            ("трение границ", self.boundary_friction, 0., 1.),
            ("скорость времени", self.time_scale, 0.1, 2.),
            (
                "размер ячейки поверхности",
                self.surface_cell_size,
                0.025,
                0.12,
            ),
            ("плотность поверхности", self.surface_iso_density, 0.1, 1.2),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(CoreError::Invalid(format!(
                    "жидкость: {name} должна быть от {min} до {max}"
                )));
            }
        }
        if !(1..=16).contains(&self.iterations) || !(1..=8).contains(&self.max_substeps) {
            return Err(CoreError::Invalid(
                "жидкость: число итераций должно быть от 1 до 16, подшагов — от 1 до 8".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    #[serde(default)]
    pub outliner: crate::Outliner,
    #[serde(default)]
    pub environment_map: Option<std::sync::Arc<crate::EnvironmentMap>>,
    #[serde(default)]
    pub environment_library: Vec<std::sync::Arc<crate::EnvironmentMap>>,
    #[serde(default)]
    pub saved_views: Vec<crate::SavedView>,
    #[serde(default)]
    pub collections: Vec<crate::Collection>,
    pub version: u32,
    pub title: String,
    pub fps: u32,
    pub duration: f64,
    pub nodes: Vec<Node>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    #[serde(default)]
    pub textures: Vec<Texture>,
    #[serde(default)]
    pub video_textures: Vec<crate::VideoTexture>,
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub animation_layers: Vec<crate::AnimationLayer>,
    pub camera: Camera,
    /// None uses the saved editor view; otherwise export follows this camera node.
    #[serde(default)]
    pub active_camera: Option<NodeId>,
    #[serde(default)]
    pub lights: Vec<Light>,
    #[serde(default)]
    pub render_settings: RenderSettings,
    #[serde(default)]
    pub fluid_settings: FluidSettings,
    #[serde(default)]
    pub poses: Vec<SavedPose>,
    /// Persistent humanoid mappings reference stable node IDs, never names.
    /// The loader migrates schema-1 projects that omit this authoring metadata.
    #[serde(default)]
    pub rig_profiles: Vec<RigProfile>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedPose {
    pub name: String,
    pub transforms: BTreeMap<NodeId, Transform>,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            outliner: crate::Outliner::default(),
            environment_map: None,
            environment_library: Vec::new(),
            saved_views: Vec::new(),
            collections: Vec::new(),
            version: SCHEMA_VERSION,
            title: "Новый проект".into(),
            fps: 24,
            duration: 4.,
            nodes: vec![],
            meshes: vec![],
            materials: vec![],
            textures: vec![],
            video_textures: vec![],
            clips: vec![],
            animation_layers: vec![],
            camera: Camera::default(),
            active_camera: None,
            lights: vec![Light::default()],
            render_settings: RenderSettings::default(),
            fluid_settings: FluidSettings::default(),
            poses: vec![],
            rig_profiles: vec![],
        }
    }
}
impl Project {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
    pub fn mesh(&self, id: AssetId) -> Option<&Mesh> {
        self.meshes.iter().find(|m| m.id == id)
    }
    pub fn material(&self, id: AssetId) -> Option<&Material> {
        self.materials.iter().find(|m| m.id == id)
    }
    pub fn next_id(&self) -> u64 {
        self.nodes
            .iter()
            .map(|n| n.id)
            .chain(self.meshes.iter().map(|m| m.id))
            .chain(self.materials.iter().map(|m| m.id))
            .chain(self.textures.iter().map(|t| t.id))
            .chain(self.collections.iter().map(|c| c.id))
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }
    pub fn global_transforms(&self) -> Result<BTreeMap<NodeId, Mat4>, CoreError> {
        self.globals_for(&BTreeMap::new())
    }
    pub fn globals_for(
        &self,
        overrides: &BTreeMap<NodeId, Transform>,
    ) -> Result<BTreeMap<NodeId, Mat4>, CoreError> {
        self.curve_globals_for(overrides, 0.)
    }

    pub fn evaluated(&self, clip: Option<usize>, time: f64) -> Result<EvaluatedScene, CoreError> {
        crate::animation::evaluate(self, clip, time, &BTreeMap::new())
    }
    pub fn validate(&self) -> Result<(), CoreError> {
        self.validate_helpers()?;
        self.validate_fx()?;
        let invalid = |s: &str| CoreError::Invalid(s.into());
        if self.version != SCHEMA_VERSION {
            return Err(invalid("неподдерживаемая версия проекта"));
        }
        if !(1..=240).contains(&self.fps) || !self.duration.is_finite() || self.duration < 0. {
            return Err(invalid("частота или длительность проекта"));
        }
        for (kind, ids) in [
            (
                "объекты",
                self.nodes.iter().map(|x| x.id).collect::<Vec<_>>(),
            ),
            ("геометрия", self.meshes.iter().map(|x| x.id).collect()),
            ("материалы", self.materials.iter().map(|x| x.id).collect()),
            ("текстуры", self.textures.iter().map(|x| x.id).collect()),
        ] {
            if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() {
                return Err(CoreError::Invalid(format!("повторяющиеся ID: {kind}")));
            }
        }
        self.validate_curve_bindings()?;
        self.validate_collision_targets()?;
        self.validate_text_objects()?;
        let globals = self.global_transforms()?;
        if self.lights.iter().any(|l| l.curve_binding.is_some()) { self.evaluated_lights(&globals, 0.)?; }
        self.validate_scene_cameras()?;
        self.validate_collections()?;
        let node_ids: BTreeSet<_> = self.nodes.iter().map(|n| n.id).collect();
        let mut skin_matrices = 0usize;
        for n in &self.nodes {
            if n.bone_display.is_some_and(|display| !display.is_valid()) {
                return Err(invalid("Цвет кости и сустава должен быть конечным sRGB 0–1"));
            }
            if n.bone_tip.is_some_and(|tip| !tip.is_finite() || tip.length_squared() < 1e-12) {
                return Err(invalid("Некорректный конец кости"));
            }
            if let Some(curve) = &n.curve {
                curve.validate()?;
                if curve.geometry.is_some() != n.mesh.is_some() || n.skin.is_some() || n.camera.is_some() || n.is_joint {
                    return Err(invalid("Кривая: геометрия должна соответствовать объекту; скелет и камера не поддерживаются"));
                }
            }
            if !n.transform.is_valid() || n.rest_transform.is_some_and(|t| !t.is_valid()) {
                return Err(CoreError::Invalid(format!("трансформация {}", n.name)));
            }
            if let Some(m) = n.mesh {
                if self.mesh(m).is_none() {
                    return Err(invalid("отсутствует геометрия объекта"));
                }
            }
            if n.material_layers.len() > 64 || (!n.material_layers.is_empty() && n.mesh.is_none()) {
                return Err(invalid("Слои материалов требуют меш; не более 64 слоёв на объект"));
            }
            let mut layer_ids = BTreeSet::new();
            for layer in &n.material_layers {
                if self.material(layer.material).is_none() || !layer_ids.insert(layer.material) {
                    return Err(invalid("Отсутствующий или повторный материал слоя"));
                }
            }
            if n.morph_weights.iter().any(|v| !v.is_finite()) {
                return Err(invalid("нечисловой morph weight"));
            }
            if let Some(mesh) = n.mesh.and_then(|id| self.mesh(id)) {
                if !n.morph_weights.is_empty() && n.morph_weights.len() != mesh.morph_targets.len()
                {
                    return Err(invalid("число morph weights не соответствует геометрии"));
                }
            }
            if let Some(skin) = &n.skin {
                if skin.joints.len() != skin.inverse_bind_matrices.len()
                    || skin.joints.len() > MAX_SKIN_JOINTS
                    || skin.joints.is_empty()
                {
                    return Err(invalid("некорректный skin или более 65 536 суставов"));
                }
                skin_matrices = skin_matrices.saturating_add(skin.joints.len());
                if skin_matrices > MAX_SKIN_MATRICES {
                    return Err(invalid("Суммарные матрицы skin превышают бюджет 64 МиБ"));
                }
                for (j, ibm) in skin.joints.iter().zip(&skin.inverse_bind_matrices) {
                    if !node_ids.contains(j) || !ibm.is_finite() {
                        return Err(invalid("сустав skin или inverse bind matrix"));
                    }
                }
                if let Some(mesh) = n.mesh.and_then(|id| self.mesh(id)) {
                    for v in &mesh.vertices {
                        for i in 0..4 {
                            if v.weights[i] > 0. && v.joints[i] as usize >= skin.joints.len() {
                                return Err(invalid("индекс сустава вне skin"));
                            }
                        }
                    }
                }
            }
        }
        for m in &self.meshes {
            m.validate_topology()?;
            if !m.indices.len().is_multiple_of(3)
                || m.indices.iter().any(|&i| i as usize >= m.vertices.len())
            {
                return Err(invalid("индексы треугольников"));
            }
            for id in m.assigned_materials() {
                if self.material(id).is_none() {
                    return Err(invalid("отсутствует материал"));
                }
            }
            for v in &m.vertices {
                if v.position
                    .iter()
                    .chain(&v.normal)
                    .chain(&v.uv)
                    .chain(&v.weights)
                    .chain(&v.color)
                    .any(|f| !f.is_finite())
                    || v.weights.iter().any(|w| *w < 0.)
                {
                    return Err(invalid("нечисловые или отрицательные данные вершины"));
                }
                if v.color.iter().any(|c| !(0. ..=1.).contains(c)) {
                    return Err(invalid("цвет вершины должен быть в диапазоне 0–1"));
                }
                let sum: f32 = v.weights.iter().sum();
                if sum > 1e-6 && (sum - 1.).abs() > 0.01 {
                    return Err(invalid("веса skin не нормализованы"));
                }
            }
            for target in &m.morph_targets {
                if target.positions.len() != m.vertices.len()
                    || (!target.normals.is_empty() && target.normals.len() != m.vertices.len())
                {
                    return Err(invalid("размер morph target"));
                }
                if target
                    .positions
                    .iter()
                    .chain(&target.normals)
                    .flatten()
                    .any(|f| !f.is_finite())
                {
                    return Err(invalid("нечисловой morph target"));
                }
            }
        }
        for t in &self.textures {
            if t.width == 0
                || t.height == 0
                || t.width > 16384
                || t.height > 16384
                || (t.width as u64 * t.height as u64 * 4) != t.rgba8.len() as u64
            {
                return Err(invalid("размер текстуры"));
            }
        }
        if self.video_textures.len() > 16 { return Err(invalid("В проекте допускается до 16 видеоклипов")); }
        let mut video_ids = BTreeSet::new();
        let mut video_bytes = 0usize;
        for video in &self.video_textures {
            video.validate()?;
            if !video_ids.insert(video.texture) || self.textures.iter().find(|t| t.id == video.texture)
                .is_none_or(|t| t.width != video.width || t.height != video.height) {
                return Err(invalid("Отсутствует или повторяется текстура видео"));
            }
            video_bytes = video_bytes.saturating_add(video.data.len());
            if video_bytes > crate::video::MAX_VIDEO_TOTAL_BYTES { return Err(invalid("Видео проекта превышают 128 МиБ")); }
        }
        for m in &self.materials {
            if m.texture_uvs.iter().any(|mapping| !mapping.valid()) {
                return Err(invalid("Некорректное преобразование UV текстуры"));
            }
            if !m.artistic.valid() {
                return Err(invalid("параметры художественной поверхности вне допустимого диапазона"));
            }
            if !m.npr.valid() {
                return Err(invalid("параметры NPR-материала вне допустимого диапазона"));
            }
            if m.uv_scale
                .iter()
                .any(|v| !v.is_finite() || !(0.001..=1000.).contains(v))
                || m.uv_offset
                    .iter()
                    .any(|v| !v.is_finite() || !(-1000. ..=1000.).contains(v))
                || !m.uv_rotation.is_finite()
                || !(-180. ..=180.).contains(&m.uv_rotation)
            {
                return Err(invalid("UV: масштаб 0,001–1000, сдвиг ±1000, угол ±180°"));
            }
            if !m.anisotropy.is_finite()
                || !(0.0..=0.95).contains(&m.anisotropy)
                || !m.anisotropy_rotation.is_finite()
                || !(-180.0..=180.0).contains(&m.anisotropy_rotation)
                || !m.sheen_weight.is_finite()
                || !(0.0..=1.0).contains(&m.sheen_weight)
                || !m.sheen_roughness.is_finite()
                || !(0.07..=1.0).contains(&m.sheen_roughness)
                || m.attenuation_color.is_some_and(|color| {
                    color
                        .iter()
                        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                })
            {
                return Err(invalid(
                    "параметры анизотропии, ткани или поглощения вне допустимого диапазона",
                ));
            }
            if m.base_color
                .iter()
                .chain(&m.emissive)
                .any(|f| !f.is_finite())
                || !m.metallic.is_finite()
                || !m.roughness.is_finite()
                || !m.ior.is_finite()
                || !m.coat_weight.is_finite()
                || !m.coat_roughness.is_finite()
                || !m.alpha_cutoff.is_finite()
                || !m.normal_scale.is_finite()
                || !m.occlusion_strength.is_finite()
            {
                return Err(invalid("нечисловой материал"));
            }
            if m.base_color.iter().any(|v| !(0.0..=1.0).contains(v))
                || m.emissive.iter().any(|v| *v < 0.)
                || !(0.0..=1.0).contains(&m.metallic)
                || !(0.0..=1.0).contains(&m.roughness)
                || !(1.0..=3.0).contains(&m.ior)
                || !(0.0..=1.0).contains(&m.coat_weight)
                || !(0.03..=1.0).contains(&m.coat_roughness)
                || m.alpha_cutoff < 0.
                || !(0.0..=1.0).contains(&m.occlusion_strength)
            {
                return Err(invalid("параметры PBR-материала вне допустимого диапазона"));
            }
            for id in [
                m.base_color_texture,
                m.metallic_roughness_texture,
                m.normal_texture,
                m.occlusion_texture,
                m.emissive_texture,
            ]
            .into_iter()
            .flatten()
            {
                if !self.textures.iter().any(|t| t.id == id) {
                    return Err(invalid("отсутствует текстура материала"));
                }
            }
        }
        for c in &self.clips {
            c.validate(self)?;
        }
        for pose in &self.poses {
            if pose
                .transforms
                .iter()
                .any(|(id, t)| self.node(*id).is_none() || !t.is_valid())
            {
                return Err(invalid("повреждённая сохранённая поза"));
            }
        }
        let mut profile_roots = BTreeSet::new();
        for profile in &self.rig_profiles {
            let root = profile
                .root
                .ok_or_else(|| invalid("сохранённый профиль рига должен иметь корень"))?;
            if !profile_roots.insert(root) {
                return Err(invalid("несколько профилей рига для одного корня"));
            }
            profile.validate(self)?;
        }
        if self.lights.iter().filter(|light| light.enabled).count() > MAX_ENABLED_LIGHTS {
            return Err(invalid(
                "одновременно поддерживается не более 8 включённых источников света",
            ));
        }
        for light in &self.lights {
            if light
                .target
                .is_some_and(|t| !t.local_center.is_finite() || self.node(t.node).is_none())
            {
                return Err(invalid("Некорректная цель источника света"));
            }
            if !light.area_rotation.is_finite()
                || !(-180. ..=180.).contains(&light.area_rotation)
                || !light.temperature_kelvin.is_finite()
                || !(1000. ..=12000.).contains(&light.temperature_kelvin)
            {
                return Err(invalid("свет: угол ±180°, температура 1000–12000 K"));
            }
            if !light.volume.strength.is_finite()
                || !(0. ..=10.).contains(&light.volume.strength)
                || light.volume.tint.iter().any(|c| !c.is_finite() || !(0. ..=1.).contains(c))
            {
                return Err(invalid("объёмный свет: сила 0–10, оттенок 0–1"));
            }
            if !light.position.is_finite()
                || !light.direction.is_finite()
                || !light.direction.length_squared().is_finite()
                || light.direction.length_squared() < 1e-8
                || light.color.iter().any(|c| !c.is_finite() || *c < 0.)
                || !light.intensity.is_finite()
                || light.intensity < 0.
                || !light.range.is_finite()
                || light.range < 0.
                || light
                    .area_size
                    .iter()
                    .any(|size| !size.is_finite() || *size <= 0. || *size > 10000.)
                || !light.inner_cone_angle.is_finite()
                || !light.outer_cone_angle.is_finite()
                || light.inner_cone_angle < 0.
                || light.inner_cone_angle >= light.outer_cone_angle
                || light.outer_cone_angle > std::f32::consts::FRAC_PI_2
            {
                return Err(invalid("параметры света"));
            }
        }
        if self.nodes.iter().filter(|n| n.fluid.is_some()).count() > 8 {
            return Err(invalid("не более восьми контейнеров жидкости"));
        }
        let mut destruction_results = BTreeSet::new();
        for n in &self.nodes {
            if let Some(settings) = n.destruction {
                settings.validate()?;
                if settings.interior_material.is_some_and(|id| self.material(id).is_none()) {
                    return Err(invalid("материал внутренних граней разрушения отсутствует"));
                }
                if n.mesh.is_none() || n.fluid.is_some() || n.physics.is_some() || n.skin.is_some() {
                    return Err(invalid("разрушение требует отдельный меш без жидкости, ткани и skin"));
                }
                if let Some(root) = settings.result {
                    if root == n.id || self.node(root).is_none() || !destruction_results.insert(root) {
                        return Err(invalid("результат разрушения отсутствует или назначен нескольким объектам"));
                    }
                    let result = self.node(root).unwrap();
                    if result.mesh.is_some() || result.camera.is_some() || result.destruction.is_some() {
                        return Err(invalid("результат разрушения должен быть отдельной группой"));
                    }
                    // The source and generated group must not contain each other:
                    // hiding either side must never hide both or delete the source.
                    for (start, forbidden) in [(n.parent, root), (result.parent, n.id)] {
                        let mut cursor = start;
                        let mut seen = BTreeSet::new();
                        while let Some(id) = cursor {
                            if id == forbidden || !seen.insert(id) {
                                return Err(invalid("исходник и осколки не могут быть вложены друг в друга"));
                            }
                            cursor = self.node(id).and_then(|node| node.parent);
                        }
                    }
                }
            }
            if let Some(settings) = n.fluid {
                settings.validate()?;
                let mesh = n.mesh.and_then(|id| self.mesh(id))
                    .ok_or_else(|| invalid("контейнер жидкости требует меш"))?;
                if n.physics.is_some() || n.skin.is_some() || !mesh.morph_targets.is_empty() {
                    return Err(invalid("контейнер жидкости: без ткани, skin и morph"));
                }
                let min = mesh.vertices.iter().map(|v| Vec3::from_array(v.position))
                    .fold(Vec3::splat(f32::INFINITY), Vec3::min);
                let max = mesh.vertices.iter().map(|v| Vec3::from_array(v.position))
                    .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
                if !min.is_finite() || !max.is_finite() || (max - min).min_element() < 0.08 {
                    return Err(invalid("контейнер жидкости должен иметь объём: каждая сторона от 0.08"));
                }
            }
            if let Some(settings) = n.physics {
                settings.validate()?;
                let Some(mesh) = n.mesh.and_then(|id| self.mesh(id)) else {
                    return Err(invalid("физика требует меш"));
                };
                if settings.kind != crate::PhysicsKind::Collider
                    && (n.skin.is_some()
                        || !mesh.morph_targets.is_empty()
                        || mesh.vertices.len() > 8192)
                {
                    return Err(invalid("ткань и мышца: до 8192 вершин, без skin/morph"));
                }
            }
        }
        for l in &self.animation_layers {
            if l.clip >= self.clips.len()
                || !l.weight.is_finite()
                || !(0.0..=1.0).contains(&l.weight)
            {
                return Err(invalid("некорректный слой анимации"));
            }
        }
        for m in &self.materials {
            if !m.transmission.is_finite()
                || !(0.0..=1.0).contains(&m.transmission)
                || !m.thickness.is_finite()
                || !(0.001..=100.0).contains(&m.thickness)
                || !m.attenuation_distance.is_finite()
                || !(0.001..=1000.0).contains(&m.attenuation_distance)
            {
                return Err(invalid("параметры пропускания материала"));
            }
        }
        if self.environment_library.len() + usize::from(self.environment_map.is_some()) > 16 {
            return Err(invalid("не более 16 панорам HDRI"));
        }
        let mut environment_bytes = 0usize;
        for map in self
            .environment_map
            .iter()
            .chain(self.environment_library.iter())
        {
            map.validate()?;
            environment_bytes = environment_bytes.saturating_add(map.rgb.len().saturating_mul(12));
        }
        if environment_bytes > 512 * 1024 * 1024 {
            return Err(invalid("панорамы HDRI превышают 512 МиБ"));
        }
        if let Some(map) = &self.environment_map {
            map.validate()?;
        }
        for material in &self.materials {
            if material.category.chars().count() > 64
                || material.category.chars().any(char::is_control)
                || material.tags.len() > 16
                || material.tags.iter().any(|s| {
                    s.is_empty() || s.chars().count() > 32 || s.chars().any(char::is_control)
                })
            {
                return Err(invalid("категория или теги материала"));
            }
            if let Some(graph) = &material.graph {
                graph.compile()?;
                if graph
                    .nodes
                    .iter()
                    .filter_map(|n| n.texture)
                    .any(|id| !self.textures.iter().any(|t| t.id == id))
                {
                    return Err(invalid("изображение узла материала отсутствует"));
                }
            }
        }
        self.render_settings.validate()?;
        self.fluid_settings.validate()?;
        let fluid = self.fluid_settings;
        if fluid.source_kind > 2
            || fluid
                .source_center
                .iter()
                .any(|x| !x.is_finite() || x.abs() > 100.)
            || fluid
                .source_size
                .iter()
                .any(|x| !x.is_finite() || !(0.08..=10.0).contains(x))
        {
            return Err(invalid("область источника жидкости"));
        }
        self.camera.validate()?;
        if self.saved_views.len() > 64 {
            return Err(invalid("не более 64 сохранённых видов"));
        }
        for view in &self.saved_views {
            if view.name.trim().is_empty()
                || view.name.chars().count() > 64
                || view.name.chars().any(char::is_control)
            {
                return Err(invalid("имя сохранённого вида"));
            }
            view.camera.validate()?;
        }
        Ok(())
    }
    /// Clone a whole hierarchy while sharing immutable geometry and textures.
    /// Skins, animation channels and complete rig profiles are remapped to the
    /// new stable node IDs. Partial subtree copies never inherit a profile that
    /// still references the original character.
    pub fn duplicate_subtree(&mut self, root: NodeId) -> Result<NodeId, CoreError> {
        if self.node(root).is_none() {
            return Err(CoreError::MissingNode(root));
        }
        let mut selected = BTreeSet::from([root]);
        loop {
            let before = selected.len();
            for n in &self.nodes {
                if n.parent.map(|p| selected.contains(&p)).unwrap_or(false) {
                    selected.insert(n.id);
                }
            }
            if selected.len() == before {
                break;
            }
        }
        let mut next = self.next_id();
        if next.checked_add(selected.len() as u64).is_none() {
            return Err(CoreError::Invalid("исчерпано пространство ID".into()));
        }
        let map: BTreeMap<_, _> = selected
            .iter()
            .map(|&id| {
                let new = next;
                next += 1;
                (id, new)
            })
            .collect();
        let copies: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| selected.contains(&n.id))
            .map(|n| {
                let mut n = n.clone();
                n.id = map[&n.id];
                n.name = format!("{} — копия", n.name);
                n.parent = n.parent.map(|p| map.get(&p).copied().unwrap_or(p));
                n.collision_targets.remap(&map);
                if let Some(fx)=&mut n.fx {fx.source=fx.source.map(|id|map.get(&id).copied().unwrap_or(id));}
                if let Some(binding) = &mut n.curve_binding { binding.curve = map.get(&binding.curve).copied().unwrap_or(binding.curve); }
                if let Some(settings) = &mut n.destruction {
                    settings.result = settings.result.and_then(|id| map.get(&id).copied());
                    if settings.result.is_none() {
                        n.visible = settings.source_visible; n.render_visible = settings.source_render_visible;
                    }
                }
                if let Some(s) = &mut n.skin {
                    for j in &mut s.joints {
                        *j = map.get(j).copied().unwrap_or(*j);
                    }
                }
                n
            })
            .collect();
        for collection in &mut self.collections {
            let additions: Vec<_> = collection
                .members
                .iter()
                .filter_map(|id| map.get(id).copied())
                .collect();
            collection.members.extend(additions);
        }
        self.nodes.extend(copies);
        for clip in &mut self.clips {
            let channels: Vec<_> = clip
                .channels
                .iter()
                .filter_map(|channel| {
                    map.get(&channel.node).map(|&new| {
                        let mut c = channel.clone();
                        c.node = new;
                        c
                    })
                })
                .collect();
            clip.channels.extend(channels);
        }
        let profiles: Vec<_> = self
            .rig_profiles
            .iter()
            .filter(|profile| {
                !profile.assignments.is_empty()
                    && profile.root.is_none_or(|id| map.contains_key(&id))
                    && profile.assignments.values().all(|id| map.contains_key(id))
            })
            .map(|profile| {
                let mut copy = profile.clone();
                copy.root = copy.root.map(|id| map[&id]);
                for id in copy.assignments.values_mut() {
                    *id = map[&*id];
                }
                copy
            })
            .collect();
        self.rig_profiles.extend(profiles);
        Ok(map[&root])
    }
}
