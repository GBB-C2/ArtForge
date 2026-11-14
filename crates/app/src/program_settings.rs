//! Application preferences are independent from project edits and undo history.
use super::*;

impl App {
    pub(super) fn draw_program_settings(&self, u: &mut Ui, x: f32, width: f32) {
        let x = x + 16.;
        let w = width - 32.;
        let p = u.palette();
        u.fitted_label([x, 179., w, 18.], "Тема интерфейса", 12., p.muted);
        let half = (w - 6.) * 0.5;
        for (i, (label, theme)) in [("Тёмная", ui::Theme::Dark), ("Светлая", ui::Theme::Light)]
            .into_iter()
            .enumerate()
        {
            u.quiet_button(
                [x + i as f32 * (half + 6.), 204., half, 30.],
                label,
                Action::InterfaceTheme(theme),
                self.preferences.theme == theme,
            );
        }
        u.fitted_label([x, 248., w, 18.], "Акцент интерфейса", 12., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 274., w, 30.],
            ui::DropdownKind::Accent,
            "Выбрать цвет",
        );
        u.fitted_label([x,324.,w,18.],"Язык",12.,p.muted);
        for (i,(label,language)) in [("Русский",crate::localization::Language::Russian),("English β",crate::localization::Language::English)].into_iter().enumerate() {
            u.quiet_button([x+i as f32*(half+6.),350.,half,30.],label,Action::Language(language),self.preferences.language==language);
        }
        u.fitted_label([x,400.,w,18.],"Сетка модели",12.,p.muted);
        let c=self.preferences.mesh_color;
        u.color_field([x,426.,w,30.], "Цвет сетки", Action::MeshColor, c);
        u.number_field([x,468.,w,44.],"Непрозрачность, %", (self.preferences.mesh_opacity*100.) as f64,
            Action::MeshOpacity, SliderRange::new(0.,100.,1.));
        u.fitted_label([x,532.,w,18.],"Настройки сохраняются автоматически",11.,p.muted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Node, Skin};
    fn app() -> App {
        let mut a = App::new(None);
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        a.preferences = Preferences {
            right_panel: true,
            ..Preferences::default()
        };
        a.width = 1280.;
        a.height = 800.;
        a
    }
    fn rig(a: &mut App, start: u64, flat: bool) {
        let mut root = Node::new(start, "Armature");
        root.is_joint = flat;
        a.editor.project.nodes.push(root);
        for i in 1..4 {
            let mut n = Node::new(start + i, format!("joint_{i}"));
            n.is_joint = true;
            n.parent = (!flat).then_some(start + i - 1);
            n.transform.translation = Vec3::Y;
            a.editor.project.nodes.push(n);
        }
        if flat {
            let mut mesh = Node::new(start + 4, "Skin owner");
            mesh.skin = Some(Skin {
                joints: (start..start + 4).collect(),
                inverse_bind_matrices: vec![Mat4::IDENTITY; 4],
            });
            a.editor.project.nodes.push(mesh);
        }
    }
    #[test]
    fn bone_selection_is_one_skeleton_in_scene_and_one_joint_in_pose() {
        for flat in [false, true] {
            let mut a = app();
            rig(&mut a, 1, flat);
            rig(&mut a, 10, flat);
            a.select_object(3);
            assert_eq!(a.editor.selected, Some(1));
            assert_eq!(a.object_selection(), (1..5).collect());
            a.tool = Tool::Move;
            assert!(a.group_gizmo().is_some());
            let before = a.editor.project.clone();
            let positions = a.editor.evaluated().unwrap().global;
            let origin = a.group_gizmo().unwrap().origin;
            a.mouse = a.project_point(origin).unwrap();
            assert!(a.begin_group_drag(crate::gizmo::Handle::Center));
            a.mouse_move(a.project_point(origin + Vec3::X * 0.5).unwrap());
            a.mouse_up(MouseButton::Left);
            let moved = a.editor.evaluated().unwrap().global;
            // The center grip moves in the view plane, which need not coincide
            // with world X. Every bone must receive that same world-space delta.
            let expected = (moved[&1].w_axis - positions[&1].w_axis).truncate();
            assert!(expected.length() > 0.1);
            for id in 1..5 {
                let delta = (moved[&id].w_axis - positions[&id].w_axis).truncate();
                assert!(delta.distance(expected) < 0.001, "joint {id}: {delta:?}");
            }
            for id in 10..14 {
                assert_eq!(moved[&id], positions[&id]);
            }
            a.action(Action::Undo);
            assert_eq!(a.editor.project, before);
            a.editor.project.node_mut(2).unwrap().locked = true;
            assert!(a.group_gizmo().is_none());
            a.editor.project.node_mut(2).unwrap().locked = false;
            a.authoring.skeleton = Some(1);
            a.action(Action::RigStage(2));
            assert_eq!(a.editor.mode, EditorMode::Pose);
            a.select_object(3);
            assert_eq!(a.editor.selected, Some(3));
            assert_eq!(a.object_selection(), BTreeSet::from([3]));
            a.action(Action::Mode(0));
            assert_eq!(a.object_selection(), (1..5).collect());
            a.modifiers = winit::keyboard::ModifiersState::SHIFT;
            a.select_object(12);
            assert_eq!(a.object_selection(), (1..5).chain(10..14).collect());
            a.select_object(2);
            assert_eq!(a.object_selection(), (10..14).collect());
        }
    }
    #[test]
    fn program_settings_change_preferences_without_changing_the_document() {
        let mut a = app();
        let before = a.editor.project.clone();
        a.action(Action::SurfaceTab(9));
        a.ui = a.build_ui();
        assert!(a
            .ui
            .hits
            .iter()
            .any(|h| h.action == Action::DropdownOpen(ui::DropdownKind::Accent)));
        a.action(Action::DropdownOpen(ui::DropdownKind::Accent));
        a.action(Action::DropdownChoose(1));
        assert_eq!(a.preferences.accent, ui::Accent::Violet);
        a.action(Action::InterfaceTheme(ui::Theme::Light));
        a.action(Action::ToggleBlendImportAnimations);
        assert!(a.preferences.blend_import_animations);
        assert_eq!(
            a.build_ui().palette().accent,
            ui::Accent::Violet.palette(ui::Theme::Light).accent
        );
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), 0);
    }
    #[test]
    fn workspace_actions_and_gear_fit_compact_and_full_layouts() {
        for (width, height) in [(900., 600.), (1280., 800.), (1600., 1000.)] {
            let mut a = app();
            a.width = width;
            a.height = height;
            a.tool = Tool::Move;
            let u = a.build_ui();
            assert!(!u
                .hits
                .iter()
                .any(|h| matches!(h.action, Action::SetAxis(_) | Action::CharacterPanel)));
            let scene = a.left_panel_rect();
            let rail = a.rail_rect();
            assert_eq!(scene[0] + scene[2] + 8., rail[0]);
            assert_eq!(scene[1], rail[1]);
            let camera = u
                .hits
                .iter()
                .find(|h| h.action == Action::CreateCamera)
                .unwrap();
            assert!(ui::contains(
                scene,
                [camera.rect[0] + 14., camera.rect[1] + 14.]
            ));
            let gear = u
                .hits
                .iter()
                .find(|h| h.action == Action::SurfaceTab(9))
                .unwrap();
            assert!(gear.rect[1] + gear.rect[3] <= a.panel_bottom());
            assert!(!u.hits.iter().any(|h| matches!(
                h.action,
                Action::FrameSelected | Action::Duplicate
            ) && ui::contains(scene, [h.rect[0], h.rect[1]])));
            a.ui = u;
            a.action(Action::DropdownOpen(ui::DropdownKind::SceneActions));
            let popup = a.build_ui();
            // Keyboard navigation reaches the moved commands, including below the first page.
            assert!(popup
                .hits
                .iter()
                .any(|h| matches!(h.action, Action::DropdownChoose(_))));
            a.action(Action::DropdownChoose(8));
            assert!(a.dropdown.is_none());
        }
    }
    #[test]
    fn panel_tracks_follow_the_selected_interface_accent() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for accent in ui::Accent::ALL {
                for action in [
                    Action::EditTransform(0),
                    Action::Primitive(ui::PrimitiveAction::Field(0)),
                    Action::Author(ui::AuthorAction::LiveField(0)),
                    Action::SurfaceLightNumber(1),
                    Action::SurfaceRenderNumber(7),
                ] {
                    let mut u = Ui::with_theme(1., theme, [110., 36.]);
                    u.accent = accent;
                    let range = SliderRange::new(0., 10., 0.1);
                    u.set_slider_range(action.clone(), range);
                    u.number_field([10., 20., 240., 32.], "Параметр", 5., action, range);
                    let track = u.hits.last().unwrap().rect;
                    let point = [track[0] + track[2] * 0.25, track[1] + track[3] * 0.5];
                    assert!(u
                        .rects
                        .iter()
                        .any(|r| r.color == accent.palette(theme).accent
                            && ui::contains(r.rect, point)));
                }
            }
        }
    }
    #[test]
    fn timeline_and_layers_are_opaque_with_a_neutral_outline() {
        let mut a = app();
        a.editor.mode = EditorMode::Animation;
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            a.preferences.theme = theme;
            a.preferences.animation_layers_open = true;
            let mut u = Ui::with_theme(1., theme, [0.; 2]);
            a.animation.focused = true;
            a.draw_animation_timeline(&mut u);
            a.draw_animation_layers(&mut u);
            let r = a.timeline.dock_rect(a.width, a.height);
            let point = [r[0] + r[2] * 0.5, r[1] + 3.];
            assert!(u
                .rects
                .iter()
                .any(|q| ui::contains(q.rect, point) && q.color == theme.palette().panel));
            assert!(!u.rects.iter().any(|q| q.color == theme.palette().accent
                && q.rect[1] == r[1]
                && q.rect[2] > 100.));
            let layers = a.animation_layers_rect().unwrap();
            assert!(u
                .rects
                .iter()
                .any(|q| ui::contains(q.rect, [layers[0] + 12., layers[1] + 2.])
                    && q.color == theme.palette().panel));
        }
    }
}
