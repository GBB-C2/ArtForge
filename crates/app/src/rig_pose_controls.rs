//! Non-destructive posing controls over authored joints; no imported constraints
//! or bind data are rewritten. One drag or part command is one transaction.
use super::*;
use crate::ui::RigAction as R;
use std::hash::{Hash, Hasher};

pub(super) struct RigPoseState {
    signature: Option<u64>,
    profile: Option<RigProfile>,
    handles: bool,
    clipboard: BTreeMap<u64, Transform>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut a = App::new(None);
        let mut project = Project::demo();
        project.rig_profiles.clear();
        a.editor = Editor::new(project);
        a.preferences = Preferences::default();
        a.width = 1440.;
        a.height = 900.;
        a.editor.mode = EditorMode::Pose;
        a.active_rig_root = None;
        a.sync_rig_pose();
        a.select_role(role_index(BoneRole::LeftHand), true);
        a.frame_all();
        a
    }
    fn bend(a: &mut App) -> [u64; 3] {
        let chain = a.pose_chain(a.editor.selected.unwrap()).unwrap();
        let eval = a.editor.evaluated().unwrap();
        let root = eval.global[&chain[0]].w_axis.truncate();
        let end = eval.global[&chain[2]].w_axis.truncate();
        a.editor
            .apply_ik(
                chain[0],
                chain[1],
                chain[2],
                end + Vec3::new(0.15, -0.15, 0.05),
                root + Vec3::Z,
            )
            .unwrap();
        chain
    }
    #[test]
    fn automatic_picker_and_fk_ik_switch_do_not_rewrite_the_authored_rig() {
        let mut a = app();
        assert_eq!(a.editor.selected, Some(9));
        assert!(a.editor.project.rig_profiles.is_empty());
        bend(&mut a);
        let original = a.editor.project.clone();
        for ik in [false, true, false, true] {
            a.rig_action(R::Kinematics(ik));
            assert_eq!(a.ik, ik);
            assert_eq!(a.editor.project, original);
        }
        let mut u = Ui::with_theme(1., ui::Theme::Dark, [0., 0.]);
        a.draw_rig_handles(&mut u);
        assert_eq!(
            u.hits
                .iter()
                .filter(|h| matches!(h.action, Action::Rig(R::Endpoint(_))))
                .count(),
            4
        );
        assert!(u.hits.iter().any(|h| h.action == Action::Rig(R::Pole)));
        a.rig_action(R::Handles);
        let mut u = Ui::with_theme(1., ui::Theme::Dark, [0., 0.]);
        a.draw_rig_handles(&mut u);
        assert!(u.hits.is_empty());
    }
    #[test]
    fn endpoint_and_bend_drags_cancel_and_commit_as_one_edit() {
        for pole in [false, true] {
            let mut a = app();
            let chain = bend(&mut a);
            let original = a.editor.project.clone();
            let end = a.editor.evaluated().unwrap().global[&chain[2]]
                .w_axis
                .truncate();
            for commit in [false, true] {
                let point = if pole { a.rig_pole().unwrap().2 } else { end };
                a.mouse = a.project_point(point).unwrap();
                a.rig_action(if pole {
                    R::Pole
                } else {
                    R::Endpoint(role_index(BoneRole::LeftHand))
                });
                assert!(a.drag.is_some());
                a.mouse_move([a.mouse[0] + 30., a.mouse[1] + 21.]);
                assert_ne!(a.editor.project, original);
                if pole {
                    let actual = a.editor.evaluated().unwrap().global[&chain[2]]
                        .w_axis
                        .truncate();
                    assert!(
                        actual.distance(end) < 0.001,
                        "pole moves bend, not endpoint: {actual:?} {end:?}"
                    );
                }
                if commit {
                    a.mouse_up(MouseButton::Left);
                    assert!(a.drag.is_none());
                    a.action(Action::Undo);
                } else {
                    a.cancel_drag();
                }
                assert_eq!(a.editor.project, original);
            }
        }
    }
    #[test]
    fn part_pose_commands_are_scoped_atomic_and_key_the_visible_pose() {
        let mut a = app();
        let chain = bend(&mut a);
        let mut head = a.editor.evaluated().unwrap().local[&6];
        head.rotation = Quat::from_rotation_y(0.2);
        a.editor.set_transform(6, head).unwrap();
        let posed = a.editor.project.clone();
        a.rig_action(R::CopyPart);
        a.rig_action(R::ResetPart);
        assert_ne!(a.editor.project, posed);
        assert_eq!(a.editor.project.node(6), posed.node(6));
        assert_eq!(a.editor.project.meshes, posed.meshes);
        for node in &posed.nodes {
            let current = a.editor.project.node(node.id).unwrap();
            assert_eq!(current.parent, node.parent);
            assert_eq!(current.skin, node.skin);
        }
        let reset = a.editor.project.clone();
        a.action(Action::Undo);
        assert_eq!(a.editor.project, posed);
        a.action(Action::Redo);
        assert_eq!(a.editor.project, reset);
        a.rig_action(R::PastePart);
        assert_eq!(a.editor.project, posed);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, reset);
        a.action(Action::Redo);
        a.editor.time = 0.7;
        let visible = a.editor.evaluated().unwrap();
        let before = a.editor.project.clone();
        a.rig_action(R::KeyPart);
        assert_eq!(a.editor.mode, EditorMode::Animation);
        let evaluated = a.editor.evaluated().unwrap();
        for id in chain {
            assert_eq!(evaluated.local[&id], visible.local[&id]);
        }
        let clip = &a.editor.project.clips[a.editor.active_clip.unwrap()];
        let keyed: BTreeSet<_> = clip
            .channels
            .iter()
            .filter(|c| c.keys.iter().any(|k| (k.time - 0.7).abs() < 1e-9))
            .map(|c| c.node)
            .collect();
        assert!(chain.iter().all(|id| keyed.contains(id)));
        let unaffected: Vec<_> = clip
            .channels
            .iter()
            .filter(|c| !chain.contains(&c.node))
            .collect();
        let previous: Vec<_> = before.clips[a.editor.active_clip.unwrap()]
            .channels
            .iter()
            .filter(|c| !chain.contains(&c.node))
            .collect();
        assert_eq!(unaffected, previous, "keys outside the limb are preserved");
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
    }
    #[test]
    fn locked_or_flat_limb_never_exposes_an_ik_drag() {
        for flat in [false, true] {
            let mut a = app();
            if flat {
                for id in [7, 8, 9] {
                    a.editor.project.node_mut(id).unwrap().parent = Some(1);
                }
                a.sync_rig_pose();
            } else {
                a.editor.project.node_mut(7).unwrap().locked = true;
            }
            let original = a.editor.project.clone();
            assert!(a.rig_chain_enabled(BoneRole::LeftHand).is_none());
            a.ik = false;
            a.rig_action(R::Kinematics(true));
            assert!(!a.ik);
            a.rig_action(R::Endpoint(role_index(BoneRole::LeftHand)));
            assert!(a.drag.is_none());
            assert_eq!(a.editor.project, original);
        }
    }

    #[test]
    fn round_ik_targets_reject_the_corners_in_both_themes_and_dpi() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {
            for scale in [1.,2.] {
                let mut a = app(); a.preferences.theme = theme; a.scale = scale;
                bend(&mut a);
                let mut u = Ui::with_theme(scale,theme,[0.;2]);
                a.draw_rig_handles(&mut u);
                for hit in u.hits.iter().filter(|h| matches!(h.action,Action::Rig(R::Endpoint(_) | R::Pole))) {
                    let r = hit.rect;
                    let center = [r[0]+r[2]*0.5,r[1]+r[3]*0.5];
                    assert_eq!(u.hit(center),Some(hit.action.clone()));
                    assert_ne!(u.hit([r[0]+0.5,r[1]+0.5]),Some(hit.action.clone()));
                }
            }
        }
    }
    #[test]
    fn body_rings_are_hidden_and_fk_commands_preserve_skin_and_pose() {
        let mut a = app(); let original = a.editor.project.clone();
        for role in [BoneRole::Pelvis,BoneRole::Spine,BoneRole::Chest,BoneRole::Head] {
            let mut u = Ui::with_theme(1.,ui::Theme::Dark,[0.;2]);
            a.draw_rig_handles(&mut u);
            let action = Action::Rig(R::Controller(role_index(role)));
            assert!(u.action_point(&action).is_none(), "body rings must not cover the skeleton");
            a.scene_tools.selected = a.pose_profile().unwrap().assignments.values().copied().collect();
            a.action(action);
            assert_eq!(a.object_selection(), a.editor.selected.into_iter().collect());
            assert_eq!(a.editor.selected,a.pose_profile().unwrap().assignments.get(&role).copied());
            assert!(!a.ik);
            assert_eq!(a.editor.project,original);
            assert_eq!(a.editor.undo_len(),0);
        }
    }
    #[test]
    fn clipped_ik_circle_keeps_its_original_pick_center() {
        for scale in [1.,2.] {
            let mut u = Ui::with_theme(scale,ui::Theme::Dark,[0.;2]);
            let action = Action::Rig(R::Pole);
            u.path_hit(action.clone(),vec![([5.,10.],[5.,10.])],15.);
            clip_scrolled(&mut u,(0,0,0),[10.,0.,20.,25.],0.);
            assert_eq!(u.hit([10.,10.]),Some(action.clone()));
            assert_eq!(u.hit([19.,10.]),Some(action));
            assert_eq!(u.hit([21.,10.]),None);
            assert_eq!(u.hit([9.,10.]),None);
        }
    }
}
impl Default for RigPoseState {
    fn default() -> Self {
        Self {
            signature: None,
            profile: None,
            handles: true,
            clipboard: BTreeMap::new(),
        }
    }
}
#[derive(Clone)]
pub(super) struct PoleDrag {
    gesture: crate::gizmo::GizmoDrag,
    chain: [u64; 3],
    target: Vec3,
    current: Vec3,
}
const ENDS: [BoneRole; 4] = [
    BoneRole::LeftHand,
    BoneRole::RightHand,
    BoneRole::LeftFoot,
    BoneRole::RightFoot,
];
fn role_index(role: BoneRole) -> usize {
    BoneRole::ALL.iter().position(|r| *r == role).unwrap()
}

