//! Per-bone overlay appearance shares the editor's color transaction and history.
use super::*;
use crate::ui::{BoneDisplayAction as B, DropdownKind};
use poseforge_core::BoneDisplay;

pub(super) const HEIGHT: f32 = 460.;

impl App {
    pub(super) fn bone_display_node(&self) -> Option<&poseforge_core::Node> {
        if self.editor.mode != EditorMode::Pose || self.authoring.rig_stage != 0 {
            return None;
        }
        self.editor
            .selected
            .and_then(|id| self.editor.project.node(id))
            .filter(|n| n.is_joint)
    }

    pub(super) fn bone_display_editable(&self) -> bool {
        self.bone_display_node()
            .is_some_and(|n| self.node_selectable(n.id))
            && self.job.is_none()
            && self.native_picker.is_none()
            && !self.scene_write_pending()
            && !self.editor.has_staged()
    }

    pub(super) fn bone_display_action(&mut self, action: B) {
        if !self.bone_display_editable() {
            return;
        }
        if matches!(action, B::BodyColor | B::JointColor) {
            self.open_color_picker(Action::BoneDisplay(action));
            return;
        }
        let node = self.bone_display_node().unwrap();
        let id = node.id;
        let previous = node.bone_display.unwrap_or_default();
        let mut display = previous;
        match action {
            B::BodyShape(shape) => display.shape = shape,
            B::JointShape(shape) => display.joint_shape = shape,
            B::ResetBodyColor => display.color = BoneDisplay::default().color,
            B::ResetJointColor => display.joint_color = BoneDisplay::default().joint_color,
            B::BodyFilled => display.filled = !display.filled,
            B::JointFilled => display.joint_filled = !display.joint_filled,
            B::BodyFollowView => display.follow_view = !display.follow_view,
            B::JointFollowView => display.joint_follow_view = !display.joint_follow_view,
            B::Reset => display = BoneDisplay::default(),
            _ => return,
        }
        if action != B::Reset && display == previous {
            return;
        }
        let value = if action == B::Reset {
            None
        } else {
            Some(display)
        };
        if node.bone_display == value {
            return;
        }
        let result = self.editor.transact("Оформление кости", |p| {
            p.node_mut(id).unwrap().bone_display = value
        });
        if self.result(result).is_some() {
            self.update_dirty();
            self.redraw();
        }
    }

    pub(super) fn draw_bone_display(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let Some(node) = self.bone_display_node() else {
            return;
        };
        let d = node.bone_display.unwrap_or_default();
        let enabled = self.bone_display_editable();
        let p = u.palette();
        u.fitted_label([x, y, w, 20.], "Оформление выбранной кости", 12., p.text);
        y += 26.;
        for (joint, color, kind, shape, filled, follow) in [
            (
                false,
                d.color,
                DropdownKind::BoneShape,
                d.shape.label(),
                d.filled,
                d.follow_view,
            ),
            (
                true,
                d.joint_color,
                DropdownKind::JointShape,
                d.joint_shape.label(),
                d.joint_filled,
                d.joint_follow_view,
            ),
        ] {
            u.fitted_label(
                [x, y, w, 18.],
                if joint {
                    "Цвет сустава"
                } else {
                    "Цвет кости"
                },
                11.,
                p.muted,
            );
            y += 20.;
            let color_action =
                Action::BoneDisplay(if joint { B::JointColor } else { B::BodyColor });
            let rgb = color.map(|c| (c * 255.).round() as u8);
            let hex = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
            let r = [x, y, w, 30.];
            if enabled {
                u.color_field(r, &hex, color_action, color);
            } else {
                u.disabled_button(r, &hex);
            }
            y += 38.;
            u.fitted_label([x, y + 7., 50., 18.], "Форма", 11., p.muted);
            let r = [x + 56., y, w - 56., 30.];
            if enabled {
                self.draw_dropdown_button(u, r, kind, shape);
            } else {
                u.disabled_button(r, shape);
            }
            y += 38.;
            for (label, action, checked) in [
                (
                    if joint {
                        "Заливка сустава"
                    } else {
                        "Заливка кости"
                    },
                    if joint { B::JointFilled } else { B::BodyFilled },
                    filled,
                ),
                (
                    if joint {
                        "Сустав следует виду"
                    } else {
                        "Кость следует виду"
                    },
                    if joint {
                        B::JointFollowView
                    } else {
                        B::BodyFollowView
                    },
                    follow,
                ),
            ] {
                let r = [x, y, w, 28.];
                if enabled {
                    u.checkbox(r, label, Action::BoneDisplay(action), checked);
                } else {
                    u.disabled_button(r, label);
                }
                y += 34.;
            }
            y += 12.;
        }
        if enabled {
            u.quiet_button(
                [x, y, w, 30.],
                "Сбросить оформление",
                Action::BoneDisplay(B::Reset),
                false,
            );
        } else {
            u.disabled_button([x, y, w, 30.], "Сбросить оформление");
        }
    }
}
