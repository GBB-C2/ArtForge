use super::*;
use crate::ui::{BoneDisplayAction as B, PrimitiveAction as P};
use poseforge_core::{BoneDisplay, BoneDisplayPreset, BoneShape, JointShape, Node};

fn app() -> App {
    let mut a = App::new(None);
    let mut project = Project::default();
    for id in 1..=3 {
        let mut node = Node::new(id, format!("Bone {id}"));
        node.is_joint = true;
        node.transform.translation = Vec3::new((id - 1) as f32 * 0.7, 0., 0.);
        node.bone_tip = Some(Vec3::Y);
        project.nodes.push(node);
    }
    a.editor = Editor::new(project);
    a.editor.mode = EditorMode::Pose;
    a.preferences = Preferences {
        right_panel: true,
        ..Default::default()
    };
    a.width = 1440.;
    a.height = 1100.;
    a.select_object(1);
    a
}
fn point(a: &mut App, action: &Action) -> [f32; 2] {
    a.ui = a.build_ui();
    let max = a.inspector_max_scroll();
    for step in 0..=(max / 30.).ceil() as usize {
        a.inspector_scroll = (step as f32 * 30.).min(max);
        a.ui = a.build_ui();
        if let Some(hit) =
            a.ui.hits
                .iter()
                .rev()
                .find(|h| &h.action == action && h.rect[3] >= 24.)
        {
            return [
                hit.rect[0] + hit.rect[2] * 0.5,
                hit.rect[1] + hit.rect[3] * 0.5,
            ];
        }
    }
    panic!("Missing control: {action:?}")
}
fn select_two(a: &mut App) {
    a.modifiers = ModifiersState::SHIFT;
    a.select_object(2);
    a.modifiers = ModifiersState::empty();
    assert_eq!(a.object_selection(), BTreeSet::from([1, 2]));
}
fn swatch(a: &mut App, joint: bool) -> [f32; 2] {
    point(
        a,
        &Action::OpenColorPicker(Box::new(Action::BoneDisplay(if joint {
            B::JointColor
        } else {
            B::BodyColor
        }))),
    )
}

#[test]
fn appearance_group_presets_sizes_shapes_and_history_preserve_other_data() {
    let mut a = app();
    select_two(&mut a);
    let before = a.editor.project.clone();
    a.action(Action::BoneDisplay(B::Preset(BoneDisplayPreset::Controls)));
    assert_eq!(a.editor.undo_len(), 1);
    for id in [1, 2] {
        assert_eq!(
            a.editor.project.node(id).unwrap().bone_display,
            Some(BoneDisplayPreset::Controls.display())
        );
    }
    assert_eq!(a.editor.project.node(3), before.node(3));
    a.apply_field_value(Action::BoneDisplay(B::Size(false)), "2.5")
        .unwrap();
    a.apply_field_value(Action::BoneDisplay(B::Size(true)), "0.5")
        .unwrap();
    a.action(Action::BoneDisplay(B::ResetBodyShape));
    a.action(Action::BoneDisplay(B::ResetJointShape));
    for id in [1, 2] {
        let d = a.editor.project.node(id).unwrap().bone_display.unwrap();
        assert_eq!((d.size, d.joint_size), (2.5, 0.5));
        assert_eq!(
            (d.shape, d.joint_shape),
            (BoneShape::Octahedron, JointShape::Circle)
        );
        assert!(!d.filled);
        assert_eq!(
            a.editor.project.node(id).unwrap().transform,
            before.node(id).unwrap().transform
        );
    }
    a.action(Action::ResetNumber(Box::new(Action::BoneDisplay(B::Size(
        false,
    )))));
    let after = a.editor.project.clone();
    a.action(Action::Undo);
    assert_eq!(
        a.editor.project.node(1).unwrap().bone_display.unwrap().size,
        2.5
    );
    a.action(Action::Redo);
    assert_eq!(a.editor.project, after);
    for invalid in ["NaN", "0", "5.1"] {
        assert!(a
            .apply_field_value(Action::BoneDisplay(B::Size(false)), invalid)
            .is_err());
        assert_eq!(a.editor.project, after);
    }
    let json = serde_json::to_string(&after).unwrap();
    assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), after);
    a.editor.project.node_mut(1).unwrap().locked = true;
    let locked = a.editor.project.clone();
    a.action(Action::BoneDisplay(B::Preset(BoneDisplayPreset::Slim)));
    assert_eq!(a.editor.project, locked);
}