impl App {
    pub(super) fn sync_rig_pose(&mut self) {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.active_rig_root.hash(&mut hash);
        for node in &self.editor.project.nodes {
            (node.id, node.parent, node.is_joint, &node.name).hash(&mut hash);
            if let Some(skin) = &node.skin {
                skin.joints.hash(&mut hash);
            }
        }
        for mesh in &self.editor.project.meshes {
            (
                mesh.id,
                mesh.vertices.as_ptr() as usize,
                mesh.vertices.len(),
            )
                .hash(&mut hash);
        }
        let signature = hash.finish();
        if self.rig_pose.signature == Some(signature) {
            return;
        }
        self.rig_pose.signature = Some(signature);
        let roots = poseforge_core::character_roots(&self.editor.project);
        let root = self
            .active_rig_root
            .or_else(|| {
                self.editor.selected.and_then(|id| {
                    roots
                        .iter()
                        .copied()
                        .find(|root| self.is_descendant(id, *root))
                })
            })
            .or_else(|| (roots.len() == 1).then(|| roots[0]));
        self.rig_pose.profile =
            Some(poseforge_core::detect_humanoid(&self.editor.project, root).resolved);
    }
    pub(super) fn pose_profile(&self) -> Option<&RigProfile> {
        self.current_profile().or(self.rig_pose.profile.as_ref())
    }
    fn rig_chain_enabled(&self, role: BoneRole) -> Option<[u64; 3]> {
        let chain = self
            .pose_profile()?
            .chain(role, &self.editor.project)
            .ok()?;
        chain
            .iter()
            .all(|id| self.node_selectable(*id))
            .then_some(chain)
    }
    fn rig_part(&self) -> Vec<u64> {
        let Some(id) = self.editor.selected else {
            return Vec::new();
        };
        for role in ENDS {
            if let Some(chain) = self.rig_chain_enabled(role).filter(|c| c.contains(&id)) {
                return chain.into();
            }
        }
        self.editor
            .project
            .node(id)
            .filter(|n| n.is_joint && self.node_selectable(id))
            .map_or_else(Vec::new, |_| vec![id])
    }
    pub(super) fn rig_pole(&self) -> Option<([u64; 3], Vec3, Vec3)> {
        if !self.ik || !matches!(self.editor.mode, EditorMode::Pose | EditorMode::Animation) {
            return None;
        }
        let chain = self.pose_chain(self.editor.selected?).ok()?;
        if chain.iter().any(|id| !self.node_selectable(*id)) {
            return None;
        }
        let eval = self.evaluated_interactive().ok()?;
        let p = chain.map(|id| eval.global[&id].w_axis.truncate());
        let axis = (p[2] - p[0]).try_normalize()?;
        let bend = (p[1] - p[0] - axis * (p[1] - p[0]).dot(axis))
            .try_normalize()
            .unwrap_or_else(|| axis.any_orthonormal_vector());
        let length = p[0].distance(p[1]) + p[1].distance(p[2]);
        Some((chain, p[2], p[1] + bend * length.max(0.1) * 0.65))
    }
    fn begin_rig_pole(&mut self) {
        let Some((chain, target, pole)) = self.rig_pole() else {
            return;
        };
        let Some(gizmo) = crate::gizmo::Gizmo::new(
            crate::gizmo::Mode::Move,
            Mat4::from_translation(pole),
            false,
            self.view(),
            self.viewport(),
        ) else {
            return;
        };
        let transform = Transform {
            translation: pole,
            ..Transform::default()
        };
        let Some(gesture) = crate::gizmo::GizmoDrag::begin(
            gizmo,
            crate::gizmo::Handle::Center,
            transform,
            Mat4::IDENTITY,
            self.mouse,
        ) else {
            return;
        };
        self.editor.playing = false;
        self.editor.begin_edit("Направление сгиба IK");
        self.drag = Some(Drag::RigPole(PoleDrag {
            gesture,
            chain,
            target,
            current: pole,
        }));
        self.drag_button = Some(MouseButton::Left);
    }
    pub(super) fn update_rig_pole(&mut self, mut drag: PoleDrag, point: [f32; 2]) {
        if let Some((transform, pole, _)) = drag.gesture.update(
            point,
            self.modifiers.shift_key(),
            self.modifiers.control_key(),
        ) {
            let result = if drag.gesture.at_start(transform, pole) {
                self.editor.cancel_edit();
                self.editor.begin_edit("Направление сгиба IK");
                Ok(())
            } else {
                self.editor.apply_ik(
                    drag.chain[0],
                    drag.chain[1],
                    drag.chain[2],
                    drag.target,
                    pole,
                )
            };
            if self.result(result).is_some() {
                drag.current = pole;
            }
        }
        self.drag = Some(Drag::RigPole(drag));
    }
    pub(super) fn rig_action(&mut self, action: R) {
        if self.job.is_some() || self.has_modal() {
            return;
        }
        self.sync_rig_pose();
        match action {
            R::Handles => self.rig_pose.handles = !self.rig_pose.handles,
            R::Controller(index) => self.select_role(index, false),
            R::Endpoint(index) => {
                self.select_role(index, true);
                if self.ik
                    && self
                        .pose_profile()
                        .and_then(|p| BoneRole::ALL.get(index).and_then(|r| p.assignments.get(r)))
                        .copied()
                        == self.editor.selected
                {
                    self.begin_gizmo_drag(crate::gizmo::Handle::Center);
                }
            }
            R::Pole => self.begin_rig_pole(),
            R::Kinematics(ik) => {
                if ik
                    && self
                        .editor
                        .selected
                        .is_none_or(|id| self.pose_chain(id).is_err())
                {
                    self.status =
                        "IK доступен для кисти или стопы с прямой цепью из трёх костей.".into();
                    return;
                }
                self.ik = ik;
                self.tool = if ik { Tool::Move } else { Tool::Rotate };
                self.editor.playing = false;
            }
            R::KeyPart | R::ResetPart | R::CopyPart | R::PastePart => {
                let ids = self.rig_part();
                if ids.is_empty() {
                    return;
                }
                if ids.iter().any(|id| !self.node_selectable(*id)) {
                    return;
                }
                if action == R::KeyPart {
                    // Capture the currently visible Pose result before enabling
                    // clip evaluation; existing keys must not replace this pose.
                    let result = self.editor.key_nodes(&ids);
                    if self.result(result).is_some() {
                        self.editor.mode = EditorMode::Animation;
                        self.preferences.animation_visible = true;
                    }
                } else if action == R::CopyPart {
                    if let Ok(eval) = self.evaluated_interactive() {
                        self.rig_pose.clipboard =
                            ids.into_iter().map(|id| (id, eval.local[&id])).collect();
                    }
                } else {
                    if action == R::PastePart
                        && ids.iter().copied().collect::<BTreeSet<_>>()
                            != self.rig_pose.clipboard.keys().copied().collect()
                    {
                        self.status =
                            "Выберите ту же часть скелета, из которой скопирована поза.".into();
                        return;
                    }
                    let selected = self.editor.selected;
                    self.editor.begin_edit(if action == R::ResetPart {
                        "Сбросить часть позы"
                    } else {
                        "Вставить часть позы"
                    });
                    for id in ids {
                        let result = if action == R::ResetPart {
                            self.editor.selected = Some(id);
                            self.editor.reset_selected()
                        } else {
                            self.editor.set_transform(id, self.rig_pose.clipboard[&id])
                        };
                        if let Err(e) = result {
                            self.editor.cancel_edit();
                            self.editor.selected = selected;
                            self.status = e.to_string();
                            return;
                        }
                    }
                    self.editor.selected = selected;
                    let result = self.editor.commit_edit();
                    self.result(result);
                }
            }
        }
        self.update_dirty();
        self.redraw();
    }
    pub(super) fn draw_rig_handles(&self, u: &mut Ui) {
        if !self.rig_pose.handles
            || self.weights.target.is_some()
            || (self.surface.tab == 0 && self.rig_context() && self.authoring.rig_stage == 1)
            || !self.preferences.skeleton
            || self.has_modal()
            || !matches!(self.editor.mode, EditorMode::Pose | EditorMode::Animation)
            || self.editor.playing
        {
            return;
        }
        let Ok(eval) = self.evaluated_interactive() else {
            return;
        };
        let p = u.palette();
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        for role in ENDS {
            let Some(chain) = self.rig_chain_enabled(role) else {
                continue;
            };
            let Some(point) = self.project_point(eval.global[&chain[2]].w_axis.truncate()) else {
                continue;
            };
            let active = self.editor.selected == Some(chain[2]) && self.ik;
            let size = if active { 26. } else { 22. };
            let color = if active { p.accent } else { crate::bones::role_color(Some(role), &p) };
            u.rounded_rect([point[0]-size/2.-1.5, point[1]-size/2.-1.5, size+3., size+3.], [0.02,0.025,0.035,0.8], (size+3.)/2.);
            u.rounded_rect(
                [point[0] - size / 2., point[1] - size / 2., size, size],
                color,
                size / 2.,
            );
            u.rounded_rect([point[0]-size/2.+2., point[1]-size/2.+2., size-4., size-4.], p.panel, (size-4.)/2.);
            Icon::Move.draw(u, [point[0] - 5., point[1] - 5., 10., 10.], color);
            // A point path retains its original centre when its broad-phase
            // rectangle is clipped at the viewport boundary.
            u.path_hit(Action::Rig(R::Endpoint(role_index(role))), vec![(point, point)], 15.);
        }
        if let Some((chain, _, pole)) = self.rig_pole() {
            let pole = match &self.drag {
                Some(Drag::RigPole(d)) => d.current,
                _ => pole,
            };
            if let (Some(point), Some(middle)) = (
                self.project_point(pole),
                self.project_point(eval.global[&chain[1]].w_axis.truncate()),
            ) {
                let color = p.accent;
                let (a,b) = (glam::Vec2::from_array(middle), glam::Vec2::from_array(point));
                let length = a.distance(b).max(1.);
                for segment in 0..((length/8.).ceil() as usize).min(300) {
                    u.smooth_segment(a.lerp(b,(segment as f32*8./length).min(1.)).to_array(),
                        a.lerp(b,((segment as f32*8.+4.)/length).min(1.)).to_array(), 1.2, color);
                }
                u.rounded_rect([point[0]-12.,point[1]-12.,24.,24.], [0.02,0.025,0.035,0.8], 12.);
                u.rounded_rect([point[0]-10.5,point[1]-10.5,21.,21.], color, 10.5);
                u.rounded_rect([point[0]-8.,point[1]-8.,16.,16.], p.panel, 8.);
                u.rounded_rect([point[0]-2.,point[1]-2.,4.,4.], color, 2.);
                u.path_hit(Action::Rig(R::Pole), vec![(point, point)], 14.);
                u.fitted_label(
                    [point[0] + 14., point[1] - 8., 100., 18.],
                    "Сгиб IK",
                    11.,
                    p.text,
                );
            }
        }
        clip_scrolled(u, start, self.viewport(), 0.);
    }

