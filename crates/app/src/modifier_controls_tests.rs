use super::*;
use crate::ui::{DropdownKind, ModifierAction as M};
use poseforge_core::{
    ModifierKind as K, ModifierSettings as S, Node, PrimitiveKind, PrimitiveSettings,
};

fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Default::default();
    a.width = 1440.;
    a.height = 1050.;
    a.preferences.right_width = Some(360.);
    a.preferences.left_panel = true;
    a.preferences.right_panel = true;
    let mut one = Node::new(1, "Куб");
    one.mesh = Some(10);
    let mut two = Node::new(2, "Другой меш");
    two.mesh = Some(10);
    two.transform.translation.x = 8.;
    a.editor = Editor::new(Project {
        nodes: vec![one, two],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(10, None)
            .unwrap()],
        ..Default::default()
    });
    a.reset_workflow_selection();
    a.editor.mode = EditorMode::Scene;
    a.editor.selected = Some(1);
    a.set_properties_tab(8);
    a
}
fn point(a: &mut App, action: Action, scene: bool) -> [f32; 2] {
    a.ui = a.build_ui();
    let r =
        a.ui.hits
            .iter()
            .rev()
            .find(|h| {
                h.action == action
                    && (!scene
                        || ui::contains(
                            a.scene_list_rect(),
                            [h.rect[0] + h.rect[2] * 0.5, h.rect[1] + h.rect[3] * 0.5],
                        ))
            })
            .unwrap_or_else(|| panic!("Missing hit {action:?}, scroll {}", a.inspector_scroll))
            .rect;
    [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]
}
fn click(a: &mut App, action: Action) {
    let p = point(a, action, false);
    a.mouse_move(p);
    a.click_ui_control();
}
fn add(a: &mut App, kind: K) -> u64 {
    a.action(Action::Modifier(M::Add(kind)));
    a.procedural.selected.unwrap().1
}
fn drag(a: &mut App, from: Action, to: Action, offset: f32, cancel: bool) {
    let from = point(a, from, true);
    let mut to = point(a, to, true);
    to[1] += offset;
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move(to);
    if cancel {
        a.action(Action::Cancel);
    }
    a.mouse_up(MouseButton::Left);
}

