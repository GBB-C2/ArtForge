//! Force fields control the existing transient solvers for their enabled receiver types.
use super::App;
use poseforge_core::{ForceFieldSettings, PhysicsKind};
use std::time::Instant;

impl App {
    pub(super) fn force_preview_editable(&self, id: u64) -> bool {
        self.node_selectable(id) && self.job.is_none() && self.native_picker.is_none()
            && !self.scene_write_pending() && !self.editor.has_staged()
    }
    pub(super) fn force_preview_running(&self, field: ForceFieldSettings) -> bool {
        (field.targets[0] && self.fluid_domains.any_playing())
            || (field.targets[1] && self.studio.physics_running)
            || (field.targets[2] && self.destruction.preview.running())
            || (field.fx && self.fx_preview.running && self.fx_field_receiver())
    }
    fn fx_field_receiver(&self)->bool {
        let visible=self.editor.project.visible_node_ids(false);
        self.editor.project.nodes.iter().any(|n|visible.contains(&n.id) && self.in_isolation(n.id)
            && n.fx.is_some_and(|f|f.enabled && f.use_fields && f.field_influence>0. && f.density>0. && f.speed>0.))
    }
    pub(super) fn force_preview_action(&mut self, reset: bool) {
        let Some(node) = self.selected_helper().filter(|n| self.force_preview_editable(n.id)) else { return; };
        let Some(field) = node.force_field else { return; };
        if self.has_modal() { return; }
        self.cancel_drag(); self.editor.playing = false;
        if reset {
            if field.targets[0] {
                for node in &self.editor.project.nodes {
                    if node.fluid.is_some() { self.fluid_domains.reset_playback(node.id); }
                }
            }
            if field.targets[1] { self.studio.physics_running = false; self.studio.physics.reset(); }
            if field.targets[2] { self.destruction.preview.reset(); }
            if field.fx {self.reset_fx_preview();}
            self.status = "В начало · выбранные типы симуляции сброшены, ключи и параметры поля сохранены.".into();
            self.redraw(); return;
        }
        if self.force_preview_running(field) {
            if field.targets[0] { self.fluid_domains.pause_playback(); }
            if field.targets[1] { self.studio.physics_running = false; }
            if field.targets[2] { self.destruction.preview.pause(); }
            if field.fx {self.pause_fx_preview();}
            self.status = "Предпросмотр выбранных типов физики на паузе.".into(); self.redraw(); return;
        }
        if !field.enabled {
            self.status = "Сначала включите силовое поле.".into(); self.redraw(); return;
        }
        let project = &self.editor.project;
        let visible = project.visible_node_ids(false);
        let fluids: Vec<_> = project.nodes.iter().filter(|n| field.targets[0] && visible.contains(&n.id)
            && !project.node_locked(n.id) && n.fluid.is_some_and(|f| f.enabled && f.visible)).map(|n|n.id).collect();
        let cloth = field.targets[1] && project.nodes.iter().any(|n| visible.contains(&n.id) && !project.node_locked(n.id)
            && n.physics.is_some_and(|p|p.visible && p.kind != PhysicsKind::Collider));
        let fragments: Vec<_> = project.nodes.iter().filter(|n| field.targets[2] && !project.node_locked(n.id)
            && n.destruction.is_some_and(|d| d.visible && d.result.is_some_and(|id|visible.contains(&id) && !project.node_locked(id))))
            .map(|n|n.id).collect();
        let fx=field.fx && self.fx_field_receiver();
        if fluids.is_empty() && !cloth && fragments.is_empty() && !fx {
            self.status = "Нет готовых получателей: добавьте жидкость, ткань, осколки или FX и включите нужные типы у поля.".into();
            self.redraw(); return;
        }
        // Validate the whole shard batch before starting any receiver. No partial launch on error.
        if !fragments.is_empty() {
            if let Err(error) = self.start_destruction_previews(&fragments) {
                self.status = error; self.redraw(); return;
            }
        }
        for id in &fluids { if !self.fluid_domains.playing(*id) { self.fluid_domains.toggle_playback(*id); } }
        if cloth { self.studio.physics_running = true; }
        if fx {self.start_fx_preview();}
        self.last_tick = Instant::now();
        self.status = format!("Без ключей · жидкостей: {} · ткань: {} · осколков: {} · FX: {}",
            fluids.len(), if cloth {"включена"}else{"—"}, fragments.len(),if fx {"включены"}else{"—"});
        self.redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::HelperAction as H;
    use glam::Vec3;
    use poseforge_core::{box_mesh, Channel, Clip, DestructionSettings, Editor, EditorMode,
        ForceFieldKind, Interpolation, Keyframe, Node, Property, Project};

    fn app() -> App {
        let mut project = Project::default();
        let mut field = Node::new(1, "Wind"); field.empty = true;
        field.force_field = Some(ForceFieldSettings { kind: ForceFieldKind::Wind, radius: 10., ..Default::default() });
        let mut liquid = Node::new(2, "Liquid"); liquid.mesh = Some(3); liquid.fluid = Some(Default::default());
        let mut cloth = Node::new(4, "Cloth"); cloth.mesh = Some(5); cloth.physics = Some(Default::default());
        project.nodes = vec![field, liquid, cloth];
        project.meshes = vec![box_mesh(3, "Liquid", Vec3::splat(0.3), None), box_mesh(5, "Cloth", Vec3::splat(0.2), None)];
        for source in [10,20] {
            let mut n = Node::new(source, "Source"); n.mesh = Some(source+1); n.visible = false; n.render_visible = false;
            n.destruction = Some(DestructionSettings { result: Some(source+2), mesh_collision: false,
                values: [2.,1.,0.,0.,9.81,-10.,0.,24.], ..Default::default() });
            let mut root = Node::new(source+2, "Fragments"); root.transform.translation = Vec3::new(source as f32/10.-1.5,2.,0.);
            let mut shard = Node::new(source+3, "Shard"); shard.parent = Some(source+2); shard.mesh = Some(source+4);
            project.nodes.extend([n,root,shard]);
            project.meshes.extend([box_mesh(source+1,"Source",Vec3::splat(0.2),None),box_mesh(source+4,"Shard",Vec3::splat(0.2),None)]);
        }
        let mut clip = Clip::new("Existing animation",4.);
        clip.channels.push(Channel { node: 4, property: Property::Translation, interpolation: Interpolation::Linear,
            keys: vec![Keyframe::new(0.,vec![0.,0.,0.]),Keyframe::new(4.,vec![1.,0.,0.])] });
        project.clips.push(clip); project.validate().unwrap();
        let mut a = App::new(None); a.editor = Editor::new(project); a.reset_workflow_selection();
        a.editor.mode = EditorMode::Scene; a.editor.active_clip = Some(0); a.editor.time = 1.25;
        a.editor.selected = Some(1); a.surface.tab = 0; a
    }
    #[test]
    fn field_starts_two_shard_groups_and_all_receivers_without_document_edits() {
        let mut a = app(); let before = a.editor.project.clone(); let undo = a.editor.undo_len();
        let original = a.editor.evaluated().unwrap().global; a.editor.playing = true;
        a.helper_action(H::Play);
        assert!(!a.editor.playing); assert!(a.fluid_domains.playing(2)); assert!(a.studio.physics_running);
        assert!(a.destruction.preview.running_for(10) && a.destruction.preview.running_for(20));
        a.tick_destruction_preview(1./30.); a.helper_action(H::Play);
        assert!(!a.fluid_domains.any_playing() && !a.studio.physics_running && !a.destruction.preview.running());
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while a.destruction.preview.is_pending() && Instant::now() < deadline {
            a.tick_destruction_preview(0.); std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(!a.destruction.preview.is_pending());
        assert!(a.destruction.preview.time(10).unwrap() > 0.);
        assert_eq!(a.destruction.preview.time(10),a.destruction.preview.time(20));
        let paused = a.evaluated_viewport().unwrap().global;
        assert!(paused[&13].w_axis.y < original[&13].w_axis.y && paused[&23].w_axis.y < original[&23].w_axis.y);
        a.tick_destruction_preview(1.); assert_eq!(a.evaluated_viewport().unwrap().global,paused);
        a.helper_action(H::Play); assert!(a.destruction.preview.running_for(20));
        a.helper_action(H::Reset);
        assert!(!a.fluid_domains.any_playing() && !a.studio.physics_running && !a.destruction.preview.running());
        assert!(a.destruction.preview.time(10).is_none() && a.destruction.preview.time(20).is_none());
        assert_eq!(a.evaluated_viewport().unwrap().global,original);
        assert_eq!(a.editor.project,before); assert_eq!(a.editor.undo_len(),undo);
        assert_eq!(a.editor.selected,Some(1)); assert_eq!(a.editor.active_clip,Some(0)); assert_eq!(a.editor.time,1.25);
        assert!(a.editor.mode == EditorMode::Scene);
    }
    #[test]
    fn unchecked_types_keep_running_and_disabled_or_locked_fields_cannot_start() {
        let mut a = app(); a.helper_action(H::Play);
        a.editor.project.node_mut(1).unwrap().force_field.as_mut().unwrap().targets = [true,false,false];
        a.helper_action(H::Play); assert!(!a.fluid_domains.any_playing());
        assert!(a.studio.physics_running && a.destruction.preview.running_for(20));
        a.helper_action(H::Reset); assert!(a.studio.physics_running && a.destruction.preview.running_for(10));
        a.editor.project.node_mut(1).unwrap().force_field.as_mut().unwrap().enabled = false;
        a.helper_action(H::Play); assert!(!a.fluid_domains.any_playing());
        a.editor.project.node_mut(1).unwrap().force_field.as_mut().unwrap().enabled = true;
        a.editor.project.node_mut(1).unwrap().locked = true;
        let before = a.editor.project.clone(); let undo = a.editor.undo_len();
        a.helper_action(H::Play); a.helper_action(H::Reset);
        assert!(!a.fluid_domains.any_playing()); assert_eq!(a.editor.project,before); assert_eq!(a.editor.undo_len(),undo);
    }
    #[test]
    fn invalid_shard_batch_does_not_partially_start_other_solvers() {
        let mut a = app(); a.editor.project.node_mut(23).unwrap().transform.scale = Vec3::ZERO;
        let before = a.editor.project.clone(); let undo = a.editor.undo_len();
        a.helper_action(H::Play);
        assert!(!a.fluid_domains.any_playing() && !a.studio.physics_running && !a.destruction.preview.running());
        assert!(a.status.contains("масштаб") || a.status.contains("вырожденную трансформацию"), "{}", a.status);
        assert_eq!(a.editor.project,before); assert_eq!(a.editor.undo_len(),undo);
    }
    #[test]
    fn field_runs_fx_without_keys_and_respects_receiver_and_field_toggles() {
        let mut a=app();let id=a.editor.create_fx(poseforge_core::FxKind::Smoke).unwrap();
        a.editor.project.node_mut(1).unwrap().force_field.as_mut().unwrap().targets=[false;3];
        a.editor.selected=Some(1);let before=a.editor.project.clone();let undo=a.editor.undo_len();let time=a.editor.time;
        a.helper_action(H::Play);assert!(a.fx_preview.running);a.tick_fx(0.05);assert!(a.fx_view_time()>time);
        a.helper_action(H::Play);assert!(!a.fx_preview.running);let paused=a.fx_view_time();a.tick_fx(0.05);assert_eq!(a.fx_view_time(),paused);
        a.helper_action(H::Reset);assert_eq!(a.fx_view_time(),0.);assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);assert_eq!(a.editor.time,time);
        a.helper_action(H::Target(3));assert!(!a.editor.project.node(1).unwrap().force_field.unwrap().fx);
        a.helper_action(H::Play);assert!(!a.fx_preview.running);a.editor.undo();
        a.editor.project.node_mut(id).unwrap().fx.as_mut().unwrap().use_fields=false;
        a.helper_action(H::Play);assert!(!a.fx_preview.running);
    }
}
