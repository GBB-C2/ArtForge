//! Bounded steady plume trajectories. Density animation is sampled separately by the renderer.
use crate::{fields::Fields, mesh_obstacles::MeshObstacles};
use glam::Vec3;
use std::sync::atomic::{AtomicBool, Ordering};

pub const FLOW_KNOTS:usize=6;
pub const MAX_FLOW_SOURCES:usize=16;
const SUBSTEPS:usize=8;

#[derive(Clone,Copy)]
pub struct FlowConfig {
    pub velocity:Vec3,
    pub drift:Vec3,
    pub lifetime:f32,
    pub radius:f32,
    pub influence:f32,
    /// Preferred pressure-release direction when a head-on impact has no tangent.
    pub escape:Vec3,
}

fn candidates(mesh:&MeshObstacles,start:Vec3,end:Vec3,radius:f32,scratch:&mut Vec<usize>,budget:&mut usize)->Result<(),String> {
    mesh.surface().candidates(start,end,radius,scratch);
    if scratch.len()>*budget {return Err("FX: превышен бюджет контактов; упростите меши-препятствия".into());}
    *budget-=scratch.len(); Ok(())
}
fn release_direction(normal:Vec3,hint:Vec3)->Vec3 {
    let tangent=hint-normal*hint.dot(normal);
    if let Some(direction)=tangent.try_normalize() {return direction;}
    let axis=if normal.x.abs()<0.8 {Vec3::X}else{Vec3::Z};
    (axis-normal*axis.dot(normal)).normalize_or_zero()
}

