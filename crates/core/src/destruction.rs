//! Persistent object-owned fracture settings and the current generated result.
use crate::CoreError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DestructionSettings {
    /// Pieces, seed, spread, impulse, gravity, floor, restitution, frame count.
    pub values: [f64; 8],
    /// Uniform shrink of each shard, in percent; zero keeps exact cut boundaries.
    pub seam_percent: f64,
    /// Darkening of newly exposed interior vertex colors; exterior paint is preserved.
    pub cut_contrast: f64,
    /// Include the receiver's selected mesh obstacles in preview and baked motion.
    pub mesh_collision: bool,
    /// Dynamic contacts between enabled fragments, in preview and baked motion.
    pub shard_collision: bool,
    pub randomize: bool,
    pub floor_enabled: bool,
    /// Editor helper only; never exported as scene geometry.
    pub show_floor: bool,
    pub dust_enabled: bool,
    pub dust_density: f64,
    pub dust_size: f64,
    pub dust_lifetime: f64,
    /// Dedicated material for newly exposed cut faces; never an all-face overlay.
    pub interior_material: Option<u64>,
    pub visible: bool,
    pub result: Option<u64>,
    pub source_visible: bool,
    pub source_render_visible: bool,
}
impl Default for DestructionSettings {
    fn default() -> Self {
        Self { values: [12., 1., 0., 1.5, 9.81, 0., 0.25, 72.], seam_percent: 0.6, cut_contrast: 0.65, visible: true,
            mesh_collision: true, shard_collision: false, interior_material: None,
            randomize: false, floor_enabled: true, show_floor: false,
            dust_enabled: false, dust_density: 0.8, dust_size: 1., dust_lifetime: 3.,
            result: None, source_visible: true, source_render_visible: true }
    }
}
impl DestructionSettings {
    pub fn validate(self) -> Result<(), CoreError> {
        for (i, (min, max)) in [(2., 64.), (0., 999999.), (0., 10.), (0., 30.), (0., 30.), (-100., 100.), (0., 0.95), (2., 240.)].into_iter().enumerate() {
            let v = self.values[i];
            if !v.is_finite() || v < min || v > max || (matches!(i, 0 | 1 | 7) && v.fract() != 0.) {
                return Err(CoreError::Invalid("Некорректные параметры разрушения".into()));
            }
        }
        if !self.seam_percent.is_finite() || !(0. ..=5.).contains(&self.seam_percent)
            || !self.cut_contrast.is_finite() || !(0. ..=1.).contains(&self.cut_contrast) {
            return Err(CoreError::Invalid("Некорректная ширина или контраст разрезов".into()));
        }
        for (value, min, max) in [(self.dust_density, 0., 5.), (self.dust_size, 0.1, 5.), (self.dust_lifetime, 0.2, 20.)] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(CoreError::Invalid("Некорректные параметры пыли".into()));
            }
        }
        Ok(())
    }
    pub fn collision_floor(self) -> f32 {
        if self.floor_enabled { self.values[5] as f32 } else { f32::NEG_INFINITY }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{box_mesh, Editor, Node, Project, Vec3};

    #[test]
    fn old_projects_keep_floor_and_new_dust_fields_round_trip_and_validate() {
        let legacy:DestructionSettings=serde_json::from_str(r#"{"values":[12,1,0,1.5,9.81,0,0.25,72]}"#).unwrap();
        assert!(legacy.floor_enabled);assert!(!legacy.show_floor && !legacy.dust_enabled && !legacy.randomize);
        let mut s=DestructionSettings {floor_enabled:false,show_floor:true,dust_enabled:true,dust_density:2.,dust_size:3.,dust_lifetime:4.,randomize:true,..legacy};
        assert_eq!(s.collision_floor(),f32::NEG_INFINITY);s.validate().unwrap();
        let again:DestructionSettings=serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();assert_eq!(again,s);
        for invalid in [f64::NAN,-1.,21.] {s.dust_lifetime=invalid;assert!(s.validate().is_err());}
    }
    fn scene() -> Project {
        let mut source = Node::new(1, "Source");
        source.mesh = Some(2);
        source.visible = false; source.render_visible = false;
        source.destruction = Some(DestructionSettings { result: Some(3), ..Default::default() });
        Project {
            nodes: vec![source, Node::new(3, "Result")],
            meshes: vec![box_mesh(2, "Cube", Vec3::ONE, None)],
            ..Default::default()
        }
    }

    #[test]
    fn deleting_source_removes_result_but_deleting_result_restores_source() {
        let before = Editor::new(scene()).project; before.validate().unwrap();
        for id in [1, 3] {
            let mut editor = Editor::new(before.clone());
            editor.delete_node(id).unwrap();
            assert!(editor.project.node(3).is_none());
            if id == 1 { assert!(editor.project.node(1).is_none()); }
            else {
                let source = editor.project.node(1).unwrap();
                assert!(source.visible && source.render_visible);
                assert!(source.destruction.unwrap().result.is_none());
            }
            editor.project.validate().unwrap();
            editor.undo(); assert_eq!(editor.project, before);
        }
    }

    #[test]
    fn copying_source_keeps_parameters_without_stealing_original_result() {
        let mut p = scene();
        p.node_mut(1).unwrap().destruction.as_mut().unwrap().values[0] = 23.;
        let copy = p.duplicate_subtree(1).unwrap();
        let node = p.node(copy).unwrap();
        assert!(node.visible && node.render_visible);
        assert_eq!(node.destruction.unwrap().values[0], 23.);
        assert!(node.destruction.unwrap().result.is_none());
        assert_eq!(p.node(1).unwrap().destruction.unwrap().result, Some(3));
        p.validate().unwrap();
    }

    #[test]
    fn result_cannot_contain_its_source_or_be_a_mesh() {
        let mut p = scene();
        p.node_mut(1).unwrap().parent = Some(3);
        assert!(p.validate().is_err());
        p.node_mut(1).unwrap().parent = None;
        p.node_mut(3).unwrap().mesh = Some(2);
        assert!(p.validate().is_err());
    }
}
