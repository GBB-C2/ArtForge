//! Conservative occupancy of selected meshes, independent of camera depth/material alpha.
//! Triangle cells block open surfaces too; connected free regions are classified only once.
use crate::{mesh_contact::Triangle, mesh_obstacles::MeshObstacles};
use glam::{Mat4, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};

pub const MASK_EDGE: usize = 32;
pub const MASK_CELLS: usize = MASK_EDGE * MASK_EDGE * MASK_EDGE;

fn spend(budget: &mut usize, cost: usize, cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) { return Err("Подготовка столкновений FX отменена".into()); }
    if cost > *budget { return Err("FX: превышен бюджет объёма столкновений; упростите препятствия".into()); }
    *budget -= cost; Ok(())
}
fn index(x: usize, y: usize, z: usize) -> usize { (z * MASK_EDGE + y) * MASK_EDGE + x }
fn center(i: usize) -> Vec3 {
    Vec3::new((i % MASK_EDGE) as f32, (i / MASK_EDGE % MASK_EDGE) as f32, (i / (MASK_EDGE * MASK_EDGE)) as f32) + Vec3::splat(0.5)
}

/// 255 means free; 0 means a surface cell or solid interior. Dimensions are fixed,
/// triangle tests and visited cells are budgeted; errors never return a partial mask.
pub fn build(meshes: &[&[Triangle]], obstacles: &MeshObstacles, model: Mat4, low: Vec3, high: Vec3,
    cancel: &AtomicBool, budget: &mut usize) -> Result<Vec<u8>, String> {
    build_with_interior(meshes,obstacles,model,low,high,cancel,budget,true)
}
pub fn build_with_interior(meshes:&[&[Triangle]],obstacles:&MeshObstacles,model:Mat4,low:Vec3,high:Vec3,
    cancel:&AtomicBool,budget:&mut usize,fill_interior:bool)->Result<Vec<u8>,String> {
    spend(budget, 0, cancel)?;
    let extent = high - low;
    if !model.is_finite() || model.determinant().abs() < 1e-8 || !model.inverse().is_finite()
        || !low.is_finite() || !high.is_finite() || extent.min_element() <= 0. {
        return Err("FX: некорректные границы столкновений".into());
    }
    let to_grid = Mat4::from_scale(Vec3::splat(MASK_EDGE as f32) / extent)
        * Mat4::from_translation(-low) * model.inverse();
    let mut mask = vec![255; MASK_CELLS];
    // A cell lies inside this sphere. Conservative rasterization cannot skip a thin wall.
    let radius = 3_f32.sqrt() * 0.5 + 0.0001;
    for triangles in meshes {
        for triangle in *triangles {
            spend(budget, 1, cancel)?;
            let triangle = Triangle(triangle.0.map(|p| to_grid.transform_point3(p)));
            if triangle.0.iter().any(|p| !p.is_finite()) { return Err("FX: нечисловая геометрия препятствия".into()); }
            let min = triangle.0[0].min(triangle.0[1]).min(triangle.0[2]) - Vec3::splat(radius);
            let max = triangle.0[0].max(triangle.0[1]).max(triangle.0[2]) + Vec3::splat(radius);
            if max.cmplt(Vec3::ZERO).any() || min.cmpge(Vec3::splat(MASK_EDGE as f32)).any() { continue; }
            let bound = |v: Vec3| v.floor().clamp(Vec3::ZERO, Vec3::splat((MASK_EDGE-1) as f32)).to_array().map(|n| n as usize);
            let a = bound(min); let b = bound(max);
            for z in a[2]..=b[2] { for y in a[1]..=b[1] {
                spend(budget, b[0]-a[0]+1, cancel)?;
                for x in a[0]..=b[0] {
                    let i = index(x,y,z); if mask[i] == 0 { continue; }
                    let p = center(i);
                    if triangle.closest(p).0.distance_squared(p) <= radius*radius { mask[i] = 0; }
                }
            }}
        }
    }
    if !fill_interior {return Ok(mask);}
    let triangles: usize = meshes.iter().map(|m|m.len()).sum();
    let mut visited = vec![false; MASK_CELLS]; let mut component = Vec::new();
    for root in 0..MASK_CELLS {
        if mask[root] == 0 || visited[root] { continue; }
        // contains() can test at most this many triangles across all closed meshes.
        spend(budget, triangles, cancel)?;
        let p = model.transform_point3(low + center(root) / MASK_EDGE as f32 * extent);
        let solid = obstacles.contains(p);
        component.clear(); component.push(root); visited[root] = true; let mut cursor = 0;
        while cursor < component.len() {
            spend(budget, 1, cancel)?;
            let i = component[cursor]; cursor += 1;
            if solid { mask[i] = 0; }
            let x = i % MASK_EDGE; let y = i / MASK_EDGE % MASK_EDGE; let z = i / (MASK_EDGE*MASK_EDGE);
            for (inside, neighbor) in [
                (x>0,i.wrapping_sub(1)),(x+1<MASK_EDGE,i+1),
                (y>0,i.wrapping_sub(MASK_EDGE)),(y+1<MASK_EDGE,i+MASK_EDGE),
                (z>0,i.wrapping_sub(MASK_EDGE*MASK_EDGE)),(z+1<MASK_EDGE,i+MASK_EDGE*MASK_EDGE),
            ] {
                if inside && !visited[neighbor] && mask[neighbor] != 0 {
                    visited[neighbor] = true; component.push(neighbor);
                }
            }
        }
    }
    spend(budget, 0, cancel)?; Ok(mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;
    fn cube(half:f32)->Vec<Triangle> {
        let points=[[-1.,-1.,-1.],[1.,-1.,-1.],[1.,1.,-1.],[-1.,1.,-1.],
            [-1.,-1.,1.],[1.,-1.,1.],[1.,1.,1.],[-1.,1.,1.]].map(|p|Vec3::from_array(p)*half);
        [[0,2,1],[0,3,2],[4,5,6],[4,6,7],[0,1,5],[0,5,4],
            [3,7,6],[3,6,2],[0,4,7],[0,7,3],[1,2,6],[1,6,5]].map(|t|Triangle(t.map(|i|points[i]))).to_vec()
    }
    fn mask(mesh:Vec<Triangle>,model:Mat4)->Vec<u8> {
        let obstacles=MeshObstacles::new(vec![mesh.clone()],&AtomicBool::new(false)).unwrap();
        build(&[mesh.as_slice()],&obstacles,model,Vec3::splat(-1.),Vec3::ONE,&AtomicBool::new(false),&mut 1_000_000).unwrap()
    }
    #[test]
    fn closed_mesh_blocks_its_interior_and_keeps_exterior_free() {
        let result=mask(cube(0.5),Mat4::IDENTITY);
        assert_eq!(result[index(16,16,16)],0);assert_eq!(result[index(2,16,16)],255);
        assert_eq!(result[index(24,16,16)],0,"surface cells must also be blocked");
        assert_eq!(result.len(),MASK_CELLS);
    }
    #[test]
    fn hollow_room_preserves_its_interior_and_still_blocks_the_shell() {
        let mesh=cube(0.5);let cancel=AtomicBool::new(false);let obstacles=MeshObstacles::new(vec![mesh.clone()],&cancel).unwrap();
        let result=build_with_interior(&[mesh.as_slice()],&obstacles,Mat4::IDENTITY,Vec3::splat(-1.),Vec3::ONE,&cancel,&mut 1_000_000,false).unwrap();
        assert_eq!(result[index(16,16,16)],255);assert_eq!(result[index(24,16,16)],0);
    }
    #[test]
    fn thin_triangle_blocks_both_sides_without_filling_its_empty_bounds() {
        let triangle=Triangle([Vec3::new(-0.9,0.,-0.9),Vec3::new(0.9,0.,-0.9),Vec3::new(-0.9,0.,0.9)]);
        let result=mask(vec![triangle],Mat4::IDENTITY);
        assert_eq!(result[index(8,15,8)],0);assert_eq!(result[index(8,16,8)],0);
        assert_eq!(result[index(26,16,26)],255,"outside actual triangle, inside its AABB");
        assert_eq!(result[index(8,10,8)],255);assert_eq!(result[index(8,22,8)],255);
        assert_eq!(mask(vec![Triangle([triangle.0[2],triangle.0[1],triangle.0[0]])],Mat4::IDENTITY),result);
    }
    #[test]
    fn transformed_domain_and_enclosing_obstacle_have_correct_occupancy() {
        let model=Mat4::from_scale_rotation_translation(Vec3::new(2.,0.75,1.3),Quat::from_rotation_z(0.7),Vec3::new(3.,2.,-1.));
        let transformed=cube(0.5).into_iter().map(|t|Triangle(t.0.map(|p|model.transform_point3(p)))).collect();
        assert_eq!(mask(transformed,model),mask(cube(0.5),Mat4::IDENTITY));
        assert!(mask(cube(2.),Mat4::IDENTITY).iter().all(|&cell|cell==0),"solid can enclose domain without intersecting its cells");
    }
    #[test]
    fn cancellation_and_budget_exhaustion_never_return_partial_masks() {
        let mesh=cube(0.5);let obstacles=MeshObstacles::new(vec![mesh.clone()],&AtomicBool::new(false)).unwrap();
        for (cancel,budget) in [(true,1_000_000),(false,1)] {
            assert!(build(&[mesh.as_slice()],&obstacles,Mat4::IDENTITY,Vec3::splat(-1.),Vec3::ONE,&AtomicBool::new(cancel),&mut {budget}).is_err());
        }
        assert!(build(&[mesh.as_slice()],&obstacles,Mat4::ZERO,Vec3::splat(-1.),Vec3::ONE,&AtomicBool::new(false),&mut 1_000_000).is_err());
    }
}
