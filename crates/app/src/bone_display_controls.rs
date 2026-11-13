//! Bone appearance is display-only and shares the editor's transactions.
use super::*;
use crate::ui::{BoneDisplayAction as B, DropdownKind};
use poseforge_core::{BoneDisplay, BoneDisplayPreset};

pub(super) const HEIGHT: f32 = 556.;

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

    pub(super) fn bone_display_ids(&self) -> BTreeSet<u64> {
        if self.bone_display_node().is_none() {
            return BTreeSet::new();
        }
        self.object_selection()
            .into_iter()
            .filter(|id| self.editor.project.node(*id).is_some_and(|n| n.is_joint))
            .collect()
    }

    pub(super) fn bone_display_editable(&self) -> bool {
        self.bone_display_node().is_some()
            && self
                .bone_display_ids()
                .iter()
                .all(|id| self.node_selectable(*id))
            && self.job.is_none()
            && self.native_picker.is_none()
            && !self.scene_write_pending()
            && !self.editor.has_staged()
    }

    pub(super) fn bone_display_height(&self) -> f32 {
        if self.bone_display_node().is_none() {
            0.
        } else if self.bone_display_collapsed {
            34.
        } else {
            HEIGHT
        }
    }

    pub(super) fn bone_display_shape_index(&self, joint: bool) -> Option<usize> {
        let primary = self.bone_display_node()?.bone_display.unwrap_or_default();
        if self.bone_display_ids().iter().any(|id| {
            let other = self
                .editor
                .project
                .node(*id)
                .unwrap()
                .bone_display
                .unwrap_or_default();
            if joint {
                other.joint_shape != primary.joint_shape
            } else {
                other.shape != primary.shape
            }
        }) {
            return None;
        }
        if joint {
            poseforge_core::JointShape::ALL
                .iter()
                .position(|s| *s == primary.joint_shape)
        } else {
            poseforge_core::BoneShape::ALL
                .iter()
                .position(|s| *s == primary.shape)
        }
    }

    pub(super) fn bone_display_preset_index(&self) -> Option<usize> {
        let ids = self.bone_display_ids();
        if ids.is_empty() {
            return None;
        }
        BoneDisplayPreset::ALL.iter().position(|preset| {
            ids.iter().all(|id| {
                self.editor
                    .project
                    .node(*id)
                    .unwrap()
                    .bone_display
                    .unwrap_or_default()
                    == preset.display()
            })
        })
    }

    pub(super) fn edit_bone_displays(
        &mut self,
        label: &str,
        edit: impl Fn(&mut BoneDisplay),
    ) -> Result<(), String> {
        if !self.bone_display_editable() {
            return Err("Кости недоступны для изменения".into());
        }
        let changes: Vec<_> = self
            .bone_display_ids()
            .into_iter()
            .filter_map(|id| {
                let before = self
                    .editor
                    .project
                    .node(id)?
                    .bone_display
                    .unwrap_or_default();
                let mut after = before;
                edit(&mut after);
                (after != before).then_some((id, after))
            })
            .collect();
        if changes.is_empty() {
            return Ok(());
        }
        self.editor
            .transact(label, |p| {
                for (id, display) in changes {
                    p.node_mut(id).unwrap().bone_display = Some(display);
                }
            })
            .map_err(|e| e.to_string())?;
        self.update_dirty();
        Ok(())
    }

    pub(super) fn bone_size_confirm(
        &mut self,
        action: &Action,
        text: &str,
    ) -> Result<bool, String> {
        let Action::BoneDisplay(B::Size(joint)) = *action else {
            return Ok(false);
        };
        let value = surfaces::number(text, 0.1, 5.)?;
        self.edit_bone_displays("Размер оформления костей", |d| {
            if joint {
                d.joint_size = value;
            } else {
                d.size = value;
            }
        })?;
        Ok(true)
    }

    pub(super) fn bone_display_action(&mut self, action: B) {
        if action == B::Collapse {
            self.bone_display_collapsed = !self.bone_display_collapsed;
            self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
            self.redraw();
            return;
        }
        if !self.bone_display_editable() {
            return;
        }
        let primary = self
            .bone_display_node()
            .unwrap()
            .bone_display
            .unwrap_or_default();
        if matches!(action, B::BodyColor | B::JointColor) {
            self.open_color_picker(Action::BoneDisplay(action));
            return;
        }
        if let B::Size(joint) = action {
            self.input(
                Action::BoneDisplay(action),
                if joint {
                    primary.joint_size
                } else {
                    primary.size
                }
                .to_string(),
            );
            return;
        }
        let defaults = BoneDisplay::default();
        let result = if action == B::Reset {
            let ids: Vec<_> = self
                .bone_display_ids()
                .into_iter()
                .filter(|id| {
                    self.editor
                        .project
                        .node(*id)
                        .unwrap()
                        .bone_display
                        .is_some()
                })
                .collect();
            if ids.is_empty() {
                return;
            }
            self.editor
                .transact("Сбросить оформление костей", |p| {
                    for id in ids {
                        p.node_mut(id).unwrap().bone_display = None;
                    }
                })
                .map_err(|e| e.to_string())
        } else {
            self.edit_bone_displays("Оформление костей", |d| match action {
                B::Preset(preset) => *d = preset.display(),
                B::BodyShape(shape) => d.shape = shape,
                B::JointShape(shape) => d.joint_shape = shape,
                B::ResetBodyShape => d.shape = defaults.shape,
                B::ResetJointShape => d.joint_shape = defaults.joint_shape,
                B::ResetBodyColor => d.color = defaults.color,
                B::ResetJointColor => d.joint_color = defaults.joint_color,
                B::BodyFilled => d.filled = !primary.filled,
                B::JointFilled => d.joint_filled = !primary.joint_filled,
                B::BodyFollowView => d.follow_view = !primary.follow_view,
                B::JointFollowView => d.joint_follow_view = !primary.joint_follow_view,
                _ => {}
            })
        };
        if let Err(error) = result {
            self.status = error;
        }
        self.update_dirty();
        self.redraw();
    }

    pub(super) fn draw_bone_display(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let Some(node) = self.bone_display_node() else {
            return;
        };
        let d = node.bone_display.unwrap_or_default();
        let ids = self.bone_display_ids();
        let enabled = self.bone_display_editable();
        let p = u.palette();
        let title = if ids.len() > 1 {
            format!("Оформление костей · {}", ids.len())
        } else {
            "Оформление выбранной кости".into()
        };
        u.quiet_icon_button(
            [x, y, w, 28.],
            &title,
            Action::BoneDisplay(B::Collapse),
            false,
            if self.bone_display_collapsed {
                Icon::ChevronRight
            } else {
                Icon::ChevronDown
            },
        );
        if self.bone_display_collapsed {
            return;
        }
        y += 34.;
        let preset = self
            .bone_display_preset_index()
            .map(|i| BoneDisplayPreset::ALL[i].label())
            .unwrap_or("Своё оформление");
        if enabled {
            self.draw_dropdown_button(u, [x, y, w, 30.], DropdownKind::BonePreset, preset);
        } else {
            u.disabled_button([x, y, w, 30.], preset);
        }
        y += 38.;
        u.fitted_label(
            [x, y, w, 18.],
            "Shift / Ctrl + клик — несколько костей",
            10.,
            p.muted,
        );
        y += 22.;
        for joint in [false, true] {
            let color = if joint { d.joint_color } else { d.color };
            let mixed = ids.iter().any(|id| {
                let other = self
                    .editor
                    .project
                    .node(*id)
                    .unwrap()
                    .bone_display
                    .unwrap_or_default();
                (if joint {
                    other.joint_color
                } else {
                    other.color
                }) != color
            });
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
            let rgb = color.map(|c| (c * 255.).round() as u8);
            let hex = if mixed {
                "Разные цвета".into()
            } else {
                format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
            };
            if enabled {
                u.color_field(
                    [x, y, w, 30.],
                    &hex,
                    Action::BoneDisplay(if joint { B::JointColor } else { B::BodyColor }),
                    color,
                );
            } else {
                u.disabled_button([x, y, w, 30.], &hex);
            }
            y += 38.;
            let shape = if joint {
                d.joint_shape.label()
            } else {
                d.shape.label()
            };
            let mixed_shape = ids.iter().any(|id| {
                let other = self
                    .editor
                    .project
                    .node(*id)
                    .unwrap()
                    .bone_display
                    .unwrap_or_default();
                if joint {
                    other.joint_shape != d.joint_shape
                } else {
                    other.shape != d.shape
                }
            });
            let shape = if mixed_shape {
                "Разные формы"
            } else {
                shape
            };
            u.fitted_label([x, y + 7., 50., 18.], "Форма", 11., p.muted);
            if enabled {
                self.draw_dropdown_button(
                    u,
                    [x + 56., y, w - 84., 30.],
                    if joint {
                        DropdownKind::JointShape
                    } else {
                        DropdownKind::BoneShape
                    },
                    shape,
                );
                u.quiet_icon_button(
                    [x + w - 24., y, 24., 30.],
                    "",
                    Action::BoneDisplay(if joint {
                        B::ResetJointShape
                    } else {
                        B::ResetBodyShape
                    }),
                    false,
                    Icon::Reset,
                );
            } else {
                u.disabled_button([x + 56., y, w - 56., 30.], shape);
            }
            y += 38.;
            let size = if joint { d.joint_size } else { d.size };
            let mixed_size = ids.iter().any(|id| {
                let other = self
                    .editor
                    .project
                    .node(*id)
                    .unwrap()
                    .bone_display
                    .unwrap_or_default();
                (if joint { other.joint_size } else { other.size }) != size
            });
            let label = if mixed_size {
                "Размер · разные"
            } else {
                "Размер"
            };
            let action = Action::BoneDisplay(B::Size(joint));
            let range = SliderRange::new(0.1, 5., 0.01);
            if enabled {
                u.number_field([x, y, w, 30.], label, size as f64, action, range);
            } else {
                u.disabled_number_field([x, y, w, 30.], label, size as f64, action, range);
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
                    if joint { d.joint_filled } else { d.filled },
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
                    if joint {
                        d.joint_follow_view
                    } else {
                        d.follow_view
                    },
                ),
            ] {
                if enabled {
                    u.checkbox([x, y, w, 28.], label, Action::BoneDisplay(action), checked);
                } else {
                    u.disabled_button([x, y, w, 28.], label);
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
