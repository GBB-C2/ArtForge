//! Authored non-photorealistic surface response. Colors are scene-linear RGB.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NprSettings {
    pub enabled: bool,
    pub light_wrap: f32,
    pub light_color: f32,
    pub halftone_strength: f32,
    pub halftone_density: f32,
    pub bands: u32,
    /// Midpoint of the lighting ramp, in normalized irradiance.
    pub threshold: f32,
    pub softness: f32,
    pub shadow_color: [f32; 3],
    pub specular_size: f32,
    pub specular_strength: f32,
    pub rim_color: [f32; 3],
    pub rim_strength: f32,
    pub rim_width: f32,
    pub ink_color: [f32; 3],
    /// Grazing-angle surface ink; does not expand the mesh silhouette.
    pub ink_width: f32,
    pub palette_strength: f32,
    pub palette_mid: [f32; 3],
    pub palette_light: [f32; 3],
    pub palette_position: f32,
    pub hatch_strength: f32,
    /// Lines per world metre; stable while navigating the camera.
    pub hatch_density: f32,
    pub hatch_angle: f32,
    pub hatch_cross: f32,
}

impl Default for NprSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            light_wrap: 0., light_color: 1., halftone_strength: 0., halftone_density: 24.,
            bands: 3,
            threshold: 0.5,
            softness: 0.015,
            shadow_color: [0.12, 0.16, 0.28],
            specular_size: 0.08,
            specular_strength: 0.35,
            rim_color: [0.65, 0.82, 1.],
            rim_strength: 0.15,
            rim_width: 0.2,
            ink_color: [0.008, 0.012, 0.025],
            ink_width: 0.,
            palette_strength: 0.,
            palette_mid: [0.45, 0.5, 0.65],
            palette_light: [1.; 3],
            palette_position: 0.5,
            hatch_strength: 0.,
            hatch_density: 20.,
            hatch_angle: 45.,
            hatch_cross: 0.,
        }
    }
}

impl NprSettings {
    pub fn valid(&self) -> bool {
        let scalar = |v: f32, min: f32, max: f32| v.is_finite() && (min..=max).contains(&v);
        scalar(self.light_wrap, 0., 1.) && scalar(self.light_color, 0., 1.)
            && scalar(self.halftone_strength, 0., 1.) && scalar(self.halftone_density, 1., 200.)
            && (2..=8).contains(&self.bands)
            && scalar(self.threshold, 0.05, 0.95)
            && scalar(self.softness, 0., 0.25)
            && scalar(self.specular_size, 0.01, 0.5)
            && scalar(self.specular_strength, 0., 2.)
            && scalar(self.rim_strength, 0., 2.)
            && scalar(self.rim_width, 0.01, 1.)
            && scalar(self.ink_width, 0., 0.5)
            && scalar(self.palette_strength, 0., 1.)
            && scalar(self.palette_position, 0.05, 0.95)
            && scalar(self.hatch_strength, 0., 1.)
            && scalar(self.hatch_density, 1., 200.)
            && scalar(self.hatch_angle, -180., 180.)
            && scalar(self.hatch_cross, 0., 1.)
            && self
                .shadow_color
                .iter()
                .chain(&self.rim_color)
                .chain(&self.ink_color)
                .chain(&self.palette_mid)
                .chain(&self.palette_light)
                .all(|v| scalar(*v, 0., 1.))
    }
}

/// Scene-wide contours of opaque/masked geometry. Normals are reconstructed from
/// depth, so this detects geometric creases, independently of smooth shading.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NprContours {
    /// Physical output pixels. Zero disables the additional depth pass.
    pub width: f32,
    pub depth_threshold: f32,
    pub normal_angle: f32,
    pub color: [f32; 3],
}
impl Default for NprContours {
    fn default() -> Self {
        Self {
            width: 0.,
            depth_threshold: 0.01,
            normal_angle: 45.,
            color: [0.008, 0.012, 0.025],
        }
    }
}
impl NprContours {
    pub fn valid(&self) -> bool {
        let finite = |v: f32, a: f32, b: f32| v.is_finite() && (a..=b).contains(&v);
        finite(self.width, 0., 8.)
            && finite(self.depth_threshold, 0.0001, 1.)
            && finite(self.normal_angle, 1., 179.)
            && self.color.iter().all(|&c| finite(c, 0., 1.))
    }
}

impl NprSettings {
    /// Style presets replace NPR response only, retaining PBR colors and maps.
    pub fn preset(index: u8) -> Option<Self> {
        let mut n = Self { enabled: true, ..Default::default() };
        match index {
            0 => { n.bands = 2; n.softness = 0.005; n.specular_strength = 0.6; n.rim_strength = 0.2; }
            1 => { n.bands = 5; n.softness = 0.09; n.light_wrap = 0.35; n.shadow_color = [0.32,0.25,0.35]; n.specular_strength = 0.12; }
            2 => { n.bands = 3; n.hatch_strength = 0.8; n.hatch_cross = 0.7; n.hatch_density = 32.; n.ink_width = 0.06; n.light_color = 0.3; n.specular_strength = 0.1; }
            3 => { n.bands = 3; n.halftone_strength = 0.85; n.halftone_density = 28.; n.ink_width = 0.04; n.light_color = 0.6; n.palette_strength = 0.3; }
            _ => return None,
        }
        Some(n)
    }
}
