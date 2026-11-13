//! Arc-length curve constraints, independent of the scene parenting hierarchy.
use crate::{CoreError, CurveData, Light, Mat4, NodeId, Project, Quat, Transform, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CurveBinding {
    pub curve: NodeId,
    /// Fraction of the curve's world-space length at timeline time zero.
    pub position: f32,
    pub follow: bool,
    /// Curve lengths per second. Zero leaves the object at the chosen position.
    pub speed: f32,
    pub repeat: bool,
}
impl Default for CurveBinding {
    fn default() -> Self { Self { curve: 0, position: 0., follow: true, speed: 0., repeat: false } }
}
impl CurveBinding {
    pub fn validate(self) -> Result<(), CoreError> {
        if !self.position.is_finite() || !(0. ..=1.).contains(&self.position)
            || !self.speed.is_finite() || self.speed.abs() > 100.
        { return Err(CoreError::Invalid("Привязка к кривой: положение 0–100 %, конечная скорость".into())); }
        Ok(())
    }
    pub(crate) fn phase(self, time: f64) -> Result<f32, CoreError> {
        self.validate()?;
        let value = self.position as f64 + self.speed as f64 * time;
        if !value.is_finite() { return Err(CoreError::Invalid("Время движения по кривой".into())); }
        Ok((if self.repeat { value.rem_euclid(1.) } else { value.clamp(0., 1.) }) as f32)
    }
}

pub(crate) struct Path {
    points: Vec<Vec3>, distances: Vec<f32>, tangents: Vec<Vec3>, ups: Vec<Vec3>,
    closed: bool,
}
fn perpendicular(tangent: Vec3, preferred: Vec3) -> Vec3 {
    let projected = preferred - tangent * tangent.dot(preferred);
    projected.try_normalize().unwrap_or_else(|| {
        let axis = if tangent.y.abs() < 0.9 { Vec3::Y } else { Vec3::Z };
        (axis - tangent * tangent.dot(axis)).normalize()
    })
}
impl Path {
    pub(crate) fn new(curve: &CurveData, model: Mat4) -> Result<Self, CoreError> {
        let mut points: Vec<Vec3> = Vec::new();
        for point in curve.samples()?.into_iter().map(|p| model.transform_point3(p)) {
            if !point.is_finite() { return Err(CoreError::Invalid("Кривая выходит за пределы координат".into())); }
            if points.last().is_none_or(|last| last.distance_squared(point) > 1e-12) { points.push(point); }
        }
        if points.len() < 2 { return Err(CoreError::Invalid("Для привязки нужна кривая ненулевой длины".into())); }
        let mut distances = vec![0.];
        for p in points.windows(2) { distances.push(distances.last().unwrap() + p[0].distance(p[1])); }
        if !distances.last().unwrap().is_finite() { return Err(CoreError::Invalid("Переполнение длины кривой".into())); }
        let segments: Vec<_> = points.windows(2).map(|p| (p[1] - p[0]).normalize()).collect();
        let last = points.len() - 1;
        let mut tangents = Vec::with_capacity(points.len());
        for i in 0..=last {
            let before = if i == 0 { if curve.closed { segments[last - 1] } else { segments[0] } } else { segments[i - 1] };
            let after = if i == last { if curve.closed { segments[0] } else { segments[last - 1] } } else { segments[i] };
            tangents.push((before + after).try_normalize().unwrap_or(after));
        }
        let mut ups = vec![perpendicular(tangents[0], model.transform_vector3(Vec3::Y))];
        for i in 1..=last {
            let rotated = Quat::from_rotation_arc(tangents[i - 1], tangents[i]) * ups[i - 1];
            ups.push(perpendicular(tangents[i], rotated));
        }
        if curve.closed {
            let twist = tangents[0].dot(ups[last].cross(ups[0])).atan2(ups[last].dot(ups[0]));
            for i in 1..=last { ups[i] = Quat::from_axis_angle(tangents[i], twist * distances[i] / distances[last]) * ups[i]; }
        }
        Ok(Self { points, distances, tangents, ups, closed: curve.closed })
    }
    pub(crate) fn for_text(curve:&CurveData,model:Mat4)->Result<Self,CoreError> {
        let mut path=Self::new(curve,model)?;let last=path.tangents.len()-1;
        let normal=model.transform_vector3(Vec3::Z).normalize_or_zero();
        path.ups[0]=perpendicular(path.tangents[0],normal.cross(path.tangents[0]));
        for i in 1..=last {
            path.ups[i]=perpendicular(path.tangents[i],Quat::from_rotation_arc(path.tangents[i-1],path.tangents[i])*path.ups[i-1]);
        }
        if path.closed {
            let twist=path.tangents[0].dot(path.ups[last].cross(path.ups[0])).atan2(path.ups[last].dot(path.ups[0]));
            for i in 1..=last {path.ups[i]=Quat::from_axis_angle(path.tangents[i],twist*path.distances[i]/path.distances[last])*path.ups[i];}
        }
        Ok(path)
    }
    pub(crate) fn length(&self)->f32 {*self.distances.last().unwrap()}
    /// Text uses X as its baseline. Open paths extend at the ends instead of
    /// collapsing long labels; closed paths wrap at the seam.
    pub(crate) fn text_frame(&self,distance:f32)->Mat4 {
        let d=if self.closed {distance.rem_euclid(self.length())}else{distance.clamp(0.,self.length())};
        let end=self.distances.partition_point(|&v|v<d).clamp(1,self.points.len()-1);let start=end-1;
        let t=(d-self.distances[start])/(self.distances[end]-self.distances[start]);
        let x=self.tangents[start].lerp(self.tangents[end],t).try_normalize().unwrap_or(self.tangents[end]);
        let y=perpendicular(x,self.ups[start].lerp(self.ups[end],t));let z=x.cross(y).normalize();
        let p=self.points[start].lerp(self.points[end],t)+if self.closed {Vec3::ZERO}else{x*(distance-d)};
        Mat4::from_cols(x.extend(0.),y.extend(0.),z.extend(0.),p.extend(1.))
    }
    fn frame(&self, binding: CurveBinding, time: f64, negative_z: bool) -> Result<Mat4, CoreError> {
        let distance = binding.phase(time)? * self.distances.last().unwrap();
        let end = self.distances.partition_point(|&d| d < distance).clamp(1, self.points.len() - 1);
        let start = end - 1;
        let t = (distance - self.distances[start]) / (self.distances[end] - self.distances[start]);
        let position = self.points[start].lerp(self.points[end], t);
        if !binding.follow { return Ok(Mat4::from_translation(position)); }
        let forward = self.tangents[start].lerp(self.tangents[end], t).try_normalize().unwrap_or(self.tangents[end]);
        let up = perpendicular(forward, self.ups[start].lerp(self.ups[end], t));
        let z = if negative_z { -forward } else { forward };
        let right = up.cross(z).normalize();
        Ok(Mat4::from_cols(right.extend(0.), z.cross(right).extend(0.), z.extend(0.), position.extend(1.)))
    }
}

/// Immutable path prepared once for a pointer gesture. Local offsets are kept;
/// only the binding position changes, including at nonzero timeline times.
pub struct CurveTranslation {
    path: Path,
    binding: CurveBinding,
    time: f64,
    local_position: Vec3,
    negative_z: bool,
    samples: Vec<(f32, Vec3)>,
    limits: [f32; 2],
}
impl CurveTranslation {
    pub fn binding(&self) -> CurveBinding { self.binding }
    fn point(&self, phase: f32) -> Result<Vec3, CoreError> {
        let binding = CurveBinding { position: phase, speed: 0., repeat: false, ..self.binding };
        let point = self.path.frame(binding, 0., self.negative_z)?.transform_point3(self.local_position);
        if !point.is_finite() { return Err(CoreError::Invalid("Переполнение смещения от кривой".into())); }
        Ok(point)
    }
    fn from_phase(&self, phase: f32) -> f32 {
        let clock = self.binding.speed as f64 * self.time;
        let value = phase as f64 - if self.binding.repeat { clock.rem_euclid(1.) } else { clock };
        if !self.binding.repeat { return value.clamp(0., 1.) as f32; }
        let initial = value.rem_euclid(1.) as f32;
        let mut best = initial; let mut error = f32::INFINITY;
        // Rounding a base offset near a loop seam must not put an open path's
        // first endpoint at its last endpoint. Check adjacent representable values.
        for position in [initial, f32::from_bits(initial.to_bits().saturating_sub(1)),
            f32::from_bits(initial.to_bits() + 1).min(1.)]
        {
            if let Ok(sample) = (CurveBinding { position, ..self.binding }).phase(self.time) {
                let distance = (sample - phase).abs();
                if distance < error { best = position; error = distance; }
            }
        }
        best
    }
    pub fn advance(&self, distance: f32) -> Result<f32, CoreError> {
        if !distance.is_finite() { return Err(CoreError::Invalid("Расстояние вдоль кривой".into())); }
        if distance == 0. || self.limits[1] - self.limits[0] < 1e-7 { return Ok(self.binding.position); }
        let phase = self.binding.phase(self.time)? as f64 + distance as f64 / *self.path.distances.last().unwrap() as f64;
        let phase = if self.binding.repeat { phase.rem_euclid(1.) }
            else { phase.clamp(self.limits[0] as f64, self.limits[1] as f64) };
        Ok(self.from_phase(phase as f32))
    }
    /// Project the desired world position onto the follower's trajectory, not
    /// onto the unoffset curve. Nearby equal-distance branches prefer the start.
    pub fn position(&self, target: Vec3) -> Result<f32, CoreError> {
        if !target.is_finite() { return Err(CoreError::Invalid("Положение на кривой".into())); }
        let current = self.binding.phase(self.time)?;
        let mut best_phase = current;
        let mut best_distance = self.point(current)?.distance_squared(target);
        if best_distance <= 1e-12 || self.limits[1] - self.limits[0] < 1e-7 { return Ok(self.binding.position); }
        let arc = |phase: f32| {
            let d = (phase - current).abs();
            if self.binding.repeat { d.min(1. - d) } else { d }
        };
        let mut interval = None;
        for pair in self.samples.windows(2) {
            let ((a, p), (b, q)) = (pair[0], pair[1]);
            let delta = q - p;
            let t = if delta.length_squared() > 1e-12 { ((target - p).dot(delta) / delta.length_squared()).clamp(0., 1.) } else { 0. };
            let phase = a + (b - a) * t;
            let distance = self.point(phase)?.distance_squared(target);
            let epsilon = 1e-8 * best_distance.max(1.);
            if distance < best_distance - epsilon
                || ((distance - best_distance).abs() <= epsilon && arc(phase) < arc(best_phase))
            {
                best_phase = phase; best_distance = distance; interval = Some((a, b));
            }
        }
        // Refine only the winning segment; frame interpolation can curve the
        // offset trajectory between samples. Preserve exact endpoints/starts.
        if let Some((mut a, mut b)) = interval {
            for _ in 0..24 {
                let left = a + (b - a) / 3.; let right = b - (b - a) / 3.;
                if self.point(left)?.distance_squared(target) < self.point(right)?.distance_squared(target) { b = right; }
                else { a = left; }
            }
            let phase = (a + b) * 0.5; let distance = self.point(phase)?.distance_squared(target);
            if distance < best_distance { best_phase = phase; }
        }
        // A repeating open path teleports at phase 1. A pointer at its last
        // endpoint should approach that endpoint instead of jumping to phase 0.
        if self.binding.repeat && !self.path.closed { best_phase = best_phase.min(1. - 1e-6); }
        if (best_phase - current).abs() < 1e-7 { Ok(self.binding.position) }
        else { Ok(self.from_phase(best_phase)) }
    }
}

impl Project {
    pub fn curve_translation(&self, binding: CurveBinding, globals: &BTreeMap<NodeId, Mat4>,
        time: f64, local_position: Vec3, negative_z: bool) -> Result<CurveTranslation, CoreError>
    {
        binding.phase(time)?;
        if !local_position.is_finite() { return Err(CoreError::Invalid("Смещение от кривой".into())); }
        let path = self.binding_path(binding, globals)?;
        let clock = binding.speed as f64 * time;
        let limits = if binding.repeat { [0., 1.] } else { [clock.clamp(0., 1.) as f32, (clock + 1.).clamp(0., 1.) as f32] };
        let mut phases = vec![limits[0], limits[1]];
        for d in path.distances.windows(2) {
            for distance in [d[0], (d[0] + d[1]) * 0.5, d[1]] {
                let phase = distance / path.distances.last().unwrap();
                if phase > limits[0] && phase < limits[1] { phases.push(phase); }
            }
        }
        phases.sort_by(f32::total_cmp); phases.dedup();
        let mut drag = CurveTranslation { path, binding, time, local_position, negative_z, samples: Vec::new(), limits };
        drag.samples = phases.into_iter().map(|p| Ok((p, drag.point(p)?))).collect::<Result<_, CoreError>>()?;
        Ok(drag)
    }
    fn binding_path(&self, binding: CurveBinding, globals: &BTreeMap<NodeId, Mat4>) -> Result<Path, CoreError> {
        let curve = self.node(binding.curve).and_then(|n| n.curve.as_ref())
            .ok_or_else(|| CoreError::Invalid("Кривая привязки отсутствует".into()))?;
        let model = globals.get(&binding.curve).ok_or(CoreError::MissingNode(binding.curve))?;
        Path::new(curve, *model)
    }
    pub fn curve_binding_frame(&self, binding: CurveBinding, globals: &BTreeMap<NodeId, Mat4>, time: f64, negative_z: bool) -> Result<Mat4, CoreError> {
        self.binding_path(binding, globals)?.frame(binding, time, negative_z)
    }
    /// Parent frame for local transform tools, including a curve constraint.
    pub fn transform_frame(&self, id: NodeId, globals: &BTreeMap<NodeId, Mat4>, time: f64) -> Result<Mat4, CoreError> {
        let node = self.node(id).ok_or(CoreError::MissingNode(id))?;
        if let Some(binding) = node.curve_binding {
            self.curve_binding_frame(binding, globals, time, node.camera.is_some())
        } else if let Some(source)=node.fx.and_then(|f|f.source) {
            globals.get(&source).copied().ok_or(CoreError::MissingNode(source))
        } else { Ok(node.parent.and_then(|p| globals.get(&p)).copied().unwrap_or(Mat4::IDENTITY)) }
    }
    pub(crate) fn curve_globals_for(&self, overrides: &BTreeMap<NodeId, Transform>, time: f64) -> Result<BTreeMap<NodeId, Mat4>, CoreError> {
        let lookup: BTreeMap<_, _> = self.nodes.iter().map(|n| (n.id, n)).collect();
        let mut out = BTreeMap::new(); let mut state = BTreeMap::new(); let mut paths = BTreeMap::new();
        for node in &self.nodes {
            if out.contains_key(&node.id) { continue; }
            let mut stack = vec![(node.id, false)];
            while let Some((id, finish)) = stack.pop() {
                if out.contains_key(&id) { continue; }
                let n = lookup.get(&id).ok_or(CoreError::MissingNode(id))?;
                if !finish {
                    if state.insert(id, true).is_some() { return Err(CoreError::HierarchyCycle(id)); }
                    stack.push((id, true));
                    // Both dependency types are checked: constraints cannot hide a parenting cycle.
                    for dependency in [n.parent, n.curve_binding.map(|b| b.curve),n.fx.and_then(|f|f.source)].into_iter().flatten() {
                        if !out.contains_key(&dependency) { stack.push((dependency, false)); }
                    }
                    continue;
                }
                let frame = if let Some(binding) = n.curve_binding {
                    if !paths.contains_key(&binding.curve) { paths.insert(binding.curve, self.binding_path(binding, &out)?); }
                    paths[&binding.curve].frame(binding, time, n.camera.is_some())?
                } else if let Some(source)=n.fx.and_then(|f|f.source) {out[&source]}
                else { n.parent.map_or(Mat4::IDENTITY, |p| out[&p]) };
                let global = frame * overrides.get(&id).unwrap_or(&n.transform).matrix();
                if !global.is_finite() { return Err(CoreError::Invalid("Переполнение трансформации привязки".into())); }
                out.insert(id, global);
            }
        }
        Ok(out)
    }
    pub fn evaluate_light(&self, light: &Light, globals: &BTreeMap<NodeId, Mat4>, time: f64) -> Result<Light, CoreError> {
        let mut result = light.clone();
        if let Some(binding) = light.curve_binding {
            let frame = self.curve_binding_frame(binding, globals, time, true)?;
            result.position = frame.transform_point3(light.position);
            result.direction = frame.transform_vector3(light.direction).normalize_or_zero();
            result.curve_binding = None;
        }
        result.direction = result.evaluated_direction(globals);
        Ok(result)
    }
    pub fn evaluated_lights(&self, globals: &BTreeMap<NodeId, Mat4>, time: f64) -> Result<Vec<Light>, CoreError> {
        let mut paths = BTreeMap::new();
        self.lights.iter().map(|light| {
            let mut resolved = light.clone();
            if let Some(binding) = light.curve_binding {
                if !paths.contains_key(&binding.curve) { paths.insert(binding.curve, self.binding_path(binding, globals)?); }
                let frame = paths[&binding.curve].frame(binding, time, true)?;
                resolved.position = frame.transform_point3(light.position);
                resolved.direction = frame.transform_vector3(light.direction).normalize_or_zero();
                resolved.curve_binding = None;
            }
            resolved.direction = resolved.evaluated_direction(globals);
            Ok(resolved)
        }).collect()
    }
    /// Freeze the path frame at this sample. Rebase authored keys as well as
    /// the rest transform, so an animated offset does not jump on detachment.
    pub fn detach_curve_node(&mut self, id: NodeId, globals: &BTreeMap<NodeId, Mat4>, sampled_local: Transform) -> Result<(), CoreError> {
        if self.node(id).ok_or(CoreError::MissingNode(id))?.curve_binding.is_none() {return Ok(());}
        self.detach_transform_constraint(id,globals,sampled_local,false)
    }
    pub fn detach_fx_source(&mut self, id: NodeId, globals: &BTreeMap<NodeId, Mat4>, sampled_local: Transform) -> Result<(), CoreError> {
        if self.node(id).ok_or(CoreError::MissingNode(id))?.fx.and_then(|f|f.source).is_none() {return Ok(());}
        self.detach_transform_constraint(id,globals,sampled_local,true)
    }
    fn detach_transform_constraint(&mut self, id: NodeId, globals: &BTreeMap<NodeId, Mat4>, sampled_local: Transform, fx:bool) -> Result<(),CoreError> {
        let node = self.node(id).ok_or(CoreError::MissingNode(id))?;
        let parent = node.parent.and_then(|p| globals.get(&p)).copied().unwrap_or(Mat4::IDENTITY);
        let world = globals.get(&id).ok_or(CoreError::MissingNode(id))?;
        let delta = parent.inverse() * *world * sampled_local.matrix().inverse();
        let local_matrix = delta * node.transform.matrix();
        let local = Transform::from_matrix(local_matrix);
        let exact = |matrix: Mat4, transform: Transform| {
            let tolerance = matrix.to_cols_array().iter().map(|v| v.abs()).fold(1., f32::max) * 0.0001;
            transform.is_valid() && matrix.to_cols_array().iter().zip(transform.matrix().to_cols_array())
                .all(|(a,b)| (*a-b).abs() <= tolerance)
        };
        if !exact(local_matrix, local) { return Err(CoreError::Invalid("Неравномерный масштаб родителя мешает сохранить положение при отвязке".into())); }
        let adjustment = Transform::from_matrix(delta);
        let animated = self.clips.iter().flat_map(|c| &c.channels)
            .any(|c| c.node == id && c.property != crate::Property::Weights);
        if animated && (!exact(delta, adjustment) || adjustment.scale.max_element() - adjustment.scale.min_element() > 0.0001) {
            return Err(CoreError::Invalid("Для отвязки анимированного объекта нужен равномерный масштаб родителя".into()));
        }
        let rest = Transform::from_matrix(delta * node.rest_transform.unwrap_or(node.transform).matrix());
        if animated {
            for channel in self.clips.iter_mut().flat_map(|c| &mut c.channels).filter(|c| c.node == id) {
                for key in &mut channel.keys {
                    match channel.property {
                        crate::Property::Translation => {
                            key.value = delta.transform_point3(Vec3::from_slice(&key.value)).to_array().to_vec();
                            for t in [&mut key.in_tangent, &mut key.out_tangent].into_iter().flatten() {
                                *t = delta.transform_vector3(Vec3::from_slice(t)).to_array().to_vec();
                            }
                        }
                        crate::Property::Rotation => {
                            let map = |value: &[f32]| (adjustment.rotation * Quat::from_xyzw(value[0],value[1],value[2],value[3])).to_array().to_vec();
                            key.value = map(&key.value);
                            for t in [&mut key.in_tangent, &mut key.out_tangent].into_iter().flatten() { *t = map(t); }
                        }
                        crate::Property::Scale => {
                            key.value = (adjustment.scale * Vec3::from_slice(&key.value)).to_array().to_vec();
                            for t in [&mut key.in_tangent, &mut key.out_tangent].into_iter().flatten() {
                                *t = (adjustment.scale * Vec3::from_slice(t)).to_array().to_vec();
                            }
                        }
                        crate::Property::Weights => {}
                    }
                }
            }
        }
        let node = self.node_mut(id).unwrap();
        if fx {node.fx.as_mut().unwrap().source=None;}else{node.curve_binding=None;}
        node.transform = local; node.rest_transform = Some(if rest.is_valid() { rest } else { local });
        Ok(())
    }
    pub(crate) fn validate_curve_bindings(&self) -> Result<(), CoreError> {
        for node in &self.nodes {
            if let Some(binding) = node.curve_binding {
                binding.validate()?;
                if node.curve.is_some() || node.is_joint || (node.mesh.is_none() && node.camera.is_none() && !node.empty)
                    || self.node(binding.curve).is_none_or(|n| n.curve.is_none())
                { return Err(CoreError::Invalid("К кривой можно привязать меш, камеру или пустышку".into())); }
            }
        }
        for light in &self.lights {
            if let Some(binding) = light.curve_binding {
                binding.validate()?;
                if self.node(binding.curve).is_none_or(|n| n.curve.is_none()) { return Err(CoreError::Invalid("Кривая света отсутствует".into())); }
            }
        }
        Ok(())
    }
}
