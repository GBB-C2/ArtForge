//! Sampling belongs to a material slot, not to the shared source image.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextureWrap {
    #[default]
    Repeat,
    MirroredRepeat,
    ClampToEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextureMagFilter {
    Nearest,
    #[default]
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextureMinFilter {
    Nearest,
    Linear,
    NearestMipmapNearest,
    LinearMipmapNearest,
    NearestMipmapLinear,
    #[default]
    LinearMipmapLinear,
}

/// Defaults preserve the editor's trilinear repeat sampling for older projects
/// and for glTF samplers with unspecified filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextureSampler {
    pub wrap_s: TextureWrap,
    pub wrap_t: TextureWrap,
    pub mag_filter: TextureMagFilter,
    pub min_filter: TextureMinFilter,
}
