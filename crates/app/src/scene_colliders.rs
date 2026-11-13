//! Cheap geometry snapshots on the UI thread; triangle transforms/BVH construction on workers.
use super::*;
use poseforge_core::{MorphTarget, PhysicsKind, SharedBuffer, Vertex};
use poseforge_simulation::mesh_contact::{MeshCollider, Triangle};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Input {
    vertices: SharedBuffer<Vertex>,
    indices: SharedBuffer<u32>,
    morphs: Vec<MorphTarget>,
    weights: Vec<f32>,
    palette: Vec<Mat4>,
    model: Mat4,
}

pub(super) fn inputs(project: &Project, globals: &BTreeMap<u64, Mat4>, render: bool,
    exclude: Option<u64>, space: Mat4, evaluated: Option<&poseforge_core::EvaluatedScene>,
    targets: Option<&[poseforge_core::CollisionTarget]>) -> Vec<Input> {
    let visible = project.visible_node_ids(render);
    let selected = targets.map(|list| list.iter().filter(|entry| entry.enabled).map(|entry| entry.node).collect::<BTreeSet<_>>());
    project.nodes.iter().filter_map(|node| {
        if Some(node.id) == exclude || !visible.contains(&node.id)
            || selected.as_ref().map_or_else(
                || node.physics.is_none_or(|p| !p.visible || p.kind != PhysicsKind::Collider),
                |ids| !ids.contains(&node.id)) { return None; }
        let mesh = evaluated.and_then(|e|e.modifier_meshes.get(&node.id).or_else(||e.text_meshes.get(&node.id))).or_else(||project.mesh(node.mesh?))?;
        let global = *globals.get(&node.id)?;
        // Skin matrices act in world space; the final conversion puts both rigid
        // and skinned obstacles in the same fluid-local (or world) coordinate system.
        let palette = node.skin.iter().flat_map(|skin| skin.joints.iter().zip(&skin.inverse_bind_matrices)
            .map(|(joint, bind)| space * globals.get(joint).copied().unwrap_or(Mat4::IDENTITY) * *bind)).collect();
        let weights = evaluated.and_then(|scene| scene.morph_weights.get(&node.id)).unwrap_or(&node.morph_weights).clone();
        Some(Input { vertices: mesh.vertices.clone(), indices: mesh.indices.clone(),
            morphs: mesh.morph_targets.clone(), weights, palette, model: space * global })
    }).collect()
}

pub(super) fn triangles(inputs: &[Input], cancel: &AtomicBool) -> Result<Vec<Vec<Triangle>>, String> {
    if inputs.len() > 64 || inputs.iter().map(|i| i.indices.len() / 3).sum::<usize>() > 200_000 {
        return Err("Столкновения: не более 64 мешей и 200000 треугольников суммарно".into());
    }
    let mut meshes = Vec::with_capacity(inputs.len());
    for input in inputs {
        if !input.model.is_finite() || input.model.determinant().abs() < 1e-8 {
            return Err("Коллайдер: восстановите ненулевой масштаб меша".into());
        }
        let mut points = Vec::with_capacity(input.vertices.len());
        for (i, v) in input.vertices.iter().enumerate() {
            if i % 256 == 0 && cancel.load(Ordering::Relaxed) { return Err("Подготовка столкновений отменена".into()); }
            let mut point = Vec3::from_array(v.position);
            for (target, weight) in input.morphs.iter().zip(&input.weights) {
                if let Some(delta) = target.positions.get(i) { point += Vec3::from_array(*delta) * *weight; }
            }
            let mut model = input.model;
            if !input.palette.is_empty() && v.weights.iter().sum::<f32>() > 0.0001 {
                model = Mat4::ZERO;
                for (&joint, &weight) in v.joints.iter().zip(&v.weights) {
                    model += input.palette.get(joint as usize).copied().unwrap_or(input.model) * weight;
                }
            }
            points.push(model.transform_point3(point));
        }
        let mut triangles = Vec::with_capacity(input.indices.len() / 3);
        for (i, indices) in input.indices.chunks_exact(3).enumerate() {
            if i % 256 == 0 && cancel.load(Ordering::Relaxed) { return Err("Подготовка столкновений отменена".into()); }
            let point = |j: u32| points.get(j as usize).copied().ok_or_else(|| "Повреждённые индексы коллайдера".to_string());
            triangles.push(Triangle([point(indices[0])?, point(indices[1])?, point(indices[2])?]));
        }
        meshes.push(triangles);
    }
    Ok(meshes)
}

