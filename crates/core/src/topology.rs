//! Editable polygon boundaries, independent from render triangles.
use crate::{CoreError, Mesh, SharedBuffer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshTopology {
    /// Per-polygon overrides. Empty means every face uses the mesh material.
    #[serde(default)]
    pub face_materials: SharedBuffer<Option<crate::AssetId>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uv_seams: Vec<[u32; 2]>,
    pub faces: SharedBuffer<Vec<u32>>,
    /// Keep the normal policy after the parametric generator is detached.
    #[serde(default)]
    pub smooth_normals: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive: Option<crate::PrimitiveSettings>,
}

impl Mesh {
    pub fn face_material(&self, face: usize) -> Option<crate::AssetId> {
        self.topology
            .as_ref()
            .and_then(|t| t.face_materials.get(face).copied().flatten())
            .or(self.material)
    }
    pub fn assigned_materials(&self) -> std::collections::BTreeSet<crate::AssetId> {
        self.material
            .into_iter()
            .chain(
                self.topology
                    .iter()
                    .flat_map(|t| t.face_materials.iter().filter_map(|m| *m)),
            )
            .collect()
    }
    pub fn clear_face_materials(&mut self) {
        if let Some(t) = &mut self.topology {
            t.face_materials = Default::default();
        }
    }

    pub fn smooth_normals_enabled(&self) -> bool {
        self.topology
            .as_ref()
            .is_some_and(|t| t.primitive.map_or(t.smooth_normals, |p| p.smooth))
    }
    pub fn face_count(&self) -> usize {
        self.topology
            .as_ref()
            .map_or(self.indices.len() / 3, |t| t.faces.len())
    }
    pub fn face_vertices(&self, face: usize) -> Option<&[u32]> {
        if let Some(t) = &self.topology {
            t.faces.get(face).map(Vec::as_slice)
        } else {
            self.indices
                .get(face.checked_mul(3)?..face.checked_mul(3)?.checked_add(3)?)
        }
    }
    pub fn triangle_face(&self, triangle: usize) -> Option<usize> {
        if triangle >= self.indices.len() / 3 {
            return None;
        }
        let Some(t) = &self.topology else {
            return Some(triangle);
        };
        let mut end = 0;
        t.faces.iter().position(|f| {
            end += f.len() - 2;
            triangle < end
        })
    }
    pub fn rebuild_polygon_indices(&mut self) -> Result<(), CoreError> {
        if let Some(t) = &self.topology {
            let mut indices = Vec::new();
            for face in &t.faces {
                indices.extend(crate::triangulate::triangulate(self, face)?);
            }
            self.indices = indices.into();
        }
        Ok(())
    }
    pub fn validate_topology(&self) -> Result<(), CoreError> {
        let Some(t) = &self.topology else {
            return Ok(());
        };
        let invalid = || CoreError::Invalid("Повреждена полигональная сетка".into());
        if (!t.face_materials.is_empty() && t.face_materials.len() != t.faces.len())
            || t.faces.is_empty()
            || t.faces.len() > self.indices.len() / 3
        {
            return Err(invalid());
        }
        if t.uv_seams.len() > self.indices.len()
            || t.uv_seams.iter().any(|&[a, b]| {
                a == b || a as usize >= self.vertices.len() || b as usize >= self.vertices.len()
            })
        {
            return Err(invalid());
        }
        let mut indices = self.indices.iter();
        for face in &t.faces {
            if !(3..=crate::MAX_FACE_CORNERS).contains(&face.len())
                || face.iter().any(|i| *i as usize >= self.vertices.len())
                || face.iter().enumerate().any(|(i, v)| face[..i].contains(v))
            {
                return Err(invalid());
            }
            for index in crate::triangulate::triangulate(self, face)? {
                if indices.next() != Some(&index) {
                    return Err(invalid());
                }
            }
        }
        if indices.next().is_some() {
            return Err(invalid());
        }
        if let Some(p) = t.primitive {
            p.validate()?;
        }
        Ok(())
    }
}
