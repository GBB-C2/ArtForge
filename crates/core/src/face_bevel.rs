use crate::mesh_modeling::{bevel, finish};
use crate::mesh_topology_tools::invalid;
use crate::{CoreError, Mesh, MeshConnectivity, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// The bevel wall is one shared strip per boundary edge. Intermediate rings
/// share their corner indices; no duplicate side polygons are appended.
pub(crate) fn apply(
    mesh: &mut Mesh,
    selection: &BTreeSet<usize>,
    inset: f32,
    shift: f32,
    segments: u32,
    profile: f32,
) -> Result<(), CoreError> {
    if !(1..=32).contains(&segments) || !profile.is_finite() || !(-1. ..=1.).contains(&profile) {
        return Err(invalid("Сегменты: 1–32; профиль: −100–100%"));
    }
    if selection.is_empty()
        || selection.iter().any(|&f| f >= mesh.face_count())
        || !inset.is_finite()
        || !(0. ..=0.95).contains(&inset)
        || !shift.is_finite()
        || shift.abs() > 1000.
    {
        return Err(invalid("Выберите грани; отступ 0–95%, высота −1000–1000 м"));
    }
    let graph = MeshConnectivity::new(mesh);
    let boundary: BTreeSet<_> = graph
        .edge_faces
        .iter()
        .filter_map(|(&e, fs)| {
            let count = fs.iter().filter(|f| selection.contains(f)).count();
            (count > 0
                && (fs.len() != 2
                    || crate::polygon_edit::face_normal(mesh, mesh.face_vertices(fs[0]).unwrap())
                        .normalize_or_zero()
                        .dot(
                            crate::polygon_edit::face_normal(
                                mesh,
                                mesh.face_vertices(fs[1]).unwrap(),
                            )
                            .normalize_or_zero(),
                        )
                        < 0.9999))
                .then_some(e)
        })
        .collect();
    let affected: BTreeSet<_> = boundary.iter().flat_map(|&(a, b)| [a, b]).collect();
    let closed = graph
        .edge_faces
        .iter()
        .all(|(&(a, b), fs)| (!affected.contains(&a) && !affected.contains(&b)) || fs.len() == 2);
    if closed && !boundary.is_empty() && inset > 0. {
        let length = |e: (usize, usize)| {
            Vec3::from_array(mesh.vertices[e.0].position)
                .distance(Vec3::from_array(mesh.vertices[e.1].position))
        };
        let requested = boundary
            .iter()
            .map(|&e| length(e))
            .fold(f32::INFINITY, f32::min)
            * inset
            * 0.5;
        let limit = graph
            .edges
            .iter()
            .filter(|(a, b)| affected.contains(a) || affected.contains(b))
            .map(|&e| length(e) * 0.2)
            .fold(f32::INFINITY, f32::min);
        crate::edge_bevel::apply(mesh, &boundary, requested.min(limit), segments, profile)?;
        if shift != 0. {
            let vertices = selection
                .iter()
                .flat_map(|&f| mesh.face_vertices(f).unwrap().iter().map(|&v| v as usize))
                .collect();
            let normal = selection
                .iter()
                .map(|&f| crate::polygon_edit::face_normal(mesh, mesh.face_vertices(f).unwrap()))
                .sum::<Vec3>()
                .normalize_or_zero();
            crate::authoring::apply_mesh_edit(
                mesh,
                crate::MeshEdit::Translate {
                    vertices,
                    delta: normal * shift,
                },
            )?;
        }
        return Ok(());
    }
    let corners: usize = selection
        .iter()
        .map(|&f| mesh.face_vertices(f).unwrap().len())
        .sum();
    if mesh
        .vertices
        .len()
        .saturating_add(corners.saturating_mul(segments as usize))
        > crate::MAX_EDIT_VERTICES
        || (mesh.indices.len() / 3).saturating_add(corners.saturating_mul(2 * segments as usize))
            > crate::MAX_EDIT_TRIANGLES
    {
        return Err(invalid(
            "Фаска превышает предел сетки; уменьшите сегменты или выделение",
        ));
    }
    let original = mesh.clone();
    bevel(mesh, selection, inset, shift)?;
    if segments == 1 || (inset == 0. && shift == 0.) {
        return Ok(());
    }
    let rings = mesh.topology.as_ref().unwrap().faces.clone();
    let materials = mesh.topology.as_ref().unwrap().face_materials.clone();
    let mut out = rings[..original.face_count()].to_vec();
    let mut mats: Vec<_> = (0..original.face_count())
        .map(|f| mesh.face_material(f))
        .collect();
    let mut sides = Vec::new();
    for &f in selection {
        let normal =
            crate::polygon_edit::face_normal(&original, original.face_vertices(f).unwrap())
                .normalize_or_zero();
        sides.extend(std::iter::repeat_n(
            normal,
            original.face_vertices(f).unwrap().len(),
        ));
    }
    let mut cache = BTreeMap::new();
    for (index, ring) in rings[original.face_count()..].iter().enumerate() {
        let normal = sides[index];
        let mut lines = [Vec::new(), Vec::new()];
        for (side, (a, b)) in [(ring[0], ring[3]), (ring[1], ring[2])]
            .into_iter()
            .enumerate()
        {
            lines[side].push(a);
            for i in 1..segments {
                let id = *cache.entry((a, b, i)).or_insert_with(|| {
                    let t = i as f32 / segments as f32;
                    let va = mesh.vertices[a as usize];
                    let vb = mesh.vertices[b as usize];
                    let start = Vec3::from_array(va.position);
                    let end = Vec3::from_array(vb.position);
                    let d = end - start;
                    let radius = (d - normal * d.dot(normal)).length() * 0.25;
                    let mut v = va;
                    v.position = (start.lerp(end, t)
                        + normal * (profile * radius * (std::f32::consts::PI * t).sin()))
                    .to_array();
                    v.uv = std::array::from_fn(|j| va.uv[j] * (1. - t) + vb.uv[j] * t);
                    v.color = std::array::from_fn(|j| va.color[j] * (1. - t) + vb.color[j] * t);
                    let id = mesh.vertices.len() as u32;
                    mesh.vertices.push(v);
                    id
                });
                lines[side].push(id);
            }
            lines[side].push(b);
        }
        for i in 0..segments as usize {
            out.push(vec![
                lines[0][i],
                lines[1][i],
                lines[1][i + 1],
                lines[0][i + 1],
            ]);
            mats.push(
                materials
                    .get(original.face_count() + index)
                    .copied()
                    .flatten(),
            );
        }
    }
    mesh.topology.as_mut().unwrap().faces = out.into();
    mesh.topology.as_mut().unwrap().face_materials = mats.into();
    finish(mesh, original.smooth_normals_enabled())
}
