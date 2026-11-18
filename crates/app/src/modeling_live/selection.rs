//! Temporary source selection keeps the current preview and its undo owner.
use super::*;
#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;

impl App {
    pub(in crate::app) fn modeling_modifiers_changed(&mut self, modifiers: ModifiersState) {
        let shift_was_down = self.modifiers.shift_key();
        self.modifiers = modifiers;
        if !modifiers.control_key() {
            self.authoring.workflow.ctrl_applied = false;
        }
        // A panel or navigation drag can begin while source selection is visible.
        // Releasing Shift must resume the preview even while that drag owns input.
        if !modifiers.shift_key() && shift_was_down {
            self.finish_live_selection();
        }
        if self.drag.is_none() && self.input.is_none() && !self.has_modal() {
            if modifiers.shift_key() && !shift_was_down {
                self.begin_live_selection();
            } else if !modifiers.shift_key() && shift_was_down {
                self.start_pending_modeling_preview();
            }
        }
        self.redraw();
    }

    fn begin_live_selection(&mut self) {
        self.begin_session_selection();
        let Some(op) = self.modeling_live.as_ref().filter(|op| !op.selecting) else {
            return;
        };
        // A zero translation restores the frozen mesh through the editor's
        // existing instance-safe preview path, without closing the transaction.
        if let Err(e) = self.editor.preview_mesh(
            op.node,
            &op.original,
            MeshEdit::Translate {
                vertices: BTreeSet::from([0]),
                delta: Vec3::ZERO,
            },
        ) {
            self.status = e.to_string();
            return;
        }
        let op = self.modeling_live.as_mut().unwrap();
        op.selection.restore(&mut self.authoring);
        op.selecting = true;
        self.invalidate_geometry();
        self.status =
            "Shift + щелчок — добавить или убрать компонент · отпустите Shift для предпросмотра"
                .into();
    }

    pub(in crate::app) fn finish_live_selection(&mut self) {
        self.finish_session_selection();
        let Some(mut op) = self.modeling_live.take() else {
            return;
        };
        if !op.selecting {
            self.modeling_live = Some(op);
            return;
        }
        op.selecting = false;
        op.selection = SelectionSnapshot::capture(&self.authoring);
        op.vertices = self.edit_vertices();
        op.faces = if op.kind == Kind::Bridge && op.selection.mode != S::Face {
            BTreeSet::new()
        } else {
            self.selected_face_region()
        };
        if op.kind == Kind::Bridge {
            op.selection.edges = self.workflow_bridge_edges();
        }
        if matches!(op.kind, Kind::Extrude | Kind::VertexExtrude) {
            op.kind = if op.faces.is_empty() && op.selection.mode != S::Face {
                Kind::VertexExtrude
            } else {
                Kind::Extrude
            };
        }
        if matches!(op.kind, Kind::Inset | Kind::VertexInset) {
            op.kind = if op.faces.is_empty() && op.selection.mode != S::Face {
                Kind::VertexInset
            } else {
                Kind::Inset
            };
        }
        let model = self
            .editor
            .project
            .global_transforms()
            .ok()
            .and_then(|g| g.get(&op.node).copied())
            .unwrap_or(Mat4::IDENTITY);
        if let Some(center) = self.edit_center() {
            op.anchor = model.transform_point3(center);
        }
        let normal_faces = if op.kind == Kind::VertexExtrude {
            let graph = self.mesh_cache(&op.original);
            let points: BTreeSet<_> = op
                .vertices
                .iter()
                .map(|&v| graph.graph.canonical[v])
                .collect();
            (0..op.original.face_count())
                .filter(|&f| {
                    op.original
                        .face_vertices(f)
                        .unwrap()
                        .iter()
                        .any(|&v| points.contains(&graph.graph.canonical[v as usize]))
                })
                .collect()
        } else {
            op.faces.clone()
        };
        let mut normal = Vec3::ZERO;
        for f in normal_faces {
            let face = op.original.face_vertices(f).unwrap();
            let p = |i: usize| Vec3::from_array(op.original.vertices[face[i] as usize].position);
            for i in 1..face.len().saturating_sub(1) {
                normal += (p(i) - p(0)).cross(p(i + 1) - p(0));
            }
        }
        op.direction = model.transform_vector3(normal.try_normalize().unwrap_or(Vec3::Y));
        op.model=model;
        self.refresh_grip_geometry(&mut op);
        let result = if op.kind == Kind::Cut && op.selection.edges.len() != 1 {
            Err("Выберите одно ребро для разреза".to_owned())
        } else {
            if op.kind == Kind::Cut {
                let &(a, b) = op.selection.edges.first().unwrap();
                op.direction = model.transform_vector3(
                    Vec3::from_array(op.original.vertices[b].position)
                        - Vec3::from_array(op.original.vertices[a].position),
                );
            }
            op.preview(&mut self.editor, op.values)
                .map_err(|e| e.to_string())
        };
        op.has_preview = result.is_ok();
        op.error = result.err();
        self.status = op.error.clone().unwrap_or_else(||"Предпросмотр обновлён · Shift — выбрать · Ctrl / Enter — применить · Esc — отменить".into());
        self.modeling_live = Some(op);
        self.sync_live_selection();
        self.invalidate_geometry();
        self.update_dirty();
        self.redraw();
    }

