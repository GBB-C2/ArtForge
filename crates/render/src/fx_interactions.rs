//! FX source sampling, force response and mesh contacts share one bounded preparation worker.
use super::fx_geometry::{self,Geometry,PreparedGeometry,matrix_hash,check};
use poseforge_core::{CollisionKind,CurveData,FxKind,FxSettings,Mat4,Project,Vec3};
use poseforge_simulation::{fields::{Field,Fields},fx_flow::{self,FlowConfig,FLOW_KNOTS,MAX_FLOW_SOURCES},mesh_obstacles::MeshObstacles};
use std::{collections::{BTreeMap,BTreeSet},hash::{Hash,Hasher},sync::{Arc,atomic::{AtomicBool,Ordering},mpsc}};

#[derive(Clone,Copy)]
pub(super) struct Guides {
    pub low:[f32;4],pub high:[f32;4],pub meta:[f32;4],
    pub points:[[[f32;4];FLOW_KNOTS];MAX_FLOW_SOURCES],
}
impl Default for Guides {fn default()->Self {Self{low:[0.;4],high:[0.;4],meta:[0.;4],points:[[[0.;4];FLOW_KNOTS];MAX_FLOW_SOURCES]}}}
#[derive(Clone)]
enum Source {Mesh(u64),Curve(CurveData,Mat4)}
struct Effect {id:u64,settings:FxSettings,model:Mat4,source_origin:Vec3,source:Option<Source>,obstacles:Vec<u64>,fields:Vec<Field>}
struct Input {key:u64,effects:Vec<Effect>,geometry:BTreeMap<u64,Geometry>}
#[derive(Clone,Default)]
struct Cache {geometry:BTreeMap<u64,Arc<PreparedGeometry>>,obstacles:BTreeMap<Vec<u64>,Arc<MeshObstacles>>}
struct Prepared {volumes:BTreeMap<u64,Guides>,masks:Vec<u8>,gas:Arc<Vec<super::fx_gas::Domain>>,cache:Cache}
struct Job {key:u64,cancel:Arc<AtomicBool>,receiver:mpsc::Receiver<Result<Prepared,String>>}
impl Drop for Job {fn drop(&mut self) {self.cancel.store(true,Ordering::Relaxed);}}

