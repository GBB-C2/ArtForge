//! Editable cable rest shape. Physics evolves the centreline, never the tube's cross-section.
use crate::{CoreError, CurveData, CurveGeometry, CurveInterpolation, Mesh, Vec3};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CableSettings {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
    pub segments: u32,
    pub sides: u32,
    pub sag: f32,
    pub pin_start: bool,
    pub pin_end: bool,
}
impl Default for CableSettings {
    fn default() -> Self {
        Self {
            start: [-1., 2., 0.],
            end: [1., 2., 0.],
            radius: 0.04,
            segments: 48,
            sides: 8,
            sag: 0.25,
            pin_start: true,
            pin_end: true,
        }
    }
}
impl CableSettings {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self
            .start
            .iter()
            .chain(&self.end)
            .any(|v| !v.is_finite() || v.abs() > 10000.)
            || Vec3::from_array(self.start).distance(Vec3::from_array(self.end)) < 0.01
            || !self.radius.is_finite()
            || !(0.001..=0.5).contains(&self.radius)
            || !(4..=128).contains(&self.segments)
            || !(3..=16).contains(&self.sides)
            || !self.sag.is_finite()
            || !(0. ..=2.).contains(&self.sag)
        {
            return Err(CoreError::Invalid(
                "Кабель: разнесите концы, радиус 0,001–0,5 м; 4–128 сегментов; провисание 0–200%"
                    .into(),
            ));
        }
        Ok(())
    }
    pub fn centers(&self) -> Result<Vec<Vec3>, CoreError> {
        self.validate()?;
        let a = Vec3::from_array(self.start);
        let b = Vec3::from_array(self.end);
        let length = a.distance(b);
        Ok((0..=self.segments)
            .map(|i| {
                let t = i as f32 / self.segments as f32;
                a.lerp(b, t) - Vec3::Y * (length * self.sag * 4. * t * (1. - t))
            })
            .collect())
    }
    pub fn mesh(&self, id: u64, material: Option<u64>) -> Result<Mesh, CoreError> {
        self.mesh_at(&self.centers()?, id, material)
    }
    pub fn mesh_at(
        &self,
        points: &[Vec3],
        id: u64,
        material: Option<u64>,
    ) -> Result<Mesh, CoreError> {
        self.validate()?;
        if points.len() != self.segments as usize + 1 {
            return Err(CoreError::Invalid(
                "Кабель: изменилось число физических сегментов".into(),
            ));
        }
        let c = CurveData {
            points: points.iter().map(|v| v.to_array()).collect(),
            interpolation: CurveInterpolation::Polyline,
            closed: false,
            resolution: 1,
            geometry: Some(CurveGeometry {
                radius: self.radius,
                sides: self.sides,
                ..Default::default()
            }),
        };
        let mut m = c.mesh(id, material)?;
        m.name = "Кабель".into();
        Ok(m)
    }
}
