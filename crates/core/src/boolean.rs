//! Bounded BSP solid operations. All calculations happen in the target's local space.
use crate::{CoreError, Mat4, Mesh, MeshTopology, Vec3, Vertex};
use glam::DVec3;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanMode {
    Union,
    Difference,
    Intersection,
}
#[derive(Clone)]
struct Corner {
    p: DVec3,
    v: Vertex,
}
impl Corner {
    fn lerp(&self, b: &Self, t: f64) -> Self {
        let mut v = self.v;
        for i in 0..2 {
            v.uv[i] += (b.v.uv[i] - v.uv[i]) * t as f32;
        }
        for i in 0..4 {
            v.color[i] += (b.v.color[i] - v.color[i]) * t as f32;
        }
        v.normal = (Vec3::from_array(v.normal).lerp(Vec3::from_array(b.v.normal), t as f32))
            .normalize_or_zero()
            .to_array();
        Self {
            p: self.p.lerp(b.p, t),
            v,
        }
    }
}
#[derive(Clone)]
struct Polygon {
    corners: Vec<Corner>,
    normal: DVec3,
    w: f64,
    material: Option<u64>,
}
impl Polygon {
    fn new(corners: Vec<Corner>, material: Option<u64>) -> Option<Self> {
        if corners.len() < 3 {
            return None;
        }
        let a = corners[0].p;
        let normal = (1..corners.len() - 1)
            .map(|i| (corners[i].p - a).cross(corners[i + 1].p - a))
            .sum::<DVec3>()
            .try_normalize()?;
        Some(Self {
            corners,
            normal,
            w: normal.dot(a),
            material,
        })
    }
    fn flip(&mut self) {
        self.corners.reverse();
        self.normal = -self.normal;
        self.w = -self.w;
        for c in &mut self.corners {
            for n in &mut c.v.normal {
                *n = -*n;
            }
        }
    }
}
struct Work<'a> {
    eps: f64,
    steps: usize,
    cancel: &'a AtomicBool,
}
impl Work<'_> {
    fn check(&mut self, n: usize) -> Result<(), CoreError> {
        self.steps = self.steps.saturating_add(n);
        if self.cancel.load(Ordering::Relaxed) {
            return Err(error("Булева операция отменена"));
        }
        if self.steps > 8_000_000 {
            return Err(error("Слишком сложное пересечение; упростите объекты"));
        }
        Ok(())
    }
}
fn error(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
#[derive(Default)]
struct Bsp {
    nodes: Vec<BspNode>,
}
#[derive(Default)]
struct BspNode {
    plane: Option<(DVec3, f64)>,
    polygons: Vec<Polygon>,
    front: Option<usize>,
    back: Option<usize>,
}
fn split(
    p: Polygon,
    n: DVec3,
    w: f64,
    eps: f64,
    coplanar_front: &mut Vec<Polygon>,
    coplanar_back: &mut Vec<Polygon>,
    front: &mut Vec<Polygon>,
    back: &mut Vec<Polygon>,
) {
    let types: Vec<_> = p
        .corners
        .iter()
        .map(|c| {
            let d = n.dot(c.p) - w;
            if d < -eps {
                2
            } else if d > eps {
                1
            } else {
                0
            }
        })
        .collect();
    match types.iter().fold(0, |a, b| a | b) {
        0 => {
            if n.dot(p.normal) > 0. {
                coplanar_front.push(p)
            } else {
                coplanar_back.push(p)
            }
        }
        1 => front.push(p),
        2 => back.push(p),
        _ => {
            let mut f = Vec::new();
            let mut b = Vec::new();
            for i in 0..p.corners.len() {
                let j = (i + 1) % p.corners.len();
                let a = &p.corners[i];
                let z = &p.corners[j];
                if types[i] != 2 {
                    f.push(a.clone());
                }
                if types[i] != 1 {
                    b.push(a.clone());
                }
                if types[i] | types[j] == 3 {
                    let t = ((w - n.dot(a.p)) / n.dot(z.p - a.p)).clamp(0., 1.);
                    let v = a.lerp(z, t);
                    f.push(v.clone());
                    b.push(v);
                }
            }
            if let Some(p) = Polygon::new(f, p.material) {
                front.push(p);
            }
            if let Some(p) = Polygon::new(b, p.material) {
                back.push(p);
            }
        }
    }
}
impl Bsp {
    fn build(
        &mut self,
        polygons: Vec<Polygon>,
        work: &mut Work,
        _depth: usize,
    ) -> Result<(), CoreError> {
        if self.nodes.is_empty() {
            self.nodes.push(BspNode::default());
        }
        let mut pending = vec![(0, polygons)];
        while let Some((id, polygons)) = pending.pop() {
            if polygons.is_empty() {
                continue;
            }
            work.check(polygons.len())?;
            let (n, w) = *self.nodes[id]
                .plane
                .get_or_insert((polygons[0].normal, polygons[0].w));
            let (mut front, mut back, mut same, mut opposite) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for p in polygons {
                split(
                    p,
                    n,
                    w,
                    work.eps,
                    &mut same,
                    &mut opposite,
                    &mut front,
                    &mut back,
                );
            }
            self.nodes[id].polygons.extend(same);
            self.nodes[id].polygons.extend(opposite);
            for (is_front, polys) in [(true, front), (false, back)] {
                if polys.is_empty() {
                    continue;
                }
                let child = if is_front {
                    self.nodes[id].front
                } else {
                    self.nodes[id].back
                };
                let index = child.unwrap_or_else(|| {
                    let i = self.nodes.len();
                    self.nodes.push(BspNode::default());
                    if is_front {
                        self.nodes[id].front = Some(i)
                    } else {
                        self.nodes[id].back = Some(i)
                    }
                    i
                });
                pending.push((index, polys));
            }
        }
        Ok(())
    }
    fn invert(&mut self) {
        for node in &mut self.nodes {
            for p in &mut node.polygons {
                p.flip();
            }
            if let Some((n, w)) = &mut node.plane {
                *n = -*n;
                *w = -*w;
            }
            std::mem::swap(&mut node.front, &mut node.back);
        }
    }
    fn clip(&self, polygons: Vec<Polygon>, work: &mut Work) -> Result<Vec<Polygon>, CoreError> {
        if self.nodes.is_empty() {
            return Ok(polygons);
        }
        let mut pending = vec![(0, polygons)];
        let mut result = Vec::new();
        while let Some((id, polygons)) = pending.pop() {
            let node = &self.nodes[id];
            let Some((n, w)) = node.plane else {
                result.extend(polygons);
                continue;
            };
            work.check(polygons.len())?;
            let (mut front, mut back, mut same, mut opposite) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for p in polygons {
                split(
                    p,
                    n,
                    w,
                    work.eps,
                    &mut same,
                    &mut opposite,
                    &mut front,
                    &mut back,
                );
            }
            front.extend(same);
            back.extend(opposite);
            if let Some(i) = node.front {
                if !front.is_empty() {
                    pending.push((i, front));
                }
            } else {
                result.extend(front);
            }
            if let Some(i) = node.back {
                if !back.is_empty() {
                    pending.push((i, back));
                }
            }
        }
        Ok(result)
    }
    fn clip_to(&mut self, other: &Self, work: &mut Work) -> Result<(), CoreError> {
        for node in &mut self.nodes {
            node.polygons = other.clip(std::mem::take(&mut node.polygons), work)?;
        }
        Ok(())
    }
    fn collect(self) -> Vec<Polygon> {
        self.nodes.into_iter().flat_map(|n| n.polygons).collect()
    }
}
fn polygons(mesh: &Mesh, model: Mat4) -> Result<Vec<Polygon>, CoreError> {
    let report = crate::inspect_topology(mesh);
    if report.boundary_edges
        + report.nonmanifold_edges
        + report.winding_edges
        + report.degenerate_faces
        > 0
    {
        return Err(error(
            "Булевы операции требуют замкнутых мешей без проблемных рёбер",
        ));
    }
    if mesh.indices.len() > 60000 {
        return Err(error("Булева операция: до 20000 треугольников на объект"));
    }
    let normal = model.inverse().transpose();
    let reverse = model.determinant() < 0.;
    let mut result = Vec::new();
    for f in 0..mesh.face_count() {
        let indices = crate::triangulate::triangulate(mesh, mesh.face_vertices(f).unwrap())?;
        for t in indices.chunks_exact(3) {
            let mut corners: Vec<_> = t
                .iter()
                .map(|&i| {
                    let mut v = mesh.vertices[i as usize];
                    v.normal = normal
                        .transform_vector3(Vec3::from_array(v.normal))
                        .normalize_or_zero()
                        .to_array();
                    Corner {
                        p: model
                            .transform_point3(Vec3::from_array(v.position))
                            .as_dvec3(),
                        v,
                    }
                })
                .collect();
            if reverse {
                corners.reverse();
            }
            if let Some(p) = Polygon::new(corners, mesh.face_material(f)) {
                result.push(p);
            }
        }
    }
    Ok(result)
}
pub fn boolean_mesh(
    a: &Mesh,
    b: &Mesh,
    b_to_a: Mat4,
    mode: BooleanMode,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    if !b_to_a.is_finite() || b_to_a.determinant().abs() < 1e-10 {
        return Err(error("Восстановите ненулевой масштаб объекта"));
    }
    let pa = polygons(a, Mat4::IDENTITY)?;
    let pb = polygons(b, b_to_a)?;
    let (min, max) = pa
        .iter()
        .chain(&pb)
        .flat_map(|p| p.corners.iter().map(|c| c.p))
        .fold(
            (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(p), hi.max(p)),
        );
    let mut work = Work {
        eps: if min.is_finite() {
            (max - min).length().max(1.) * 1e-7
        } else {
            1e-7
        },
        steps: 0,
        cancel,
    };
    let polygons = if pa.is_empty() || pb.is_empty() {
        match mode {
            BooleanMode::Union => pa.into_iter().chain(pb).collect(),
            BooleanMode::Difference => pa,
            BooleanMode::Intersection => vec![],
        }
    } else {
        let mut left = Bsp::default();
        left.build(pa, &mut work, 0)?;
        let mut right = Bsp::default();
        right.build(pb, &mut work, 0)?;
        match mode {
            BooleanMode::Union => {
                left.clip_to(&right, &mut work)?;
                right.clip_to(&left, &mut work)?;
                right.invert();
                right.clip_to(&left, &mut work)?;
                right.invert();
                left.build(right.collect(), &mut work, 0)?;
            }
            BooleanMode::Difference => {
                left.invert();
                left.clip_to(&right, &mut work)?;
                right.clip_to(&left, &mut work)?;
                right.invert();
                right.clip_to(&left, &mut work)?;
                right.invert();
                left.build(right.collect(), &mut work, 0)?;
                left.invert();
            }
            BooleanMode::Intersection => {
                left.invert();
                right.clip_to(&left, &mut work)?;
                right.invert();
                left.clip_to(&right, &mut work)?;
                right.clip_to(&left, &mut work)?;
                left.build(right.collect(), &mut work, 0)?;
                left.invert();
            }
        }
        left.collect()
    };
    let mut unique = std::collections::BTreeMap::new();
    for p in &polygons {
        for c in &p.corners {
            unique
                .entry(c.p.to_array().map(|x| (x / work.eps).round() as i64))
                .or_insert(c.p);
        }
    }
    let points: Vec<_> = unique.into_values().collect();
    // Restrict T-junction searches to the narrowest axis range of each edge.
    let sorted: [Vec<(f64, usize)>; 3] = std::array::from_fn(|axis| {
        let mut v: Vec<_> = points
            .iter()
            .enumerate()
            .map(|(i, p)| (p[axis], i))
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v
    });
    let mut mesh = a.clone();
    mesh.vertices = Default::default();
    mesh.indices = Default::default();
    mesh.morph_targets.clear();
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    for p in polygons {
        let mut ring = Vec::new();
        for i in 0..p.corners.len() {
            let a = &p.corners[i];
            let b = &p.corners[(i + 1) % p.corners.len()];
            let d = b.p - a.p;
            let len = d.length_squared();
            if len <= work.eps.powi(2) {
                continue;
            }
            let mut cuts = vec![(0., a.clone())];
            let (axis, low, high) = (0..3)
                .map(|axis| {
                    let lo = a.p[axis].min(b.p[axis]) - work.eps * 2.;
                    let hi = a.p[axis].max(b.p[axis]) + work.eps * 2.;
                    (
                        axis,
                        sorted[axis].partition_point(|p| p.0 < lo),
                        sorted[axis].partition_point(|p| p.0 <= hi),
                    )
                })
                .min_by_key(|(_, lo, hi)| hi - lo)
                .unwrap();
            work.check(high - low)?;
            for &(_, index) in &sorted[axis][low..high] {
                let q = points[index];
                let t = (q - a.p).dot(d) / len;
                if t > 1e-7
                    && t < 1. - 1e-7
                    && (a.p + d * t).distance_squared(q) < work.eps.powi(2) * 4.
                {
                    let mut c = a.lerp(b, t);
                    c.p = q;
                    cuts.push((t, c));
                }
            }
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
            cuts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-7);
            for (_, c) in cuts {
                let mut v = c.v;
                v.position = c.p.as_vec3().to_array();
                ring.push(mesh.vertices.len() as u32);
                mesh.vertices.push(v);
            }
        }
        if ring.len() < 3 {
            continue;
        }
        if ring.len() > crate::MAX_FACE_CORNERS {
            return Err(error("Слишком много пересечений на одной грани"));
        }
        if ring.len() == 4 {
            for _ in 0..4 {
                let q = |i: usize| Vec3::from_array(mesh.vertices[ring[i] as usize].position);
                if (q(1) - q(0)).cross(q(2) - q(0)).length_squared() > 1e-16
                    && (q(2) - q(0)).cross(q(3) - q(0)).length_squared() > 1e-16
                {
                    break;
                }
                ring.rotate_left(1);
            }
        }
        faces.push(ring);
        materials.push(p.material);
        if mesh.vertices.len() > crate::MAX_EDIT_VERTICES {
            return Err(error("Результат превышает предел сетки"));
        }
    }
    mesh.topology = Some(MeshTopology {
        faces: faces.into(),
        face_materials: materials.into(),
        loose_edges: vec![],
        uv_seams: vec![],
        primitive: None,
        smooth_normals: false,
    });
    mesh.rebuild_polygon_indices()?;
    crate::recompute_normals(&mut mesh, false);
    mesh.validate_topology()?;
    let report = crate::inspect_topology(&mesh);
    if report.nonmanifold_edges
        + report.boundary_edges
        + report.winding_edges
        + report.degenerate_faces
        > 0
    {
        return Err(error(
            "Не удалось построить замкнутое пересечение; проверьте совпадающие поверхности",
        ));
    }
    Ok(mesh)
}
