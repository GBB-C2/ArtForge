//! Skeleton authoring, binding and pose tools belong to the selected object.
use super::*;
use crate::ui::{AuthorAction as A, RigAction as R};
pub(super) const RIG_OBJECT_START: f32 = 526.;
impl App {
    pub(super) fn is_skeleton_object(&self, id: u64) -> bool {
        self.editor.project.node(id).is_some_and(|n| {
            !n.empty && !n.is_joint && n.mesh.is_none() && n.camera.is_none()
                && self.editor.project.nodes.iter().any(|joint| joint.is_joint && joint.parent == Some(id))
        })
    }
    pub(super) fn selected_joint_properties(&self) -> bool {
        self.editor.selected.and_then(|id| self.editor.project.node(id)).is_some_and(|n| n.is_joint)
    }
    pub(super) fn sync_rig_object_selection(&mut self) {
        let Some(id) = self.editor.selected else { return; };
        let container = self.is_skeleton_object(id);
        if !container && !self.selected_joint_properties() { return; }
        let root = self.saved_root_for(id).unwrap_or_else(|| {
            if container { id } else { self.skeleton_object_selection(id).0 }
        });
        self.authoring.skeleton = Some(root);
        self.active_rig_root = Some(root);
    }
    pub(super) fn rig_context(&self) -> bool {
        if self.active_light_index().is_some() || self.selected_camera_id().is_some() {
            return false;
        }
        self.editor.selected.is_some_and(|id| {
            self.editor.project.node(id).is_some_and(|n| {
                n.is_joint
                    || (!n.empty && n.mesh.is_none()
                        && (self.active_rig_root == Some(id)
                            || self.authoring.skeleton == Some(id)
                            || self
                                .editor
                                .project
                                .nodes
                                .iter()
                                .any(|joint| joint.is_joint && joint.parent == Some(id))))
            })
        }) || self.rig_binding_context()
    }
    pub(super) fn rig_binding_context(&self) -> bool {
        self.authoring.rig_stage == 1
            && matches!(self.editor.mode, EditorMode::Pose | EditorMode::Animation)
            && self.editor.selected.is_some_and(|id| {
                self.authoring.bind_target == Some(id)
                    && self.editor.project.node(id).is_some_and(|n| n.mesh.is_some() && n.curve.is_none())
            })
            && self.authoring.skeleton.is_some_and(|id| self.editor.project.node(id).is_some())
    }
    pub(super) fn rig_object_bottom(&self) -> f32 {
        RIG_OBJECT_START + match self.authoring.rig_stage {
            0 => 200. + if self.bone_display_node().is_some() { bone_display_controls::HEIGHT } else { 0. },
            1 => 512., _ => 534.
        }
    }
    pub(super) fn rig_stage_action(&mut self, stage: u8) {
        if stage > 2 || self.job.is_some() || self.scene_write_pending() {
            return;
        }
        if self.editor.has_staged() {
            self.status = "Сначала поставьте ключ или отмените временную позу.".into();
            return;
        }
        self.cancel_drag();
        self.cancel_modeling_live();
        if self.uv.open {
            self.uv_action(ui::UvAction::Close);
        }
        self.editor.playing = false;
        self.authoring.editing = None;
        self.authoring.vertices.clear();
        self.authoring.edges.clear();
        self.authoring.faces.clear();
        self.authoring.bind_target = self.rig_target();
        self.weights.target = None;
        self.scene_tools.selected.clear();
        if let Some(id) = self.editor.selected {
            self.sync_active_character(id);
        }
        self.sync_rig_object_selection();
        let roots = self.skeleton_roots();
        self.authoring.skeleton = self
            .active_rig_root
            .filter(|r| self.editor.project.node(*r).is_some())
            .or(self.authoring.skeleton.filter(|r| roots.contains(r)))
            .or_else(|| roots.first().copied());
        self.authoring.rig_stage = stage;
        if stage != 2 || self.editor.mode != EditorMode::Animation {
            self.editor.mode = EditorMode::Pose;
        }
        if stage != 1 {
            let selected_joint = self
                .editor
                .selected
                .is_some_and(|id| self.editor.project.node(id).is_some_and(|n| n.is_joint));
            if !selected_joint {
                if let Some(root) = self.authoring.skeleton {
                    self.editor.selected = self
                        .editor
                        .project
                        .nodes
                        .iter()
                        .find(|n| {
                            n.is_joint
                                && self.is_descendant(n.id, root)
                                && self.node_selectable(n.id)
                        })
                        .map(|n| n.id);
                }
            }
            self.ik = false;
            self.tool = if stage == 0 { Tool::Move } else { Tool::Rotate };
        } else if let Some(id) = self.rig_target().filter(|id| self.node_selectable(*id)) {
            self.editor.selected = Some(id);
            self.authoring.bind_target = Some(id);
        }
        if stage == 1 {
            self.tool = Tool::Orbit;
        }
        if let Some(id) = self.editor.selected {
            self.reveal_scene_node(id);
        }
        self.set_properties_tab(0);
        self.focus_rig_settings();
        self.preferences.right_panel = true;
        self.preferences.skeleton = true;
        self.sync_rig_pose();
        self.status = match stage {
            0 => "Скелет и IK · построение, положение костей и проверка цепей",
            1 => "Привязка · выберите меш и скелет, затем рассчитайте веса",
            _ => "Поза · выбор части тела, FK/IK и ключи",
        }
        .into();
    }
    pub(super) fn focus_rig_settings(&mut self) {
        self.preferences.collapsed_sections.remove(&1901);
        self.inspector_scroll = self.property_section_offset(1901).unwrap_or(0.).min(self.inspector_max_scroll());
    }
    pub(super) fn draw_rig_modes(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        let width = (w - 12.) / 4.;
        for (i, (action, icon)) in [
            (Action::Mode(0), Icon::Scene), (Action::RigStage(0), Icon::Bone),
            (Action::RigStage(1), Icon::RigBind), (Action::RigStage(2), Icon::Pose),
        ].into_iter().enumerate() {
            let r = [x + i as f32 * (width + 4.), y, width, 32.];
            if self.job.is_some() || self.native_picker.is_some() || self.editor.has_staged() {
                u.disabled_icon_button(r, "", icon);
            } else {
                let active = if i == 0 { self.editor.mode == EditorMode::Scene }
                    else { self.editor.mode != EditorMode::Scene && self.authoring.rig_stage == i as u8 - 1 };
                u.quiet_icon_button(r, "", action, active, icon);
            }
        }
    }
    pub(super) fn draw_rig_workflow(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let p = u.palette();
        let library_rect = [x, self.rig_object_bottom() - 44., w, 30.];
        if self.job.is_some() || self.native_picker.is_some() {
            u.disabled_icon_button(library_rect, "Библиотека поз", Icon::Library);
        } else {
            u.quiet_icon_button(library_rect, "Библиотека поз", Action::Poses, false, Icon::Library);
        }
        if self.authoring.rig_stage == 2 {
            self.draw_rig_pose_panel(u, x, y, w);
            return;
        }
        if self.authoring.rig_stage == 0 {
            let label = "Добавить дочернюю кость…";
            let enabled = self.editor.selected.is_some_and(|id| {
                self.node_selectable(id)
                    && self.editor.project.node(id).is_some_and(|n| n.is_joint)
            });
            if enabled {
                u.quiet_icon_button(
                    [x, y, w, 30.], label, Action::Author(A::ExtendJoint), false, Icon::Bone,
                );
            } else {
                u.disabled_icon_button([x, y, w, 30.], label, Icon::Bone);
            }
            y += 40.;
            let half = (w - 6.) / 2.;
            u.quiet_button(
                [x, y, half, 30.],
                "FK",
                Action::Rig(R::Kinematics(false)),
                !self.ik,
            );
            if self
                .editor
                .selected
                .is_some_and(|id| self.pose_chain(id).is_ok())
            {
                u.quiet_button(
                    [x + half + 6., y, half, 30.],
                    "IK",
                    Action::Rig(R::Kinematics(true)),
                    self.ik,
                );
            } else {
                u.disabled_button([x + half + 6., y, half, 30.], "IK");
            }
            y += 42.;
            u.quiet_button(
                [x, y, w, 30.],
                "Соответствие костей…",
                Action::CharacterPanel,
                false,
            );
            y += 42.;
            u.fitted_label([x, y, w, 20.], "IK: выберите кисть или стопу", 11., p.muted);
            self.draw_bone_display(u, x, y + 28., w);
        } else {
            u.fitted_label([x, y, w, 18.], "Скелет", 12., p.text);
            y += 26.;
            self.draw_rig_reference(u, [x, y, w, 32.], ui::RigReference::Skeleton);
            y += 42.;
            u.fitted_label(
                [x, y, w, 18.],
                "Меш для привязки",
                11.,
                p.muted,
            );
            y += 26.;
            let target = self
                .rig_target()
                .and_then(|id| self.editor.project.node(id));
            self.draw_rig_reference(u, [x, y, w, 32.], ui::RigReference::Mesh);
            y += 44.;
            for (label, a) in [
                ("Привязать · автовеса", A::Bind),
                ("Снять привязку", A::Unbind),
            ] {
                let enabled = target.is_some_and(|n| {
                    self.node_selectable(n.id)
                        && if a == A::Bind {
                            n.skin.is_none() && self.authoring.skeleton.is_some()
                        } else {
                            n.skin.is_some()
                        }
                });
                if enabled {
                    u.quiet_button([x, y, w, 30.], label, Action::Author(a), false);
                } else {
                    u.disabled_button([x, y, w, 30.], label);
                }
                y += 40.;
            }
            y += 14.;
            self.draw_weight_controls(u, x, y, w);
            y += 178.;
            u.fitted_label(
                [x, y, w, 20.],
                "Автовеса: до 4 влияний на вершину",
                11.,
                p.muted,
            );
            y += 26.;
            u.fitted_label([x, y, w, 20.], "Расчёт выполняется в фоне", 11., p.muted);
        }
    }

