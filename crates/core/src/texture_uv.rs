use serde::{Deserialize, Serialize};

/// KHR_texture_transform's offset + rotation * (uv * scale). Rotation is in
/// radians; negative and zero scales are valid, including mirrored textures.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TextureUvTransform {
    pub offset: [f32; 2],
    pub rotation: f32,
    pub scale: [f32; 2],
}

impl Default for TextureUvTransform {
    fn default() -> Self {
        Self {
            offset: [0.; 2],
            rotation: 0.,
            scale: [1.; 2],
        }
    }
}
impl TextureUvTransform {
    pub fn valid(&self) -> bool {
        self.offset
            .iter()
            .chain(&self.scale)
            .all(|v| v.is_finite() && v.abs() <= 1e6)
            && self.rotation.is_finite()
    }
    /// Affine matrix rows padded to uniform-buffer alignment.
    pub fn rows(&self) -> [[f32; 4]; 2] {
        let (sin, cos) = self.rotation.sin_cos();
        [
            [
                cos * self.scale[0],
                -sin * self.scale[1],
                self.offset[0],
                0.,
            ],
            [sin * self.scale[0], cos * self.scale[1], self.offset[1], 0.],
        ]
    }
    pub fn transform(&self, uv: [f32; 2]) -> [f32; 2] {
        self.rows()
            .map(|row| row[0] * uv[0] + row[1] * uv[1] + row[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_order_rotation_and_reflection_match_the_affine_definition() {
        let mapping = TextureUvTransform {
            offset: [0.1, 0.2],
            rotation: std::f32::consts::FRAC_PI_2,
            scale: [2., -3.],
        };
        let uv = mapping.transform([0.25, 0.5]);
        assert!((uv[0] - 1.6).abs() < 1e-6 && (uv[1] - 0.7).abs() < 1e-6);
        assert_eq!(
            TextureUvTransform::default().transform([0.25, 0.5]),
            [0.25, 0.5]
        );
        assert!(TextureUvTransform {
            scale: [0., -1.],
            ..Default::default()
        }
        .valid());
        assert!(!TextureUvTransform {
            rotation: f32::NAN,
            ..Default::default()
        }
        .valid());
    }
}
