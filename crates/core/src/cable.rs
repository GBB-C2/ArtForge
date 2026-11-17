//! Parametric cable profiles. Physics evolves the centreline and preserves each cross-section.
use crate::{CoreError, Mesh, MeshTopology, Quat, Vec3, Vertex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum CableKind {
    #[default]
    Round,
    Ribbon,
    Twisted,
    Coiled,
    Corrugated,
    Bundle,
}
impl CableKind {
    pub const ALL: [Self; 6] = [
        Self::Round,
        Self::Ribbon,
        Self::Twisted,
        Self::Coiled,
        Self::Corrugated,
        Self::Bundle,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Round => "Обычный",
            Self::Ribbon => "Плоский",
            Self::Twisted => "Витой",
            Self::Coiled => "Спиральный",
            Self::Corrugated => "Гофрированный",
            Self::Bundle => "Пучок проводов",
        }
    }
}
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
    pub kind: CableKind,
    pub strands: u32,
    pub turns: f32,
    pub spread: f32,
    pub ribs: u32,
    pub groove: f32,
    pub width: f32,
    pub thickness: f32,
    pub rotation: f32,
    pub taper: f32,
    pub caps: bool,
    pub uv_repeat: f32,
    pub collision_margin: f32,
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
            kind: CableKind::Round,
            strands: 3,
            turns: 6.,
            spread: 0.1,
            ribs: 24,
            groove: 0.35,
            width: 0.2,
            thickness: 0.02,
            rotation: 0.,
            taper: 1.,
            caps: true,
            uv_repeat: 1.,
            collision_margin: 0.,
        }
    }
}
fn invalid(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
fn frames(points: &[Vec3]) -> Result<Vec<(Vec3, Vec3, Vec3)>, CoreError> {
    let mut result = Vec::with_capacity(points.len());
    let mut frame = None;
    for i in 0..points.len() {
        let before = points[i.saturating_sub(1)];
        let after = points[(i + 1).min(points.len() - 1)];
        let tangent = (after - before)
            .try_normalize()
            .or_else(|| (after - points[i]).try_normalize())
            .or_else(|| (points[i] - before).try_normalize())
            .ok_or_else(|| invalid("Кабель: совпавшие точки центральной линии"))?;
        let previous = frame.unwrap_or_else(|| tangent.any_orthonormal_vector());
        let normal = (previous - tangent * previous.dot(tangent))
            .try_normalize()
            .unwrap_or_else(|| tangent.any_orthonormal_vector());
        frame = Some(normal);
        result.push((normal, tangent.cross(normal), tangent));
    }
    Ok(result)
}
impl CableSettings {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self
            .start
            .iter()
            .chain(&self.end)
            .any(|v| !v.is_finite() || v.abs() > 10000.)
            || Vec3::from_array(self.start).distance(Vec3::from_array(self.end)) < 0.01
        {
            return Err(invalid("Кабель: разнесите концы минимум на 0,01 м"));
        }
        for (value, min, max) in [
            (self.radius, 0.001, 0.5),
            (self.sag, 0., 2.),
            (self.turns, -32., 32.),
            (self.spread, 0.002, 1.),
            (self.groove, 0., 0.9),
            (self.width, 0.002, 2.),
            (self.thickness, 0.001, 0.5),
            (self.rotation, -360., 360.),
            (self.taper, 0.05, 2.),
            (self.uv_repeat, 0.1, 100.),
            (self.collision_margin, 0., 0.1),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(invalid("Параметры кабеля вне допустимого диапазона"));
            }
        }
        if !(4..=256).contains(&self.segments)
            || !(3..=16).contains(&self.sides)
            || !(2..=6).contains(&self.strands)
            || !(1..=64).contains(&self.ribs)
        {
            return Err(invalid(
                "Кабель: 4–256 сегментов, 3–16 сторон, 2–6 жил, 1–64 кольца",
            ));
        }
        Ok(())
    }
    /// Keep at least twelve samples per turn and eight per corrugation to avoid aliasing.
    pub fn sample_segments(&self) -> usize {
        self.segments
            .max(match self.kind {
                CableKind::Twisted | CableKind::Coiled => (self.turns.abs() * 12.).ceil() as u32,
                CableKind::Corrugated => self.ribs.saturating_mul(8),
                _ => 0,
            })
            .min(512) as usize
    }
    pub fn collision_radius(&self) -> f32 {
        let r = if self.kind == CableKind::Ribbon {
            self.width.max(self.thickness) * 0.5
        } else {
            self.radius
        };
        let envelope = if matches!(self.kind, CableKind::Twisted | CableKind::Bundle) {
            self.spread
        } else {
            0.
        };
        r * self.taper.max(1.) + envelope + self.collision_margin
    }
    pub fn centers(&self) -> Result<Vec<Vec3>, CoreError> {
        self.validate()?;
        let a = Vec3::from_array(self.start);
        let b = Vec3::from_array(self.end);
        let length = a.distance(b);
        let count = self.sample_segments();
        let mut points: Vec<_> = (0..=count)
            .map(|i| {
                let t = i as f32 / count as f32;
                a.lerp(b, t) - Vec3::Y * (length * self.sag * 4. * t * (1. - t))
            })
            .collect();
        if self.kind == CableKind::Coiled && self.turns.abs() > 1e-6 {
            let basis = frames(&points)?;
            for (i, p) in points.iter_mut().enumerate() {
                let t = i as f32 / count as f32;
                let fade = (t.min(1. - t) * 10.).clamp(0., 1.);
                let fade = fade * fade * (3. - 2. * fade);
                let angle = self.rotation.to_radians() + std::f32::consts::TAU * self.turns * t;
                *p += (basis[i].0 * angle.cos() + basis[i].1 * angle.sin()) * self.spread * fade;
            }
        }
        Ok(points)
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
        if points.len() != self.sample_segments() + 1 || points.iter().any(|p| !p.is_finite()) {
            return Err(invalid("Кабель: некорректная центральная линия"));
        }
        let strands = if matches!(self.kind, CableKind::Twisted | CableKind::Bundle) {
            self.strands
        } else {
            1
        };
        let basis = frames(points)?;
        let mut result = None;
        for strand in 0..strands {
            let path: Vec<_> = points
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    if strands == 1 {
                        return p;
                    }
                    let t = i as f32 / (points.len() - 1) as f32;
                    let phase = std::f32::consts::TAU
                        * (strand as f32 / strands as f32
                            + if self.kind == CableKind::Twisted {
                                self.turns * t
                            } else {
                                0.
                            })
                        + self.rotation.to_radians();
                    p + (basis[i].0 * phase.cos() + basis[i].1 * phase.sin()) * self.spread
                })
                .collect();
            let mut mesh = self.tube(&path, id, material)?;
            if let Some(m) = &mut result {
                let m: &mut Mesh = m;
                let offset = m.vertices.len() as u32;
                let faces = mesh.topology.take().unwrap().faces;
                for face in faces.iter() {
                    m.topology
                        .as_mut()
                        .unwrap()
                        .faces
                        .push(face.iter().map(|v| v + offset).collect());
                }
                m.vertices.extend(mesh.vertices.iter().copied());
            } else {
                result = Some(mesh);
            }
        }
        let mut mesh = result.unwrap();
        if mesh.vertices.len() > crate::MAX_EDIT_VERTICES {
            return Err(invalid(
                "Кабель слишком подробный: уменьшите стороны или жилы",
            ));
        }
        if strands > 1 {
            mesh.rebuild_polygon_indices()?;
        }
        mesh.name = format!("Кабель · {}", self.kind.label());
        mesh.validate_topology()?;
        Ok(mesh)
    }
    fn tube(&self, points: &[Vec3], id: u64, material: Option<u64>) -> Result<Mesh, CoreError> {
        let sides = self.sides as usize;
        let rings = points.len();
        let stride = sides + 1;
        let basis = frames(points)?;
        let mut lengths = vec![0.; rings];
        for i in 1..rings {
            lengths[i] = lengths[i - 1] + points[i].distance(points[i - 1]);
        }
        let mut mesh = Mesh {
            id,
            name: "Кабель".into(),
            material,
            vertices: Default::default(),
            indices: Default::default(),
            morph_targets: vec![],
            topology: Some(MeshTopology {
                faces: Default::default(),
                face_materials: Default::default(),
                loose_edges: vec![],
                uv_seams: vec![],
                primitive: None,
                smooth_normals: true,
            }),
        };
        for i in 0..rings {
            let n = Quat::from_axis_angle(basis[i].2, self.rotation.to_radians()) * basis[i].0;
            let b = basis[i].2.cross(n);
            for j in 0..=sides {
                let angle = if j == sides {
                    0.
                } else {
                    std::f32::consts::TAU * j as f32 / sides as f32
                };
                let radial = n * angle.cos() + b * angle.sin();
                mesh.vertices.push(Vertex {
                    position: (points[i] + radial * self.radius).to_array(),
                    normal: radial.to_array(),
                    uv: [
                        lengths[i] / lengths[rings - 1].max(1e-8),
                        j as f32 / sides as f32,
                    ],
                    ..Default::default()
                });
            }
        }
        for i in 0..rings - 1 {
            for j in 0..sides {
                let a = (i * stride + j) as u32;
                let b = a + stride as u32;
                mesh.topology
                    .as_mut()
                    .unwrap()
                    .faces
                    .push(vec![a, a + 1, b + 1, b]);
            }
        }
        if self.caps {
            for end in [0, rings - 1] {
                let base = mesh.vertices.len() as u32;
                for j in 0..sides {
                    let mut v = mesh.vertices[end * stride + j];
                    v.normal = (basis[end].2 * if end == 0 { -1. } else { 1. }).to_array();
                    let angle = std::f32::consts::TAU * j as f32 / sides as f32;
                    v.uv = [0.5 + 0.5 * angle.cos(), 0.5 + 0.5 * angle.sin()];
                    mesh.vertices.push(v);
                }
                let mut face: Vec<_> = (base..base + sides as u32).collect();
                if end == 0 {
                    face.reverse();
                }
                mesh.topology.as_mut().unwrap().faces.push(face);
            }
        }
        mesh.rebuild_polygon_indices()?;
        let changed = self.kind == CableKind::Ribbon
            || self.kind == CableKind::Corrugated
            || self.taper != 1.;
        if changed {
            let basis = frames(points)?;
            for i in 0..rings {
                let t = i as f32 / (rings - 1) as f32;
                let taper = 1. + (self.taper - 1.) * t;
                let radius = self.radius
                    * taper
                    * if self.kind == CableKind::Corrugated {
                        1. - self.groove
                            * 0.5
                            * (1. - (std::f32::consts::TAU * self.ribs as f32 * t).cos())
                    } else {
                        1.
                    };
                let n = Quat::from_axis_angle(basis[i].2, self.rotation.to_radians()) * basis[i].0;
                let b = basis[i].2.cross(n);
                for j in 0..=sides {
                    let angle = if j == sides {
                        0.
                    } else {
                        std::f32::consts::TAU * j as f32 / sides as f32
                    };
                    let (rx, ry) = if self.kind == CableKind::Ribbon {
                        (self.width * 0.5 * taper, self.thickness * 0.5 * taper)
                    } else {
                        (radius, radius)
                    };
                    mesh.vertices[i * stride + j].position =
                        (points[i] + n * (rx * angle.cos()) + b * (ry * angle.sin())).to_array();
                }
            }
            if self.caps {
                for (cap, ring) in [(0, 0), (1, rings - 1)] {
                    for j in 0..sides {
                        mesh.vertices[rings * stride + cap * sides + j].position =
                            mesh.vertices[ring * stride + j].position;
                    }
                }
            }
            crate::recompute_normals(&mut mesh, true);
        }
        for v in mesh.vertices.iter_mut().take(rings * stride) {
            v.uv[0] *= self.uv_repeat;
        }
        Ok(mesh)
    }
}
