//! Source -> advection -> reaction -> forces -> pressure, on the shared MAC grid.
//! A bounded visual combustion model; it does not model compressible shock waves.
use super::*;
#[derive(Clone,Copy,Debug)]
pub struct Pyro {
    pub kind:u8,pub mode:u8,pub rate:f32,pub temperature:f32,pub fuel:f32,pub ignition:f32,pub burn_rate:f32,
    pub heat_release:f32,pub smoke_yield:f32,pub cooling:f32,pub expansion:f32,pub impulse:f32,pub burst_seconds:f32,
    pub vorticity:f32,pub fog_falloff:f32,pub fill_domain:bool,pub noise_scale:f32,pub initial_size:f32,
    pub cinematic_blast:bool,
}
impl Pyro {
    pub fn validate(self)->Result<(),String> {
        if self.kind>3 || self.mode>2 {return Err("Неизвестный тип источника FX".into());}
        for (v,lo,hi) in [(self.rate,0.,12.),(self.temperature,0.,2.),(self.fuel,0.,4.),(self.ignition,0.,1.),(self.burn_rate,0.05,20.),
            (self.heat_release,0.,2.),(self.smoke_yield,0.,3.),(self.cooling,0.,5.),(self.expansion,0.,20.),(self.impulse,0.,12.),
            (self.burst_seconds,0.02,5.),(self.vorticity,0.,8.),(self.fog_falloff,0.,8.),(self.noise_scale,1.,16.),(self.initial_size,0.,1.)] {
            if !v.is_finite() || !(lo..=hi).contains(&v) {return Err("Некорректный источник или реакция FX".into());}
        }Ok(())
    }
    /// Analytically integrated pulse: even a burst shorter than a frame is emitted,
    /// and its total amount is independent of the number of solver substeps.
    fn dose(self,time:f32,dt:f32,duration:f32)->f32 {
        match self.mode {
            0=>self.rate*dt,
            1=>{let width=self.burst_seconds.min(duration);let cdf=|t:f32|{let u=(t/width).clamp(0.,1.);u-(std::f32::consts::TAU*u).sin()/std::f32::consts::TAU};self.rate*(cdf(time+dt)-cdf(time)).max(0.)},
            _=>0.,
        }
    }
}
fn noise(p:Vec3,seed:u32)->f32 {
    let t=(seed%1024) as f32*0.137;
    ((p.x*1.37+p.y*0.71+t).sin()*(p.y*1.83-p.z*0.93+t*1.7).sin()*(p.z*1.19+p.x*0.47-t).cos()).clamp(-1.,1.)
}
impl Solver {
    fn source_weight(&self,i:usize,size:f32,p:Pyro)->f32 {
        let c=&self.config;if c.solid[i] || size<=0. {return 0.;}
        // Static, seeded crown/stem/dust shape is built once when the domain
        // starts, not per cell, per substep or per pressure iteration.
        if p.cinematic_blast && p.kind==0 && p.mode==1 {
            return self.work.blast_profile[i]*size.clamp(0.,1.);
        }
        let pos=c.pos(i%c.n,i/c.n%c.n,i/(c.n*c.n));
        let base=if p.fill_domain {
            let uv=(pos-c.low)/(c.high-c.low);let edge=uv.min(Vec3::ONE-uv).min_element();
            (edge*c.n as f32).min(1.)*(-uv.y*p.fog_falloff).exp()*size
        }else{
            let radius=size.max(self.work.source_min_size);
            (1.-self.work.distance[i]/radius).clamp(0.,1.).powi(2)
        };
        base*(1.+noise(pos*p.noise_scale,c.seed)*c.turbulence*0.7).max(0.)
    }
    pub(super) fn prepare_pyro(&mut self,cancel:&AtomicBool)->Result<(),String> {
        let c=self.config.clone();let p=c.pyro.unwrap();let n=c.n;
        // Identify sealed pockets once. The pressure system in such a component
        // has a constant nullspace; its right-hand side must have zero mean.
        let mut visited=vec![false;c.cells()];
        for root in 0..c.cells() {
            if c.solid[root] || visited[root] {continue;}let mut cells=vec![root];visited[root]=true;let mut cursor=0;let mut open=false;
            while cursor<cells.len() {
                if cursor%1024==0 {check(cancel)?;}let i=cells[cursor];cursor+=1;let xyz=[(i%n) as i32,(i/n%n) as i32,(i/(n*n)) as i32];
                for a in 0..3 {for sign in [-1,1] {let mut q=xyz;q[a]+=sign;match c.cell(q) {
                    None=>open=true,Some(j) if !c.solid[j] && !visited[j]=>{visited[j]=true;cells.push(j);},_=>{}
                }}}
            }
            if !open {self.work.sealed.push(cells);}
        }
        if p.cinematic_blast && p.kind==0 && p.mode==1 {
            self.work.blast_profile=vec![0.;c.cells()];
            let lobe=|q:Vec3,center:Vec3,radii:Vec3|->f32 {
                let d=((q-center)/radii).length();(1.-d).max(0.).powi(2)
            };
            for i in 0..c.cells() {
                if i%(n*n)==0 {check(cancel)?;}
                if c.solid[i] {continue;}
                let pos=c.pos(i%n,i/n%n,i/(n*n));
                let mut weight=0_f32;
                for &source in &c.sources {
                    let q=(pos-source)/c.radius;
                    let crown=lobe(q,Vec3::new(0.,1.8,0.),Vec3::new(2.8,1.7,2.8))*0.95;
                    let left=lobe(q,Vec3::new(-1.45,1.18,0.35),Vec3::new(1.7,1.3,1.9))*0.86;
                    let right=lobe(q,Vec3::new(1.4,1.35,-0.4),Vec3::new(1.9,1.5,1.7))*0.9;
                    let stem=lobe(q,Vec3::new(0.,0.7,0.),Vec3::new(1.05,1.7,1.05))*0.9;
                    let radial=(q.x*q.x+q.z*q.z).sqrt();
                    let skirt=(1.-(radial-2.1).abs()/0.9).max(0.).powi(2)
                        *(1.-(q.y-0.3).abs()/0.6).max(0.).powi(2)*0.58;
                    let cloud=crown.max(left).max(right).max(stem).max(skirt);
                    weight=weight.max(cloud);
                }
                let billow=1.+noise(pos*p.noise_scale,c.seed)*c.turbulence*0.65;
                self.work.blast_profile[i]=(weight*billow.max(0.2)).min(1.);
            }
        }
        if p.mode==2 {
            // Source weights contain trigonometric noise. Cache one weight per
            // cell instead of evaluating it again for every neighboring face.
            for i in 0..c.cells() {
                if i%(n*n)==0 {check(cancel)?;}
                let weight=self.source_weight(i,p.initial_size,p);
                self.work.injection[i]=weight;
            }
            for i in 0..c.cells() {
                let amount=self.work.injection[i]*p.rate;
                if p.kind==0 {self.frame.fuel[i]=(amount*p.fuel).min(4.);}else{self.frame.density[i]=amount.min(1.);}
                self.frame.heat[i]=(p.temperature*amount.min(1.)).clamp(0.,2.);
            }
            // Fill sources have one initial momentum deposit, not an ongoing jet.
            for z in 0..=n {check(cancel)?;for y in 0..=n {for x in 0..=n {let i=c.face_index(x,y,z);for axis in 0..3 {
                if !c.face_valid([x,y,z],axis) || !c.face_free(x,y,z,axis) {continue;}
                let (a,b)=c.neighbors(x,y,z,axis);let weight=(a.map_or(0.,|j|self.work.injection[j])+b.map_or(0.,|j|self.work.injection[j]))*0.5;
                self.frame.velocity[i][axis]=c.source_velocity[axis]*p.impulse*(weight*p.rate).min(1.);
            }}}}self.project(1./HZ as f32,cancel)?;
        }check(cancel)
    }
    fn vorticity(&mut self,cancel:&AtomicBool)->Result<(),String> {
        let c=self.config.clone();let n=c.n;let h=c.h();let gain=c.pyro.unwrap().vorticity;
        if gain<=0. {self.work.confinement.fill(Vec3::ZERO);return Ok(());}
        for z in 0..n {check(cancel)?;for y in 0..n {for x in 0..n {let i=(z*n+y)*n+x;let face=c.face_index(x,y,z);let v=&self.frame.velocity;
            self.work.cell_velocity[i]=if c.solid[i]{Vec3::ZERO}else{Vec3::new((v[face].x+v[c.face_index(x+1,y,z)].x)*0.5,(v[face].y+v[c.face_index(x,y+1,z)].y)*0.5,(v[face].z+v[c.face_index(x,y,z+1)].z)*0.5)};
        }}}
        for z in 0..n {check(cancel)?;for y in 0..n {for x in 0..n {let i=(z*n+y)*n+x;
            if c.solid[i] {self.work.curl[i]=Vec3::ZERO;continue;}
            let derivative=|axis:usize|{let mut a=[x as i32,y as i32,z as i32];let mut b=a;a[axis]-=1;b[axis]+=1;
                let at=|q|c.cell(q).filter(|&j|!c.solid[j]).map_or(self.work.cell_velocity[i],|j|self.work.cell_velocity[j]);(at(b)-at(a))/(2.*h[axis])};
            let dx=derivative(0);let dy=derivative(1);let dz=derivative(2);self.work.curl[i]=Vec3::new(dy.z-dz.y,dz.x-dx.z,dx.y-dy.x);
        }}}
        for z in 0..n {check(cancel)?;for y in 0..n {for x in 0..n {let i=(z*n+y)*n+x;
            if c.solid[i] {self.work.confinement[i]=Vec3::ZERO;continue;}
            let mut gradient=Vec3::ZERO;
            for axis in 0..3 {let mut a=[x as i32,y as i32,z as i32];let mut b=a;a[axis]-=1;b[axis]+=1;
                let at=|q|c.cell(q).filter(|&j|!c.solid[j]).map_or(self.work.curl[i].length(),|j|self.work.curl[j].length());gradient[axis]=(at(b)-at(a))/(2.*h[axis]);}
            self.work.confinement[i]=(gradient.normalize_or_zero().cross(self.work.curl[i])*gain*h.min_element()).clamp_length_max(200.);
        }}}Ok(())
    }
    pub(super) fn step_pyro(&mut self,size:f32,cancel:&AtomicBool)->Result<(),String> {
        let c=self.config.clone();let p=c.pyro.unwrap();let speed=self.frame.velocity.iter().map(|v|v.abs().max_element()).fold(0.,f32::max);
        let substeps=((speed+p.impulse*0.25)/(HZ as f32*c.h().min_element()*0.7)).ceil().clamp(1.,4.) as usize;
        let dt=1./(HZ as f32*substeps as f32);
        // The envelope size is constant during this frame, including all its
        // CFL substeps. Compute expensive noisy source weights just once.
        for i in 0..c.cells() {
            if i%(c.n*c.n)==0 {check(cancel)?;}
            let weight=self.source_weight(i,size,p);
            self.work.injection[i]=weight;
        }
        for sub in 0..substeps {
            check(cancel)?;let time=(self.frame.tick as f32+sub as f32/substeps as f32)/HZ as f32;
            self.pyro_substep(time,dt,cancel)?;
        }
        self.frame.tick+=1;check(cancel)
    }
    fn pyro_substep(&mut self,time:f32,dt:f32,cancel:&AtomicBool)->Result<(),String> {
        let c=self.config.clone();let p=c.pyro.unwrap();let n=c.n;let h=c.h();let dose=p.dose(time,dt,c.duration);
        self.vorticity(cancel)?;
        // Transport all reactants and smoke using the previous projected velocity.
        for z in 0..n {check(cancel)?;for y in 0..n {for x in 0..n {let i=(z*n+y)*n+x;
            if c.solid[i] {self.work.density[i]=0.;self.work.heat[i]=0.;self.work.fuel[i]=0.;self.work.flame[i]=0.;self.work.expansion[i]=0.;continue;}
            let pos=c.pos(x,y,z);let v=velocity(&c,&self.frame.velocity,pos);let mid=c.trace(pos,pos-v*dt*0.5);
            let back=c.trace(pos,pos-velocity(&c,&self.frame.velocity,mid)*dt);let source=self.work.injection[i]*dose;
            let mut rho=sample_scalar(&c,&self.frame.density,back)*(-c.dissipation*dt).exp();
            let mut heat=sample_scalar(&c,&self.frame.heat,back);let mut fuel=sample_scalar(&c,&self.frame.fuel,back);
            let mut flame=sample_scalar(&c,&self.frame.flame,back)*(-dt/0.16).exp();
            let mut sum=0.;let mut count=0.;
            for axis in 0..3 {for sign in [-1,1] {let mut q=[x as i32,y as i32,z as i32];q[axis]+=sign;
                if let Some(j)=c.cell(q) {if !c.solid[j] {sum+=self.frame.heat[j];count+=1.;}}
            }}
            if count>0. {heat+=(sum/count-heat)*(0.002*dt/(h.min_element()*h.min_element())).min(0.15);}
            heat*=(-p.cooling*dt*(1.+heat*0.5)).exp();
            if source>0. {heat+=(p.temperature-heat).max(0.)*(1.-(-source*6.).exp());}
            let mut expansion=if p.mode==1 {source/dt*p.expansion*0.25}else{0.};
            if p.kind==0 {
                fuel=(fuel+source*p.fuel).clamp(0.,4.);
                // The cinematic shock skirt carries soot as well as fuel. It
                // remains a simulated scalar, advected and dissipated normally.
                if p.cinematic_blast {rho+=source*p.smoke_yield*0.14;}
                let burnt=if heat>=p.ignition {fuel*(1.-(-p.burn_rate*dt).exp())}else{0.};
                fuel-=burnt;heat+=burnt*p.heat_release;rho+=burnt*p.smoke_yield;
                flame=(flame+burnt/dt*0.2).min(1.);expansion+=burnt/dt*p.expansion;
            }else{rho+=source;fuel=0.;flame=0.;}
            self.work.density[i]=rho.clamp(0.,1.);self.work.heat[i]=heat.clamp(0.,2.);self.work.fuel[i]=fuel;self.work.flame[i]=flame;self.work.expansion[i]=expansion.min(120.);
        }}}
        let cap=h.min_element()*0.7/dt;
        for z in 0..=n {check(cancel)?;for y in 0..=n {for x in 0..=n {let i=c.face_index(x,y,z);
            for axis in 0..3 {
                if !c.face_valid([x,y,z],axis) || !c.face_free(x,y,z,axis) {self.work.velocity[i][axis]=0.;continue;}
                let pos=c.face_pos(x,y,z,axis);let current=velocity(&c,&self.frame.velocity,pos);let mid=c.trace(pos,pos-current*dt*0.5);
                let back=c.trace(pos,pos-velocity(&c,&self.frame.velocity,mid)*dt);let value=sample_face(&c,&self.frame.velocity,back,axis)*(-dt*0.08).exp();
                let (a,b)=c.neighbors(x,y,z,axis);let heat=(a.map_or(0.,|j|self.work.heat[j])+b.map_or(0.,|j|self.work.heat[j]))*0.5;
                let confinement=(a.map_or(Vec3::ZERO,|j|self.work.confinement[j])+b.map_or(Vec3::ZERO,|j|self.work.confinement[j]))*0.5;
                let weight=(a.map_or(0.,|j|self.work.injection[j])+b.map_or(0.,|j|self.work.injection[j]))*0.5;
                let radial=(a.map_or(Vec3::ZERO,|j|self.work.source_direction[j])+b.map_or(Vec3::ZERO,|j|self.work.source_direction[j]))*0.5;
                let q=pos*p.noise_scale;let t=time*1.7+(c.seed%1024) as f32*0.137;
                // A divergence-free perturbation local to active injection.
                let curl_noise=Vec3::new((q.y+t).sin()-(q.z-t).cos(),(q.z+t).sin()-(q.x-t).cos(),(q.x+t).sin()-(q.y-t).cos());
                let force=c.fields.acceleration(pos,current)*c.influence+Vec3::Y*(heat*c.buoyancy)+confinement;
                let push=c.source_velocity*p.impulse+if p.mode==1 {radial*p.impulse*2.}else{Vec3::ZERO};
                let impulse=(push+curl_noise*c.turbulence*0.4)*weight*dose;
                self.work.velocity[i][axis]=(value+dt*force[axis]+impulse[axis]).clamp(-cap,cap);
            }
        }}}
        std::mem::swap(&mut self.frame.velocity,&mut self.work.velocity);self.project(dt,cancel)?;
        std::mem::swap(&mut self.frame.density,&mut self.work.density);std::mem::swap(&mut self.frame.heat,&mut self.work.heat);
        std::mem::swap(&mut self.frame.fuel,&mut self.work.fuel);std::mem::swap(&mut self.frame.flame,&mut self.work.flame);
        if self.frame.velocity.iter().any(|v|!v.is_finite()) || self.frame.density.iter().chain(&self.frame.heat).chain(&self.frame.fuel).chain(&self.frame.flame).any(|v|!v.is_finite()) {return Err("Переполнение расчёта горения FX".into());}check(cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pyro()->Pyro {Pyro {kind:0,mode:0,rate:0.,temperature:0.5,fuel:1.,ignition:0.2,burn_rate:3.,heat_release:0.4,smoke_yield:0.3,
        cooling:0.,expansion:0.,impulse:0.,burst_seconds:0.02,vorticity:0.,fog_falloff:2.,fill_domain:false,noise_scale:4.,initial_size:1.,cinematic_blast:false}}
    fn config(p:Pyro)->Config {Config {n:8,low:Vec3::ZERO,high:Vec3::ONE,solid:vec![false;512],sources:vec![Vec3::splat(0.5)],radius:0.3,
        fields:Fields::default(),influence:1.,buoyancy:0.,dissipation:0.,turbulence:0.,temperature:1.,source_velocity:Vec3::ZERO,
        seed:17,iterations:64,cloud:true,burst:p.mode==1,duration:3.,pyro:Some(p)}}
    #[test]
    fn cinematic_blast_builds_billow_and_dust_once_and_remains_replayable() {
        let cancel=AtomicBool::new(false);
        let p=Pyro{mode:1,rate:6.,temperature:1.,fuel:3.,ignition:0.05,
            burn_rate:7.,heat_release:0.8,smoke_yield:1.2,expansion:8.,impulse:7.,
            burst_seconds:0.22,vorticity:1.5,cinematic_blast:true,..pyro()};
        let mut c=config(p);c.n=12;c.low=Vec3::new(-0.9,-0.1,-0.9);
        c.high=Vec3::new(0.9,1.4,0.9);c.radius=0.32;
        c.sources=vec![Vec3::new(0.,0.1,0.)];c.solid=vec![false;12_usize.pow(3)];
        let mut s=Solver::new(Arc::new(c),&cancel).unwrap();
        assert_eq!(s.work.blast_profile.len(),12_usize.pow(3));
        assert!(s.work.blast_profile.iter().all(|v|v.is_finite() && (0. ..=1.).contains(v)));
        assert!(s.work.blast_profile.iter().filter(|&&v|v>0.1).count()>12);
        let profile=s.work.blast_profile.clone();
        for _ in 0..5 {s.step(1.,&cancel).unwrap();}
        let checkpoint=s.frame.clone();
        assert!(checkpoint.flame.iter().any(|&v|v>0.));
        assert!(checkpoint.density.iter().any(|&v|v>0.));
        assert_eq!(profile,s.work.blast_profile); // no noisy rebuild each tick
        for _ in 0..3 {s.step(1.,&cancel).unwrap();}
        let after=s.frame.clone();
        s.restore(checkpoint).unwrap();
        for _ in 0..3 {s.step(1.,&cancel).unwrap();}
        assert_eq!(after.density,s.frame.density);assert_eq!(after.flame,s.frame.flame);
    }
    #[test]
    fn short_pulse_emits_the_same_total_with_any_substep_count() {
        let p=Pyro{mode:1,rate:2.5,..pyro()};
        for hz in [30,60,120,1000] {let amount=(0..hz).map(|i|p.dose(i as f32/hz as f32,1./hz as f32,3.)).sum::<f32>();assert!((amount-p.rate).abs()<1e-5);}
        assert_eq!(p.dose(0.1,1./30.,3.),0.);assert_eq!(Pyro{mode:2,..p}.dose(0.,1./30.,3.),0.);
    }
    #[test]
    fn ignition_consumes_fuel_and_releases_heat_and_soot_without_creating_mass_from_cold_fuel() {
        let cancel=AtomicBool::new(false);let mut s=Solver::new(Arc::new(config(pyro())),&cancel).unwrap();
        s.frame.fuel.fill(1.);s.frame.heat.fill(0.1);s.step(1.,&cancel).unwrap();
        assert!(s.frame.fuel.iter().all(|&v|(v-1.).abs()<1e-6));assert!(s.frame.density.iter().all(|&v|v==0.));assert!(s.frame.flame.iter().all(|&v|v==0.));
        s.frame.heat.fill(0.5);s.step(1.,&cancel).unwrap();let burned=1.-(-3_f32/30.).exp();
        assert!(s.frame.fuel.iter().all(|&v|(v-(1.-burned)).abs()<1e-5));
        assert!(s.frame.heat.iter().all(|&v|(v-(0.5+burned*0.4)).abs()<1e-5));
        assert!(s.frame.density.iter().all(|&v|(v-burned*0.3).abs()<1e-5));assert!(s.frame.flame.iter().all(|&v|v>0.));
    }
    #[test]
    fn initial_fog_is_cold_and_motionless_until_a_force_is_applied() {
        let cancel=AtomicBool::new(false);let p=Pyro{kind:2,mode:2,rate:0.5,temperature:0.,fill_domain:true,..pyro()};
        let mut c=config(p);c.buoyancy=4.;let mut s=Solver::new(Arc::new(c),&cancel).unwrap();let density=s.frame.density.clone();
        assert!(density.iter().any(|&v|v>0.));for _ in 0..4 {s.step(1.,&cancel).unwrap();}
        assert!(s.frame.heat.iter().all(|&v|v==0.));assert!(s.frame.velocity.iter().all(|&v|v==Vec3::ZERO));
        assert!(density.iter().zip(&s.frame.density).all(|(a,b)|(a-b).abs()<1e-6));
        let empty=Solver::new(Arc::new(config(Pyro{initial_size:0.,..p})),&cancel).unwrap();assert!(empty.frame.density.iter().all(|&v|v==0.));
    }
    #[test]
    fn combustion_expansion_has_outflow_and_stays_finite_in_a_sealed_pocket() {
        let cancel=AtomicBool::new(false);let p=Pyro{expansion:5.,..pyro()};let mut c=config(p);let center=(4*8+4)*8+4;
        let mut open=Solver::new(Arc::new(c.clone()),&cancel).unwrap();open.frame.fuel[center]=1.;open.frame.heat[center]=0.5;open.step(1.,&cancel).unwrap();assert!(open.divergence(4,4,4)>0.1);
        for z in 0..8 {for y in 0..8 {for x in 0..8 {c.solid[(z*8+y)*8+x]=[x,y,z].iter().any(|&v|v==0 || v==7);}}}
        let mut closed=Solver::new(Arc::new(c),&cancel).unwrap();assert_eq!(closed.work.sealed.len(),1);
        closed.frame.fuel[center]=1.;closed.frame.heat[center]=0.5;for _ in 0..12 {closed.step(1.,&cancel).unwrap();}
        assert!(closed.frame.velocity.iter().all(|v|v.is_finite()));assert!(closed.work.pressure.iter().all(|v|v.is_finite() && v.abs()<1e6));
    }
    #[test]
    fn pyro_checkpoints_restore_all_four_fields_and_texture_channels() {
        let cancel=AtomicBool::new(false);let p=Pyro{mode:1,rate:2.,ignition:0.01,expansion:3.,impulse:2.,vorticity:1.,..pyro()};
        let mut s=Solver::new(Arc::new(config(p)),&cancel).unwrap();for _ in 0..5 {s.step(1.,&cancel).unwrap();}let saved=s.frame.clone();
        for _ in 0..5 {s.step(1.,&cancel).unwrap();}let expected=s.frame.clone();let image=s.texture(8,&cancel).unwrap();assert_eq!(image.len(),8*8*8*4);
        s.restore(saved).unwrap();for _ in 0..5 {s.step(1.,&cancel).unwrap();}
        assert_eq!(s.frame.fuel,expected.fuel);assert_eq!(s.frame.flame,expected.flame);assert_eq!(s.frame.heat,expected.heat);assert_eq!(s.frame.density,expected.density);assert_eq!(s.frame.velocity,expected.velocity);
        assert_eq!(s.texture(8,&cancel).unwrap(),image);
        s.frame.density.fill(0.);s.frame.heat.fill(1.);s.frame.flame.fill(0.5);s.frame.fuel.fill(4.);
        let rgba=s.texture(8,&cancel).unwrap();assert!(rgba.chunks_exact(4).all(|p|p==&[0,128,128,255]));
    }
    #[test]
    fn subcell_sources_emit_in_anisotropic_grids() {
        let cancel=AtomicBool::new(false);let p=Pyro{kind:1,rate:3.,..pyro()};let mut c=config(p);c.high=Vec3::new(4.,0.5,4.);c.radius=0.02;c.sources=vec![Vec3::new(2.,0.1,2.)];
        let mut s=Solver::new(Arc::new(c),&cancel).unwrap();s.step(1.,&cancel).unwrap();assert!(s.frame.density.iter().any(|&v|v>0.));
    }
    #[test]
    fn pyro_validation_cancellation_and_scalar_bounds_are_enforced() {
        let mut p=pyro();p.temperature=f32::INFINITY;assert!(config(p).validate().is_err());p=pyro();p.mode=3;assert!(config(p).validate().is_err());
        let cancel=AtomicBool::new(true);assert!(Solver::new(Arc::new(config(pyro())),&cancel).is_err());cancel.store(false,Ordering::Relaxed);
        let p=Pyro{rate:12.,fuel:4.,temperature:1.,heat_release:1.,expansion:20.,impulse:12.,vorticity:8.,..pyro()};let mut s=Solver::new(Arc::new(config(p)),&cancel).unwrap();
        for _ in 0..5 {s.step(1.,&cancel).unwrap();}assert!(s.frame.fuel.iter().all(|v|(0. ..=4.).contains(v)));assert!(s.frame.heat.iter().all(|v|(0. ..=2.).contains(v)));
        let tick=s.frame.tick;cancel.store(true,Ordering::Relaxed);assert!(s.step(1.,&cancel).is_err());assert_eq!(tick,s.frame.tick);
    }
}