#[test]
fn appearance_group_color_cancel_and_commit_restore_distinct_originals() {
    let mut a = app();
    a.editor.project.node_mut(1).unwrap().bone_display = Some(BoneDisplay {
        color: [0.3, 0.8, 0.7],
        size: 2.,
        ..Default::default()
    });
    select_two(&mut a);
    let before = a.editor.project.clone();
    let source = Action::BoneDisplay(B::BodyColor);
    for apply in [false, true] {
        a.open_color_picker(source.clone());
        a.color_confirm(
            &Action::ColorPicker(crate::color_picker::ColorControl::Hex),
            "#CC3366",
        )
        .unwrap();
        assert_eq!(
            a.editor
                .project
                .node(1)
                .unwrap()
                .bone_display
                .unwrap()
                .color,
            a.editor
                .project
                .node(2)
                .unwrap()
                .bone_display
                .unwrap()
                .color
        );
        if !apply {
            a.cancel_color_picker();
            assert_eq!(a.editor.project, before);
            assert_eq!(a.editor.undo_len(), 0);
        } else {
            assert!(a.commit_color_picker());
        }
    }
    assert_eq!(a.editor.undo_len(), 1);
    let after = a.editor.project.clone();
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::Redo);
    assert_eq!(a.editor.project, after);
    assert_eq!(
        a.editor.project.node(1).unwrap().bone_display.unwrap().size,
        2.
    );
    // Pasting the primary's existing color must still update the other bone.
    a.action(Action::Undo);
    let color = a.swatch_value(&source).unwrap();
    a.apply_swatch(source, color);
    assert_eq!(
        a.editor
            .project
            .node(1)
            .unwrap()
            .bone_display
            .unwrap_or_default()
            .color,
        a.editor
            .project
            .node(2)
            .unwrap()
            .bone_display
            .unwrap_or_default()
            .color
    );
    assert_eq!(a.editor.undo_len(), 1);
}

#[test]
fn appearance_swatches_context_copy_paste_reset_undo_redo_and_dismissal() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let mut a = app();
        a.preferences.theme = theme;
        a.clipboard = None;
        a.editor.project.node_mut(1).unwrap().bone_display = Some(BoneDisplay {
            color: [0.2, 0.4, 0.8],
            ..Default::default()
        });
        let before = a.editor.project.clone();
        a.mouse = swatch(&mut a, false);
        a.mouse_down(MouseButton::Right);
        assert!(a.swatch_menu_open());
        assert!(a.color_picker.is_none());
        assert!(a.drag.is_none());
        let menu = a.build_ui();
        for text in ["Сбросить", "Отменить", "Вернуть", "Копировать", "Вставить"]
        {
            assert!(menu.texts.iter().any(|t| t.text == text));
        }
        assert!(!menu
            .hits
            .iter()
            .any(|h| h.action == Action::SwatchCommand(4)));
        a.action(Action::SwatchCommand(3));
        a.mouse = swatch(&mut a, true);
        a.mouse_down(MouseButton::Right);
        a.action(Action::SwatchCommand(4));
        assert_eq!(
            a.editor
                .project
                .node(1)
                .unwrap()
                .bone_display
                .unwrap()
                .joint_color,
            [0.2, 0.4, 0.8]
        );
        assert_eq!(a.editor.undo_len(), 1);
        a.mouse = swatch(&mut a, true);
        a.mouse_down(MouseButton::Right);
        a.action(Action::SwatchCommand(1));
        assert_eq!(a.editor.project, before);
        a.mouse = swatch(&mut a, true);
        a.mouse_down(MouseButton::Right);
        a.action(Action::SwatchCommand(2));
        assert_eq!(a.editor.undo_len(), 1);
        a.mouse = swatch(&mut a, true);
        a.mouse_down(MouseButton::Right);
        a.action(Action::SwatchCommand(0));
        assert_eq!(
            a.editor
                .project
                .node(1)
                .unwrap()
                .bone_display
                .unwrap()
                .joint_color,
            BoneDisplay::default().joint_color
        );
        let after = a.editor.project.clone();
        let selected = a.editor.selected;
        a.mouse = swatch(&mut a, false);
        a.mouse_down(MouseButton::Right);
        a.mouse = [700., 500.];
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
        assert!(!a.swatch_menu_open());
        assert!(a.drag.is_none());
        assert_eq!(a.editor.project, after);
        assert_eq!(a.editor.selected, selected);
    }
}

