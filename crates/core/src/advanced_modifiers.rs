//! Instance-isolated copy transforms and a reversible bevel modifier kernel.
use crate::modifiers::{cancelled, geometry_budget};
use crate::{CoreError, Mat4, Mesh, MeshConnectivity, MeshEdit, MeshTopology, Vec3};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};

pub(crate) fn copies(
    source: &Mesh,
    frames: &[Mat4],
    groups: &mut Vec<u64>,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    geometry_budget(
        source.vertices.len().saturating_mul(frames.len()),
        (source.indices.len() / 3).saturating_mul(frames.len()),
    )?;
    let stride = groups.iter().copied().max().unwrap_or(0) + 1;
    let mut mesh = source.clone();
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    let mut seams = Vec::new();
    let mut wires = Vec::new();
    let mut output_groups = Vec::new();
    for (copy, frame) in frames.iter().enumerate() {
        cancelled(cancel)?;
        if !frame.is_finite() || frame.determinant().abs() < 1e-12 {
            return Err(CoreError::Invalid(
                "Недопустимая трансформация копии".into(),
            ));
        }
        let start = vertices.len() as u32;
        let normal = frame.inverse().transpose();
        for (i, v) in source.vertices.iter().enumerate() {
            if i % 4096 == 0 {
                cancelled(cancel)?;
            }
            let mut v = *v;
            v.position = frame
                .transform_point3(Vec3::from_array(v.position))
                .to_array();
            v.normal = normal
                .transform_vector3(Vec3::from_array(v.normal))
                .normalize_or_zero()
                .to_array();
            if !Vec3::from_array(v.position).is_finite()
                || Vec3::from_array(v.position).abs().max_element() > 1e8
            {
                return Err(CoreError::Invalid(
                    "Массив выходит за пределы координат".into(),
                ));
            }
            vertices.push(v);
            output_groups.push(groups[i] + copy as u64 * stride);
        }
        for f in 0..source.face_count() {
            let mut face: Vec<_> = source
                .face_vertices(f)
                .unwrap()
                .iter()
                .map(|v| v + start)
                .collect();
            if frame.determinant() < 0. {
                face.reverse();
            }
            faces.push(face);
            materials.push(source.face_material(f));
        }
        if let Some(t) = &source.topology {
            seams.extend(t.uv_seams.iter().map(|e| e.map(|v| v + start)));
            wires.extend(t.loose_edges.iter().map(|e| e.map(|v| v + start)));
        }
    }
    mesh.vertices = vertices.into();
    mesh.topology = Some(MeshTopology {
        faces: faces.into(),
        face_materials: materials.into(),
        uv_seams: seams,
        loose_edges: wires,
        smooth_normals: source.smooth_normals_enabled(),
        primitive: None,
    });
    mesh.rebuild_polygon_indices()?;
    mesh.validate_topology()?;
    *groups = output_groups;
    Ok(mesh)
}

