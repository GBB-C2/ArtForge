//! Render-only polygon batches. Authoring objects and quad boundaries stay intact.
use poseforge_core::{Mat4, Mesh, Project};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

#[derive(Clone)]
struct Batch { mesh: Mesh, painted: bool }

type Expanded = (Arc<Project>, Arc<BTreeMap<u64, Mat4>>);
#[derive(Default)]
pub(crate) struct MaterialParts {
    meshes: HashMap<u64, (Mesh, Vec<Batch>)>,
    source: Option<Project>,
    expanded: Option<Arc<Project>>,
    ray_project: Option<Arc<Project>>,
    globals: BTreeMap<u64, Mat4>,
    expanded_globals: Arc<BTreeMap<u64, Mat4>>,
    owners: Vec<(u64, u64)>,
    ray_object_ids: HashMap<u64, f32>,
    layer_masks: HashMap<u64, f32>,
    // Source material ID, raster copy index, overlay node ID.
    overlays: Vec<(u64, usize, u64)>,
}
impl MaterialParts {
    pub fn layer_mask(&self, node: u64) -> Option<f32> { self.layer_masks.get(&node).copied() }
    pub fn ray_object_id(&self, node: u64) -> Option<f32> { self.ray_object_ids.get(&node).copied() }
    pub fn ray_scene(&self) -> Option<Arc<Project>> { self.ray_project.clone() }
    pub fn selection_parts(&self, selected: &[u64]) -> std::collections::BTreeSet<u64> {
        selected
            .iter()
            .copied()
            .chain(
                self.owners
                    .iter()
                    .filter(|(owner, _)| selected.contains(owner))
                    .map(|(_, part)| *part),
            )
            .collect()
    }
    pub fn scene(
        &mut self,
        source: &Project,
        globals: &BTreeMap<u64, Mat4>,
    ) -> Result<Option<Expanded>, String> {
        if !source.meshes.iter().any(has_overrides) && source.nodes.iter().all(|n| n.material_layers.is_empty()) {
            *self = Self::default();
            return Ok(None);
        }
        let changed = self.source.as_ref() != Some(source);
        let rebuild = self.source.as_ref().is_none_or(|previous|
            previous.nodes.len() != source.nodes.len()
                || previous.nodes.iter().zip(&source.nodes).any(|(a,b)|
                    a.id != b.id || a.mesh != b.mesh || (a.mesh.is_some() && a != b))
                || previous.meshes != source.meshes
                || previous.materials.len() != source.materials.len()
                || previous.materials.iter().zip(&source.materials).any(|(a,b)| a.id != b.id || a.visible != b.visible));
        if rebuild {
            self.rebuild(source)?;
        } else if changed {
            // Cameras, joints and helpers do not change polygon partitioning.
            // Keep their authored data current for hierarchy/visibility checks.
            if self.source.as_ref().is_some_and(|old| old.nodes != source.nodes) {
                let expanded=Arc::make_mut(self.expanded.as_mut().unwrap());
                let rays=Arc::make_mut(self.ray_project.as_mut().unwrap());
                for (index,node) in source.nodes.iter().enumerate().filter(|(_,n)|n.mesh.is_none()) {
                    expanded.nodes[index]=node.clone();rays.nodes[index]=node.clone();
                }
            }
            // Material values and textures do not change polygon batches or BVH IDs.
            if self.source.as_ref().is_some_and(|old| old.materials != source.materials) {
                self.update_materials(source);
            }
            sync_scene_settings(Arc::make_mut(self.expanded.as_mut().unwrap()), source);
            sync_scene_settings(Arc::make_mut(self.ray_project.as_mut().unwrap()), source);
            self.source = Some(source.clone());
        }
        if rebuild || self.globals != *globals {
            self.globals.clone_from(globals);
            let mut expanded = globals.clone();
            for &(original, part) in &self.owners {
                if let Some(global) = globals.get(&original) {
                    expanded.insert(part, *global);
                }
            }
            self.expanded_globals = Arc::new(expanded);
        }
        Ok(Some((
            self.expanded.as_ref().unwrap().clone(),
            self.expanded_globals.clone(),
        )))
    }
    fn update_materials(&mut self, source: &Project) {
        let expanded = Arc::make_mut(self.expanded.as_mut().unwrap());
        expanded.materials[..source.materials.len()].clone_from_slice(&source.materials);
        let originals: HashMap<_,_> = source.materials.iter().map(|m|(m.id,m)).collect();
        self.layer_masks.clear();
        for &(id,index,node) in &self.overlays {
            let material = originals[&id];
            if material.alpha_mode == poseforge_core::AlphaMode::Mask {
                self.layer_masks.insert(node,material.alpha_cutoff);
            }
            let copy_id = expanded.materials[index].id;
            let mut copy = material.clone();
            copy.id = copy_id;
            copy.alpha_mode = poseforge_core::AlphaMode::Blend;
            expanded.materials[index] = copy;
        }
        Arc::make_mut(self.ray_project.as_mut().unwrap()).materials.clone_from(&source.materials);
    }
    fn rebuild(&mut self, source: &Project) -> Result<(), String> {
        self.meshes
            .retain(|id, _| source.mesh(*id).is_some_and(has_overrides));
        self.owners.clear(); self.ray_object_ids.clear(); self.layer_masks.clear(); self.overlays.clear();
        let mut project = source.clone();
        let mut next_mesh = source.meshes.iter().map(|m| m.id).max().unwrap_or(0);
        let mut next_node = source.nodes.iter().map(|n| n.id).max().unwrap_or(0);
        let mut parts = HashMap::new();
        for mesh in &mut project.meshes {
            if !has_overrides(mesh) {
                continue;
            }
            let entry = self
                .meshes
                .entry(mesh.id)
                .or_insert_with(|| (mesh.clone(), batches(mesh)));
            if entry.0 != *mesh {
                *entry = (mesh.clone(), batches(mesh));
            }
            parts.insert(mesh.id, entry.1.clone());
        }
        let mut meshes = HashMap::new();
        for mesh in &source.meshes {
            let Some(batches) = parts.get(&mesh.id) else {
                continue;
            };
            let mut ids = Vec::new();
            for (i, batch) in batches.iter().enumerate() {
                let mut copy = batch.mesh.clone();
                if i == 0 {
                    *project.meshes.iter_mut().find(|m| m.id == mesh.id).unwrap() = copy;
                    ids.push((mesh.id,batch.painted));
                } else {
                    next_mesh = next_mesh
                        .checked_add(1)
                        .ok_or("Закончились ID мешей для рендера")?;
                    copy.id = next_mesh;
                    ids.push((copy.id,batch.painted));
                    project.meshes.push(copy);
                }
            }
            meshes.insert(mesh.id, ids);
        }
        for node in &source.nodes {
            let Some(ids) = node.mesh.and_then(|id| meshes.get(&id)) else {
                continue;
            };
            if ids.first().is_some_and(|(_,painted)| *painted) {
                project.node_mut(node.id).unwrap().material_layers.clear();
            }
            for &(mesh,painted) in ids.iter().skip(1) {
                next_node = next_node
                    .checked_add(1)
                    .ok_or("Закончились ID узлов для рендера")?;
                let mut copy = node.clone();
                copy.id = next_node;
                copy.mesh = Some(mesh);
                copy.parent = Some(node.id);
                copy.camera = None;
                if painted { copy.material_layers.clear(); }
                self.owners.push((node.id, copy.id));
                project.nodes.push(copy);
            }
        }
        // Secondary rays shade the stack once on the foundation geometry.
        // Keeping a separate immutable view avoids coincident BVH triangles.
        self.ray_project = Some(Arc::new(project.clone()));
        // Every layer reuses the exact source geometry and skin. Equal-depth alpha
        // passes are appended bottom-to-top; stable transparent sorting preserves
        // that order without displacing the surface or changing authoring topology.
        let mut next_material = source.materials.iter().map(|m| m.id).max().unwrap_or(0);
        for node in &source.nodes {
            if node.material_layers.iter().all(|layer| !layer.visible) { continue; }
            let Some(original) = node.mesh.and_then(|id| source.mesh(id)) else { continue; };
            // Explicit polygon assignments cover the object-wide layer stack.
            // The same split is used for raster passes and secondary rays.
            let mut base_indices = Vec::new(); let mut base_faces = Vec::new(); let mut offset = 0;
            for face in 0..original.face_count() {
                let ring = original.face_vertices(face).unwrap(); let count = (ring.len()-2)*3;
                let painted = original.topology.as_ref().and_then(|t|t.face_materials.get(face)).is_some_and(|m|m.is_some());
                if !painted { base_indices.extend_from_slice(&original.indices[offset..offset+count]); base_faces.push(ring.to_vec()); }
                offset += count;
            }
            if base_indices.is_empty() { continue; }
            for layer in &node.material_layers {
                let Some(material) = source.material(layer.material).filter(|m| layer.visible && m.visible) else { continue; };
                next_mesh = next_mesh.checked_add(1).ok_or("Закончились ID мешей")?;
                next_node = next_node.checked_add(1).ok_or("Закончились ID узлов")?;
                next_material = next_material.checked_add(1).ok_or("Закончились ID материалов")?;
                if material.alpha_mode == poseforge_core::AlphaMode::Mask { self.layer_masks.insert(next_node, material.alpha_cutoff); }
                self.overlays.push((material.id,project.materials.len(),next_node));
                let mut material = material.clone();
                material.id = next_material;
                material.alpha_mode = poseforge_core::AlphaMode::Blend;
                let mut mesh = original.clone();
                mesh.id = next_mesh; mesh.material = Some(material.id);
                mesh.clear_face_materials();
                if base_indices.len()!=original.indices.len() {
                    mesh.indices = base_indices.clone().into();
                    mesh.topology = Some(poseforge_core::MeshTopology {
                        faces:base_faces.clone().into(),face_materials:Default::default(),uv_seams:vec![],
                        smooth_normals:original.smooth_normals_enabled(),primitive:None,
                    });
                }
                let mut copy = node.clone();
                copy.id = next_node; copy.mesh = Some(mesh.id); copy.parent = Some(node.id);
                copy.camera = None; copy.fluid = None; copy.physics = None; copy.destruction = None;
                copy.material_layers.clear();
                self.owners.push((node.id, copy.id));
                if let Some(index) = self.ray_project.as_ref().unwrap().nodes.iter().position(|n| n.id == node.id) {
                    self.ray_object_ids.insert(copy.id, index as f32 + 1.);
                }
                project.materials.push(material); project.meshes.push(mesh); project.nodes.push(copy);
            }
        }
        self.source = Some(source.clone());
        self.expanded = Some(Arc::new(project));
        Ok(())
    }
}
fn sync_scene_settings(destination: &mut Project, source: &Project) {
    let Project { nodes: _, meshes: _, materials: _, outliner, environment_map,
        environment_library, saved_views, collections, version, title, fps, duration,
        textures, video_textures, clips, animation_layers, camera, active_camera,
        lights, render_settings, fluid_settings, poses, rig_profiles } = source;
    macro_rules! copy { ($($field:ident),* $(,)?) => { $(destination.$field.clone_from($field);)* }; }
    copy!(outliner, environment_map, environment_library, saved_views, collections,
        version, title, fps, duration, textures, video_textures, clips, animation_layers,
        camera, active_camera, lights, render_settings, fluid_settings, poses, rig_profiles);
}
fn has_overrides(mesh: &Mesh) -> bool {
    mesh.topology
        .as_ref()
        .is_some_and(|t| !t.face_materials.is_empty())
}
fn batches(mesh: &Mesh) -> Vec<Batch> {
    let mut groups = BTreeMap::<(bool,Option<u64>), (Vec<u32>, Vec<Vec<u32>>)>::new();
    let mut offset = 0;
    for (face, ring) in mesh.topology.as_ref().unwrap().faces.iter().enumerate() {
        let count = (ring.len() - 2) * 3;
        if let Some(indices) = mesh.indices.get(offset..offset + count) {
            let painted = mesh.topology.as_ref().unwrap().face_materials.get(face).is_some_and(|m|m.is_some());
            let group = groups.entry((painted,mesh.face_material(face))).or_default();
            group.0.extend_from_slice(indices);
            group.1.push(ring.clone());
        }
        offset += count;
    }
    groups
        .into_iter()
        .map(|((painted,material), (indices, faces))| {
            // A many-material model must not upload its entire vertex buffer once
            // per material. Compact each batch; total vertices are bounded by the
            // original index count, including seams and shared boundary vertices.
            let mut map = HashMap::new();
            let mut source = Vec::new();
            let faces = faces.into_iter().map(|face| face.into_iter().map(|index| {
                *map.entry(index).or_insert_with(|| {
                    let next = source.len() as u32;
                    source.push(index as usize);
                    next
                })
            }).collect()).collect();
            let indices = indices.into_iter().map(|index| map[&index]).collect();
            let mut copy = mesh.clone();
            copy.material = material;
            copy.topology = Some(poseforge_core::MeshTopology {
                faces, face_materials: Default::default(), uv_seams: vec![], primitive: None,
                smooth_normals: mesh.smooth_normals_enabled(),
            });
            copy.vertices = source.iter().map(|&i| mesh.vertices[i]).collect();
            copy.indices = indices;
            for target in &mut copy.morph_targets {
                target.positions = source.iter().map(|&i| target.positions[i]).collect();
                if !target.normals.is_empty() {
                    target.normals = source.iter().map(|&i| target.normals[i]).collect();
                }
            }
            Batch { mesh:copy,painted }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Editor, Material, Node, PrimitiveKind, PrimitiveSettings, Vec3};
    #[test]
    fn moving_lights_and_changing_exposure_reuse_cut_layer_geometry() {
        let mut node=Node::new(1,"Cube");node.mesh=Some(2);
        let mut e=Editor::new(Project{nodes:vec![node],meshes:vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap()],
            materials:vec![Material{id:3,..Default::default()},Material{id:4,..Default::default()}],..Default::default()});
        e.add_material_layer(1,None).unwrap();
        e.assign_material_region(1,4,Some(&std::collections::BTreeSet::from([0]))).unwrap();
        let globals=e.project.global_transforms().unwrap();let mut cache=MaterialParts::default();
        let (first,first_globals)=cache.scene(&e.project,&globals).unwrap().unwrap();
        let last=first.nodes.last().unwrap().mesh.unwrap();let source=first.mesh(last).unwrap();
        let revision=source.indices.revision();
        e.project.lights[0].position += Vec3::X;e.project.render_settings.exposure=1.;
        let (second,second_globals)=cache.scene(&e.project,&globals).unwrap().unwrap();
        let mesh=second.mesh(last).unwrap();assert_eq!(mesh.indices.revision(),revision);
        assert!(mesh.indices.shares_storage(&source.indices));assert!(Arc::ptr_eq(&first_globals,&second_globals));
        assert_eq!(second.lights,e.project.lights);assert_eq!(cache.ray_scene().unwrap().lights,e.project.lights);
        assert_ne!(first.lights,second.lights,"old snapshots remain immutable");
        assert_eq!(second.render_settings.exposure,1.);assert_eq!(first.render_settings.exposure,0.);
    }
    use std::collections::BTreeSet;
    #[test]
    fn polygon_paint_overrides_layers_in_primary_and_secondary_rays() {
        let mut node=Node::new(1,"Cube");node.mesh=Some(2);
        let mut e=Editor::new(Project{nodes:vec![node],meshes:vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap()],
            materials:vec![Material{id:3,..Default::default()},Material{id:4,..Default::default()}],..Default::default()});
        e.add_material_layer(1,None).unwrap();e.assign_material_region(1,4,Some(&BTreeSet::from([0]))).unwrap();
        let original=e.project.clone();let globals=original.global_transforms().unwrap();let mut cache=MaterialParts::default();
        let (expanded,_)=cache.scene(&original,&globals).unwrap().unwrap();
        let rays=cache.ray_scene().unwrap();let painted=rays.nodes.iter().find(|n|rays.mesh(n.mesh.unwrap()).unwrap().material==Some(4)).unwrap();
        assert!(painted.material_layers.is_empty());
        assert_eq!(rays.mesh(painted.mesh.unwrap()).unwrap().face_count(),1);
        let overlay=expanded.nodes.last().unwrap();assert_eq!(expanded.mesh(overlay.mesh.unwrap()).unwrap().face_count(),5);
        assert_eq!(e.project,original);
        e.project.materials.iter_mut().find(|m|m.id==4).unwrap().visible=false;
        let hidden_source=e.project.clone();
        let (_,matrices)=cache.scene(&hidden_source,&globals).unwrap().unwrap();
        let rays=cache.ray_scene().unwrap();
        let triangles=crate::world::build_geometry(&rays,&matrices,false).unwrap().1;
        assert_eq!(triangles.len(),12, "hiding a face material must not cut a hole");
        assert_eq!(triangles.iter().filter(|t|t.a[3]==0.).count(),2);
        assert_eq!(e.project,hidden_source);
    }
    #[test]
    fn overlays_keep_geometry_order_visibility_and_one_ray_surface() {
        let mut node=Node::new(1,"Cube"); node.mesh=Some(2);
        let mut editor=Editor::new(Project {nodes:vec![node],meshes:vec![poseforge_core::box_mesh(2,"Cube",Vec3::ONE,None)],..Default::default()});
        let first=editor.add_material_layer(1,None).unwrap();
        let second=editor.add_material_layer(1,Some(first)).unwrap();
        editor.project.materials[1].base_color=[0.,1.,0.,0.4];
        let original=editor.project.clone(); let globals=original.global_transforms().unwrap();
        let mut cache=MaterialParts::default(); let (scene,matrices)=cache.scene(&original,&globals).unwrap().unwrap();
        assert_eq!(scene.nodes.len(),3); assert_eq!(cache.ray_scene().unwrap().nodes.len(),1);
        let last=&scene.nodes[2]; let mesh=scene.mesh(last.mesh.unwrap()).unwrap(); let material=scene.material(mesh.material.unwrap()).unwrap();
        assert_eq!(material.base_color,[0.,1.,0.,0.4]); assert_eq!(material.alpha_mode,poseforge_core::AlphaMode::Blend);
        assert_eq!(mesh.vertices.revision(),original.meshes[0].vertices.revision());
        assert_eq!(matrices[&last.id],globals[&1]); assert_eq!(cache.ray_object_id(last.id),Some(1.));
        assert_eq!(editor.project,original);
        editor.project.nodes[0].material_layers.iter_mut().find(|l|l.material==second).unwrap().visible=false;
        let (hidden,_)=cache.scene(&editor.project,&globals).unwrap().unwrap(); assert_eq!(hidden.nodes.len(),2);
        editor.project.nodes[0].material_layers.clear(); assert!(cache.scene(&editor.project,&globals).unwrap().is_none());
    }
    #[test]
    fn batches_reuse_buffers_and_project_during_navigation_and_animation() {
        let mut node = Node::new(1, "Cube");
        node.mesh = Some(2);
        let mut e = Editor::new(Project {
            nodes: vec![node],
            meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
                .mesh(2, Some(3))
                .unwrap()],
            materials: vec![
                Material {
                    id: 3,
                    ..Material::default()
                },
                Material {
                    id: 4,
                    ..Material::default()
                },
            ],
            ..Project::default()
        });
        e.assign_face_material(2, &[1], &BTreeSet::from([0]), 4)
            .unwrap();
        let original = e.project.clone();
        let mut cache = MaterialParts::default();
        let globals = BTreeMap::from([(1, Mat4::IDENTITY)]);
        let (a, ga) = cache.scene(&e.project, &globals).unwrap().unwrap();
        let (b, gb) = cache.scene(&e.project, &globals).unwrap().unwrap();
        assert!(Arc::ptr_eq(&a, &b) && Arc::ptr_eq(&ga, &gb));
        assert_eq!(a.nodes.len(), 2);
        assert_eq!(a.meshes.iter().map(|mesh| mesh.face_count()).sum::<usize>(), original.meshes[0].face_count());
        for mesh in &a.meshes {
            assert!(mesh.topology.is_some());
            mesh.validate_topology().unwrap();
        }
        assert_eq!(
            a.meshes.iter().map(|m| m.indices.len()).sum::<usize>(),
            original.meshes[0].indices.len()
        );
        assert!(
            a.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
                <= original.meshes[0].indices.len()
        );
        let moved = BTreeMap::from([(1, Mat4::from_translation(Vec3::X))]);
        let (c, gc) = cache.scene(&e.project, &moved).unwrap().unwrap();
        assert!(Arc::ptr_eq(&a, &c));
        assert!(!Arc::ptr_eq(&ga, &gc));
        assert_eq!(gc[&a.nodes[1].id], moved[&1]);
        assert_eq!(e.project, original);
        e.project.nodes[0].shadow_only = true;
        let (shadow, _) = cache.scene(&e.project, &globals).unwrap().unwrap();
        assert!(shadow.nodes.iter().filter(|n| n.mesh.is_some()).all(|n| n.shadow_only));
        assert!(cache.ray_scene().unwrap().nodes.iter().filter(|n| n.mesh.is_some()).all(|n| n.shadow_only));
        assert!(!Arc::ptr_eq(&a, &shadow));
        e.project.nodes[0].shadow_only = false;
        e.project.nodes[0].visible = false;
        let (hidden, _) = cache.scene(&e.project, &globals).unwrap().unwrap();
        assert!(hidden.visible_node_ids(false).is_empty());
        e.assign_material(2, &[1], 3).unwrap();
        assert!(cache.scene(&e.project, &globals).unwrap().is_none());
        assert!(cache.source.is_none() && cache.meshes.is_empty());
    }
}

