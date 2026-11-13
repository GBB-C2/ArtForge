//! Optional CPU isosurface for a completed/reference fluid state.
//!
//! The field is SPH density / rest_density, triangulated with marching tetrahedra.
//! It is deliberately separate from stepping; call it on demand or from a worker.

use crate::{cell, Cell, FluidError, FluidWorld, Kernel};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SurfaceMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// Extract the dimensionless density isosurface (`iso_density` usually 0.45–0.6).
/// `cell_size` is in meters. Requests larger than 500,000 cells are rejected.
/// Identical grid-edge crossings share indexed vertices and smooth field normals.
/// Interior crossings use the analytic field gradients for a monotone cubic
/// reconstruction, reducing grid-shaped contour error without adding triangles.
pub fn extract_surface(
    world: &FluidWorld,
    cell_size: f32,
    iso_density: f32,
) -> Result<SurfaceMesh, FluidError> {
    extract_surface_cancellable(world, cell_size, iso_density, &AtomicBool::new(false))
}

/// Background extraction with bounded cancellation latency in both field sampling
/// and polygonization. A cancelled or over-budget job never returns a partial mesh.
pub fn extract_surface_cancellable(
    world: &FluidWorld,
    cell_size: f32,
    iso_density: f32,
    cancelled: &AtomicBool,
) -> Result<SurfaceMesh, FluidError> {
    extract_surface_with_budget(world, cell_size, iso_density, cancelled, 500_000)
}

/// Final rendering can resolve thinner silhouettes than the interactive preview.
/// It remains cancellable and retains the vertex/triangle memory guards.
pub fn extract_surface_render_cancellable(
    world: &FluidWorld, cell_size: f32, iso_density: f32, cancelled: &AtomicBool,
) -> Result<SurfaceMesh, FluidError> {
    extract_surface_with_budget(world, cell_size, iso_density, cancelled, 2_100_000)
}

