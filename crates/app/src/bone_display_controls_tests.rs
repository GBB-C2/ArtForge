use super::*;
use crate::color_picker::ColorControl as C;
use crate::ui::BoneDisplayAction as B;
use poseforge_core::{BoneDisplay, BoneShape, JointShape};

fn app() -> App {
    let mut a = App::new(None);
    a.editor = Editor::new(Project::default());
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    a.preferences.left_panel = true;
    a.width = 1280.;
    a.height = 900.;
    a.action(Action::Author(ui::AuthorAction::CreateBone));
    a.action(Action::RigStage(0));
    a.ui = a.build_ui();
    a
}

fn click_action(a: &mut App, action: Action) {
    let max = a.inspector_max_scroll();
    for i in 0..=(max / 40.).ceil() as usize {
        a.inspector_scroll = (i as f32 * 40.).min(max);
        a.ui = a.build_ui();
        if let Some(hit) =
            a.ui.hits
                .iter()
                .find(|h| h.action == action && h.rect[3] >= 24.)
        {
            let r = hit.rect;
            a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
            a.mouse_down(MouseButton::Left);
            a.mouse_up(MouseButton::Left);
            return;
        }
    }
    panic!("Control is unreachable: {action:?}");
}

#[test]
fn each_color_reset_is_independent_reversible_and_does_not_create_empty_history() {
    for reset in [B::ResetBodyColor, B::ResetJointColor] {
        let mut a = app();
        let id = a.editor.selected.unwrap();
        let initial = a.editor.project.clone();
        let history = a.editor.undo_len();
        a.action(Action::BoneDisplay(reset));
        assert_eq!(a.editor.project, initial);
        assert_eq!(a.editor.undo_len(), history);
        let custom = BoneDisplay {
            color: [0.2, 0.7, 0.8],
            joint_color: [0.9, 0.3, 0.5],
            shape: BoneShape::Cone,
            joint_shape: JointShape::Hexagon,
            filled: false,
            joint_filled: false,
            follow_view: true,
            joint_follow_view: false,
            ..Default::default()
        };
        a.editor.project.node_mut(id).unwrap().bone_display = Some(custom);
        let before = a.editor.project.clone();
        click_action(&mut a, Action::BoneDisplay(reset));
        let mut expected = before.clone();
        let d = expected
            .node_mut(id)
            .unwrap()
            .bone_display
            .as_mut()
            .unwrap();
        if reset == B::ResetBodyColor {
            d.color = BoneDisplay::default().color;
        } else {
            d.joint_color = BoneDisplay::default().joint_color;
        }
        assert_eq!(a.editor.project, expected);
        assert_eq!(a.editor.undo_len(), history + 1);
        a.action(Action::BoneDisplay(reset));
        assert_eq!(a.editor.undo_len(), history + 1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
        a.action(Action::Redo);
        assert_eq!(a.editor.project, expected);
        a.editor.project.node_mut(id).unwrap().locked = true;
        let locked = a.editor.project.clone();
        a.action(Action::BoneDisplay(reset));
        assert_eq!(a.editor.project, locked);
    }
}

#[test]
fn shape_lists_commit_every_variant_and_cancel_or_stale_selection_never_edits() {
    for (kind, count) in [
        (ui::DropdownKind::BoneShape, 6),
        (ui::DropdownKind::JointShape, 5),
    ] {
        let mut a = app();
        let initial = a.editor.project.clone();
        let history = a.editor.undo_len();
        click_action(&mut a, Action::DropdownOpen(kind));
        a.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
        assert_eq!(a.editor.project, initial);
        a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
        assert_eq!(a.editor.project, initial);
        for i in 0..count {
            let before = a.editor.project.clone();
            click_action(&mut a, Action::DropdownOpen(kind));
            a.ui = a.build_ui();
            let point = a.ui.action_point(&Action::DropdownChoose(i)).unwrap();
            a.mouse_move(point);
            a.mouse_down(MouseButton::Left);
            a.mouse_up(MouseButton::Left);
            assert_eq!(a.dropdown_selected(kind), Some(i));
            assert_eq!(a.editor.undo_len(), history + usize::from(i != 0));
            if i != 0 {
                let after = a.editor.project.clone();
                a.action(Action::Undo);
                assert_eq!(a.editor.project, before);
                a.action(Action::Redo);
                assert_eq!(a.editor.project, after);
                a.action(Action::Undo);
            }
        }
        click_action(&mut a, Action::DropdownOpen(kind));
        a.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
        a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
        assert_eq!(a.dropdown_selected(kind), Some(count - 1));
        a.action(Action::Undo);
        assert_eq!(a.editor.project, initial);
        click_action(&mut a, Action::DropdownOpen(kind));
        a.editor.selected = None;
        a.action(Action::DropdownChoose(count - 1));
        assert_eq!(a.editor.project, initial);
        assert_eq!(a.editor.undo_len(), history);
    }
}

#[test]
fn bone_and_joint_color_previews_cancel_and_commit_independently_as_one_undo() {
    for action in [B::BodyColor, B::JointColor] {
        let mut a = app();
        let id = a.editor.selected.unwrap();
        let before = a.editor.project.clone();
        let history = a.editor.undo_len();
        a.action(Action::BoneDisplay(action));
        assert!(a.color_picker.is_some());
        assert_eq!(
            a.color_picker.as_ref().unwrap().picker.draft.hex(),
            "#949EA3"
        );
        assert!(a
            .color_confirm(&Action::ColorPicker(C::Hex), "oops")
            .is_err());
        a.color_confirm(&Action::ColorPicker(C::Hex), "#CC3366")
            .unwrap();
        let value = a.editor.project.node(id).unwrap().bone_display.unwrap();
        assert_eq!(
            if action == B::BodyColor {
                value.color
            } else {
                value.joint_color
            },
            [0.8, 0.2, 0.4]
        );
        assert_eq!(
            if action == B::BodyColor {
                value.joint_color
            } else {
                value.color
            },
            BoneDisplay::default().color
        );
        assert_eq!(a.editor.undo_len(), history);
        a.action(Action::ColorPicker(C::Cancel));
        assert_eq!(a.editor.project, before);
        a.action(Action::BoneDisplay(action));
        a.color_confirm(&Action::ColorPicker(C::Hex), "#CC3366")
            .unwrap();
        a.action(Action::ColorPicker(C::Apply));
        assert_eq!(a.editor.undo_len(), history + 1);
        let after = a.editor.project.clone();
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
        a.action(Action::Redo);
        assert_eq!(a.editor.project, after);
        assert_eq!(
            a.editor.project.node(id).unwrap().transform,
            before.node(id).unwrap().transform
        );
        a.action(Action::Undo);
        a.action(Action::BoneDisplay(action));
        a.color_confirm(&Action::ColorPicker(C::Hex), "#112233")
            .unwrap();
        a.action(Action::ColorPicker(C::RestoreOriginal));
        a.action(Action::ColorPicker(C::Apply));
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), history);
    }
}

