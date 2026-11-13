//! Transient mode for editing a follower's local offset with the ordinary move gizmo.
use super::{curve_binding_controls::BindingTarget, clip_scrolled, App, Tool};
use crate::ui::Ui;
use glam::Vec3;
use poseforge_core::EditorMode;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct EditTarget { target: BindingTarget, curve: u64 }

impl App {
    pub(super) fn path_offset_editing_for(&self, target: BindingTarget) -> bool {
        self.path_edit.is_some_and(|edit| edit.target == target
            && self.path_target() == Some(target)
            && self.path_binding().is_some_and(|b| b.curve == edit.curve))
    }
    pub(super) fn editing_path_curve(&self) -> Option<u64> {
        let edit = self.path_edit?;
        self.path_offset_editing_for(edit.target).then_some(edit.curve)
    }
    pub(super) fn prune_path_edit(&mut self) {
        if self.path_edit.is_some() && (self.editing_path_curve().is_none() || !self.path_editable() || self.editor.playing || self.editor.mode != EditorMode::Scene) {
            self.path_edit = None;
        }
    }
    pub(super) fn path_offset(&self) -> Option<Vec3> {
        self.path_binding()?;
        match self.path_target()? {
            BindingTarget::Node(id) => Some(self.editor.project.node(id)?.transform.translation),
            BindingTarget::Light(index) => Some(self.editor.project.lights.get(index)?.position),
        }
    }
    pub(super) fn toggle_path_edit(&mut self) {
        self.cancel_drag();
        if self.path_edit.take().is_some() {
            self.status = "Edit выключен · перемещение снова вдоль кривой.".into(); self.redraw(); return;
        }
        let (Some(target), Some(binding)) = (self.path_target(), self.path_binding()) else { return; };
        if !self.path_editable() { return; }
        self.cancel_modeling_live(); self.authoring.editing = None;
        self.close_collision_selection(); self.authoring.reference_pick = None;
        self.path_pick = None; self.light_aim = None; self.focus_pick = None; self.dropdown = None;
        self.editor.playing = false; self.editor.mode = EditorMode::Scene; self.tool = Tool::Move;
        self.path_edit = Some(EditTarget { target, curve: binding.curve });
        self.status = "Edit · свободное смещение гизмо X/Y/Z · сброс возвращает на кривую · Esc — выход.".into();
        self.redraw();
    }
    pub(super) fn reset_path_offset(&mut self) {
        let Some(target) = self.path_target() else { return; };
        if !self.path_editable() || self.path_offset().is_none_or(|p| p == Vec3::ZERO) { return; }
        self.cancel_drag(); self.editor.playing = false; self.editor.mode = EditorMode::Scene;
        let result = self.editor.transact("Сбросить смещение от кривой", |project| match target {
            BindingTarget::Node(id) => project.node_mut(id).unwrap().transform.translation = Vec3::ZERO,
            BindingTarget::Light(index) => project.lights[index].position = Vec3::ZERO,
        });
        if self.result(result).is_some() {
            self.status = "Смещение сброшено · положение на пути, привязка, поворот и масштаб сохранены.".into();
        }
        self.redraw();
    }
    pub(super) fn draw_path_offset_guide(&self, u: &mut Ui) {
        if self.has_modal() { return; }
        let Some(curve) = self.editing_path_curve() else { return; };
        if !self.editor.project.node_visible(curve, false) { return; }
        let Some(binding) = self.path_binding() else { return; };
        let Ok(scene) = self.editor.evaluated() else { return; };
        let (position, negative_z) = match self.path_target() {
            Some(BindingTarget::Node(id)) => {
                let Some(node) = self.editor.project.node(id) else { return; };
                let Some(matrix) = scene.global.get(&id) else { return; };
                (matrix.w_axis.truncate(), node.camera.is_some())
            }
            Some(BindingTarget::Light(index)) => {
                let Some(light) = scene.lights.get(index) else { return; }; (light.position, true)
            }
            None => return,
        };
        let Ok(frame) = self.editor.project.curve_binding_frame(binding, &scene.global, self.editor.time, negative_z) else { return; };
        let anchor = frame.transform_point3(Vec3::ZERO);
        let p = u.palette(); let start = (u.rects.len(), u.texts.len(), u.hits.len());
        if let Some((a,b,_)) = crate::bones::project_segment(self.view().view_projection(), self.viewport(), anchor, position) {
            u.smooth_segment(a.to_array(), b.to_array(), 1.5, p.accent);
        }
        if let Some([x,y]) = self.project_point(anchor).filter(|p| self.canvas_input_at(*p)) {
            u.rounded_rect([x-5.,y-5.,10.,10.],p.accent,5.);
            u.rounded_rect([x-3.,y-3.,6.,6.],p.panel,3.);
        }
        clip_scrolled(u, start, self.viewport(), 0.);
    }
}
