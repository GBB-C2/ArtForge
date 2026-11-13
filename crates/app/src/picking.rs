//! CPU triangle picking using the same morph-before-skin order as the GPU.
//! Runs on pointer-down, not every frame. Bone/controller picking remains the
//! caller's first priority. Coordinates use the same units for pointer/viewport.
#[path = "picking_acceleration.rs"]
mod acceleration;
use glam::{DVec3, Mat4, Vec3, Vec4};
use poseforge_core::{AlphaMode, EvaluatedScene, Material, Node, Project, Texture};
use std::collections::{BTreeMap, BTreeSet};

/// Return the nearest selectable mesh node beneath the pointer.
///
/// The view-projection matrix uses wgpu's [0, 1] depth convention. Geometry is
/// culled using each material's double-sided flag and the renderer's mirrored
/// object winding. Hidden and locked ancestor branches are excluded. Morph
/// weights come from the evaluated scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceHit {
    pub node: u64,
    pub face: usize,
    pub position: Vec3,
}
pub fn pick_mesh(
    project: &Project,
    eval: &EvaluatedScene,
    view_projection: Mat4,
    viewport: [f32; 4],
    pointer: [f32; 2],
) -> Option<u64> {
    pick_surface(project, eval, view_projection, viewport, pointer).map(|hit| hit.node)
}
pub fn pick_surface(
    project: &Project,
    eval: &EvaluatedScene,
    view_projection: Mat4,
    viewport: [f32; 4],
    pointer: [f32; 2],
) -> Option<SurfaceHit> {
    pick_surface_impl(project,eval,view_projection,viewport,pointer,false)
}
/// Locked surfaces still block a material drop; they must not count as empty space.
pub fn pick_material_surface(project: &Project, eval: &EvaluatedScene, view_projection: Mat4, viewport: [f32;4], pointer: [f32;2]) -> Option<SurfaceHit> {
    pick_surface_impl(project,eval,view_projection,viewport,pointer,true)
}
fn pick_surface_impl(project: &Project, eval: &EvaluatedScene, view_projection: Mat4, viewport: [f32;4], pointer: [f32;2], include_locked: bool) -> Option<SurfaceHit> {
    if viewport
        .iter()
        .chain(pointer.iter())
        .any(|v| !v.is_finite())
        || viewport[2] <= 0.
        || viewport[3] <= 0.
        || pointer[0] < viewport[0]
        || pointer[0] >= viewport[0] + viewport[2]
        || pointer[1] < viewport[1]
        || pointer[1] >= viewport[1] + viewport[3]
        || !view_projection.is_finite()
    {
        return None;
    }
    let inverse = view_projection.inverse();
    if !inverse.is_finite() {
        return None;
    }
    let x = ((pointer[0] - viewport[0]) / viewport[2]) * 2. - 1.;
    let y = 1. - ((pointer[1] - viewport[1]) / viewport[3]) * 2.;
    let unproject = |z: f32| {
        let h = inverse * Vec4::new(x, y, z, 1.);
        if !h.is_finite() || h.w == 0. {
            None
        } else {
            let p = h.truncate() / h.w;
            p.is_finite().then_some(p.as_dvec3())
        }
    };
    // Using the middle of the depth range also supports an infinite far plane.
    let origin = unproject(0.)?;
    let direction = (unproject(0.5)? - origin).try_normalize()?;
    let nodes: BTreeMap<_, _> = project.nodes.iter().map(|n| (n.id, n)).collect();
    let meshes: BTreeMap<_, _> = project.meshes.iter().map(|m| (m.id, m)).collect();
    let materials: BTreeMap<_, _> = project.materials.iter().map(|m| (m.id, m)).collect();
    let textures: BTreeMap<_, _> = project.textures.iter().map(|t| (t.id, t)).collect();
    let selectable_ids = if include_locked { project.visible_node_ids(false) } else { project.selectable_node_ids() };
    let mut best_distance = f64::INFINITY;
    let mut best_id = None;
    for node in &project.nodes {
        if node.shadow_only { continue; }
        if (!include_locked && !selectable(node, &nodes)) || !selectable_ids.contains(&node.id) {
            continue;
        }
        let Some(mesh) = eval.text_meshes.get(&node.id).or_else(||node.mesh.and_then(|id| meshes.get(&id).copied())) else {
            continue;
        };
        let Some(&model) = eval.global.get(&node.id) else {
            continue;
        };
        if !model.is_finite() {
            continue;
        }
        let skin_matrices = match &node.skin {
            Some(skin) => {
                let inverse_model = model.inverse();
                if !inverse_model.is_finite()
                    || skin.joints.len() != skin.inverse_bind_matrices.len()
                {
                    continue;
                }
                let matrices: Option<Vec<_>> = skin
                    .joints
                    .iter()
                    .zip(&skin.inverse_bind_matrices)
                    .map(|(id, bind)| {
                        let matrix = inverse_model * *eval.global.get(id)? * *bind;
                        matrix.is_finite().then_some(matrix)
                    })
                    .collect();
                let Some(matrices) = matrices else { continue };
                Some(matrices)
            }
            None => None,
        };
        let weights = eval
            .morph_weights
            .get(&node.id)
            .unwrap_or(&node.morph_weights);
        let geometry = acceleration::cached(mesh, model, &skin_matrices, weights, || {
            mesh.vertices
                .iter()
                .enumerate()
                .map(|(index, vertex)| {
                    let mut position = Vec3::from_array(vertex.position);
                    for (target, &weight) in mesh.morph_targets.iter().zip(weights) {
                        if weight != 0. {
                            position += Vec3::from_array(*target.positions.get(index)?) * weight;
                        }
                    }
                    let point = position.extend(1.);
                    let local = match &skin_matrices {
                        Some(joints) if vertex.weights.iter().sum::<f32>() > 0.0001 => {
                            let mut weighted = Vec4::ZERO;
                            for influence in 0..4 {
                                let weight = vertex.weights[influence];
                                if weight != 0. {
                                    weighted += *joints.get(vertex.joints[influence] as usize)?
                                        * point
                                        * weight;
                                }
                            }
                            weighted
                        }
                        _ => point,
                    };
                    let world = model * local;
                    if !world.is_finite() || world.w == 0. {
                        None
                    } else {
                        let p = world.truncate() / world.w;
                        p.is_finite().then_some(p.as_dvec3())
                    }
                })
                .collect()
        });
        let Some(geometry) = geometry else { continue };
        let positions = &geometry.positions;
        if positions.is_empty() {
            continue;
        }
        // The GPU selects Cw only for a reflected object matrix. Use the actual
        // deformed triangle, not vertex normals or per-joint determinant guesses.
        let winding = if model.determinant() < 0. { -1. } else { 1. };
        for triangle_index in geometry.candidates(origin, direction) {
            let triangle = &mesh.indices[triangle_index * 3..triangle_index * 3 + 3];
            let face = geometry.faces[triangle_index];
            let material = mesh
                .face_material(face)
                .and_then(|id| materials.get(&id).copied()).filter(|m| m.visible);
            let double_sided = material.is_some_and(|m| m.double_sided);
            let [a, b, c] = [
                triangle[0] as usize,
                triangle[1] as usize,
                triangle[2] as usize,
            ];
            let Some((&pa, &pb, &pc)) = positions
                .get(a)
                .zip(positions.get(b))
                .zip(positions.get(c))
                .map(|((a, b), c)| (a, b, c))
            else {
                continue;
            };
            if !double_sided && (pb - pa).cross(pc - pa).dot(-direction) * winding <= 0. {
                continue;
            }
            let Some((distance, u, v)) = intersect_triangle(origin, direction, pa, pb, pc) else {
                continue;
            };
            if distance >= best_distance {
                continue;
            }
            let hit = origin + direction * distance;
            let clip = view_projection * hit.as_vec3().extend(1.);
            if !clip.is_finite()
                || clip.w <= 0.
                || clip.z < -1e-5 * clip.w
                || clip.z > (1. + 1e-5) * clip.w
            {
                continue;
            }
            if let Some(material) = material {
                let va = &mesh.vertices[a];
                let vb = &mesh.vertices[b];
                let vc = &mesh.vertices[c];
                let bary = [1. - u - v, u, v];
                let uv = [
                    (va.uv[0] as f64 * bary[0]
                        + vb.uv[0] as f64 * bary[1]
                        + vc.uv[0] as f64 * bary[2]) as f32,
                    (va.uv[1] as f64 * bary[0]
                        + vb.uv[1] as f64 * bary[1]
                        + vc.uv[1] as f64 * bary[2]) as f32,
                ];
                if !material_covers(material, &textures, uv) {
                    continue;
                }
            }
            best_distance = distance;
            best_id = Some(SurfaceHit {
                node: node.id,
                face,
                position: hit.as_vec3(),
            });
        }
    }
    best_id
}