#[test]
fn appearance_color_drag_copies_once_and_cancel_or_empty_drop_do_nothing() {
    let mut a = app();
    a.editor.project.node_mut(1).unwrap().bone_display = Some(BoneDisplay {
        color: [0.21, 0.43, 0.87],
        ..Default::default()
    });
    // Scroll to the joint swatch; the body swatch is visible in the same panel.
    let target = swatch(&mut a, true);
    let source_action = Action::OpenColorPicker(Box::new(Action::BoneDisplay(B::BodyColor)));
    let r =
        a.ui.hits
            .iter()
            .find(|h| h.action == source_action)
            .unwrap()
            .rect;
    let source = [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5];
    let before = a.editor.project.clone();
    a.mouse = source;
    a.mouse_down(MouseButton::Left);
    assert!(a.color_picker.is_none());
    a.mouse_move(target);
    assert_eq!(a.editor.project, before);
    a.mouse_up(MouseButton::Left);
    assert_eq!(
        a.editor
            .project
            .node(1)
            .unwrap()
            .bone_display
            .unwrap()
            .joint_color,
        [0.21, 0.43, 0.87]
    );
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    for cancel in [false, true] {
        a.mouse = swatch(&mut a, false);
        a.mouse_down(MouseButton::Left);
        a.mouse_move([600., 400.]);
        if cancel {
            a.action(Action::Cancel);
        }
        a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), 0);
        assert!(a.color_picker.is_none());
    }
}

#[test]
fn appearance_collapse_and_size_slider_have_separate_view_and_document_history() {
    let mut a = app();
    select_two(&mut a);
    let before = a.editor.project.clone();
    let field = Action::BoneDisplay(B::Size(false));
    let _ = point(&mut a, &field);
    let (rect, action) =
        a.ui.hits
            .iter()
            .find(|h| matches!(&h.action,Action::Slide(inner,..) if **inner==field))
            .map(|h| (h.rect, h.action.clone()))
            .unwrap();
    a.mouse = [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5];
    a.action(action.clone());
    a.mouse_move([a.mouse[0] + 25., a.mouse[1]]);
    assert_ne!(a.editor.project, before);
    a.cancel_drag();
    assert_eq!(a.editor.project, before);
    a.action(action);
    a.mouse_move([a.mouse[0] + 25., a.mouse[1]]);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.undo_len(), 1);
    let height = a.bone_display_height();
    let history = a.editor.undo_len();
    a.action(Action::BoneDisplay(B::Collapse));
    assert!(a.bone_display_height() < height);
    let u = a.build_ui();
    assert!(!u.hits.iter().any(|h| h.action == field));
    a.action(Action::BoneDisplay(B::Collapse));
    assert_eq!(a.bone_display_height(), height);
    assert_eq!(a.editor.undo_len(), history);
}

