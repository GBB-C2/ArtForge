//! End-to-end outliner action routing. No native window is required.
use super::*;
use poseforge_core::{CameraLens, Light, LightKind, Node, MAX_ENABLED_LIGHTS};

fn tree_app() -> App {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.preferences.left_panel = true;
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    let mut camera = Node::new(3, "Главный ракурс");
    camera.camera = Some(CameraLens::default());
    let mut joint = Node::new(2, "Кисть");
    joint.is_joint = true;
    joint.parent = Some(1);
    app.editor = Editor::new(Project {
        nodes: vec![Node::new(1, "Модель"), joint, camera],
        lights: vec![
            Light {
                name: "Рисующий".into(),
                kind: LightKind::Directional,
                ..Light::default()
            },
            Light {
                name: "Блик".into(),
                kind: LightKind::Area,
                ..Light::default()
            },
            Light {
                name: "Контур".into(),
                kind: LightKind::Spot,
                enabled: false,
                ..Light::default()
            },
        ],
        ..Project::default()
    });
    app.editor.selected = Some(1);
    app.editor.mode = EditorMode::Scene;
    app.reset_workflow_selection();
    app.saved_snapshot = app.editor.project.clone();
    app.dirty = false;
    app
}

fn light_action(app: &App, index: usize, toggle: bool) -> Action {
    let light = Box::new(app.editor.project.lights[index].clone());
    if toggle {
        Action::SceneToggleLight(index, light)
    } else {
        Action::SceneSelectLight(index, light)
    }
}

fn click(app: &mut App, action: Action) {
    app.ui = app.build_ui();
    let rect = app
        .ui
        .hits
        .iter()
        .rev()
        .find(|hit| hit.action == action)
        .expect("Visible outliner control")
        .rect;
    let point = [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5];
    assert_eq!(app.ui.control_at(point), Some(action));
    app.mouse_move(point);
    app.click_ui_control();
}

#[test]
fn scene_lights_are_visible_selectable_and_open_the_matching_inspector() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 1.5, 2.] {
            let mut app = tree_app();
            app.preferences.theme = theme;
            app.scale = scale;
            let original = app.editor.project.clone();
            app.ui = app.build_ui();
            let selection_hits = app
                .ui
                .hits
                .iter()
                .filter(|hit| matches!(hit.action, Action::SceneSelectLight(..)))
                .count();
            assert_eq!(
                selection_hits, 3,
                "Lights must be visible without scrolling a character hierarchy"
            );
            assert!(app
                .ui
                .hits
                .iter()
                .any(|hit| hit.action == Action::CreateCamera));
            let select = light_action(&app, 1, false);
            click(&mut app, select);
            assert_eq!(app.active_light_index(), Some(1));
            assert_eq!(app.editor.selected, None);
            assert_eq!(app.surface.tab, 0);
            assert!(app.preferences.right_panel);
            assert_eq!(app.editor.project, original);
            assert_eq!(app.editor.undo_len(), 0);
            click(&mut app, Action::Select(1));
            assert_eq!(app.editor.selected, Some(1));
            assert_eq!(app.active_light_index(), None);
        }
    }
}

#[test]
fn light_eye_is_independent_of_selection_and_undoable_and_saved() {
    let mut app = tree_app();
    let original = app.editor.project.clone();
    let toggle = light_action(&app, 1, true);
    click(&mut app, toggle);
    assert!(!app.editor.project.lights[1].enabled);
    assert_eq!(app.editor.selected, Some(1));
    assert_eq!(app.active_light_index(), None);
    assert_eq!(app.editor.project.nodes, original.nodes);
    assert_eq!(app.editor.undo_len(), 1);
    let reopened: Project =
        serde_json::from_str(&serde_json::to_string(&app.editor.project).unwrap()).unwrap();
    reopened.validate().unwrap();
    assert!(!reopened.lights[1].enabled);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
    app.action(Action::Redo);
    assert!(!app.editor.project.lights[1].enabled);
}

