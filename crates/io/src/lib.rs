//! File import and crash-safe, self-contained project persistence.
//! No GPU handles or editor hover state cross this boundary.

mod blender;
mod gltf_preflight;
mod import;
mod import_limits;
mod obj;
mod obj_polygon;
mod obj_resource;
mod project;
mod rig_manifest;
mod skin_weights;
mod texture;

pub use blender::{
    import_blend_with_options, import_blend_with_progress, locate_blender, BlendImportOptions,
};
pub use import::{import_gltf, import_gltf_with_progress, ImportReport, ImportResult};
pub use obj::{import_obj, import_obj_with_progress};
pub use project::{load_project, load_project_bytes, merge_project, save_project};
pub use rig_manifest::{parse_rig_manifest, RigBone, RigManifest, RigMatchReport};
pub use texture::{load_texture, load_texture_bytes, TextureReduction};

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("Ошибка файла: {0}")]
    File(#[from] std::io::Error),
    #[error("Неверный JSON проекта: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Ошибка glTF: {0}")]
    Gltf(#[from] gltf::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Limit(String),
    #[error("Импорт отменён")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, IoError>;

/// Import a supported interchange model; Blender projects use the separate
/// installed-Blender bridge so callers can supply its executable and progress.
pub fn import_model(path: impl AsRef<std::path::Path>) -> Result<ImportResult> {
    import_model_with_progress(path, &std::sync::atomic::AtomicBool::new(false), |_, _| {})
}

pub fn import_model_with_progress(
    path: impl AsRef<std::path::Path>,
    cancelled: &std::sync::atomic::AtomicBool,
    progress: impl FnMut(f32, &str),
) -> Result<ImportResult> {
    let path = path.as_ref();
    match path.extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "gltf" | "glb" => import_gltf_with_progress(path, cancelled, progress),
        "obj" => import_obj_with_progress(path, cancelled, progress),
        _ => Err(IoError::Invalid("Поддерживаются .blend, .glb, .gltf и .obj; для .blend используется установленный Blender".into())),
    }
}

mod environment;
pub use environment::load_environment;

pub mod video;
pub use video::{decode_video_frame, is_video_path, load_video};
