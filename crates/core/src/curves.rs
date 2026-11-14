//! Curve control points are the source; optional surface geometry is a derived mesh.
use crate::{CoreError, Editor, Material, Mesh, MeshTopology, Node, PrimitiveKind, Project, Vec3, Vertex};
use serde::{Deserialize, Serialize};

pub const MAX_CURVE_POINTS: usize = 256;
pub const MAX_CURVE_SAMPLES: usize = 2048;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum CurveInterpolation { Polyline, #[default] Smooth, Bezier }
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum CurveProfile { #[default] Tube, Ribbon }
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CurveGeometry {
    pub profile: CurveProfile,
    pub radius: f32,
    /// Cross-section rotation in degrees, transported along the entire curve.
    pub rotation: f32,
    pub sides: u32,
    pub caps: bool,
}
impl Default for CurveGeometry {
    fn default() -> Self { Self { profile: CurveProfile::Tube, radius: 0.05, rotation: 0., sides: 8, caps: true } }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CurveData {
    pub points: Vec<[f32; 3]>,
    pub interpolation: CurveInterpolation,
    pub closed: bool,
    /// Samples per spline segment; bounded again by the total sample budget.
    pub resolution: u32,
    pub geometry: Option<CurveGeometry>,
}
impl Default for CurveData {
    fn default() -> Self {
        Self { points: vec![[-1., 0., 0.], [1., 0., 0.]], interpolation: CurveInterpolation::Smooth,
            closed: false, resolution: 12, geometry: None }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurvePreset { Bezier, Circle, Line, Arc, Ellipse, Rectangle, Spiral, Helix }
impl CurvePreset {
    /// Compact creation menus; ALL remains available for old documents and tests.
    pub const BASIC: [Self; 3] = [Self::Bezier, Self::Circle, Self::Line];
    pub const ALL: [Self; 8] = [Self::Bezier, Self::Circle, Self::Line, Self::Arc, Self::Ellipse, Self::Rectangle, Self::Spiral, Self::Helix];
    pub fn label(self) -> &'static str {
        match self { Self::Bezier => "Безье", Self::Circle => "Окружность", Self::Line => "Линия",
            Self::Arc => "Дуга", Self::Ellipse => "Эллипс", Self::Rectangle => "Прямоугольник",
            Self::Spiral => "Спираль", Self::Helix => "Винтовая линия" }
    }
    pub fn curve(self) -> CurveData {
        let mut curve = CurveData::default();
        match self {
            Self::Bezier => { curve.interpolation = CurveInterpolation::Bezier;
                curve.points = vec![[-1., 0., 0.], [-0.5, 1., 0.], [0.5, -1., 0.], [1., 0., 0.]];
                curve.resolution = 32; }
            Self::Line => curve.interpolation = CurveInterpolation::Polyline,
            Self::Rectangle => { curve.interpolation = CurveInterpolation::Polyline; curve.closed = true;
                curve.points = vec![[-1., -0.6, 0.], [1., -0.6, 0.], [1., 0.6, 0.], [-1., 0.6, 0.]]; }
            Self::Circle | Self::Ellipse => {
                // Four cubic arcs using the standard tangent approximation.
                let k = 0.552_284_8;
                curve.points = vec![[1.,0.,0.],[1.,k,0.],[k,1.,0.],[0.,1.,0.],[-k,1.,0.],[-1.,k,0.],[-1.,0.,0.],
                    [-1.,-k,0.],[-k,-1.,0.],[0.,-1.,0.],[k,-1.,0.],[1.,-k,0.],[1.,0.,0.]];
                if self == Self::Ellipse { for p in &mut curve.points { p[0] *= 1.5; p[1] *= 0.75; } }
                curve.closed = true; curve.interpolation = CurveInterpolation::Bezier;
            }
            Self::Arc => {
                curve.points = (0..=8).map(|i| { let a = i as f32 * std::f32::consts::PI / 8.; [a.cos(), a.sin(), 0.] }).collect();
            }
            Self::Spiral | Self::Helix => {
                curve.points = (0..=32).map(|i| { let t = i as f32 / 32.; let a = t * std::f32::consts::TAU * 2.;
                    let r = if self == Self::Spiral { 0.1 + t * 1.4 } else { 0.8 };
                    [a.cos() * r, if self == Self::Helix { t * 2. - 1. } else { a.sin() * r },
                        if self == Self::Helix { a.sin() * r } else { 0. }] }).collect();
            }
        }
        curve
    }
}
fn invalid(message: &str) -> CoreError { CoreError::Invalid(message.into()) }
impl CurveData {
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(2..=MAX_CURVE_POINTS).contains(&self.points.len()) || !(1..=64).contains(&self.resolution)
            || self.points.iter().flatten().any(|v| !v.is_finite() || v.abs() > 10000.) {
            return Err(invalid("Кривая: 2–256 точек, детализация 1–64, координаты от −10000 до 10000"));
        }
        if self.interpolation == CurveInterpolation::Bezier
            && (self.points.len() < 4 || !(self.points.len() - 1).is_multiple_of(3)
                || (self.closed && self.points.first() != self.points.last())) {
            return Err(invalid("Безье: цепочка из 3n + 1 точек; концы замкнутой кривой должны совпадать"));
        }
        if self.closed && self.points.len() < 3 { return Err(invalid("Замкнутой кривой нужны три точки")); }
        if let Some(g) = self.geometry {
            if !g.rotation.is_finite() || g.rotation.abs() > 3600. || !g.radius.is_finite() || !(0.001..=1000.).contains(&g.radius) || !(3..=32).contains(&g.sides) {
                return Err(invalid("Геометрия кривой: радиус 0,001–1000 м; 3–32 стороны"));
            }
        }
        Ok(())
    }
    pub fn samples(&self) -> Result<Vec<Vec3>, CoreError> {
        self.validate()?;
        let bezier = self.interpolation == CurveInterpolation::Bezier;
        let count = self.points.len();
        let segments = if bezier { (count - 1) / 3 } else if self.closed { count } else { count - 1 };
        let steps = if self.interpolation == CurveInterpolation::Polyline { 1 }
            else { (self.resolution as usize).min((MAX_CURVE_SAMPLES - 1) / segments).max(1) };
        let point = |index: isize| Vec3::from_array(self.points[if self.closed && !bezier {
            index.rem_euclid(count as isize) as usize
        } else { index.clamp(0, count as isize - 1) as usize }]);
        let mut samples = Vec::with_capacity(segments * steps + 1);
        for i in 0..segments {
            for j in 0..steps {
                let t = j as f32 / steps as f32;
                let p = if bezier {
                    let a = (i * 3) as isize; let u = 1. - t;
                    point(a) * u.powi(3) + point(a + 1) * (3. * u * u * t)
                        + point(a + 2) * (3. * u * t * t) + point(a + 3) * t.powi(3)
                } else if self.interpolation == CurveInterpolation::Polyline { point(i as isize) }
                else {
                    let a = i as isize;
                    let (p0, p1, p2, p3) = (point(a - 1), point(a), point(a + 1), point(a + 2));
                    (p1 * 2. + (-p0 + p2) * t + (p0 * 2. - p1 * 5. + p2 * 4. - p3) * t * t
                        + (-p0 + p1 * 3. - p2 * 3. + p3) * t * t * t) * 0.5
                };
                if samples.last().is_none_or(|last: &Vec3| last.distance_squared(p) > 1e-12) { samples.push(p); }
            }
        }
        let last = if self.closed { samples.first().copied().unwrap_or(Vec3::ZERO) } else { point(count as isize - 1) };
        if samples.last().is_none_or(|p| p.distance_squared(last) > 1e-12) { samples.push(last); }
        if samples.len() < (if self.closed { 4 } else { 2 }) { return Err(invalid("Кривая имеет нулевую длину")); }
        Ok(samples)
    }
    pub fn mesh(&self, id: u64, material: Option<u64>) -> Result<Mesh, CoreError> {
        let g = self.geometry.ok_or_else(|| invalid("Сначала добавьте геометрию кривой"))?;
        let centers = self.samples()?;
        let steps = centers.len() - 1;
        let sides = if g.profile == CurveProfile::Tube { g.sides as usize } else { 1 };
        if centers.len() * (sides + 1) + sides * 2 > crate::MAX_EDIT_VERTICES {
            return Err(invalid("Геометрия кривой превышает предел вершин; уменьшите детализацию"));
        }
        let mut mesh = Mesh { id, name: "Геометрия кривой".into(), material, vertices: Default::default(),
            indices: Default::default(), morph_targets: Vec::new(), topology: Some(MeshTopology { loose_edges: Vec::new(),
                faces: Default::default(), face_materials: Default::default(), uv_seams: Vec::new(), primitive: None, smooth_normals: true }) };
        let mut tangents = Vec::with_capacity(centers.len());
        for i in 0..=steps {
            let before = if self.closed && (i == 0 || i == steps) { centers[steps - 1] } else { centers[i.saturating_sub(1)] };
            let after = if self.closed && (i == 0 || i == steps) { centers[1] } else { centers[(i + 1).min(steps)] };
            tangents.push((after - before).try_normalize().or_else(|| (after - centers[i]).try_normalize())
                .ok_or_else(|| invalid("Резкий разворот кривой: раздвиньте соседние точки"))?);
        }
        let mut frames = Vec::with_capacity(centers.len());
        let mut normal = tangents[0].any_orthonormal_vector();
        for t in &tangents {
            normal = (normal - *t * normal.dot(*t)).try_normalize().unwrap_or_else(|| t.any_orthonormal_vector());
            frames.push(normal);
        }
        if self.closed {
            let last = frames[steps]; let first = frames[0]; let axis = tangents[0];
            let correction = axis.dot(last.cross(first)).atan2(last.dot(first));
            for i in 0..=steps { frames[i] = crate::Quat::from_axis_angle(tangents[i], correction * i as f32 / steps as f32) * frames[i]; }
            frames[steps] = frames[0]; tangents[steps] = tangents[0];
        }
        let mut length = vec![0.; centers.len()];
        for i in 1..centers.len() { length[i] = length[i - 1] + centers[i].distance(centers[i - 1]); }
        for i in 0..=steps {
            let normal = crate::Quat::from_axis_angle(tangents[i], g.rotation.to_radians()) * frames[i]; let binormal = tangents[i].cross(normal);
            for j in 0..=sides {
                let radial = if g.profile == CurveProfile::Ribbon { normal * if j == 0 { -1. } else { 1. } }
                    else { let a = if j == sides { 0. } else { j as f32 / sides as f32 * std::f32::consts::TAU };
                        normal * a.cos() + binormal * a.sin() };
                mesh.vertices.push(Vertex { position: (centers[i] + radial * g.radius).to_array(),
                    normal: if g.profile == CurveProfile::Ribbon { binormal.to_array() } else { radial.to_array() },
                    uv: [length[i] / length[steps].max(1e-8), j as f32 / sides as f32], ..Default::default() });
            }
        }
        for i in 0..steps { for j in 0..sides {
            let a = (i * (sides + 1) + j) as u32; let b = a + (sides + 1) as u32;
            mesh.topology.as_mut().unwrap().faces.push(vec![a, a + 1, b + 1, b]);
        } }
        if g.caps && !self.closed && g.profile == CurveProfile::Tube {
            for end in [0, steps] {
                let base = mesh.vertices.len() as u32;
                for j in 0..sides {
                    let mut v = mesh.vertices[end * (sides + 1) + j];
                    v.normal = (tangents[end] * if end == 0 { -1. } else { 1. }).to_array();
                    let angle = j as f32 / sides as f32 * std::f32::consts::TAU;
                    v.uv = [0.5 + angle.cos() * 0.5, 0.5 + angle.sin() * 0.5]; mesh.vertices.push(v);
                }
                let mut face: Vec<_> = (base..base + sides as u32).collect(); if end == 0 { face.reverse(); }
                mesh.topology.as_mut().unwrap().faces.push(face);
            }
        }
        mesh.rebuild_polygon_indices()?; mesh.validate_topology()?; Ok(mesh)
    }
}
impl Editor {
    pub fn create_curve(&mut self, name: &str, curve: CurveData) -> Result<u64, CoreError> {
        curve.samples()?;
        if curve.geometry.is_some() { return Err(invalid("Создайте кривую, затем добавьте геометрию")); }
        let id = self.project.next_id(); let mut node = Node::new(id, name); node.curve = Some(curve);
        self.transact("Создать кривую", move |p| p.nodes.push(node))?; Ok(id)
    }
    pub fn set_curve(&mut self, id: u64, curve: CurveData) -> Result<(), CoreError> {
        self.authoring_unlocked(id)?; curve.samples()?;
        let node = self.project.node(id).ok_or(CoreError::MissingNode(id))?;
        if node.curve.is_none() || node.skin.is_some() { return Err(invalid("Выберите кривую без скелетной привязки")); }
        if node.curve.as_ref() == Some(&curve) { return Ok(()); }
        let old_mesh = node.mesh;
        let source = node.mesh.and_then(|m| self.project.mesh(m));
        let mesh_id = source.filter(|m| !self.project.nodes.iter().any(|n| n.id != id && n.mesh == Some(m.id)))
            .map_or_else(|| self.project.next_id(), |m| m.id);
        let material_id = source.map(|m| m.material).unwrap_or(Some(self.project.next_id().max(mesh_id + 1)));
        let mesh = if curve.geometry.is_some() { Some(curve.mesh(mesh_id, material_id)?) } else { None };
        self.transact("Изменить кривую", move |p| {
            if let Some(mesh) = mesh {
                if let Some(id) = material_id.filter(|id| p.material(*id).is_none()) {
                    p.materials.push(Material { id, name: "Материал кривой".into(), double_sided: true, ..Default::default() });
                }
                if let Some(slot) = p.meshes.iter_mut().find(|m| m.id == mesh.id) { *slot = mesh; } else { p.meshes.push(mesh); }
                p.node_mut(id).unwrap().mesh = Some(mesh_id);
            } else {
                let node = p.node_mut(id).unwrap(); node.mesh = None; node.material_layers.clear();
                node.fluid = None; node.physics = None; node.destruction = None;
            }
            p.node_mut(id).unwrap().curve = Some(curve);
            if let Some(old) = old_mesh.filter(|old| !p.nodes.iter().any(|n| n.mesh == Some(*old))) {
                p.meshes.retain(|m| m.id != old);
            }
        })
    }
    pub fn convert_curve_to_mesh(&mut self, id: u64) -> Result<(), CoreError> {
        self.authoring_unlocked(id)?;
        let node = self.project.node(id).ok_or(CoreError::MissingNode(id))?;
        if node.curve.is_none() || node.mesh.is_none() { return Err(invalid("Добавьте геометрию перед преобразованием")); }
        self.transact("Преобразовать кривую в меш", |p| p.node_mut(id).unwrap().curve = None)
    }
}
impl Project {
    /// Legacy tubes retain their geometry and material while gaining curve controls.
    pub fn migrate_legacy_curves(&mut self) {
        let curves: std::collections::BTreeMap<_, _> = self.meshes.iter().filter_map(|mesh| {
            let s = mesh.topology.as_ref()?.primitive?; if !s.is_curve() || !mesh.morph_targets.is_empty() { return None; }
            let mut curve = if s.kind == PrimitiveKind::Bezier { CurvePreset::Bezier.curve() } else { CurvePreset::Circle.curve() };
            if s.kind == PrimitiveKind::Bezier { curve.points = s.curve_points.to_vec(); }
            else { for p in &mut curve.points { for v in p { *v *= s.size[0]; } } }
            curve.resolution = (s.segments[0] / if curve.closed { 4 } else { 1 }).clamp(1, 64);
            curve.geometry = Some(CurveGeometry { radius: s.radius, sides: s.segments[1].clamp(3, 32), ..Default::default() });
            Some((mesh.id, curve))
        }).collect();
        for node in &mut self.nodes { if node.curve.is_none() && node.skin.is_none() && node.camera.is_none() && !node.is_joint {
            node.curve = node.mesh.and_then(|id| curves.get(&id).cloned());
        } }
    }
}