#[cfg(test)]
mod appearance_cache_tests {
    use super::*;
    use poseforge_core::{AlphaMode,Editor,Material,Node,PrimitiveKind,PrimitiveSettings};
    #[test]
    fn material_sliders_preserve_geometry_but_refresh_layers_masks_and_ray_materials() {
        let mut node=Node::new(1,"Cube");node.mesh=Some(2);
        let mut editor=Editor::new(Project{nodes:vec![node],
            meshes:vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap()],
            materials:vec![Material{id:3,..Default::default()},Material{id:4,..Default::default()}],..Default::default()});
        let layer=editor.add_material_layer(1,None).unwrap();
        editor.assign_material_region(1,4,Some(&std::collections::BTreeSet::from([0]))).unwrap();
        let globals=editor.project.global_transforms().unwrap();let mut cache=MaterialParts::default();
        let (first,first_globals)=cache.scene(&editor.project,&globals).unwrap().unwrap();
        let first_rays=cache.ray_scene().unwrap();
        let (_,copy_index,overlay_node)=cache.overlays[0];let copy_id=first.materials[copy_index].id;
        let material=editor.project.materials.iter_mut().find(|m|m.id==layer).unwrap();
        material.emissive=[3.,1.,0.];material.emission_volume=true;material.roughness=0.2;
        material.alpha_mode=AlphaMode::Mask;material.alpha_cutoff=0.7;
        let (second,second_globals)=cache.scene(&editor.project,&globals).unwrap().unwrap();
        assert_eq!(first.meshes.len(),second.meshes.len());
        for (old,new) in first.meshes.iter().zip(&second.meshes) {
            assert_eq!(old.id,new.id);assert_eq!(old.indices.revision(),new.indices.revision());
            assert!(old.vertices.shares_storage(&new.vertices));
        }
        assert!(Arc::ptr_eq(&first_globals,&second_globals));
        assert_eq!(second.materials[copy_index].id,copy_id);
        assert_eq!(second.materials[copy_index].emissive,[3.,1.,0.]);
        assert_eq!(second.materials[copy_index].alpha_mode,AlphaMode::Blend);
        assert_eq!(cache.layer_mask(overlay_node),Some(0.7));
        assert_eq!(cache.ray_scene().unwrap().materials,editor.project.materials);
        assert!(!first_rays.material(layer).unwrap().emission_volume);
        assert_eq!(first.materials[copy_index].emissive,[0.;3]);
        editor.project.materials.iter_mut().find(|m|m.id==layer).unwrap().alpha_mode=AlphaMode::Blend;
        cache.scene(&editor.project,&globals).unwrap();assert_eq!(cache.layer_mask(overlay_node),None);
        editor.project.materials.iter_mut().find(|m|m.id==layer).unwrap().visible=false;
        let (hidden,_)=cache.scene(&editor.project,&globals).unwrap().unwrap();
        assert!(cache.overlays.is_empty());assert!(!hidden.materials.iter().any(|m|m.id==copy_id));
    }
    #[test]
    fn moving_camera_with_layered_meshes_preserves_batches_and_updates_metadata() {
        let mut node=Node::new(1,"Cube");node.mesh=Some(2);
        let camera=Node::new(10,"Camera");
        let mut editor=Editor::new(Project{nodes:vec![node,camera],
            meshes:vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,Some(3)).unwrap()],
            materials:vec![Material{id:3,..Default::default()},Material{id:4,..Default::default()}],..Default::default()});
        editor.add_material_layer(1,None).unwrap();
        editor.assign_material_region(1,4,Some(&std::collections::BTreeSet::from([0]))).unwrap();
        let globals=editor.project.global_transforms().unwrap();let mut cache=MaterialParts::default();
        let (first,_)=cache.scene(&editor.project,&globals).unwrap().unwrap();
        editor.project.node_mut(10).unwrap().transform.translation=poseforge_core::Vec3::X;
        let moved=editor.project.global_transforms().unwrap();
        let (second,transforms)=cache.scene(&editor.project,&moved).unwrap().unwrap();
        for (a,b) in first.meshes.iter().zip(&second.meshes) {assert_eq!(a.indices.revision(),b.indices.revision());}
        assert_eq!(second.node(10).unwrap(),editor.project.node(10).unwrap());
        assert_eq!(cache.ray_scene().unwrap().node(10).unwrap(),editor.project.node(10).unwrap());
        assert_eq!(transforms[&10],moved[&10]);assert_eq!(first.node(10).unwrap().transform.translation,poseforge_core::Vec3::ZERO);
    }

}
