//! Primary controls next to the model. The Geometry inspector owns the full settings.
use super::*;
use crate::ui::{AuthorAction as A, ModelingTool as M};

impl App {
    pub(super) fn draw_alignment_mode(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        self.draw_dropdown_button(
            u,
            [x, y, w, 30.],
            ui::DropdownKind::Alignment,
            ["В плоскость", "В цепочку", "Равномерный шаг"]
                [self.mesh_tools.alignment_mode as usize],
        );
    }

    // Use one layout for measurement and drawing so the footer always stays visible,
    // including when the editable section or the selection changes during a preview.
    pub(super) fn quick_modeling_controls(
        &self,
        mut ui: Option<&mut Ui>,
        x: f32,
        y: f32,
        w: f32,
    ) -> f32 {
        let Some(tool) = self.authoring.modeling_tool else {
            return 0.;
        };
        let mut height = 0.;
        let mut row = |pitch: f32, draw: &dyn Fn(&mut Ui, f32)| {
            if let Some(u) = ui.as_deref_mut() {
                draw(u, y + height);
            }
            height += pitch;
        };
        if self.modeling_live.is_some() {
            for &i in self.quick_live_fields() {
                row(48., &|u, y| self.draw_live_field(u, [x, y, w, 44.], i));
            }
        } else {
            match tool {
                M::Knife => row(44., &|u, y| self.draw_workflow_field(u, x, y, w, 0, 38.)),
                M::LoopCut if self.authoring.knife.is_some() => {
                    row(34., &|u, y| self.draw_section_modes(u, x, y, w));
                    let edit = self
                        .authoring
                        .knife
                        .as_ref()
                        .is_some_and(|k| k.section_edit);
                    if !edit || self.section_value(10).is_some() {
                        row(44., &|u, y| {
                            self.draw_workflow_field(u, x, y, w, if edit { 10 } else { 9 }, 38.)
                        });
                    }
                }
                M::Array => row(44., &|u, y| self.draw_workflow_field(u, x, y, w, 2, 38.)),
                M::Bridge | M::BridgeFaces => row(48., &|u, y| {
                    self.draw_pending_modeling_field(u, x, y, w, A::BridgeSegments)
                }),
                M::Inset | M::Bevel if self.authoring.selection_mode == ui::MeshSelection::Face => {
                    row(48., &|u, y| {
                        self.draw_pending_modeling_field(u, x, y, w, A::InsetAmount)
                    });
                }
                M::Move | M::Scale | M::Rotate => {
                    row(34., &|u, y| self.draw_modeling_axes(u, x, y, w));
                    if tool != M::Move || self.authoring.constraint.is_some() {
                        row(38., &|u, y| {
                            self.draw_transform_value(
                                u,
                                [x, y, w, 32.],
                                tool,
                                self.authoring.constraint.unwrap_or(self.axis),
                            )
                        });
                    }
                }
                M::Mirror => row(36., &|u, y| self.draw_mirror_axes(u, x, y, w)),
                M::Subdivide => row(48., &|u, y| self.draw_density_value(u, x, y, w)),
                M::Relax => row(48., &|u, y| self.draw_relax_strength(u, x, y, w)),
                M::Sphere | M::Circle | M::Inflate | M::Twist => {
                    row(48., &|u, y| self.draw_deform_amount(u, x, y, w, tool));
                }
                M::Align => row(36., &|u, y| self.draw_alignment_mode(u, x, y, w)),
                M::Select(_) => row(32., &|u, y| {
                    u.checkbox(
                        [x, y, w, 26.],
                        "Сквозное выделение · X-Ray",
                        Action::Author(A::Xray),
                        self.authoring.xray,
                    )
                }),
                M::Inspect => row(36., &|u, y| {
                    u.quiet_button(
                        [x, y, w, 30.],
                        "Выделить ошибки категории",
                        Action::Author(A::SelectIssue),
                        false,
                    )
                }),
                _ => {}
            }
        }
        if let Some(message) = self.modeling_feedback() {
            row(24., &|u, y| {
                u.fitted_label([x, y, w, 20.], message, 10., u.palette().muted)
            });
        }
        height
    }
}
