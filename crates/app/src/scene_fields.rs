//! Snapshot only enabled scene fields. Solver jobs own the resulting immutable set.
use super::*;
use poseforge_simulation::fields::{Field,Fields};
pub(super) fn collect(project:&Project,globals:&BTreeMap<u64,Mat4>,render:bool,target:usize,space:Mat4)->Fields {
    if !project.nodes.iter().any(|n|n.force_field.is_some()) {return Fields::default();}
    let visible=project.visible_node_ids(render);
    let items=project.nodes.iter().filter_map(|n| {
        let s=n.force_field.filter(|s|s.enabled&&s.targets.get(target)==Some(&true)&&visible.contains(&n.id))?;
        let m=*globals.get(&n.id)?;let scale=m.to_scale_rotation_translation().0.abs().max_element();
        let axis=m.transform_vector3(Vec3::Z).try_normalize()?;
        Some(Field{kind:s.kind as u8,center:m.transform_point3(Vec3::ZERO),axis,strength:s.strength,radius:s.radius*scale,
            falloff:s.falloff,damping:s.damping})
    }).collect();
    Fields::new(items,space).unwrap_or_default()
}
