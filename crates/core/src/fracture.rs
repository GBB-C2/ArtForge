//! Deterministic, capped plane fracture for closed manifold meshes.
#[path = "fracture_caps.rs"]
mod caps;
use crate::{Mesh, MeshTopology, Vec3, Vertex};
use std::{collections::BTreeMap, sync::atomic::{AtomicBool, Ordering}};

#[derive(Clone, Debug)]
pub struct Fragment {
    pub mesh: Mesh,
    pub center: Vec3,
}
#[derive(Clone)]
struct Polygon {
    vertices: Vec<Vertex>,
    material: Option<u64>,
}
#[derive(Clone)]
struct Solid(Vec<Polygon>);
fn position(v: &Vertex) -> Vec3 { Vec3::from_array(v.position) }
fn normal(vertices: &[Vertex]) -> Vec3 {
    let mut n = Vec3::ZERO;
    for i in 0..vertices.len() {
        n += position(&vertices[i]).cross(position(&vertices[(i + 1) % vertices.len()]));
    }
    n.normalize_or_zero()
}
fn interpolate(a: Vertex, b: Vertex, t: f32) -> Vertex {
    Vertex {
        position: position(&a).lerp(position(&b), t).to_array(),
        normal: Vec3::from_array(a.normal).lerp(Vec3::from_array(b.normal), t).normalize_or_zero().to_array(),
        uv: std::array::from_fn(|i| a.uv[i] + (b.uv[i] - a.uv[i]) * t),
        color: std::array::from_fn(|i| a.color[i] + (b.color[i] - a.color[i]) * t),
        ..Default::default()
    }
}
impl Solid {
    fn bounds(&self) -> (Vec3, Vec3) {
        self.0.iter().flat_map(|f| f.vertices.iter()).fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), v| (lo.min(position(v)), hi.max(position(v))))
    }
    fn volume(&self) -> f32 {
        let (lo, hi) = self.bounds();
        let center = (lo + hi) * 0.5;
        self.0.iter().map(|face| {
            let a = position(&face.vertices[0]) - center;
            (1..face.vertices.len() - 1).map(|i| {
                a.dot((position(&face.vertices[i]) - center).cross(position(&face.vertices[i + 1]) - center)) / 6.
            }).sum::<f32>()
        }).sum::<f32>().abs()
    }
    fn clip(&self, n: Vec3, d: f32, epsilon: f32, cut_material: Option<u64>, cut_contrast: f32, cancel: &AtomicBool) -> Result<Option<Self>, String> {
        let mut faces = Vec::new();

        for (index, face) in self.0.iter().enumerate() {
            if index % 128 == 0 && cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
            let mut vertices = Vec::new();
            for i in 0..face.vertices.len() {
                let a = face.vertices[i];
                let b = face.vertices[(i + 1) % face.vertices.len()];
                let snap = |v: Vertex| {
                    let distance = n.dot(position(&v)) - d;
                    if distance.abs() <= epsilon { 0. } else { distance }
                };
                let da = snap(a); let db = snap(b);
                if da <= 0. { vertices.push(a); }

                if (da < 0. && db > 0.) || (da > 0. && db < 0.) {
                    let mut v = interpolate(a, b, da / (da - db));
                    // Use one edge direction for identical coordinates on adjacent faces.
                    v.position = if a.position < b.position {
                        position(&a).lerp(position(&b), da / (da - db)).to_array()
                    } else {
                        position(&b).lerp(position(&a), db / (db - da)).to_array()
                    };
                    vertices.push(v);
                }
            }
            vertices.dedup_by(|a, b| position(a).distance_squared(position(b)) < epsilon * epsilon);
            if vertices.len() > 2 && position(&vertices[0]).distance_squared(position(vertices.last().unwrap())) < epsilon * epsilon { vertices.pop(); }
            if vertices.len() >= 3 && normal(&vertices) != Vec3::ZERO { faces.push(Polygon { vertices, material: face.material }); }
        }
        if faces.is_empty() { return Ok(None); }
        let Some(mut cap) = caps::close_cut(&faces, n, d, epsilon, cut_material, cancel)? else { return Ok(None); };
        for face in &mut cap {
            for vertex in &mut face.vertices {
                for channel in &mut vertex.color[..3] { *channel *= 1. - 0.85 * cut_contrast; }
            }
        }
        faces.extend(cap);
        // A nearly coplanar cut can remove a zero-area face while leaving a
        // T-junction on its neighbours. Reject that plane and try another one;
        // volume conservation alone cannot detect an open boundary.
        let key = |v: &Vertex| (position(v) / epsilon).to_array().map(|x| x.round() as i64);
        let mut edges = BTreeMap::new();
        let mut welded = BTreeMap::new();
        for (index, face) in faces.iter_mut().enumerate() {
            if index % 128 == 0 && cancel.load(Ordering::Relaxed) {
                return Err("Разрушение отменено".into());
            }
            for vertex in &mut face.vertices {
                vertex.position = *welded.entry(key(vertex)).or_insert(vertex.position);
            }
            for i in 0..face.vertices.len() {
                let a = key(&face.vertices[i]);
                let b = key(&face.vertices[(i + 1) % face.vertices.len()]);
                if a != b { *edges.entry((a.min(b), a.max(b))).or_insert(0usize) += 1; }
            }
        }
        if edges.values().any(|&count| count != 2) { return Ok(None); }
        Ok(Some(Self(faces)))
    }
    fn into_fragment(self, source: &Mesh, index: usize) -> Result<Fragment, String> {
        let points: Vec<_> = self.0.iter().flat_map(|p| p.vertices.iter().map(position)).collect();
        let center = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        let mut vertices = Vec::new(); let mut faces = Vec::new(); let mut materials = Vec::new();
        for polygon in self.0 {
            let start = vertices.len() as u32;
            faces.push((start..start + polygon.vertices.len() as u32).collect());
            materials.push(polygon.material);
            vertices.extend(polygon.vertices.into_iter().map(|mut v| {
                v.position = (position(&v) - center).to_array(); v
            }));
        }
        let mut mesh = Mesh {
            id: 0, name: format!("{} · осколок {:02}", source.name, index + 1),
            vertices: vertices.into(), indices: Default::default(), material: source.material,
            topology: Some(MeshTopology { loose_edges: Vec::new(), faces: faces.into(), face_materials: materials.into(),
                uv_seams: vec![], primitive: None, smooth_normals: source.smooth_normals_enabled() }),
            morph_targets: vec![],
        };
        mesh.rebuild_polygon_indices().map_err(|e| e.to_string())?;
        Ok(Fragment { mesh, center })
    }
}
struct Random(u64);
impl Random {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / 16777216.
    }
}
/// Fracture preserves the original mesh and outer face materials, and caps every cut.
/// Connected cut contours preserve concave shapes; open/nonmanifold meshes are rejected.
pub fn fracture_mesh(mesh: &Mesh, pieces: usize, seed: u32, cancel: &AtomicBool) -> Result<Vec<Fragment>, String> {
    fracture_mesh_with_cuts(mesh, pieces, seed, 0., cancel)
}
/// Backward-compatible fracture using the mesh fallback on newly cut faces.
pub fn fracture_mesh_with_cuts(mesh: &Mesh, pieces: usize, seed: u32, cut_contrast: f32,
    cancel: &AtomicBool) -> Result<Vec<Fragment>, String> {
    fracture_mesh_with_material(mesh, pieces, seed, cut_contrast, mesh.material, cancel)
}