    pub(super) fn draw_rig_pose_panel(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let p = u.palette();
        u.fitted_label([x, y, w, 18.], "Выбор части тела", 12., p.text);
        y += 24.;
        let cell = (w - 12.) / 3.;
        // Front-facing body layout: the character's right side is screen-left.
        let slots = [
            (BoneRole::Head, 1, 0),
            (BoneRole::Neck, 1, 1),
            (BoneRole::Chest, 1, 2),
            (BoneRole::Spine, 1, 3),
            (BoneRole::Pelvis, 1, 4),
            (BoneRole::RightUpperArm, 0, 2),
            (BoneRole::RightForearm, 0, 3),
            (BoneRole::RightHand, 0, 4),
            (BoneRole::LeftUpperArm, 2, 2),
            (BoneRole::LeftForearm, 2, 3),
            (BoneRole::LeftHand, 2, 4),
            (BoneRole::RightThigh, 0, 5),
            (BoneRole::RightShin, 0, 6),
            (BoneRole::RightFoot, 0, 7),
            (BoneRole::LeftThigh, 2, 5),
            (BoneRole::LeftShin, 2, 6),
            (BoneRole::LeftFoot, 2, 7),
        ];
        for (role, col, row) in slots {
            let rect = [
                x + col as f32 * (cell + 6.),
                y + row as f32 * 30.,
                cell,
                26.,
            ];
            let node = self
                .pose_profile()
                .and_then(|p| p.assignments.get(&role))
                .copied();
            let label = match role {
                BoneRole::LeftUpperArm | BoneRole::RightUpperArm => "Плечо",
                BoneRole::LeftForearm | BoneRole::RightForearm => "Локоть",
                BoneRole::LeftHand | BoneRole::RightHand => "Кисть",
                BoneRole::LeftThigh | BoneRole::RightThigh => "Бедро",
                BoneRole::LeftShin | BoneRole::RightShin => "Колено",
                BoneRole::LeftFoot | BoneRole::RightFoot => "Стопа",
                _ => role.label(),
            };
            if node.is_some_and(|id| self.node_selectable(id)) {
                let action = if role.is_ik_end() && self.rig_chain_enabled(role).is_some() {
                    Action::IkRole(role_index(role))
                } else {
                    Action::SelectRole(role_index(role))
                };
                u.quiet_button(rect, label, action, self.editor.selected == node);
            } else {
                u.disabled_button(rect, label);
            }
        }
        u.fitted_label([x, y + 4., cell, 16.], "Правая", 10., p.muted);
        u.fitted_label(
            [x + 2. * (cell + 6.), y + 4., cell, 16.],
            "Левая",
            10.,
            p.muted,
        );
        y += 246.;
        let half = (w - 6.) * 0.5;
        u.quiet_button(
            [x, y, half, 28.],
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
                [x + half + 6., y, half, 28.],
                "IK",
                Action::Rig(R::Kinematics(true)),
                self.ik,
            );
        } else {
            u.disabled_button([x + half + 6., y, half, 28.], "IK");
        }
        y += 36.;
        u.checkbox(
            [x, y, w, 28.],
            "Ручки кистей, стоп и сгиба",
            Action::Rig(R::Handles),
            self.rig_pose.handles,
        );
        y += 36.;
        let part = self.rig_part();
        for (i, (label, a, icon)) in [
            ("Ключ части", R::KeyPart, Icon::Key),
            ("Сбросить", R::ResetPart, Icon::Undo),
            ("Копия", R::CopyPart, Icon::Copy),
            ("Вставить", R::PastePart, Icon::Paste),
        ]
        .into_iter()
        .enumerate()
        {
            let r = [
                x + (i % 2) as f32 * (half + 6.),
                y + (i / 2) as f32 * 36.,
                half,
                30.,
            ];
            if part.is_empty() || (a == R::PastePart && self.rig_pose.clipboard.is_empty()) {
                u.disabled_icon_button(r, label, icon);
            } else {
                u.quiet_icon_button(r, label, Action::Rig(a), false, icon);
            }
        }
        y += 80.;
        u.quiet_button(
            [x, y, w, 30.],
            "Соответствие костей…",
            Action::CharacterPanel,
            false,
        );
        y += 38.;
        u.fitted_label(
            [x, y, w, 18.],
            if self.current_profile().is_some() {
                "Используется сохранённый профиль"
            } else {
                "Автоопределение · проверьте соответствие"
            },
            10.,
            p.muted,
        );
    }
}
