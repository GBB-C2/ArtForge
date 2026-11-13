//! Bounded relative vertex/surface sweeps with symmetric impulses. No spherical proxy contacts.
use super::{ShardMotion, ShardShape};
use glam::Vec3;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ShardContact<'a> {
    pub shape: &'a ShardShape,
    pub motion: &'a mut ShardMotion,
    pub previous: ShardMotion,
    pub enabled: bool,
    pub restitution: f32,
    pub floor: f32,
}

fn bounds(shape:&ShardShape, motion:ShardMotion)->(Vec3,Vec3) {
    let (lo,hi)=shape.bounds;
    let mut a=Vec3::splat(f32::INFINITY);let mut b=Vec3::splat(f32::NEG_INFINITY);
    for x in [lo.x,hi.x] {for y in [lo.y,hi.y] {for z in [lo.z,hi.z] {
        let p=motion.position+motion.rotation*Vec3::new(x,y,z);a=a.min(p);b=b.max(p);
    }}}
    (a-Vec3::splat(shape.margin),b+Vec3::splat(shape.margin))
}
fn swept_bounds(item:&ShardContact<'_>)->(Vec3,Vec3) {
    let old=bounds(item.shape,item.previous);let now=bounds(item.shape,*item.motion);
    (old.0.min(now.0),old.1.max(now.1))
}
fn sweep(a:&ShardContact<'_>,b:&ShardContact<'_>,scratch:&mut Vec<usize>,budget:&mut usize,cancel:&AtomicBool)
    ->Result<(Vec3,Vec3),String> {
    let mut best=Vec3::ZERO;let mut point=Vec3::ZERO;
    let margin=a.shape.margin.max(b.shape.margin);
    for (index,vertex) in a.shape.vertices.iter().enumerate() {
        if index%64==0 && cancel.load(Ordering::Relaxed) {return Err("Столкновения осколков отменены".into());}
        if *budget==0 {return Err("Слишком сложные взаимные контакты; уменьшите число осколков или упростите меш".into());}
        *budget-=1;
        let start=b.previous.rotation.conjugate()*(a.previous.position+a.previous.rotation * *vertex-b.previous.position);
        let world=a.motion.position+a.motion.rotation * *vertex;
        let end=b.motion.rotation.conjugate()*(world-b.motion.position);
        let push=b.motion.rotation*(b.shape.surface.project(start,end,margin,scratch)-end);
        if push.length_squared()>best.length_squared() {best=push;point=world;}
    }
    Ok((best,point))
}
fn omega(motion:&ShardMotion)->Vec3 {motion.angular_axis*motion.angular_speed}
fn set_omega(motion:&mut ShardMotion,value:Vec3) {
    let speed=value.length().min(40.);motion.angular_speed=speed;
    motion.angular_axis=value.try_normalize().unwrap_or(Vec3::Y);
}
fn inertia(shape:&ShardShape)->f32 {shape.inverse_mass/(0.4*shape.radius*shape.radius).max(1e-6)}

/// Call after the same fixed step for all bodies. Both objects must opt in.
/// Cancellation discards the worker generation; authored geometry is never modified.
pub fn solve_shard_contacts(items:&mut[ShardContact<'_>],cancel:&AtomicBool)->Result<(),String> {
    if items.len()>64 {return Err("Столкновения: максимум 64 осколка одновременно".into());}
    let mut scratch=Vec::new();let mut budget=200_000usize;
    for iteration in 0..4 {
        if cancel.load(Ordering::Relaxed) {return Err("Столкновения осколков отменены".into());}
        let boxes:Vec<_>=items.iter().map(swept_bounds).collect();let mut changed=false;
        for i in 0..items.len() {for j in i+1..items.len() {
            if !items[i].enabled || !items[j].enabled || boxes[i].0.cmpgt(boxes[j].1).any() || boxes[j].0.cmpgt(boxes[i].1).any() {continue;}
            let (left,right)=items.split_at_mut(j);let a=&mut left[i];let b=&mut right[0];
            // Later passes resolve only residual proximity, never replay the original crossing.
            if iteration>0 {a.previous=*a.motion;b.previous=*b.motion;}
            let (push_a,point_a)=sweep(a,b,&mut scratch,&mut budget,cancel)?;
            let (push_b,point_b)=sweep(b,a,&mut scratch,&mut budget,cancel)?;
            let (push,point)=if push_a.length_squared()>=push_b.length_squared() {(push_a,point_a)}else{(-push_b,point_b)};
            let Some(normal)=push.try_normalize() else{continue;};
            if push.length_squared()<1e-12 {continue;}
            changed=true;
            let ma=a.shape.inverse_mass;let mb=b.shape.inverse_mass;let total=ma+mb;
            a.motion.position+=push*(ma/total);b.motion.position-=push*(mb/total);
            let ra=point-a.motion.position;let rb=point-b.motion.position;
            let ia=inertia(a.shape);let ib=inertia(b.shape);
            let relative=a.motion.velocity+omega(a.motion).cross(ra)-b.motion.velocity-omega(b.motion).cross(rb);
            let incoming=relative.dot(normal);
            if incoming<0. {
                let bounce=if incoming < -0.15 {a.restitution.min(b.restitution)}else{0.};
                let denominator=total+ra.cross(normal).length_squared()*ia+rb.cross(normal).length_squared()*ib;
                let impulse=normal*(-(1.+bounce)*incoming/denominator.max(1e-8));
                a.motion.velocity=(a.motion.velocity+impulse*ma).clamp_length_max(100.);
                b.motion.velocity=(b.motion.velocity-impulse*mb).clamp_length_max(100.);
                let wa=omega(a.motion)+ra.cross(impulse)*ia;let wb=omega(b.motion)-rb.cross(impulse)*ib;
                set_omega(a.motion,wa);set_omega(b.motion,wb);
            }
        }}
        // Contact correction must not push a resting fragment below its configured floor.
        for item in items.iter_mut().filter(|i|i.enabled) {
            let bottom=item.shape.vertices.iter().map(|p|(item.motion.rotation * *p).y).fold(f32::INFINITY,f32::min)+item.motion.position.y;
            if bottom<item.floor {item.motion.position.y+=item.floor-bottom;item.motion.velocity.y=item.motion.velocity.y.max(0.);}
            if !item.motion.position.is_finite() || !item.motion.velocity.is_finite() {return Err("Некорректный контакт осколков; уменьшите импульс".into());}
        }
        if !changed {break;}
    }
    Ok(())
}