#[derive(Default)]
pub(super) struct State {
    job:Option<Job>,cache:Cache,ready:Option<(u64,BTreeMap<u64,Guides>,Vec<u8>,Arc<Vec<super::fx_gas::Domain>>)>,failed:Option<u64>,
    required:BTreeSet<u64>,current:u64,message:Option<String>,
}
impl State {
    pub fn pending(&self)->bool {self.job.is_some()}
    pub fn blocked(&self)->bool {!self.required.is_empty() && self.ready.as_ref().is_none_or(|(key,_,_,_)|*key!=self.current)}
    pub fn status(&self)->Option<&str> {self.message.as_deref()}
    pub fn requires(&self,id:u64)->bool {self.required.contains(&id)}
    pub fn guides(&self,id:u64)->Option<&Guides> {self.ready.as_ref().filter(|(key,_,_,_)|*key==self.current)?.1.get(&id)}
    pub fn masks(&self)->Option<(u64,&[u8])> {
        let (key,_,data,_)=self.ready.as_ref().filter(|(key,_,_,_)|*key==self.current)?;
        Some((*key,data.as_slice()))
    }
    pub fn gas(&self)->Option<(u64,Arc<Vec<super::fx_gas::Domain>>)> {
        let (key,_,_,gas)=self.ready.as_ref().filter(|(key,_,_,_)|*key==self.current)?;
        Some((*key,gas.clone()))
    }
    pub fn available(&self,id:u64)->bool {!self.requires(id) || self.guides(id).is_some_and(|g|g.meta[0]>0. || g.meta[3]>0.)}
    #[cfg(test)]
    pub fn update(&mut self,project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool)->Result<(),String> {
        self.update_with_cancel(project,globals,export,&AtomicBool::new(false))
    }
    pub fn update_with_cancel(&mut self,project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,cancel:&AtomicBool)->Result<(),String> {
        let input=match collect(project,globals,export) {
            Ok(input)=>input,Err(error)=>{
                if let Some(job)=&self.job {
                    job.cancel.store(true,Ordering::Relaxed);
                    if !matches!(job.receiver.try_recv(),Err(mpsc::TryRecvError::Empty)) {self.job=None;}
                }
                self.ready=None;self.required=project.nodes.iter().filter(|n|n.fx.is_some()).map(|n|n.id).collect();self.message=Some(error.clone());
                if export {return Err(error);}return Ok(());
            }
        };self.current=input.key;
        self.required=input.effects.iter().map(|e|e.id).collect();
        if let Some(job)=&self.job {
            if job.key!=input.key || export {job.cancel.store(true,Ordering::Relaxed);}
            let result=match job.receiver.try_recv() {
                Ok(result)=>Some(result),Err(mpsc::TryRecvError::Empty)=>None,
                Err(mpsc::TryRecvError::Disconnected)=>Some(Err("Подготовка FX остановлена".into())),
            };
            if let Some(result)=result {
                let current=job.key==input.key && !job.cancel.load(Ordering::Relaxed);
                self.job=None;
                match result {
                    Ok(prepared)=>{
                        self.cache=prepared.cache;
                        if current {self.ready=Some((input.key,prepared.volumes,prepared.masks,prepared.gas));self.failed=None;self.message=None;}
                    },
                    Err(error) if current=>{self.failed=Some(input.key);self.message=Some(error);self.ready=None;},
                    Err(_)=>{}
                }
            }
        }
        if input.effects.is_empty() {
            if let Some(job)=&self.job {job.cancel.store(true,Ordering::Relaxed);}
            self.ready=None;self.failed=None;self.message=None;self.cache=Cache::default();return Ok(());
        }
        if self.ready.as_ref().is_some_and(|(key,_,_,_)|*key==input.key) {return Ok(());}
        if self.failed==Some(input.key) && !export {return Ok(());}
        if export {
            let prepared=build(&input,self.cache.clone(),cancel)?;
            self.cache=prepared.cache;self.ready=Some((input.key,prepared.volumes,prepared.masks,prepared.gas));self.failed=None;self.message=None;return Ok(());
        }
        if self.job.is_some() {self.message=Some("Подготовка потока FX…".into());return Ok(());}
        let key=input.key;let cache=self.cache.clone();let cancel=Arc::new(AtomicBool::new(false));let flag=cancel.clone();
        let (sender,receiver)=mpsc::channel();
        match std::thread::Builder::new().name("fx-flow".into()).spawn(move || {let _=sender.send(build(&input,cache,&flag));}) {
            Ok(_)=>{self.job=Some(Job{key,cancel,receiver});self.message=Some("Подготовка потока FX…".into());}
            Err(error)=>{self.failed=Some(key);self.message=Some(format!("Не удалось подготовить FX: {error}"));}
        }
        Ok(())
    }
}