/// Preserve exterior polygon assignments and assign every cap its own material.
pub fn fracture_mesh_with_material(mesh: &Mesh, pieces: usize, seed: u32, cut_contrast: f32,
    interior: Option<u64>, cancel: &AtomicBool) -> Result<Vec<Fragment>, String> {
    if !cut_contrast.is_finite() || !(0. ..=1.).contains(&cut_contrast) {
        return Err("Контраст разреза должен быть от 0 до 1".into());
    }
    if !(2..=64).contains(&pieces) || mesh.face_count() > 32768 || mesh.vertices.len() > 100000 {
        return Err("Разрушение: 2–64 осколка, до 32768 граней и 100000 вершин исходного меша".into());
    }
    if !mesh.morph_targets.is_empty() || mesh.vertices.is_empty() || mesh.vertices.iter().any(|v| !position(v).is_finite()) {
        return Err("Выберите статический замкнутый меш".into());
    }
    let size = mesh.vertices.iter().map(position).fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max)
        - mesh.vertices.iter().map(position).fold(Vec3::splat(f32::INFINITY), Vec3::min);
    if !size.is_finite() || size.min_element() < 1e-4 { return Err("Для разрушения нужен объёмный меш".into()); }
    let epsilon = size.max_element() * 1e-6;
    let key = |p: Vec3| (p / epsilon).to_array().map(|v| v.round() as i64);
    let mut edges = BTreeMap::<([i64;3], [i64;3]), Vec<(usize,bool)>>::new();
    let mut faces = Vec::new();
    let mut welded = BTreeMap::new();
    for face in 0..mesh.face_count() {
        if face % 16 == 0 && cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
        let ring = mesh.face_vertices(face).ok_or("Повреждена грань")?;
        let triangles = crate::triangulate::triangulate(mesh, ring).map_err(|e| e.to_string())?;
        for triangle in triangles.chunks_exact(3) {
            let mut vertices = triangle.iter().map(|&i| mesh.vertices[i as usize]).collect::<Vec<_>>();
            for vertex in &mut vertices {
                let p=position(vertex); vertex.position=(*welded.entry(key(p)).or_insert(p)).to_array();
            }
            if normal(&vertices)==Vec3::ZERO {continue;}
            for i in 0..3 {
                let a=key(position(&vertices[i])); let b=key(position(&vertices[(i+1)%3]));
                if a!=b {edges.entry((a.min(b),a.max(b))).or_default().push((faces.len(), a<b));}
            }
            faces.push(Polygon {vertices,material:mesh.topology.as_ref()
                .and_then(|t| t.face_materials.get(face).copied().flatten())});
        }
    }
    if faces.len()<4 || edges.values().any(|owners|owners.len()!=2) {
        return Err("Разрушению нужен замкнутый меш без отверстий и разветвлённых рёбер".into());
    }
    // Repair winding by adjacency, not by a centroid test (invalid for concave meshes).
    let mut adjacent=vec![Vec::<(usize,bool)>::new();faces.len()];
    for owners in edges.values() {
        let (a,da)=owners[0];let (b,db)=owners[1];
        adjacent[a].push((b,da==db));adjacent[b].push((a,da==db));
    }
    let mut flip=vec![None;faces.len()];
    for start in 0..faces.len() {
        if flip[start].is_some() {continue;}
        flip[start]=Some(false); let mut pending=vec![start]; let mut component=Vec::new();
        while let Some(face)=pending.pop() {
            if cancel.load(Ordering::Relaxed) {return Err("Разрушение отменено".into());}
            component.push(face);
            for &(next,reverse) in &adjacent[face] {
                let value=flip[face].unwrap()^reverse;
                match flip[next] {
                    Some(old) if old!=value => return Err("Несогласованная ориентация поверхности".into()),
                    Some(_) => {},
                    None => {flip[next]=Some(value);pending.push(next);}
                }
            }
        }
        for &face in &component {if flip[face]==Some(true) {faces[face].vertices.reverse();}}
        let origin=position(&faces[start].vertices[0]);
        let signed:f32=component.iter().map(|&i| {
            let v=&faces[i].vertices;(position(&v[0])-origin).dot((position(&v[1])-origin).cross(position(&v[2])-origin))/6.
        }).sum();
        if signed<0. {for &face in &component {faces[face].vertices.reverse();}}
    }
    let mut solids = vec![Solid(faces)]; let mut random = Random(u64::from(seed).wrapping_add(0x9e3779b97f4a7c15));
    while solids.len() < pieces {
        if cancel.load(Ordering::Relaxed) { return Err("Разрушение отменено".into()); }
        let index = (0..solids.len()).max_by(|&a, &b| solids[a].volume().total_cmp(&solids[b].volume())).unwrap();
        let source = solids.remove(index); let (lo, hi) = source.bounds();
        let extent = hi - lo;
        let axis = if extent.x >= extent.y && extent.x >= extent.z { Vec3::X } else if extent.y >= extent.z { Vec3::Y } else { Vec3::Z };
        let mut split = None;
        let source_volume = source.volume();
        for _ in 0..24 {
            let jitter = Vec3::new(random.next() - 0.5, random.next() - 0.5, random.next() - 0.5);
            let n = (axis + jitter * 1.3).normalize();
            let d = n.dot((lo + hi) * 0.5) + (random.next() - 0.5) * extent.min_element() * 0.25;
            let a = source.clip(n, d, epsilon, interior, cut_contrast, cancel)?;
            let b = source.clip(-n, -d, epsilon, interior, cut_contrast, cancel)?;
            if let (Some(a), Some(b)) = (a, b) {
                if a.volume() > source_volume * 0.025 && b.volume() > source_volume * 0.025
                    && (a.volume() + b.volume() - source_volume).abs() < source_volume * 0.001 { split = Some((a, b)); break; }
            }
        }
        let (a, b) = split.ok_or("Не удалось получить устойчивый срез; уменьшите число осколков")?;
        solids.extend([a, b]);
        if solids.iter().flat_map(|s| &s.0).map(|p| p.vertices.len()).sum::<usize>() > 500000 {
            return Err("Слишком сложное разрушение; уменьшите число осколков".into());
        }
    }
    solids.into_iter().enumerate().map(|(i, solid)| solid.into_fragment(mesh, i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PrimitiveKind, PrimitiveSettings};
    fn volume(mesh: &Mesh) -> f32 {
        mesh.indices.chunks_exact(3).map(|t| {
            let a = position(&mesh.vertices[t[0] as usize]);
            let b = position(&mesh.vertices[t[1] as usize]);
            let c = position(&mesh.vertices[t[2] as usize]);
            a.dot(b.cross(c)) / 6.
        }).sum::<f32>().abs()
    }
    fn closed(mesh: &Mesh) -> bool {
        // Shared corners are welded exactly; coarse rounding merges distinct thin edges.
        let key = |index: u32| mesh.vertices[index as usize].position.map(f32::to_bits);
        let mut edges = BTreeMap::new();
        for face in 0..mesh.face_count() {
            let ring = mesh.face_vertices(face).unwrap();
            for i in 0..ring.len() {
                let a = key(ring[i]); let b = key(ring[(i + 1) % ring.len()]);
                if a != b { *edges.entry((a.min(b), a.max(b))).or_insert(0usize) += 1; }
            }
        }
        edges.values().all(|&count| count == 2)
    }
    #[test]
    fn concave_edited_mesh_preserves_volume_instead_of_replacing_with_hull() {
        let outline = [[0.,0.],[2.,0.],[2.,1.],[1.,1.],[1.,2.],[0.,2.]];
        let vertices: Vec<_> = [0.,1.].into_iter().flat_map(|y| outline.iter().map(move |p| Vertex {
            position: [p[0],y,p[1]], ..Default::default()
        })).collect();
        let mut faces = vec![vec![0,1,2,3,4,5],vec![11,10,9,8,7,6]];
        for i in 0..6u32 { let j=(i+1)%6; faces.push(vec![i,i+6,j+6,j]); }
        let mut mesh = Mesh { id: 1, name: "Concave L".into(), vertices: vertices.into(), indices: Default::default(),
            topology: Some(MeshTopology { loose_edges: Vec::new(), faces: faces.into(), face_materials: Default::default(), uv_seams:vec![],primitive:None,smooth_normals:false }),
            material: None,morph_targets:vec![] };
        mesh.rebuild_polygon_indices().unwrap(); let original=mesh.clone();
        for seed in [1,19,72] {
            let parts=fracture_mesh(&mesh,8,seed,&AtomicBool::new(false)).unwrap();
            assert_eq!(parts.len(),8);
            assert!((parts.iter().map(|p|volume(&p.mesh)).sum::<f32>()-3.).abs()<0.003);
            assert!(parts.iter().all(|p|closed(&p.mesh)));
            for part in parts {for triangle in part.mesh.indices.chunks_exact(3) {
                let centroid=triangle.iter().map(|&i|position(&part.mesh.vertices[i as usize])+part.center).sum::<Vec3>()/3.;
                assert!(centroid.x<=1.001 || centroid.z<=1.001,"cap filled the missing corner");
            }}
        }
        assert_eq!(mesh,original);
    }
    #[test]
    fn cuts_are_closed_conserve_volume_preserve_material_and_are_repeatable() {
        let mesh = PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2, Some(7)).unwrap();
        let original = mesh.clone();
        for pieces in [2, 12, 32] {
            let parts = fracture_mesh(&mesh, pieces, 19, &AtomicBool::new(false)).unwrap();
            let again = fracture_mesh(&mesh, pieces, 19, &AtomicBool::new(false)).unwrap();
            assert_eq!(parts.len(), pieces);
            assert!((parts.iter().map(|f| volume(&f.mesh)).sum::<f32>() - volume(&mesh)).abs() < 0.0001);
            for (part, repeat) in parts.iter().zip(again) {
                part.mesh.validate_topology().unwrap();
                assert!(closed(&part.mesh));
                assert_eq!(part.mesh.material, Some(7));
                assert_eq!(part.mesh, repeat.mesh);
                assert_eq!(part.center, repeat.center);
            }
        }
        assert_eq!(mesh, original);
    }
    #[test]
    fn sphere_uses_its_geometry_and_cancellation_never_changes_input() {
        let mesh = PrimitiveSettings::new(PrimitiveKind::Sphere).mesh(2, None).unwrap();
        let parts = fracture_mesh(&mesh, 8, 3, &AtomicBool::new(false)).unwrap();
        assert!((parts.iter().map(|f| volume(&f.mesh)).sum::<f32>() - volume(&mesh)).abs() < 0.001);
        assert!(parts.iter().all(|f| closed(&f.mesh)));
        assert!(fracture_mesh(&mesh, 8, 3, &AtomicBool::new(true)).is_err());
        assert!(fracture_mesh(&mesh, 65, 3, &AtomicBool::new(false)).is_err());
        let plane = PrimitiveSettings::new(PrimitiveKind::Plane).mesh(2, None).unwrap();
        assert!(fracture_mesh(&plane, 4, 3, &AtomicBool::new(false)).is_err());
    }

    #[test]
    fn cut_contrast_changes_only_interior_colors_and_preserves_closed_geometry() {
        let mut mesh = crate::box_mesh(2, "Painted", Vec3::ONE, Some(9));
        for v in &mut mesh.vertices { v.color = [0.8, 0.4, 0.2, 1.]; }
        let plain = fracture_mesh_with_cuts(&mesh, 4, 7, 0., &AtomicBool::new(false)).unwrap();
        let dark = fracture_mesh_with_cuts(&mesh, 4, 7, 1., &AtomicBool::new(false)).unwrap();
        let mut unchanged = 0; let mut changed = 0;
        for (a, b) in plain.iter().zip(&dark) {
            assert_eq!(a.center, b.center); assert_eq!(a.mesh.indices, b.mesh.indices);
            assert!(closed(&b.mesh));
            for (x, y) in a.mesh.vertices.iter().zip(&b.mesh.vertices) {
                assert_eq!(x.position, y.position); assert_eq!(x.normal, y.normal); assert_eq!(x.color[3], y.color[3]);
                if x.color == y.color { unchanged += 1; }
                else { changed += 1; assert!(y.color[0] < x.color[0]); }
            }
            // Unpainted exterior faces keep inheriting object material layers.
            assert!(b.mesh.topology.as_ref().unwrap().face_materials.iter().any(Option::is_none));
        }
        assert!(unchanged > 0 && changed > 0);
        assert!(fracture_mesh_with_cuts(&mesh, 4, 7, f32::NAN, &AtomicBool::new(false)).is_err());
    }


    #[test]
    fn cut_material_survives_repeated_splits_and_outer_paint_is_preserved() {
        let mut mesh=crate::box_mesh(1,"Painted",Vec3::splat(0.5),Some(41));
        let faces=mesh.face_count();let mut materials=vec![None;faces];materials[0]=Some(42);
        mesh.topology=Some(MeshTopology { loose_edges: Vec::new(),faces:mesh.indices.chunks_exact(3).map(|t|t.to_vec()).collect::<Vec<_>>().into(),
            face_materials:materials.into(),uv_seams:vec![],primitive:None,smooth_normals:false});let before=mesh.clone();
        let shards=fracture_mesh_with_material(&mesh,12,19,0.7,Some(43),&AtomicBool::new(false)).unwrap();
        let mut exterior=0;let mut cuts=0;let mut painted=0;
        for shard in &shards {for face in 0..shard.mesh.face_count() {
            let points:Vec<_>=shard.mesh.face_vertices(face).unwrap().iter().map(|&i|
                position(&shard.mesh.vertices[i as usize])+shard.center).collect();
            let is_outer=(0..3).any(|axis|[-0.5_f32,0.5].iter().any(|side|points.iter().all(|p|(p[axis]-side).abs()<1e-5)));
            let material=shard.mesh.face_material(face);
            if is_outer {exterior+=1;assert!(material==Some(41)||material==Some(42), "outer material={material:?}, points={points:?}");if material==Some(42) {painted+=1;}}
            else {cuts+=1;assert_eq!(material,Some(43));}
        }}
        assert!(exterior>0 && cuts>0 && painted>0);assert_eq!(mesh,before);
    }
}
