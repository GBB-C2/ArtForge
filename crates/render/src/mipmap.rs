//! Material-aware CPU mipmaps. Filtering encoded sRGB or straight-alpha colors
//! directly darkens distant textures and introduces dark fringes on cutouts.
//! Every reduction integrates the complete source footprint, including odd rows
//! and columns. Normal RGB stores the normalized mean direction; its alpha stores
//! the mean vector's length so later levels retain unresolved normal variation.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MipSemantic {
    ColorSrgb,
    ColorSrgbOpaque,
    LinearData,
    Normal,
}

#[derive(Clone, Debug)]
pub(crate) struct MipLevel {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

// RGBA8 input has only 256 possible channel values. Compute the exact
// conversion once instead of calling powf for every source texel on every frame.
fn srgb_table() -> &'static [f64; 256] {
    static TABLE: std::sync::OnceLock<[f64; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| srgb_to_linear(i as f64 / 255.)))
}

fn linear_to_srgb(value: f64) -> f64 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1. / 2.4) - 0.055
    }
}

fn byte(value: f64) -> u8 {
    (value.clamp(0., 1.) * 255.).round() as u8
}

/// Build all standard GPU levels, including the source level. Normal-map alpha
/// is reserved for the mean-vector length and starts at one; authored normal
/// alpha is not a material opacity channel. Source RGB remains unmodified.
/// Callers must supply a validated nonempty RGBA8 image.
pub(crate) fn generate(
    width: u32,
    height: u32,
    rgba: &[u8],
    semantic: MipSemantic,
) -> Vec<MipLevel> {
    assert!(width > 0 && height > 0, "mipmap source must be nonempty");
    let bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .expect("mipmap source dimensions overflow");
    assert_eq!(
        rgba.len(),
        bytes,
        "mipmap source must contain complete RGBA8 pixels"
    );
    let linear = srgb_table();
    let mut source_rgba = rgba.to_vec();
    if semantic == MipSemantic::Normal {
        for pixel in source_rgba.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
    }
    let mut levels = vec![MipLevel {
        width,
        height,
        rgba: source_rgba,
    }];
    while levels
        .last()
        .is_some_and(|level| level.width > 1 || level.height > 1)
    {
        let source = levels.last().unwrap();
        let width = (source.width / 2).max(1);
        let height = (source.height / 2).max(1);
        let mut rgba = vec![0; width as usize * height as usize * 4];
        let scale_x = source.width as f64 / width as f64;
        let scale_y = source.height as f64 / height as f64;
        for y in 0..height {
            let top = y as f64 * scale_y;
            let bottom = (y + 1) as f64 * scale_y;
            for x in 0..width {
                let left = x as f64 * scale_x;
                let right = (x + 1) as f64 * scale_x;
                let mut sum = [0.; 4];
                let mut weight_sum = 0.;
                for sy in top.floor() as u32..(bottom.ceil() as u32).min(source.height) {
                    let wy = (bottom.min((sy + 1) as f64) - top.max(sy as f64)).max(0.);
                    for sx in left.floor() as u32..(right.ceil() as u32).min(source.width) {
                        let wx = (right.min((sx + 1) as f64) - left.max(sx as f64)).max(0.);
                        let weight = wx * wy;
                        let offset = (sy as usize * source.width as usize + sx as usize) * 4;
                        let pixel = &source.rgba[offset..offset + 4];
                        let alpha = pixel[3] as f64 / 255.;
                        let mut normal = [0.; 3];
                        if semantic == MipSemantic::Normal {
                            normal = std::array::from_fn(|i| pixel[i] as f64 / 255. * 2. - 1.);
                            let length = normal.iter().map(|x| x * x).sum::<f64>().sqrt();
                            if length > 1e-12 {
                                for component in &mut normal {
                                    *component /= length;
                                }
                            } else {
                                normal = [0., 0., 1.];
                            }
                        }
                        for channel in 0..3 {
                            let value = pixel[channel] as f64 / 255.;
                            sum[channel] += weight
                                * match semantic {
                                    MipSemantic::ColorSrgb => linear[pixel[channel] as usize] * alpha,
                                    MipSemantic::ColorSrgbOpaque => linear[pixel[channel] as usize],
                                    MipSemantic::LinearData => value,
                                    // Reconstruct the previous level's first
                                    // moment instead of averaging unit vectors
                                    // and forgetting its unresolved variance.
                                    MipSemantic::Normal => normal[channel] * alpha,
                                };
                        }
                        sum[3] += alpha * weight;
                        weight_sum += weight;
                    }
                }
                let offset = (y as usize * width as usize + x as usize) * 4;
                match semantic {
                    MipSemantic::ColorSrgb => {
                        for channel in 0..3 {
                            rgba[offset + channel] = if sum[3] > 0. {
                                byte(linear_to_srgb(sum[channel] / sum[3]))
                            } else {
                                0
                            };
                        }
                    }
                    MipSemantic::ColorSrgbOpaque => {
                        for channel in 0..3 {
                            rgba[offset + channel] =
                                byte(linear_to_srgb(sum[channel] / weight_sum));
                        }
                    }
                    MipSemantic::LinearData => {
                        for channel in 0..3 {
                            rgba[offset + channel] = byte(sum[channel] / weight_sum);
                        }
                    }
                    MipSemantic::Normal => {
                        let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
                        let normal = if length > 1e-12 {
                            [sum[0] / length, sum[1] / length, sum[2] / length]
                        } else {
                            [0., 0., 1.]
                        };
                        for channel in 0..3 {
                            rgba[offset + channel] = byte(normal[channel] * 0.5 + 0.5);
                        }
                        rgba[offset + 3] = byte(length / weight_sum);
                    }
                }
                if semantic != MipSemantic::Normal {
                    rgba[offset + 3] = byte(sum[3] / weight_sum);
                }
            }
        }
        levels.push(MipLevel {
            width,
            height,
            rgba,
        });
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_lookup_preserves_every_srgb_input_exactly() {
        for (value, &actual) in srgb_table().iter().enumerate() {
            let value = value as f64 / 255.;
            let expected = if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) };
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn emission_and_opaque_color_do_not_use_texture_alpha() {
        let pixels = [255, 0, 0, 0, 255, 0, 0, 0];
        let opaque = generate(2, 1, &pixels, MipSemantic::ColorSrgbOpaque);
        assert_eq!(opaque[1].rgba, vec![255, 0, 0, 0]);
        let cutout = generate(2, 1, &pixels, MipSemantic::ColorSrgb);
        assert_eq!(cutout[1].rgba, vec![0, 0, 0, 0]);
    }

    #[test]
    fn color_filtering_is_linear_light_while_data_remains_linear() {
        let pixels = [0, 0, 0, 255, 255, 255, 255, 255];
        let color = generate(2, 1, &pixels, MipSemantic::ColorSrgb);
        let data = generate(2, 1, &pixels, MipSemantic::LinearData);
        assert_eq!(color[1].rgba, [188, 188, 188, 255]);
        assert_eq!(data[1].rgba, [128, 128, 128, 255]);
    }

    #[test]
    fn transparent_texel_color_does_not_darken_or_tint_a_cutout() {
        // Invisible saturated green must not contaminate an opaque white edge.
        let pixels = [255, 255, 255, 255, 0, 255, 0, 0];
        let levels = generate(2, 1, &pixels, MipSemantic::ColorSrgb);
        assert_eq!(levels[0].rgba, pixels);
        assert_eq!(levels[1].rgba, [255, 255, 255, 128]);
        let invisible = generate(2, 1, &[255, 0, 0, 0, 0, 255, 0, 0], MipSemantic::ColorSrgb);
        assert_eq!(invisible[1].rgba, [0, 0, 0, 0]);
    }

    #[test]
    fn normal_filtering_preserves_direction_and_unit_length() {
        let pixels = [255, 128, 128, 255, 128, 255, 128, 255];
        let levels = generate(2, 1, &pixels, MipSemantic::Normal);
        let pixel = &levels[1].rgba;
        let n = std::array::from_fn::<_, 3, _>(|i| pixel[i] as f64 / 255. * 2. - 1.);
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((length - 1.).abs() < 0.01);
        assert!((n[0] - n[1]).abs() < 0.01);
        assert!(n[0] > 0.7 && n[2].abs() < 0.02);
        let opposite = generate(
            2,
            1,
            &[255, 127, 127, 255, 0, 128, 128, 255],
            MipSemantic::Normal,
        );
        assert_eq!(opposite[1].rgba, [128, 128, 255, 0]);
    }

    #[test]
    fn normal_source_alpha_is_ignored_and_rgb_is_preserved() {
        let pixels = [218, 128, 218, 0, 37, 128, 218, 77];
        let reference = [218, 128, 218, 255, 37, 128, 218, 255];
        let levels = generate(2, 1, &pixels, MipSemantic::Normal);
        let expected = generate(2, 1, &reference, MipSemantic::Normal);
        assert_eq!(levels[0].rgba, reference);
        for (actual, expected) in levels.iter().zip(expected) {
            assert_eq!(actual.rgba, expected.rgba);
        }
    }

    #[test]
    fn uniform_normal_maps_keep_full_mean_length_at_every_level() {
        let pixels = [218, 91, 208, 0].repeat(9 * 5);
        let levels = generate(9, 5, &pixels, MipSemantic::Normal);
        for level in levels {
            assert!(level
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255));
        }
    }

    #[test]
    fn normal_first_moment_survives_successive_mip_reductions() {
        // Alternating +/-45-degree normals have an unresolved mean length of
        // roughly cos(45). Coarser mips must not replace it with unit length.
        let pixels = [218, 128, 218, 255, 37, 128, 218, 255].repeat(8 * 16 / 2);
        let levels = generate(16, 8, &pixels, MipSemantic::Normal);
        for level in &levels[1..] {
            for pixel in level.rgba.as_chunks::<4>().0 {
                assert!((pixel[3] as i32 - 180).abs() <= 2, "{}", pixel[3]);
                assert!((pixel[0] as i32 - 128).abs() <= 1);
                assert!(pixel[2] >= 254);
            }
        }

        // A larger footprint can cancel coherent child directions, too.
        let opposite_blocks = [
            255, 127, 127, 255, 255, 127, 127, 255, 0, 128, 128, 255, 0, 128, 128, 255,
        ];
        let cancellation = generate(4, 1, &opposite_blocks, MipSemantic::Normal);
        assert_eq!(cancellation[1].rgba[3], 255);
        assert_eq!(cancellation[1].rgba[7], 255);
        assert_eq!(cancellation[2].rgba, [128, 128, 255, 0]);
    }

    #[test]
    fn odd_dimensions_include_last_rows_and_columns() {
        let mut pixels = vec![0; 3 * 3 * 4];
        for pixel in pixels.chunks_mut(4) {
            pixel[3] = 255;
        }
        pixels[8 * 4] = 255;
        let levels = generate(3, 3, &pixels, MipSemantic::LinearData);
        assert_eq!(levels[1].rgba, [28, 0, 0, 255]);

        let mut pixels = vec![0; 5 * 3 * 4];
        for (index, pixel) in pixels.chunks_mut(4).enumerate() {
            pixel[0] = (index * 17) as u8;
            pixel[3] = 255;
        }
        let levels = generate(5, 3, &pixels, MipSemantic::LinearData);
        assert!((levels.last().unwrap().rgba[0] as i32 - 119).abs() <= 1);
    }

    #[test]
    fn non_square_chain_uses_exact_gpu_dimensions_and_preserves_constants() {
        let pixels = [25, 80, 180, 133].repeat(7 * 3);
        let levels = generate(7, 3, &pixels, MipSemantic::ColorSrgb);
        let dimensions: Vec<_> = levels
            .iter()
            .map(|level| (level.width, level.height))
            .collect();
        assert_eq!(dimensions, [(7, 3), (3, 1), (1, 1)]);
        for level in &levels {
            assert_eq!(
                level.rgba.len(),
                level.width as usize * level.height as usize * 4
            );
            assert!(level
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [25, 80, 180, 133]));
        }
        let single = generate(1, 1, &[1, 2, 3, 4], MipSemantic::LinearData);
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].rgba, [1, 2, 3, 4]);
    }
}
