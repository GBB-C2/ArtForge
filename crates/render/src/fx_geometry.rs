//! Small immutable geometry snapshots; expensive sampling and BVH preparation run off the UI thread.
use poseforge_core::{CurveData,Mat4,Mesh,MorphTarget,Node,SharedBuffer,Vec3,Vertex};
use poseforge_simulation::{fx_flow::MAX_FLOW_SOURCES,mesh_contact::Triangle};
use std::{collections::BTreeMap,hash::{Hash,Hasher},sync::atomic::{AtomicBool,Ordering}};

pub(super) const MAX_TRIANGLES:usize=200_000;
#[derive(Clone)]
pub(super) struct Geometry {
    pub key:u64,
    pub indices:SharedBuffer<u32>,
    vertices:SharedBuffer<Vertex>,
    morphs:Vec<MorphTarget>,weights:Vec<f32>,palette:Vec<Mat4>,model:Mat4,
}
pub(super) struct PreparedGeometry {pub triangles:Vec<Triangle>,pub samples:Vec<Vec3>}
pub(super) fn matrix_hash(matrix:Mat4,hash:&mut impl Hasher) {matrix.to_cols_array().map(f32::to_bits).hash(hash);}
pub(super) fn check(cancel:&AtomicBool)->Result<(),String> {
    if cancel.load(Ordering::Relaxed) {Err("Подготовка FX отменена".into())}else{Ok(())}
}
impl Geometry {
    pub fn snapshot(node:&Node,mesh:&Mesh,globals:&BTreeMap<u64,Mat4>)->Result<Self,String> {
        if mesh.indices.len()/3>MAX_TRIANGLES {return Err("FX: источник или препятствие содержит больше 200000 треугольников".into());}
        let model=*globals.get(&node.id).ok_or("FX: объект без трансформации")?;
        if !model.is_finite() || model.determinant().abs()<1e-8 || !model.inverse().is_finite() {return Err("FX: вырожденный масштаб источника или препятствия".into());}
        let palette=node.skin.as_ref().map(|s|s.joints.iter().zip(&s.inverse_bind_matrices)
            .map(|(id,bind)|model.inverse()*globals.get(id).copied().unwrap_or(Mat4::IDENTITY)* *bind).collect()).unwrap_or_default();
        let mut result=Self {key:0,vertices:mesh.vertices.clone(),indices:mesh.indices.clone(),
            morphs:mesh.morph_targets.clone(),weights:node.morph_weights.clone(),palette,model};
        let mut h=std::collections::hash_map::DefaultHasher::new();
        result.vertices.revision().hash(&mut h);result.indices.revision().hash(&mut h);matrix_hash(model,&mut h);
        for m in &result.morphs {m.positions.revision().hash(&mut h);}
        for w in &result.weights {w.to_bits().hash(&mut h);}
        for p in &result.palette {matrix_hash(*p,&mut h);}
        result.key=h.finish();Ok(result)
    }
    pub fn build(&self,cancel:&AtomicBool)->Result<PreparedGeometry,String> {
        check(cancel)?;let mut positions=Vec::with_capacity(self.vertices.len());
        for (i,v) in self.vertices.iter().enumerate() {
            if i%1024==0 {check(cancel)?;}
            let mut p=Vec3::from_array(v.position);
            for (m,&w) in self.morphs.iter().zip(&self.weights) {
                p+=Vec3::from_array(*m.positions.get(i).ok_or("FX: повреждённая форма меша")?)*w;
            }
            let mut skin=Mat4::IDENTITY;
            if !self.palette.is_empty() && v.weights.iter().sum::<f32>()>0.0001 {
                skin=Mat4::ZERO;
                for (&joint,&weight) in v.joints.iter().zip(&v.weights) {if weight>0. {skin+=self.palette.get(joint as usize).copied().unwrap_or(Mat4::IDENTITY)*weight;}}
            }
            let p=(self.model*skin*p.extend(1.)).truncate();if !p.is_finite() {return Err("FX: нечисловая геометрия".into());}positions.push(p);
        }
        let mut triangles=Vec::with_capacity(self.indices.len()/3);
        for (i,ids) in self.indices.chunks_exact(3).enumerate() {
            if i%1024==0 {check(cancel)?;}
            let mut triangle=[Vec3::ZERO;3];for j in 0..3 {triangle[j]=*positions.get(ids[j] as usize).ok_or("FX: неверный индекс меша")?;}
            if (triangle[1]-triangle[0]).cross(triangle[2]-triangle[0]).length_squared()>1e-16 {triangles.push(Triangle(triangle));}
        }
        let samples=sample_surface(&triangles);check(cancel)?;Ok(PreparedGeometry{triangles,samples})
    }
}

