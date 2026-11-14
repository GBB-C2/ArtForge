//! Polygon edits preserve the authoring boundary and rebuild render triangles.
use crate::{
    position_key, recompute_normals, CoreError, Mesh, MeshEdit, Vec3, Vertex, MAX_EDIT_TRIANGLES,
    MAX_EDIT_VERTICES,
};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
pub(crate) fn face_normal(mesh: &Mesh, face: &[u32]) -> Vec3 {
    let p = |i: u32| Vec3::from_array(mesh.vertices[i as usize].position);
    (1..face.len() - 1)
        .map(|i| (p(face[i]) - p(face[0])).cross(p(face[i + 1]) - p(face[0])))
        .sum()
}
fn compact(mesh: &mut Mesh) -> Result<(), CoreError> {
    let mut mapping = BTreeMap::new();
    let mut vertices = Vec::new();
    for face in &mut mesh.topology.as_mut().unwrap().faces {
        for index in face {
            *index = *mapping.entry(*index).or_insert_with(|| {
                let next = vertices.len() as u32;
                vertices.push(mesh.vertices[*index as usize]);
                next
            });
        }
    }
    mesh.vertices = vertices.into();
    mesh.rebuild_polygon_indices()
}
pub(crate) fn flat_normals(mesh: &mut Mesh) -> Result<(), CoreError> {
    let faces = mesh.topology.as_ref().unwrap().faces.clone();
    let mut vertices = Vec::new();
    let mut out = Vec::new();
    for face in &faces {
        let normal = face_normal(mesh, face)
            .try_normalize()
            .unwrap_or(Vec3::Y)
            .to_array();
        let mut next = Vec::new();
        for &index in face {
            let mut v = mesh.vertices[index as usize];
            v.normal = normal;
            next.push(vertices.len() as u32);
            vertices.push(v);
        }
        out.push(next);
    }
    mesh.vertices = vertices.into();
    mesh.topology.as_mut().unwrap().faces = out.into();
    mesh.rebuild_polygon_indices()
}
fn selected(mesh: &Mesh, faces: &BTreeSet<usize>) -> Result<(), CoreError> {
    if faces.is_empty() || faces.iter().any(|&i| i >= mesh.face_count()) {
        Err(invalid("Выберите грани меша"))
    } else {
        Ok(())
    }
}
pub(crate) fn apply(mesh: &mut Mesh, operation: MeshEdit) -> Result<(), CoreError> {
    let smooth = mesh.smooth_normals_enabled();
    let shading_only = matches!(operation, MeshEdit::SmoothNormals | MeshEdit::FlatNormals);
    match operation {
        MeshEdit::ExtrudeVertices { .. } | MeshEdit::DeleteComponents { .. }
        | MeshEdit::SubdivideSelected { .. } | MeshEdit::BridgeFaces { .. } | MeshEdit::BridgeAdvanced {..} | MeshEdit::Topology {..} | MeshEdit::KnifeStroke {..} | MeshEdit::KnifeGap {..} | MeshEdit::KnifeGapClosed {..} | MeshEdit::Dissolve {..} | MeshEdit::Array {..} | MeshEdit::ExtrudeDistributed {..} | MeshEdit::ExtrudeManifold {..} | MeshEdit::FaceBevel {..} => unreachable!(),
        MeshEdit::Normals { .. }
        | MeshEdit::Shape { .. }
        | MeshEdit::Knife { .. }
        | MeshEdit::Slide { .. }
        | MeshEdit::WeightedTransform { .. }
        | MeshEdit::Uv(_)
        | MeshEdit::VertexBevel { .. }
        | MeshEdit::EdgeBevel { .. }
        | MeshEdit::MergeVertices { .. }
        | MeshEdit::WeldVertices { .. }
        | MeshEdit::Bridge { .. }
        | MeshEdit::DissolveEdges { .. }
        | MeshEdit::Transform { .. }
        | MeshEdit::Bevel { .. }
        | MeshEdit::LoopCut { .. } => {
            unreachable!()
        }
        MeshEdit::Translate { vertices, delta } => {
            if vertices.is_empty()
                || vertices.iter().any(|&i| i >= mesh.vertices.len())
                || !delta.is_finite()
                || delta.abs().max_element() > 10000.
            {
                return Err(invalid("Выберите вершины и задайте конечное смещение"));
            }
            if delta == Vec3::ZERO {
                return Ok(());
            }
            let keys: BTreeSet<_> = vertices
                .iter()
                .map(|&i| position_key(mesh.vertices[i].position))
                .collect();
            let all = mesh
                .vertices
                .iter()
                .all(|v| keys.contains(&position_key(v.position)));
            for v in &mut mesh.vertices {
                if keys.contains(&position_key(v.position)) {
                    v.position = (Vec3::from_array(v.position) + delta).to_array();
                }
            }
            if !all {
                recompute_normals(mesh, smooth);
            }
        }
        MeshEdit::Extrude { faces, distance } => {
            selected(mesh, &faces)?;
            if distance==0. {return Ok(());}
            if !distance.is_finite() || distance.abs() < 1e-6 || distance.abs() > 1000. {
                return Err(invalid(
                    "Расстояние экструзии: ненулевое число от −1000 до 1000",
                ));
            }
            let source = mesh.topology.as_ref().unwrap().faces.clone();
            let mut weld = BTreeMap::new();
            let canonical: Vec<_> = mesh
                .vertices
                .iter()
                .enumerate()
                .map(|(i, v)| *weld.entry(position_key(v.position)).or_insert(i as u32))
                .collect();
            let mut normal = Vec3::ZERO;
            let mut used = BTreeSet::new();
            let mut edges = BTreeMap::<(u32, u32), Vec<(u32, u32)>>::new();
            let mut edge_materials = BTreeMap::new();
            for &i in &faces {
                let face = &source[i];
                normal += face_normal(mesh, face);
                for j in 0..face.len() {
                    let (a, b) = (face[j], face[(j + 1) % face.len()]);
                    used.insert(a);
                    edge_materials.insert(
                        (a, b),
                        mesh.topology
                            .as_ref()
                            .unwrap()
                            .face_materials
                            .get(i)
                            .copied()
                            .flatten(),
                    );
                    let (ca, cb) = (canonical[a as usize], canonical[b as usize]);
                    edges
                        .entry((ca.min(cb), ca.max(cb)))
                        .or_default()
                        .push((a, b));
                }
            }
            let delta = normal.try_normalize().ok_or_else(|| {
                invalid("У области нет общего направления; выберите одну сторону")
            })? * distance;
            if mesh.vertices.len() + used.len() > MAX_EDIT_VERTICES {
                return Err(invalid("Слишком много вершин для экструзии"));
            }
            let mut cap = BTreeMap::new();
            for i in used {
                let mut v = mesh.vertices[i as usize];
                v.position = (Vec3::from_array(v.position) + delta).to_array();
                cap.insert(i, mesh.vertices.len() as u32);
                mesh.vertices.push(v);
            }
            let t = mesh.topology.as_mut().unwrap();
            for &i in &faces {
                for v in &mut t.faces[i] {
                    *v = cap[v];
                }
            }
            for boundary in edges.values() {
                if let [(a, b)] = boundary.as_slice() {
                    t.faces.push(vec![*a, *b, cap[b], cap[a]]);
                    if !t.face_materials.is_empty() {
                        t.face_materials.push(edge_materials[&(*a, *b)]);
                    }
                }
            }
            compact(mesh)?;
            recompute_normals(mesh, smooth);
        }
        MeshEdit::DeleteFaces(faces) => {
            selected(mesh, &faces)?;
            if faces.len() == mesh.face_count() {
                return Err(invalid(
                    "Оставьте хотя бы одну грань; весь объект удаляется в режиме сцены",
                ));
            }
            let t = mesh.topology.as_mut().unwrap();
            if !t.face_materials.is_empty() {
                t.face_materials = t
                    .face_materials
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !faces.contains(i))
                    .map(|(_, m)| *m)
                    .collect();
            }
            t.faces = t
                .faces
                .iter()
                .enumerate()
                .filter(|(i, _)| !faces.contains(i))
                .map(|(_, f)| f.clone())
                .collect();
            compact(mesh)?;
            recompute_normals(mesh, smooth);
        }
        MeshEdit::Subdivide => {
            let source = mesh.topology.as_ref().unwrap().faces.clone();
            let materials = mesh.topology.as_ref().unwrap().face_materials.clone();
            let corners: usize = source.iter().map(Vec::len).sum();
            if corners * 2 > MAX_EDIT_TRIANGLES
                || mesh.vertices.len() + corners + source.len() > MAX_EDIT_VERTICES
            {
                return Err(invalid("Слишком много граней для подразделения"));
            }
            let mut midpoints = BTreeMap::new();
            let mut output = Vec::new();
            for face in &source {
                let n = face.len() as f32;
                let mut center = Vertex {
                    color: [0.; 4],
                    ..Vertex::default()
                };
                for &i in face {
                    let v = mesh.vertices[i as usize];
                    for axis in 0..3 {
                        center.position[axis] += v.position[axis] / n;
                    }
                    for axis in 0..2 {
                        center.uv[axis] += v.uv[axis] / n;
                    }
                    for axis in 0..4 {
                        center.color[axis] += v.color[axis] / n;
                    }
                }
                center.color = center.color.map(|c| c.clamp(0., 1.));
                let center_id = mesh.vertices.len() as u32;
                mesh.vertices.push(center);
                let mut mids = Vec::new();
                for j in 0..face.len() {
                    let (a, b) = (face[j], face[(j + 1) % face.len()]);
                    let id = *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                        let (va, vb) = (mesh.vertices[a as usize], mesh.vertices[b as usize]);
                        let id = mesh.vertices.len() as u32;
                        mesh.vertices.push(Vertex {
                            position: ((Vec3::from_array(va.position)
                                + Vec3::from_array(vb.position))
                                * 0.5)
                                .to_array(),
                            uv: [(va.uv[0] + vb.uv[0]) * 0.5, (va.uv[1] + vb.uv[1]) * 0.5],
                            color: std::array::from_fn(|i| (va.color[i] + vb.color[i]) * 0.5),
                            ..Vertex::default()
                        });
                        id
                    });
                    mids.push(id);
                }
                for j in 0..face.len() {
                    output.push(vec![
                        face[j],
                        mids[j],
                        center_id,
                        mids[(j + face.len() - 1) % face.len()],
                    ]);
                }
            }
            if !materials.is_empty() {
                mesh.topology.as_mut().unwrap().face_materials = source
                    .iter()
                    .enumerate()
                    .flat_map(|(i, f)| std::iter::repeat_n(materials[i], f.len()))
                    .collect();
            }
            mesh.topology.as_mut().unwrap().faces = output.into();
            mesh.rebuild_polygon_indices()?;
            recompute_normals(mesh, smooth);
        }
        MeshEdit::SmoothNormals => {
            recompute_normals(mesh, true);
            let t = mesh.topology.as_mut().unwrap();
            t.smooth_normals = true;
            if let Some(p) = &mut t.primitive {
                p.smooth = true;
            }
        }
        MeshEdit::FlatNormals => {
            if mesh
                .topology
                .as_ref()
                .unwrap()
                .faces
                .iter()
                .map(Vec::len)
                .sum::<usize>()
                > MAX_EDIT_VERTICES
            {
                return Err(invalid("Слишком много вершин для плоских нормалей"));
            }
            flat_normals(mesh)?;
            let t = mesh.topology.as_mut().unwrap();
            t.smooth_normals = false;
            if let Some(p) = &mut t.primitive {
                p.smooth = false;
            }
        }
    }
    if !shading_only {
        let t = mesh.topology.as_mut().unwrap();
        t.smooth_normals = smooth;
        t.primitive = None;
    }
    if mesh.vertices.len() > MAX_EDIT_VERTICES || mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES {
        return Err(invalid(
            "Предел редактирования: 65536 вершин и 131072 треугольника",
        ));
    }
    mesh.validate_topology()
}
