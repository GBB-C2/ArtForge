//! Bounded component transforms, individual polygon bevels and quad loop cuts.
use crate::{
    CoreError, Mat4, Mesh, MeshConnectivity, MeshTopology, Vec3, Vertex, MAX_EDIT_TRIANGLES,
    MAX_EDIT_VERTICES,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
fn detach(mesh: &mut Mesh, smooth: bool) {
    if let Some(t) = &mut mesh.topology {
        t.smooth_normals = smooth;
        t.primitive = None;
    }
}
fn finish(mesh: &mut Mesh, smooth: bool) -> Result<(), CoreError> {
    mesh.rebuild_polygon_indices()?;
    crate::recompute_normals(mesh, smooth);
    detach(mesh, smooth);
    if mesh.vertices.len() > MAX_EDIT_VERTICES || mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES {
        return Err(invalid("Операция превысит предел редактируемой сетки"));
    }
    mesh.validate_topology()
}
fn polygons(mesh: &mut Mesh) {
    if mesh.topology.is_none() {
        mesh.topology = Some(MeshTopology {
            face_materials: Default::default(),
            uv_seams: Vec::new(),
            faces: mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|f| f.to_vec())
                .collect(),
            primitive: None,
            smooth_normals: false,
        });
    }
}
pub(crate) fn transform(
    mesh: &mut Mesh,
    selection: &BTreeSet<usize>,
    matrix: Mat4,
) -> Result<(), CoreError> {
    if selection.is_empty() || selection.iter().any(|&i| i >= mesh.vertices.len()) {
        return Err(invalid("Выберите компоненты меша"));
    }
    if !matrix.is_finite()
        || matrix.determinant() <= 1e-12
        || matrix.x_axis.w != 0.
        || matrix.y_axis.w != 0.
        || matrix.z_axis.w != 0.
        || matrix.w_axis.w != 1.
    {
        return Err(invalid(
            "Задайте конечную трансформацию без нулевого или отрицательного масштаба",
        ));
    }
    if matrix == Mat4::IDENTITY {
        return Ok(());
    }
    let keys: BTreeSet<_> = selection
        .iter()
        .map(|&i| crate::position_key(mesh.vertices[i].position))
        .collect();
    let selected: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| keys.contains(&crate::position_key(v.position)))
        .collect();
    let all = selected.iter().all(|v| *v);
    let smooth = mesh.smooth_normals_enabled();
    let normal = matrix.inverse().transpose();
    for (v, selected) in mesh.vertices.iter_mut().zip(selected) {
        if selected {
            let p = matrix.transform_point3(Vec3::from_array(v.position));
            if !p.is_finite() || p.abs().max_element() > 1e8 {
                return Err(invalid("Трансформация выходит за допустимые координаты"));
            }
            v.position = p.to_array();
            if all {
                v.normal = normal
                    .transform_vector3(Vec3::from_array(v.normal))
                    .try_normalize()
                    .unwrap_or(Vec3::Y)
                    .to_array();
            }
        }
    }
    if !all {
        crate::recompute_normals(mesh, smooth);
    }
    detach(mesh, smooth);
    Ok(())
}
pub(crate) fn bevel(
    mesh: &mut Mesh,
    selection: &BTreeSet<usize>,
    inset: f32,
    shift: f32,
) -> Result<(), CoreError> {
    if selection.is_empty() || selection.iter().any(|&f| f >= mesh.face_count()) {
        return Err(invalid("Выберите грани для фаски"));
    }
    if !inset.is_finite()
        || !(0.0..=0.95).contains(&inset)
        || !shift.is_finite()
        || shift.abs() > 1000.
    {
        return Err(invalid("Отступ: 0–95%; высота: −1000–1000 м"));
    }
    if inset == 0. && shift == 0. {
        return Ok(());
    }
    let corners: usize = selection
        .iter()
        .map(|&f| mesh.face_vertices(f).unwrap().len())
        .sum();
    if mesh.vertices.len() + corners > MAX_EDIT_VERTICES
        || mesh.indices.len() / 3 + corners * 2 > MAX_EDIT_TRIANGLES
    {
        return Err(invalid("Слишком много граней для фаски"));
    }
    let smooth = mesh.smooth_normals_enabled();
    polygons(mesh);
    let source = mesh.topology.as_ref().unwrap().faces.clone();
    for &f in selection {
        let face = &source[f];
        let center = face
            .iter()
            .map(|&i| Vec3::from_array(mesh.vertices[i as usize].position))
            .sum::<Vec3>()
            / face.len() as f32;
        let normal = crate::polygon_edit::face_normal(mesh, face)
            .try_normalize()
            .ok_or_else(|| invalid("Вырожденная грань: фаска невозможна"))?;
        let uv = face.iter().fold([0.; 2], |mut a, &i| {
            for (axis, v) in a.iter_mut().enumerate() {
                *v += mesh.vertices[i as usize].uv[axis] / face.len() as f32;
            }
            a
        });
        let mut cap = Vec::new();
        for &i in face {
            let mut v = mesh.vertices[i as usize];
            v.position =
                (Vec3::from_array(v.position).lerp(center, inset) + normal * shift).to_array();
            for (axis, center) in uv.iter().enumerate() {
                v.uv[axis] = v.uv[axis] * (1. - inset) + center * inset;
            }
            cap.push(mesh.vertices.len() as u32);
            mesh.vertices.push(v);
        }
        let t = mesh.topology.as_mut().unwrap();
        t.faces[f] = cap.clone();
        for j in 0..face.len() {
            let k = (j + 1) % face.len();
            t.faces.push(vec![face[j], face[k], cap[k], cap[j]]);
            if !t.face_materials.is_empty() {
                let material = t.face_materials[f];
                t.face_materials.push(material);
            }
        }
    }
    finish(mesh, smooth)
}
pub(crate) fn loop_cut(
    mesh: &mut Mesh,
    edge: (usize, usize),
    position: f32,
) -> Result<(), CoreError> {
    if edge.0 >= mesh.vertices.len()
        || edge.1 >= mesh.vertices.len()
        || !position.is_finite()
        || !(0.01..=0.99).contains(&position)
    {
        return Err(invalid("Выберите ребро; положение разреза: 1–99%"));
    }
    let graph = MeshConnectivity::new(mesh);
    let edge_key = |a: usize, b: usize| {
        let (a, b) = (graph.canonical[a], graph.canonical[b]);
        (a.min(b), a.max(b))
    };
    let seed = edge_key(edge.0, edge.1);
    if !graph.edges.contains(&seed) {
        return Err(invalid("Выберите существующее ребро"));
    }
    let mut cuts = BTreeMap::from([(seed, position)]);
    let mut faces = BTreeMap::new();
    let mut queue = VecDeque::from([seed]);
    while let Some(edge) = queue.pop_front() {
        let adjacent = &graph.edge_faces[&edge];
        if adjacent.len() > 2 {
            return Err(invalid(
                "Разрез остановлен: ребро соединяет более двух граней",
            ));
        }
        for &f in adjacent {
            let face = mesh.face_vertices(f).unwrap();
            if face.len() != 4 {
                return Err(invalid(
                    "Разрез петлёй требует непрерывную полосу четырёхугольников",
                ));
            }
            let j = (0..4)
                .find(|&j| edge_key(face[j] as usize, face[(j + 1) % 4] as usize) == edge)
                .unwrap();
            if let Some(&old) = faces.get(&f) {
                if j != old && j != (old + 2) % 4 {
                    return Err(invalid("Петля пересекает сама себя"));
                }
                continue;
            }
            faces.insert(f, j);
            let (a, b, c, d) = (
                face[j] as usize,
                face[(j + 1) % 4] as usize,
                face[(j + 2) % 4] as usize,
                face[(j + 3) % 4] as usize,
            );
            let local = if graph.canonical[a] < graph.canonical[b] {
                cuts[&edge]
            } else {
                1. - cuts[&edge]
            };
            let opposite = edge_key(c, d);
            let next = if graph.canonical[d] < graph.canonical[c] {
                local
            } else {
                1. - local
            };
            if let Some(previous) = cuts.get(&opposite) {
                if (previous - next).abs() > 1e-4 {
                    return Err(invalid(
                        "Петля меняет ориентацию; используйте центральный разрез",
                    ));
                }
            } else {
                cuts.insert(opposite, next);
                queue.push_back(opposite);
            }
        }
    }
    if mesh.vertices.len() + faces.len() * 2 > MAX_EDIT_VERTICES
        || mesh.indices.len() / 3 + faces.len() * 2 > MAX_EDIT_TRIANGLES
    {
        return Err(invalid("Слишком много граней для разреза"));
    }
    let smooth = mesh.smooth_normals_enabled();
    polygons(mesh);
    let source = mesh.topology.as_ref().unwrap().faces.clone();
    let mut split = BTreeMap::new();
    for (&f, &j) in &faces {
        let face = &source[f];
        let (a, b, c, d) = (
            face[j],
            face[(j + 1) % 4],
            face[(j + 2) % 4],
            face[(j + 3) % 4],
        );
        let mut middle = |a: u32, b: u32| -> u32 {
            *split.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let key = edge_key(a as usize, b as usize);
                let t = cuts[&key];
                let local = if graph.canonical[a as usize] < graph.canonical[b as usize] {
                    t
                } else {
                    1. - t
                };
                let va = mesh.vertices[a as usize];
                let vb = mesh.vertices[b as usize];
                let p = Vec3::from_array(mesh.vertices[key.0].position)
                    .lerp(Vec3::from_array(mesh.vertices[key.1].position), t);
                let v = Vertex {
                    position: p.to_array(),
                    uv: [
                        va.uv[0] * (1. - local) + vb.uv[0] * local,
                        va.uv[1] * (1. - local) + vb.uv[1] * local,
                    ],
                    color: std::array::from_fn(|i| {
                        va.color[i] * (1. - local) + vb.color[i] * local
                    }),
                    ..va
                };
                let id = mesh.vertices.len() as u32;
                mesh.vertices.push(v);
                id
            })
        };
        let ab = middle(a, b);
        let dc = middle(d, c);
        let t = mesh.topology.as_mut().unwrap();
        t.faces[f] = vec![a, ab, dc, d];
        t.faces.push(vec![ab, b, c, dc]);
        if !t.face_materials.is_empty() {
            let material = t.face_materials[f];
            t.face_materials.push(material);
        }
    }
    finish(mesh, smooth)
}
