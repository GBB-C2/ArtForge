//! One reversible tool operation, including nested transform drags and fields.
use super::component_selection::SelectionSnapshot;
use super::*;
use crate::ui::{AuthorAction as A, ModelingTool as M, ShapeAction as S};

pub(super) struct Session {
    tool: M,
    selection: SelectionSnapshot,
    original_selection: SelectionSnapshot,
    waiting: bool,
    selecting: bool,
    selected: Option<u64>,
    editing: Option<u64>,
    command: Option<Action>,
    pub refreshing: bool,
    pub error: Option<String>,
}
impl Session {
    fn interactive(&self) -> bool {
        matches!(
            self.tool,
            M::Select(_) | M::Move | M::Rotate | M::Scale | M::Inspect
        )
    }
}
impl App {
    pub(super) fn begin_modeling_session(&mut self, tool: M) {
        self.cancel_modeling_live();
        self.editor.begin_tool_edit(tool.title());
        self.authoring.workflow.session = Some(Session {
            tool,
            selection: SelectionSnapshot::capture(&self.authoring),
            original_selection: SelectionSnapshot::capture(&self.authoring),
            waiting: false,
            selecting: false,
            selected: self.editor.selected,
            editing: self.authoring.editing,
            command: if tool == M::Align {
                Some(Action::Shape(match self.mesh_tools.alignment_mode {
                    1 => S::Straighten,
                    2 => S::Space,
                    _ => S::Flatten(1),
                }))
            } else {
                tool.command()
            },
            refreshing: false,
            error: None,
        });
        if matches!(
            tool,
            M::Select(_) | M::Move | M::Rotate | M::Scale | M::Inspect
        ) {
            if let Some(action) = tool.command() {
                self.action(action);
            }
        } else {
            self.refresh_modeling_session();
        }
    }
    pub(super) fn start_pending_modeling_session(&mut self) {
        let Some(session) = self.authoring.workflow.session.as_ref() else {
            return;
        };
        if session.waiting
            && !session.refreshing
            && !session.selecting
            && !self.modifiers.shift_key()
            && self.drag.is_none()
            && self.input.is_none()
            && self.modeling_tool_ready(session.tool)
        {
            let selection = SelectionSnapshot::capture(&self.authoring);
            self.authoring.workflow.session.as_mut().unwrap().selection = selection;
            self.refresh_modeling_session();
        }
    }
    pub(super) fn begin_session_selection(&mut self) {
        let Some(session) = self.authoring.workflow.session.as_mut() else {
            return;
        };
        if session.interactive() || session.selecting {
            return;
        }
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.editor.reset_tool_edit();
        self.editor.selected = session.selected;
        self.authoring.editing = session.editing;
        let selection = session.selection.clone();
        session.selecting = true;
        selection.restore(&mut self.authoring);
        self.invalidate_geometry();
    }
    pub(super) fn finish_session_selection(&mut self) {
        if !self
            .authoring
            .workflow
            .session
            .as_ref()
            .is_some_and(|s| s.selecting)
        {
            return;
        }
        let selection = SelectionSnapshot::capture(&self.authoring);
        let session = self.authoring.workflow.session.as_mut().unwrap();
        session.selection = selection;
        session.selecting = false;
        self.refresh_modeling_session();
    }
    pub(super) fn refresh_modeling_session(&mut self) {
        let Some(mut session) = self.authoring.workflow.session.take() else {
            return;
        };
        if session.refreshing || session.interactive() {
            self.authoring.workflow.session = Some(session);
            return;
        }
        // Preview dispatch must not cancel the numeric drag that requested it.
        let slider = if matches!(self.drag, Some(Drag::Slider(_))) {
            self.drag.take()
        } else {
            None
        };
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.editor.reset_tool_edit();
        self.editor.selected = session.selected;
        self.authoring.editing = session.editing;
        session.selection.restore(&mut self.authoring);
        session.error = None;
        if session.tool == M::Align {
            session.command = Some(Action::Shape(match self.mesh_tools.alignment_mode {
                1 => S::Straighten,
                2 => S::Space,
                _ => match session.command {
                    Some(Action::Shape(S::Flatten(axis))) => S::Flatten(axis),
                    _ => S::Flatten(1),
                },
            }));
        }
        session.waiting = !self.modeling_tool_ready(session.tool);
        if session.waiting {
            session.error = Some("Выберите нужные компоненты".into());
        }
        let action = session
            .error
            .is_none()
            .then(|| session.command.clone())
            .flatten();
        session.refreshing = true;
        self.authoring.workflow.session = Some(session);
        if let Some(action) = action {
            self.action(action);
        }
        if let Some(session) = &mut self.authoring.workflow.session {
            session.refreshing = false;
        }
        if let Some(slider) = slider {
            self.drag = Some(slider);
        }
        self.invalidate_geometry();
        self.update_dirty();
        self.redraw();
    }
    pub(super) fn cancel_modeling_session(&mut self) {
        if let Some(session) = self.authoring.workflow.session.take() {
            if let Some(job) = self.job.take() {
                job.cancel.store(true, Ordering::Relaxed);
            }
            self.cancel_drag();
            self.end_input();
            self.editor.cancel_tool_edit();
            self.editor.selected = session.selected;
            self.authoring.editing = session.editing;
            session.original_selection.restore(&mut self.authoring);
            self.authoring.diagnostics = None;
            self.invalidate_geometry();
            self.update_dirty();
            self.status = "Изменения инструмента отменены".into();
        }
    }
    pub(super) fn apply_modeling_session(&mut self) {
        self.finish_session_selection();
        if self.input.is_some() && !self.resolve_input() {
            return;
        }
        if self.job.is_some() {
            self.status = "Дождитесь завершения предпросмотра".into();
            return;
        }
        if self.modeling_live.is_some() {
            self.accept_modeling_live();
            return;
        }
        if self.authoring.knife.is_some() {
            self.action(Action::Author(A::KnifeAccept));
            return;
        }
        let Some(session) = self.authoring.workflow.session.as_ref() else {
            if self.authoring.workflow.pending_preview.is_none() {
                self.dismiss_modeling_tool_panel();
            }
            return;
        };
        if let Some(error) = &session.error {
            self.status = error.clone();
            return;
        }
        if self.drag.is_some() {
            self.cancel_drag();
        }
        let session = self.authoring.workflow.session.take().unwrap();
        match self.editor.commit_tool_edit() {
            Ok(()) => {
                self.status = format!("{} · применено", session.tool.title());
                self.dismiss_modeling_tool_panel();
            }
            Err(e) => {
                self.editor.selected = session.selected;
                self.authoring.editing = session.editing;
                session.selection.restore(&mut self.authoring);
                self.status = e.to_string();
                self.dismiss_modeling_tool_panel();
            }
        }
        self.invalidate_geometry();
        self.update_dirty();
        self.redraw();
    }
    pub(super) fn route_modeling_session(&mut self, action: &Action) -> bool {
        if Self::session_field(action)
            || matches!(action, Action::Slide(..) | Action::ResetNumber(_))
        {
            self.finish_session_selection();
        }
        let Some(session) = self.authoring.workflow.session.as_ref() else {
            return false;
        };
        if session.refreshing {
            return false;
        }
        match action {
            Action::ModelingTool(tool) if *tool == session.tool => false,
            Action::ModelingTool(_) if matches!(session.tool, M::Select(_)) => {
                self.apply_modeling_session();
                false
            }
            Action::Author(A::SelectMode(_)) if session.interactive() => false,
            Action::Author(A::ToolApply) => {
                self.apply_modeling_session();
                true
            }
            Action::Cancel | Action::Undo | Action::Redo | Action::Author(A::ToolCancel) => {
                self.cancel_modeling_session();
                self.dismiss_modeling_tool_panel();
                self.redraw();
                true
            }
            a if !session.interactive()
                && (session.command.as_ref() == Some(a)
                    || (session.tool == M::Align
                        && matches!(
                            a,
                            Action::Shape(S::Flatten(_) | S::Straighten | S::Space)
                        ))) =>
            {
                self.authoring.workflow.session.as_mut().unwrap().command = Some(a.clone());
                self.refresh_modeling_session();
                true
            }
            a if Self::preview_navigation_action(a) => false,
            Action::Slide(field, ..) | Action::ResetNumber(field) if Self::session_field(field) => {
                false
            }
            a if Self::session_field(a) => false,
            Action::Author(
                A::ToolPanelDrag
                | A::ToolPanelToggle
                | A::ToolPanelScroll
                | A::LiveBackdrop
                | A::SelectTool(_)
                | A::SelectAll
                | A::Clear
                | A::Grow
                | A::Shrink
                | A::Connected
                | A::Invert
                | A::EdgeLoop
                | A::EdgeRing
                | A::Xray
                | A::PivotOrigin
                | A::Constrain(_)
                | A::Proportional
                | A::SnapToggle
                | A::SnapMode(_)
                | A::SnapGridStep(_)
                | A::RepairMode(_)
                | A::SelectIssue
                | A::RepairIssue
                | A::Inspect
                | A::SelectDefects,
            ) => false,
            Action::Shape(S::Axis(_) | S::Pin | S::AlignmentMode(_)) => false,
            Action::Move | Action::Rotate | Action::Scale if session.interactive() => false,
            Action::DropdownOpen(_)
            | Action::DropdownChoose(_)
            | Action::DropdownDismiss
            | Action::ToggleSection(super::modeling_palette::TOOLS_SECTION)
            | Action::ScrollPanel(0)
            | Action::ToggleRightPanel => false,
            _ => {
                self.cancel_modeling_session();
                self.dismiss_modeling_tool_panel();
                false
            }
        }
    }
    fn session_field(action: &Action) -> bool {
        matches!(
            action,
            Action::Normal(ui::NormalAction::Field(_))
                | Action::Author(
                    A::DensityMode(_) | A::SimplifyRatio | A::MirrorAxis(_) | A::MirrorField(_) | A::MirrorMerge | A::SubdivideCount
                        | A::Coordinate(_)
                        | A::RotateDegrees(_)
                        | A::ScaleFactor(_)
                        | A::ProportionalRadius
                        | A::SnapStep
                        | A::WeldDistance
                )
        )
    }
    pub(super) fn draw_modeling_confirmation(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        let half = (w - 6.) * 0.5;
        let error = self
            .authoring
            .workflow
            .session
            .as_ref()
            .and_then(|s| s.error.as_ref());
        if error.is_some()
            || self.job.is_some()
            || self.authoring.workflow.pending_preview.is_some()
        {
            u.disabled_button([x, y, half, 30.], "Применить");
        } else {
            u.button(
                [x, y, half, 30.],
                "Применить",
                Action::Author(A::ToolApply),
                true,
            );
        }
        u.quiet_button(
            [x + half + 6., y, half, 30.],
            "Отмена",
            Action::Author(A::ToolCancel),
            false,
        );
        if let Some(error) = error {
            u.fitted_label([x, y - 26., w, 22.], error, 10., u.palette().muted);
        }
    }
}
