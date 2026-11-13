//! Bounded 3-D MAC smoke solver: semi-Lagrangian transport and pressure projection.
//! World geometry is supplied as a conservative solid mask. All loops are cancellable.
use crate::fields::Fields;
use glam::Vec3;
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
pub const HZ:u32=30;
pub const MAX_EDGE:usize=32;
#[path="pyro.rs"]
mod pyro;
pub use pyro::Pyro;
fn check(cancel:&AtomicBool)->Result<(),String> {if cancel.load(Ordering::Relaxed){Err("Расчёт газа отменён".into())}else{Ok(())}}
#[derive(Clone)]
pub struct Config {
    pub n:usize,pub low:Vec3,pub high:Vec3,pub solid:Vec<bool>,pub sources:Vec<Vec3>,pub radius:f32,
    pub fields:Fields,pub influence:f32,pub buoyancy:f32,pub dissipation:f32,pub turbulence:f32,
    pub temperature:f32,pub source_velocity:Vec3,pub seed:u32,pub iterations:usize,
    pub cloud:bool,pub burst:bool,pub duration:f32,
    pub pyro:Option<Pyro>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config()->Config {Config {n:8,low:Vec3::ZERO,high:Vec3::ONE,solid:vec![false;512],sources:vec![Vec3::new(0.25,0.2,0.5)],radius:0.22,
        fields:Fields::default(),influence:1.,buoyancy:2.,dissipation:0.,turbulence:0.2,temperature:1.,source_velocity:Vec3::Y,
        seed:7,iterations:64,cloud:false,burst:false,duration:3.,pyro:None}}
    fn norm(s:&Solver)->f32 {let mut sum=0.;let n=s.config.n;for z in 0..n {for y in 0..n {for x in 0..n {
        if !s.config.solid[(z*n+y)*n+x] {sum+=s.divergence(x,y,z).powi(2);}
    }}}sum.sqrt()}
    #[test]
    fn cached_pressure_stencil_matches_original_neighbors_and_boundary_weights() {
        let cancel=AtomicBool::new(false);
        let mut c=config();c.high=Vec3::new(2.,1.,4.);
        c.solid[(3*8+2)*8+4]=true;
        c.solid[(4*8+2)*8+4]=true;
        let s=Solver::new(Arc::new(c),&cancel).unwrap();
        let c=&s.config;
        let weights=Vec3::ONE/(c.h()*c.h());
        for z in 0..c.n {for y in 0..c.n {for x in 0..c.n {
            let i=(z*c.n+y)*c.n+x;
            let cached=&s.work.pressure_stencil[i];
            let mut expected=[u32::MAX;6];let mut diagonal=0.;
            if !c.solid[i] {
                for axis in 0..3 {for (side,sign) in [-1,1].into_iter().enumerate() {
                    let mut cell=[x as i32,y as i32,z as i32];cell[axis]+=sign;
                    let other=c.cell(cell);
                    if other.is_some_and(|j|c.solid[j]) {continue;}
                    diagonal+=weights[axis];
                    if let Some(j)=other {expected[axis*2+side]=j as u32;}
                }}
            }
            assert_eq!(cached.neighbors,expected);
            let inverse=if diagonal>0. {1./diagonal}else{0.};
            assert!((cached.inverse_diagonal-inverse).abs()<1e-7);
        }}}
    }
    #[test]
    fn pressure_reduces_divergence_and_deflects_flow_below_a_ceiling() {
        let cancel=AtomicBool::new(false);let mut c=config();
        for z in 0..8 {for x in 0..8 {c.solid[(z*8+6)*8+x]=true;}}
        let mut s=Solver::new(Arc::new(c),&cancel).unwrap();
        for z in 2..6 {for x in 2..6 {let i=s.config.face_index(x,5,z);s.frame.velocity[i].y=0.5;}}
        let before=norm(&s);s.project(1./HZ as f32,&cancel).unwrap();assert!(norm(&s)<before*0.35);
        assert!(s.frame.velocity.iter().any(|v|v.x.abs()>0.01 || v.z.abs()>0.01));
        for z in 0..8 {for x in 0..8 {assert_eq!(s.frame.velocity[s.config.face_index(x,6,z)].y,0.);}}
    }
    #[test]
    fn solid_wall_blocks_density_but_an_opening_allows_transport() {
        let cancel=AtomicBool::new(false);
        let run=|opening:bool| {
            let mut c=config();c.sources.clear();c.buoyancy=0.;c.turbulence=0.;
            c.fields=Fields::new(vec![crate::fields::Field {kind:1,center:Vec3::splat(0.5),axis:Vec3::X,strength:15.,radius:10.,falloff:0.,damping:0.}],glam::Mat4::IDENTITY).unwrap();
            for z in 0..8 {for y in 0..8 {c.solid[(z*8+y)*8+4]=!opening || !(2..6).contains(&y) || !(2..6).contains(&z);}}
            let mut s=Solver::new(Arc::new(c),&cancel).unwrap();
            for z in 0..8 {for y in 0..8 {for x in 0..4 {s.frame.density[(z*8+y)*8+x]=1.;}}}
            for _ in 0..12 {s.step(0.,&cancel).unwrap();}
            let mut right=0.;for z in 0..8 {for y in 0..8 {for x in 5..8 {right+=s.frame.density[(z*8+y)*8+x];}}}
            right
        };
        assert_eq!(run(false),0.);assert!(run(true)>0.1);
    }
    #[test]
    fn checkpoints_reproduce_transport_and_temperature_exactly() {
        let cancel=AtomicBool::new(false);let mut s=Solver::new(Arc::new(config()),&cancel).unwrap();
        for _ in 0..8 {s.step(1.,&cancel).unwrap();}let checkpoint=s.frame.clone();
        for _ in 0..6 {s.step(0.7,&cancel).unwrap();}let expected=s.frame.clone();let image=s.texture(32,&cancel).unwrap();
        s.restore(checkpoint).unwrap();for _ in 0..6 {s.step(0.7,&cancel).unwrap();}
        assert_eq!(s.frame.density,expected.density);assert_eq!(s.frame.heat,expected.heat);assert_eq!(s.frame.velocity,expected.velocity);
        assert_eq!(s.texture(32,&cancel).unwrap(),image);assert!(image.iter().any(|v|*v>0));
    }
    #[test]
    fn cancellation_invalid_inputs_and_strong_fields_stay_bounded() {
        let cancel=AtomicBool::new(true);assert!(Solver::new(Arc::new(config()),&cancel).is_err());cancel.store(false,Ordering::Relaxed);
        let mut c=config();c.n=64;assert!(c.validate().is_err());c=config();c.high.x=f32::NAN;assert!(c.validate().is_err());
        c=config();c.fields=Fields::new(vec![crate::fields::Field {kind:1,center:Vec3::ZERO,axis:Vec3::Z,strength:1000.,radius:10.,falloff:0.,damping:0.};64],glam::Mat4::IDENTITY).unwrap();
        let mut s=Solver::new(Arc::new(c),&cancel).unwrap();assert!(s.step(f32::NAN,&cancel).is_err());
        for _ in 0..3 {s.step(1.,&cancel).unwrap();}
        let cap=s.config.h().min_element()*0.7*HZ as f32;
        assert!(s.frame.velocity.iter().all(|v|v.is_finite() && v.abs().max_element()<=cap+1e-5));
        let tick=s.frame.tick;cancel.store(true,Ordering::Relaxed);assert!(s.step(1.,&cancel).is_err());assert_eq!(s.frame.tick,tick);
        assert!(s.texture(32,&cancel).is_err());
    }
}
impl Config {
    fn cells(&self)->usize {self.n*self.n*self.n}
    fn faces(&self)->usize {(self.n+1).pow(3)}
    fn h(&self)->Vec3 {(self.high-self.low)/self.n as f32}
    fn cell(&self,p:[i32;3])->Option<usize> {p.iter().all(|&v|v>=0 && v<self.n as i32).then(||(p[2] as usize*self.n+p[1] as usize)*self.n+p[0] as usize)}
    fn pos(&self,x:usize,y:usize,z:usize)->Vec3 {self.low+(Vec3::new(x as f32,y as f32,z as f32)+Vec3::splat(0.5))*self.h()}
    fn at(&self,p:Vec3)->Option<usize> {self.cell(((p-self.low)/self.h()).floor().to_array().map(|v|v as i32))}
    fn face_index(&self,x:usize,y:usize,z:usize)->usize {(z*(self.n+1)+y)*(self.n+1)+x}
    fn face_valid(&self,p:[usize;3],axis:usize)->bool {(0..3).all(|a|p[a] < self.n+usize::from(a==axis))}
    fn neighbors(&self,x:usize,y:usize,z:usize,axis:usize)->(Option<usize>,Option<usize>) {
        let right=[x as i32,y as i32,z as i32];let mut left=right;left[axis]-=1;(self.cell(left),self.cell(right))
    }
    fn face_free(&self,x:usize,y:usize,z:usize,axis:usize)->bool {
        let (a,b)=self.neighbors(x,y,z,axis);!a.is_some_and(|i|self.solid[i]) && !b.is_some_and(|i|self.solid[i])
    }
    fn face_pos(&self,x:usize,y:usize,z:usize,axis:usize)->Vec3 {let mut offset=Vec3::splat(0.5);offset[axis]=0.;self.low+(Vec3::new(x as f32,y as f32,z as f32)+offset)*self.h()}
    fn trace(&self,start:Vec3,end:Vec3)->Vec3 {
        let steps=((end-start).length()/self.h().min_element()*3.).ceil().clamp(1.,256.) as usize;
        let mut safe=start;
        for j in 1..=steps {let p=start.lerp(end,j as f32/steps as f32);if self.at(p).is_some_and(|i|self.solid[i]) {return safe;}safe=p;}
        end
    }
    pub fn validate(&self)->Result<(),String> {
        if !(8..=MAX_EDGE).contains(&self.n) || self.solid.len()!=self.cells() || self.sources.len()>16
            || !self.low.is_finite() || !self.high.is_finite() || !self.h().is_finite() || self.h().min_element()<=1e-6
            || !self.radius.is_finite() || self.radius<=0. || self.sources.iter().any(|p|!p.is_finite())
            || !self.source_velocity.is_finite() || !(4..=64).contains(&self.iterations)
            || [self.influence,self.buoyancy,self.dissipation,self.turbulence,self.temperature,self.duration].iter().any(|v|!v.is_finite() || *v<0.)
            || self.duration<0.1 || self.duration>30. || self.influence>2. || self.buoyancy>20. || self.dissipation>3. || self.turbulence>1. || self.temperature>1. {
            return Err("Некорректная область расчёта газа FX".into());
        }if let Some(p)=self.pyro {p.validate()?;}Ok(())
    }
}
#[derive(Clone)]
pub struct Frame {pub tick:u32,pub density:Vec<f32>,pub heat:Vec<f32>,pub fuel:Vec<f32>,pub flame:Vec<f32>,velocity:Vec<Vec3>}
impl Frame {pub fn bytes(&self)->usize {(self.density.len()+self.heat.len()+self.fuel.len()+self.flame.len())*4+self.velocity.len()*12}}
// Pressure connectivity is fixed until the domain is rebuilt. Resolving all six
// neighbors once avoids millions of boundary/solid checks per simulated second.
#[derive(Clone,Copy)]
struct PressureStencil {neighbors:[u32;6],inverse_diagonal:f32}
struct Work {velocity:Vec<Vec3>,density:Vec<f32>,heat:Vec<f32>,pressure:Vec<f32>,next_pressure:Vec<f32>,rhs:Vec<f32>,distance:Vec<f32>,injection:Vec<f32>,source_direction:Vec<Vec3>,
    fuel:Vec<f32>,flame:Vec<f32>,expansion:Vec<f32>,cell_velocity:Vec<Vec3>,curl:Vec<Vec3>,confinement:Vec<Vec3>,sealed:Vec<Vec<usize>>,source_min_size:f32,blast_profile:Vec<f32>,
    pressure_stencil:Vec<PressureStencil>}