fn selectable(node: &Node, nodes: &BTreeMap<u64, &Node>) -> bool {
    let mut current = node;
    let mut seen = BTreeSet::new();
    loop {
        if !current.visible || current.locked || !seen.insert(current.id) {
            return false;
        }
        match current.parent {
            Some(parent) => match nodes.get(&parent) {
                Some(parent) => current = parent,
                None => return false,
            },
            None => return true,
        }
    }
}

fn intersects_bounds(
    origin: DVec3,
    direction: DVec3,
    minimum: DVec3,
    maximum: DVec3,
    max_distance: f64,
) -> bool {
    let mut enter: f64 = 0.;
    let mut exit = max_distance;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-15 {
            if origin[axis] < minimum[axis] || origin[axis] > maximum[axis] {
                return false;
            }
            continue;
        }
        let a = (minimum[axis] - origin[axis]) / direction[axis];
        let b = (maximum[axis] - origin[axis]) / direction[axis];
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return false;
        }
    }
    true
}

/// Möller–Trumbore in f64 avoids rejecting small, valid imported triangles.
fn intersect_triangle(
    origin: DVec3,
    direction: DVec3,
    a: DVec3,
    b: DVec3,
    c: DVec3,
) -> Option<(f64, f64, f64)> {
    let ab = b - a;
    let ac = c - a;
    let p = direction.cross(ac);
    let determinant = ab.dot(p);
    let area_bound = ab.length() * ac.length();
    if !area_bound.is_finite() || area_bound == 0. || determinant.abs() <= 1e-12 * area_bound {
        return None;
    }
    let inverse = determinant.recip();
    let offset = origin - a;
    let u = offset.dot(p) * inverse;
    let q = offset.cross(ab);
    let v = direction.dot(q) * inverse;
    let epsilon = 1e-9;
    if u < -epsilon || v < -epsilon || u + v > 1. + epsilon {
        return None;
    }
    let distance = ac.dot(q) * inverse;
    (distance >= 0. && distance.is_finite()).then_some((distance, u, v))
}

