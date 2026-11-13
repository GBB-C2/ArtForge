//! Bounded world-space accelerations, evaluated in each solver's own coordinates.
use glam::{Mat4,Vec3};
use serde::{Deserialize,Serialize};
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
pub struct Field {pub kind:u8,pub center:Vec3,pub axis:Vec3,pub strength:f32,pub radius:f32,pub falloff:f32,pub damping:f32}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
pub struct Fields {items:Vec<Field>,world:Mat4,inverse:Mat4}
impl Default for Fields {fn default()->Self {Self{items:vec![],world:Mat4::IDENTITY,inverse:Mat4::IDENTITY}}}
impl Fields {
    pub fn new(items:Vec<Field>,world:Mat4)->Result<Self,String> {
        if items.is_empty() {return Ok(Self::default());}
        if items.len()>64 || !world.is_finite() || world.determinant().abs()<1e-8 {return Err("Некорректное пространство силового поля".into());}
        for f in &items {if f.kind>3 || !f.center.is_finite() || !f.axis.is_finite() || (f.axis.length_squared()-1.).abs()>0.01
            || !f.strength.is_finite() || f.strength.abs()>1000. || !f.radius.is_finite() || f.radius<=0.
            || !f.falloff.is_finite() || !(0. ..=8.).contains(&f.falloff) || !f.damping.is_finite() || !(0. ..=20.).contains(&f.damping) {return Err("Некорректные параметры силового поля".into());}}
        let inverse=world.inverse();if !inverse.is_finite() {return Err("Некорректное пространство силового поля".into());}
        Ok(Self{items,world,inverse})
    }
    pub fn is_empty(&self)->bool {self.items.is_empty()}
    pub fn acceleration(&self,position:Vec3,velocity:Vec3)->Vec3 {
        if self.items.is_empty() {return Vec3::ZERO;}
        let p=self.world.transform_point3(position);let velocity=self.world.transform_vector3(velocity);let mut total=Vec3::ZERO;
        for f in &self.items {
            let offset=p-f.center;let distance=offset.length();if !distance.is_finite() || distance>=f.radius {continue;}
            let attenuation=(1.-distance/f.radius).max(0.).powf(f.falloff);
            let radial=offset.normalize_or_zero();
            let a=match f.kind {
                0=>radial*f.strength,
                1=>f.axis*f.strength,
                2=>f.axis.cross(offset-f.axis*offset.dot(f.axis)).normalize_or_zero()*f.strength,
                3=>-radial*f.strength-velocity*f.damping,
                _=>Vec3::ZERO,
            };total+=a*attenuation;
        }
        let a=self.inverse.transform_vector3(total);
        if a.is_finite() {a.clamp_length_max(1000.)} else {Vec3::ZERO}
    }
    pub fn acceleration_limit(&self,max_speed:f32)->f32 {
        if self.is_empty() {return 0.;}
        let norm=|m:Mat4|(m.x_axis.truncate().length_squared()+m.y_axis.truncate().length_squared()+m.z_axis.truncate().length_squared()).sqrt();
        let speed=max_speed*norm(self.world);
        (self.items.iter().map(|f|f.strength.abs()+if f.kind==3 {f.damping*speed}else{0.}).sum::<f32>()*norm(self.inverse)).min(1000.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn field(kind:u8)->Field {Field{kind,center:Vec3::ZERO,axis:Vec3::Z,strength:8.,radius:4.,falloff:0.,damping:0.5}}
    #[test]
    fn four_modes_have_distinct_directions_falloff_and_finite_centers() {
        for (kind,direction) in [(0,Vec3::X),(1,Vec3::Z),(2,Vec3::Y),(3,-Vec3::X)] {let f=Fields::new(vec![field(kind)],Mat4::IDENTITY).unwrap();assert!(f.acceleration(Vec3::X,Vec3::ZERO).dot(direction)>7.9);assert_eq!(f.acceleration(Vec3::X*4.,Vec3::ZERO),Vec3::ZERO);assert!(f.acceleration(Vec3::ZERO,Vec3::ZERO).is_finite());}
        let mut f=field(0);f.falloff=2.;let fields=Fields::new(vec![f.clone()],Mat4::IDENTITY).unwrap();assert!(fields.acceleration(Vec3::X,Vec3::ZERO).length()>fields.acceleration(Vec3::X*3.,Vec3::ZERO).length());
        f.strength=-8.;assert!(Fields::new(vec![f],Mat4::IDENTITY).unwrap().acceleration(Vec3::X,Vec3::ZERO).x<0.);
        let magnet=Fields::new(vec![field(3)],Mat4::IDENTITY).unwrap();assert!(magnet.acceleration(Vec3::X,Vec3::Y).y<0.);
    }
    #[test]
    fn solver_coordinates_preserve_world_direction_and_validation_bounds_work() {
        let mut f=field(1);f.center=Vec3::new(3.,2.,1.);let world=Mat4::from_scale_rotation_translation(Vec3::splat(2.),glam::Quat::from_rotation_y(0.7),f.center);
        let fields=Fields::new(vec![f.clone()],world).unwrap();assert!((world.transform_vector3(fields.acceleration(Vec3::ZERO,Vec3::ZERO))-Vec3::Z*8.).length()<1e-4);
        f.strength=f32::NAN;assert!(Fields::new(vec![f],Mat4::IDENTITY).is_err());assert!(Fields::new(vec![field(0);65],Mat4::IDENTITY).is_err());
        let fields=Fields::new(vec![Field{strength:1000.,..field(0)};64],Mat4::IDENTITY).unwrap();assert!(fields.acceleration(Vec3::X,Vec3::ZERO).length()<=1000.01);
        assert_eq!(Fields::new(vec![],world).unwrap(),Fields::default());
    }
    #[test]
    fn liquid_reacts_and_old_cache_is_invalidated_by_a_field_change() {
        use crate::{FluidConfig,FluidWorld,CacheTimeline};
        let config=FluidConfig{gravity:Vec3::ZERO,..Default::default()};let mut world=FluidWorld::new(config,vec![Vec3::new(0.,0.8,0.)]).unwrap();
        let cache=CacheTimeline::new(&world);world.set_fields(Fields::new(vec![field(1)],Mat4::IDENTITY).unwrap());assert!(!cache.is_valid_for(&world));
        let before=world.positions()[0];for _ in 0..12 {world.step();}assert!(world.positions()[0].z>before.z+0.001);let tick=world.tick();
        world.set_fields(Fields::new(vec![field(1)],Mat4::IDENTITY).unwrap());assert_eq!(world.tick(),tick);
    }
}
