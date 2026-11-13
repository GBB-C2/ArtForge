//! Editable volume settings; runtime simulation frames remain outside the document.
use crate::{FxQuality,FxEnvelope,CoreError, Editor, EmptyShape, Node, NodeId, Project, Vec3};
use serde::{Deserialize, Serialize};
pub const MAX_FX: usize = 8;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxKind { #[default] Fire, Smoke, Fog, Steam }
impl FxKind {
    pub const ALL:[Self;4]=[Self::Fire,Self::Smoke,Self::Fog,Self::Steam];
    pub fn label(self)->&'static str {match self {Self::Fire=>"Огонь",Self::Smoke=>"Дым",Self::Fog=>"Туман",Self::Steam=>"Пар"}}
    pub fn index(self)->u32 {match self {Self::Fire=>0,Self::Smoke=>1,Self::Fog=>2,Self::Steam=>3}}
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxSettings {
    pub kind:FxKind,pub enabled:bool,pub density:f32,pub emission:f32,
    pub detail:f32,pub turbulence:f32,pub speed:f32,pub seed:u32,pub color:[f32;3],
    /// Shape inside the transformed domain, in fractions of its width/height.
    pub width:f32,pub height:f32,pub softness:f32,
    pub drift:[f32;2],pub smoke:f32,pub flicker:f32,
    /// Old scenes must not gain a new light merely by being loaded.
    #[serde(default)]
    pub illuminate:bool,
    pub source:Option<NodeId>,pub source_radius:f32,
    pub use_fields:bool,pub field_influence:f32,
    #[serde(default)]
    pub mesh_collision:bool,
    /// Shape and emission modes of the four base FX kinds.
    pub cloud:bool,pub burst:bool,pub start:f64,pub duration:f32,pub looped:bool,
    pub gas:bool,pub gas_hollow:bool,pub gas_domain:f32,pub dissipation:f32,pub buoyancy:f32,
    pub quality:FxQuality,pub envelope:FxEnvelope,
    #[serde(default)]
    pub pyro:Option<crate::FxPyro>,
}
impl Default for FxSettings {fn default()->Self {Self::preset(FxKind::Fire)}}
impl FxSettings {
    pub fn preset(kind:FxKind)->Self {
        let (density,emission,detail,turbulence,speed,color)=match kind {
            FxKind::Fire=>(8.,6.,7.,0.45,1.,[1.,0.16,0.015]),
            FxKind::Smoke=>(5.,0.,4.,0.6,0.45,[0.16,0.18,0.21]),
            FxKind::Fog=>(1.4,0.,3.,0.25,0.08,[0.65,0.72,0.78]),
            FxKind::Steam=>(2.7,0.,5.,0.65,0.4,[0.85,0.9,0.96]),
        };
        Self {kind,enabled:true,density,emission,detail,turbulence,speed,color,seed:1,
            width:1.,height:1.,softness:0.45,drift:[0.;2],smoke:0.35,flicker:0.25,
            illuminate:kind==FxKind::Fire,source:None,source_radius:0.25,
            use_fields:true,field_influence:1.,mesh_collision:false,cloud:false,burst:false,start:0.,duration:3.,looped:false,gas:false,gas_hollow:false,gas_domain:2.,dissipation:0.2,buoyancy:1.,quality:FxQuality::Balanced,envelope:FxEnvelope::default(),pyro:None}
    }
    pub fn sample_time(self,time:f64)->f32 {
        if !time.is_finite() {return 0.;}
        if self.burst || self.gas || self.envelope.enabled {self.local_time(time)} else {(time.max(0.).rem_euclid(4096.)*self.speed as f64) as f32}
    }

    pub fn finished(self,time:f64)->bool {
        self.burst && !self.looped && self.speed>0. && time>=self.start+self.duration as f64/self.speed as f64
    }
    pub fn visible_at(self,time:f64)->bool {
        !(self.burst || self.gas || self.envelope.enabled) || (time.is_finite() && time>=self.start && !self.finished(time))
    }
    /// The burst flash and continuous flame share a clock with their optional scene light.
    pub fn flame_pulse(self,time:f64)->f32 {
        let t=self.sample_time(time);let seed=(self.seed%1024) as f32*0.618;
        let pulse=(1.+self.flicker*((t*6.7+seed).sin()*0.55+(t*11.3+seed*0.7).sin()*0.3
            +(t*17.1+seed*1.3).sin()*0.15)).clamp(0.1,2.);
        if self.burst && self.pyro.is_none() {
            if !self.visible_at(time) {return 0.;}
            let phase=(t/self.duration.max(0.1)).clamp(0.,1.);
            pulse*(phase/0.04).min(1.)*(-phase*6.).exp()*(1.-phase)
        } else {pulse}
    }
    pub fn validate(self)->Result<(),CoreError> {
        for (value,min,max) in [(self.density,0.,30.),(self.emission,0.,30.),(self.detail,1.,16.),
            (self.turbulence,0.,1.),(self.speed,0.,5.),(self.width,0.1,1.),(self.height,0.1,1.),
            (self.softness,0.02,1.),(self.smoke,0.,1.),(self.flicker,0.,1.),
            (self.drift[0],-0.5,0.5),(self.drift[1],-0.5,0.5),
            (self.source_radius,0.02,2.),(self.field_influence,0.,2.),(self.duration,0.1,30.)] {
            if !value.is_finite() || !(min..=max).contains(&value) {return Err(CoreError::Invalid("Некорректные параметры FX".into()));}
        }
        self.envelope.validate()?;
        if let Some(p)=self.pyro {p.validate()?;if !self.gas || self.burst!=(p.mode==crate::FxEmission::Burst) {return Err(CoreError::Invalid("FX: режим источника не согласован с расчётом".into()));}}
        for (value,lo,hi) in [(self.gas_domain,1.,4.),(self.dissipation,0.,3.),(self.buoyancy,0.,4.)] {
            if !value.is_finite() || !(lo..=hi).contains(&value) {return Err(CoreError::Invalid("Некорректные параметры газа FX".into()));}
        }
        if !self.start.is_finite() || !(0. ..=86400.).contains(&self.start) {return Err(CoreError::Invalid("Начало FX: от 0 до 86400 секунд".into()));}
        if self.seed>999999 || self.color.iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v)) {
            return Err(CoreError::Invalid("FX: цвет 0–1, случайность 0–999999".into()));
        }
        Ok(())
    }
}
impl Editor {
    pub fn set_fx_source(&mut self,owner:NodeId,source:Option<NodeId>)->Result<(),CoreError> {
        if self.has_staged() {return Err(CoreError::UncommittedPose);}
        if self.project.node_locked(owner) {return Err(CoreError::Invalid("FX заблокирован".into()));}
        let node=self.project.node(owner).ok_or(CoreError::MissingNode(owner))?;
        let settings=node.fx.ok_or_else(||CoreError::Invalid("Выберите FX".into()))?;
        if settings.source==source {return Ok(());}
        if source.is_some_and(|id|!self.project.fx_source_allowed(owner,id)) {
            return Err(CoreError::Invalid("Источник FX: нужен другой меш или кривая без циклической связи".into()));
        }
        let sampled=self.evaluated_base()?;let mut candidate=self.project.clone();
        if source.is_none() {
            candidate.detach_fx_source(owner,&sampled.global,sampled.local[&owner])?;
        } else {
            let n=candidate.node_mut(owner).unwrap();n.curve_binding=None;
            n.transform.translation=Vec3::ZERO;n.transform.rotation=crate::Quat::IDENTITY;n.rest_transform=Some(n.transform);
            n.fx.as_mut().unwrap().source=source;
            if let Some(entries)=&mut n.collision_targets.fx {entries.retain(|e|Some(e.node)!=source);}
        }
        self.transact("Источник FX",|p|*p=candidate)
    }
    pub fn create_fx(&mut self,kind:FxKind)->Result<u64,CoreError> {
        self.create_fx_settings(FxSettings::preset(kind))
    }
    pub fn create_pyro(&mut self,kind:FxKind)->Result<u64,CoreError> {
        self.create_pyro_at(kind,0.)
    }
    pub fn create_pyro_at(&mut self,kind:FxKind,start:f64)->Result<u64,CoreError> {
        let mut settings=FxSettings::modern(kind);settings.start=start;settings.validate()?;self.create_fx_settings(settings)
    }
    fn create_fx_settings(&mut self,settings:FxSettings)->Result<u64,CoreError> {
        let kind=settings.kind;
        if self.has_staged() {return Err(CoreError::UncommittedPose);}
        if self.project.nodes.iter().filter(|n|n.fx.is_some()).count()>=MAX_FX {return Err(CoreError::Invalid("В сцене поддерживается до 8 объёмных FX".into()));}
        let id=self.project.next_id();let mut node=Node::new(id,format!("FX · {}",kind.label()));
        node.empty=true;node.empty_shape=EmptyShape::Cube;
        node.fx=Some(FxSettings {seed:(id%999999).max(1) as u32,..settings});
        node.collision_targets.fx=Some(Vec::new());
        node.transform.scale=match kind {FxKind::Fire=>Vec3::new(1.,2.,1.),FxKind::Smoke=>Vec3::new(1.8,3.,1.8),
            FxKind::Fog=>Vec3::new(5.,1.5,5.),FxKind::Steam=>Vec3::new(1.5,2.5,1.5)};
        self.transact("Создать FX",|p|p.nodes.push(node))?;Ok(id)
    }
}
impl Project {
    pub fn fx_source_allowed(&self,owner:NodeId,target:NodeId)->bool {
        if owner==target || self.node(owner).is_none_or(|n|n.fx.is_none())
            || self.node(target).is_none_or(|n|n.mesh.is_none() && n.curve.is_none()) {return false;}
        let mut stack=vec![target];let mut seen=std::collections::BTreeSet::new();
        while let Some(id)=stack.pop() {
            if id==owner {return false;}if !seen.insert(id) {continue;}
            if let Some(n)=self.node(id) {stack.extend([n.parent,n.curve_binding.map(|b|b.curve),n.fx.and_then(|f|f.source)].into_iter().flatten());}
        }
        true
    }
    pub(crate) fn validate_fx(&self)->Result<(),CoreError> {
        if self.nodes.iter().filter(|n|n.fx.is_some()).count()>MAX_FX {return Err(CoreError::Invalid("В сцене больше 8 FX".into()));}
        for n in &self.nodes {if let Some(fx)=n.fx {
            fx.validate()?;
            if let Some(source)=fx.source {
                if source==n.id || self.node(source).is_none_or(|s|s.mesh.is_none() && s.curve.is_none()) || n.curve_binding.is_some() {
                    return Err(CoreError::Invalid("Источник FX: выберите другой меш или кривую; источник и путь используются отдельно".into()));
                }
            }
            if !n.empty || n.mesh.is_some() || n.camera.is_some() || n.is_joint || n.curve.is_some()
                || n.force_field.is_some() || n.physics.is_some() || n.fluid.is_some() || n.destruction.is_some() {
                return Err(CoreError::Invalid("FX должен быть отдельным объёмным объектом".into()));
            }
        }}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volume_creation_preserves_scene_and_has_one_undo() {
        let mut editor=Editor::new(Project::default());let before=editor.project.clone();
        let id=editor.create_fx(FxKind::Fire).unwrap();let node=editor.project.node(id).unwrap();
        assert_eq!(node.transform.translation,Vec3::ZERO);assert_eq!(node.fx.unwrap(),FxSettings::preset(FxKind::Fire));
        assert!(node.empty && node.mesh.is_none());assert_eq!(editor.project.meshes,before.meshes);
        editor.project.validate().unwrap();editor.undo();assert_eq!(editor.project,before);
    }
    #[test]
    fn all_volume_presets_validate_and_limit_rejection_is_atomic() {
        let mut editor=Editor::new(Project::default());
        for kind in FxKind::ALL.into_iter().cycle().take(MAX_FX) {editor.create_fx(kind).unwrap();}
        let before=editor.project.clone();assert!(editor.create_fx(FxKind::Fog).is_err());assert_eq!(editor.project,before);
        let id=editor.project.nodes[0].id;editor.project.node_mut(id).unwrap().fx.as_mut().unwrap().density=f32::NAN;
        assert!(editor.project.validate().is_err());
    }
    #[test]
    fn new_shape_values_are_validated_and_legacy_fx_do_not_gain_scene_light() {
        let preset=FxSettings::preset(FxKind::Fire);
        let mut old=serde_json::to_value(preset).unwrap();
        for name in ["width","height","softness","drift","smoke","flicker","illuminate"] {old.as_object_mut().unwrap().remove(name);}
        let loaded:FxSettings=serde_json::from_value(old).unwrap();
        assert_eq!(loaded,FxSettings {illuminate:false,..preset});loaded.validate().unwrap();
        for settings in [
            FxSettings {width:0.,..preset},FxSettings {height:f32::NAN,..preset},
            FxSettings {softness:f32::INFINITY,..preset},FxSettings {drift:[0.6,0.],..preset},
            FxSettings {smoke:-0.1,..preset},FxSettings {flicker:1.1,..preset},
        ] {assert!(settings.validate().is_err());}
    }
    #[test]
    fn fire_pulse_is_deterministic_bounded_and_freezes_with_zero_speed() {
        let settings=FxSettings {flicker:1.,..FxSettings::default()};
        let samples:Vec<_>=(0..100).map(|i|settings.flame_pulse(i as f64*0.03)).collect();
        assert!(samples.iter().all(|v|v.is_finite() && (0.1..=2.).contains(v)));
        assert!(samples.windows(2).any(|p|(p[0]-p[1]).abs()>0.01));
        assert_eq!(samples[20],settings.flame_pulse(20.0*0.03));
        let frozen=FxSettings {speed:0.,..settings};assert_eq!(frozen.flame_pulse(0.),frozen.flame_pulse(10.));
        let steady=FxSettings {flicker:0.,..settings};assert_eq!(steady.flame_pulse(9.),1.);
        assert!(settings.sample_time(1e30).is_finite());assert_eq!(settings.sample_time(f64::NAN),0.);
    }
    #[test]
    fn repeated_effect_creation_varies_seed_and_undo_preserves_existing_effect() {
        let mut editor=Editor::new(Project::default());let first=editor.create_fx(FxKind::Smoke).unwrap();
        let before=editor.project.clone();let second=editor.create_fx(FxKind::Smoke).unwrap();
        assert_ne!(editor.project.node(first).unwrap().fx.unwrap().seed,editor.project.node(second).unwrap().fx.unwrap().seed);
        editor.undo();assert_eq!(editor.project,before);
    }
    #[test]
    fn four_base_kinds_keep_identity_and_legacy_defaults_remain_continuous() {
        assert_eq!(FxKind::ALL,[FxKind::Fire,FxKind::Smoke,FxKind::Fog,FxKind::Steam]);
        for (index,kind) in FxKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.index(),index as u32);let before=FxSettings::preset(kind);
            let mut old=serde_json::to_value(before).unwrap();
            for key in ["cloud","burst","start","duration","looped"] {old.as_object_mut().unwrap().remove(key);}
            let after:FxSettings=serde_json::from_value(old).unwrap();assert_eq!(after,before);
            assert!(after.visible_at(1000.) && !after.finished(1000.));
        }
    }
    #[test]
    fn burst_clock_is_deterministic_bounded_and_repeatable_without_keyframes() {
        let f=FxSettings{burst:true,start:2.,duration:4.,speed:2.,..FxSettings::default()};
        assert!(!f.visible_at(1.9));assert_eq!(f.sample_time(1.),0.);assert_eq!(f.flame_pulse(1.),0.);
        assert_eq!(f.sample_time(2.5),1.);assert!(f.visible_at(2.5));
        assert!(f.finished(4.) && !f.visible_at(4.));assert_eq!(f.sample_time(100.),4.);assert_eq!(f.flame_pulse(4.),0.);
        let looped=FxSettings{looped:true,..f};assert!(!looped.finished(100.));
        assert_eq!(looped.sample_time(2.5),looped.sample_time(4.5));assert_eq!(looped.flame_pulse(2.5),looped.flame_pulse(4.5));
        assert!(looped.sample_time(f64::MAX).is_finite());assert_eq!(FxSettings{speed:0.,..looped}.sample_time(f64::MAX),0.);
        assert_eq!(looped.sample_time(f64::NAN),0.);assert!(!looped.visible_at(f64::NAN));
        for value in [FxSettings{duration:0.,..f},FxSettings{duration:f32::NAN,..f},FxSettings{start:-1.,..f},FxSettings{start:f64::INFINITY,..f}] {assert!(value.validate().is_err());}
    }

}
