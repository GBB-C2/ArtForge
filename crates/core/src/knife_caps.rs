//! Closed section cuts use sequential solid clipping so later cuts also trim caps.
use crate::{CoreError, KnifeCut, Mesh, MeshConnectivity, Vec3};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) fn apply(mesh: &mut Mesh, cuts: &[KnifeCut], width: f32) -> Result<bool, CoreError> {
    if !width.is_finite() || width <= 0. || width > 1000. {
        return Ok(false);
    }
    if cuts.is_empty()
        || cuts.len() > 2048
        || cuts.iter().any(|c| {
            c.face >= mesh.face_count()
                || !Vec3::from_array(c.first).is_finite()
                || !Vec3::from_array(c.second).is_finite()
                || !crate::knife_interior::contains(mesh, c.face, Vec3::from_array(c.first))
                || !crate::knife_interior::contains(mesh, c.face, Vec3::from_array(c.second))
        })
    {
        return Ok(false);
    }
    let mut endpoints = BTreeMap::<[i64; 3], Vec<usize>>::new();
    let mut segments = Vec::new();
    let mut seen = BTreeSet::new();
    for c in cuts {
        if c.face >= mesh.face_count() {
            return Ok(false);
        }
        let a = crate::position_key(c.first);
        let b = crate::position_key(c.second);
        if a == b {
            continue;
        }
        if seen.insert((a.min(b), a.max(b))) {
            let i = segments.len();
            segments.push(c);
            endpoints.entry(a).or_default().push(i);
            endpoints.entry(b).or_default().push(i);
        }
    }
    if endpoints.is_empty() || endpoints.values().any(|v| v.len() != 2) {
        return Ok(false);
    }
    let mut remaining: BTreeSet<_> = (0..segments.len()).collect();
    let mut planes = Vec::<(Vec3, f32, BTreeSet<usize>)>::new();
    while let Some(&seed) = remaining.first() {
        let mut group = Vec::new();
        let mut pending = vec![seed];
        while let Some(i) = pending.pop() {
            if remaining.remove(&i) {
                group.push(i);
                for p in [segments[i].first, segments[i].second] {
                    pending.extend(&endpoints[&crate::position_key(p)]);
                }
            }
        }
        let points: Vec<_> = group
            .iter()
            .flat_map(|&i| [segments[i].first, segments[i].second])
            .map(Vec3::from_array)
            .collect();
        let a = points[0];
        let u = points[1] - a;
        let Some(mut n) = points
            .iter()
            .map(|p| u.cross(*p - a))
            .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
            .and_then(|v| v.try_normalize())
        else {
            return Ok(false);
        };
        let extent = points.iter().map(|p| p.distance(a)).fold(1_f32, f32::max);
        if points.iter().any(|p| (*p - a).dot(n).abs() > extent * 1e-5) {
            return Ok(false);
        }
        let axis = if n.x.abs() > n.y.abs() && n.x.abs() > n.z.abs() {
            0
        } else if n.y.abs() > n.z.abs() {
            1
        } else {
            2
        };
        if n[axis] < 0. {
            n = -n;
        }
        let d = n.dot(a);
        let faces = group.into_iter().map(|i| segments[i].face);
        if let Some((_, _, existing)) = planes
            .iter_mut()
            .find(|(old_n, old_d, _)| old_n.dot(n) > 0.99999 && (*old_d - d).abs() < extent * 1e-5)
        {
            existing.extend(faces);
        } else {
            planes.push((n, d, faces.collect()));
        }
    }
    let g = MeshConnectivity::new(mesh);
    let mut pending_faces: BTreeSet<_> = (0..mesh.face_count()).collect();
    let mut components = Vec::new();
    while let Some(&seed) = pending_faces.first() {
        let mut ids = BTreeSet::new();
        let mut pending = vec![seed];
        while let Some(f) = pending.pop() {
            if pending_faces.remove(&f) {
                ids.insert(f);
                pending.extend(&g.face_neighbors[f]);
            }
        }
        let selected = planes
            .iter()
            .any(|(_, _, fs): &(Vec3, f32, BTreeSet<usize>)| !fs.is_disjoint(&ids));
        if selected
            && g.edge_faces
                .values()
                .any(|fs| fs.iter().any(|f| ids.contains(f)) && fs.len() != 2)
        {
            return Ok(false);
        }
        // A plane may meet a torus or concave shell in another, unselected
        // contour. Whole-solid clipping is valid only for a complete section.
        for (n, d, cut_faces) in &planes {
            if ids.is_disjoint(cut_faces) {
                continue;
            }
            if ids.iter().any(|&f| {
                if cut_faces.contains(&f) {
                    return false;
                }
                let distances: Vec<_> = mesh
                    .face_vertices(f)
                    .unwrap()
                    .iter()
                    .map(|&i| Vec3::from_array(mesh.vertices[i as usize].position).dot(*n) - *d)
                    .collect();
                distances.iter().any(|&v| v > 1e-5) && distances.iter().any(|&v| v < -1e-5)
            }) {
                return Ok(false);
            }
        }
        components.push(ids);
    }
    crate::fracture::section_gaps(mesh, &components, &planes, width).map_err(CoreError::Invalid)?;
    Ok(true)
}