pub(crate) fn bevel(
    source: &Mesh,
    width: f32,
    segments: u32,
    profile: f32,
    angle: f32,
    groups: &mut Vec<u64>,
    cancel: &AtomicBool,
) -> Result<Mesh, CoreError> {
    if width == 0. {
        return Ok(source.clone());
    }
    let mut parts = Vec::new();
    for group in groups.iter().copied().collect::<BTreeSet<_>>() {
        cancelled(cancel)?;
        let ids: Vec<_> = (0..source.vertices.len())
            .filter(|&i| groups[i] == group)
            .collect();
        let remap: BTreeMap<_, _> = ids
            .iter()
            .enumerate()
            .map(|(new, &old)| (old, new as u32))
            .collect();
        let mut mesh = source.clone();
        mesh.vertices = ids.iter().map(|&i| source.vertices[i]).collect();
        let selected_faces: Vec<_> = (0..source.face_count())
            .filter(|&f| {
                source
                    .face_vertices(f)
                    .unwrap()
                    .iter()
                    .all(|&v| remap.contains_key(&(v as usize)))
            })
            .collect();
        let edges = |list: &Vec<[u32; 2]>| {
            list.iter()
                .filter_map(|e| {
                    Some([*remap.get(&(e[0] as usize))?, *remap.get(&(e[1] as usize))?])
                })
                .collect()
        };
        mesh.topology = Some(MeshTopology {
            faces: selected_faces
                .iter()
                .map(|&f| {
                    source
                        .face_vertices(f)
                        .unwrap()
                        .iter()
                        .map(|&v| remap[&(v as usize)])
                        .collect()
                })
                .collect(),
            face_materials: selected_faces
                .iter()
                .map(|&f| source.face_material(f))
                .collect(),
            uv_seams: source
                .topology
                .as_ref()
                .map_or_else(Vec::new, |t| edges(&t.uv_seams)),
            loose_edges: source
                .topology
                .as_ref()
                .map_or_else(Vec::new, |t| edges(&t.loose_edges)),
            smooth_normals: source.smooth_normals_enabled(),
            primitive: None,
        });
        mesh.rebuild_polygon_indices()?;
        let graph = MeshConnectivity::new(&mesh);
        let blocked: BTreeSet<_> = graph
            .edge_faces
            .iter()
            .filter(|(_, f)| f.len() != 2)
            .flat_map(|(&(a, b), _)| [a, b])
            .collect();
        let selected: BTreeSet<_> = graph
            .edge_faces
            .iter()
            .filter_map(|(&e, fs)| {
                if fs.len() != 2 || blocked.contains(&e.0) || blocked.contains(&e.1) {
                    return None;
                }
                let normal = |f| {
                    crate::polygon_edit::face_normal(&mesh, mesh.face_vertices(f).unwrap())
                        .normalize_or_zero()
                };
                let dot = normal(fs[0]).dot(normal(fs[1])).clamp(-1., 1.);
                (dot.acos() >= angle.max(0.0001) && dot > -0.99999).then_some(e)
            })
            .collect();
        if !selected.is_empty() {
            let affected: BTreeSet<_> = selected.iter().flat_map(|&(a, b)| [a, b]).collect();
            let limit = graph
                .edges
                .iter()
                .filter(|(a, b)| affected.contains(a) || affected.contains(b))
                .map(|&(a, b)| {
                    Vec3::from_array(mesh.vertices[a].position)
                        .distance(Vec3::from_array(mesh.vertices[b].position))
                        * 0.2
                })
                .fold(width, f32::min);
            let original = mesh.clone();
            let mut applied = None;
            let mut failure = None;
            for attempt in 0..7 {
                cancelled(cancel)?;
                let mut candidate = original.clone();
                match crate::authoring::apply_mesh_edit(
                    &mut candidate,
                    MeshEdit::EdgeBevel {
                        edges: selected.clone(),
                        width: limit * 0.5_f32.powi(attempt),
                        segments,
                        profile,
                    },
                ) {
                    Ok(()) => {
                        applied = Some(candidate);
                        break;
                    }
                    Err(e) => failure = Some(e),
                }
            }
            mesh = applied.ok_or_else(|| failure.unwrap())?;
        }
        parts.push((group, mesh));
    }
    let mut output = source.clone();
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut materials = Vec::new();
    let mut seams = Vec::new();
    let mut wires = Vec::new();
    let mut next_groups = Vec::new();
    for (group, mesh) in parts {
        let start = vertices.len() as u32;
        next_groups.extend(std::iter::repeat_n(group, mesh.vertices.len()));
        vertices.extend(mesh.vertices.iter().copied());
        for f in 0..mesh.face_count() {
            faces.push(
                mesh.face_vertices(f)
                    .unwrap()
                    .iter()
                    .map(|v| v + start)
                    .collect(),
            );
            materials.push(mesh.face_material(f));
        }
        if let Some(t) = mesh.topology {
            seams.extend(t.uv_seams.iter().map(|e| e.map(|v| v + start)));
            wires.extend(t.loose_edges.iter().map(|e| e.map(|v| v + start)));
        }
    }
    output.vertices = vertices.into();
    output.topology = Some(MeshTopology {
        faces: faces.into(),
        face_materials: materials.into(),
        uv_seams: seams,
        loose_edges: wires,
        smooth_normals: source.smooth_normals_enabled(),
        primitive: None,
    });
    output.rebuild_polygon_indices()?;
    geometry_budget(output.vertices.len(), output.indices.len() / 3)?;
    output.validate_topology()?;
    *groups = next_groups;
    Ok(output)
}