fn collect(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool)->Result<Input,String> {
    let visible=project.visible_node_ids(export);let mut fields=Vec::new();
    for n in &project.nodes {
        let Some(f)=n.force_field.filter(|f|f.enabled && f.fx && visible.contains(&n.id)) else{continue;};
        let Some(model)=globals.get(&n.id).filter(|m|m.is_finite()) else{continue;};
        let Some(axis)=model.transform_vector3(Vec3::Z).try_normalize() else{continue;};
        let scale=model.to_scale_rotation_translation().0.abs().max_element();
        fields.push(Field{kind:f.kind as u8,center:model.transform_point3(Vec3::ZERO),axis,strength:f.strength,
            radius:f.radius*scale,falloff:f.falloff,damping:f.damping});
    }
    let mut geometry=BTreeMap::new();let mut effects=Vec::new();
    let mut hasher=std::collections::hash_map::DefaultHasher::new();
    for n in &project.nodes {
        let Some(settings)=n.fx.filter(|f|f.enabled && f.density>0. && visible.contains(&n.id)) else{continue;};
        let Some(&model)=globals.get(&n.id).filter(|m|m.is_finite() && m.determinant().abs()>1e-8 && m.inverse().is_finite()) else{continue;};
        let selected:Vec<_>=if settings.mesh_collision {project.resolved_collision_targets(n.id,CollisionKind::Fx).into_iter()
            .filter(|t|t.enabled && visible.contains(&t.node)).collect()}else{vec![]};
        let effect_fields=if settings.use_fields && settings.field_influence>0. {fields.clone()}else{vec![]};
        if !settings.gas && settings.source.is_none() && selected.is_empty() && effect_fields.is_empty() {continue;}
        let mut source_origin=Vec3::ZERO;
        let source=if let Some(id)=settings.source {
            let node=project.node(id).ok_or("Источник FX удалён")?;let source_model=*globals.get(&id).ok_or("Источник FX без трансформации")?;
            source_origin=source_model.transform_point3(Vec3::ZERO);
            if let Some(curve)=&node.curve {Some(Source::Curve(curve.clone(),source_model))}
            else {
                let mesh=node.mesh.and_then(|id|project.mesh(id)).ok_or("Источник FX без геометрии")?;
                let snapshot=Geometry::snapshot(node,mesh,globals)?;let key=snapshot.key;geometry.entry(key).or_insert(snapshot);Some(Source::Mesh(key))
            }
        }else{None};
        let mut obstacles=Vec::new();
        for target in selected {
            if Some(target.node)==settings.source {continue;}
            let node=project.node(target.node).ok_or("Препятствие FX удалено")?;
            let Some(mesh)=node.mesh.and_then(|id|project.mesh(id)) else{continue;};
            let snapshot=Geometry::snapshot(node,mesh,globals)?;let key=snapshot.key;geometry.entry(key).or_insert(snapshot);obstacles.push(key);
        }
        obstacles.sort_unstable();obstacles.dedup();
        settings.gas.hash(&mut hasher);
        if settings.gas {
            settings.gas_hollow.hash(&mut hasher);(settings.quality as u8).hash(&mut hasher);settings.seed.hash(&mut hasher);
            for v in [settings.gas_domain,settings.dissipation,settings.buoyancy,settings.turbulence,settings.duration] {v.to_bits().hash(&mut hasher);}
            settings.envelope.enabled.hash(&mut hasher);settings.envelope.smooth.hash(&mut hasher);
            for k in settings.envelope.keys {k.at.to_bits().hash(&mut hasher);k.size.to_bits().hash(&mut hasher);}
            settings.pyro.is_some().hash(&mut hasher);
            if let Some(p)=settings.pyro {p.mode.index().hash(&mut hasher);p.cinematic_blast.hash(&mut hasher);
                for v in [p.source_rate,p.source_temperature,p.fuel,p.ignition,p.burn_rate,p.heat_release,p.smoke_yield,p.cooling,p.expansion,p.impulse,p.burst_seconds,p.vorticity,p.fog_falloff,settings.detail] {v.to_bits().hash(&mut hasher);}
            }
        }
        n.id.hash(&mut hasher);settings.cloud.hash(&mut hasher);settings.burst.hash(&mut hasher);settings.kind.index().hash(&mut hasher);settings.source.hash(&mut hasher);matrix_hash(model,&mut hasher);
        for v in [settings.height,settings.width,settings.source_radius,settings.field_influence,settings.drift[0],settings.drift[1]] {v.to_bits().hash(&mut hasher);}
        match &source {
            Some(Source::Mesh(key))=>key.hash(&mut hasher),Some(Source::Curve(curve,matrix))=>{
                matrix_hash(*matrix,&mut hasher);curve.closed.hash(&mut hasher);(curve.interpolation as u8).hash(&mut hasher);curve.resolution.hash(&mut hasher);
                for point in &curve.points {point.map(f32::to_bits).hash(&mut hasher);}
            },None=>{}
        }
        obstacles.hash(&mut hasher);effect_fields.len().hash(&mut hasher);
        for f in &effect_fields {
            f.kind.hash(&mut hasher);
            for v in [f.center.x,f.center.y,f.center.z,f.axis.x,f.axis.y,f.axis.z,f.strength,f.radius,f.falloff,f.damping] {v.to_bits().hash(&mut hasher);}
        }
        effects.push(Effect{id:n.id,settings,model,source_origin,source,obstacles,fields:effect_fields});
    }
    if geometry.values().map(|g|g.indices.len()/3).sum::<usize>()>fx_geometry::MAX_TRIANGLES {
        return Err("FX: источники и препятствия превышают 200000 треугольников суммарно".into());
    }
    Ok(Input{key:hasher.finish(),effects,geometry})
}