pub struct Solver {pub config:Arc<Config>,pub frame:Frame,work:Work}
fn build_pressure_stencil(c:&Config,cancel:&AtomicBool)->Result<Vec<PressureStencil>,String> {
    let n=c.n;
    let weights=Vec3::ONE/(c.h()*c.h());
    let mut cells=Vec::with_capacity(c.cells());
    for z in 0..n {
        check(cancel)?;
        for y in 0..n {for x in 0..n {
            let i=(z*n+y)*n+x;
            let mut neighbors=[u32::MAX;6];
            let mut diagonal=0.;
            if !c.solid[i] {
                for axis in 0..3 {for (side,sign) in [-1,1].into_iter().enumerate() {
                    let mut p=[x as i32,y as i32,z as i32];p[axis]+=sign;
                    let adjacent=c.cell(p);
                    if adjacent.is_some_and(|j|c.solid[j]) {continue;}
                    // Open domain edges contribute to the diagonal, but have zero pressure.
                    diagonal+=weights[axis];
                    if let Some(j)=adjacent {neighbors[axis*2+side]=j as u32;}
                }}
            }
            cells.push(PressureStencil{neighbors,inverse_diagonal:if diagonal>0. {1./diagonal}else{0.}});
        }}
    }
    Ok(cells)
}
fn sample_face(c:&Config,v:&[Vec3],p:Vec3,axis:usize)->f32 {
    let mut offset=Vec3::splat(0.5);offset[axis]=0.;let q=(p-c.low)/c.h()-offset;let base=q.floor();let t=q-base;
    let b=base.to_array().map(|v|v as i32);let mut value=0.;
    for z in 0..2 {for y in 0..2 {for x in 0..2 {
        let delta=[x,y,z];let coord=[b[0]+x,b[1]+y,b[2]+z];if coord.iter().any(|&v|v<0) {continue;}
        let u=coord.map(|v|v as usize);if !c.face_valid(u,axis){continue;}
        let weight=(0..3).map(|a|if delta[a]==0 {1.-t[a]}else{t[a]}).product::<f32>();
        value+=weight*v[c.face_index(u[0],u[1],u[2])][axis];
    }}}value
}
fn velocity(c:&Config,v:&[Vec3],p:Vec3)->Vec3 {Vec3::new(sample_face(c,v,p,0),sample_face(c,v,p,1),sample_face(c,v,p,2))}
fn sample_scalar(c:&Config,v:&[f32],p:Vec3)->f32 {
    let q=(p-c.low)/c.h()-Vec3::splat(0.5);let base=q.floor();let t=q-base;let b=base.to_array().map(|v|v as i32);let mut value=0.;let mut weight=1.;
    for z in 0..2 {for y in 0..2 {for x in 0..2 {
        let delta=[x,y,z];let w=(0..3).map(|a|if delta[a]==0 {1.-t[a]}else{t[a]}).product::<f32>();
        if let Some(i)=c.cell([b[0]+x,b[1]+y,b[2]+z]) {if c.solid[i] {weight-=w;}else{value+=w*v[i];}}
    }}}if weight>1e-6 {value/weight}else{0.}
}
impl Solver {
    pub fn new(config:Arc<Config>,cancel:&AtomicBool)->Result<Self,String> {
        check(cancel)?;config.validate()?;let cells=config.cells();let faces=config.faces();
        let frame=Frame{tick:0,density:vec![0.;cells],heat:vec![0.;cells],fuel:vec![0.;cells],flame:vec![0.;cells],velocity:vec![Vec3::ZERO;faces]};
        let extra=if config.pyro.is_some(){cells}else{0};
        let pressure_stencil=build_pressure_stencil(&config,cancel)?;
        let mut work=Work{velocity:vec![Vec3::ZERO;faces],density:vec![0.;cells],heat:vec![0.;cells],pressure:vec![0.;cells],next_pressure:vec![0.;cells],rhs:vec![0.;cells],distance:vec![f32::INFINITY;cells],injection:vec![0.;cells],source_direction:vec![Vec3::ZERO;cells],
            fuel:vec![0.;cells],flame:vec![0.;cells],expansion:vec![0.;cells],cell_velocity:vec![Vec3::ZERO;extra],curl:vec![Vec3::ZERO;extra],confinement:vec![Vec3::ZERO;extra],sealed:vec![],source_min_size:0.,blast_profile:vec![],pressure_stencil};
        let c=&config;let n=c.n;let vertical=if c.cloud || c.burst {c.radius}else{(c.radius*0.18).max(c.h().y*1.1)};
        // A source smaller than a cell still covers its nearest cells. The
        // normalized cell diagonal handles anisotropic domains and round sources.
        work.source_min_size=if c.pyro.is_some(){(c.h()/Vec3::new(c.radius,vertical,c.radius)).length()*0.6}else{0.};
        for z in 0..n {check(cancel)?;for y in 0..n {for x in 0..n {
            let i=(z*n+y)*n+x;if c.solid[i] {continue;}let p=c.pos(x,y,z);
            for &source in &c.sources {
                let center=source+Vec3::Y*vertical*0.65;
                if c.at(center).is_some_and(|i|c.solid[i]) {continue;}
                let d=p-center;let distance=(d/Vec3::new(c.radius,vertical,c.radius)).length();
                // A cinematic crown extends farther than a normal emitter;
                // keep radial directions there so the same impulse also pushes
                // upper lobes and the ground skirt outward.
                let reach: f32=if c.pyro.is_some_and(|p|p.cinematic_blast && p.kind==0 && p.mode==1) {4.5}else{2.};
                if distance>reach.max(work.source_min_size) || distance>=work.distance[i] || c.trace(center,p).distance_squared(p)>1e-10 {continue;}
                work.distance[i]=distance;work.source_direction[i]=d.normalize_or_zero();
            }
        }}}
        let mut solver=Self{config,frame,work};if solver.config.pyro.is_some(){solver.prepare_pyro(cancel)?;}Ok(solver)
    }
    pub fn bytes(&self)->usize {self.frame.bytes()+self.config.solid.len()+self.work.velocity.len()*12+self.config.cells()*52
        +(self.work.cell_velocity.len()+self.work.curl.len()+self.work.confinement.len())*12
        +self.work.pressure_stencil.len()*std::mem::size_of::<PressureStencil>()+self.work.blast_profile.len()*4
        +self.work.sealed.iter().map(|v|v.len()*std::mem::size_of::<usize>()).sum::<usize>()}
    pub fn restore(&mut self,frame:Frame)->Result<(),String> {
        if frame.density.len()!=self.config.cells() || frame.heat.len()!=self.config.cells() || frame.fuel.len()!=self.config.cells() || frame.flame.len()!=self.config.cells() || frame.velocity.len()!=self.config.faces() {return Err("Размер кэша газа не совпадает с областью".into());}
        self.frame=frame;Ok(())
    }
    fn divergence(&self,x:usize,y:usize,z:usize)->f32 {
        let c=&self.config;let v=&self.frame.velocity;let h=c.h();let i=c.face_index(x,y,z);
        (v[c.face_index(x+1,y,z)].x-v[i].x)/h.x+(v[c.face_index(x,y+1,z)].y-v[i].y)/h.y+(v[c.face_index(x,y,z+1)].z-v[i].z)/h.z
    }
    fn project(&mut self,dt:f32,cancel:&AtomicBool)->Result<(),String> {
        let c=self.config.clone();let n=c.n;let h=c.h();let weights=Vec3::ONE/(h*h);self.work.pressure.fill(0.);
        for z in 0..n {check(cancel)?;for y in 0..n {for x in 0..n {let i=(z*n+y)*n+x;self.work.rhs[i]=if c.solid[i]{0.}else{(self.divergence(x,y,z)-self.work.expansion[i])/dt};}}}
        // A completely sealed incompressible pocket cannot expand. Remove the
        // incompatible constant term instead of letting the pressure drift forever.
        for cells in &self.work.sealed {let mean=cells.iter().map(|&i|self.work.rhs[i] as f64).sum::<f64>()/cells.len().max(1) as f64;for &i in cells {self.work.rhs[i]-=mean as f32;}}
        let coefficients=[weights.x,weights.x,weights.y,weights.y,weights.z,weights.z];
        for _ in 0..c.iterations {
            check(cancel)?;
            for i in 0..c.cells() {
                let stencil=&self.work.pressure_stencil[i];
                let mut sum=0.;
                for axis in 0..6 {
                    let neighbor=stencil.neighbors[axis];
                    if neighbor!=u32::MAX {sum+=coefficients[axis]*self.work.pressure[neighbor as usize];}
                }
                self.work.next_pressure[i]=(sum-self.work.rhs[i])*stencil.inverse_diagonal;
            }
            std::mem::swap(&mut self.work.pressure,&mut self.work.next_pressure);
        }
        for z in 0..=n {check(cancel)?;for y in 0..=n {for x in 0..=n {let i=c.face_index(x,y,z);
            for axis in 0..3 {
                if !c.face_valid([x,y,z],axis) || !c.face_free(x,y,z,axis) {self.frame.velocity[i][axis]=0.;continue;}
                let (a,b)=c.neighbors(x,y,z,axis);let p=|i:Option<usize>|i.map_or(0.,|j|self.work.pressure[j]);
                self.frame.velocity[i][axis]-=dt*(p(b)-p(a))/h[axis];
            }
        }}}
        // Uniform scaling preserves projection while limiting transport to < one cell/axis.
        let max=self.frame.velocity.iter().map(|v|v.abs().max_element()).fold(0.,f32::max);
        let cap=h.min_element()*0.7/dt;if max>cap {let ratio=cap/max;for v in &mut self.frame.velocity {*v*=ratio;}}
        Ok(())
    }
    pub fn step(&mut self,size:f32,cancel:&AtomicBool)->Result<(),String> {
        check(cancel)?;if !size.is_finite() || !(0. ..=1.).contains(&size) {return Err("Некорректный размер источника газа".into());}
        if self.config.pyro.is_some(){return self.step_pyro(size,cancel);}
        let c=self.config.clone();let dt=1./HZ as f32;let n=c.n;let h=c.h();let time=self.frame.tick as f32/HZ as f32;
        let phase=(time/c.duration).clamp(0.,1.);let radius=if c.burst {size*(0.2+0.8*(phase/0.2).min(1.))}else{size};
        let strength=if c.burst {if phase<0.2 {20.}else{0.}}else{4.};
        for i in 0..c.cells() {self.work.injection[i]=if radius<=0. {0.}else{(1.-self.work.distance[i]/radius.max(h.min_element()/c.radius*0.55)).clamp(0.,1.).powi(2)};}
        let cap=h.min_element()*0.7/dt;
        for z in 0..=n {check(cancel)?;for y in 0..=n {for x in 0..=n {let i=c.face_index(x,y,z);
            for axis in 0..3 {
                if !c.face_valid([x,y,z],axis) || !c.face_free(x,y,z,axis) {self.work.velocity[i][axis]=0.;continue;}
                let p=c.face_pos(x,y,z,axis);let current=velocity(&c,&self.frame.velocity,p);
                let mid=c.trace(p,p-current*dt*0.5);let back=c.trace(p,p-velocity(&c,&self.frame.velocity,mid)*dt);
                let value=sample_face(&c,&self.frame.velocity,back,axis)*(-dt*0.08).exp();
                let heat=sample_scalar(&c,&self.frame.heat,p);let rho=sample_scalar(&c,&self.frame.density,p);
                let wobble=Vec3::new((p.y*6.7+time+c.seed as f32*0.13).sin(),(p.z*5.3-time*0.7).sin(),(p.x*7.1+time*0.8).sin())*c.turbulence*0.8;
                let force=c.fields.acceleration(p,current)*c.influence+Vec3::Y*(heat*c.buoyancy-rho*0.03)+wobble;
                let (a,b)=c.neighbors(x,y,z,axis);let weight=(a.map_or(0.,|j|self.work.injection[j])+b.map_or(0.,|j|self.work.injection[j]))*0.5;
                let radial=(a.map_or(Vec3::ZERO,|j|self.work.source_direction[j])+b.map_or(Vec3::ZERO,|j|self.work.source_direction[j]))*0.5;
                let push=c.source_velocity+if c.burst {radial*3.}else{Vec3::ZERO};
                self.work.velocity[i][axis]=(value+dt*(force[axis]+push[axis]*weight*strength)).clamp(-cap,cap);
            }
        }}}
        std::mem::swap(&mut self.frame.velocity,&mut self.work.velocity);self.project(dt,cancel)?;
        for z in 0..n {check(cancel)?;for y in 0..n {for x in 0..n {let i=(z*n+y)*n+x;
            if c.solid[i] {self.work.density[i]=0.;self.work.heat[i]=0.;continue;}
            let p=c.pos(x,y,z);let v=velocity(&c,&self.frame.velocity,p);let mid=c.trace(p,p-v*dt*0.5);
            let back=c.trace(p,p-velocity(&c,&self.frame.velocity,mid)*dt);
            let source=self.work.injection[i]*strength*dt;
            self.work.density[i]=(sample_scalar(&c,&self.frame.density,back)*(-c.dissipation*dt).exp()+source).clamp(0.,1.);
            self.work.heat[i]=(sample_scalar(&c,&self.frame.heat,back)*(-dt*(0.4+c.dissipation)).exp()+source*c.temperature).clamp(0.,1.);
        }}}
        std::mem::swap(&mut self.frame.density,&mut self.work.density);std::mem::swap(&mut self.frame.heat,&mut self.work.heat);
        if self.frame.velocity.iter().any(|v|!v.is_finite()) || self.frame.density.iter().chain(&self.frame.heat).any(|v|!v.is_finite()) {return Err("Переполнение расчёта газа FX".into());}
        self.frame.tick+=1;check(cancel)
    }
    pub fn texture(&self,edge:usize,cancel:&AtomicBool)->Result<Vec<u8>,String> {
        if !(8..=32).contains(&edge) {return Err("Некорректный размер текстуры газа".into());}
        let mut bytes=Vec::with_capacity(edge.pow(3)*4);let c=&self.config;
        for z in 0..edge {check(cancel)?;for y in 0..edge {for x in 0..edge {
            let uv=(Vec3::new(x as f32,y as f32,z as f32)+Vec3::splat(0.5))/edge as f32;let p=c.low+uv*(c.high-c.low);
            let scale=if c.pyro.is_some(){2.}else{1.};
            for (v,gain) in [(&self.frame.density,1.),(&self.frame.heat,1./scale),(&self.frame.flame,1.),(&self.frame.fuel,0.25)] {bytes.push((sample_scalar(c,v,p)*gain).clamp(0.,1.).mul_add(255.,0.5) as u8);}
        }}}Ok(bytes)
    }
}
