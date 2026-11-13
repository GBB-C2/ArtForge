//! One cancellable worker, bounded frame/checkpoint caches and exact export sampling.
use poseforge_core::{FxSettings,Project};
use poseforge_simulation::{gas,fx_collision::MASK_EDGE};
use std::{collections::BTreeMap,sync::{Arc,atomic::{AtomicBool,Ordering},mpsc},hash::{Hash,Hasher}};
const FRAME_LIMIT:usize=64;
const CHECKPOINT_LIMIT:usize=6;
const CACHE_BYTES:usize=96*1024*1024;
const BATCH_STEPS:usize=8;
#[derive(Clone)]
pub(super) struct Domain {pub id:u64,pub slot:usize,pub config:Arc<gas::Config>,pub settings:FxSettings}
#[derive(Clone)]
pub(super) struct Image {pub slot:usize,pub tick:u32,pub pixels:Arc<Vec<u8>>,light:f32,temperature:f32,center:poseforge_core::Vec3}
struct CachedImage {pixels:Arc<Vec<u8>>,stamp:u64,light:f32,temperature:f32,center:poseforge_core::Vec3}
struct Checkpoint {frame:gas::Frame,stamp:u64}
struct Timeline {solver:gas::Solver,images:BTreeMap<u32,CachedImage>,checkpoints:BTreeMap<u32,Checkpoint>,stamp:u64}
impl Timeline {
    fn new(domain:&Domain,cancel:&AtomicBool)->Result<Self,String> {
        let solver=gas::Solver::new(domain.config.clone(),cancel)?;let frame=solver.frame.clone();
        Ok(Self{solver,images:BTreeMap::new(),checkpoints:BTreeMap::from([(0,Checkpoint{frame,stamp:0})]),stamp:0})
    }
    fn image(&mut self,domain:&Domain,target:u32,budget:usize,cancel:&AtomicBool)->Result<Image,String> {
        self.stamp=self.stamp.wrapping_add(1);
        if let Some(image)=self.images.get_mut(&target) {image.stamp=self.stamp;return Ok(Image{slot:domain.slot,tick:target,pixels:image.pixels.clone(),light:image.light,temperature:image.temperature,center:image.center});}
        let from=self.checkpoints.range(..=target).next_back().map(|(&tick,_)|tick).unwrap_or(0);
        if self.solver.frame.tick>target || self.solver.frame.tick<from {
            let checkpoint=self.checkpoints.get_mut(&from).unwrap();checkpoint.stamp=self.stamp;self.solver.restore(checkpoint.frame.clone())?;
        }
        let end=target.min(self.solver.frame.tick+budget as u32);
        while self.solver.frame.tick<end {
            if cancel.load(Ordering::Relaxed) {return Err("Расчёт газа отменён".into());}
            let phase=self.solver.frame.tick as f32/(gas::HZ as f32*domain.settings.duration);
            self.solver.step(domain.settings.envelope.sample(phase).size,cancel)?;
            if self.solver.frame.tick%gas::HZ==0 {
                self.checkpoints.insert(self.solver.frame.tick,Checkpoint{frame:self.solver.frame.clone(),stamp:self.stamp});
                self.prune();
            }
        }
        let tick=self.solver.frame.tick;let pixels=Arc::new(self.solver.texture(MASK_EDGE,cancel)?);
        let modern=domain.config.pyro.is_some();
        let light=pixels.chunks_exact(4).map(|p|if modern {p[2] as f32/255.*(p[1] as f32/255.*2.).min(1.)}else{p[0] as f32/255.*(p[1] as f32/255.).powi(2)}).fold(0.,f32::max);
        let mut mass=0.;let mut temperature=0.;let mut center=poseforge_core::Vec3::ZERO;
        for (i,p) in pixels.chunks_exact(4).enumerate() {let weight=if modern {p[2] as f32*p[1] as f32}else{p[0] as f32*p[1] as f32};
            let uv=(poseforge_core::Vec3::new((i%32) as f32,(i/32%32) as f32,(i/1024) as f32)+poseforge_core::Vec3::splat(0.5))/32.;
            mass+=weight;temperature+=(293.+p[1] as f32/255.*4400.)*weight;center+=uv*weight;}
        if mass>0. {temperature/=mass;center=domain.config.low+center/mass*(domain.config.high-domain.config.low);}
        else {temperature=293.;center=(domain.config.low+domain.config.high)*0.5;}
        self.images.insert(tick,CachedImage{pixels:pixels.clone(),stamp:self.stamp,light,temperature,center});self.prune();
        Ok(Image{slot:domain.slot,tick,pixels,light,temperature,center})
    }
    fn prune(&mut self) {
        while self.images.len()>FRAME_LIMIT {let key=self.images.iter().min_by_key(|(_,i)|i.stamp).map(|(&k,_)|k).unwrap();self.images.remove(&key);}
        while self.checkpoints.len()>CHECKPOINT_LIMIT {let key=self.checkpoints.iter().filter(|(&k,_)|k!=0).min_by_key(|(_,c)|c.stamp).map(|(&k,_)|k).unwrap();self.checkpoints.remove(&key);}
    }
    fn bytes(&self)->usize {self.solver.bytes()+self.images.values().map(|f|f.pixels.len()).sum::<usize>()+self.checkpoints.values().map(|c|c.frame.bytes()).sum::<usize>()}
}
#[derive(Default)]
struct Cache {key:u64,timelines:BTreeMap<u64,Timeline>}
impl Cache {
    fn bytes(&self)->usize {self.timelines.values().map(Timeline::bytes).sum()}
    fn trim(&mut self) {
        // Fixed per-domain bounds are supplemented by a shared ceiling.
        while self.bytes()>CACHE_BYTES {
            let candidate=self.timelines.iter().flat_map(|(&id,t)|t.images.iter().map(move |(&tick,c)|(c.stamp,id,tick))).min();
            if let Some((_,id,tick))=candidate {self.timelines.get_mut(&id).unwrap().images.remove(&tick);continue;}
            let candidate=self.timelines.iter().flat_map(|(&id,t)|t.checkpoints.iter().filter(|(&tick,_)|tick!=0).map(move |(&tick,c)|(c.stamp,id,tick))).min();
            if let Some((_,id,tick))=candidate {self.timelines.get_mut(&id).unwrap().checkpoints.remove(&tick);}else{break;}
        }
    }
}
struct Prepared {cache:Cache,images:BTreeMap<u64,Image>}
fn run(mut cache:Cache,key:u64,domains:&[Domain],targets:&[(u64,u32)],budget:usize,cancel:&AtomicBool)->Result<Prepared,String> {
    if cancel.load(Ordering::Relaxed) {return Err("Расчёт газа отменён".into());}
    if cache.key!=key {cache=Cache{key,..Default::default()};}
    cache.timelines.retain(|id,_|domains.iter().any(|d|d.id==*id));let mut images=BTreeMap::new();
    let each=(budget/targets.len().max(1)).max(1);
    for &(id,tick) in targets {
        let d=domains.iter().find(|d|d.id==id).ok_or("Область газа удалена")?;
        if !cache.timelines.contains_key(&id) {cache.timelines.insert(id,Timeline::new(d,cancel)?);}
        let image=cache.timelines.get_mut(&id).unwrap().image(d,tick,each,cancel)?;images.insert(id,image);
    }
    cache.trim();Ok(Prepared{cache,images})
}
struct Job {key:u64,request:u64,cancel:Arc<AtomicBool>,receiver:mpsc::Receiver<Result<Prepared,String>>}
impl Drop for Job {fn drop(&mut self){self.cancel.store(true,Ordering::Relaxed);}}
#[derive(Default)]
pub(super) struct State {
    job:Option<Job>,cache:Cache,key:u64,generation:u64,targets:Vec<(u64,u32)>,ready:BTreeMap<u64,Image>,
    failed:Option<u64>,message:Option<String>,memory:usize,
}
impl State {
    pub fn pending(&self)->bool {self.job.is_some()}
    pub fn blocked(&self)->bool {self.targets.iter().any(|(id,tick)|self.ready.get(id).is_none_or(|f|f.tick!=*tick))}
    pub fn available(&self,id:u64)->bool {self.ready.contains_key(&id)}
    pub fn light(&self,id:u64)->f32 {self.ready.get(&id).map_or(0.,|i|i.light)}
    pub fn light_temperature(&self,id:u64)->f32 {self.ready.get(&id).map_or(293.,|i|i.temperature)}
    pub fn light_center(&self,id:u64)->Option<poseforge_core::Vec3> {self.ready.get(&id).map(|i|i.center)}
    pub fn images(&self)->(u64,&BTreeMap<u64,Image>) {(self.key,&self.ready)}
    pub fn status(&self)->Option<&str> {self.message.as_deref()}
    pub fn memory(&self)->usize {self.memory}
    pub fn clear(&mut self) {
        if let Some(job)=&self.job {job.cancel.store(true,Ordering::Relaxed);}
        self.generation=self.generation.wrapping_add(1);self.cache=Cache::default();self.ready.clear();self.targets.clear();self.failed=None;self.message=None;self.memory=0;
    }
    pub fn update(&mut self,input:Option<(u64,Arc<Vec<Domain>>)>,project:&Project,time:f64,export:bool,external_cancel:&AtomicBool)->Result<(),String> {
        let (key,domains)=input.unwrap_or_else(||(0,Arc::new(vec![])));
        let mut version=std::collections::hash_map::DefaultHasher::new();key.hash(&mut version);self.generation.hash(&mut version);let key=version.finish();
        let targets:Vec<_>=domains.iter().filter_map(|d|project.node(d.id).and_then(|n|n.fx).filter(|f|f.enabled && f.visible_at(time))
            .map(|f|(d.id,(f.local_time(time)*gas::HZ as f32+0.0001).floor() as u32))).collect();
        let mut hash=std::collections::hash_map::DefaultHasher::new();key.hash(&mut hash);targets.hash(&mut hash);let request=hash.finish();
        if self.key!=key {self.ready.clear();self.failed=None;self.memory=0;}
        self.key=key;self.targets=targets.clone();
        if let Some(job)=&self.job {
            // A new clock tick does not invalidate the simulation. Let the batch
            // finish and reuse its checkpoint instead of restarting from frame 0.
            if job.key!=key || domains.is_empty() {job.cancel.store(true,Ordering::Relaxed);}
            let result=match job.receiver.try_recv() {Ok(r)=>Some(r),Err(mpsc::TryRecvError::Empty)=>None,Err(mpsc::TryRecvError::Disconnected)=>Some(Err("Рабочий поток газа остановлен".into()))};
            // Exports need an exact frame. Reuse an already completed batch; only
            // cancel one that is still computing so the export can run synchronously.
            if export && result.is_none() {job.cancel.store(true,Ordering::Relaxed);}
            if let Some(result)=result {
                let compatible=job.key==key && !job.cancel.load(Ordering::Relaxed);
                let current=compatible && job.request==request;
                self.job=None;
                match result {
                    Ok(prepared) if compatible && prepared.cache.key==key=>{
                        self.cache=prepared.cache;self.memory=self.cache.bytes();self.failed=None;
                        // Show incremental progress when playback moved forward.
                        // Never show a future frame after scrubbing backwards.
                        for (id,image) in prepared.images {
                            if targets.iter().any(|&(target_id,tick)|target_id==id && image.tick<=tick)
                                && self.ready.get(&id).is_none_or(|previous|previous.tick<=image.tick) {
                                self.ready.insert(id,image);
                            }
                        }
                    },
                    Err(error) if current=>{self.memory=0;self.failed=Some(request);self.message=Some(error);},
                    _=>{},
                }
            }
        }
        if domains.is_empty() {self.ready.clear();self.targets.clear();self.cache=Cache::default();self.message=None;self.memory=0;return Ok(());}
        self.ready.retain(|id,image|targets.iter().any(|&(other,tick)|*id==other && image.tick<=tick));
        if targets.is_empty() {self.message=None;return Ok(());}
        if !self.blocked() {self.message=None;return Ok(());}
        if self.failed==Some(request) && !export {return Ok(());}
        if export {
            let prepared=run(std::mem::take(&mut self.cache),key,&domains,&targets,gas::HZ as usize*30*poseforge_core::MAX_FX,external_cancel)?;
            self.cache=prepared.cache;self.memory=self.cache.bytes();self.ready=prepared.images;self.failed=None;self.message=None;return Ok(());
        }
        // Cache hits do not launch a worker or rebuild the simulation.
        if self.job.is_none() && self.cache.key==key && targets.iter().all(|(id,tick)|self.cache.timelines.get(id).is_some_and(|t|t.images.contains_key(tick))) {
            let prepared=run(std::mem::take(&mut self.cache),key,&domains,&targets,0,external_cancel)?;
            self.cache=prepared.cache;self.memory=self.cache.bytes();self.ready=prepared.images;self.message=None;return Ok(());
        }
        let done:u32=self.ready.values().map(|i|i.tick).sum();let total:u32=targets.iter().map(|(_,t)|*t).sum();
        self.message=Some(format!("Газ: кадры {done}/{total} · кэш {:.1} МБ",self.memory as f32/1048576.));
        if self.job.is_some() {return Ok(());}
        let cache=std::mem::take(&mut self.cache);let flag=Arc::new(AtomicBool::new(false));let cancel=flag.clone();let (sender,receiver)=mpsc::channel();
        match std::thread::Builder::new().name("fx-gas".into()).spawn(move || {let _=sender.send(run(cache,key,&domains,&targets,BATCH_STEPS,&cancel));}) {
            Ok(_)=>self.job=Some(Job{key,request,cancel:flag,receiver}),
            Err(e)=>{self.memory=0;self.failed=Some(request);self.message=Some(format!("Не удалось запустить расчёт газа: {e}"));},
        }Ok(())
    }
}