#[test]
fn dropdown_adds_every_kind_and_stale_selection_cannot_retarget_the_command() {
    for (index, kind) in K::ALL.into_iter().enumerate() {
        let mut a = app();
        click(&mut a, Action::DropdownOpen(DropdownKind::ModifierAdd));
        a.choose_dropdown(index);
        assert_eq!(
            a.editor.project.node(1).unwrap().modifiers[0]
                .settings
                .kind(),
            kind
        );
        assert_eq!(a.editor.undo_len(), 1);
        assert!(a
            .outliner_rows()
            .iter()
            .any(|r| matches!(r, scene_tree::SceneEntry::Modifier(1, 1, _))));
        a.action(Action::Undo);
        assert!(a.editor.project.node(1).unwrap().modifiers.is_empty());
    }
    let mut a = app();
    click(&mut a, Action::DropdownOpen(DropdownKind::ModifierAdd));
    let before = a.editor.project.clone();
    a.editor.selected = Some(2);
    a.choose_dropdown(0);
    assert_eq!(a.editor.project, before);
}
#[test]
fn parameter_entry_cancel_invalid_integer_reset_and_history_use_real_controls() {
    let mut a = app();
    let id = add(&mut a, K::Array);
    let field = Action::Modifier(M::Field(1, id, 1));
    click(&mut a, field.clone());
    let draft = a.input.as_mut().unwrap();
    draft.select_all();
    draft.insert("3,5");
    assert!(!a.confirm_input());
    assert_eq!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        K::Array.settings()
    );
    let draft = a.input.as_mut().unwrap();
    draft.select_all();
    draft.insert("5");
    assert!(a.confirm_input());
    assert_eq!(a.editor.undo_len(), 2);
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        S::Array { count: 5, .. }
    ));
    click(&mut a, field);
    let draft = a.input.as_mut().unwrap();
    draft.select_all();
    draft.insert("7");
    a.action(Action::Cancel);
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        S::Array { count: 5, .. }
    ));
    click(&mut a, Action::Modifier(M::Reset(1, id)));
    assert_eq!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        K::Array.settings()
    );
    a.action(Action::Undo);
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        S::Array { count: 5, .. }
    ));
}
#[test]
fn scene_drag_reorders_then_moves_or_copies_between_meshes_with_one_undo_step() {
    let mut a = app();
    let first = add(&mut a, K::Array);
    let second = add(&mut a, K::Subdivision);
    let before = a.editor.project.clone();
    let history = a.editor.undo_len();
    drag(
        &mut a,
        Action::Modifier(M::Select(1, second)),
        Action::Modifier(M::Select(1, first)),
        -8.,
        true,
    );
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), history);
    drag(
        &mut a,
        Action::Modifier(M::Select(1, second)),
        Action::Modifier(M::Select(1, first)),
        -8.,
        false,
    );
    assert_eq!(a.editor.project.nodes[0].modifiers[0].id, second);
    assert_eq!(a.editor.undo_len(), history + 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.modifiers = ModifiersState::CONTROL;
    drag(
        &mut a,
        Action::Modifier(M::Select(1, first)),
        Action::Select(2),
        0.,
        false,
    );
    assert_eq!(a.editor.project.nodes[0].modifiers.len(), 2);
    assert_eq!(a.editor.project.nodes[1].modifiers.len(), 1);
    a.action(Action::Undo);
    a.modifiers = ModifiersState::empty();
    drag(
        &mut a,
        Action::Modifier(M::Select(1, first)),
        Action::Select(2),
        0.,
        false,
    );
    assert_eq!(a.editor.project.nodes[0].modifiers.len(), 1);
    assert_eq!(a.editor.project.nodes[1].modifiers.len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn locked_drop_and_release_after_history_change_leave_the_stack_untouched() {
    let mut a = app();
    let id = add(&mut a, K::Array);
    a.editor.project.node_mut(2).unwrap().locked = true;
    let before = a.editor.project.clone();
    drag(
        &mut a,
        Action::Modifier(M::Select(1, id)),
        Action::Select(2),
        0.,
        false,
    );
    assert_eq!(a.editor.project, before);
    a.editor.project.node_mut(2).unwrap().locked = false;
    let from = point(&mut a, Action::Modifier(M::Select(1, id)), true);
    let to = point(&mut a, Action::Select(2), true);
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move(to);
    a.editor.add_modifier(1, K::Mirror).unwrap();
    let changed = a.editor.project.clone();
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project, changed);
}
#[test]
fn keyboard_destination_picker_copies_and_scene_eye_restores_the_original_mesh() {
    let mut a = app();
    let id = add(&mut a, K::Array);
    let original = a.editor.project.mesh(10).unwrap().clone();
    click(
        &mut a,
        Action::DropdownOpen(DropdownKind::ModifierTarget(true, 1)),
    );
    a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
    assert_eq!(a.editor.project.nodes[0].modifiers.len(), 1);
    assert_eq!(a.editor.project.nodes[1].modifiers.len(), 1);
    a.action(Action::Undo);
    a.action(Action::Modifier(M::Select(1, id)));
    a.editor.evaluated().unwrap();
    assert_eq!(
        a.evaluated_interactive().unwrap().modifier_meshes[&1]
            .vertices
            .len(),
        original.vertices.len() * 2
    );
    click(&mut a, Action::Modifier(M::Toggle(1, id)));
    assert!(a
        .evaluated_interactive()
        .unwrap()
        .modifier_meshes
        .is_empty());
    assert_eq!(a.editor.project.mesh(10), Some(&original));
    a.action(Action::Undo);
    assert_eq!(a.editor.project.nodes[0].modifiers[0].enabled, true);
}
#[test]
fn render_picking_and_framing_use_the_evaluated_instance_with_optional_edit_preview() {
    let mut a = app();
    let id = add(&mut a, K::Array);
    a.modifier_confirm(&Action::Modifier(M::Field(1, id, 2)), "3")
        .unwrap();
    let eval = a.editor.evaluated().unwrap();
    let frame = frame_preparation::render_project(&a.editor.project, &eval, false);
    assert_ne!(frame.node(1).unwrap().mesh, frame.node(2).unwrap().mesh);
    assert_eq!(a.editor.project.nodes[0].mesh, Some(10));
    let camera = Mat4::orthographic_rh(-5., 5., -3., 3., 0.1, 30.)
        * Mat4::look_at_rh(Vec3::new(0., 0., 10.), Vec3::ZERO, Vec3::Y);
    assert_eq!(
        crate::picking::pick_mesh(
            &a.editor.project,
            &eval,
            camera,
            [0., 0., 1000., 600.],
            [800., 300.]
        ),
        Some(1)
    );
    assert!(a.frame_modifier_selection(&BTreeSet::from([1])));
    assert!((a.target.x - 1.5).abs() < 0.001);
    a.authoring.editing = Some(1);
    assert!(a.evaluated_interactive().unwrap().modifier_meshes.contains_key(&1));
    a.authoring.preview_modifiers=false;
    assert!(!a
        .evaluated_interactive()
        .unwrap()
        .modifier_meshes
        .contains_key(&1));
}
#[test]
fn worker_discards_old_settings_and_never_commits_geometry_to_the_document() {
    let mut a = app();
    let id = add(&mut a, K::Array);
    a.tick_modifiers();
    a.modifier_confirm(&Action::Modifier(M::Field(1, id, 1)), "5")
        .unwrap();
    let before = a.editor.project.clone();
    let started = Instant::now();
    loop {
        a.tick_modifiers();
        if a.evaluated_interactive()
            .unwrap()
            .modifier_meshes
            .get(&1)
            .is_some_and(|m| m.vertices.len() == before.meshes[0].vertices.len() * 5)
        {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 2);
}
#[test]
fn modifier_panels_and_popups_are_reachable_in_both_themes_and_narrow_panels() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            for width in [320., 420.] {
                let mut a = app();
                a.preferences.theme = theme;
                a.scale = scale;
                a.preferences.right_width = Some(width);
                let id = add(&mut a, K::Array);
                let ui = a.build_ui();
                assert!(ui
                    .action_point(&Action::Modifier(M::Field(1, id, 1)))
                    .is_some());
                click(&mut a, Action::DropdownOpen(DropdownKind::ModifierAdd));
                assert!(a.dropdown_is_open(DropdownKind::ModifierAdd));
                a.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
                a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
                assert_eq!(
                    a.editor.project.nodes[0]
                        .modifiers
                        .last()
                        .unwrap()
                        .settings
                        .kind(),
                    K::Bevel
                );
            }
        }
    }
}
#[test]
fn short_inspector_scroll_reaches_apply_and_bakes_the_visible_instance() {
    for scale in [1., 2.] {
        let mut a = app();
        a.scale = scale;
        a.height = 760.;
        a.preferences.right_width = Some(280.);
        add(&mut a, K::Array);
        a.editor.evaluated().unwrap();
        let before = a.editor.project.clone();
        a.inspector_scroll = a.inspector_max_scroll();
        let p = point(&mut a, Action::Modifier(M::Apply(1)), false);
        assert!(ui::contains(a.properties_content_rect(), p));
        a.mouse_move(p);
        a.click_ui_control();
        assert!(a.editor.project.node(1).unwrap().modifiers.is_empty());
        assert_ne!(a.editor.project.node(1).unwrap().mesh, Some(10));
        assert!(a.editor.undo());
        assert_eq!(a.editor.project, before);
    }
}