fn material_covers(material: &Material, textures: &BTreeMap<u64, &Texture>, uv: [f32; 2]) -> bool {
    if !material.visible { return true; }
    if material.alpha_mode == AlphaMode::Opaque {
        return true;
    }
    let texture_alpha = material
        .base_color_texture
        .and_then(|id| textures.get(&id))
        .map(|t| sample_alpha(t, uv))
        .unwrap_or(1.);
    let alpha = material.base_color[3] * texture_alpha;
    match material.alpha_mode {
        AlphaMode::Mask => alpha >= material.alpha_cutoff,
        AlphaMode::Blend => alpha > 1. / 255.,
        AlphaMode::Opaque => true,
    }
}

/// Bilinear alpha in a repeating texture, matching the renderer's sampler.
fn sample_alpha(texture: &Texture, uv: [f32; 2]) -> f32 {
    if texture.width == 0 || texture.height == 0 || uv.iter().any(|u| !u.is_finite()) {
        return 1.;
    }
    let x = uv[0].rem_euclid(1.) * texture.width as f32 - 0.5;
    let y = uv[1].rem_euclid(1.) * texture.height as f32 - 0.5;
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let fetch = |x: i64, y: i64| {
        let x = x.rem_euclid(texture.width as i64) as usize;
        let y = y.rem_euclid(texture.height as i64) as usize;
        texture
            .rgba8
            .get((y * texture.width as usize + x) * 4 + 3)
            .copied()
            .unwrap_or(255) as f32
            / 255.
    };
    let tx = x - x.floor();
    let ty = y - y.floor();
    let top = fetch(ix, iy) * (1. - tx) + fetch(ix + 1, iy) * tx;
    let bottom = fetch(ix, iy + 1) * (1. - tx) + fetch(ix + 1, iy + 1) * tx;
    top * (1. - ty) + bottom * ty
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Mesh, MorphTarget, Skin, Transform, Vertex};

    fn triangle(id: u64) -> Mesh {
        Mesh {
            id,
            name: "Triangle".into(),
            vertices: [[-0.5, -0.5, 0.], [0.5, -0.5, 0.], [0., 0.5, 0.]]
                .into_iter()
                .map(|position| Vertex {
                    position,
                    ..Vertex::default()
                })
                .collect(),
            indices: vec![0, 1, 2].into(),
            material: None,
            topology: None,
            morph_targets: vec![],
        }
    }
    fn camera() -> Mat4 {
        Mat4::perspective_rh(60f32.to_radians(), 1., 0.1, 30.)
            * Mat4::look_at_rh(Vec3::new(0., 0., 5.), Vec3::ZERO, Vec3::Y)
    }
    fn pointer_at(position: Vec3, vp: Mat4) -> [f32; 2] {
        let clip = vp * position.extend(1.);
        let ndc = clip.truncate() / clip.w;
        [
            100. + (ndc.x * 0.5 + 0.5) * 400.,
            50. + (0.5 - ndc.y * 0.5) * 400.,
        ]
    }
    fn pick(project: &Project, pointer: [f32; 2]) -> Option<u64> {
        pick_mesh(
            project,
            &project.evaluated(None, 0.).unwrap(),
            camera(),
            [100., 50., 400., 400.],
            pointer,
        )
    }
    fn scene() -> Project {
        let mut p = Project::default();
        p.meshes.push(triangle(100));
        let mut far = Node::new(1, "Far");
        far.mesh = Some(100);
        let mut near = Node::new(2, "Near");
        near.mesh = Some(100);
        near.transform.translation = Vec3::Z;
        p.nodes = vec![far, near];
        p
    }

    #[test]
    fn nearest_triangle_wins_regardless_of_node_order_and_viewport_offset() {
        let mut p = scene();
        assert_eq!(pick(&p, [300., 250.]), Some(2));
        p.nodes.reverse();
        assert_eq!(pick(&p, [300., 250.]), Some(2));
        assert_eq!(pick(&p, [99., 250.]), None);
        assert_eq!(pick(&p, [300., 49.]), None);
    }

    #[test]
    fn shadow_only_surface_does_not_intercept_picking_but_stays_selectable_in_scene() {
        let mut p = scene();
        p.node_mut(2).unwrap().shadow_only = true;
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        assert!(p.selectable_node_ids().contains(&2));
        p.nodes.reverse();
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        p.node_mut(1).unwrap().shadow_only = true;
        assert_eq!(pick(&p, [300., 250.]), None);
        p.node_mut(2).unwrap().shadow_only = false;
        assert_eq!(pick(&p, [300., 250.]), Some(2));
    }

    #[test]
    fn hidden_or_locked_ancestor_excludes_descendants() {
        let mut p = scene();
        let mut group = Node::new(3, "Group");
        group.visible = false;
        p.node_mut(2).unwrap().parent = Some(3);
        p.nodes.push(group);
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        p.node_mut(3).unwrap().visible = true;
        p.node_mut(3).unwrap().locked = true;
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        p.node_mut(3).unwrap().locked = false;
        assert_eq!(pick(&p, [300., 250.]), Some(2));
    }

    #[test]
    fn morph_then_skin_matches_visible_geometry_even_with_mesh_transform() {
        let mut p = Project::default();
        let mut mesh = triangle(100);
        for vertex in &mut mesh.vertices {
            vertex.weights = [1., 0., 0., 0.];
        }
        mesh.morph_targets.push(MorphTarget {
            name: "Shift".into(),
            positions: vec![[1., 0., 0.]; 3].into(),
            normals: vec![].into(),
        });
        p.meshes.push(mesh);
        let mut joint = Node::new(10, "Joint");
        joint.transform = Transform {
            translation: Vec3::new(1., 0., 0.),
            rotation: glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ..Transform::default()
        };
        let mut node = Node::new(1, "Skin");
        node.mesh = Some(100);
        node.transform.translation = Vec3::new(7., 0., 0.);
        node.morph_weights = vec![0.]; // evaluated override must take precedence
        node.skin = Some(Skin {
            joints: vec![10],
            inverse_bind_matrices: vec![Mat4::IDENTITY],
        });
        p.nodes = vec![node, joint];
        let mut evaluated = p.evaluated(None, 0.).unwrap();
        evaluated.morph_weights.insert(1, vec![1.]);
        let expected = Vec3::new(1., 1., 0.);
        assert_eq!(
            pick_mesh(
                &p,
                &evaluated,
                camera(),
                [100., 50., 400., 400.],
                pointer_at(expected, camera())
            ),
            Some(1)
        );
        assert_eq!(
            pick_mesh(
                &p,
                &evaluated,
                camera(),
                [100., 50., 400., 400.],
                pointer_at(Vec3::new(1., 0., 0.), camera())
            ),
            None
        );
    }

    #[test]
    fn backface_picking_matches_material_and_reflected_object_pipeline() {
        let mut p = scene();
        p.nodes.retain(|node| node.id == 1);
        p.meshes[0].material = Some(200);
        p.materials.push(Material {
            id: 200,
            ..Material::default()
        });
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        p.meshes[0].indices = vec![0, 2, 1].into();
        assert_eq!(
            pick(&p, [300., 250.]),
            None,
            "single-sided back is invisible"
        );
        p.materials[0].double_sided = true;
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        p.materials[0].double_sided = false;
        p.meshes[0].indices = vec![0, 1, 2].into();
        let mut parent = Node::new(10, "Reflected parent");
        parent.transform.scale.x = -1.;
        p.nodes[0].parent = Some(10);
        p.nodes.push(parent);
        assert_eq!(
            pick(&p, [300., 250.]),
            Some(1),
            "renderer switches to clockwise for reflected global matrix"
        );
        let back_camera = Mat4::perspective_rh(60f32.to_radians(), 1., 0.1, 30.)
            * Mat4::look_at_rh(Vec3::new(0., 0., -5.), Vec3::ZERO, Vec3::Y);
        assert_eq!(
            pick_mesh(
                &p,
                &p.evaluated(None, 0.).unwrap(),
                back_camera,
                [100., 50., 400., 400.],
                [300., 250.]
            ),
            None
        );
        p.materials[0].double_sided = true;
        assert_eq!(
            pick_mesh(
                &p,
                &p.evaluated(None, 0.).unwrap(),
                back_camera,
                [100., 50., 400., 400.],
                [300., 250.]
            ),
            Some(1)
        );
    }

    #[test]
    fn skinned_winding_uses_deformed_positions_and_object_pipeline_not_joint_sign() {
        let mut p = scene();
        p.nodes.retain(|node| node.id == 1);
        for vertex in &mut p.meshes[0].vertices {
            vertex.weights = [1., 0., 0., 0.];
        }
        p.nodes[0].skin = Some(Skin {
            joints: vec![10],
            inverse_bind_matrices: vec![Mat4::IDENTITY],
        });
        let mut joint = Node::new(10, "Reflecting joint");
        joint.transform.scale.x = -1.;
        p.nodes.push(joint);
        assert_eq!(
            pick(&p, [300., 250.]),
            None,
            "reflected skin reverses the triangle while the positive object still uses CCW"
        );
        p.meshes[0].material = Some(200);
        p.materials.push(Material {
            id: 200,
            double_sided: true,
            ..Material::default()
        });
        assert_eq!(pick(&p, [300., 250.]), Some(1));
    }

    #[test]
    fn alpha_mask_hole_reveals_far_mesh() {
        let mut p = scene();
        let mut near_mesh = triangle(101);
        near_mesh.material = Some(200);
        p.meshes.push(near_mesh);
        p.node_mut(2).unwrap().mesh = Some(101);
        p.materials.push(Material {
            id: 200,
            base_color_texture: Some(300),
            alpha_mode: AlphaMode::Mask,
            ..Material::default()
        });
        p.textures.push(Texture {
            id: 300,
            name: "Hole".into(),
            width: 1,
            height: 1,
            rgba8: vec![255, 255, 255, 0].into(),
        });
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        p.materials[0].visible = false;
        assert_eq!(pick(&p, [300., 250.]), Some(2), "hidden material uses opaque default geometry");
        p.materials[0].visible = true;
        assert_eq!(pick(&p, [300., 250.]), Some(1));
        p.textures[0].rgba8[3] = 255;
        assert_eq!(pick(&p, [300., 250.]), Some(2));
    }

    #[test]
    fn far_clipping_singular_camera_and_orthographic_view_are_safe() {
        let mut p = scene();
        p.nodes.retain(|n| n.id == 1);
        p.nodes[0].transform.translation.z = -40.;
        assert_eq!(pick(&p, [300., 250.]), None);
        let p = scene();
        let eval = p.evaluated(None, 0.).unwrap();
        assert_eq!(
            pick_mesh(&p, &eval, Mat4::ZERO, [0., 0., 400., 400.], [200., 200.]),
            None
        );
        let vp = Mat4::orthographic_rh(-2., 2., -2., 2., 0.1, 30.)
            * Mat4::look_at_rh(Vec3::new(0., 0., 5.), Vec3::ZERO, Vec3::Y);
        assert_eq!(
            pick_mesh(&p, &eval, vp, [0., 0., 400., 400.], [200., 200.]),
            Some(2)
        );
    }
}