/// Stratified triangle-area sampling, not samples of a mesh's empty bounding box.
fn sample_surface(triangles:&[Triangle])->Vec<Vec3> {
    let mut cumulative=Vec::with_capacity(triangles.len());let mut total=0_f64;
    for Triangle([a,b,c]) in triangles {total+=((*b-*a).cross(*c-*a).length() as f64)*0.5;cumulative.push(total);}
    if !total.is_finite() || total<=1e-12 {return vec![];}
    (0..MAX_FLOW_SOURCES).map(|i| {
        let at=(i as f64+0.5)/MAX_FLOW_SOURCES as f64*total;
        let index=cumulative.partition_point(|&v|v<at).min(triangles.len()-1);
        let Triangle([a,b,c])=triangles[index];
        let u=((i as f32+0.5)*0.618034).fract().sqrt();let v=((i as f32+0.5)*0.41421357).fract();
        a*(1.-u)+b*(u*(1.-v))+c*(u*v)
    }).collect()
}

/// Equal arc-length sites preserve open endpoints and avoid a duplicate at closed seams.
pub(super) fn sample_curve(curve:&CurveData,model:Mat4)->Result<(Vec<Vec3>,f32),String> {
    let points:Vec<_>=curve.samples().map_err(|e|e.to_string())?.into_iter().map(|p|model.transform_point3(p)).collect();
    let mut lengths=vec![0.];for pair in points.windows(2) {lengths.push(lengths.last().unwrap()+pair[0].distance(pair[1]));}
    let total=*lengths.last().unwrap_or(&0.);if !total.is_finite() || total<1e-6 {return Err("FX: кривая имеет нулевую длину".into());}
    let denominator=if curve.closed {MAX_FLOW_SOURCES}else{MAX_FLOW_SOURCES-1};
    let samples=(0..MAX_FLOW_SOURCES).map(|i| {
        let distance=total*i as f32/denominator as f32;
        let j=lengths.partition_point(|&d|d<distance).clamp(1,points.len()-1);
        points[j-1].lerp(points[j],((distance-lengths[j-1])/(lengths[j]-lengths[j-1]).max(1e-8)).clamp(0.,1.))
    }).collect();Ok((samples,total/denominator as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surface_sites_lie_on_real_triangles_and_ignore_unused_bounds() {
        let t=Triangle([Vec3::new(0.,2.,0.),Vec3::new(2.,2.,0.),Vec3::new(0.,2.,1.)]);
        let samples=sample_surface(&[t]);assert_eq!(samples.len(),MAX_FLOW_SOURCES);
        assert!(samples.iter().all(|p|t.closest(*p).0.distance_squared(*p)<1e-10));
        assert!(sample_surface(&[]).is_empty());
    }
    #[test]
    fn curve_sites_follow_world_transform_and_uniform_arc_length() {
        let curve=CurveData{interpolation:poseforge_core::CurveInterpolation::Polyline,
            points:vec![[0.,0.,0.],[1.,0.,0.],[4.,0.,0.]],..Default::default()};
        let model=Mat4::from_translation(Vec3::new(2.,3.,4.));let (sites,spacing)=sample_curve(&curve,model).unwrap();
        assert_eq!(sites[0],Vec3::new(2.,3.,4.));assert_eq!(*sites.last().unwrap(),Vec3::new(6.,3.,4.));
        assert!(sites.windows(2).all(|p|(p[0].distance(p[1])-spacing).abs()<1e-5));
    }
}