#[test]
fn display_shapes_and_view_tracking_persist_without_changing_other_bones_or_geometry() {
    let mut a = app();
    let id = a.editor.selected.unwrap();
    let before = a.editor.project.clone();
    for action in [
        B::BodyShape(BoneShape::Rectangle),
        B::JointShape(JointShape::Rectangle),
        B::BodyFilled,
        B::JointFilled,
        B::BodyFollowView,
        B::JointFollowView,
    ] {
        a.action(Action::BoneDisplay(action));
    }
    let d = a.editor.project.node(id).unwrap().bone_display.unwrap();
    assert_eq!(
        d,
        BoneDisplay {
            shape: BoneShape::Rectangle,
            joint_shape: JointShape::Rectangle,
            filled: false,
            joint_filled: false,
            follow_view: true,
            joint_follow_view: false,
            ..Default::default()
        }
    );
    let json = serde_json::to_string(&a.editor.project).unwrap();
    assert_eq!(
        serde_json::from_str::<Project>(&json).unwrap(),
        a.editor.project
    );
    for n in &before.nodes {
        assert_eq!(n.transform, a.editor.project.node(n.id).unwrap().transform);
    }
    a.action(Action::BoneDisplay(B::Reset));
    assert_eq!(a.editor.project, before);
    a.action(Action::Undo);
    assert_eq!(a.editor.project.node(id).unwrap().bone_display, Some(d));
    a.editor.project.node_mut(id).unwrap().locked = true;
    let locked = a.editor.project.clone();
    a.action(Action::BoneDisplay(B::Reset));
    a.action(Action::BoneDisplay(B::BodyColor));
    assert_eq!(a.editor.project, locked);
    assert!(a.color_picker.is_none());
    a.editor.project.node_mut(id).unwrap().locked = false;
    a.action(Action::Mode(0));
    let scene = a.editor.project.clone();
    a.action(Action::BoneDisplay(B::Reset));
    assert_eq!(a.editor.project, scene);
}

