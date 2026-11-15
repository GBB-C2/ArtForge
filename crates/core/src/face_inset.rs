//! Inset a connected region without adding walls along its interior edges.
use crate::mesh_topology_tools::invalid;
use crate::{CoreError, FaceTreatment, Mesh, MeshConnectivity, Vec3};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn apply(
    mesh: &mut Mesh,
    selected: &BTreeSet<usize>,
    inset: f32,
    depth: f32,
    mode: FaceTreatment,
) -> Result<(), CoreError> {
    if selected.is_empty()
        || selected.iter().any(|&f| f >= mesh.face_count())
        || !inset.is_finite()
        || !(0. ..=0.95).contains(&inset)
        || !depth.is_finite()
        || depth.abs() > 1000.
    {
        return Err(invalid(
            "Выберите грани; отступ 0–95%, глубина −1000–1000 м",
        ));
    }
    if inset == 0. && depth == 0. {
        return Ok(());
    }
    if mode == FaceTreatment::Individual {
        return crate::mesh_modeling::bevel(mesh, selected, inset, depth);
    }
    crate::component_mesh::ensure_topology(mesh);
    let original = mesh.clone();
    let graph = MeshConnectivity::new(&original);
    let mut pending = selected.clone();
    while let Some(seed) = pending.pop_first() {
        let mut region = BTreeSet::from([seed]);
        let mut stack = vec![seed];
        while let Some(f) = stack.pop() {
            let ring = original.face_vertices(f).unwrap();
            for i in 0..ring.len() {
                let a = graph.canonical[ring[i] as usize];
                let b = graph.canonical[ring[(i + 1) % ring.len()] as usize];
                if let Some(faces) = graph.edge_faces.get(&(a.min(b), a.max(b))) {
                    for &next in faces {
                        if pending.remove(&next) {
                            region.insert(next);
                            stack.push(next);
                        }
                    }
                }
            }
        }
        inset_region(mesh, &original, &graph, &region, inset, depth, mode)?;
    }
    crate::mesh_modeling::finish(mesh, original.smooth_normals_enabled())
}

fn inset_region(
    mesh: &mut Mesh,
    source: &Mesh,
    graph: &MeshConnectivity,
    region: &BTreeSet<usize>,
    inset: f32,
    depth: f32,
    mode: FaceTreatment,
) -> Result<(), CoreError> {
    let pos = |v: u32| Vec3::from_array(source.vertices[v as usize].position);
    let mut normals = BTreeMap::<usize, Vec3>::new();
    let mut used = BTreeSet::new();
    let mut boundary = Vec::new();
    let mut inward = BTreeMap::<usize, Vec<Vec3>>::new();
    let mut lengths = Vec::new();
    let mut normal = Vec3::ZERO;
    for &f in region {
        let ring = source.face_vertices(f).unwrap();
        let n = crate::polygon_edit::face_normal(source, ring);
        let unit = n
            .try_normalize()
            .ok_or_else(|| invalid("У выбранной грани нет нормали"))?;
        normal += n;
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            used.insert(a);
            let (ca, cb) = (graph.canonical[a as usize], graph.canonical[b as usize]);
            *normals.entry(ca).or_default() += n;
            if ca == cb {
                continue;
            }
            let incident = &graph.edge_faces[&(ca.min(cb), ca.max(cb))];
            if incident.iter().filter(|f| region.contains(f)).count() == 1 {
                let edge = pos(b) - pos(a);
                let direction = unit.cross(edge).normalize_or_zero();
                lengths.push(edge.length());
                inward.entry(ca).or_default().push(direction);
                inward.entry(cb).or_default().push(direction);
                boundary.push((a, b, f));
            }
        }
    }
    if boundary.is_empty() && inset > 0. && depth == 0. {
        return Err(invalid("У замкнутого выделения нет границы для вставки; выберите часть поверхности или задайте глубину"));
    }
    if mesh.vertices.len().saturating_add(used.len()) > crate::MAX_EDIT_VERTICES
        || mesh.indices.len() / 3 + boundary.len() * 2 > crate::MAX_EDIT_TRIANGLES
    {
        return Err(invalid("Вставка превышает предел редактируемой сетки"));
    }
    let region_normal = normal.try_normalize();
    if mode == FaceTreatment::Region && depth != 0. && region_normal.is_none() {
        return Err(invalid(
            "У области нет общего направления; выберите «По нормалям»",
        ));
    }
    lengths.sort_by(f32::total_cmp);
    let width = lengths.get(lengths.len() / 2).copied().unwrap_or(0.) * inset * 0.5;
    let mut shifts = BTreeMap::new();
    for (&v, dirs) in &inward {
        let direction = dirs.iter().copied().sum::<Vec3>().normalize_or_zero();
        let projection = dirs.iter().map(|n| direction.dot(*n)).sum::<f32>() / dirs.len() as f32;
        if projection < 0.05 && width > 0. {
            return Err(invalid(
                "Граница вставки разворачивается; уменьшите выделение у острого угла",
            ));
        }
        shifts.insert(v, direction * (width / projection.max(0.05)));
    }
    let mut cap = BTreeMap::new();
    for v in used {
        let canonical = graph.canonical[v as usize];
        let n = if mode == FaceTreatment::Normals {
            normals[&canonical].normalize_or_zero()
        } else {
            region_normal.unwrap_or(Vec3::ZERO)
        };
        let mut vertex = source.vertices[v as usize];
        vertex.position =
            (pos(v) + shifts.get(&canonical).copied().unwrap_or(Vec3::ZERO) + n * depth).to_array();
        cap.insert(v, mesh.vertices.len() as u32);
        mesh.vertices.push(vertex);
    }
    for &f in region {
        let ring = source.face_vertices(f).unwrap();
        let next: Vec<_> = ring.iter().map(|v| cap[v]).collect();
        if crate::polygon_edit::face_normal(source, ring)
            .dot(crate::polygon_edit::face_normal(mesh, &next))
            <= 0.
        {
            return Err(invalid("Отступ переворачивает грань; уменьшите его"));
        }
        mesh.topology.as_mut().unwrap().faces[f] = next;
    }
    let topology = mesh.topology.as_mut().unwrap();
    for (a, b, f) in boundary {
        topology.faces.push(vec![a, b, cap[&b], cap[&a]]);
        if !topology.face_materials.is_empty() {
            topology.face_materials.push(source.face_material(f));
        }
    }
    Ok(())
}
