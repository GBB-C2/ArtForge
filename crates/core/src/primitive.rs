//! Bounded parametric primitives with persistent, all-quad authoring topology.
use crate::{CoreError, Mesh, MeshTopology, Vec3, Vertex};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrimitiveKind {
    Cube,
    Plane,
    Sphere,
    Cloth,
    Fox,
    Bezier,
    CurveCircle,
    Cylinder, Cone, Torus, Capsule, Pyramid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PrimitiveSettings {
    pub kind: PrimitiveKind,
    pub size: [f32; 3],
    pub segments: [u32; 3],
    pub radius: f32,
    pub smooth: bool,
    #[serde(default = "default_curve_points")]
    pub curve_points: [[f32; 3]; 4],
}
fn default_curve_points() -> [[f32; 3]; 4] {
    [[-1., 0., 0.], [-0.5, 1., 0.], [0.5, -1., 0.], [1., 0., 0.]]
}
impl PrimitiveKind {
    pub const MESH_MENU:[(u8,Self);8]=[(0,Self::Cube),(1,Self::Plane),(2,Self::Sphere),(7,Self::Cylinder),(8,Self::Cone),(9,Self::Torus),(10,Self::Capsule),(4,Self::Fox)];
    pub fn from_code(code:u8)->Option<Self> {match code {3=>Some(Self::Cloth),5=>Some(Self::Bezier),6=>Some(Self::CurveCircle),_=>Self::MESH_MENU.into_iter().find(|(i,_)|*i==code).map(|(_,kind)|kind)}}
    pub fn extended(self)->bool {matches!(self,Self::Cylinder|Self::Cone|Self::Torus|Self::Capsule|Self::Pyramid)}
}
impl PrimitiveSettings {
    pub fn is_curve(self) -> bool { matches!(self.kind, PrimitiveKind::Bezier | PrimitiveKind::CurveCircle) }

    pub fn new(kind: PrimitiveKind) -> Self {
        Self {
            kind,
            size: if kind == PrimitiveKind::Torus {[0.18,2.,2.]} else if kind == PrimitiveKind::Fox {
                [1.6, 2., 1.6]
            } else {
                [2.; 3]
            },
            segments: match kind {
                PrimitiveKind::Sphere => [12; 3],
                PrimitiveKind::Cylinder | PrimitiveKind::Cone => [32,1,1],
                PrimitiveKind::Torus | PrimitiveKind::Capsule => [32,12,1],
                PrimitiveKind::Cloth => [16, 16, 1],
                PrimitiveKind::Fox => [3, 1, 1],
                PrimitiveKind::Bezier | PrimitiveKind::CurveCircle => [32, 8, 1],
                _ => [1; 3],
            },
            radius: if matches!(kind, PrimitiveKind::Bezier | PrimitiveKind::CurveCircle) { 0.035 } else { 0.5 },
            smooth: matches!(kind, PrimitiveKind::Sphere | PrimitiveKind::Fox | PrimitiveKind::Bezier | PrimitiveKind::CurveCircle | PrimitiveKind::Cylinder | PrimitiveKind::Cone | PrimitiveKind::Torus | PrimitiveKind::Capsule),
            curve_points: default_curve_points(),
        }
    }
    pub fn label(self) -> &'static str {
        match self.kind {
            PrimitiveKind::Cylinder=>"Цилиндр",PrimitiveKind::Cone=>"Конус",PrimitiveKind::Torus=>"Тор",PrimitiveKind::Capsule=>"Капсула",PrimitiveKind::Pyramid=>"Пирамида",
            PrimitiveKind::Cube => "Куб",
            PrimitiveKind::Plane => "Плоскость",
            PrimitiveKind::Sphere => "Сфера",
            PrimitiveKind::Cloth => "Ткань",
            PrimitiveKind::Fox => "Лиса",
            PrimitiveKind::Bezier => "Кривая Безье",
            PrimitiveKind::CurveCircle => "Кривая · окружность",
        }
    }
    pub fn validate(self) -> Result<(), CoreError> {
        if self.kind.extended() && self.kind!=PrimitiveKind::Pyramid && (self.segments[0]<3 || (matches!(self.kind,PrimitiveKind::Torus|PrimitiveKind::Capsule) && self.segments[1]<3)) {
            return Err(CoreError::Invalid("Окружность и скругление: минимум 3 сегмента".into()));
        }
        if self.kind==PrimitiveKind::Torus && self.size[0]>=self.radius {return Err(CoreError::Invalid("Радиус трубки должен быть меньше радиуса кольца".into()));}
        if self.kind==PrimitiveKind::Capsule && self.size[1]<2.*self.radius {return Err(CoreError::Invalid("Высота капсулы должна быть не меньше диаметра".into()));}
        if self.is_curve() && (self.segments[0] < 3 || self.segments[1] < 3
            || self.curve_points.iter().flatten().any(|v| !v.is_finite() || v.abs() > 10000.)) {
            return Err(CoreError::Invalid("Кривая: минимум 3 сегмента; координаты точек от −10000 до 10000 м".into()));
        }
        if self
            .size
            .iter()
            .any(|v| !v.is_finite() || !(0.001..=10000.).contains(v))
            || self
                .segments
                .iter()
                .any(|v| !(1..=self.max_segments()).contains(v))
            || !self.radius.is_finite()
            || !(0.001..=10000.).contains(&self.radius)
        {
            return Err(CoreError::Invalid(format!(
                "Размер: 0,001–10000 м; подразделения: 1–{}",
                self.max_segments()
            )));
        }
        Ok(())
    }
    pub fn mesh(self, id: u64, material: Option<u64>) -> Result<Mesh, CoreError> {
        self.validate()?;
        let mut mesh = Mesh {
            id,
            name: self.label().into(),
            material,
            vertices: Default::default(),
            indices: Default::default(),
            morph_targets: vec![],
            topology: Some(MeshTopology {
                face_materials: Default::default(),
                uv_seams: Vec::new(),
                faces: Default::default(),
                smooth_normals: self.smooth,
                primitive: Some(self),
            }),
        };
        if self.kind.extended() {crate::advanced_primitives::build(&mut mesh,self)?;return Ok(mesh);}
        if self.is_curve() {
            crate::curve_primitive::build(&mut mesh, self)?;
            return Ok(mesh);
        }
        if self.kind == PrimitiveKind::Fox {
            crate::fox_primitive::build(&mut mesh, self)?;
            return Ok(mesh);
        }
        let cube = matches!(self.kind, PrimitiveKind::Cube | PrimitiveKind::Sphere);
        let frames = if cube {
            vec![
                (Vec3::X, Vec3::Y, Vec3::Z),
                (-Vec3::X, -Vec3::Y, Vec3::Z),
                (Vec3::Y, Vec3::Z, Vec3::X),
                (-Vec3::Y, -Vec3::Z, Vec3::X),
                (Vec3::Z, Vec3::X, Vec3::Y),
                (-Vec3::Z, -Vec3::X, Vec3::Y),
            ]
        } else if self.kind == PrimitiveKind::Plane {
            vec![(Vec3::Y, Vec3::X, -Vec3::Z)]
        } else {
            vec![(Vec3::Z, Vec3::X, Vec3::Y)]
        };
        for (normal, u, v) in frames {
            let axis = |vector: Vec3| {
                if vector.x.abs() > 0.5 {
                    0
                } else if vector.y.abs() > 0.5 {
                    1
                } else {
                    2
                }
            };
            let (cols, rows) = match self.kind {
                PrimitiveKind::Sphere => (self.segments[0], self.segments[0]),
                PrimitiveKind::Cube => (self.segments[axis(u)], self.segments[axis(v)]),
                _ => (self.segments[0], self.segments[1]),
            };
            let base = mesh.vertices.len() as u32;
            for y in 0..=rows {
                for x in 0..=cols {
                    let uv = [x as f32 / cols as f32, y as f32 / rows as f32];
                    let point = if cube {
                        // Form the signed integer numerator before division.
                        // Opposite face frames then produce identical seam
                        // coordinates, even at high subdivision counts.
                        let coordinate = |i: u32, count: u32| {
                            (2 * i as i32 - count as i32) as f32 / count as f32
                        };
                        normal + u * coordinate(x, cols) + v * coordinate(y, rows)
                    } else {
                        u * ((uv[0] - 0.5) * self.size[0]) + v * ((uv[1] - 0.5) * self.size[1])
                    };
                    let (position, n) = if self.kind == PrimitiveKind::Sphere {
                        let n = point.normalize();
                        (n * self.radius, n)
                    } else if cube {
                        (point * Vec3::from_array(self.size) * 0.5, normal)
                    } else {
                        (point, normal)
                    };
                    mesh.vertices.push(Vertex {
                        position: position.to_array(),
                        normal: n.to_array(),
                        uv,
                        ..Vertex::default()
                    });
                }
            }
            for y in 0..rows {
                for x in 0..cols {
                    let a = base + y * (cols + 1) + x;
                    mesh.topology.as_mut().unwrap().faces.push(vec![
                        a,
                        a + 1,
                        a + cols + 2,
                        a + cols + 1,
                    ]);
                }
            }
        }
        mesh.rebuild_polygon_indices()?;
        if !self.smooth && self.kind == PrimitiveKind::Sphere {
            crate::polygon_edit::flat_normals(&mut mesh)?;
        } else if self.smooth && self.kind == PrimitiveKind::Cube {
            crate::recompute_normals(&mut mesh, true);
        }
        Ok(mesh)
    }
    pub fn max_segments(self) -> u32 {
        if self.kind == PrimitiveKind::Fox {
            4
        } else if self.kind == PrimitiveKind::Sphere {
            48
        } else {
            64
        }
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    #[test]
    fn removed_pyramid_cannot_be_created_from_menu_but_old_settings_still_load() {
        assert!(PrimitiveKind::MESH_MENU.iter().all(|(_, kind)| *kind != PrimitiveKind::Pyramid));
        assert_eq!(PrimitiveKind::from_code(11), None);
        let saved = serde_json::to_string(&PrimitiveSettings::new(PrimitiveKind::Pyramid)).unwrap();
        let settings: PrimitiveSettings = serde_json::from_str(&saved).unwrap();
        let mesh = settings.mesh(1, None).unwrap();
        assert_eq!(settings.kind, PrimitiveKind::Pyramid); assert!(!mesh.indices.is_empty());
    }
}
