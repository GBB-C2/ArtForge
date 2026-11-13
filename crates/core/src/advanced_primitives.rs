//! Closed primitives preserve polygon rings; triangulation is only a render index buffer.
use crate::{CoreError,Mesh,PrimitiveKind as K,PrimitiveSettings,Vec3,Vertex};
fn vertex(m:&mut Mesh,p:Vec3,uv:[f32;2])->u32 {let id=m.vertices.len() as u32;m.vertices.push(Vertex{position:p.to_array(),uv,..Default::default()});id}
fn face(m:&mut Mesh,ring:Vec<u32>) {m.topology.as_mut().unwrap().faces.push(ring);}
fn lathe(m:&mut Mesh,profile:&[(f32,f32)],cols:usize,caps:bool) {
    let mut rings=Vec::new();
    for (row,&(y,r)) in profile.iter().enumerate() {
        let v=row as f32/(profile.len()-1) as f32;
        let ring=if r.abs()<1e-7 {vec![vertex(m,Vec3::new(0.,y,0.),[0.5,v])]} else {(0..=cols).map(|i| {
            let a=(i%cols) as f32/cols as f32*std::f32::consts::TAU;
            vertex(m,Vec3::new(r*a.cos(),y,r*a.sin()),[i as f32/cols as f32,v])
        }).collect()};
        let lo=profile[row.saturating_sub(1)];let hi=profile[(row+1).min(profile.len()-1)];
        for (i,&id) in ring.iter().enumerate() {let a=(i%cols) as f32/cols as f32*std::f32::consts::TAU;
            let normal=if ring.len()==1 {if row==0 {-Vec3::Y}else{Vec3::Y}}else{Vec3::new((hi.0-lo.0)*a.cos(),lo.1-hi.1,(hi.0-lo.0)*a.sin()).normalize_or_zero()};
            m.vertices[id as usize].normal=normal.to_array();
        }rings.push(ring);
    }
    for rows in rings.windows(2) {for i in 0..cols {
        let (a,b)=(&rows[0],&rows[1]);
        let ring=if a.len()==1 {vec![a[0],b[i],b[i+1]]} else if b.len()==1 {vec![a[i],b[0],a[i+1]]} else {vec![a[i],b[i],b[i+1],a[i+1]]};face(m,ring);
    }}
    if caps {for (index,reverse) in [(0,false),(rings.len()-1,true)] {
        if rings[index].len()==1 {continue;}
        // Independent cap vertices keep cap normals flat even with smooth sides.
        let mut ids=Vec::new();for i in 0..cols {let p=Vec3::from_array(m.vertices[rings[index][i] as usize].position);let r=profile[index].1;let id=vertex(m,p,[0.5+p.x/(2.*r),0.5+p.z/(2.*r)]);m.vertices[id as usize].normal=(if reverse {Vec3::Y}else{-Vec3::Y}).to_array();ids.push(id);}
        if reverse {ids.reverse();}face(m,ids);
    }}
}
pub(crate) fn build(m:&mut Mesh,s:PrimitiveSettings)->Result<(),CoreError> {
    let n=s.segments[0] as usize;let rows=s.segments[1] as usize;let h=s.size[1];let r=s.radius;
    match s.kind {
        K::Cylinder|K::Cone=>{let profile:Vec<_>=(0..=rows).map(|j| {let t=j as f32/rows as f32;((t-0.5)*h,if s.kind==K::Cone {r*(1.-t)} else {r})}).collect();lathe(m,&profile,n,true);}
        K::Capsule=>{
            let half=(h-2.*r)*0.5;let mut profile=Vec::new();
            for j in 0..=rows {let a=-std::f32::consts::FRAC_PI_2+j as f32/rows as f32*std::f32::consts::FRAC_PI_2;profile.push((-half+r*a.sin(),if j==0 {0.}else{r*a.cos()}));}
            if half>0. {profile.push((half,r));}
            for j in 1..=rows {let a=j as f32/rows as f32*std::f32::consts::FRAC_PI_2;profile.push((half+r*a.sin(),if j==rows {0.}else{r*a.cos()}));}
            lathe(m,&profile,n,false);
        }
        K::Torus=>{
            for j in 0..=rows {let b=(j%rows) as f32/rows as f32*std::f32::consts::TAU;for i in 0..=n {let a=(i%n) as f32/n as f32*std::f32::consts::TAU;
                let id=vertex(m,Vec3::new((r+s.size[0]*b.cos())*a.cos(),s.size[0]*b.sin(),(r+s.size[0]*b.cos())*a.sin()),[i as f32/n as f32,j as f32/rows as f32]);m.vertices[id as usize].normal=Vec3::new(b.cos()*a.cos(),b.sin(),b.cos()*a.sin()).to_array();}}
            for j in 0..rows {for i in 0..n {let a=(j*(n+1)+i) as u32;face(m,vec![a,a+(n+1) as u32,a+(n+2) as u32,a+1]);}}
        }
        K::Pyramid=>{
            for (p,uv) in [(Vec3::new(-s.size[0]/2.,-h/2.,-s.size[2]/2.),[0.,0.]),(Vec3::new(s.size[0]/2.,-h/2.,-s.size[2]/2.),[1.,0.]),(Vec3::new(s.size[0]/2.,-h/2.,s.size[2]/2.),[1.,1.]),(Vec3::new(-s.size[0]/2.,-h/2.,s.size[2]/2.),[0.,1.]),(Vec3::new(0.,h/2.,0.),[0.5,0.5])] {vertex(m,p,uv);}
            face(m,vec![0,1,2,3]);for i in 0..4 {face(m,vec![i,4,(i+1)%4]);}
        }
        _=>unreachable!(),
    }
    m.rebuild_polygon_indices()?;
    if s.smooth {if s.kind==K::Pyramid {crate::recompute_normals(m,true);}} else {crate::polygon_edit::flat_normals(m)?;}
    Ok(())
}