    fn draw_rig_reference(&self, u: &mut Ui, rect: [f32; 4], reference: ui::RigReference) {
        let [x, y, w, h] = rect;
        let name = self.rig_reference_value(reference).and_then(|id| self.editor.project.node(id))
            .map_or(match reference { ui::RigReference::Skeleton => "Выбрать скелет", ui::RigReference::Mesh => "Выбрать меш" }, |n| n.name.as_str());
        if self.rig_reference_editable() {
            self.draw_dropdown_button(u, [x, y, w - 38., h], reference.dropdown(), name);
            u.quiet_icon_button([x+w-32., y, 32., h], "", Action::RigReferencePick(reference),
                self.authoring.reference_pick == Some(reference), Icon::Eyedropper);
        } else {
            u.disabled_button([x, y, w-38., h], name);
            u.disabled_icon_button([x+w-32., y, 32., h], "", Icon::Eyedropper);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut a = App::new(None);
        a.editor = Editor::new(Project::demo());
        a.preferences = Preferences::default();
        a.width = 1280.;
        a.height = 900.;
        a.editor.mode = EditorMode::Pose;
        a.action(Action::Select(9));
        a
    }
    fn click_stage(a: &mut App, stage: u8) {
        a.action(Action::SurfaceTab(0));
        a.focus_rig_settings();
        a.ui = a.build_ui();
        let h =
            a.ui.hits
                .iter()
                .find(|h| h.action == Action::RigStage(stage) && h.rect[1] < a.properties_content_rect()[1])
                .unwrap();
        a.mouse = [h.rect[0] + h.rect[2] / 2., h.rect[1] + h.rect[3] / 2.];
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
    }
    #[test]
    fn bone_toolbar_exposes_three_working_stages_in_both_themes_and_dpi() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for scale in [1., 2.] {
                let mut a = app();
                a.scale = scale;
                a.preferences.theme = theme;
                let before = a.editor.project.clone();
                for stage in [0, 1, 2] {
                    click_stage(&mut a, stage);
                    assert_eq!(a.authoring.rig_stage, stage);
                    assert_eq!(a.surface.tab, 0);
                    let ui = a.build_ui();
                    assert!(!ui
                        .hits
                        .iter()
                        .any(|h| matches!(h.action, Action::Author(A::SelectMode(_)))));
                    let required = match stage {
                        0 => Action::Author(A::ExtendJoint),
                        1 => Action::DropdownOpen(ui::DropdownKind::RigMesh),
                        _ => Action::Rig(R::KeyPart),
                    };
                    let mut reachable = ui.hits.iter().any(|h| h.action == required);
                    let max = a.inspector_max_scroll();
                    for offset in 0..=((max / 80.).ceil() as usize) {
                        a.inspector_scroll = (offset as f32 * 80.).min(max);
                        reachable |= a.build_ui().hits.iter().any(|h| h.action == required);
                    }
                    assert!(reachable);
                    assert!(!a.build_ui().hits.iter().any(|h| h.action == Action::SurfaceTab(8)));
                    assert_eq!(a.editor.project, before);
                    assert_eq!(a.editor.undo_len(), 0);
                }
            }
        }
    }
    #[test]
    fn rig_stage_buttons_stay_once_in_the_fixed_component_row_when_scrolled() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
            let mut a=app();a.preferences.theme=theme;a.scale=scale;
            for stage in [0,1,2] {
                a.action(Action::RigStage(stage));a.inspector_scroll=a.inspector_max_scroll();
                let u=a.build_ui();let toolbar=a.creation_toolbar_rect();
                for target in 0..=2 {
                    let hits:Vec<_>=u.hits.iter().filter(|h|h.action==Action::RigStage(target)).collect();
                    assert_eq!(hits.len(),1);let r=hits[0].rect;
                    assert!(r[1]>=toolbar[1]+38.&&r[1]+r[3]<=toolbar[1]+toolbar[3]);
                    assert!(r[1]+r[3]<=a.properties_content_rect()[1]);
                }
            }
        }}
    }
    #[test]
    fn binding_remembers_the_mesh_when_building_and_creating_a_skeleton() {
        let mut a = app();
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        a.create_primitive(0);
        let target = a.editor.selected.unwrap();
        a.action(Action::SurfaceTab(0));
        a.ui = a.build_ui();
        a.open_dropdown(ui::DropdownKind::SkeletonCreate);
        assert!(a.dropdown_is_open(ui::DropdownKind::SkeletonCreate));
        a.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
        a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
        let root = a.authoring.skeleton.unwrap();
        assert_eq!(a.editor.selected, Some(root));
        assert_eq!(a.editor.mode, EditorMode::Scene);
        assert_eq!(a.surface.tab, 0);
        assert!(a.is_skeleton_object(root));
        assert_eq!(a.object_selection().len(), poseforge_core::HUMANOID_JOINT_COUNT + 1);
        assert_eq!(a.authoring.bind_target, Some(target));
        a.action(Action::Select(target));
        a.action(Action::Select(root + 1));
        assert_eq!(a.editor.selected, Some(root));
        assert_eq!(a.surface.tab, 0);
        click_stage(&mut a, 0);
        assert_eq!(a.editor.mode, EditorMode::Pose);
        assert!(a.selected_joint_properties());
        assert!(a.is_descendant(a.editor.selected.unwrap(), root));
        click_stage(&mut a, 1);
        assert_eq!(a.editor.selected, Some(target));
        assert_eq!(a.authoring.skeleton, Some(root));
        a.ui = a.build_ui();
        assert!(a
            .ui
            .hits
            .iter()
            .any(|h| h.action == Action::Author(A::Bind)));
        click_stage(&mut a, 2);
        assert!(
            a.editor
                .project
                .node(a.editor.selected.unwrap())
                .unwrap()
                .is_joint
        );
    }
    #[test]
    fn empty_bone_menu_and_object_settings_follow_selection_and_undo() {
        let mut a = app();
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        a.create_primitive(0);
        let mesh = a.editor.selected.unwrap();
        let before = a.editor.project.clone();
        let history = a.editor.undo_len();
        a.ui = a.build_ui();
        a.open_dropdown(ui::DropdownKind::SkeletonCreate);
        a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
        assert_eq!(a.editor.project, before);
        a.ui = a.build_ui();
        a.open_dropdown(ui::DropdownKind::SkeletonCreate);
        a.dropdown_key(&Key::Named(NamedKey::Home), false, false, false);
        a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
        let root = a.editor.selected.unwrap();
        assert!(a.is_skeleton_object(root));
        assert_eq!(a.object_selection().len(), 2);
        assert_eq!(a.editor.undo_len(), history + 1);
        assert_eq!(a.editor.project.node(mesh), before.node(mesh));
        assert_eq!(a.surface.tab, 0);
        assert!(a.rig_context());
        a.preferences.collapsed_sections.insert(1900);
        a.preferences.collapsed_sections.insert(1901);
        click_stage(&mut a, 0);
        assert_eq!(a.editor.selected, Some(root + 1));
        assert!(a.build_ui().hits.iter().any(|h| h.action == Action::Author(A::ExtendJoint)));
        a.action(Action::Mode(0));
        a.action(Action::Select(mesh));
        assert!(!a.rig_context());
        assert!(!a.build_ui().hits.iter().any(|h| matches!(h.action, Action::RigStage(_))));
        a.action(Action::Curve(ui::CurveAction::Create(0)));
        let curve = a.editor.selected.unwrap();
        assert!(a.selected_curve().is_some());
        let document = a.editor.project.clone();
        a.action(Action::Select(root));
        assert!(a.rig_context());
        a.action(Action::Select(curve));
        assert!(!a.rig_context());
        assert!(a.selected_curve().is_some());
        assert_eq!(a.editor.project, document);
        a.action(Action::Undo);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
    }

    #[test]
    fn object_settings_resolve_the_selected_skeleton_instead_of_the_previous_one() {
        let mut a = app();
        a.editor = Editor::new(Project::default());
        a.action(Action::Author(A::CreateBone));
        let first = a.editor.selected.unwrap();
        a.action(Action::Author(A::CreateBone));
        let second = a.editor.selected.unwrap();
        let document = a.editor.project.clone();
        for root in [first, second, first] {
            a.action(Action::Mode(0));
            a.action(Action::Select(root));
            assert_eq!(a.authoring.skeleton, Some(root));
            assert_eq!(a.active_rig_root, Some(root));
            click_stage(&mut a, 0);
            assert_eq!(a.editor.selected, Some(root + 1));
            assert_eq!(a.surface.tab, 0);
        }
        assert_eq!(a.editor.project, document);
        assert_eq!(a.editor.undo_len(), 2);
    }

    #[test]
    fn staged_pose_blocks_stage_switch_and_scene_returns_to_skeleton_object_selection() {
        let mut a = app();
        a.action(Action::RigStage(2));
        a.editor.mode = EditorMode::Animation;
        a.editor.auto_key = false;
        let mut t = a.editor.evaluated().unwrap().local[&9];
        t.rotation = Quat::from_rotation_x(0.3);
        a.editor.set_transform(9, t).unwrap();
        let before = a.editor.project.clone();
        a.action(Action::RigStage(1));
        assert_eq!(a.authoring.rig_stage, 2);
        assert!(a.editor.has_staged());
        assert_eq!(a.editor.project, before);
        a.action(Action::Discard);
        a.action(Action::Mode(0));
        assert!(a.object_selection().len() > 1);
    }
}