pub(super) fn surface(inputs: &[Input], cancel: &AtomicBool) -> Result<MeshCollider, String> {
    MeshCollider::new(triangles(inputs, cancel)?.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{box_mesh, Node, PhysicsSettings};
    fn project() -> Project {
        let mut p = Project::default();
        let mut node = Node::new(1, "Obstacle"); node.mesh = Some(2);
        node.physics = Some(PhysicsSettings { kind: PhysicsKind::Collider, ..Default::default() });
        p.nodes.push(node); p.meshes.push(box_mesh(2, "Cube", Vec3::splat(0.2), None)); p
    }
    #[test]
    fn collider_visibility_parent_visibility_and_receiver_exclusion_are_respected() {
        let mut p = project(); let mut parent = Node::new(3, "Parent"); parent.visible = false;
        p.nodes[0].parent = Some(3); p.nodes.push(parent);
        let collect = |p: &Project, render, exclude| inputs(p, &p.global_transforms().unwrap(), render, exclude, Mat4::IDENTITY, None, None);
        assert!(collect(&p, false, None).is_empty());
        p.node_mut(3).unwrap().visible = true;
        assert_eq!(collect(&p, false, None).len(), 1);
        assert!(collect(&p, false, Some(1)).is_empty());
        p.nodes[0].render_visible = false;
        assert!(collect(&p, true, None).is_empty());
        assert_eq!(collect(&p, false, None).len(), 1);
        p.nodes[0].physics.as_mut().unwrap().visible = false;
        assert!(collect(&p, false, None).is_empty());
    }
    #[test]
    fn collider_triangles_use_morphed_geometry_and_requested_coordinate_space() {
        let mut p = project();
        p.nodes[0].transform.translation = Vec3::new(3.,2.,-4.);
        p.nodes[0].transform.rotation = Quat::from_rotation_z(0.4);
        p.nodes[0].transform.scale = Vec3::new(2.,0.5,-1.);
        let count = p.meshes[0].vertices.len();
        p.meshes[0].morph_targets.push(MorphTarget { name: "Raise".into(), positions: vec![[0.,0.2,0.];count].into(), normals: vec![[0.;3];count].into() });
        p.nodes[0].morph_weights = vec![0.5];
        let globals = p.global_transforms().unwrap();
        let space = Mat4::from_translation(Vec3::new(-2.,-1.,3.));
        let snapshots = inputs(&p, &globals, false, None, space, None, None);
        let triangles = triangles(&snapshots, &AtomicBool::new(false)).unwrap();
        let vertex = &p.meshes[0].vertices[p.meshes[0].indices[0] as usize];
        let expected = (space*globals[&1]).transform_point3(Vec3::from_array(vertex.position)+Vec3::Y*0.1);
        assert!(triangles[0][0].0[0].distance(expected) < 0.00001);
        let mut renamed = p.clone(); renamed.meshes[0].name = "Renamed".into(); renamed.meshes[0].material = Some(99);
        assert_eq!(inputs(&renamed, &globals, false, None, space, None, None), snapshots);
        assert!(super::triangles(&snapshots, &AtomicBool::new(true)).is_err());
    }
    #[test]
    fn explicit_targets_accept_plain_meshes_and_empty_or_disabled_means_no_contact() {
        let mut p=project();p.nodes[0].physics=None;
        let globals=p.global_transforms().unwrap();
        let enabled=[poseforge_core::CollisionTarget {node:1,enabled:true}];
        let disabled=[poseforge_core::CollisionTarget {node:1,enabled:false}];
        assert!(inputs(&p,&globals,false,None,Mat4::IDENTITY,None,None).is_empty());
        assert_eq!(inputs(&p,&globals,false,None,Mat4::IDENTITY,None,Some(&enabled)).len(),1);
        assert!(inputs(&p,&globals,false,None,Mat4::IDENTITY,None,Some(&disabled)).is_empty());
        assert!(inputs(&p,&globals,false,None,Mat4::IDENTITY,None,Some(&[])).is_empty());
        assert!(inputs(&p,&globals,false,Some(1),Mat4::IDENTITY,None,Some(&enabled)).is_empty());
        p.nodes[0].render_visible=false;
        assert!(inputs(&p,&globals,true,None,Mat4::IDENTITY,None,Some(&enabled)).is_empty());
    }
}
