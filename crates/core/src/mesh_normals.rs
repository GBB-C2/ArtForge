//! Corner normals preserve UV/material boundaries and never move geometry.
use crate::{CoreError, Mesh, MeshConnectivity, MeshTopology, Quat, Vec3, MAX_EDIT_VERTICES};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub enum NormalEdit {
    Recalculate { angle: f32, weighted: bool },
    FlipFaces,
    Direction(Vec3),
    Rotate(Quat),
}
fn invalid(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}
pub(crate) fn apply(
    mesh: &mut Mesh,
    selection: &BTreeSet<usize>,
    edit: &NormalEdit,
) -> Result<(), CoreError> {
    if selection.is_empty() || selection.iter().any(|f| *f >= mesh.face_count()) {
        return Err(invalid("Выберите грани для обработки нормалей"));
    }
    match edit {
        NormalEdit::Recalculate { angle, .. }
            if !angle.is_finite() || !(0.0..=180.0).contains(angle) =>
        {
            return Err(invalid("Угол сглаживания: 0–180°"))
        }
        NormalEdit::Direction(v) if !v.is_finite() || v.length_squared() < 1e-12 => {
            return Err(invalid("Направление нормали должно быть ненулевым"))
        }
        NormalEdit::Rotate(q) if !q.is_finite() || (q.length_squared() - 1.).abs() > 0.001 => {
            return Err(invalid("Некорректный поворот нормалей"))
        }
        _ => {}
    }
    let faces = crate::mesh_topology_tools::faces(mesh);
    let graph = MeshConnectivity::new(mesh);
    let mut offsets = vec![0];
    for face in &faces {
        offsets.push(offsets.last().unwrap() + face.len());
    }
    let count = *offsets.last().unwrap();
    let mut normals = Vec::with_capacity(count);
    for face in &faces {
        normals.extend(
            face.iter()
                .map(|i| Vec3::from_array(mesh.vertices[*i as usize].normal)),
        );
    }
    if let NormalEdit::Recalculate { angle, weighted } = edit {
        let areas: Vec<_> = faces
            .iter()
            .map(|f| crate::polygon_edit::face_normal(mesh, f))
            .collect();
        let units: Vec<_> = areas
            .iter()
            .map(|n| {
                n.try_normalize()
                    .ok_or_else(|| invalid("Вырожденная грань: сначала исправьте геометрию"))
            })
            .collect::<Result<_, _>>()?;
        // Join only adjacent polygon fans, never surfaces that merely touch at a point.
        let mut sides = BTreeMap::<(usize, usize), Vec<(usize, usize, usize)>>::new();
        for (f, face) in faces.iter().enumerate() {
            for c in 0..face.len() {
                let a = graph.canonical[face[c] as usize];
                let b = graph.canonical[face[(c + 1) % face.len()] as usize];
                let ca = offsets[f] + c;
                let cb = offsets[f] + (c + 1) % face.len();
                sides
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push(if a < b { (f, ca, cb) } else { (f, cb, ca) });
            }
        }
        let mut parents: Vec<_> = (0..count).collect();
        let threshold = angle.to_radians().cos();
        for entries in sides.values() {
            let [a, b] = entries.as_slice() else { continue };
            if units[a.0].dot(units[b.0]) + 1e-6 >= threshold {
                for (i, j) in [(a.1, b.1), (a.2, b.2)] {
                    let ra = root(&mut parents, i);
                    let rb = root(&mut parents, j);
                    parents[ra] = rb;
                }
            }
        }
        let mut sums = vec![Vec3::ZERO; count];
        for (f, face) in faces.iter().enumerate() {
            for (c, &index) in face.iter().enumerate() {
                let p = Vec3::from_array(mesh.vertices[index as usize].position);
                let a = Vec3::from_array(
                    mesh.vertices[face[(c + face.len() - 1) % face.len()] as usize].position,
                ) - p;
                let b =
                    Vec3::from_array(mesh.vertices[face[(c + 1) % face.len()] as usize].position)
                        - p;
                let mut corner = a
                    .normalize_or_zero()
                    .dot(b.normalize_or_zero())
                    .clamp(-1., 1.)
                    .acos();
                if a.cross(b).dot(units[f]) > 0. {
                    corner = std::f32::consts::TAU - corner;
                }
                let weight = corner * if *weighted { areas[f].length() } else { 1. };
                let r = root(&mut parents, offsets[f] + c);
                sums[r] += units[f] * weight;
            }
        }
        for &f in selection {
            for c in 0..faces[f].len() {
                let i = offsets[f] + c;
                normals[i] = sums[root(&mut parents, i)]
                    .try_normalize()
                    .unwrap_or(units[f]);
            }
        }
    } else {
        for &f in selection {
            for normal in &mut normals[offsets[f]..offsets[f + 1]] {
                *normal = match edit {
                    NormalEdit::FlipFaces => -*normal,
                    NormalEdit::Direction(v) => *v,
                    NormalEdit::Rotate(q) => *q * *normal,
                    _ => unreachable!(),
                }
                .try_normalize()
                .ok_or_else(|| invalid("Нулевая нормаль: выполните пересчёт"))?;
            }
        }
    }
    // Reuse identical corners; split a shared vertex only when its normals differ.
    let mut mapping = BTreeMap::new();
    let mut vertices = mesh.vertices.to_vec();
    let mut used = BTreeSet::new();
    let mut output = Vec::new();
    for (f, face) in faces.iter().enumerate() {
        let mut next = Vec::new();
        for (c, &index) in face.iter().enumerate() {
            let normal = normals[offsets[f] + c].to_array();
            let key = (index, normal.map(f32::to_bits));
            let new = *mapping.entry(key).or_insert_with(|| {
                let mut v = mesh.vertices[index as usize];
                v.normal = normal;
                if used.insert(index) {
                    vertices[index as usize] = v;
                    index
                } else {
                    let id = vertices.len() as u32;
                    vertices.push(v);
                    id
                }
            });
            next.push(new);
        }
        if matches!(edit, NormalEdit::FlipFaces) && selection.contains(&f) {
            next[1..].reverse();
        }
        output.push(next);
    }
    if vertices.len() > MAX_EDIT_VERTICES {
        return Err(invalid(
            "Разделение нормалей превысит 65536 вершин; обработайте меньшую область",
        ));
    }
    let mut topology = mesh.topology.clone().unwrap_or(MeshTopology { loose_edges: Vec::new(),
        faces: Default::default(),
        primitive: None,
        smooth_normals: false,
        face_materials: Default::default(),
        uv_seams: Vec::new(),
    });
    topology.faces = output.into();
    topology.primitive = None;
    if let NormalEdit::Recalculate { angle, .. } = edit {
        topology.smooth_normals = *angle > 0.;
    }
    mesh.vertices = vertices.into();
    mesh.topology = Some(topology);
    mesh.rebuild_polygon_indices()?;
    mesh.validate_topology()
}
