//! Bounded component alignment and relaxation; UV seams move together.
use crate::{CoreError, Mesh, MeshConnectivity, Vec3};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub enum ShapeEdit {
    Flatten(usize),
    Sphere { strength: f32 },
    Circle { axis: usize, strength: f32 },
    Inflate { distance: f32 },
    Twist { axis: usize, radians: f32 },
    Straighten,
    Space,
    Relax {
        iterations: u32,
        strength: f32,
        pin_boundary: bool,
    },
}
fn invalid(s: &str) -> CoreError {
    CoreError::Invalid(s.into())
}
pub(crate) fn apply(
    mesh: &mut Mesh,
    selection: &BTreeSet<usize>,
    edges: &BTreeSet<(usize, usize)>,
    edit: ShapeEdit,
) -> Result<(), CoreError> {
    if selection.len() < 2 || selection.iter().any(|i| *i >= mesh.vertices.len()) {
        return Err(invalid("Выберите минимум две вершины"));
    }
    let graph = MeshConnectivity::new(mesh);
    let selected: BTreeSet<_> = selection.iter().map(|i| graph.canonical[*i]).collect();
    let mut positions: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| Vec3::from_array(v.position))
        .collect();
    let original = positions.clone();
    match edit {
        ShapeEdit::Sphere { strength } | ShapeEdit::Circle { strength, .. } => {
            if !strength.is_finite() || !(0.0..=1.0).contains(&strength) || selected.len()<3 {
                return Err(invalid("Выберите минимум три вершины; сила от 0 до 1"));
            }
            let center = selected.iter().map(|i|positions[*i]).sum::<Vec3>()/selected.len() as f32;
            let axis = if let ShapeEdit::Circle { axis,.. } = edit {
                if axis>2 { return Err(invalid("Ось окружности: X, Y или Z")); } Some(axis)
            } else { None };
            let radial = |point: Vec3| { let mut delta = point-center; if let Some(axis)=axis { delta[axis]=0.; } delta };
            let radius = selected.iter().map(|i|radial(positions[*i]).length()).sum::<f32>()/selected.len() as f32;
            if radius<1e-8 { return Err(invalid("Выделение не имеет радиуса")); }
            if strength==0. { return Ok(()); }
            for &i in &selected {
                let normal = radial(original[i]).try_normalize().ok_or_else(||invalid("Вершина в центре: измените выделение"))?;
                positions[i] = original[i].lerp(center+normal*radius,strength);
            }
        }
        ShapeEdit::Inflate { distance } => {
            if !distance.is_finite() || distance.abs()>10000. { return Err(invalid("Смещение: от −10000 до 10000 м")); }
            if distance==0. { return Ok(()); }
            let mut normals = vec![Vec3::ZERO;mesh.vertices.len()];
            for tri in mesh.indices.chunks_exact(3) {
                let a=tri[0] as usize; let b=tri[1] as usize; let c=tri[2] as usize;
                let normal=(original[b]-original[a]).cross(original[c]-original[a]);
                for i in [a,b,c] { normals[graph.canonical[i]]+=normal; }
            }
            for &i in &selected {
                let normal=normals[i].try_normalize().ok_or_else(||invalid("Не удалось определить нормаль вершины"))?;
                positions[i]=original[i]+normal*distance;
            }
        }
        ShapeEdit::Twist { axis, radians } => {
            if axis>2 || !radians.is_finite() || radians.abs()>std::f32::consts::TAU*10. {
                return Err(invalid("Скручивание: ось X/Y/Z, угол от −3600 до 3600°"));
            }
            if radians==0. { return Ok(()); }
            let center = selected.iter().map(|i|positions[*i]).sum::<Vec3>()/selected.len() as f32;
            let min = selected.iter().map(|i|positions[*i][axis]).fold(f32::INFINITY,f32::min);
            let max = selected.iter().map(|i|positions[*i][axis]).fold(f32::NEG_INFINITY,f32::max);
            if max-min<1e-8 { return Err(invalid("Выделение не имеет высоты по выбранной оси")); }
            let mut direction = Vec3::ZERO; direction[axis]=1.;
            for &i in &selected {
                let angle = radians*(original[i][axis]-min)/(max-min);
                positions[i] = center+crate::Quat::from_axis_angle(direction,angle)*(original[i]-center);
            }
        }
        ShapeEdit::Flatten(axis) => {
            if axis > 2 {
                return Err(invalid("Ось выравнивания: X, Y или Z"));
            }
            let value = selected
                .iter()
                .map(|i| positions[*i][axis] as f64)
                .sum::<f64>()
                / selected.len() as f64;
            for i in &selected {
                positions[*i][axis] = value as f32;
            }
        }
        ShapeEdit::Straighten | ShapeEdit::Space => {
            let path = chain(&graph, &selected, edges)?;
            let mut distances = vec![0.];
            for pair in path.windows(2) {
                distances.push(
                    distances.last().unwrap() + positions[pair[0]].distance(positions[pair[1]]),
                );
            }
            let total = *distances.last().unwrap();
            if total < 1e-8 {
                return Err(invalid("Длина цепочки равна нулю"));
            }
            let start = positions[path[0]];
            let end = positions[*path.last().unwrap()];
            if matches!(edit, ShapeEdit::Straighten) && start.distance(end) < 1e-8 {
                return Err(invalid("Концы цепочки совпадают"));
            }
            let mut segment = 0;
            for (k, &id) in path.iter().enumerate().skip(1).take(path.len() - 2) {
                positions[id] = if matches!(edit, ShapeEdit::Straighten) {
                    start.lerp(end, distances[k] / total)
                } else {
                    let d = total * k as f32 / (path.len() - 1) as f32;
                    while segment + 1 < distances.len() - 1 && distances[segment + 1] < d {
                        segment += 1;
                    }
                    let fraction = (d - distances[segment])
                        / (distances[segment + 1] - distances[segment]).max(1e-12);
                    original[path[segment]].lerp(original[path[segment + 1]], fraction)
                };
            }
        }
        ShapeEdit::Relax {
            iterations,
            strength,
            pin_boundary,
        } => {
            if !(1..=32).contains(&iterations)
                || !strength.is_finite()
                || !(0.0..=1.0).contains(&strength)
            {
                return Err(invalid("Релакс: 1–32 прохода, сила 0–1"));
            }
            let mut border = BTreeSet::new();
            if pin_boundary {
                for (&(a, b), faces) in &graph.edge_faces {
                    if faces.len() != 2 {
                        border.extend([a, b]);
                    }
                }
                for &i in &selected {
                    if graph.vertex_neighbors[i]
                        .iter()
                        .any(|j| !selected.contains(j))
                    {
                        border.insert(i);
                    }
                }
            }
            for _ in 0..iterations {
                let updates: Vec<_> = selected
                    .iter()
                    .filter(|i| !border.contains(i))
                    .filter_map(|&i| {
                        let neighbors = &graph.vertex_neighbors[i];
                        (!neighbors.is_empty()).then(|| {
                            (
                                i,
                                positions[i].lerp(
                                    neighbors.iter().map(|j| positions[*j]).sum::<Vec3>()
                                        / neighbors.len() as f32,
                                    strength,
                                ),
                            )
                        })
                    })
                    .collect();
                for (i, p) in updates {
                    positions[i] = p;
                }
            }
        }
    }
    if selected.iter().all(|i| positions[*i] == original[*i]) {
        return Ok(());
    }
    for (i, v) in mesh.vertices.iter_mut().enumerate() {
        let p = positions[graph.canonical[i]];
        if !p.is_finite() || p.abs().max_element() > 1e8 {
            return Err(invalid("Выравнивание выходит за пределы координат"));
        }
        if selected.contains(&graph.canonical[i]) {
            v.position = p.to_array();
        }
    }
    // Do not allow a destructive collapse to silently remove a face.
    for f in 0..mesh.face_count() {
        if crate::polygon_edit::face_normal(mesh, mesh.face_vertices(f).unwrap()).length_squared()
            < 1e-18
        {
            return Err(invalid(
                "Операция схлопывает грань; уменьшите область или силу",
            ));
        }
    }
    mesh.rebuild_polygon_indices()?;
    crate::recompute_normals(mesh, mesh.smooth_normals_enabled());
    if let Some(t) = &mut mesh.topology {
        t.primitive = None;
    }
    mesh.validate_topology()
}
fn chain(
    graph: &MeshConnectivity,
    selected: &BTreeSet<usize>,
    edges: &BTreeSet<(usize, usize)>,
) -> Result<Vec<usize>, CoreError> {
    let mut neighbors = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in if edges.is_empty() {
        &graph.edges
    } else {
        edges
    } {
        if !graph.edges.contains(&(a, b)) {
            return Err(invalid("Ребро отсутствует в сетке"));
        }
        if selected.contains(&a) && selected.contains(&b) {
            neighbors.entry(a).or_default().insert(b);
            neighbors.entry(b).or_default().insert(a);
        }
    }
    let ends: Vec<_> = neighbors
        .iter()
        .filter(|(_, n)| n.len() == 1)
        .map(|(i, _)| *i)
        .collect();
    if ends.len() != 2
        || neighbors.len() != selected.len()
        || neighbors.values().any(|n| n.len() > 2)
    {
        return Err(invalid("Выберите одну открытую цепочку без ветвлений"));
    }
    let mut path = vec![ends[0]];
    let mut seen = BTreeSet::from([ends[0]]);
    while let Some(&next) = neighbors[path.last().unwrap()]
        .iter()
        .find(|i| !seen.contains(i))
    {
        path.push(next);
        seen.insert(next);
    }
    if path.len() != selected.len() || path.len() < 3 {
        return Err(invalid("Нужна связная цепочка минимум из трёх вершин"));
    }
    Ok(path)
}
