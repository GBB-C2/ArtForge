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
fn subtract(poly: &[Vertex], clip: &[Vec3]) -> Vec<Vec<Vertex>> {
    let normal = (clip[1] - clip[0])
        .cross(clip[2] - clip[0])
        .normalize_or_zero();
    let mut inside = poly.to_vec();
    let mut result = Vec::new();
    for i in 0..clip.len() {
        if inside.len() < 3 {
            break;
        }
        let origin = clip[i];
        let n = normal
            .cross(clip[(i + 1) % clip.len()] - origin)
            .normalize_or_zero();
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
    let mut trims = BTreeMap::<usize, Vec<Vec<Vec3>>>::new();
    let mut sweeps = Vec::new();
    for &face in selected {
        let ring = source.face_vertices(face).unwrap();
        for i in 0..ring.len() {
            let (a, b) = (
                graph.canonical[ring[i] as usize],
                graph.canonical[ring[(i + 1) % ring.len()] as usize],
            );
            let Some(fs) = graph.edge_faces.get(&(a.min(b), a.max(b))) else {
                continue;
            };
            if fs.len() != 2 || fs.iter().filter(|f| selected.contains(f)).count() != 1 {
                continue;
            }
            let other = *fs.iter().find(|f| !selected.contains(f)).unwrap();
            let n = crate::polygon_edit::face_normal(&source, source.face_vertices(other).unwrap())
                .normalize_or_zero();
            let a = Vec3::from_array(source.vertices[a].position);
            let b = Vec3::from_array(source.vertices[b].position);
            // Cancel only opposite-facing overlap. The sweep can be oblique,
            // extend beyond the neighbouring wall, or cross several grid cells.
            if (b - a).cross(delta).normalize_or_zero().dot(n) > -0.99999 {
                continue;
            }
            // The neighbour can be a tessellated wall. Trim its whole coplanar
            // connected patch, not just the triangle incident to the source edge.
            let mut wall = BTreeSet::from([other]);
            let mut pending = vec![other];
            while let Some(f) = pending.pop() {
                for &next in &graph.face_neighbors[f] {
                    if selected.contains(&next) || wall.contains(&next) {
                        continue;
                    }
                    let ring = source.face_vertices(next).unwrap();
                    let nn = crate::polygon_edit::face_normal(&source, ring).normalize_or_zero();
                    if nn.dot(n) > 0.99999
                        && ring.iter().all(|&i| {
                            (Vec3::from_array(source.vertices[i as usize].position) - a)
                                .dot(n)
                                .abs()
                                < 1e-5
                        })
                    {
                        wall.insert(next);
                        pending.push(next);
                    }
                }
            }
            let sweep = vec![a, b, b + delta, a + delta];
            for &f in &wall {
                trims.entry(f).or_default().push(sweep.clone());
            }
            sweeps.push((sweep, wall));
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
    let mut origins = Vec::new();
    let mut touched = BTreeSet::new();
    for f in 0..mesh.face_count() {
        let ring = mesh.face_vertices(f).unwrap().to_vec();
        let mat = mesh.face_material(f);
        let mut cutters = trims.get(&f).cloned().unwrap_or_default();
        if f >= source.face_count() {
            if let Some((_, wall)) = sweeps.iter().find(|(sweep, _)| {
                ring.iter().all(|&i| {
                    sweep
                        .iter()
                        .any(|q| pos(mesh.vertices[i as usize]).distance_squared(*q) < 1e-10)
                })
            }) {
                // Keep the portion of a new wall outside the original surface.
                // Deleting the entire wall prevents inward extrusion past its edge.
                for &neighbor in wall {
                    let triangles = crate::triangulate::triangulate(
                        &source,
                        source.face_vertices(neighbor).unwrap(),
                    )?;
                    cutters.extend(triangles.chunks_exact(3).map(|tri| {
                        tri.iter()
                            .map(|&i| pos(source.vertices[i as usize]))
                            .collect()
                    }));
                }
            }
        }
        if !cutters.is_empty() {
            let tris = crate::triangulate::triangulate(mesh, &ring)?;
            for tri in tris.chunks_exact(3) {
                let mut pieces = vec![tri
                    .iter()
                    .map(|&i| mesh.vertices[i as usize])
                    .collect::<Vec<_>>()];
                for clip in &cutters {
                    pieces = pieces.iter().flat_map(|p| subtract(p, clip)).collect();
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
                    origins.push(f);
                }
            }
        } else {
            if f >= source.face_count() {
                touched.insert(faces.len());
            }
            faces.push(ring);
            materials.push(mat);
            origins.push(f);
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
                if mesh.vertices.len() >= crate::MAX_EDIT_VERTICES {
                    return Err(invalid("Слишком много вершин"));
                }
                let id = mesh.vertices.len() as u32;
                mesh.vertices.push(lerp(a, b, t));
                expanded.push(id);
            }
        }
        *ring = expanded;
    }
    // At an exact meeting with the opposite face, keep the moving cap once.
    let mut seen = BTreeMap::new();
    let mut remove = BTreeSet::new();
    for (i, ring) in faces.iter().enumerate() {
        let mut key: Vec<_> = ring
            .iter()
            .map(|&v| crate::position_key(mesh.vertices[v as usize].position))
            .collect();
        key.sort_unstable();
        key.dedup();
        if let Some(&previous) = seen.get(&key) {
            if selected.contains(&origins[i]) {
                remove.insert(previous);
                seen.insert(key, i);
            } else if selected.contains(&origins[previous]) {
                remove.insert(i);
            }
        } else {
            seen.insert(key, i);
        }
    }
    if !remove.is_empty() {
        let keep: Vec<_> = (0..faces.len()).filter(|f| !remove.contains(f)).collect();
        touched = keep
            .iter()
            .enumerate()
            .filter_map(|(i, f)| touched.contains(f).then_some(i))
            .collect();
        faces = keep.iter().map(|&i| faces[i].clone()).collect();
        materials = keep.iter().map(|&i| materials[i]).collect();
        origins = keep.iter().map(|&i| origins[i]).collect();
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
                // Keep existing grid edges and borders between distinct sweeps.
                // Only internal clipping edges and the old/new join dissolve.
                || (origins[fs[0]] != origins[fs[1]]
                    && (origins[fs[0]] < source.face_count()) == (origins[fs[1]] < source.face_count()))
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
    let graph = MeshConnectivity::new(mesh);
    let selected_vertices: BTreeSet<_> = selected
        .iter()
        .flat_map(|&f| source.face_vertices(f).unwrap())
        .map(|&v| crate::position_key(source.vertices[v as usize].position))
        .collect();
    let mut protected: BTreeSet<_> = source
        .vertices
        .iter()
        .map(|v| crate::position_key(v.position))
        .filter(|p| !selected_vertices.contains(p))
        .collect();
    protected.extend(
        selected
            .iter()
            .flat_map(|&f| source.face_vertices(f).unwrap())
            .map(|&v| crate::position_key((pos(source.vertices[v as usize]) + delta).to_array())),
    );
    let redundant: BTreeSet<_> = graph
        .vertex_neighbors
        .iter()
        .enumerate()
        .filter_map(|(v, neighbors)| {
            if neighbors.len() != 2
                || protected.contains(&crate::position_key(mesh.vertices[v].position))
            {
                return None;
            }
            let mut neighbors = neighbors.iter();
            let a = pos(mesh.vertices[*neighbors.next().unwrap()]) - pos(mesh.vertices[v]);
            let b = pos(mesh.vertices[*neighbors.next().unwrap()]) - pos(mesh.vertices[v]);
            (a.dot(b) < 0.
                && a.cross(b).length_squared() < 1e-10 * a.length_squared() * b.length_squared())
            .then_some(v)
        })
        .collect();
    let faces: Vec<Vec<_>> = crate::mesh_topology_tools::faces(mesh)
        .into_iter()
        .map(|ring| {
            ring.into_iter()
                .filter(|&v| !redundant.contains(&graph.canonical[v as usize]))
                .collect()
        })
        .collect();
    mesh.topology.as_mut().unwrap().faces = faces.clone().into();
    crate::mesh_topology_tools::finish(mesh, faces, smooth)?;
    mesh.validate_topology()
}
