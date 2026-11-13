//! Stable-ID rig references shared by dropdowns and viewport/outliner eyedroppers.
use super::*;
use crate::ui::RigReference;

pub(super) fn roots(project: &Project) -> Vec<u64> {
    let nodes: BTreeMap<_, _> = project.nodes.iter().map(|n| (n.id, n)).collect();
    let mut roots = BTreeSet::new();
    for joint in project.nodes.iter().filter(|n| n.is_joint) {
        let mut node = joint;
        let mut highest_joint = joint;
        let mut container = None;
        let mut seen = BTreeSet::new();
        while seen.insert(node.id) {
            let Some(parent) = node.parent.and_then(|id| nodes.get(&id).copied()) else { break; };
            if parent.is_joint {
                highest_joint = parent;
                container = None;
            } else if parent.mesh.is_some() || parent.camera.is_some() || parent.curve.is_some() {
                break;
            } else if container.is_none() {
                container = Some(parent);
            }
            node = parent;
        }
        roots.insert(container.unwrap_or(highest_joint).id);
    }
    roots.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::Node;

    fn app() -> App {
        let mut a = App::new(None);
        let mut project = Project::demo();
        let root = Node::new(50_000, "Другой скелет");
        let mut joint = Node::new(50_001, "Joint");
        joint.is_joint = true;
        joint.parent = Some(root.id);
        let mut mesh = Node::new(50_002, "Меш");
        mesh.mesh = Some(project.meshes[0].id);
        let mut duplicate = mesh.clone();
        duplicate.id = 50_003;
        project.nodes.extend([root, joint, mesh, duplicate]);
        a.editor = Editor::new(project);
        a.preferences = Preferences::default();
        a.width = 1440.; a.height = 960.;
        a.editor.mode = EditorMode::Pose;
        a.action(Action::Select(9));
        a.action(Action::RigStage(1));
        a.frame_all();
        a
    }
    fn show(a: &mut App, reference: RigReference) {
        let max = a.inspector_max_scroll();
        for offset in 0..=((max/32.).ceil() as usize) {
            a.inspector_scroll = (offset as f32*32.).min(max);
            a.ui = a.build_ui();
            if a.ui.action_point(&Action::DropdownOpen(reference.dropdown())).is_some() { return; }
        }
        panic!("reference selector must be reachable");
    }
    #[test]
    fn roots_cross_helper_nodes_and_references_exclude_locked_or_hidden_objects() {
        let mut project = Project::default();
        let root = Node::new(1, "Skeleton");
        let mut a = Node::new(2, "Root joint"); a.is_joint = true; a.parent = Some(1);
        let mut helper = Node::new(3, "Helper"); helper.parent = Some(2);
        let mut b = Node::new(4, "Child joint"); b.is_joint = true; b.parent = Some(3);
        let mut mesh = Node::new(5, "Mesh"); mesh.mesh = Some(6);
        project.nodes = vec![root,a,helper,b,mesh];
        assert_eq!(roots(&project), vec![1]);
        assert_eq!(choices(&project,RigReference::Mesh),vec![5]);
        project.node_mut(1).unwrap().locked = true;
        project.node_mut(5).unwrap().visible = false;
        assert!(choices(&project,RigReference::Skeleton).is_empty());
        assert!(choices(&project,RigReference::Mesh).is_empty());
    }
    #[test]
    fn dropdowns_and_eyedroppers_are_reachable_in_both_themes_and_dpi() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {
            for scale in [1.,1.5,2.] {
                let mut a = app(); a.scale = scale; a.preferences.theme = theme;
                let original = a.editor.project.clone();
                for reference in [RigReference::Skeleton,RigReference::Mesh] {
                    show(&mut a,reference);
                    let pick = a.ui.action_point(&Action::RigReferencePick(reference)).unwrap();
                    let open = a.ui.action_point(&Action::DropdownOpen(reference.dropdown())).unwrap();
                    assert!(pick[0] > open[0]);
                    a.action(Action::DropdownOpen(reference.dropdown()));
                    assert!(a.dropdown_is_open(reference.dropdown()));
                    a.dropdown_key(&Key::Named(NamedKey::End),false,false,false);
                    if reference == RigReference::Mesh {
                        a.ui = a.build_ui();
                        let last = choices(&a.editor.project,reference).len()-1;
                        a.mouse = a.ui.action_point(&Action::DropdownChoose(last)).unwrap();
                        a.mouse_down(MouseButton::Left);
                        a.mouse_up(MouseButton::Left);
                    } else {
                        a.dropdown_key(&Key::Named(NamedKey::Enter),false,false,false);
                    }
                    assert_eq!(a.rig_reference_value(reference), Some(if reference == RigReference::Skeleton {50_000} else {50_003}));
                }
                assert_eq!(a.authoring.skeleton,Some(50_000));
                assert_eq!(a.editor.project,original);
                assert_eq!(a.editor.undo_len(),0);
            }
        }
    }
    #[test]
    fn stale_dropdown_cannot_retarget_by_a_shifted_row() {
        let mut a = app(); show(&mut a,RigReference::Mesh);
        let selected = a.editor.selected;
        let target = a.rig_target();
        a.open_dropdown(ui::DropdownKind::RigMesh);
        let last = choices(&a.editor.project,RigReference::Mesh).len()-1;
        a.editor.project.node_mut(50_003).unwrap().locked = true;
        a.choose_dropdown(last);
        assert!(a.dropdown.is_none());
        assert_eq!(a.editor.selected,selected);
        assert_eq!(a.rig_target(),target);
    }
    #[test]
    fn eyedroppers_share_ids_without_binding_and_cancel_without_click_through() {
        let mut a = app(); let original = a.editor.project.clone();
        a.begin_rig_reference_pick(RigReference::Skeleton);
        a.action(Action::Select(50_001));
        assert_eq!(a.authoring.skeleton,Some(50_000));
        a.begin_rig_reference_pick(RigReference::Mesh);
        a.action(Action::Select(50_001));
        assert_eq!(a.authoring.reference_pick,Some(RigReference::Mesh));
        a.action(Action::Select(50_002));
        assert_eq!(a.rig_target(),Some(50_002));
        assert_eq!(a.authoring.skeleton,Some(50_000));
        assert_eq!(a.authoring.reference_pick,None);
        a.begin_rig_reference_pick(RigReference::Skeleton);
        a.action(Action::Cancel);
        assert!(a.authoring.reference_pick.is_none());
        a.begin_rig_reference_pick(RigReference::Mesh);
        a.ui.hits.clear();
        a.ui.hits.push(ui::Hit { rect:[0.,0.,30.,30.],action:Action::Delete });
        a.mouse = [12.,12.];
        assert!(a.rig_reference_pointer(MouseButton::Left));
        assert!(a.authoring.reference_pick.is_none());
        assert_eq!(a.editor.selected,Some(50_002));
        assert_eq!(a.editor.project,original);
        assert_eq!(a.editor.undo_len(),0);
    }
    #[test]
    fn viewport_joint_pick_resolves_its_skeleton() {
        let mut a = app();
        a.set_rig_reference(RigReference::Skeleton,50_000);
        a.begin_rig_reference_pick(RigReference::Skeleton);
        a.ui = a.build_ui();
        let position = a.editor.evaluated().unwrap().global[&9].w_axis.truncate();
        a.mouse = a.project_point(position).unwrap();
        assert!(a.canvas_input_at(a.mouse));
        assert!(a.rig_reference_pointer(MouseButton::Left));
        assert_eq!(a.authoring.skeleton,Some(1));
        assert!(a.authoring.reference_pick.is_none());
        assert_eq!(a.editor.undo_len(),0);
    }
    #[test]
    fn locked_targets_and_playback_cannot_start_or_commit_a_pick() {
        let mut a = app();
        a.editor.project.node_mut(50_002).unwrap().locked = true;
        let target = a.rig_target();
        a.begin_rig_reference_pick(RigReference::Mesh);
        a.action(Action::Select(50_002));
        assert_eq!(a.rig_target(),target);
        assert!(a.authoring.reference_pick.is_some());
        a.action(Action::Cancel);
        a.editor.playing = true;
        a.begin_rig_reference_pick(RigReference::Skeleton);
        assert!(a.authoring.reference_pick.is_none());
        a.set_rig_reference(RigReference::Mesh,50_003);
        assert_eq!(a.rig_target(),target);
    }
}

