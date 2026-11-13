//! Close plane cuts by their connected contours, including concavities and holes.
use super::{normal, position, Polygon};
use crate::{Vec3, Vertex};
use std::{collections::{BTreeMap, BTreeSet}, sync::atomic::{AtomicBool, Ordering}};

type Point = [i64; 3];
fn inside(point: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut result = false;
    for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0] { result = !result; }
    }
    result
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0]-a[0])*(c[1]-a[1]) - (b[1]-a[1])*(c[0]-a[0])
}
fn crosses(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2], eps: f64) -> bool {
    cross(a,b,c)*cross(a,b,d) < -eps*eps && cross(c,d,a)*cross(c,d,b) < -eps*eps
}

pub(super) fn close_cut(faces: &[Polygon], n: Vec3, d: f32, epsilon: f32, material: Option<u64>, cancel: &AtomicBool)
    -> Result<Option<Vec<Polygon>>, String>
{
    let key = |p: Vec3| (p / epsilon).to_array().map(|v| v.round() as i64);
    let mut points = BTreeMap::<Point, Vec3>::new();
    let mut edges = BTreeMap::<(Point, Point), usize>::new();
    for face in faces {
        for i in 0..face.vertices.len() {
            let a = position(&face.vertices[i]); let b = position(&face.vertices[(i+1)%face.vertices.len()]);
            if (n.dot(a)-d).abs() > epsilon*4. || (n.dot(b)-d).abs() > epsilon*4. { continue; }
            let ka=key(a); let kb=key(b);
            if ka == kb { continue; }
            points.entry(ka).or_insert(a); points.entry(kb).or_insert(b);
            *edges.entry((ka.min(kb),ka.max(kb))).or_default() += 1;
        }
    }
    let mut remaining: BTreeSet<_> = edges.into_iter().filter(|(_,count)| *count == 1).map(|(edge,_)| edge).collect();
    let mut adjacency = BTreeMap::<Point,Vec<Point>>::new();
    for &(a,b) in &remaining { adjacency.entry(a).or_default().push(b); adjacency.entry(b).or_default().push(a); }
    if adjacency.values().any(|neighbors| neighbors.len()!=2) { return Ok(None); }
    let u=n.cross(if n.x.abs()<0.8 {Vec3::X} else {Vec3::Y}).normalize(); let v=n.cross(u);
    let project = |p: Vec3| [p.dot(u) as f64,p.dot(v) as f64];
    let mut rings = Vec::<Vec<Vec3>>::new();
    while let Some((start, mut cursor)) = remaining.pop_first() {
        let mut ring=vec![points[&start]]; let mut previous=start;
        while cursor!=start {
            if cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
            ring.push(points[&cursor]);
            let next=adjacency[&cursor].iter().find(|&&p| p!=previous).copied().unwrap();
            if !remaining.remove(&(cursor.min(next),cursor.max(next))) { return Ok(None); }
            previous=cursor;cursor=next;
            if ring.len()>4096 { return Ok(None); }
        }
        if ring.len()>=3 { rings.push(ring); }
    }
    let flat: Vec<Vec<_>> = rings.iter().map(|r| r.iter().copied().map(project).collect()).collect();
    let depths:Vec<usize> = flat.iter().enumerate().map(|(i,r)| flat.iter().enumerate().filter(|(j,other)| *j!=i && inside(r[0],other)).count()).collect();
    // Cross products have units of area. Account for the f32 position error
    // across the contour, otherwise a collinear boundary point can be clipped
    // as an ear and leave a degenerate cap (and a T-junction on the next cut).
    let origin = flat.iter().flatten().next().copied().unwrap_or([0.; 2]);
    let extent = flat.iter().flatten().map(|p| (p[0]-origin[0]).abs().max((p[1]-origin[1]).abs()))
        .fold(epsilon as f64, f64::max);
    let eps = epsilon as f64 * extent * 2.;
    let mut caps=Vec::new();
    for outer in 0..rings.len() {
        if depths[outer]%2!=0 { continue; }
        let vertex = |p: Vec3| Vertex {position:p.to_array(),normal:n.to_array(),uv:[p.dot(u),p.dot(v)],..Default::default()};
        let mut contour:Vec<_>=rings[outer].iter().copied().map(vertex).collect();
        if normal(&contour).dot(n)<0. {contour.reverse();}
        let holes:Vec<_>=(0..rings.len()).filter(|&i| depths[i]==depths[outer]+1 && inside(flat[i][0],&flat[outer])).collect();
        // Join each hole through a visible bridge. Repeated endpoints describe
        // the two sides of the bridge; triangulation ignores coincident corners.
        for &hole in &holes {
            let mut ring:Vec<_>=rings[hole].iter().copied().map(vertex).collect();
            if normal(&ring).dot(n)>0. {ring.reverse();}
            let mut bridge=None; let mut distance=f32::INFINITY;
            for (a,va) in contour.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {return Err("Разрушение отменено".into());}
                for (b,vb) in ring.iter().enumerate() {
                    let pa=project(position(va));let pb=project(position(vb));
                    let length=position(va).distance_squared(position(vb));
                    if length>=distance {continue;}
                    let middle=[(pa[0]+pb[0])*0.5,(pa[1]+pb[1])*0.5];
                    if !inside(middle,&flat[outer]) || holes.iter().any(|&h|inside(middle,&flat[h])) {continue;}
                    let blocked=flat.iter().any(|r|r.iter().zip(r.iter().cycle().skip(1)).take(r.len()).any(|(&c,&d)|crosses(pa,pb,c,d,eps)))
                        || contour.iter().zip(contour.iter().cycle().skip(1)).take(contour.len()).any(|(c,d)|crosses(pa,pb,project(position(c)),project(position(d)),eps));
                    if !blocked {bridge=Some((a,b));distance=length;}
                }
            }
            let Some((a,b))=bridge else {return Ok(None);};
            let mut joined=contour[..=a].to_vec();
            joined.extend((0..=ring.len()).map(|i|ring[(b+i)%ring.len()]));
            joined.extend_from_slice(&contour[a..]);contour=joined;
        }
        let projected:Vec<_>=contour.iter().map(|v|project(position(v))).collect();
        let mut indices:Vec<_>=(0..contour.len()).collect();
        let same=|a:[f64;2],b:[f64;2]| (a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)<(epsilon as f64).powi(2);
        while indices.len()>3 {
            if cancel.load(Ordering::Relaxed) {return Err("Разрушение отменено".into());}
            let mut ear=None;
            for i in 0..indices.len() {
                let a=indices[(i+indices.len()-1)%indices.len()];let b=indices[i];let c=indices[(i+1)%indices.len()];
                if cross(projected[a],projected[b],projected[c])<=eps {continue;}
                if !indices.iter().any(|&p| p!=a&&p!=b&&p!=c
                    && !same(projected[p],projected[a])&&!same(projected[p],projected[b])&&!same(projected[p],projected[c])
                    && cross(projected[a],projected[b],projected[p])>=-eps
                    && cross(projected[b],projected[c],projected[p])>=-eps
                    && cross(projected[c],projected[a],projected[p])>=-eps) {ear=Some((i,a,b,c));break;}
            }
            let Some((i,a,b,c))=ear else {return Ok(None);};
            caps.push(Polygon{vertices:vec![contour[a],contour[b],contour[c]],material});indices.remove(i);
        }
        if indices.len()==3 {caps.push(Polygon{vertices:indices.iter().map(|&i|contour[i]).collect(),material});}
    }
    Ok(Some(caps))
}
