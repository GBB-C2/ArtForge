//! Connected face bevels keep selected interior edges shared by their caps.
use super::*;
use crate::FaceTreatment;

pub(super) fn apply(
    mesh: &mut Mesh,
    selected: &BTreeSet<usize>,
    inset: f32,
    shift: f32,
    segments: u32,
    profile: f32,
    mode: FaceTreatment,
) -> Result<(), CoreError> {
    if inset == 0. && shift == 0. {
        return Ok(());
    }
    let graph = MeshConnectivity::new(mesh);
    let outline: BTreeSet<_> = graph
        .edge_faces
        .iter()
        .filter_map(|(&edge, faces)| {
            (faces.iter().filter(|f| selected.contains(f)).count() == 1).then_some(edge)
        })
        .collect();
    // A completely selected closed shell still supports rounding its sharp edges.
    // A partial region only rounds its perimeter, never a shared selected edge.
    let boundary: BTreeSet<_> = graph
        .edge_faces
        .iter()
        .filter_map(|(&edge, faces)| {
            let selected_edge = if outline.is_empty() {
                faces.iter().any(|f| selected.contains(f))
            } else {
                outline.contains(&edge)
            };
            (selected_edge
                && (faces.len() != 2 || {
                    let normal = |f| {
                        crate::polygon_edit::face_normal(mesh, mesh.face_vertices(f).unwrap())
                            .normalize_or_zero()
                    };
                    normal(faces[0]).dot(normal(faces[1])) < 0.9999
                }))
            .then_some(edge)
        })
        .collect();
    let affected: BTreeSet<_> = boundary.iter().flat_map(|&(a, b)| [a, b]).collect();
    let closed = graph.edge_faces.iter().all(|(&(a, b), faces)| {
        (!affected.contains(&a) && !affected.contains(&b)) || faces.len() == 2
    });
    if closed && !boundary.is_empty() && inset > 0. {
        let mut lengths: Vec<_> = boundary
            .iter()
            .map(|&(a, b)| {
                Vec3::from_array(mesh.vertices[a].position)
                    .distance(Vec3::from_array(mesh.vertices[b].position))
            })
            .collect();
        lengths.sort_by(f32::total_cmp);
        crate::edge_bevel::apply(
            mesh,
            &boundary,
            lengths[lengths.len() / 2] * inset * 0.5,
            segments,
            profile,
        )?;
        if shift != 0. {
            let (graph, directions) = cap_directions(mesh, selected, mode)?;
            for (i, vertex) in mesh.vertices.iter_mut().enumerate() {
                if let Some(direction) = directions.get(&graph.canonical[i]) {
                    vertex.position =
                        (Vec3::from_array(vertex.position) + *direction * shift).to_array();
                }
            }
            finish(mesh, mesh.smooth_normals_enabled())?;
        }
        return Ok(());
    }
    surface(mesh, selected, inset, shift, segments, profile, mode)
}

fn cap_directions(
    mesh: &Mesh,
    selected: &BTreeSet<usize>,
    mode: FaceTreatment,
) -> Result<(MeshConnectivity, BTreeMap<usize, Vec3>), CoreError> {
    let graph = MeshConnectivity::new(mesh);
    let mut pending = selected.clone();
    let mut directions = BTreeMap::new();
    while let Some(seed) = pending.pop_first() {
        let mut stack = vec![seed];
        let mut normal = Vec3::ZERO;
        let mut normals = BTreeMap::<usize, Vec3>::new();
        while let Some(f) = stack.pop() {
            let face = mesh.face_vertices(f).unwrap();
            let n = crate::polygon_edit::face_normal(mesh, face);
            normal += n;
            for i in 0..face.len() {
                let a = graph.canonical[face[i] as usize];
                let b = graph.canonical[face[(i + 1) % face.len()] as usize];
                *normals.entry(a).or_default() += n;
                if let Some(adjacent) = graph.edge_faces.get(&(a.min(b), a.max(b))) {
                    for &next in adjacent {
                        if pending.remove(&next) {
                            stack.push(next);
                        }
                    }
                }
            }
        }
        let region_normal = normal.try_normalize();
        if mode == FaceTreatment::Region && region_normal.is_none() {
            return Err(invalid(
                "У области нет общего направления; выберите «По нормалям»",
            ));
        }
        for (v, n) in normals {
            directions.insert(
                v,
                if mode == FaceTreatment::Region {
                    region_normal.unwrap()
                } else {
                    n.normalize_or_zero()
                },
            );
        }
    }
    Ok((graph, directions))
}

