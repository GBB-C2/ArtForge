use crate::{IoError, Result};
use poseforge_core::Texture;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Cursor, Read},
    path::Path,
};

/// One source image's initial and final dimensions after memory adaptation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextureReduction {
    pub texture_id: u64,
    pub name: String,
    pub original_size: [u32; 2],
    pub imported_size: [u32; 2],
}

pub(crate) const MAX_IMAGE_FILE: u64 = 64 * 1024 * 1024;
const MAX_DECODED: u64 = 256 * 1024 * 1024;

/// Decode a local image as an unattached asset. The caller assigns an asset ID
/// only after the worker has completed and the document is ready to commit.
pub fn load_texture(path: impl AsRef<Path>) -> Result<Texture> {
    let path = path.as_ref();
    decode_texture(
        &read_image(path)?,
        &path.file_name().unwrap_or_default().to_string_lossy(),
        MAX_DECODED,
    )
}

pub(crate) fn read_image(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_IMAGE_FILE {
        return Err(IoError::Limit("Изображение превышает 64 МиБ".into()));
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_FILE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_IMAGE_FILE {
        return Err(IoError::Limit("Изображение превышает 64 МиБ".into()));
    }
    Ok(bytes)
}

/// Check headers and the remaining document budget before allocating pixels.
pub(crate) fn decode_texture(bytes: &[u8], name: &str, budget: u64) -> Result<Texture> {
    if bytes.len() as u64 > MAX_IMAGE_FILE {
        return Err(IoError::Limit("Изображение превышает 64 МиБ".into()));
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    // TGA has no reliable magic number. Use its filename only when content
    // sniffing could not identify another format (e.g. a PNG renamed to .tga).
    if reader.format().is_none()
        && Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tga"))
    {
        reader.set_format(image::ImageFormat::Tga);
    }
    let format = reader.format();
    if !matches!(
        format,
        Some(
            image::ImageFormat::Png
                | image::ImageFormat::Jpeg
                | image::ImageFormat::Tga
                | image::ImageFormat::Bmp
                | image::ImageFormat::Dds
        )
    ) {
        return Err(IoError::Invalid(
            "Поддерживаются PNG, JPEG, TGA, BMP и DDS (BC1/BC2/BC3); преобразуйте эту карту в PNG"
                .into(),
        ));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(budget.min(MAX_DECODED));
    reader.limits(limits.clone());
    let (width, height) = reader.into_dimensions().map_err(image_error)?;
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(IoError::Limit(
            "Размер текстуры должен быть от 1×1 до 8192×8192".into(),
        ));
    }
    if width as u64 * height as u64 * 4 > budget.min(MAX_DECODED) {
        return Err(IoError::Limit(
            "Суммарные декодированные текстуры превышают 256 МиБ".into(),
        ));
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes));
    reader.set_format(format.unwrap());
    reader.limits(limits);
    let decoded = reader.decode().map_err(image_error)?;
    let (width, height) = (decoded.width(), decoded.height());
    if width == 0 || height == 0 || width as u64 * height as u64 * 4 > MAX_DECODED {
        return Err(IoError::Invalid(
            "Некорректный размер текстуры или превышен лимит 256 МиБ".into(),
        ));
    }
    Ok(Texture {
        id: 0,
        name: name.into(),
        width,
        height,
        rgba8: decoded.into_rgba8().into_raw().into(),
    })
}

fn image_error(error: image::ImageError) -> IoError {
    if matches!(error, image::ImageError::Limits(_)) {
        IoError::Limit(format!("Превышен лимит декодирования текстуры: {error}"))
    } else {
        IoError::Invalid(format!("Не удалось прочитать текстуру: {error}"))
    }
}

/// Keep every imported map and its stable ID, reducing the largest maps first.
/// Decoding remains independently bounded; this runs in the import worker.
pub(crate) fn fit_texture_budget(
    textures: &mut [Texture],
    budget: usize,
) -> Result<Vec<TextureReduction>> {
    let mut total: usize = textures.iter().map(|t| t.rgba8.len()).sum();
    let mut changed = std::collections::BTreeMap::new();
    while total > budget {
        let Some((index, texture)) = textures
            .iter_mut()
            .enumerate()
            .filter(|(_, t)| t.width > 1 || t.height > 1)
            .max_by_key(|(_, t)| t.rgba8.len())
        else {
            return Err(IoError::Limit("Слишком много отдельных текстур".into()));
        };
        changed
            .entry(index)
            .or_insert((texture.width, texture.height));
        let old = texture.rgba8.len();
        // A 2×2 box reduction has bounded scratch memory (only the output),
        // unlike a separable float resampler on an 8K source.
        let width = texture.width.div_ceil(2);
        let height = texture.height.div_ceil(2);
        let mut pixels = vec![0; width as usize * height as usize * 4];
        for y in 0..height {
            for x in 0..width {
                let mut sum = [0u32; 4];
                let mut count = 0;
                for sy in y * 2..(y * 2 + 2).min(texture.height) {
                    for sx in x * 2..(x * 2 + 2).min(texture.width) {
                        let offset = (sy as usize * texture.width as usize + sx as usize) * 4;
                        for (c, channel) in sum.iter_mut().enumerate() {
                            *channel += texture.rgba8[offset + c] as u32;
                        }
                        count += 1;
                    }
                }
                let offset = (y as usize * width as usize + x as usize) * 4;
                for (c, channel) in sum.into_iter().enumerate() {
                    pixels[offset + c] = ((channel + count / 2) / count) as u8;
                }
            }
        }
        texture.width = width;
        texture.height = height;
        texture.rgba8 = pixels.into();
        total = total - old + texture.rgba8.len();
    }
    Ok(changed
        .into_iter()
        .map(|(index, (w, h))| {
            let texture = &textures[index];
            TextureReduction {
                texture_id: texture.id,
                name: texture.name.clone(),
                original_size: [w, h],
                imported_size: [texture.width, texture.height],
            }
        })
        .collect())
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn repeated_budget_passes_report_each_source_once_with_final_dimensions() {
        let mut report = crate::ImportReport::default();
        let mut maps = vec![Texture {
            id: 1,
            name: "shared name".into(),
            width: 8,
            height: 8,
            rgba8: [20, 80, 160, 128].repeat(64).into(),
        }];
        report.record_texture_reductions(fit_texture_budget(&mut maps, 64).unwrap());
        assert_eq!(maps[0].width, 4);
        maps.push(Texture {
            id: 2,
            name: "shared name".into(),
            width: 8,
            height: 8,
            rgba8: [20, 80, 160, 128].repeat(64).into(),
        });
        report.record_texture_reductions(fit_texture_budget(&mut maps, 32).unwrap());
        report.finish_texture_report(&maps, 32);
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.texture_reductions.len(), 2);
        for (change, texture) in report.texture_reductions.iter().zip(&maps) {
            assert_eq!(change.texture_id, texture.id);
            assert_eq!(change.original_size, [8, 8]);
            assert_eq!(change.imported_size, [2, 2]);
        }
        assert!(report.warnings[0].contains("8×8 → 2×2: 2"));
        assert!(maps.iter().map(|t| t.rgba8.len()).sum::<usize>() <= 32);
        let json = serde_json::to_string(&report).unwrap();
        let restored: crate::ImportReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.texture_reductions, restored.texture_reductions);
    }
    #[test]
    fn fitting_keeps_all_maps_ids_alpha_and_pixel_budget() {
        let mut maps: Vec<_> = (1..=5)
            .map(|id| Texture {
                id,
                name: format!("map{id}"),
                width: 64,
                height: 32,
                rgba8: [20, 80, 160, 128].repeat(64 * 32).into(),
            })
            .collect();
        let warnings = fit_texture_budget(&mut maps, 10_000).unwrap();
        assert!(!warnings.is_empty());
        assert!(maps.iter().map(|t| t.rgba8.len()).sum::<usize>() <= 10_000);
        for (i, texture) in maps.iter().enumerate() {
            assert_eq!(texture.id, i as u64 + 1);
            assert_eq!(texture.width, texture.height * 2);
            assert!(texture
                .rgba8
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [20, 80, 160, 128]));
        }
        assert!(fit_texture_budget(&mut maps, 10_000).unwrap().is_empty());
    }
}