    pub(in crate::app) fn modeling_modifier_pointer(&mut self, button: MouseButton) -> bool {
        if button != MouseButton::Left
            || self.authoring.editing.is_none()
            || self.input.is_some()
            || self.has_modal()
            || self
                .authoring
                .knife
                .as_ref()
                .is_some_and(|k| k.press.is_some())
            || self.menu.is_some()
            || self.dropdown.is_some()
            || self.color_picker.is_some()
            || self.scene_write_pending()
            || self.job.is_some()
        {
            return false;
        }
        let over_model = self.canvas_input_at(self.mouse)
            || (ui::contains(self.modeling_area(), self.mouse)
                && matches!(
                    self.ui.hit(self.mouse),
                    Some(Action::Author(A::LiveDrag(_) | A::ArraySpacing | A::SectionSelect(_) | A::SectionGrip(_)))
                ));
        if !over_model {
            return false;
        }
        if self.modifiers.control_key() && self.authoring.workflow.ctrl_applied {
            return true;
        }
        if self.authoring.modeling_tool.is_none()
            || matches!(
                self.authoring.modeling_tool,
                Some(ui::ModelingTool::Select(_) | ui::ModelingTool::Inspect)
            )
        {
            return false;
        }
        if self.modifiers.control_key() {
            let handled = self.apply_active_modeling_tool();
            if handled && self.authoring.modeling_tool.is_none() {
                self.authoring.workflow.ctrl_applied = true;
            }
            return handled;
        }
        if !self.modifiers.shift_key() {
            return false;
        }
        self.begin_live_selection();
        self.select_mesh_component_at_pointer();
        self.redraw();
        true
    }

    pub(in crate::app) fn modeling_control_key(&mut self, key: &Key, repeat: bool) -> bool {
        if *key != Key::Named(NamedKey::Control)
            || self.authoring.editing.is_none()
            || repeat
            || self.drag.is_some()
            || self
                .authoring
                .knife
                .as_ref()
                .is_some_and(|k| k.press.is_some())
            || self.input.is_some()
            || self.has_modal()
            || self.menu.is_some()
            || self.dropdown.is_some()
            || self.color_picker.is_some()
            || self.scene_write_pending()
            || self.job.is_some()
        {
            return false;
        }
        let handled = self.apply_active_modeling_tool();
        if handled && self.authoring.modeling_tool.is_none() {
            self.authoring.workflow.ctrl_applied = true;
        }
        handled
    }

    fn apply_active_modeling_tool(&mut self) -> bool {
        if self.authoring.workflow.session.is_some()
            && !matches!(self.authoring.modeling_tool,Some(ui::ModelingTool::Select(_) | ui::ModelingTool::Inspect)) {
            self.apply_modeling_session(); return true;
        }
        if self.modeling_live.is_some() {
            self.accept_modeling_live();
            return true;
        }
        if self.authoring.knife.is_some() {
            self.action(Action::Author(A::KnifeAccept));
            return true;
        }
        let Some(tool) = self.authoring.modeling_tool else {
            return false;
        };
        if matches!(
            tool,
            ui::ModelingTool::Select(_) | ui::ModelingTool::Inspect
        ) {
            return false;
        }
        if matches!(
            tool,
            ui::ModelingTool::Move | ui::ModelingTool::Rotate | ui::ModelingTool::Scale
        ) {
            self.dismiss_modeling_tool_panel();
            self.redraw();
            return true;
        }
        if let Some(action) = tool.command() {
            self.action(action);
            if self.modeling_live.is_some() {
                self.accept_modeling_live();
            }
            return true;
        }
        if tool == ui::ModelingTool::Align && self.mesh_tools.alignment_mode != 0 {
            self.action(Action::Shape(if self.mesh_tools.alignment_mode == 1 {
                ui::ShapeAction::Straighten
            } else {
                ui::ShapeAction::Space
            }));
            return true;
        }
        false
    }
}