pub(super) fn prepare(id:u64,slot:usize,f:FxSettings,model:poseforge_core::Mat4,sources:Vec<poseforge_core::Vec3>,radius:f32,
    obstacles:Option<&poseforge_simulation::mesh_obstacles::MeshObstacles>,meshes:&[&[poseforge_simulation::mesh_contact::Triangle]],
    fields:Vec<poseforge_simulation::fields::Field>,cancel:&AtomicBool,budget:&mut usize)
    ->Result<(super::fx_interactions::Guides,Domain,Vec<u8>),String> {
    use poseforge_core::Vec3;use poseforge_simulation::{fx_collision,fields::Fields};
    if slot>=poseforge_core::MAX_FX {return Err("Слишком много областей газа".into());}
    let sources:Vec<_>=sources.into_iter().map(|p|model.inverse().transform_point3(p)).collect();
    let mut low=sources.first().copied().unwrap_or(Vec3::ZERO);let mut high=low;
    for &p in &sources {low=low.min(p);high=high.max(p);}
    let pad=(f.width*f.gas_domain*0.5).max(radius*1.25);
    let bottom=if f.pyro.is_some(){(f.height*0.12).max(radius*0.4)}else{f.height*0.12};
    let top=if f.pyro.is_some(){(f.height*f.gas_domain).max(radius*1.7)}else{f.height*f.gas_domain};
    low-=Vec3::new(pad,bottom,pad);high+=Vec3::new(pad,top,pad);
    let mask=if let Some(obstacles)=obstacles {fx_collision::build_with_interior(meshes,obstacles,model,low,high,cancel,budget,!f.gas_hollow)?}
        else{vec![255;fx_collision::MASK_CELLS]};
    let n=f.quality.grid();let mut solid=vec![false;n.pow(3)];
    // Every overlapping high-resolution obstacle cell participates in a coarse cell.
    for z in 0..n {if cancel.load(Ordering::Relaxed){return Err("Подготовка газа отменена".into());}for y in 0..n {for x in 0..n {
        let lo=[x,y,z].map(|v|v*MASK_EDGE/n);let hi=[x+1,y+1,z+1].map(|v|(v*MASK_EDGE+n-1)/n);
        solid[(z*n+y)*n+x]=(lo[2]..hi[2]).any(|k|(lo[1]..hi[1]).any(|j|(lo[0]..hi[0]).any(|i|mask[(k*MASK_EDGE+j)*MASK_EDGE+i]==0)));
    }}}
    let pyro=f.pyro.map(|p|gas::Pyro {kind:f.kind.index() as u8,mode:p.mode.index(),rate:p.source_rate,temperature:(p.source_temperature-293.)/2200.,fuel:p.fuel,
        ignition:(p.ignition-293.)/2200.,burn_rate:p.burn_rate,heat_release:p.heat_release/2200.,smoke_yield:p.smoke_yield,cooling:p.cooling,expansion:p.expansion,
        impulse:p.impulse,burst_seconds:p.burst_seconds,vorticity:p.vorticity,fog_falloff:p.fog_falloff,
        fill_domain:f.source.is_none() && f.kind==poseforge_core::FxKind::Fog && p.mode!=poseforge_core::FxEmission::Burst,noise_scale:f.detail,initial_size:f.envelope.sample(0.).size,
        cinematic_blast:p.cinematic_blast});
    let config=gas::Config {n,low,high,solid,sources,radius,fields:Fields::new(fields,model)?,influence:f.field_influence,
        buoyancy:f.buoyancy*[3.,2.,0.15,3.][f.kind.index() as usize],dissipation:f.dissipation,turbulence:f.turbulence,
        temperature:[1.,0.5,0.03,0.85][f.kind.index() as usize],source_velocity:Vec3::new(f.drift[0],f.height*0.6,f.drift[1]),
        seed:f.seed,iterations:f.quality.iterations(),cloud:f.cloud,burst:f.burst,duration:f.duration,pyro};
    config.validate()?;
    let guides=super::fx_interactions::Guides {low:low.extend(0.).to_array(),high:high.extend(0.).to_array(),
        meta:[0.,1.,if obstacles.is_some(){(slot+1) as f32}else{0.},(slot+1) as f32],..Default::default()};
    Ok((guides,Domain{id,slot,config:Arc::new(config),settings:f},mask))
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Editor,FxKind,Vec3};
    fn scene()->(Project,Arc<Vec<Domain>>,u64) {
        let mut e=Editor::new(Project::default());let id=e.create_fx(FxKind::Smoke).unwrap();
        let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.gas=true;f.speed=1.;
        let config=gas::Config {n:8,low:Vec3::new(-0.5,-0.1,-0.5),high:Vec3::new(0.5,1.,0.5),solid:vec![false;512],sources:vec![Vec3::ZERO],radius:0.22,
            fields:Default::default(),influence:1.,buoyancy:2.,dissipation:0.1,turbulence:0.2,temperature:0.5,source_velocity:Vec3::Y,seed:1,iterations:12,cloud:false,burst:false,duration:3.,pyro:None};
        let d=Domain{id,slot:0,config:Arc::new(config),settings:*f};(e.project,Arc::new(vec![d]),id)
    }
    #[test]
    fn cached_scrubbing_reuses_images_and_evicted_frames_replay_from_checkpoints() {
        let (p,domains,id)=scene();let cancel=AtomicBool::new(false);let mut s=State::default();
        s.update(Some((7,domains.clone())),&p,0.4,true,&cancel).unwrap();let first=s.ready[&id].pixels.clone();
        s.update(Some((7,domains.clone())),&p,0.6,true,&cancel).unwrap();
        s.update(Some((7,domains.clone())),&p,0.4,false,&cancel).unwrap();assert!(!s.pending());assert!(Arc::ptr_eq(&first,&s.ready[&id].pixels));
        s.cache.timelines.get_mut(&id).unwrap().images.remove(&12);s.ready.clear();
        s.update(Some((7,domains)),&p,0.4,true,&cancel).unwrap();assert_eq!(*s.ready[&id].pixels,*first);assert!(!s.blocked());
    }
    #[test]
    fn clear_invalidates_already_completed_jobs_and_stale_frames() {
        let (p,domains,id)=scene();let cancel=AtomicBool::new(false);let mut s=State::default();
        s.update(Some((9,domains.clone())),&p,0.3,true,&cancel).unwrap();let old_key=s.key;
        let (sender,receiver)=mpsc::channel();let flag=Arc::new(AtomicBool::new(false));
        let prepared=Prepared{cache:std::mem::take(&mut s.cache),images:std::mem::take(&mut s.ready)};
        assert!(sender.send(Ok(prepared)).is_ok());s.job=Some(Job{key:old_key,request:1,cancel:flag.clone(),receiver});
        s.clear();assert!(flag.load(Ordering::Relaxed));s.update(Some((9,domains)),&p,0.,true,&cancel).unwrap();
        assert_ne!(s.key,old_key);assert_eq!(s.ready[&id].tick,0);assert!(s.ready[&id].pixels.iter().all(|b|*b==0));
    }
    #[test]
    fn hidden_deleted_and_cancelled_exports_do_not_publish_old_density() {
        let (mut p,domains,id)=scene();let cancel=AtomicBool::new(false);let mut s=State::default();
        s.update(Some((4,domains.clone())),&p,0.2,true,&cancel).unwrap();assert!(s.available(id));
        p.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=false;
        s.update(Some((4,domains.clone())),&p,0.2,true,&cancel).unwrap();assert!(!s.available(id));assert!(!s.blocked());
        s.update(None,&p,0.2,false,&cancel).unwrap();assert_eq!(s.memory(),0);
        p.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=true;cancel.store(true,Ordering::Relaxed);
        assert!(s.update(Some((4,domains)),&p,1.,true,&cancel).is_err());assert!(!s.available(id));
    }
    #[test]
    fn bounded_lru_keeps_recent_images_and_the_initial_checkpoint() {
        let (_,domains,_)=scene();let mut t=Timeline::new(&domains[0],&AtomicBool::new(false)).unwrap();
        for tick in 0..100 {t.images.insert(tick,CachedImage{pixels:Arc::new(vec![0;32*32*32*4]),stamp:tick as u64,light:0.,temperature:293.,center:Vec3::ZERO});
            let mut frame=t.solver.frame.clone();frame.tick=tick;t.checkpoints.insert(tick,Checkpoint{frame,stamp:tick as u64});t.prune();}
        assert_eq!(t.images.len(),FRAME_LIMIT);assert!(t.images.contains_key(&99));assert!(!t.images.contains_key(&0));
        assert_eq!(t.checkpoints.len(),CHECKPOINT_LIMIT);assert!(t.checkpoints.contains_key(&0));assert!(t.bytes()<CACHE_BYTES);
    }
    #[test]
    fn changed_playback_time_keeps_the_running_batch_alive() {
        let (p,domains,id)=scene();let cancel=AtomicBool::new(false);let mut s=State::default();
        s.update(Some((21,domains.clone())),&p,0.,true,&cancel).unwrap();
        let targets=vec![(id,3_u32)];let mut hash=std::collections::hash_map::DefaultHasher::new();
        s.key.hash(&mut hash);targets.hash(&mut hash);
        let (_sender,receiver)=mpsc::channel();let flag=Arc::new(AtomicBool::new(false));
        s.job=Some(Job{key:s.key,request:hash.finish(),cancel:flag.clone(),receiver});
        s.update(Some((21,domains)),&p,0.2,false,&cancel).unwrap();
        assert!(s.pending());assert!(!flag.load(Ordering::Relaxed));
        s.clear();assert!(flag.load(Ordering::Relaxed));
    }
    #[test]
    fn stale_completed_batch_preserves_progress_toward_newer_frame() {
        let (p,domains,id)=scene();let cancel=AtomicBool::new(false);let mut s=State::default();
        s.update(Some((22,domains.clone())),&p,0.,true,&cancel).unwrap();
        let old=vec![(id,5_u32)];let mut hash=std::collections::hash_map::DefaultHasher::new();
        s.key.hash(&mut hash);old.hash(&mut hash);
        let prepared=run(std::mem::take(&mut s.cache),s.key,&domains,&old,2,&cancel).unwrap();
        assert_eq!(prepared.images[&id].tick,2);
        let (sender,receiver)=mpsc::channel();sender.send(Ok(prepared)).unwrap();
        s.job=Some(Job{key:s.key,request:hash.finish(),cancel:Arc::new(AtomicBool::new(false)),receiver});
        s.update(Some((22,domains)),&p,0.3,false,&cancel).unwrap();
        assert_eq!(s.ready[&id].tick,2);
        assert!(s.blocked());
        // The next worker owns the cache now. Starting at tick 2 reaches tick 9
        // in one eight-step batch; restarting at zero would only reach tick 8.
        let job=s.job.take().unwrap();
        let prepared=job.receiver.recv_timeout(std::time::Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(prepared.cache.timelines[&id].solver.frame.tick,9);
        assert_eq!(prepared.images[&id].tick,9);
        s.clear();
    }
    #[test]
    fn backwards_scrub_never_displays_a_future_gas_image() {
        let (p,domains,id)=scene();let cancel=AtomicBool::new(false);let mut s=State::default();
        s.update(Some((23,domains.clone())),&p,0.4,true,&cancel).unwrap();
        assert_eq!(s.ready[&id].tick,12);
        s.update(Some((23,domains)),&p,0.1,false,&cancel).unwrap();
        assert!(!s.available(id));assert!(s.blocked());
        s.clear();
    }
    #[test]
    fn disconnected_worker_reports_once_and_clear_allows_recovery() {
        let (p,domains,id)=scene();let cancel=AtomicBool::new(false);let mut s=State::default();
        s.update(Some((3,domains.clone())),&p,0.,true,&cancel).unwrap();s.ready.clear();
        let targets=vec![(id,3_u32)];let mut h=std::collections::hash_map::DefaultHasher::new();s.key.hash(&mut h);targets.hash(&mut h);
        let (sender,receiver)=mpsc::channel();drop(sender);s.job=Some(Job{key:s.key,request:h.finish(),cancel:Arc::new(AtomicBool::new(false)),receiver});
        s.update(Some((3,domains.clone())),&p,0.1,false,&cancel).unwrap();assert!(!s.pending());assert!(s.status().unwrap().contains("остановлен"));
        s.update(Some((3,domains.clone())),&p,0.1,false,&cancel).unwrap();assert!(!s.pending());
        s.clear();s.update(Some((3,domains)),&p,0.1,true,&cancel).unwrap();assert!(!s.blocked());assert!(s.status().is_none());
    }
}
