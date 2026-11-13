//! Time profiles and quality presets shared by preview, export and persistence.
use crate::{CoreError,FxKind,FxSettings};
use serde::{Deserialize,Serialize};
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub enum FxQuality {Fast,#[default] Balanced,Fine}
impl FxQuality {
    pub const ALL:[Self;3]=[Self::Fast,Self::Balanced,Self::Fine];
    pub fn label(self)->&'static str {match self {Self::Fast=>"Быстро",Self::Balanced=>"Баланс",Self::Fine=>"Детально"}}
    pub fn grid(self)->usize {match self {Self::Fast=>16,Self::Balanced=>24,Self::Fine=>32}}
    pub fn iterations(self)->usize {match self {Self::Fast=>12,Self::Balanced=>20,Self::Fine=>32}}
    pub fn ray_steps(self,export:bool)->u32 {match (self,export) {(Self::Fast,false)=>16,(Self::Fast,true)=>24,(Self::Balanced,false)=>32,(Self::Balanced,true)=>48,(Self::Fine,false)=>48,(Self::Fine,true)=>80}}
}
#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default)]
pub struct FxKey {pub at:f32,pub size:f32,pub density:f32,pub emission:f32,pub color:[f32;3]}
impl Default for FxKey {fn default()->Self {Self {at:0.,size:1.,density:1.,emission:1.,color:[1.;3]}}}
#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default)]
pub struct FxEnvelope {pub enabled:bool,pub smooth:bool,pub keys:[FxKey;4]}
impl Default for FxEnvelope {fn default()->Self {Self {enabled:false,smooth:true,keys:[0.,0.3,0.7,1.].map(|at|FxKey{at,..Default::default()})}}}
impl FxEnvelope {
    pub fn sample(self,phase:f32)->FxKey {
        if !self.enabled {return FxKey::default();}
        let t=if phase.is_finite(){phase.clamp(0.,1.)}else{0.};
        let end=self.keys.iter().position(|k|k.at>=t).unwrap_or(3).max(1);let a=self.keys[end-1];let b=self.keys[end];
        let mut u=((t-a.at)/(b.at-a.at).max(0.0001)).clamp(0.,1.);if self.smooth {u=u*u*(3.-2.*u);}
        let mix=|a:f32,b:f32|a+(b-a)*u;
        FxKey {at:t,size:mix(a.size,b.size),density:mix(a.density,b.density),emission:mix(a.emission,b.emission),color:std::array::from_fn(|i|mix(a.color[i],b.color[i]))}
    }
    pub fn validate(self)->Result<(),CoreError> {
        if self.keys[0].at!=0. || self.keys[3].at!=1. || self.keys.windows(2).any(|p|p[1].at-p[0].at<0.0099)
            || self.keys.iter().any(|k| !k.at.is_finite() || !(0. ..=1.).contains(&k.at)
                || !k.size.is_finite() || !(0. ..=1.).contains(&k.size)
                || !k.density.is_finite() || !(0. ..=2.).contains(&k.density)
                || !k.emission.is_finite() || !(0. ..=2.).contains(&k.emission)
                || k.color.iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v))) {
            return Err(CoreError::Invalid("FX: точки времени должны идти от 0 до 100%, размер 0–1, плотность/свечение 0–2".into()));
        }
        Ok(())
    }
}
impl FxSettings {
    pub fn preset_labels(kind:FxKind)->Vec<&'static str> {match kind {
        FxKind::Fire=>vec!["Костёр","Свеча","Огненный взрыв","Кинематографичный взрыв"],
        FxKind::Smoke=>vec!["Густой дым","Облако дыма","Пылевой выброс"],
        FxKind::Fog=>vec!["Низкий туман","Облачный слой","Сгусток тумана"],
        FxKind::Steam=>vec!["Мягкий пар","Струя пара","Выброс пара"],
    }}
    pub fn styled(self,index:u8)->Option<Self> {
        if index>2 && !(index==3 && self.kind==FxKind::Fire) {return None;}
        // The cinematic burst always uses the combustion solver, including when
        // invoked on a legacy procedural Fire object. Other presets stay unchanged.
        let mut f=if self.pyro.is_some() || index==3 {Self::modern(self.kind)}else{Self::preset(self.kind)};
        f.gas=true;f.duration=10.;f.speed=1.;f.gas_domain=2.;
        match (self.kind,index) {
            (FxKind::Fire,0)=>{f.turbulence=0.65;f.density=7.;f.dissipation=0.5;},
            (FxKind::Fire,1)=>{f.width=0.3;f.turbulence=0.08;f.smoke=0.08;f.emission=4.;f.buoyancy=0.7;},
            (FxKind::Fire,2)=>{f.burst=true;f.duration=2.5;f.emission=12.;f.density=14.;f.turbulence=0.8;},
            (FxKind::Fire,3)=>{
                // Large billowing crown above a dusty radial blast. One bounded
                // 3-D solver powers both viewport and export; no extra FX slot.
                f.burst=true;f.cloud=true;f.duration=4.5;f.gas_domain=3.6;
                f.source_radius=0.32;f.width=1.;f.height=1.;
                f.density=15.;f.emission=20.;f.turbulence=0.8;f.detail=9.;
                f.dissipation=0.09;f.buoyancy=1.7;f.softness=0.35;
                f.envelope=FxEnvelope{enabled:true,smooth:true,keys:[
                    FxKey{at:0.,size:1.,density:1.,emission:1.4,color:[1.;3]},
                    FxKey{at:0.18,size:1.,density:1.,emission:1.2,color:[1.;3]},
                    FxKey{at:0.58,size:1.,density:1.1,emission:0.5,color:[1.;3]},
                    FxKey{at:1.,size:1.,density:0.45,emission:0.,color:[1.;3]},
                ]};
            },
            (FxKind::Smoke,0)=>{f.density=12.;f.dissipation=0.08;f.duration=12.;},
            (FxKind::Smoke,1)=>{f.cloud=true;f.color=[0.65,0.68,0.72];f.buoyancy=0.25;f.density=6.;},
            (FxKind::Smoke,_)=>{f.burst=true;f.duration=4.;f.color=[0.45,0.3,0.18];f.buoyancy=0.35;f.density=9.;},
            (FxKind::Fog,0)=>{f.height=0.35;f.buoyancy=0.05;f.density=3.;f.dissipation=0.05;f.duration=20.;},
            (FxKind::Fog,1)=>{f.cloud=true;f.buoyancy=0.1;f.color=[0.8,0.85,0.9];f.density=4.;},
            (FxKind::Fog,_)=>{f.cloud=true;f.burst=true;f.duration=6.;f.density=7.;f.buoyancy=0.1;},
            (FxKind::Steam,0)=>{f.dissipation=0.65;f.density=4.;},
            (FxKind::Steam,1)=>{f.width=0.3;f.buoyancy=2.;f.turbulence=0.25;f.density=7.;},
            (FxKind::Steam,_)=>{f.burst=true;f.duration=3.;f.density=8.;f.dissipation=0.4;},
            _=>return None,
        }
        // Presets change appearance, preserving object links, timing origin and quality.
        f.enabled=self.enabled;f.seed=self.seed;f.source=self.source;f.source_radius=if index==3 {self.source_radius.max(0.32)}else{self.source_radius};
        f.use_fields=self.use_fields;f.field_influence=self.field_influence;f.mesh_collision=self.mesh_collision;
        f.gas_hollow=self.gas_hollow;f.start=self.start;f.quality=self.quality;
        if let Some(p)=&mut f.pyro {
            if f.burst {p.mode=crate::FxEmission::Burst;p.expansion=if f.kind==FxKind::Fire {12.}else{6.};p.impulse=5.;p.source_rate=4.;p.burst_seconds=0.2;}
            if f.kind==FxKind::Fire {f.color=[1.;3];if index==1 {p.source_rate=1.2;p.fuel=0.7;p.smoke_yield=0.02;p.vorticity=0.3;p.expansion=0.2;}if index==2 {p.fuel=3.;p.heat_release=1900.;p.smoke_yield=0.8;}}
            if f.kind==FxKind::Fire && index==3 {
                p.cinematic_blast=true;p.mode=crate::FxEmission::Burst;
                p.source_rate=7.;p.source_temperature=2450.;p.fuel=3.6;
                p.ignition=540.;p.burn_rate=7.5;p.heat_release=2200.;
                p.smoke_yield=1.4;p.cooling=0.55;p.expansion=15.;
                p.impulse=9.;p.burst_seconds=0.24;p.vorticity=3.8;
                p.smoke_color=[0.10,0.095,0.09];p.anisotropy=0.25;
            }
            if f.kind==FxKind::Fog {if index!=2 {p.mode=crate::FxEmission::Fill;}p.fog_falloff=if index==1 {0.5}else{3.};}
        }
        Some(f)
    }
    pub fn local_time(self,time:f64)->f32 {
        if !time.is_finite() || self.speed<=0. {return 0.;}
        let elapsed=(time-self.start).max(0.);let speed=self.speed as f64;let duration=self.duration.max(0.1) as f64;
        (if self.looped {elapsed.rem_euclid(duration/speed)*speed}else{(elapsed*speed).min(duration)}) as f32
    }
    pub fn envelope_at(self,time:f64)->FxKey {self.envelope.sample(self.local_time(time)/self.duration.max(0.1))}
    pub fn playback_finished(self,time:f64)->bool {
        (self.burst || self.gas || self.envelope.enabled) && !self.looped && self.speed>0. && time>=self.start+self.duration as f64/self.speed as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_settings_keep_procedural_appearance_and_quality_is_bounded() {
        let f:FxSettings=serde_json::from_str(r#"{"kind":"Smoke","density":4.0}"#).unwrap();
        assert!(!f.gas);assert!(!f.envelope.enabled);assert_eq!(f.envelope_at(100.),FxKey::default());f.validate().unwrap();
        for q in FxQuality::ALL {assert!((16..=32).contains(&q.grid()));assert!(q.ray_steps(true)<=80);assert!(q.iterations()<=32);}
    }
    #[test]
    fn envelopes_interpolate_every_channel_without_overshoot_and_reject_crossed_points() {
        let mut e=FxEnvelope{enabled:true,..Default::default()};
        for (i,k) in e.keys.iter_mut().enumerate() {let v=if i%2==0 {0.}else{1.};k.size=v;k.density=2.*v;k.emission=2.*(1.-v);k.color=[v,1.-v,0.5];}
        e.validate().unwrap();assert_eq!(e.sample(0.),e.keys[0]);assert_eq!(e.sample(1.),e.keys[3]);
        for smooth in [false,true] {e.smooth=smooth;for i in 0..=100 {let k=e.sample(i as f32/100.);assert!((0. ..=1.).contains(&k.size));assert!((0. ..=2.).contains(&k.density));assert!((0. ..=2.).contains(&k.emission));assert!((k.color[0]+k.color[1]-1.).abs()<1e-6);}}
        e.keys[2].at=e.keys[1].at;assert!(e.validate().is_err());e=FxEnvelope::default();e.keys[0].at=0.1;assert!(e.validate().is_err());e=FxEnvelope::default();e.keys[1].color[0]=f32::NAN;assert!(e.validate().is_err());
    }
    #[test]
    fn thirteen_presets_keep_four_kinds_and_preserve_scene_links() {
        assert_eq!(FxKind::ALL.len(),4);
        for kind in FxKind::ALL {for index in 0..3 {
            let f=FxSettings {source:Some(42),start:12.,mesh_collision:true,gas_hollow:true,seed:19,quality:FxQuality::Fast,..FxSettings::preset(kind)};
            let p=f.styled(index).unwrap();p.validate().unwrap();assert_eq!(p.kind,kind);assert!(p.gas);assert_eq!(p.source,f.source);
            assert_eq!(p.start,f.start);assert_eq!(p.quality,f.quality);assert_eq!(p.seed,f.seed);assert!(p.mesh_collision && p.gas_hollow);
        }}
        assert_eq!(FxSettings::preset_labels(FxKind::Fire).len(),4);
        for kind in [FxKind::Smoke,FxKind::Fog,FxKind::Steam] {
            assert_eq!(FxSettings::preset_labels(kind).len(),3);
            assert!(FxSettings::preset(kind).styled(3).is_none());
        }
        let original=FxSettings::preset(FxKind::Fire);
        let blast=original.styled(3).unwrap();blast.validate().unwrap();
        assert!(blast.gas && blast.burst && blast.cloud && blast.envelope.enabled);
        assert!(blast.pyro.unwrap().cinematic_blast);
        assert_eq!(blast.source_mode(),crate::FxEmission::Burst);
        assert_eq!(original,FxSettings::preset(FxKind::Fire));
        assert!(blast.styled(3).unwrap().pyro.unwrap().cinematic_blast);
        let saved=serde_json::to_string(&blast).unwrap();
        assert_eq!(serde_json::from_str::<FxSettings>(&saved).unwrap(),blast);
    }
    #[test]
    fn gas_holds_last_frame_and_looping_has_finite_relative_time() {
        let mut f=FxSettings {gas:true,start:2.,duration:4.,speed:2.,..Default::default()};
        assert!(!f.visible_at(1.));assert!(f.visible_at(20.));assert!(f.playback_finished(4.));assert_eq!(f.local_time(20.),4.);
        f.looped=true;assert_eq!(f.local_time(4.),0.);assert!(!f.playback_finished(100.));assert!(f.local_time(f64::MAX).is_finite());
        f.gas=false;f.envelope.enabled=true;assert_eq!(f.sample_time(2.5),f.sample_time(4.5));
        f.looped=false;assert!(f.playback_finished(20.));assert_eq!(f.sample_time(20.),4.);
        f.speed=0.;assert_eq!(f.local_time(f64::MAX),0.);
    }
}
