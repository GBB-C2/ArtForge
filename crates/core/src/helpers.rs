//! Non-rendering scene helpers and bounded simulation field parameters.
use crate::{CoreError,Editor,Node,Project};
use serde::{Deserialize,Serialize};
#[derive(Debug,Default,Clone,Copy,Serialize,Deserialize,PartialEq,Eq)]
pub enum EmptyShape {#[default] Axes,Arrows,Circle,Cube,Sphere,Cone}
impl EmptyShape {
    pub const ALL:[Self;6]=[Self::Axes,Self::Arrows,Self::Circle,Self::Cube,Self::Sphere,Self::Cone];
    pub fn label(self)->&'static str {match self {Self::Axes=>"Оси",Self::Arrows=>"Стрелки",Self::Circle=>"Окружность",Self::Cube=>"Куб",Self::Sphere=>"Сфера",Self::Cone=>"Конус"}}
}
pub fn default_empty_size()->f32 {1.}
#[derive(Debug,Default,Clone,Copy,Serialize,Deserialize,PartialEq,Eq)]
pub enum ForceFieldKind {#[default] Force,Wind,Vortex,Magnet}
impl ForceFieldKind {
    pub const ALL:[Self;4]=[Self::Force,Self::Wind,Self::Vortex,Self::Magnet];
    pub fn label(self)->&'static str {match self {Self::Force=>"Сила",Self::Wind=>"Ветер",Self::Vortex=>"Вихрь",Self::Magnet=>"Магнит"}}
}
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct ForceFieldSettings {
    pub kind:ForceFieldKind,pub enabled:bool,pub strength:f32,pub radius:f32,pub falloff:f32,pub damping:f32,
    /// Liquid, deformable bodies, fractured shards.
    pub targets:[bool;3],
    pub fx:bool,
}
impl Default for ForceFieldSettings {fn default()->Self {Self{kind:ForceFieldKind::Force,enabled:true,strength:10.,radius:3.,falloff:2.,damping:0.5,targets:[true;3],fx:true}}}
impl ForceFieldSettings {
    pub fn validate(self)->Result<(),CoreError> {
        if !self.strength.is_finite() || self.strength.abs()>1000. || !self.radius.is_finite() || !(0.01..=10000.).contains(&self.radius)
            || !self.falloff.is_finite() || !(0. ..=8.).contains(&self.falloff) || !self.damping.is_finite() || !(0. ..=20.).contains(&self.damping) {
            return Err(CoreError::Invalid("Поле: сила −1000…1000, радиус 0,01…10000 м, спад 0…8, торможение 0…20".into()));
        } Ok(())
    }
}
impl Editor {
    pub fn create_empty_shape(&mut self,shape:EmptyShape)->Result<u64,CoreError> {
        if self.has_staged() {return Err(CoreError::Invalid("Сначала запишите или отмените временную позу".into()));}
        let id=self.project.next_id();let mut node=Node::new(id,format!("Пустышка · {}",shape.label()));node.empty=true;node.empty_shape=shape;
        self.transact("Создать пустышку",|p|p.nodes.push(node))?;Ok(id)
    }
    pub fn create_force_field(&mut self,kind:ForceFieldKind)->Result<u64,CoreError> {
        if self.has_staged() {return Err(CoreError::Invalid("Сначала запишите или отмените временную позу".into()));}
        let id=self.project.next_id();let mut node=Node::new(id,kind.label());node.empty=true;
        node.force_field=Some(ForceFieldSettings{kind,..Default::default()});
        self.transact("Создать силовое поле",|p|p.nodes.push(node))?;Ok(id)
    }
}
impl Project {
    pub(crate) fn validate_helpers(&self)->Result<(),CoreError> {
        if self.nodes.iter().filter(|n|n.force_field.is_some()).count()>64 {return Err(CoreError::Invalid("Не более 64 силовых полей в сцене".into()));}
        for n in &self.nodes {
            if !n.empty_size.is_finite() || !(0.01..=1000.).contains(&n.empty_size) {return Err(CoreError::Invalid("Размер пустышки: 0,01…1000 м".into()));}
            if let Some(s)=n.force_field {s.validate()?;if !n.empty || n.physics.is_some() || n.fluid.is_some() || n.destruction.is_some() {return Err(CoreError::Invalid("Силовое поле должно быть отдельным объектом без меша".into()));}}
        } Ok(())
    }
}