pub(super) fn choices(project: &Project, reference: RigReference) -> Vec<u64> {
    let selectable = project.selectable_node_ids();
    match reference {
        RigReference::Skeleton => roots(project).into_iter().filter(|id| selectable.contains(id)).collect(),
        RigReference::Mesh => project.nodes.iter().filter(|n| n.mesh.is_some() && n.curve.is_none()
            && !n.is_joint && selectable.contains(&n.id)).map(|n| n.id).collect(),
    }
}

impl App {
    pub(super) fn rig_reference_editable(&self) -> bool {
        self.job.is_none() && self.native_picker.is_none() && !self.scene_write_pending()
            && !self.editor.has_staged() && !self.editor.playing
            && self.authoring.rig_stage == 1
    }
    pub(super) fn rig_reference_value(&self, reference: RigReference) -> Option<u64> {
        match reference { RigReference::Skeleton => self.authoring.skeleton, RigReference::Mesh => self.rig_target() }
    }
    pub(super) fn set_rig_reference(&mut self, reference: RigReference, id: u64) {
        if !self.rig_reference_editable() || self.has_modal() || !choices(&self.editor.project, reference).contains(&id) {
            self.status = "Выберите доступный скелет или обычный незаблокированный меш.".into();
            return;
        }
        self.authoring.reference_pick = None;
        match reference {
            RigReference::Skeleton => {
                self.authoring.skeleton = Some(id);
                self.active_rig_root = Some(id);
                self.sync_rig_pose();
            }
            RigReference::Mesh => {
                let skeleton = self.authoring.skeleton;
                self.authoring.bind_target = Some(id);
                self.action(Action::Select(id));
                // Choosing a skinned target must not replace the explicitly chosen source rig.
                self.authoring.skeleton = skeleton;
                self.active_rig_root = skeleton.or(self.active_rig_root);
                self.authoring.rig_stage = 1;
                self.set_properties_tab(0);
                self.focus_rig_settings();
            }
        }
        self.status = "Выбор обновлён. «Привязать · автовеса» применит привязку.".into();
        self.redraw();
    }
    pub(super) fn begin_rig_reference_pick(&mut self, reference: RigReference) {
        if !self.rig_reference_editable() || self.has_modal() || self.input.is_some() { return; }
        if self.authoring.reference_pick == Some(reference) {
            self.authoring.reference_pick = None;
            self.status = "Выбор пипеткой отменён.".into();
        } else {
            self.cancel_drag(); self.cancel_modeling_live();
            self.light_aim = None; self.focus_pick = None; self.dropdown = None;
            self.authoring.reference_pick = Some(reference);
            self.preferences.skeleton = true;
            self.status = match reference {
                RigReference::Skeleton => "Пипетка скелета: нажмите кость, скелет в сцене или его привязанный меш · Esc — отмена",
                RigReference::Mesh => "Пипетка меша: нажмите меш в просмотре или в сцене · Esc — отмена",
            }.into();
        }
        self.redraw();
    }
    fn rig_reference_hit(&self, reference: RigReference, id: u64) -> Option<u64> {
        let options = choices(&self.editor.project, reference);
        if options.contains(&id) { return Some(id); }
        if reference == RigReference::Mesh || !self.node_selectable(id) { return None; }
        let node = self.editor.project.node(id)?;
        if node.is_joint {
            return options.into_iter().find(|root| self.is_descendant(id, *root));
        }
        let skin = node.skin.as_ref()?;
        options.into_iter().find(|root| !skin.joints.is_empty() && skin.joints.iter().all(|id| self.is_descendant(*id, *root)))
    }
    pub(super) fn pick_rig_reference_node(&mut self, id: u64) {
        let Some(reference) = self.authoring.reference_pick else { return; };
        if let Some(id) = self.rig_reference_hit(reference, id) { self.set_rig_reference(reference, id); }
        else { self.status = "Объект не подходит. Выберите скелет/кость или меш нужного типа · Esc — отмена".into(); }
        self.redraw();
    }
    pub(super) fn rig_reference_pointer(&mut self, button: MouseButton) -> bool {
        let Some(reference) = self.authoring.reference_pick else { return false; };
        if !self.rig_reference_editable() || self.has_modal() { self.authoring.reference_pick = None; return false; }
        if button == MouseButton::Right {
            self.authoring.reference_pick = None; self.status = "Выбор пипеткой отменён.".into(); self.redraw(); return true;
        }
        if button != MouseButton::Left { return false; }
        if let Some(action) = self.ui.hit(self.mouse) {
            match action {
                Action::Select(id) => self.pick_rig_reference_node(id),
                Action::RigReferencePick(kind) => self.begin_rig_reference_pick(kind),
                _ => { self.authoring.reference_pick = None; self.status = "Выбор пипеткой отменён.".into(); }
            }
            self.redraw(); return true;
        }
        if self.canvas_input_at(self.mouse) {
            if let Ok(evaluated) = self.editor.evaluated() {
                let project = self.picking_project();
                let bone = (reference == RigReference::Skeleton).then(|| crate::bones::BoneOverlay::new(
                    &project, &evaluated, &self.view(), self.viewport()).hit(self.mouse)).flatten();
                let id = bone.or_else(|| crate::picking::pick_mesh(&project, &evaluated,
                    self.view().view_projection(), self.viewport(), self.mouse));
                drop(project);
                if let Some(id) = id { self.pick_rig_reference_node(id); }
                else { self.status = "Здесь нет подходящего объекта. Пипетка остаётся активной · Esc — отмена".into(); }
            }
        } else { self.authoring.reference_pick = None; }
        self.redraw(); true
    }
}
