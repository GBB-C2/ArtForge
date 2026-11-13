//! Bounded mesh-emission quadrature. Surface candidates are prepared on the
//! existing cancellable geometry worker; material edits only repack a tiny table.
use super::world::RayTriangle;
use glam::Vec3;
use std::{collections::BTreeMap, sync::atomic::{AtomicBool, Ordering}};

pub(super) const MAX_SAMPLES: usize = 16;
const CANDIDATES: usize = 16;
pub(super) fn sample_budget(export: bool, interactive: bool) -> usize {
    if export {MAX_SAMPLES} else if interactive {2} else {8}
}
#[derive(Default)]
pub(super) struct Surface {
    pub material: usize,
    area: f64,
    samples: Vec<[f32; 3]>, // triangle index and two barycentric coordinates
}
#[derive(Default)]
struct Builder { surface: Surface, cumulative: f64 }
fn area(t: &RayTriangle) -> f64 {
    let a = Vec3::from_array(t.a[..3].try_into().unwrap());
    let b = Vec3::from_array(t.b[..3].try_into().unwrap());
    let c = Vec3::from_array(t.c[..3].try_into().unwrap());
    let area = (b-a).cross(c-a).length() as f64 * 0.5;
    if t.colors[3] & 1 != 0 || !area.is_finite() { 0. } else { area }
}
pub(super) fn prepare(triangles: &[RayTriangle], cancel: &AtomicBool) -> Result<Vec<Surface>, String> {
    let mut groups: BTreeMap<usize, Builder> = BTreeMap::new();
    for (i, t) in triangles.iter().enumerate() {
        if i % 1024 == 0 && cancel.load(Ordering::Relaxed) { return Err("Подготовка свечения отменена".into()); }
        let material = t.a[3] as usize;
        if material == 0 { continue; }
        let a = area(t);
        if a <= 0. { continue; }
        let g = groups.entry(material).or_default();
        g.surface.material = material;
        g.surface.area += a;
    }
    for (i, t) in triangles.iter().enumerate() {
        if i % 1024 == 0 && cancel.load(Ordering::Relaxed) { return Err("Подготовка свечения отменена".into()); }
        let Some(g) = groups.get_mut(&(t.a[3] as usize)) else { continue; };
        let a = area(t);
        if a <= 0. { continue; }
        let end = g.cumulative + a;
        while g.surface.samples.len() < CANDIDATES {
            let j = g.surface.samples.len();
            let target = (j as f64 + 0.5) / CANDIDATES as f64 * g.surface.area;
            if target > end { break; }
            // Interior points avoid triangle edges and remain stable between frames.
            let u = (0.37 + j as f32 * 0.618_034).fract().sqrt();
            let v = (0.23 + j as f32 * 0.754_878).fract();
            g.surface.samples.push([i as f32, u * (1.-v), u * v]);
        }
        g.cumulative = end;
    }
    Ok(groups.into_values().map(|g| g.surface).filter(|s| !s.samples.is_empty()).collect())
}

/// Importance-stratified selection across all emissive material groups. The
/// fourth component is represented area, including the selection probability.
pub(super) fn pack(surfaces: &[Surface], energy: impl Fn(usize) -> f32, budget: usize)
    -> ([[f32; 4]; MAX_SAMPLES], usize) {
    let mut packed = [[0.; 4]; MAX_SAMPLES];
    let active: Vec<_> = surfaces.iter().filter_map(|s| {
        let e = energy(s.material) as f64;
        (e > 0. && e.is_finite() && !s.samples.is_empty()).then_some((s,e))
    }).collect();
    let total: f64 = active.iter().map(|(s,e)| s.area * e).sum();
    let count = budget.min(MAX_SAMPLES);
    if total <= 0. || !total.is_finite() || count == 0 { return (packed, 0); }
    let mut group = 0;
    let mut start = 0.;
    for (i, dst) in packed.iter_mut().take(count).enumerate() {
        let target = (i as f64 + 0.5) / count as f64 * total;
        while group + 1 < active.len() && target > start + active[group].0.area * active[group].1 {
            start += active[group].0.area * active[group].1;
            group += 1;
        }
        let (surface, energy) = active[group];
        let local = ((target-start)/(surface.area*energy) * surface.samples.len() as f64) as usize;
        let sample = surface.samples[local.min(surface.samples.len()-1)];
        *dst = [sample[0], sample[1], sample[2], (total / (energy * count as f64)).min(f32::MAX as f64) as f32];
    }
    (packed, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;
    fn triangle(material: f32, scale: f32) -> RayTriangle {
        let mut t = RayTriangle::zeroed();
        t.a = [0.,0.,0.,material]; t.b = [scale,0.,0.,1.]; t.c = [0.,scale,0.,0.]; t
    }
    #[test]
    fn candidates_cover_area_and_stay_inside_real_triangles() {
        let triangles = [triangle(1.,2.), triangle(2.,4.)];
        let groups = prepare(&triangles, &AtomicBool::new(false)).unwrap();
        assert_eq!(groups.len(),2);
        assert_eq!(groups[0].area,2.); assert_eq!(groups[1].area,8.);
        for g in &groups { for s in &g.samples { assert!(s[1]>0. && s[2]>0. && s[1]+s[2]<1.); } }
        let (samples,n) = pack(&groups, |_| 1.,16);
        assert_eq!(n,16);
        assert!((samples.iter().map(|s|s[3]).sum::<f32>()-10.).abs()<0.0001);
    }
    #[test]
    fn disabled_and_shadow_only_surfaces_never_become_lights() {
        let mut hidden=triangle(2.,9.); hidden.colors[3]=1;
        let groups=prepare(&[triangle(1.,2.),hidden,triangle(0.,10.)],&AtomicBool::new(false)).unwrap();
        assert_eq!(groups.len(),1);
        assert_eq!(pack(&groups, |_| 0.,8).1,0);
        assert_eq!(pack(&groups, |_| f32::NAN,8).1,0);
        assert_eq!(pack(&groups, |_| 1.,8).1,8);
    }
    #[test]
    fn material_edits_reuse_candidates_and_preserve_total_power() {
        let groups=prepare(&[triangle(1.,2.),triangle(2.,4.)],&AtomicBool::new(false)).unwrap();
        let (samples,n)=pack(&groups, |m|if m==1 {8.} else {1.},8);
        let power:f32=samples[..n].iter().map(|s|s[3]*if s[0]==0. {8.} else {1.}).sum();
        assert!((power-24.).abs()<0.0001);
        let (samples,n)=pack(&groups, |m|if m==1 {0.} else {1.},8);
        assert!(samples[..n].iter().all(|s|s[0]==1.));
        assert!((samples[..n].iter().map(|s|s[3]).sum::<f32>()-8.).abs()<0.0001);
    }
    #[test]
    fn interaction_reduces_work_without_changing_uniform_emitter_power_or_export_budget() {
        let groups=prepare(&[triangle(1.,2.)],&AtomicBool::new(false)).unwrap();
        for (export,interactive,expected) in [(false,false,8),(false,true,2),(true,true,16)] {
            let (samples,n)=pack(&groups, |_|8.,sample_budget(export,interactive));
            assert_eq!(n,expected);
            let power=samples[..n].iter().map(|s|s[3]*8.).sum::<f32>();
            assert!((power-16.).abs()<0.0001);
        }
    }
    #[test]
    fn preparation_can_be_cancelled_before_scanning_scene() {
        assert!(prepare(&[triangle(1.,2.)],&AtomicBool::new(true)).is_err());
    }
}