#[test]
fn appearance_creation_panel_moves_clamps_cancels_and_click_collapses() {
    for curve in [false, true] {
        let mut a = app();
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        if curve {
            a.create_curve(1);
        } else {
            a.create_primitive(1);
        }
        let initial = a.primitive_panel_rect().unwrap();
        let document = a.editor.project.clone();
        let history = a.editor.undo_len();
        let header = point(&mut a, &Action::Primitive(P::Collapse));
        a.mouse = header;
        a.mouse_down(MouseButton::Left);
        a.mouse_move([header[0] + 120., header[1] - 80.]);
        a.mouse_up(MouseButton::Left);
        assert_ne!(a.primitive_panel_rect().unwrap(), initial);
        assert!(!a.primitive_collapsed);
        assert_eq!(a.editor.project, document);
        assert_eq!(a.editor.undo_len(), history);
        let position = a.preferences.creation_panel_position;
        let header = point(&mut a, &Action::Primitive(P::Collapse));
        a.mouse = header;
        a.mouse_down(MouseButton::Left);
        a.mouse_move([-500., -500.]);
        let r = a.primitive_panel_rect().unwrap();
        assert!(r[0] >= 0. && r[1] >= 0.);
        a.action(Action::Cancel);
        assert_eq!(a.preferences.creation_panel_position, position);
        let header = point(&mut a, &Action::Primitive(P::Collapse));
        a.mouse = header;
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
        assert!(a.primitive_collapsed);
        a.width = 900.;
        a.height = 600.;
        let r = a.primitive_panel_rect().unwrap();
        assert!(r[0] + r[2] <= a.width && r[1] + r[3] <= a.height);
    }
}

#[test]
fn appearance_settings_only_offer_theme_accent_and_language() {
    let a = app();
    let mut u = Ui::new(1.);
    a.draw_program_settings(&mut u, 100., 320.);
    assert!(u.hits.iter().all(|h| matches!(
        h.action,
        Action::InterfaceTheme(_)
            | Action::Language(_)
            | Action::DropdownOpen(ui::DropdownKind::Accent)
    )));
    assert!(!a.welcome.open);
}

#[test]
fn appearance_multi_bone_group_drag_and_modifier_toggle_are_reversible() {
    let mut a = app();
    select_two(&mut a);
    a.tool = Tool::Move;
    a.frame_all();
    let before = a.editor.project.clone();
    let origin = a.group_gizmo().unwrap().origin;
    a.mouse = a.project_point(origin).unwrap();
    assert!(a.begin_group_drag(crate::gizmo::Handle::Center));
    a.mouse_move(a.project_point(origin + Vec3::X * 0.5).unwrap());
    a.mouse_up(MouseButton::Left);
    assert_ne!(
        a.editor.project.node(1).unwrap().transform,
        before.node(1).unwrap().transform
    );
    assert_ne!(
        a.editor.project.node(2).unwrap().transform,
        before.node(2).unwrap().transform
    );
    assert_eq!(a.editor.project.node(3), before.node(3));
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.modifiers = ModifiersState::CONTROL;
    a.select_object(1);
    assert_eq!(a.object_selection(), BTreeSet::from([2]));
}

