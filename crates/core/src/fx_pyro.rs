//! Authored source, combustion and optical properties. Old files have no Pyro block.
use crate::{CoreError,FxKind,FxSettings};
use serde::{Deserialize,Serialize};
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub enum FxEmission {#[default] Continuous,Burst,Fill}
impl FxEmission {
    pub const ALL:[Self;3]=[Self::Continuous,Self::Burst,Self::Fill];
    pub fn label(self)->&'static str {match self {Self::Continuous=>"Постоянная подача",Self::Burst=>"Короткий импульс",Self::Fill=>"Объём в начале"}}
    pub fn index(self)->u8 {match self {Self::Continuous=>0,Self::Burst=>1,Self::Fill=>2}}
}
#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default)]
pub struct FxPyro {
    pub mode:FxEmission,pub source_rate:f32,pub source_temperature:f32,pub fuel:f32,
    pub ignition:f32,pub burn_rate:f32,pub heat_release:f32,pub smoke_yield:f32,
    pub cooling:f32,pub expansion:f32,pub impulse:f32,pub burst_seconds:f32,pub vorticity:f32,
    pub fog_falloff:f32,pub anisotropy:f32,pub self_shadow:bool,pub temperature_color:bool,pub smoke_color:[f32;3],
    /// Optional art-directed mushroom burst. Old scene files deserialize as false.
    pub cinematic_blast:bool,
}
impl Default for FxPyro {fn default()->Self {Self::preset(FxKind::Fire)}}
impl FxPyro {
    pub fn preset(kind:FxKind)->Self {
        let mut p=Self{mode:FxEmission::Continuous,source_rate:3.,source_temperature:1100.,fuel:1.5,ignition:650.,burn_rate:3.,heat_release:1400.,smoke_yield:0.45,
            cooling:0.8,expansion:0.8,impulse:1.,burst_seconds:0.25,vorticity:2.,fog_falloff:2.5,anisotropy:0.15,self_shadow:true,temperature_color:true,smoke_color:[0.16,0.17,0.18],cinematic_blast:false};
        match kind {
            FxKind::Fire=>{},
            FxKind::Smoke=>{p.fuel=0.;p.source_temperature=500.;p.cooling=0.35;p.expansion=0.;p.smoke_color=[0.3,0.32,0.34];},
            FxKind::Fog=>{p.mode=FxEmission::Fill;p.fuel=0.;p.source_rate=0.65;p.source_temperature=293.;p.expansion=0.;p.impulse=0.;p.vorticity=0.2;p.anisotropy=0.5;p.smoke_color=[0.82,0.86,0.9];},
            FxKind::Steam=>{p.fuel=0.;p.source_temperature=420.;p.cooling=1.4;p.expansion=0.;p.vorticity=1.2;p.smoke_color=[0.9,0.94,0.98];},
        }p
    }
    pub fn validate(self)->Result<(),CoreError> {
        for (v,lo,hi) in [(self.source_rate,0.,12.),(self.source_temperature,293.,3000.),(self.fuel,0.,4.),(self.ignition,300.,1800.),(self.burn_rate,0.05,20.),
            (self.heat_release,0.,2500.),(self.smoke_yield,0.,3.),(self.cooling,0.,5.),(self.expansion,0.,20.),(self.impulse,0.,12.),(self.burst_seconds,0.02,5.),
            (self.vorticity,0.,8.),(self.fog_falloff,0.,8.),(self.anisotropy,-0.8,0.8)] {
            if !v.is_finite() || !(lo..=hi).contains(&v) {return Err(CoreError::Invalid("Некорректные параметры источника или горения FX".into()));}
        }
        if self.smoke_color.iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v)) {return Err(CoreError::Invalid("Цвет дыма FX: значения от 0 до 1".into()));}Ok(())
    }
}
impl FxSettings {
    pub fn modern(kind:FxKind)->Self {
        let mut f=Self::preset(kind);let p=FxPyro::preset(kind);f.pyro=Some(p);f.gas=true;f.speed=1.;f.duration=8.;f.flicker=0.1;
        f.color=if kind==FxKind::Fire {[1.;3]}else{p.smoke_color};
        match kind {FxKind::Fire=>{f.emission=15.;f.dissipation=0.12;},FxKind::Smoke=>{f.dissipation=0.06;f.duration=12.;},
            FxKind::Fog=>{f.height=0.5;f.gas_domain=1.5;f.buoyancy=0.;f.dissipation=0.;f.duration=20.;},FxKind::Steam=>{f.dissipation=0.9;f.buoyancy=1.5;}}
        f
    }
    pub fn upgrade_pyro(self)->Self {
        if self.pyro.is_some() {return self;}
        let mut next=self;let mut p=FxPyro::preset(self.kind);
        if self.burst {p.mode=FxEmission::Burst;p.expansion=8.;p.impulse=4.;}
        next.pyro=Some(p);next.gas=true;next
    }
    pub fn source_mode(self)->FxEmission {self.pyro.map_or(if self.burst {FxEmission::Burst}else{FxEmission::Continuous},|p|p.mode)}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creation_has_four_kinds_and_new_presets_always_validate() {
        assert_eq!(FxKind::ALL.len(),4);
        for kind in FxKind::ALL {let f=FxSettings::modern(kind);f.validate().unwrap();assert!(f.gas && f.pyro.is_some());
            for index in 0..3 {let styled=f.styled(index).unwrap();styled.validate().unwrap();assert_eq!(styled.kind,kind);assert!(styled.pyro.is_some());}
        }
        let fog=FxSettings::modern(FxKind::Fog);assert_eq!(fog.source_mode(),FxEmission::Fill);assert_eq!(fog.pyro.unwrap().source_temperature,293.);assert_eq!(fog.buoyancy,0.);
        let mut old_pyro=serde_json::to_value(FxPyro::preset(FxKind::Fire)).unwrap();
        old_pyro.as_object_mut().unwrap().remove("cinematic_blast");
        let restored:FxPyro=serde_json::from_value(old_pyro).unwrap();
        assert!(!restored.cinematic_blast);restored.validate().unwrap();
        let mut editor=crate::Editor::new(crate::Project::default());let before=editor.project.clone();let id=editor.create_pyro(FxKind::Fire).unwrap();
        assert!(editor.project.node(id).unwrap().fx.unwrap().pyro.is_some());assert_eq!(editor.undo_len(),1);editor.undo();assert_eq!(editor.project,before);
    }
    #[test]
    fn legacy_settings_opt_in_without_changing_links_or_timing() {
        let old:FxSettings=serde_json::from_str(r#"{"kind":"Smoke","source":42,"start":2.5,"seed":9,"burst":true}"#).unwrap();
        assert!(old.pyro.is_none());old.validate().unwrap();let new=old.upgrade_pyro();new.validate().unwrap();
        assert_eq!(new.source,old.source);assert_eq!(new.start,old.start);assert_eq!(new.seed,old.seed);assert_eq!(new.color,old.color);
        assert_eq!(new.source_mode(),FxEmission::Burst);assert_eq!(new.upgrade_pyro(),new);
        let json=serde_json::to_string(&new).unwrap();assert_eq!(serde_json::from_str::<FxSettings>(&json).unwrap(),new);
    }
    #[test]
    fn invalid_combustion_and_inconsistent_modes_are_rejected() {
        let good=FxSettings::modern(FxKind::Fire);
        let mut f=good;f.pyro.as_mut().unwrap().source_temperature=f32::NAN;assert!(f.validate().is_err());
        f=good;f.pyro.as_mut().unwrap().burn_rate=0.;assert!(f.validate().is_err());
        f=good;f.pyro.as_mut().unwrap().smoke_color[0]=2.;assert!(f.validate().is_err());
        f=good;f.pyro.as_mut().unwrap().anisotropy=1.;assert!(f.validate().is_err());
        f=good;f.gas=false;assert!(f.validate().is_err());f=good;f.burst=true;assert!(f.validate().is_err());
    }
}
