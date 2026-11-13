use crate::{IoError, Result};
use image::ImageDecoder;
use poseforge_core::EnvironmentMap;
use std::{fs::File, io::BufReader, path::Path};

/// Radiance RGBE remains scene-linear and above one; ordinary images are decoded from sRGB.
pub fn load_environment(path: impl AsRef<Path>) -> Result<EnvironmentMap> {
    let path = path.as_ref();
    let file = File::open(path)?;
    if file.metadata()?.len() > 128 * 1024 * 1024 {
        return Err(IoError::Invalid("HDRI превышает 128 МиБ".into()));
    }
    let reader = image::ImageReader::new(BufReader::new(file)).with_guessed_format()?;
    let hdr = reader.format() == Some(image::ImageFormat::Hdr);
    let decoder = reader
        .into_decoder()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || width > 4096 || height > 2048 {
        return Err(IoError::Invalid(
            "Панорама HDRI должна быть не более 4096×2048".into(),
        ));
    }
    let decoded = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| IoError::Invalid(e.to_string()))?
        .into_rgb32f();
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let rgb = decoded
        .pixels()
        .map(|p| p.0.map(|v| if hdr { v } else { linear(v) }))
        .collect();
    let map = EnvironmentMap {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        width,
        height,
        rgb,
    };
    map.validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    Ok(map)
}
