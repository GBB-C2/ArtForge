//! Shared light color and rectangle orientation for rendering and editor markers.
use crate::{Light, Vec3};
use std::sync::OnceLock;

/// Planck spectrum integrated over 380–780 nm using the piecewise CIE 1931
/// approximation by Wyman, Sloan and Shirley (JCGT 2(2), equation 4).
/// Linear sRGB is gamut-clipped and normalized to unit luminance, so temperature
/// does not silently replace the independent light power or authored color tint.
fn integrate_temperature(kelvin: f64) -> [f32; 3] {
    let mut xyz = [0.; 3];
    for nm in (380..=780).step_by(5) {
        let wavelength = nm as f64;
        let gaussian = |center: f64, left: f64, right: f64| {
            let x = (wavelength - center) * if wavelength < center { left } else { right };
            (-0.5 * x * x).exp()
        };
        let matching = [
            1.056 * gaussian(599.8, 0.0264, 0.0323) + 0.362 * gaussian(442., 0.0624, 0.0374)
                - 0.065 * gaussian(501.1, 0.0490, 0.0382),
            0.821 * gaussian(568.8, 0.0213, 0.0247) + 0.286 * gaussian(530.9, 0.0613, 0.0322),
            1.217 * gaussian(437., 0.0845, 0.0278) + 0.681 * gaussian(459., 0.0385, 0.0725),
        ];
        let meters = wavelength * 1e-9;
        let spectrum = 1. / (meters.powi(5) * (0.014_387_768_77 / (meters * kelvin)).exp_m1());
        for channel in 0..3 {
            xyz[channel] += matching[channel] * spectrum;
        }
    }
    let [x, y, z] = xyz;
    let rgb = [
        (3.2406 * x - 1.5372 * y - 0.4986 * z).max(0.),
        (-0.9689 * x + 1.8758 * y + 0.0415 * z).max(0.),
        (0.0557 * x - 0.2040 * y + 1.0570 * z).max(0.),
    ];
    let luminance = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
    rgb.map(|c| (c / luminance) as f32)
}

/// A small process-wide 50 K table avoids spectral integration on every frame.
pub fn temperature_rgb(kelvin: f32) -> [f32; 3] {
    static TABLE: OnceLock<[[f32; 3]; 221]> = OnceLock::new();
    let table = TABLE
        .get_or_init(|| std::array::from_fn(|i| integrate_temperature(1000. + i as f64 * 50.)));
    let kelvin = if kelvin.is_finite() {
        kelvin.clamp(1000., 12000.)
    } else {
        6500.
    };
    let t = (kelvin - 1000.) / 50.;
    let index = t.floor() as usize;
    std::array::from_fn(|c| {
        table[index][c] * (1. - t.fract()) + table[(index + 1).min(220)][c] * t.fract()
    })
}

impl Light {
    pub fn effective_color(&self) -> [f32; 3] {
        if !self.use_temperature {
            return self.color;
        }
        let temperature = temperature_rgb(self.temperature_kelvin);
        std::array::from_fn(|c| self.color[c] * temperature[c])
    }

    pub fn area_basis(&self) -> (Vec3, Vec3) {
        let normal = self.direction.try_normalize().unwrap_or(Vec3::NEG_Y);
        let helper = if normal.y.abs() <= 0.95 {
            Vec3::Y
        } else {
            Vec3::Z
        };
        let tangent = normal.cross(helper).normalize();
        let bitangent = normal.cross(tangent);
        let (sin, cos) = self.area_rotation.to_radians().sin_cos();
        (
            tangent * cos + bitangent * sin,
            bitangent * cos - tangent * sin,
        )
    }
}