#[test]
fn export_actual_modifier_panels_and_example_when_requested() {
    let Some(dir) = std::env::var_os("ARTFORGE_QA_MODIFIER_DIR") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, theme) in [("dark", ui::Theme::Dark), ("light", ui::Theme::Light)] {
        let mut a = app();
        a.preferences.theme = theme;
        add(&mut a, K::Subdivision);
        let id = add(&mut a, K::Array);
        a.modifier_confirm(&Action::Modifier(M::Field(1, id, 2)), "3")
            .unwrap();
        a.modifier_confirm(&Action::Modifier(M::Field(1, id, 1)), "3")
            .unwrap();
        for popup in [false, true] {
            if popup {
                click(&mut a, Action::DropdownOpen(DropdownKind::ModifierAdd));
            }
            let ui = a.build_ui();
            let data = serde_json::json!({"width":a.width,"height":a.height,"background":ui.palette().viewport,
                "source":"Actual native UI primitives; CPU preview, not a native GPU screenshot",
                "rectangles":ui.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),
                "texts":ui.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
            std::fs::write(
                dir.join(format!(
                    "modifiers-{name}-{}.json",
                    if popup { "menu" } else { "panel" }
                )),
                serde_json::to_vec(&data).unwrap(),
            )
            .unwrap();
        }
        if name == "dark" {
            poseforge_io::save_project(&a.editor.project, dir.join("modifiers.artforge")).unwrap();
        }
    }
}