/// Open surfaces get one strip around each connected island, including holes.
/// Shared corner directions keep segmented profiles joined across UV seams.
fn surface(
    mesh: &mut Mesh,
    selected: &BTreeSet<usize>,
    inset: f32,
    shift: f32,
    segments: u32,
    profile: f32,
    mode: FaceTreatment,
) -> Result<(), CoreError> {
    let original = mesh.clone();
    let (graph, directions) = cap_directions(&original, selected, mode)?;
    crate::face_inset::apply(mesh, selected, inset, shift, mode)?;
    if segments == 1 {
        return Ok(());
    }
    let topology = mesh.topology.as_ref().unwrap();
    let rings = topology.faces.clone();
    let wall_count = rings.len() - original.face_count();
    let extra_corners = wall_count.saturating_mul(2 * (segments as usize - 1));
    if mesh.vertices.len().saturating_add(extra_corners) > crate::MAX_EDIT_VERTICES
        || (mesh.indices.len() / 3).saturating_add(extra_corners) > crate::MAX_EDIT_TRIANGLES
    {
        return Err(invalid(
            "Фаска превышает предел сетки; уменьшите сегменты или выделение",
        ));
    }
    let mut faces = rings[..original.face_count()].to_vec();
    let mut materials: Vec<_> = (0..original.face_count())
        .map(|f| mesh.face_material(f))
        .collect();
    let mut cache = BTreeMap::new();
    for (index, ring) in rings[original.face_count()..].iter().enumerate() {
        // finish() may rotate a quad to choose a valid rendering diagonal.
        let start = (0..4)
            .find(|&i| {
                (ring[i] as usize) < original.vertices.len()
                    && (ring[(i + 1) % 4] as usize) < original.vertices.len()
            })
            .ok_or_else(|| invalid("Не найден контур фаски"))?;
        let ring: Vec<_> = (0..4).map(|i| ring[(start + i) % 4]).collect();
        let mut lines = [vec![ring[0]], vec![ring[1]]];
        for (side, (a, b)) in [(ring[0], ring[3]), (ring[1], ring[2])]
            .into_iter()
            .enumerate()
        {
            let normal = directions[&graph.canonical[a as usize]];
            for i in 1..segments {
                let id = *cache.entry((a, b, i)).or_insert_with(|| {
                    let t = i as f32 / segments as f32;
                    let (va, vb) = (mesh.vertices[a as usize], mesh.vertices[b as usize]);
                    let (start, end) =
                        (Vec3::from_array(va.position), Vec3::from_array(vb.position));
                    let delta = end - start;
                    let radius = (delta - normal * delta.dot(normal)).length() * 0.25;
                    let mut vertex = va;
                    vertex.position = (start.lerp(end, t)
                        + normal * (profile * radius * (std::f32::consts::PI * t).sin()))
                    .to_array();
                    vertex.uv = std::array::from_fn(|j| va.uv[j] * (1. - t) + vb.uv[j] * t);
                    vertex.color =
                        std::array::from_fn(|j| va.color[j] * (1. - t) + vb.color[j] * t);
                    let id = mesh.vertices.len() as u32;
                    mesh.vertices.push(vertex);
                    id
                });
                lines[side].push(id);
            }
            lines[side].push(b);
        }
        for i in 0..segments as usize {
            faces.push(vec![
                lines[0][i],
                lines[1][i],
                lines[1][i + 1],
                lines[0][i + 1],
            ]);
            materials.push(mesh.face_material(original.face_count() + index));
        }
    }
    let topology = mesh.topology.as_mut().unwrap();
    topology.faces = faces.into();
    topology.face_materials = materials.into();
    finish(mesh, original.smooth_normals_enabled())
}