fn build(input:&Input,mut cache:Cache,cancel:&AtomicBool)->Result<Prepared,String> {
    check(cancel)?;cache.geometry.retain(|key,_|input.geometry.contains_key(key));
    for (&key,geometry) in &input.geometry {
        check(cancel)?;if !cache.geometry.contains_key(&key) {cache.geometry.insert(key,Arc::new(geometry.build(cancel)?));}
    }
    let groups:BTreeSet<_>=input.effects.iter().filter(|e|!e.obstacles.is_empty()).map(|e|e.obstacles.clone()).collect();
    let triangle_count:usize=groups.iter().flat_map(|g|g.iter()).map(|key|cache.geometry[key].triangles.len()).sum();
    if triangle_count>fx_geometry::MAX_TRIANGLES {return Err("FX: наборы столкновений превышают 200000 треугольников; используйте общие препятствия".into());}
    cache.obstacles.retain(|group,_|groups.contains(group));
    for group in groups {
        check(cancel)?;
        if !cache.obstacles.contains_key(&group) {
            let meshes=group.iter().map(|key|cache.geometry[key].triangles.clone()).collect();
            cache.obstacles.insert(group,Arc::new(MeshObstacles::new(meshes,cancel)?));
        }
    }
    let mut volumes=BTreeMap::new();let mut gas=Vec::new();let mut contact_budget=500_000;let mut mask_budget=8_000_000;
    let mut masks=vec![255;poseforge_simulation::fx_collision::MASK_CELLS*poseforge_core::MAX_FX];
    for (slot,effect) in input.effects.iter().enumerate() {
        check(cancel)?;let f=effect.settings;let model=effect.model;let inverse=model.inverse();
        let offset=model.transform_point3(Vec3::ZERO)-effect.source_origin;
        let mut diameter=if effect.source.is_some() {f.source_radius*2.}else{1.};
        let sources:Vec<Vec3>=match &effect.source {
            Some(Source::Mesh(key))=>cache.geometry[key].samples.iter().map(|p|*p+offset).collect(),
            Some(Source::Curve(curve,source_model))=>{
                let (points,spacing)=fx_geometry::sample_curve(curve,*source_model)?;
                let inverse_scale=inverse.transform_vector3(Vec3::X).length().max(inverse.transform_vector3(Vec3::Y).length()).max(inverse.transform_vector3(Vec3::Z).length());
                diameter=diameter.max(spacing*inverse_scale*1.65/f.width.max(0.1));points.into_iter().map(|p|p+offset).collect()
            },
            None if !f.gas && (f.kind==FxKind::Fog || f.cloud || f.burst || !effect.obstacles.is_empty())=>{
                diameter=0.7;[-0.16,0.,0.16].into_iter().flat_map(|x|[-0.16,0.,0.16].into_iter().map(move |z|model.transform_point3(Vec3::new(x*f.width,0.,z*f.width)))).collect()
            },
            None=>vec![model.transform_point3(Vec3::ZERO)],
        };
        let fields=Fields::new(effect.fields.clone(),Mat4::IDENTITY)?;
        let obstacles=cache.obstacles.get(&effect.obstacles).map(Arc::as_ref);
        if f.gas {
            let meshes:Vec<_>=effect.obstacles.iter().map(|key|cache.geometry[key].triangles.as_slice()).collect();
            let radius=if effect.source.is_some() {diameter*f.width*0.5}else if f.pyro.is_some() {f.source_radius*f.width}else{f.width*if f.cloud || f.burst {0.4}else{0.18}};
            let (guides,domain,mask)=super::fx_gas::prepare(effect.id,slot,f,model,sources,radius,obstacles,&meshes,effect.fields.clone(),cancel,&mut mask_budget)?;
            let cells=poseforge_simulation::fx_collision::MASK_CELLS;masks[slot*cells..(slot+1)*cells].copy_from_slice(&mask);
            volumes.insert(effect.id,guides);gas.push(domain);continue;
        }
        let lifetime=match f.kind {FxKind::Fire=>0.7,FxKind::Smoke=>1.5,FxKind::Fog=>1.4,FxKind::Steam=>1.1};
        // A curved plume can point along any local axis, including nonuniform scale.
        let parcel_scale=model.x_axis.truncate().length().min(model.y_axis.truncate().length()).min(model.z_axis.truncate().length());
        let config=FlowConfig{velocity:model.transform_vector3(Vec3::Y*f.height/lifetime),
            drift:model.transform_vector3(Vec3::new(f.drift[0],0.,f.drift[1])),lifetime,
            radius:(diameter*f.width*0.035*parcel_scale).max(0.002),influence:f.field_influence,escape:Vec3::ZERO};
        let mut guides=Guides::default();let mut count=0;let mut low=Vec3::splat(f32::INFINITY);let mut high=Vec3::splat(f32::NEG_INFINITY);
        for (site,source) in sources.into_iter().take(MAX_FLOW_SOURCES).enumerate() {
            let radial=source-model.transform_point3(Vec3::ZERO);
            let axis=config.velocity.normalize_or_zero();let tangent=radial-axis*radial.dot(axis);
            let angle=site as f32*2.399963;
            let escape=if tangent.length_squared()>1e-8 {tangent}else{model.transform_vector3(Vec3::new(angle.cos(),0.,angle.sin()))};
            let Some(points)=fx_flow::trace(source,FlowConfig{escape,..config},&fields,obstacles,cancel,&mut contact_budget)? else{continue;};
            for (j,point) in points.into_iter().enumerate() {
                let local=inverse.transform_point3(point);low=low.min(local);high=high.max(local);
                guides.points[count][j]=local.extend(j as f32/(FLOW_KNOTS-1) as f32).to_array();
            }
            count+=1;
        }
        if count>0 {
            let padding=Vec3::splat(diameter*f.width*0.65);
            guides.low=(low-padding).extend(0.).to_array();guides.high=(high+padding).extend(0.).to_array();guides.meta=[count as f32,diameter,0.,0.];
            if let Some(obstacles)=obstacles {
                let meshes:Vec<_>=effect.obstacles.iter().map(|key|cache.geometry[key].triangles.as_slice()).collect();
                let mask=poseforge_simulation::fx_collision::build(&meshes,obstacles,model,low-padding,high+padding,cancel,&mut mask_budget)?;
                let cells=poseforge_simulation::fx_collision::MASK_CELLS;
                if slot>=poseforge_core::MAX_FX {return Err("Слишком много FX для объёма столкновений".into());}
                masks[slot*cells..(slot+1)*cells].copy_from_slice(&mask);guides.meta[2]=(slot+1) as f32;
            }
        }
        volumes.insert(effect.id,guides);
    }
    check(cancel)?;Ok(Prepared{volumes,masks,gas:Arc::new(gas),cache})
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Editor,Node,ForceFieldSettings,ForceFieldKind,box_mesh,CollisionTarget};
    fn scene()->(Editor,u64) {let mut e=Editor::new(Project::default());let id=e.create_fx(FxKind::Smoke).unwrap();(e,id)}
    #[test]
    fn combustion_invalidates_transport_but_optical_controls_reuse_the_cache() {
        let mut e=Editor::new(Project::default());let id=e.create_pyro(FxKind::Fire).unwrap();let globals=e.project.global_transforms().unwrap();
        let key=collect(&e.project,&globals,false).unwrap().key;
        let p=e.project.node_mut(id).unwrap().fx.as_mut().unwrap().pyro.as_mut().unwrap();p.smoke_color=[0.1,0.2,0.3];p.anisotropy=-0.4;p.temperature_color=false;p.self_shadow=false;
        assert_eq!(collect(&e.project,&globals,false).unwrap().key,key);
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().pyro.as_mut().unwrap().burn_rate=7.;assert_ne!(collect(&e.project,&globals,false).unwrap().key,key);
        let input=collect(&e.project,&globals,false).unwrap();let result=build(&input,Cache::default(),&AtomicBool::new(false)).unwrap();
        let p=result.gas[0].config.pyro.unwrap();assert_eq!(p.burn_rate,7.);assert!((p.temperature-(1100.-293.)/2200.).abs()<1e-6);
        assert_eq!(result.gas[0].config.radius,0.25);
    }
    #[test]
    fn gas_physics_invalidates_only_for_changes_that_affect_transport() {
        let (mut e,id)=scene();let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.gas=true;f.envelope.enabled=true;
        let globals=e.project.global_transforms().unwrap();let before=collect(&e.project,&globals,false).unwrap();
        let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.color=[0.9,0.4,0.2];f.density=20.;f.emission=2.;f.speed=2.;f.start=3.;f.looped=true;
        f.envelope.keys[1].color=[0.1,0.3,1.];f.envelope.keys[2].density=0.4;f.envelope.keys[2].emission=0.2;
        assert_eq!(collect(&e.project,&globals,false).unwrap().key,before.key);
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().envelope.keys[1].size=0.5;
        let size=collect(&e.project,&globals,false).unwrap();assert_ne!(size.key,before.key);
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().quality=poseforge_core::FxQuality::Fine;
        assert_ne!(collect(&e.project,&globals,false).unwrap().key,size.key);
    }
    #[test]
    fn cinematic_shape_invalidates_cached_source_and_transport() {
        let mut e=Editor::new(Project::default());
        let id=e.create_pyro(FxKind::Fire).unwrap();
        e.project.node_mut(id).unwrap().fx=Some(FxSettings::modern(FxKind::Fire).styled(3).unwrap());
        let globals=e.project.global_transforms().unwrap();
        let mut state=State::default();
        state.update(&e.project,&globals,true).unwrap();
        let (cinematic_key,domains)=state.gas().unwrap();
        assert!(domains[0].config.pyro.unwrap().cinematic_blast);
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().pyro.as_mut().unwrap().cinematic_blast=false;
        state.update(&e.project,&globals,true).unwrap();
        let (regular_key,domains)=state.gas().unwrap();
        assert_ne!(regular_key,cinematic_key);
        assert!(!domains[0].config.pyro.unwrap().cinematic_blast);
    }
    #[test]
    fn prepared_gas_guides_and_grid_share_bounds_and_atlas_slot() {
        let (mut e,id)=scene();let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.gas=true;f.quality=poseforge_core::FxQuality::Fast;f.gas_domain=3.;
        let global=e.project.global_transforms().unwrap();let input=collect(&e.project,&global,false).unwrap();
        let result=build(&input,Cache::default(),&AtomicBool::new(false)).unwrap();let d=&result.gas[0];let g=result.volumes[&id];
        assert_eq!(d.config.n,16);assert_eq!(g.low,d.config.low.extend(0.).to_array());assert_eq!(g.high,d.config.high.extend(0.).to_array());
        assert_eq!(g.meta[0],0.);assert_eq!(g.meta[3],(d.slot+1) as f32);assert!(d.config.solid.iter().all(|v|!*v));
        assert_eq!(result.masks.len(),poseforge_simulation::fx_collision::MASK_CELLS*poseforge_core::MAX_FX);
    }
    #[test]
    fn source_and_collision_snapshots_follow_geometry_and_preserve_the_document() {
        let (mut e,id)=scene();let mut source=Node::new(20,"Source");source.mesh=Some(21);source.transform.translation=Vec3::new(3.,0.,0.);
        e.project.meshes.push(box_mesh(21,"Cube",Vec3::ONE,None));e.project.nodes.push(source);e.set_fx_source(id,Some(20)).unwrap();
        let before=e.project.clone();let global=e.project.global_transforms().unwrap();let input=collect(&e.project,&global,false).unwrap();
        let prepared=build(&input,Cache::default(),&AtomicBool::new(false)).unwrap();assert_eq!(prepared.volumes[&id].meta[0],16.);assert_eq!(e.project,before);
        e.project.node_mut(20).unwrap().transform.translation.x=4.;let moved=collect(&e.project,&e.project.global_transforms().unwrap(),false).unwrap();assert_ne!(input.key,moved.key);
        e.project.node_mut(id).unwrap().collision_targets.fx=Some(vec![CollisionTarget{node:20,enabled:true}]);
        assert!(e.project.validate().is_err());
    }
    #[test]
    fn fields_are_opt_in_per_effect_and_can_be_disabled_at_the_field() {
        let (mut e,id)=scene();let mut field=Node::new(20,"Wind");field.empty=true;
        field.force_field=Some(ForceFieldSettings{kind:ForceFieldKind::Wind,radius:10.,..Default::default()});e.project.nodes.push(field);
        let global=e.project.global_transforms().unwrap();let input=collect(&e.project,&global,false).unwrap();assert_eq!(input.effects.len(),1);
        let ready=build(&input,Cache::default(),&AtomicBool::new(false)).unwrap();assert!(ready.volumes[&id].points[0][5][2]>0.);
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().use_fields=false;assert!(collect(&e.project,&global,false).unwrap().effects.is_empty());
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().use_fields=true;e.project.node_mut(20).unwrap().force_field.as_mut().unwrap().fx=false;
        assert!(collect(&e.project,&global,false).unwrap().effects.is_empty());
    }
    #[test]
    fn cancelled_preparation_never_returns_partial_guides() {
        let (mut e,id)=scene();let mut source=Node::new(20,"Source");source.mesh=Some(21);
        e.project.meshes.push(box_mesh(21,"Cube",Vec3::ONE,None));e.project.nodes.push(source);e.set_fx_source(id,Some(20)).unwrap();
        let input=collect(&e.project,&e.project.global_transforms().unwrap(),false).unwrap();assert!(build(&input,Cache::default(),&AtomicBool::new(true)).is_err());
    }
    #[test]
    fn color_and_time_do_not_rebuild_geometry_but_morphs_do() {
        let (mut e,id)=scene();let mut source=Node::new(20,"Source");source.mesh=Some(21);
        let mut mesh=box_mesh(21,"Cube",Vec3::ONE,None);mesh.morph_targets.push(poseforge_core::MorphTarget{name:"Rise".into(),positions:vec![[0.,0.2,0.];mesh.vertices.len()].into(),normals:Default::default()});
        source.morph_weights=vec![0.];e.project.meshes.push(mesh);e.project.nodes.push(source);e.set_fx_source(id,Some(20)).unwrap();
        let global=e.project.global_transforms().unwrap();let input=collect(&e.project,&global,false).unwrap();
        let first=build(&input,Cache::default(),&AtomicBool::new(false)).unwrap();let geometry=first.cache.geometry.values().next().unwrap().clone();
        let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.color=[0.2,0.4,0.6];f.speed=2.;f.density=10.;
        let unchanged=collect(&e.project,&global,false).unwrap();assert_eq!(unchanged.key,input.key);
        let reused=build(&unchanged,first.cache,&AtomicBool::new(false)).unwrap();assert!(Arc::ptr_eq(&geometry,reused.cache.geometry.values().next().unwrap()));
        e.project.node_mut(20).unwrap().morph_weights[0]=1.;let morphed=collect(&e.project,&global,false).unwrap();assert_ne!(morphed.key,input.key);
        let deformed=build(&morphed,reused.cache,&AtomicBool::new(false)).unwrap();
        let moved=deformed.cache.geometry.values().next().unwrap();
        for (a,b) in geometry.samples.iter().zip(&moved.samples) {assert!((*b-*a).abs_diff_eq(Vec3::Y*0.2,1e-5));}
    }
    #[test]
    fn disconnected_and_stale_workers_do_not_publish_partial_or_wrong_scene_data() {
        let (mut e,id)=scene();let mut field=Node::new(20,"Wind");field.empty=true;field.force_field=Some(ForceFieldSettings{kind:ForceFieldKind::Wind,..Default::default()});e.project.nodes.push(field);
        let global=e.project.global_transforms().unwrap();let input=collect(&e.project,&global,false).unwrap();
        let (sender,receiver)=mpsc::channel();drop(sender);
        let mut state=State{job:Some(Job{key:input.key,cancel:Arc::new(AtomicBool::new(false)),receiver}),..Default::default()};
        state.update(&e.project,&global,false).unwrap();assert!(!state.pending());assert!(state.blocked() && !state.available(id));assert!(state.status().is_some());
        let prepared=build(&input,Cache::default(),&AtomicBool::new(false)).unwrap();
        let (sender,receiver)=mpsc::channel();sender.send(Ok(prepared)).ok().unwrap();
        state.job=Some(Job{key:input.key,cancel:Arc::new(AtomicBool::new(false)),receiver});
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().use_fields=false;
        state.update(&e.project,&global,false).unwrap();assert!(!state.pending());assert!(state.guides(id).is_none());assert!(state.available(id));
    }
    #[test]
    fn invalid_input_retires_cancelled_worker_and_export_propagates_failure() {
        let (mut e,id)=scene();let mut source=Node::new(20,"Source");source.mesh=Some(21);
        e.project.meshes.push(box_mesh(21,"Cube",Vec3::ONE,None));e.project.nodes.push(source);e.set_fx_source(id,Some(20)).unwrap();
        let (sender,receiver)=mpsc::channel();drop(sender);let flag=Arc::new(AtomicBool::new(false));
        let mut state=State{job:Some(Job{key:0,cancel:flag.clone(),receiver}),..Default::default()};
        let mut global=e.project.global_transforms().unwrap();global.insert(20,Mat4::ZERO);
        state.update(&e.project,&global,false).unwrap();assert!(flag.load(Ordering::Relaxed));assert!(!state.pending());assert!(state.blocked());
        assert!(state.update(&e.project,&global,true).is_err());
        state.update(&e.project,&e.project.global_transforms().unwrap(),true).unwrap();assert!(state.available(id));assert!(state.status().is_none());
    }
    #[test]
    fn selected_obstacles_publish_matching_masks_and_eye_toggle_removes_them() {
        let (mut e,id)=scene();let mut obstacle=Node::new(20,"Plate");obstacle.mesh=Some(21);obstacle.transform.translation=Vec3::new(0.,0.8,0.);
        e.project.nodes.push(obstacle);e.project.meshes.push(box_mesh(21,"Plate",Vec3::new(0.5,0.1,0.5),None));
        let n=e.project.node_mut(id).unwrap();n.fx.as_mut().unwrap().mesh_collision=true;
        n.collision_targets.fx=Some(vec![CollisionTarget{node:20,enabled:true}]);
        let global=e.project.global_transforms().unwrap();let before=e.project.clone();let mut state=State::default();
        state.update(&e.project,&global,true).unwrap();assert!(state.available(id));
        assert_eq!(state.guides(id).unwrap().meta[2],1.);
        let (key,data)=state.masks().unwrap();assert_eq!(data.len(),poseforge_simulation::fx_collision::MASK_CELLS*poseforge_core::MAX_FX);
        assert!(data.iter().any(|&v|v==0));assert!(data.iter().any(|&v|v==255));assert_eq!(e.project,before);
        let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.color=[0.2,0.3,0.4];f.start=2.;f.duration=5.;f.looped=true;
        state.update(&e.project,&global,true).unwrap();assert_eq!(state.masks().unwrap().0,key,"clock and color reuse occupancy");
        e.project.node_mut(id).unwrap().collision_targets.fx.as_mut().unwrap()[0].enabled=false;
        state.update(&e.project,&global,true).unwrap();assert!(state.masks().is_none());assert!(state.guides(id).is_none());assert!(state.available(id));
    }
    #[test]
    fn mask_is_unavailable_when_an_old_preparation_no_longer_matches_the_scene() {
        let (mut e,id)=scene();let mut source=Node::new(20,"Source");source.mesh=Some(21);
        e.project.meshes.push(box_mesh(21,"Cube",Vec3::ONE,None));e.project.nodes.push(source);e.set_fx_source(id,Some(20)).unwrap();
        let global=e.project.global_transforms().unwrap();let mut state=State::default();state.update(&e.project,&global,true).unwrap();
        assert!(state.masks().is_some());state.current=state.current.wrapping_add(1);
        assert!(state.masks().is_none() && state.guides(id).is_none() && state.blocked());
    }

}
