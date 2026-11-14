//! Region extrusion with adjacent-wall trimming and planar-edge dissolution.
//! Geometry is rebuilt from the source for each preview, never from a previous result.
use crate::{CoreError, Mesh, MeshConnectivity, MeshEdit, Vec3, Vertex};
use std::collections::{BTreeMap, BTreeSet};
fn invalid(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
fn pos(v: Vertex) -> Vec3 {
    Vec3::from_array(v.position)
}
fn lerp(a: Vertex, b: Vertex, t: f32) -> Vertex {
    let mut v = a;
    v.position = pos(a).lerp(pos(b), t).to_array();
    v.normal = Vec3::from_array(a.normal)
        .lerp(Vec3::from_array(b.normal), t)
        .normalize_or_zero()
        .to_array();
    v.uv = std::array::from_fn(|i| a.uv[i] * (1. - t) + b.uv[i] * t);
    v.color = std::array::from_fn(|i| a.color[i] * (1. - t) + b.color[i] * t);
    v
}
fn split(poly: &[Vertex], origin: Vec3, normal: Vec3) -> (Vec<Vertex>, Vec<Vertex>) {
    let (mut inside, mut outside) = (Vec::new(), Vec::new());
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let da = (pos(a) - origin).dot(normal);
        let db = (pos(b) - origin).dot(normal);
        if da >= -1e-6 {
            inside.push(a);
        }
        if da <= 1e-6 {
            outside.push(a);
        }
        if (da > 1e-6 && db < -1e-6) || (da < -1e-6 && db > 1e-6) {
            let v = lerp(a, b, da / (da - db));
            inside.push(v);
            outside.push(v);
        }
    }
    (inside, outside)
}
fn area(poly: &[Vertex]) -> f32 {
    if poly.len() < 3 {
        return 0.;
    }
    let a = pos(poly[0]);
    (1..poly.len() - 1)
        .map(|i| (pos(poly[i]) - a).cross(pos(poly[i + 1]) - a))
        .sum::<Vec3>()
        .length()
}
fn subtract(poly: &[Vertex], a: Vec3, b: Vec3, delta: Vec3) -> Vec<Vec<Vertex>> {
    let u = (b - a).normalize();
    let v = delta.normalize();
    let mut inside = poly.to_vec();
    let mut result = Vec::new();
    for (origin, n) in [(a, u), (b, -u), (a, v), (a + delta, -v)] {
        if inside.len() < 3 {
            break;
        }
        let (next, outside) = split(&inside, origin, n);
        if area(&outside) > 1e-10 {
            result.push(outside);
        }
        inside = next;
    }
    result
}
pub(crate) fn apply(
    mesh: &mut Mesh,
    selected: &BTreeSet<usize>,
    distance: f32,
) -> Result<(), CoreError> {
    if selected.is_empty()
        || selected.iter().any(|&f| f >= mesh.face_count())
        || !distance.is_finite()
        || distance.abs() > 1000.
    {
        return Err(invalid(
            "Выберите грани; расстояние распределительного экструда — от −1000 до 1000 м",
        ));
    }
    if distance.abs() < 1e-6 {
        return Ok(());
    }
    let source = mesh.clone();
    let graph = MeshConnectivity::new(&source);
    let normal = selected
        .iter()
        .map(|&f| crate::polygon_edit::face_normal(&source, source.face_vertices(f).unwrap()))
        .sum::<Vec3>()
        .try_normalize()
        .ok_or_else(|| invalid("У области нет общего направления экструдирования"))?;
    let delta = normal * distance;
    let mut trims = BTreeMap::<usize, Vec<(Vec3, Vec3)>>::new();
    if distance < 0. {
        for (&(a, b), fs) in &graph.edge_faces {
            if fs.len() != 2 || fs.iter().filter(|f| selected.contains(f)).count() != 1 {
                continue;
            }
            let other = *fs.iter().find(|f| !selected.contains(f)).unwrap();
            let n = crate::polygon_edit::face_normal(&source, source.face_vertices(other).unwrap())
                .normalize_or_zero();
            if n.dot(normal).abs() > 1e-4 {
                continue;
            }
            let a = Vec3::from_array(source.vertices[a].position);
            let b = Vec3::from_array(source.vertices[b].position);
            if !crate::knife_interior::contains(&source, other, a + delta * 1.0001)
                || !crate::knife_interior::contains(&source, other, b + delta * 1.0001)
            {
                return Err(invalid(
                    "Распределительный экструд достигает противоположного края; уменьшите глубину",
                ));
            }
            trims.entry(other).or_default().push((a, b));
        }
    }
    crate::authoring::apply_mesh_edit(
        mesh,
        MeshEdit::Extrude {
            faces: selected.clone(),
            distance,
        },
    )?;
    let smooth = mesh.smooth_normals_enabled();
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    let mut touched = BTreeSet::new();
    for f in 0..mesh.face_count() {
        let ring = mesh.face_vertices(f).unwrap().to_vec();
        let mat = mesh.face_material(f);
        // The inward extrusion wall lies on a neighbouring wall; its swept area
        // is removed from that neighbour, leaving a shared edge at the new cap.
        if f >= source.face_count()
            && trims.values().flatten().any(|&(a, b)| {
                ring.iter().all(|&i| {
                    let p = Vec3::from_array(mesh.vertices[i as usize].position);
                    [a, b, a + delta, b + delta]
                        .iter()
                        .any(|q| p.distance_squared(*q) < 1e-10)
                })
            })
        {
            continue;
        }
        if let Some(rectangles) = trims.get(&f) {
            let tris = crate::triangulate::triangulate(mesh, &ring)?;
            for tri in tris.chunks_exact(3) {
                let mut pieces = vec![tri
                    .iter()
                    .map(|&i| mesh.vertices[i as usize])
                    .collect::<Vec<_>>()];
                for &(a, b) in rectangles {
                    pieces = pieces
                        .iter()
                        .flat_map(|p| subtract(p, a, b, delta))
                        .collect();
                }
                for poly in pieces {
                    if mesh.vertices.len() + poly.len() > crate::MAX_EDIT_VERTICES {
                        return Err(invalid("Слишком много вершин"));
                    }
                    let ids = poly
                        .into_iter()
                        .map(|v| {
                            let id = mesh.vertices.len() as u32;
                            mesh.vertices.push(v);
                            id
                        })
                        .collect();
                    touched.insert(faces.len());
                    faces.push(ids);
                    materials.push(mat);
                }
            }
        } else {
            if f >= source.face_count() {
                touched.insert(faces.len());
            }
            faces.push(ring);
            materials.push(mat);
        }
    }
    // Insert every newly created point into edges which span it. This stitches
    // the trimmed triangles to cap polygons and neighbouring material/UV seams.
    let mut unique = BTreeMap::new();
    for &f in &touched {
        for &i in &faces[f] {
            let p = mesh.vertices[i as usize].position;
            unique.insert(crate::position_key(p), Vec3::from_array(p));
        }
    }
    let points: Vec<_> = unique.into_values().collect();
    if faces
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        .saturating_mul(points.len())
        > 8_000_000
    {
        return Err(invalid(
            "Слишком большая область распределительного экструда; уменьшите выделение",
        ));
    }
    for ring in &mut faces {
        let mut expanded = Vec::new();
        for j in 0..ring.len() {
            let x = ring[j];
            let y = ring[(j + 1) % ring.len()];
            expanded.push(x);
            let a = mesh.vertices[x as usize];
            let b = mesh.vertices[y as usize];
            let d = pos(b) - pos(a);
            let len = d.length_squared();
            let mut mids: Vec<_> = points
                .iter()
                .filter_map(|&p| {
                    let t = (p - pos(a)).dot(d) / len;
                    ((1e-5..1. - 1e-5).contains(&t) && p.distance_squared(pos(a) + d * t) < 1e-10)
                        .then_some(t)
                })
                .collect();
            mids.sort_by(f32::total_cmp);
            mids.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
            for t in mids {
                let id = mesh.vertices.len() as u32;
                mesh.vertices.push(lerp(a, b, t));
                expanded.push(id);
            }
        }
        *ring = expanded;
    }
    let topology = mesh.topology.as_mut().unwrap();
    topology.faces = faces.into();
    topology.face_materials = materials.into();
    topology.primitive = None;
    topology.uv_seams.clear();
    mesh.rebuild_polygon_indices()?;
    let graph = MeshConnectivity::new(mesh);
    let edges: BTreeSet<_> = graph
        .edge_faces
        .iter()
        .filter_map(|(&e, fs)| {
            if fs.len() != 2
                || !fs.iter().any(|f| touched.contains(f))
                || mesh.face_material(fs[0]) != mesh.face_material(fs[1])
            {
                return None;
            }
            let n = |f| {
                crate::polygon_edit::face_normal(mesh, mesh.face_vertices(f).unwrap())
                    .normalize_or_zero()
            };
            (n(fs[0]).dot(n(fs[1])) > 0.99999).then_some(e)
        })
        .collect();
    if !edges.is_empty() {
        let mut candidate = mesh.clone();
        if crate::mesh_topology_tools::dissolve(&mut candidate, &edges).is_ok() {
            *mesh = candidate;
        }
    }
    let faces = crate::mesh_topology_tools::faces(mesh);
    crate::mesh_topology_tools::finish(mesh, faces, smooth)?;
    mesh.validate_topology()
}
