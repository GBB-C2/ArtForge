//! Persistent artwork controls. Runtime simulation state never enters the document.
use crate::{CoreError, Mesh, Project, Property, Transform, Vertex};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackgroundMode {
    #[default]
    Theme,
    Solid,
    Linear,
    Radial,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Atmosphere {
    pub density: f32,
    pub height: f32,
    pub distance: f32,
    pub anisotropy: f32,
    pub color: [f32; 3],
}
impl Default for Atmosphere {
    fn default() -> Self {
        Self {
            density: 0.,
            height: 8.,
            distance: 30.,
            anisotropy: 0.2,
            color: [0.8, 0.88, 1.],
        }
    }
}
impl Atmosphere {
    pub fn validate(&self) -> Result<(), CoreError> {
        for (v, a, b) in [
            (self.density, 0., 1.),
            (self.height, 0.1, 1000.),
            (self.distance, 1., 500.),
            (self.anisotropy, -0.9, 0.9),
        ] {
            if !v.is_finite() || !(a..=b).contains(&v) {
                return Err(CoreError::Invalid("параметры атмосферы".into()));
            }
        }
        if self
            .color
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(CoreError::Invalid("цвет атмосферы".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhysicsKind {
    Collider,
    Cloth,
    Muscle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PhysicsSettings {
    pub visible: bool,
    pub self_collision: bool,
    /// Separate bend compliance multiplier and tangential contact friction.
    pub bending: f32,
    pub friction: f32,
    pub iterations: u32,
    pub kind: PhysicsKind,
    pub compliance: f32,
    pub damping: f32,
    pub gravity: f32,
    pub wind: f32,
    pub thickness: f32,
    /// Cloth: pin the highest row. Muscle: pin the lowest ring.
    pub pinned: bool,
    pub activation: f32,
    pub contraction: f32,
}
impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            kind: PhysicsKind::Cloth,
            visible: true,
            self_collision: false,
            bending: 8.,
            friction: 0.35,
            iterations: 8,
            compliance: 0.00001,
            damping: 0.03,
            gravity: 9.81,
            wind: 0.,
            thickness: 0.015,
            pinned: true,
            activation: 0.,
            contraction: 0.3,
        }
    }
}
impl PhysicsSettings {
    pub fn validate(&self) -> Result<(), CoreError> {
        for (v, a, b) in [
            (self.bending, 1., 100.),
            (self.friction, 0., 1.),
            (self.compliance, 0., 0.01),
            (self.damping, 0., 1.),
            (self.gravity, 0., 30.),
            (self.wind, -30., 30.),
            (self.thickness, 0.001, 0.5),
            (self.activation, 0., 1.),
            (self.contraction, 0., 0.6),
        ] {
            if !v.is_finite() || !(a..=b).contains(&v) {
                return Err(CoreError::Invalid("параметры физики".into()));
            }
        }
        if !(1..=24).contains(&self.iterations) {
            return Err(CoreError::Invalid("итерации физики: 1–24".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimationLayer {
    pub name: String,
    pub clip: usize,
    pub weight: f32,
    pub additive: bool,
    pub enabled: bool,
    pub solo: bool,
}

/// Ordered layer stack; additive values are relative to the source at t=0.
pub(crate) fn apply_layers(
    project: &Project,
    time: f64,
    local: &mut BTreeMap<u64, Transform>,
    morphs: &mut BTreeMap<u64, Vec<f32>>,
) -> Result<(), CoreError> {
    let solo = project.animation_layers.iter().any(|l| l.enabled && l.solo);
    for layer in &project.animation_layers {
        if !layer.enabled || layer.weight == 0. || (solo && !layer.solo) {
            continue;
        }
        let clip = project
            .clips
            .get(layer.clip)
            .ok_or_else(|| CoreError::Invalid("клип слоя отсутствует".into()))?;
        let w = layer.weight;
        for channel in &clip.channels {
            let Some(value) = channel.sample(time) else {
                continue;
            };
            let reference = channel.sample(0.).unwrap_or_else(|| value.clone());
            let t = local
                .get_mut(&channel.node)
                .ok_or(CoreError::MissingNode(channel.node))?;
            match channel.property {
                Property::Translation => {
                    let v = Vec3::from_slice(&value);
                    t.translation = if layer.additive {
                        t.translation + (v - Vec3::from_slice(&reference)) * w
                    } else {
                        t.translation.lerp(v, w)
                    };
                }
                Property::Scale => {
                    let v = Vec3::from_slice(&value);
                    t.scale = if layer.additive {
                        t.scale * Vec3::ONE.lerp(v / Vec3::from_slice(&reference), w)
                    } else {
                        t.scale.lerp(v, w)
                    };
                }
                Property::Rotation => {
                    let q = Quat::from_array(value[..4].try_into().unwrap()).normalize();
                    t.rotation = if layer.additive {
                        let r = Quat::from_array(reference[..4].try_into().unwrap()).normalize();
                        (t.rotation * Quat::IDENTITY.slerp(r.conjugate() * q, w)).normalize()
                    } else {
                        t.rotation.slerp(q, w).normalize()
                    };
                }
                Property::Weights => {
                    let current = morphs.entry(channel.node).or_default();
                    current.resize(value.len(), 0.);
                    for i in 0..value.len() {
                        current[i] = if layer.additive {
                            current[i] + (value[i] - reference[i]) * w
                        } else {
                            current[i] + (value[i] - current[i]) * w
                        };
                    }
                }
            }
        }
    }
    Ok(())
}

/// Indexed XY cloth or closed contractile cylinder; vertices are solver particles.
pub fn physics_mesh(id: u64, material: u64, muscle: bool) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let (cols, rows) = if muscle { (12, 20) } else { (20, 20) };
    for y in 0..=rows {
        for x in 0..=cols {
            let u = x as f32 / cols as f32;
            let v = y as f32 / rows as f32;
            let (p, n) = if muscle {
                let angle = u * std::f32::consts::TAU;
                let radius = 0.06 + 0.17 * (v * std::f32::consts::PI).sin();
                (
                    Vec3::new(radius * angle.cos(), v * 1.6, radius * angle.sin()),
                    Vec3::new(angle.cos(), 0., angle.sin()),
                )
            } else {
                (Vec3::new((u - 0.5) * 2., v * 2., 0.), Vec3::Z)
            };
            vertices.push(Vertex {
                position: p.to_array(),
                normal: n.to_array(),
                uv: [u, v],
                ..Vertex::default()
            });
        }
    }
    for y in 0..rows {
        for x in 0..cols {
            let a = (y * (cols + 1) + x) as u32;
            let b = a + (cols + 1) as u32;
            if muscle {
                indices.extend([a, b, a + 1, a + 1, b, b + 1]);
            } else {
                indices.extend([a, a + 1, b, a + 1, b + 1, b]);
            }
        }
    }
    if muscle {
        for top in [false, true] {
            let center = vertices.len() as u32;
            vertices.push(Vertex {
                position: [0., if top { 1.6 } else { 0. }, 0.],
                normal: [0., if top { 1. } else { -1. }, 0.],
                ..Vertex::default()
            });
            let base = if top { rows * (cols + 1) } else { 0 } as u32;
            for x in 0..cols as u32 {
                if top {
                    indices.extend([center, base + x + 1, base + x]);
                } else {
                    indices.extend([center, base + x, base + x + 1]);
                }
            }
        }
    }
    Mesh {
        id,
        name: if muscle { "Мышца" } else { "Ткань" }.into(),
        vertices: vertices.into(),
        indices: indices.into(),
        material: Some(material),
        topology: None,
        morph_targets: vec![],
    }
}
