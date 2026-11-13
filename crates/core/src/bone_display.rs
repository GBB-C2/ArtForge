//! Editor-only per-joint appearance; never changes skinning or authored transforms.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum BoneShape {
    #[default]
    Octahedron,
    Rectangle,
    Triangle,
    Box,
    Cylinder,
    Cone,
}

impl BoneShape {
    pub const ALL: [Self; 6] = [
        Self::Octahedron,
        Self::Rectangle,
        Self::Triangle,
        Self::Box,
        Self::Cylinder,
        Self::Cone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Octahedron => "Гранёная",
            Self::Rectangle => "Прямоугольник",
            Self::Triangle => "Треугольник",
            Self::Box => "Объёмный блок",
            Self::Cylinder => "Цилиндр",
            Self::Cone => "Конус",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum JointShape {
    #[default]
    Circle,
    Rectangle,
    Diamond,
    Triangle,
    Hexagon,
}

impl JointShape {
    pub const ALL: [Self; 5] = [
        Self::Circle,
        Self::Rectangle,
        Self::Diamond,
        Self::Triangle,
        Self::Hexagon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Circle => "Круг",
            Self::Rectangle => "Прямоугольник",
            Self::Diamond => "Ромб",
            Self::Triangle => "Треугольник",
            Self::Hexagon => "Шестиугольник",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BoneDisplay {
    /// Encoded sRGB, like the editor overlay palette (not scene-linear material RGB).
    pub color: [f32; 3],
    pub joint_color: [f32; 3],
    pub shape: BoneShape,
    pub joint_shape: JointShape,
    /// Display-only width multiplier; the bone endpoints and bind pose stay fixed.
    pub size: f32,
    /// Display-only joint marker multiplier, shared by drawing and picking.
    pub joint_size: f32,
    pub filled: bool,
    pub joint_filled: bool,
    pub follow_view: bool,
    pub joint_follow_view: bool,
}

impl Default for BoneDisplay {
    fn default() -> Self {
        Self {
            color: [0.58, 0.62, 0.64],
            joint_color: [0.58, 0.62, 0.64],
            shape: BoneShape::Octahedron,
            joint_shape: JointShape::Circle,
            size: 1.,
            joint_size: 1.,
            filled: true,
            joint_filled: true,
            follow_view: false,
            joint_follow_view: true,
        }
    }
}

impl BoneDisplay {
    pub fn is_valid(&self) -> bool {
        self.color
            .iter()
            .chain(&self.joint_color)
            .all(|v| v.is_finite() && (0. ..=1.).contains(v))
            && [self.size, self.joint_size]
                .iter()
                .all(|v| v.is_finite() && (0.1..=5.).contains(v))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoneDisplayPreset {
    Classic,
    Outline,
    Blocks,
    Slim,
    Controls,
}

impl BoneDisplayPreset {
    pub const ALL: [Self; 5] = [
        Self::Classic,
        Self::Outline,
        Self::Blocks,
        Self::Slim,
        Self::Controls,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Стандартное",
            Self::Outline => "Контур",
            Self::Blocks => "Блоки",
            Self::Slim => "Тонкие кости",
            Self::Controls => "Контроллеры",
        }
    }

    pub fn display(self) -> BoneDisplay {
        let mut d = BoneDisplay::default();
        match self {
            Self::Classic => {}
            Self::Outline => {
                d.filled = false;
                d.joint_filled = false;
            }
            Self::Blocks => {
                d.shape = BoneShape::Box;
                d.joint_shape = JointShape::Rectangle;
            }
            Self::Slim => {
                d.shape = BoneShape::Cylinder;
                d.size = 0.35;
                d.joint_size = 0.65;
            }
            Self::Controls => {
                d.shape = BoneShape::Rectangle;
                d.joint_shape = JointShape::Diamond;
                d.follow_view = true;
                d.filled = false;
                d.joint_filled = false;
                d.joint_size = 1.5;
            }
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_and_partial_display_data_keep_the_existing_skeleton_style() {
        let legacy = crate::Node::new(1, "Joint");
        let json = serde_json::to_value(&legacy).unwrap();
        assert!(json.get("bone_display").is_none());
        assert_eq!(serde_json::from_value::<crate::Node>(json).unwrap(), legacy);
        let partial: BoneDisplay = serde_json::from_str(r#"{"shape":"Rectangle"}"#).unwrap();
        assert_eq!(
            partial,
            BoneDisplay {
                shape: BoneShape::Rectangle,
                ..Default::default()
            }
        );
    }

    #[test]
    fn size_defaults_keep_old_files_valid_and_reject_invalid_scales() {
        let old: BoneDisplay = serde_json::from_str(r#"{"color":[0.2,0.4,0.8]}"#).unwrap();
        assert_eq!((old.size, old.joint_size), (1., 1.));
        for value in [0., -1., 5.01, f32::NAN, f32::INFINITY] {
            assert!(!BoneDisplay { size: value, ..old }.is_valid());
            assert!(!BoneDisplay {
                joint_size: value,
                ..old
            }
            .is_valid());
        }
        for preset in BoneDisplayPreset::ALL {
            assert!(preset.display().is_valid());
        }
    }

    #[test]
    fn appearance_roundtrips_and_invalid_colors_are_rejected() {
        let mut joint = crate::Node::new(1, "Joint");
        joint.is_joint = true;
        joint.bone_display = Some(BoneDisplay {
            color: [0.1, 0.8, 0.2],
            joint_color: [0.7, 0.2, 0.9],
            shape: BoneShape::Rectangle,
            joint_shape: JointShape::Rectangle,
            filled: false,
            joint_filled: false,
            follow_view: true,
            joint_follow_view: false,
            ..Default::default()
        });
        let mut project = crate::Project {
            nodes: vec![joint],
            ..Default::default()
        };
        project.validate().unwrap();
        for shape in BoneShape::ALL {
            for joint_shape in JointShape::ALL {
                let d = project.nodes[0].bone_display.as_mut().unwrap();
                d.shape = shape;
                d.joint_shape = joint_shape;
                let json = serde_json::to_string(&project).unwrap();
                assert_eq!(
                    serde_json::from_str::<crate::Project>(&json).unwrap(),
                    project
                );
            }
        }
        for bad in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
            project.nodes[0].bone_display.as_mut().unwrap().color[0] = bad;
            assert!(project.validate().is_err());
        }
    }
}
