//! Bound offsets by intersections of moving corners, not a fraction of edge length.
use super::*;

pub(super) fn widths(
    mesh: &Mesh,
    graph: &MeshConnectivity,
    source: &[Vec<u32>],
    selected: &BTreeSet<(usize, usize)>,
    affected: &BTreeSet<usize>,
    positions: &[Vec3],
    requested: f32,
    region: Option<&BTreeSet<usize>>,
) -> Result<BTreeMap<usize, f32>, CoreError> {
    let mut widths: BTreeMap<_, _> = affected.iter().map(|&v| (v, requested)).collect();
    let mut cuts = BTreeMap::<(usize, usize), f32>::new();
    for &(a, b) in &graph.edges {
        if !selected.contains(&edge(a, b)) {
            if affected.contains(&a) {
                cuts.insert((a, b), 1.);
            }
            if affected.contains(&b) {
                cuts.insert((b, a), 1.);
            }
        }
    }
    let mut corners = BTreeMap::<(usize, usize), (Vec3, Option<usize>)>::new();
    for (f, face) in source.iter().enumerate() {
        let normal = crate::polygon_edit::face_normal(mesh, face).normalize_or_zero();
        for i in 0..face.len() {
            let v = graph.canonical[face[i] as usize];
            if !affected.contains(&v) {
                continue;
            }
            let a = graph.canonical[face[(i + face.len() - 1) % face.len()] as usize];
            let b = graph.canonical[face[(i + 1) % face.len()] as usize];
            let left = selected.contains(&edge(a, v));
            let right = selected.contains(&edge(v, b));
            if !left && !right {
                continue;
            }
            let incoming = (positions[v] - positions[a]).normalize();
            let outgoing = (positions[b] - positions[v]).normalize();
            let ni = normal.cross(incoming);
            let no = normal.cross(outgoing);
            let cosine = ni.dot(no);
            let determinant = 1. - cosine * cosine;
            let straight = incoming.dot(outgoing) > 0.99999;
            if determinant < 1e-8 && !straight {
                return Err(invalid("Слишком острый угол для устойчивой фаски"));
            }
            let wi = if left { 1. } else { 0. };
            let wo = if right { 1. } else { 0. };
            let miter = if straight {
                ni
            } else {
                (ni * (wi - cosine * wo) + no * (wo - cosine * wi)) / determinant
            };
            let neighbor = (left != right && !straight).then_some(if left { b } else { a });
            if let Some(neighbor) = neighbor {
                let d = miter.dot((positions[neighbor] - positions[v]).normalize());
                let cut = cuts.entry((v, neighbor)).or_default();
                *cut = cut.max(d);
            }
            corners.insert((f, v), (miter, neighbor));
        }
    }
    let mut constrain = |a: usize, b: usize, limit: f32| {
        for v in [a, b] {
            if let Some(width) = widths.get_mut(&v) {
                *width = width.min(limit);
            }
        }
    };
    // On an untouched edge, cuts may approach from one or both ends.
    for &(a, b) in &graph.edges {
        if interior(graph, region, edge(a, b)) {
            continue;
        }
        let speed =
            cuts.get(&(a, b)).copied().unwrap_or(0.) + cuts.get(&(b, a)).copied().unwrap_or(0.);
        if speed > 0. {
            constrain(a, b, positions[a].distance(positions[b]) * 0.999 / speed);
        }
    }
    // Selected collinear segments do not restrict a perpendicular offset.
    // Only the rate at which the two inset endpoints approach each other matters.
    let mut cap_motion = BTreeMap::new();
    if let Some(region) = region {
        let outline = crate::region_offset::boundary(source, graph, region);
        for &(a, b) in &outline {
            cap_motion.entry(a).or_insert(Vec3::ZERO);
            cap_motion.entry(b).or_insert(Vec3::ZERO);
        }
        for (&(f, v), &(miter, neighbor)) in &corners {
            if region.contains(&f) && cap_motion.contains_key(&v) {
                cap_motion.insert(
                    v,
                    neighbor.map_or(miter, |n| {
                        (positions[n] - positions[v]).normalize() * cuts[&(v, n)]
                    }),
                );
            }
        }
        crate::region_offset::straighten_samples(positions, &outline, &mut cap_motion);
    }
    for (f, face) in source.iter().enumerate() {
        let motion = |v| {
            if region.is_some_and(|faces| faces.contains(&f)) {
                if let Some(&offset) = cap_motion.get(&v) {
                    return offset;
                }
            }
            corners
                .get(&(f, v))
                .map_or(Vec3::ZERO, |&(miter, neighbor)| {
                    neighbor.map_or(miter, |n| {
                        (positions[n] - positions[v]).normalize() * cuts[&(v, n)]
                    })
                })
        };
        for i in 0..face.len() {
            let a = graph.canonical[face[i] as usize];
            let b = graph.canonical[face[(i + 1) % face.len()] as usize];
            if !selected.contains(&edge(a, b)) {
                continue;
            }
            let length = positions[a].distance(positions[b]);
            let speed = (motion(a) - motion(b)).dot((positions[b] - positions[a]) / length);
            if speed > 1e-6 {
                constrain(a, b, length * 0.999 / speed);
            }
        }
    }
    // Keep both sides and endpoints equally wide within each connected selection.
    let mut pending = affected.clone();
    while let Some(seed) = pending.pop_first() {
        let mut component = vec![seed];
        let mut cursor = 0;
        let mut limit = widths[&seed];
        while cursor < component.len() {
            let vertex = component[cursor];
            cursor += 1;
            for &neighbor in &graph.vertex_neighbors[vertex] {
                if selected.contains(&edge(vertex, neighbor)) && pending.remove(&neighbor) {
                    limit = limit.min(widths[&neighbor]);
                    component.push(neighbor);
                }
            }
        }
        for vertex in component {
            widths.insert(vertex, limit);
        }
    }
    Ok(widths)
}

pub(super) fn interior(
    graph: &MeshConnectivity,
    region: Option<&BTreeSet<usize>>,
    edge: (usize, usize),
) -> bool {
    region.is_some_and(|faces| {
        graph
            .edge_faces
            .get(&edge)
            .is_some_and(|adj| adj.len() == 2 && adj.iter().all(|f| faces.contains(f)))
    })
}
