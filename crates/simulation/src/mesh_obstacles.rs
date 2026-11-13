//! Static, two-sided mesh obstacles. Closed meshes also exclude initial fluid volume.
use crate::mesh_contact::{MeshCollider, MeshVolume, Triangle};
use glam::Vec3;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct MeshObstacles {
    surface: MeshCollider,
    solids: Vec<MeshVolume>,
    signature: u64,
}

impl MeshObstacles {
    /// Build once on a worker, then share the immutable acceleration structure.
    /// Open meshes are valid surfaces; only watertight meshes have a solid interior.
    pub fn new(meshes: Vec<Vec<Triangle>>, cancel: &AtomicBool) -> Result<Self, String> {
        let count = meshes.iter().map(Vec::len).sum::<usize>();
        if count > 200_000 || meshes.len() > 64 {
            return Err("Столкновения: не более 64 мешей и 200000 треугольников суммарно".into());
        }
        let mut triangles = Vec::with_capacity(count);
        let mut solids = Vec::new();
        let mut signature = 0xcbf29ce484222325u64;
        let mut hash = |bytes: &[u8]| {
            for &byte in bytes { signature = (signature ^ u64::from(byte)).wrapping_mul(0x100000001b3); }
        };
        for mesh in meshes {
            if cancel.load(Ordering::Relaxed) { return Err("Подготовка столкновений отменена".into()); }
            // Mesh boundaries affect solid occupancy, even with identical combined triangles.
            hash(&(mesh.len() as u64).to_le_bytes());
            for (i, t) in mesh.iter().enumerate() {
                if i % 256 == 0 && cancel.load(Ordering::Relaxed) { return Err("Подготовка столкновений отменена".into()); }
                for p in t.0 {
                    if !p.is_finite() { return Err("Коллайдер содержит некорректные координаты".into()); }
                    for v in p.to_array() { hash(&v.to_bits().to_le_bytes()); }
                }
            }
            if let Ok(volume) = MeshVolume::new(mesh.clone()) { solids.push(volume); }
            triangles.extend(mesh);
        }
        let surface = MeshCollider::new(triangles)?;
        if cancel.load(Ordering::Relaxed) { return Err("Подготовка столкновений отменена".into()); }
        Ok(Self { surface, solids, signature })
    }
    pub fn signature(&self) -> u64 { self.signature }
    pub fn surface(&self) -> &MeshCollider { &self.surface }
    pub fn contains(&self, point: Vec3) -> bool { self.solids.iter().any(|solid| solid.contains(point)) }

    pub fn accepts_particle(&self, point: Vec3, radius: f32, scratch: &mut Vec<usize>) -> bool {
        if self.contains(point) { return false; }
        self.surface.candidates(point, point, radius, scratch);
        !scratch.iter().any(|&i| self.surface.triangle(i).closest(point).0.distance_squared(point) < radius * radius)
    }
    pub fn project(&self, start: Vec3, end: Vec3, radius: f32, scratch: &mut Vec<usize>) -> Vec3 {
        let first = self.surface.project(start, end, radius, scratch);
        self.surface.project(start, first, radius, scratch)
    }
}