/// Small transport parcels slide along surfaces. A persistent driving flow resumes
/// after the edge; conservative density masks handle the plume's visible width.
pub fn trace(source:Vec3,config:FlowConfig,fields:&Fields,obstacles:Option<&MeshObstacles>,
    cancel:&AtomicBool,budget:&mut usize)->Result<Option<[Vec3;FLOW_KNOTS]>,String> {
    if cancel.load(Ordering::Relaxed) {return Err("Подготовка FX отменена".into());}
    if !source.is_finite() || !config.velocity.is_finite() || !config.drift.is_finite() || !config.escape.is_finite()
        || !config.lifetime.is_finite() || config.lifetime<=0. || !config.radius.is_finite() || config.radius<=0.
        || !config.influence.is_finite() || !(0. ..=2.).contains(&config.influence) {
        return Err("Некорректные параметры потока FX".into());
    }
    let mut scratch=Vec::new(); let mut source=source;
    if let Some(mesh)=obstacles {
        let nudge=config.velocity.normalize_or_zero()*(config.radius*0.001);
        if mesh.contains(source+nudge) {return Ok(None);}
        if !mesh.accepts_particle(source,config.radius,&mut scratch) {
            source=mesh.project(source+nudge,source+nudge,config.radius,&mut scratch);
            if mesh.contains(source) {return Ok(None);}
        }
    }
    let mut points=[source;FLOW_KNOTS];let mut velocity=config.velocity;let mut touched=false;
    let segment_dt=config.lifetime/(FLOW_KNOTS-1) as f32;let dt=segment_dt/SUBSTEPS as f32;
    let drive=config.velocity.length().max(0.01);
    let max_speed=(drive+config.drift.length()/config.lifetime)*8.;
    for i in 1..FLOW_KNOTS {
        let start=points[i-1];let mut end=start;
        for _ in 0..SUBSTEPS {
            if cancel.load(Ordering::Relaxed) {return Err("Подготовка FX отменена".into());}
            let a=fields.acceleration(end,velocity)*config.influence+config.drift*(2./config.lifetime.powi(2));
            if touched {velocity+=(config.velocity-velocity)*(dt/config.lifetime*2.).min(1.);}
            velocity=(velocity+a*dt).clamp_length_max(max_speed);
            let previous=end;end+=velocity*dt;
            if let Some(mesh)=obstacles {
                for _ in 0..3 {
                    candidates(mesh,previous,end,config.radius,&mut scratch,budget)?;
                    let mut corrected=false;
                    for &j in &scratch {
                        if let Some((push,_))=mesh.surface().triangle(j).contact(previous,end,config.radius) {
                            let Some(normal)=push.try_normalize() else{continue;};
                            end+=push; touched=true; corrected=true;
                            let inward=velocity.dot(normal).min(0.); velocity-=normal*inward;
                            // Pressure release gives an exactly head-on plume somewhere to go.
                            if inward<0. && velocity.length()<drive*0.65 {
                                velocity+=release_direction(normal,config.escape)*(drive*0.65-velocity.length());
                            }
                        }
                    }
                    if !corrected {break;}
                }
                candidates(mesh,previous,end,0.,&mut scratch,budget)?;
                if mesh.surface().occluded(previous,end,&mut scratch) {end=previous;velocity=Vec3::ZERO;}
            }
        }
        if let Some(mesh)=obstacles {
            // The six rendered knots must not shortcut through a corner followed by substeps.
            candidates(mesh,start,end,0.,&mut scratch,budget)?;
            if mesh.surface().occluded(start,end,&mut scratch) {
                for _ in 0..3 {
                    candidates(mesh,start,end,config.radius,&mut scratch,budget)?;
                    for &j in &scratch {if let Some((push,_))=mesh.surface().triangle(j).contact(start,end,config.radius) {end+=push;}}
                }
                candidates(mesh,start,end,0.,&mut scratch,budget)?;
                if mesh.surface().occluded(start,end,&mut scratch) {end=start;}
            }
        }
        if !end.is_finite() {return Err("Переполнение потока FX".into());}
        points[i]=end;
    }
    Ok(Some(points))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fields::Field,mesh_contact::Triangle};
    use glam::Mat4;
    fn config()->FlowConfig {FlowConfig{velocity:Vec3::Y,drift:Vec3::ZERO,lifetime:1.,radius:0.02,influence:1.,escape:Vec3::X}}
    fn field(kind:u8)->Fields {Fields::new(vec![Field{kind,center:Vec3::ZERO,axis:Vec3::Z,
        strength:2.,radius:100.,falloff:0.,damping:0.5}],Mat4::IDENTITY).unwrap()}
    fn run(source:Vec3,fields:&Fields,mesh:Option<&MeshObstacles>)->[Vec3;FLOW_KNOTS] {
        trace(source,config(),fields,mesh,&AtomicBool::new(false),&mut 100000).unwrap().unwrap()
    }
    #[test]
    fn all_fields_deflect_flow_and_zero_influence_preserves_the_original_path() {
        let source=Vec3::X;let base=run(source,&Fields::default(),None);
        assert!(base[5].abs_diff_eq(Vec3::new(1.,1.,0.),1e-5));
        assert!(run(source,&field(0),None)[5].x>base[5].x);
        assert!(run(source,&field(1),None)[5].z>0.5);
        assert!(run(source,&field(2),None)[5].x<base[5].x);
        assert!(run(source,&field(3),None)[5].x<base[5].x);
        let frozen=trace(source,FlowConfig{influence:0.,..config()},&field(1),None,&AtomicBool::new(false),&mut 100000).unwrap().unwrap();
        assert_eq!(frozen,base);
    }
    #[test]
    fn a_thin_plane_stops_the_flow_but_its_empty_bounding_box_does_not() {
        let barrier=Triangle([Vec3::new(-2.,0.5,-2.),Vec3::new(2.,0.5,-2.),Vec3::new(0.,0.5,2.)]);
        let mesh=MeshObstacles::new(vec![vec![barrier]],&AtomicBool::new(false)).unwrap();
        let hit=run(Vec3::ZERO,&Fields::default(),Some(&mesh));assert!(hit.iter().all(|p|p.y<=0.481));
        let clear=run(Vec3::new(1.9,0.,1.9),&Fields::default(),Some(&mesh));assert!(clear[5].y>0.99);
    }
    #[test]
    fn cancellation_and_exhausted_contact_budget_return_errors_not_partial_paths() {
        assert!(trace(Vec3::ZERO,config(),&field(1),None,&AtomicBool::new(true),&mut 100).is_err());
        let plane=Triangle([Vec3::new(-2.,0.5,-2.),Vec3::new(2.,0.5,-2.),Vec3::new(0.,0.5,2.)]);
        let mesh=MeshObstacles::new(vec![vec![plane]],&AtomicBool::new(false)).unwrap();
        assert!(trace(Vec3::ZERO,config(),&Fields::default(),Some(&mesh),&AtomicBool::new(false),&mut 0).is_err());
    }
    #[test]
    fn head_on_flow_slides_and_resumes_rising_after_the_edge() {
        let a=Vec3::new(-0.15,0.3,-2.);let b=Vec3::new(0.15,0.3,-2.);
        let c=Vec3::new(0.15,0.3,2.);let d=Vec3::new(-0.15,0.3,2.);
        let mesh=MeshObstacles::new(vec![vec![Triangle([a,b,c]),Triangle([a,c,d])]],&AtomicBool::new(false)).unwrap();
        let points=trace(Vec3::ZERO,FlowConfig{lifetime:2.,..config()},&Fields::default(),Some(&mesh),&AtomicBool::new(false),&mut 100_000).unwrap().unwrap();
        assert!(points.iter().any(|p|p.x>0.2),"head-on flow must find a tangent");
        assert!(points[5].y>0.45,"buoyancy must recover after the finite plate ends");
        let mut scratch=Vec::new();for pair in points.windows(2) {assert!(!mesh.surface().occluded(pair[0],pair[1],&mut scratch));}
    }
    #[test]
    fn surface_winding_does_not_change_the_side_or_slide_direction() {
        let vertices=[Vec3::new(-3.,0.4,-3.),Vec3::new(3.,0.4,-3.),Vec3::new(0.,0.4,3.)];
        let a=MeshObstacles::new(vec![vec![Triangle(vertices)]],&AtomicBool::new(false)).unwrap();
        let b=MeshObstacles::new(vec![vec![Triangle([vertices[2],vertices[1],vertices[0]])]],&AtomicBool::new(false)).unwrap();
        let first=run(Vec3::ZERO,&Fields::default(),Some(&a));let second=run(Vec3::ZERO,&Fields::default(),Some(&b));
        for (a,b) in first.iter().zip(second) {assert!(a.abs_diff_eq(b,1e-5));assert!(a.y<0.401);}
        assert!(first[5].x>0.1,"collision must not freeze the plume");
    }

}
