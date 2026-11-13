use crate::CoreError;
use serde::{Deserialize, Serialize};

/// Self-contained linear radiance panorama. Shared by undo snapshots without copying pixels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvironmentMap {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<[f32; 3]>,
}
impl EnvironmentMap {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.width == 0
            || self.height == 0
            || self.width > 4096
            || self.height > 2048
            || u64::from(self.width) * u64::from(self.height) != self.rgb.len() as u64
            || self
                .rgb
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=65000.0).contains(v))
        {
            return Err(CoreError::Invalid(
                "HDRI: не более 4096×2048, конечные RGB 0–65000".into(),
            ));
        }
        Ok(())
    }
}