#[test]
#[ignore = "requires ARTFORGE_UI_CAPTURE_DIR; GPU capture is opt-in via ARTFORGE_UI_CAPTURE_GPU"]
fn appearance_capture_editor_ui() {
    let directory =
        PathBuf::from(std::env::var_os("ARTFORGE_UI_CAPTURE_DIR").expect("capture directory"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = std::env::var_os("ARTFORGE_UI_CAPTURE_GPU")
        .map(|_| pollster::block_on(Renderer::headless(1440, 1100)).unwrap());
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let name = if theme == ui::Theme::Dark {
            "dark"
        } else {
            "light"
        };
        for case in [
            "bones",
            "bottom",
            "collapsed",
            "preset",
            "colors",
            "settings",
            "panel",
        ] {
            let mut a = app();
            a.preferences.theme = theme;
            select_two(&mut a);
            a.frame_all();
            a.action(Action::BoneDisplay(B::Preset(BoneDisplayPreset::Controls)));
            if case == "settings" {
                a.action(Action::SurfaceTab(9));
            } else if case == "panel" {
                a.editor = Editor::new(Project::default());
                a.editor.mode = EditorMode::Scene;
                a.create_primitive(1);
                a.preferences.creation_panel_position = Some([430., 180.]);
                a.frame_all();
            } else {
                let _ = swatch(&mut a, true);
                if case == "bottom" {
                    a.inspector_scroll = a.inspector_max_scroll();
                }
                if case == "collapsed" {
                    a.action(Action::BoneDisplay(B::Collapse));
                }
                if case == "colors" {
                    a.mouse = swatch(&mut a, false);
                    a.mouse_down(MouseButton::Right);
                }
                if case == "preset" {
                    let _ = point(&mut a, &Action::DropdownOpen(ui::DropdownKind::BonePreset));
                    a.action(Action::DropdownOpen(ui::DropdownKind::BonePreset));
                }
            }
            let u = a.build_ui();
            let data = serde_json::json!({"width":a.width,"height":a.height,"background":u.palette().viewport,
                "source":"Actual App::build_ui output; CPU preview, not a native window capture",
                "rectangles":u.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),
                "texts":u.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
            std::fs::write(
                directory.join(format!("bones-{case}-{name}.json")),
                serde_json::to_vec(&data).unwrap(),
            )
            .unwrap();
            let Some(renderer) = renderer.as_mut() else {
                continue;
            };
            let evaluated = a.editor.evaluated().unwrap();
            renderer.invalidate_assets();
            renderer.set_editor_appearance(
                a.preferences.palette().viewport_linear(),
                a.preferences.grid,
            );
            renderer
                .capture_viewport_png(
                    &a.editor.project,
                    &evaluated.global,
                    &a.view(),
                    &u.rects,
                    &u.texts,
                    &[],
                    directory.join(format!("{case}-{name}.png")),
                    1440,
                    1100,
                )
                .unwrap();
        }
    }
}

#[test]
fn appearance_viewport_modifier_click_selects_bones_instead_of_starting_a_transform() {
    let mut a = app();
    a.tool = Tool::Move;
    a.frame_all();
    a.ui = a.build_ui();
    let before = a.editor.project.clone();
    let position = a.editor.project.node(2).unwrap().transform.translation + Vec3::Y * 0.4;
    a.mouse = a.project_point(position).unwrap();
    a.modifiers = ModifiersState::SHIFT;
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.object_selection(), BTreeSet::from([1, 2]));
    assert!(a.drag.is_none());
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
    a.ui = a.build_ui();
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.object_selection(), BTreeSet::from([1]));
}

#[test]
fn appearance_exact_color_text_keeps_caret_selection_and_keyboard_menu() {
    let mut a = app();
    let source = Action::BoneDisplay(B::BodyColor);
    let p = swatch(&mut a, false);
    a.input(source.clone(), "#334455".into());
    a.ui = a.build_ui();
    a.mouse = p;
    a.mouse_down(MouseButton::Left);
    assert!(a.text_selecting);
    assert!(a.drag.is_none());
    assert!(a.color_picker.is_none());
    a.mouse_move([p[0] + 12., p[1]]);
    a.mouse_up(MouseButton::Left);
    a.modifiers = ModifiersState::SHIFT;
    assert!(a.swatch_open_key(&Key::Named(NamedKey::F10)));
    assert!(a.swatch_menu_open());
    assert!(a.swatch_menu_key(&Key::Named(NamedKey::Escape)));
    assert!(!a.swatch_menu_open());
}