fn extract_surface_with_budget(
    world: &FluidWorld, cell_size: f32, iso_density: f32, cancelled: &AtomicBool, max_cells: usize,
) -> Result<SurfaceMesh, FluidError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(FluidError::Cancelled);
    }
    if !cell_size.is_finite()
        || cell_size < 0.005
        || !iso_density.is_finite()
        || !(0.01..=2.0).contains(&iso_density)
    {
        return Err(FluidError::InvalidConfig(
            "некорректная сетка поверхности жидкости".into(),
        ));
    }
    let config = world.config();
    let h = config.smoothing_radius;
    let particle_min = world
        .positions()
        .iter()
        .fold(Vec3::splat(f32::MAX), |a, &p| a.min(p));
    let particle_max = world
        .positions()
        .iter()
        .fold(Vec3::splat(f32::MIN), |a, &p| a.max(p));
    // Include empty samples outside the boundary so the extracted volume closes.
    let minimum = (particle_min - Vec3::splat(h)).max(config.bounds_min - Vec3::splat(cell_size));
    let maximum = (particle_max + Vec3::splat(h)).min(config.bounds_max + Vec3::splat(cell_size));
    let dimensions = ((maximum - minimum) / cell_size).ceil().as_uvec3();
    let [nx, ny, nz] = dimensions.to_array().map(|n| n as usize);
    let cells = nx
        .checked_mul(ny)
        .and_then(|v| v.checked_mul(nz))
        .unwrap_or(usize::MAX);
    if cells > max_cells || cells == 0 {
        return Err(FluidError::InvalidConfig(format!(
            "слишком много ячеек поверхности: {cells}; предел {max_cells}"
        )));
    }
    let sx = nx + 1;
    let sy = ny + 1;
    let index = |x: usize, y: usize, z: usize| x + sx * (y + sy * z);
    let count = sx * sy * (nz + 1);
    let mut field = vec![Sample::default(); count];
    let mut grid: HashMap<Cell, Vec<usize>> = HashMap::with_capacity(world.positions().len());
    for (i, &point) in world.positions().iter().enumerate() {
        grid.entry(cell(point, h)).or_default().push(i);
    }
    let kernel = Kernel::new(h);
    for z in 0..=nz {
        for y in 0..=ny {
            for x in 0..=nx {
                if index(x, y, z) & 1023 == 0 && cancelled.load(Ordering::Relaxed) {
                    return Err(FluidError::Cancelled);
                }
                let point = minimum + Vec3::new(x as f32, y as f32, z as f32) * cell_size;
                let mut density = 0.0;
                let mut outward_gradient = Vec3::ZERO;
                if point.cmpge(config.bounds_min).all() && point.cmple(config.bounds_max).all() {
                    let (gx, gy, gz) = cell(point, h);
                    for dz in -1..=1 {
                        for dy in -1..=1 {
                            for dx in -1..=1 {
                                if let Some(bin) = grid.get(&(gx + dx, gy + dy, gz + dz)) {
                                    for &i in bin {
                                        let delta = point - world.positions()[i];
                                        let r2 = delta.length_squared();
                                        density += kernel.poly6(r2) * config.particle_mass
                                            / config.rest_density;
                                        if r2 < kernel.h_squared {
                                            outward_gradient += delta
                                                * (6.0
                                                    * kernel.poly6_factor
                                                    * (kernel.h_squared - r2).powi(2)
                                                    * config.particle_mass
                                                    / config.rest_density);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if density > 0. && world.container().is_some_and(|shape| !shape.contains(point)) {
                    density = 0.; outward_gradient = Vec3::ZERO;
                }
                field[index(x, y, z)] = Sample {
                    position: point,
                    outward_gradient,
                    density,
                };
            }
        }
    }
    let mut mesh = SurfaceBuilder::default();
    let corners = [
        (0, 0, 0),
        (1, 0, 0),
        (1, 1, 0),
        (0, 1, 0),
        (0, 0, 1),
        (1, 0, 1),
        (1, 1, 1),
        (0, 1, 1),
    ];
    let tetrahedra = [
        [0, 5, 1, 6],
        [0, 1, 2, 6],
        [0, 2, 3, 6],
        [0, 3, 7, 6],
        [0, 7, 4, 6],
        [0, 4, 5, 6],
    ];
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                if index(x, y, z) & 1023 == 0 && cancelled.load(Ordering::Relaxed) {
                    return Err(FluidError::Cancelled);
                }
                let cube = corners.map(|(dx, dy, dz)| field[index(x + dx, y + dy, z + dz)]);
                if cube.iter().all(|s| s.density < iso_density)
                    || cube.iter().all(|s| s.density >= iso_density)
                {
                    continue;
                }
                for tet in tetrahedra {
                    polygonize(
                        tet.map(|i| cube[i]),
                        iso_density,
                        config.bounds_min,
                        config.bounds_max,
                        &mut mesh,
                    )?;
                }
            }
        }
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(FluidError::Cancelled);
    }
    if let Some(shape) = world.container() {
        for (i, point) in mesh.mesh.positions.iter_mut().enumerate() {
            if i & 255 == 0 && cancelled.load(Ordering::Relaxed) { return Err(FluidError::Cancelled); }
            let p = Vec3::from_array(*point);
            if !shape.contains(p) {
                let closest = shape.closest(p);
                let normal = (p - closest).normalize_or_zero();
                *point = closest.to_array();
                if normal != Vec3::ZERO { mesh.mesh.normals[i] = normal.to_array(); }
            }
        }
        let positions = &mesh.mesh.positions;
        mesh.mesh.indices = mesh.mesh.indices.chunks_exact(3).filter(|t| {
            let a = Vec3::from_array(positions[t[0] as usize]);
            let b = Vec3::from_array(positions[t[1] as usize]);
            let c = Vec3::from_array(positions[t[2] as usize]);
            (b - a).cross(c - a).length_squared() > 1e-18
        }).flatten().copied().collect();
    }
    stitch_clipped_box_edges(
        &mut mesh.mesh,
        config.bounds_min,
        config.bounds_max,
        cancelled,
    )?;
    for (i, normal) in mesh.mesh.normals.iter_mut().enumerate() {
        if i & 1023 == 0 && cancelled.load(Ordering::Relaxed) {
            return Err(FluidError::Cancelled);
        }
        *normal = Vec3::from_array(*normal).normalize_or_zero().to_array();
    }
    Ok(mesh.mesh)
}

#[derive(Default)]
struct SurfaceBuilder {
    mesh: SurfaceMesh,
    vertices: HashMap<[u32; 3], u32>,
}

/// Clamping a closed field to the container can collapse a cap triangle onto
/// a box edge. Removing that zero-area triangle leaves A--B and B--C opposite
/// an unsplit A--C edge. Split the neighboring face at existing vertex B, so
/// the visible mesh remains closed without exporting degenerate triangles.
fn stitch_clipped_box_edges(
    mesh: &mut SurfaceMesh,
    min: Vec3,
    max: Vec3,
    cancelled: &AtomicBool,
) -> Result<(), FluidError> {
    let mut edge_counts = HashMap::<[u32; 2], (usize, usize)>::new();
    for (i, tri) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
        if i & 1023 == 0 && cancelled.load(Ordering::Relaxed) {
            return Err(FluidError::Cancelled);
        }
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let pa = Vec3::from_array(mesh.positions[tri[a] as usize]);
            let pb = Vec3::from_array(mesh.positions[tri[b] as usize]);
            if let Some(line) = box_edge_line(pa, pb, min, max) {
                let mut edge = [tri[a], tri[b]];
                edge.sort_unstable();
                edge_counts.entry(edge).or_insert((0, line)).0 += 1;
            }
        }
    }
    let mut points: [Vec<u32>; 12] = std::array::from_fn(|_| Vec::new());
    for (edge, (count, line)) in edge_counts {
        if count == 1 {
            points[line].extend(edge);
        }
    }
    if points.iter().all(Vec::is_empty) {
        return Ok(());
    }
    for (line, vertices) in points.iter_mut().enumerate() {
        let axis = line / 4;
        vertices.sort_unstable_by(|&a, &b| {
            mesh.positions[a as usize][axis].total_cmp(&mesh.positions[b as usize][axis])
        });
        vertices.dedup();
    }
    let original = std::mem::take(&mut mesh.indices);
    mesh.indices = Vec::with_capacity(original.len());
    let mut pending = Vec::with_capacity(16);
    let mut processed = 0usize;
    for tri in original.as_chunks::<3>().0 {
        pending.push([tri[0], tri[1], tri[2]]);
        while let Some(tri) = pending.pop() {
            processed += 1;
            if processed & 1023 == 0 && cancelled.load(Ordering::Relaxed) {
                return Err(FluidError::Cancelled);
            }
            let mut split = None;
            for (a, b, c) in [(0, 1, 2), (1, 2, 0), (2, 0, 1)] {
                let pa = Vec3::from_array(mesh.positions[tri[a] as usize]);
                let pb = Vec3::from_array(mesh.positions[tri[b] as usize]);
                if let Some(line) = box_edge_line(pa, pb, min, max) {
                    let axis = line / 4;
                    let lo = pa[axis].min(pb[axis]);
                    let hi = pa[axis].max(pb[axis]);
                    let vertices = &points[line];
                    let next =
                        vertices.partition_point(|&i| mesh.positions[i as usize][axis] <= lo);
                    if let Some(&vertex) = vertices.get(next) {
                        if mesh.positions[vertex as usize][axis] < hi {
                            split = Some(([tri[a], vertex, tri[c]], [vertex, tri[b], tri[c]]));
                            break;
                        }
                    }
                }
            }
            if let Some((first, second)) = split {
                pending.push(second);
                pending.push(first);
            } else {
                if mesh.indices.len() + 3 > 6_000_000 {
                    return Err(FluidError::InvalidConfig(
                        "поверхность превышает предел геометрии; увеличьте размер ячейки".into(),
                    ));
                }
                mesh.indices.extend(tri);
            }
        }
    }
    Ok(())
}

/// Twelve possible lines: varying axis * 4 + the two fixed min/max sides.
fn box_edge_line(a: Vec3, b: Vec3, min: Vec3, max: Vec3) -> Option<usize> {
    let mut varying = None;
    let mut sides = 0;
    let mut fixed = 0;
    for axis in 0..3 {
        if a[axis] != b[axis] {
            if varying.is_some() {
                return None;
            }
            varying = Some(axis);
        } else if a[axis] == min[axis] || a[axis] == max[axis] {
            if a[axis] == max[axis] {
                sides |= 1 << fixed;
            }
            fixed += 1;
        } else {
            return None;
        }
    }
    varying.filter(|_| fixed == 2).map(|axis| axis * 4 + sides)
}

#[derive(Clone, Copy, Default)]
struct Sample {
    position: Vec3,
    outward_gradient: Vec3,
    density: f32,
}

fn edge(mut a: Sample, mut b: Sample, iso: f32, min: Vec3, max: Vec3) -> (Vec3, Vec3) {
    // Canonical ordering makes a shared grid-edge crossing bit-identical even
    // when adjacent tetrahedra enumerate the two endpoints in opposite orders.
    if a.position.to_array() > b.position.to_array() {
        std::mem::swap(&mut a, &mut b);
    }
    // Outside samples deliberately contain zero density to close the container.
    // That discontinuity has no meaningful analytic derivative, so keep the
    // linear/clamped crossing on those edges and refine only interior edges.
    let interior = a.position.cmpge(min).all()
        && a.position.cmple(max).all()
        && b.position.cmpge(min).all()
        && b.position.cmple(max).all();
    let t = if interior {
        hermite_crossing(a, b, iso)
    } else {
        ((iso - a.density) / (b.density - a.density)).clamp(0.0, 1.0)
    };
    let position = a.position.lerp(b.position, t).clamp(min, max);
    let normal = a
        .outward_gradient
        .lerp(b.outward_gradient, t)
        .normalize_or_zero();
    (position, normal)
}

/// A shape-preserving cubic uses density derivatives along the edge instead of
/// assuming that the curved SPH field is linear between grid samples. Limiting
/// the endpoint slopes keeps one bracketed crossing: an overshooting cubic must
/// never create extra lobes or change marching-tetrahedra connectivity.
fn hermite_crossing(a: Sample, b: Sample, iso: f32) -> f32 {
    let difference = b.density - a.density;
    let target = ((iso - a.density) / difference).clamp(0.0, 1.0);
    if target == 0.0 || target == 1.0 || difference.abs() < f32::EPSILON {
        return target;
    }
    let direction = b.position - a.position;
    // The stored vector is the *outward* gradient, i.e. minus grad(density).
    let mut start = (-a.outward_gradient.dot(direction) / difference).max(0.0);
    let mut end = (-b.outward_gradient.dot(direction) / difference).max(0.0);
    if !start.is_finite() || !end.is_finite() {
        return target;
    }
    // This sufficient monotonicity condition also handles noisy sample
    // derivatives near tiny density differences without losing the bracket.
    let magnitude = start.hypot(end);
    if magnitude > 3.0 {
        let scale = 3.0 / magnitude;
        start *= scale;
        end *= scale;
    }
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..12 {
        let t = (low + high) * 0.5;
        let t2 = t * t;
        let t3 = t2 * t;
        let value = (-2.0 * t3 + 3.0 * t2) + (t3 - 2.0 * t2 + t) * start + (t3 - t2) * end;
        if value < target {
            low = t;
        } else {
            high = t;
        }
    }
    (low + high) * 0.5
}

fn polygonize(
    tetra: [Sample; 4],
    iso: f32,
    min: Vec3,
    max: Vec3,
    mesh: &mut SurfaceBuilder,
) -> Result<(), FluidError> {
    let mut inside = [Sample::default(); 4];
    let mut outside = [Sample::default(); 4];
    let mut inside_count = 0;
    let mut outside_count = 0;
    for sample in tetra {
        if sample.density >= iso {
            inside[inside_count] = sample;
            inside_count += 1;
        } else {
            outside[outside_count] = sample;
            outside_count += 1;
        }
    }
    let interpolate = |a, b| edge(a, b, iso, min, max);
    match inside_count {
        1 => triangle(
            [
                interpolate(inside[0], outside[0]),
                interpolate(inside[0], outside[1]),
                interpolate(inside[0], outside[2]),
            ],
            mesh,
        ),
        2 => {
            let a = interpolate(inside[0], outside[0]);
            let b = interpolate(inside[0], outside[1]);
            let c = interpolate(inside[1], outside[1]);
            let d = interpolate(inside[1], outside[0]);
            triangle([a, b, c], mesh)?;
            triangle([a, c, d], mesh)
        }
        3 => triangle(
            [
                interpolate(outside[0], inside[0]),
                interpolate(outside[0], inside[1]),
                interpolate(outside[0], inside[2]),
            ],
            mesh,
        ),
        _ => Ok(()),
    }
}

fn triangle(
    mut vertices: [(Vec3, Vec3); 3],
    builder: &mut SurfaceBuilder,
) -> Result<(), FluidError> {
    let face = (vertices[1].0 - vertices[0].0).cross(vertices[2].0 - vertices[0].0);
    // Keep thin, nonzero faces: clipping to the container can produce slivers
    // one float ULP wide. An arbitrary area threshold opens the neighboring
    // triangles along those edges, even though the original field is closed.
    if face.length_squared() == 0.0 {
        return Ok(());
    }
    let desired = vertices[0].1 + vertices[1].1 + vertices[2].1;
    if face.dot(desired) < 0.0 {
        vertices.swap(1, 2);
    }
    let fallback_normal = (vertices[1].0 - vertices[0].0)
        .cross(vertices[2].0 - vertices[0].0)
        .normalize_or_zero();
    // Fixed output budgets protect the worker even for pathological fields.
    if builder.mesh.positions.len() + 3 > 1_000_000 || builder.mesh.indices.len() + 3 > 6_000_000 {
        return Err(FluidError::InvalidConfig(
            "поверхность превышает предел геометрии; увеличьте размер ячейки".into(),
        ));
    }
    for (position, normal) in vertices {
        let normal = if normal.length_squared() < 0.1 {
            fallback_normal
        } else {
            normal
        };
        // Canonical edge ordering makes shared crossings bit-identical. Normalize
        // signed zero so boundary intersections use one vertex as well.
        let key = position
            .to_array()
            .map(|v| if v == 0.0 { 0 } else { v.to_bits() });
        let index = if let Some(&index) = builder.vertices.get(&key) {
            let accumulated = Vec3::from_array(builder.mesh.normals[index as usize]) + normal;
            builder.mesh.normals[index as usize] = accumulated.to_array();
            index
        } else {
            let index = builder.mesh.positions.len() as u32;
            builder.mesh.positions.push(position.to_array());
            builder.mesh.normals.push(normal.to_array());
            builder.vertices.insert(key, index);
            index
        };
        builder.mesh.indices.push(index);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_reconstruction_reduces_curved_contour_error_without_more_geometry() {
        // An exact quadratic density section is representative of a smooth
        // fluid contour. Linear interpolation visibly contracts this contour;
        // its endpoint derivatives reconstruct the quadratic exactly.
        let sample = |x: f32| Sample {
            position: Vec3::new(x, 0.0, 0.0),
            outward_gradient: Vec3::new(2.0 * x, 0.0, 0.0),
            density: 1.0 - x * x,
        };
        let a = sample(0.2);
        let b = sample(0.8);
        let iso = 0.65;
        let (point, normal) = edge(a, b, iso, Vec3::splat(-2.0), Vec3::splat(2.0));
        let linear_t = (iso - a.density) / (b.density - a.density);
        let linear_x = a.position.lerp(b.position, linear_t).x;
        let linear_error = (1.0 - linear_x * linear_x - iso).abs();
        let refined_error = (1.0 - point.x * point.x - iso).abs();
        assert!(refined_error < linear_error * 0.01);
        assert!(refined_error < 0.0002);
        assert_eq!(normal, Vec3::X);
        // Neighboring tetrahedra enumerate shared edges in reverse order.
        assert_eq!(
            (point, normal),
            edge(b, a, iso, Vec3::splat(-2.0), Vec3::splat(2.0))
        );
    }

    #[test]
    fn poly6_droplet_contour_is_closer_to_its_analytic_radius() {
        let h = 0.14;
        let kernel = Kernel::new(h);
        let particle_volume = 0.512 / 1000.0;
        let iso = 0.1;
        let sample = |x: f32| {
            let q = (h * h - x * x).max(0.0);
            Sample {
                position: Vec3::new(x, 0.0, 0.0),
                outward_gradient: Vec3::X
                    * (6.0 * kernel.poly6_factor * particle_volume * x * q * q),
                density: particle_volume * kernel.poly6(x * x),
            }
        };
        let a = sample(0.04);
        let b = sample(0.10);
        let expected_radius =
            (h * h - (iso / (particle_volume * kernel.poly6_factor)).cbrt()).sqrt();
        let linear_t = (iso - a.density) / (b.density - a.density);
        let linear_radius = a.position.lerp(b.position, linear_t).x;
        let (point, _) = edge(a, b, iso, Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!((point.x - expected_radius).abs() < (linear_radius - expected_radius).abs() * 0.1);
    }

    #[test]
    fn reconstructed_crossings_stay_ordered_and_boundaries_keep_linear_caps() {
        for gradients in [(-1000.0, 1000.0), (1000.0, -1000.0), (0.0, 0.0)] {
            let a = Sample {
                position: Vec3::ZERO,
                outward_gradient: Vec3::X * gradients.0,
                density: 0.0,
            };
            let b = Sample {
                position: Vec3::X,
                outward_gradient: Vec3::X * gradients.1,
                density: 1.0,
            };
            let mut previous = 0.0;
            for step in 0..=100 {
                let t = hermite_crossing(a, b, step as f32 / 100.0);
                assert!(t.is_finite() && (previous..=1.0).contains(&t));
                previous = t;
            }
            // Endpoint a is outside this container. Its synthetic zero field
            // must not acquire a curved continuation through the wall.
            let (point, _) = edge(a, b, 0.5, Vec3::splat(0.25), Vec3::splat(2.0));
            assert_eq!(point, Vec3::new(0.5, 0.25, 0.25));
        }
    }

    #[test]
    fn cancellation_never_returns_a_partial_surface() {
        let world = FluidWorld::demo();
        assert!(matches!(
            extract_surface_cancellable(&world, 0.05, 0.5, &AtomicBool::new(true)),
            Err(FluidError::Cancelled)
        ));
    }

    #[test]
    fn shared_surface_vertices_are_indexed_and_default_domain_fits_finest_ui_grid() {
        let world = FluidWorld::demo();
        let mesh = extract_surface(&world, 0.045, 0.5).unwrap();
        assert!(
            mesh.positions.len() * 2 < mesh.indices.len(),
            "shared crossings should substantially reduce output storage"
        );
        let mut positions = std::collections::BTreeSet::new();
        for p in &mesh.positions {
            assert!(positions.insert(p.map(|v| if v == 0.0 { 0 } else { v.to_bits() })));
        }
        assert!(mesh
            .indices
            .iter()
            .all(|&index| index < mesh.positions.len() as u32));
        // A sparse state spanning the whole default container exercises grid sizing,
        // not just the initial central volume. It must fit the finest exposed UI step.
        let cfg = world.config().clone();
        let min = cfg.bounds_min + Vec3::splat(cfg.particle_radius);
        let max = cfg.bounds_max - Vec3::splat(cfg.particle_radius);
        let sparse = FluidWorld::new(cfg, vec![min, max]).unwrap();
        assert!(extract_surface(&sparse, 0.025, 0.5).is_ok());
    }

    #[test]
    fn surface_has_finite_oriented_triangles_and_obeys_bounds() {
        let world = FluidWorld::demo();
        let mesh = extract_surface(&world, 0.06, 0.5).unwrap();
        assert!(mesh.indices.len() > 100);
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        for (position, normal) in mesh.positions.iter().zip(&mesh.normals) {
            let point = Vec3::from_array(*position);
            let normal = Vec3::from_array(*normal);
            assert!(point.is_finite() && normal.is_finite());
            assert!((normal.length() - 1.0).abs() < 1e-4);
            assert!(point.cmpge(world.config().bounds_min).all());
            assert!(point.cmple(world.config().bounds_max).all());
        }
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let a = Vec3::from_array(mesh.positions[triangle[0] as usize]);
            let b = Vec3::from_array(mesh.positions[triangle[1] as usize]);
            let c = Vec3::from_array(mesh.positions[triangle[2] as usize]);
            let normals: Vec3 = triangle
                .iter()
                .map(|&i| Vec3::from_array(mesh.normals[i as usize]))
                .sum();
            assert!((b - a).cross(c - a).dot(normals) >= -1e-7);
        }
    }

    #[test]
    fn surface_budget_is_checked_before_allocating_grid() {
        let world = FluidWorld::demo();
        assert!(extract_surface(&world, 0.005, 0.5).is_err());
        assert!(extract_surface(&world, f32::NAN, 0.5).is_err());
        assert!(extract_surface(&world, 0.05, 0.0).is_err());
    }

    #[test]
    fn initial_surface_has_no_open_edges() {
        // Canonical edge interpolation allows exact welding for this check.
        let mesh = extract_surface(&FluidWorld::demo(), 0.05, 0.5).unwrap();
        assert_closed_surface(&mesh);
    }

    #[test]
    fn evolved_surface_clipped_at_box_corners_remains_closed() {
        let mut world = FluidWorld::demo();
        for _ in 0..120 {
            world.step();
        }
        for cell_size in [0.045, 0.025] {
            let mesh = extract_surface(&world, cell_size, 0.5).unwrap();
            assert_closed_surface(&mesh);
            for tri in mesh.indices.as_chunks::<3>().0 {
                let a = Vec3::from_array(mesh.positions[tri[0] as usize]);
                let b = Vec3::from_array(mesh.positions[tri[1] as usize]);
                let c = Vec3::from_array(mesh.positions[tri[2] as usize]);
                assert!((b - a).cross(c - a).length_squared() > 0.0);
            }
        }
    }

    fn assert_closed_surface(mesh: &SurfaceMesh) {
        let quantize = |p: [f32; 3]| p.map(f32::to_bits);
        let mut edges = std::collections::BTreeMap::new();
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let points = triangle
                .iter()
                .map(|&index| quantize(mesh.positions[index as usize]))
                .collect::<Vec<_>>();
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let (a, b) = if points[a] < points[b] {
                    (points[a], points[b])
                } else {
                    (points[b], points[a])
                };
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        assert!(!edges.is_empty());
        assert!(
            edges.values().all(|&count| count == 2),
            "every edge of the closed reference volume must have two faces: {:?}",
            edges
                .iter()
                .filter(|(_, &count)| count != 2)
                .take(12)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn final_surface_keeps_preview_results_at_the_same_resolution_and_is_cancellable() {
        let world = FluidWorld::demo();
        let preview = extract_surface(&world, 0.08, 0.6).unwrap();
        let render = extract_surface_render_cancellable(&world, 0.08, 0.6, &AtomicBool::new(false)).unwrap();
        assert_eq!(render.positions, preview.positions); assert_eq!(render.indices, preview.indices);
        assert!(matches!(extract_surface_render_cancellable(&world, 0.005, 0.6, &AtomicBool::new(true)), Err(FluidError::Cancelled)));
        assert!(extract_surface_render_cancellable(&world, f32::NAN, 0.6, &AtomicBool::new(false)).is_err());
    }

}