#[test]
fn duplicate_delete_and_history_cannot_target_a_previously_selected_node_or_light() {
    let mut app = tree_app();
    let original = app.editor.project.clone();
    let select = light_action(&app, 1, false);
    app.action(select);
    app.action(Action::Duplicate);
    assert_eq!(app.editor.project.lights.len(), original.lights.len() + 1);
    assert_eq!(app.active_light_index(), Some(3));
    assert_eq!(app.editor.project.nodes, original.nodes);
    assert_eq!(app.editor.project.lights[3].kind, LightKind::Area);
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::ToggleVisible);
    assert!(!app.editor.project.lights[3].enabled);
    app.action(Action::Delete);
    assert_eq!(app.editor.project.lights, original.lights);
    assert_eq!(app.editor.project.nodes, original.nodes);
    app.action(Action::Undo);
    assert_eq!(app.editor.project.lights.len(), 4);
    assert_eq!(app.active_light_index(), None);
    app.action(Action::Delete);
    assert_eq!(
        app.editor.project.lights.len(),
        4,
        "History clears ambiguous array-index selection"
    );
    assert_eq!(app.editor.project.nodes, original.nodes);
}

#[test]
fn stale_light_hits_are_rejected_after_list_replacement_or_reorder() {
    let mut app = tree_app();
    let select = light_action(&app, 0, false);
    let toggle = light_action(&app, 0, true);
    app.editor.project.lights.swap(0, 1);
    let reordered = app.editor.project.clone();
    app.action(select);
    app.action(toggle);
    assert_eq!(app.editor.project, reordered);
    assert_eq!(app.editor.selected, Some(1));
    assert_eq!(app.active_light_index(), None);
    assert_eq!(app.editor.undo_len(), 0);
    assert!(app.status.contains("обновился"));
    let select = light_action(&app, 1, false);
    app.action(select);
    app.action(Action::LightingPreset(0));
    assert_eq!(app.active_light_index(), None);
}

#[test]
fn outliner_search_types_bone_filter_and_scroll_cover_all_scene_entries() {
    let mut app = tree_app();
    assert_eq!(app.outliner_rows().len(), 6);
    app.scene_query = "ПЛОЩАДНОЙ".into();
    assert!(matches!(
        app.outliner_rows().as_slice(),
        [scene_tree::SceneEntry::Light(1)]
    ));
    app.scene_query = "контур".into();
    assert!(matches!(
        app.outliner_rows().as_slice(),
        [scene_tree::SceneEntry::Light(2)]
    ));
    app.scene_query = "камера".into();
    assert!(
        matches!(app.outliner_rows().as_slice(), [scene_tree::SceneEntry::Node(row)] if row.id == 3)
    );
    app.scene_query.clear();
    app.joints_only = true;
    assert!(app
        .outliner_rows()
        .iter()
        .all(|entry| matches!(entry, scene_tree::SceneEntry::Node(_))));
    app.joints_only = false;
    for i in 0..30 {
        app.editor.project.lights.push(Light {
            name: format!("Фон {i}"),
            enabled: false,
            ..Light::default()
        });
    }
    app.mouse = [40., 250.];
    app.scroll(-100.);
    assert_eq!(
        app.scene_scroll,
        app.outliner_rows().len() - app.scene_capacity()
    );
    app.ui = app.build_ui();
    assert!(
        app.ui
            .hits
            .iter()
            .any(|hit| hit.action == Action::Select(3)),
        "Last camera row remains reachable after light rows"
    );
    app.reveal_scene_light(0);
    assert_eq!(app.scene_scroll, 0);
}

#[test]
fn duplicating_at_the_light_budget_creates_a_valid_disabled_copy() {
    let mut app = tree_app();
    app.editor.project.lights = (0..MAX_ENABLED_LIGHTS)
        .map(|i| Light {
            name: format!("Источник {i}"),
            ..Light::default()
        })
        .collect();
    let select = light_action(&app, 0, false);
    app.action(select);
    app.action(Action::Duplicate);
    assert_eq!(app.editor.project.lights.len(), MAX_ENABLED_LIGHTS + 1);
    assert!(!app.editor.project.lights.last().unwrap().enabled);
    app.editor.project.validate().unwrap();
    let before = app.editor.project.clone();
    app.action(Action::ToggleVisible);
    assert_eq!(
        app.editor.project, before,
        "Enabling a ninth light must roll back atomically"
    );
    assert_eq!(app.editor.undo_len(), 1);
}

#[test]
fn all_scene_node_kinds_have_borderless_independent_visibility_controls() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = tree_app();
            a.preferences.theme = theme;
            a.scale = scale;
            for id in [1, 2, 3] {
                let before = a.editor.project.clone();
                let selection = a.editor.selected;
                click(&mut a, Action::SceneToggleNode(id));
                assert!(!a.editor.project.node(id).unwrap().visible);
                assert_eq!(a.editor.selected, selection);
                a.action(Action::Undo);
                assert_eq!(a.editor.project, before);
            }
        }
    }
}