#[test]
fn appearance_controls_remain_reachable_in_both_themes_and_narrow_panels() {
    let required = [
        Action::BoneDisplay(B::BodyColor),
        Action::BoneDisplay(B::JointColor),
        Action::BoneDisplay(B::ResetBodyColor),
        Action::BoneDisplay(B::ResetJointColor),
        Action::DropdownOpen(ui::DropdownKind::BoneShape),
        Action::DropdownOpen(ui::DropdownKind::JointShape),
        Action::BoneDisplay(B::BodyFilled),
        Action::BoneDisplay(B::JointFilled),
        Action::BoneDisplay(B::BodyFollowView),
        Action::BoneDisplay(B::JointFollowView),
        Action::BoneDisplay(B::Reset),
    ];
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            for width in [280., 336., 440.] {
                let mut a = app();
                a.preferences.theme = theme;
                a.scale = scale;
                a.preferences.right_width = Some(width);
                let id = a.editor.selected.unwrap();
                a.editor.project.node_mut(id).unwrap().bone_display = Some(BoneDisplay {
                    color: [0.2, 0.7, 0.8],
                    joint_color: [0.9, 0.3, 0.5],
                    ..Default::default()
                });
                let mut seen = Vec::new();
                let max = a.inspector_max_scroll();
                for i in 0..=(max / 60.).ceil() as usize {
                    a.inspector_scroll = (i as f32 * 60.).min(max);
                    let ui = a.build_ui();
                    for hit in &ui.hits {
                        if required.contains(&hit.action) {
                            if hit.rect[3] >= 24. {
                                seen.push(hit.action.clone());
                            }
                            assert!(
                                hit.rect[0] >= a.right_panel_rect()[0]
                                    && hit.rect[0] + hit.rect[2] <= a.width + 0.1
                            );
                        }
                        assert_ne!(hit.action, Action::ToggleBodyMenu);
                    }
                }
                for action in &required {
                    assert!(
                        seen.contains(action),
                        "missing {action:?}, width={width}, scale={scale}"
                    );
                }
                for (kind, count) in [
                    (ui::DropdownKind::BoneShape, 6),
                    (ui::DropdownKind::JointShape, 5),
                ] {
                    click_action(&mut a, Action::DropdownOpen(kind));
                    assert!(a.dropdown_is_open(kind));
                    a.ui = a.build_ui();
                    for i in 0..count {
                        let hit =
                            a.ui.hits
                                .iter()
                                .find(|h| h.action == Action::DropdownChoose(i))
                                .unwrap();
                        assert!(hit.rect[0] >= 0. && hit.rect[1] >= 0.);
                        assert!(
                            hit.rect[0] + hit.rect[2] <= a.width
                                && hit.rect[1] + hit.rect[3] <= a.height
                        );
                        assert!(hit.rect[3] >= 24.);
                    }
                    a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
                }
            }
        }
    }
}

fn export_panel(a: &mut App, dir: &std::path::Path, name: &str) {
    let ui = a.build_ui();
    let data = serde_json::json!({"width":a.width,"height":a.height,"background":ui.palette().viewport,
        "source":"Actual application UI primitives; CPU preview, not a native GPU screenshot",
        "rectangles":ui.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),
        "texts":ui.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
    std::fs::write(
        dir.join(format!("bones-{name}.json")),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
}

#[test]
fn appearance_panel_exports_real_ui_for_visual_review() {
    let Some(dir) = std::env::var_os("ARTFORGE_QA_BONE_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, theme) in [("dark", ui::Theme::Dark), ("light", ui::Theme::Light)] {
        let mut a = app();
        a.preferences.theme = theme;
        a.preferences.right_width = Some(360.);
        a.height = 1050.;
        let id = a.editor.selected.unwrap();
        a.editor.project.node_mut(id).unwrap().bone_display = Some(BoneDisplay {
            shape: BoneShape::Cylinder,
            joint_shape: JointShape::Diamond,
            color: [0.25, 0.6, 0.85],
            joint_color: [0.9, 0.6, 0.3],
            follow_view: true,
            ..Default::default()
        });
        a.inspector_scroll = a.inspector_max_scroll();
        export_panel(&mut a, &dir, &format!("panel-{name}"));
        for (kind, label) in [
            (ui::DropdownKind::BoneShape, "body-list"),
            (ui::DropdownKind::JointShape, "joint-list"),
        ] {
            click_action(&mut a, Action::DropdownOpen(kind));
            export_panel(&mut a, &dir, &format!("{label}-{name}"));
            a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
        }
    }
}
