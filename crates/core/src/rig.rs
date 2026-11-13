use crate::{CoreError, NodeId, Project, Transform};
use glam::{Quat, Vec3};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct TwoBoneSolution {
    pub root: Vec3,
    pub middle: Vec3,
    pub end: Vec3,
    pub reached: bool,
}
/// Analytic two-link IK. The pole chooses the bend half-plane. Unreachable
/// targets are clamped geometrically; neither segment is stretched.
pub fn solve_two_bone(
    root: Vec3,
    middle: Vec3,
    end: Vec3,
    target: Vec3,
    pole: Vec3,
) -> Result<TwoBoneSolution, CoreError> {
    if [root, middle, end, target, pole]
        .iter()
        .any(|v| !v.is_finite())
    {
        return Err(CoreError::Invalid("нечисловая IK-цель".into()));
    }
    let a = root.distance(middle);
    let b = middle.distance(end);
    if a < 1e-6 || b < 1e-6 {
        return Err(CoreError::Invalid(
            "IK требует два ненулевых сегмента".into(),
        ));
    }
    let to_target = target - root;
    let distance = to_target.length();
    let axis = if distance > 1e-7 {
        to_target / distance
    } else {
        (end - root).try_normalize().unwrap_or(Vec3::Y)
    };
    let min = (a - b).abs().max(1e-6);
    let max = a + b;
    let d = distance.clamp(min, max);
    let along = (a * a - b * b + d * d) / (2. * d);
    let height = (a * a - along * along).max(0.).sqrt();
    let projected = (pole - root) - axis * (pole - root).dot(axis);
    let fallback = (middle - root) - axis * (middle - root).dot(axis);
    let bend = projected
        .try_normalize()
        .or_else(|| fallback.try_normalize())
        .unwrap_or_else(|| axis.any_orthonormal_vector());
    Ok(TwoBoneSolution {
        root,
        middle: root + axis * along + bend * height,
        end: root + axis * d,
        reached: (d - distance).abs() < 1e-4,
    })
}
/// Return local FK rotations that realize the analytic solution, preserving
/// translation/scale and leaving the authored project untouched.
pub fn two_bone_transforms(
    project: &Project,
    locals: &BTreeMap<NodeId, Transform>,
    root: NodeId,
    middle: NodeId,
    end: NodeId,
    target: Vec3,
    pole: Vec3,
) -> Result<BTreeMap<NodeId, Transform>, CoreError> {
    if project
        .node(middle)
        .ok_or(CoreError::MissingNode(middle))?
        .parent
        != Some(root)
        || project.node(end).ok_or(CoreError::MissingNode(end))?.parent != Some(middle)
    {
        return Err(CoreError::Invalid(
            "IK требует непосредственную цепь из трёх суставов".into(),
        ));
    }
    let globals = project.globals_for(locals)?;
    for id in [root, middle, end] {
        let scale = globals[&id].to_scale_rotation_translation().0;
        if scale.min_element() <= 0. || scale.max_element() - scale.min_element() > 1e-4 {
            return Err(CoreError::Invalid("IK требует равномерный положительный масштаб цепи; примените масштаб до настройки IK".into()));
        }
    }
    let r = globals[&root].transform_point3(Vec3::ZERO);
    let m = globals[&middle].transform_point3(Vec3::ZERO);
    let e = globals[&end].transform_point3(Vec3::ZERO);
    let solution = solve_two_bone(r, m, e, target, pole)?;
    let mut result = locals.clone();
    let root_global_rotation = globals[&root].to_scale_rotation_translation().1;
    let root_parent_rotation = project
        .node(root)
        .and_then(|n| n.parent)
        .map(|p| globals[&p].to_scale_rotation_translation().1)
        .unwrap_or(Quat::IDENTITY);
    let delta = Quat::from_rotation_arc((m - r).normalize(), (solution.middle - r).normalize());
    let mut rt = *result
        .get(&root)
        .unwrap_or(&project.node(root).unwrap().transform);
    rt.rotation = (root_parent_rotation.inverse() * delta * root_global_rotation).normalize();
    result.insert(root, rt);
    let globals = project.globals_for(&result)?;
    let m = globals[&middle].transform_point3(Vec3::ZERO);
    let e = globals[&end].transform_point3(Vec3::ZERO);
    let middle_global_rotation = globals[&middle].to_scale_rotation_translation().1;
    let parent_rotation = globals[&root].to_scale_rotation_translation().1;
    let delta = Quat::from_rotation_arc((e - m).normalize(), (solution.end - m).normalize());
    let mut mt = *result
        .get(&middle)
        .unwrap_or(&project.node(middle).unwrap().transform);
    mt.rotation = (parent_rotation.inverse() * delta * middle_global_rotation).normalize();
    result.insert(middle, mt);
    Ok(BTreeMap::from([
        (root, result[&root]),
        (middle, result[&middle]),
    ]))
}
