use super::*;
use poseforge_core::{CameraLens, Collection, LightKind, Node};

fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.preferences.left_panel = true;
    a.preferences.right_panel = true;
    a.width = 1180.;
    a.height = 800.;
    a.editor = Editor::new(Project::default());
    a.editor.mode = EditorMode::Scene;
    a.action(Action::Studio(ui::StudioAction::Mesh(0)));
    a.primitive_panel = None;
    a
}
#[test]
fn viewport_selection_reveals_filtered_nested_node_without_changing_document() {
    let mut a = app();
    let selected = a.editor.selected.unwrap();
    let parent = 99;
    a.editor.project.nodes.push(Node::new(parent, "Родитель"));
    a.editor.project.node_mut(selected).unwrap().parent = Some(parent);
    a.editor.project.collections.push(Collection {
        id: 199,
        name: "Папка".into(),
        members: vec![parent],
        ..Default::default()
    });
    for id in 200..260 {
        a.editor
            .project
            .nodes
            .push(Node::new(id, format!("Узел {id}")));
    }
    a.collapsed.extend([parent, 199]);
    a.scene_query = "не найдено".into();
    a.scene_scroll = 80;
    a.scene_tools.hidden_types = 15;
    let before = a.editor.project.clone();
    a.select_object(selected);
    let index = a.scene_node_offset(selected);
    assert!(index >= a.scene_scroll && index < a.scene_scroll + a.scene_capacity());
    assert!(!a.collapsed.contains(&parent) && !a.collapsed.contains(&199));
    assert!(a
        .outliner_rows()
        .iter()
        .any(|r| matches!(r, scene_tree::SceneEntry::Node(n) if n.id==selected)));
    assert_eq!(a.editor.project, before);
}
#[test]
fn material_tab_is_absent_for_camera_and_light_including_keyboard_cycle() {
    let mut a = app();
    let mut camera = Node::new(99, "Камера");
    camera.camera = Some(CameraLens::default());
    a.editor.project.nodes.push(camera);
    for light in [false, true] {
        if light {
            a.select_light(0);
        } else {
            a.action(Action::Select(99));
        }
        a.action(Action::SurfaceTab(1));
        assert_ne!(a.surface.tab, 1);
        for _ in 0..9 {
            a.properties_cycle(1);
            assert_ne!(a.surface.tab, 1);
        }
        let ui = a.build_ui();
        assert!(!ui.hits.iter().any(|h| h.action == Action::SurfaceTab(1)));
    }
    a.clear_light_selection();
    a.editor.selected = None;
    a.set_properties_tab(1);
    assert_eq!(a.surface.tab, 1, "unassigned library remains available");
}
#[test]
fn light_tracking_moves_with_parent_stops_and_deletes_as_one_undoable_change() {
    let mut a = app();
    let node = a.editor.selected.unwrap();
    a.editor.project.lights[0].kind = LightKind::Spot;
    a.editor.project.lights[0].position = Vec3::new(4., 3., 2.);
    a.aim_light_at(0, node, true);
    assert_eq!(a.editor.project.lights[0].target.unwrap().node, node);
    let before = a.editor.project.lights[0].direction;
    a.editor
        .project
        .node_mut(node)
        .unwrap()
        .transform
        .translation = Vec3::new(3., 1., 0.);
    let globals = a.editor.evaluated().unwrap().global;
    let after = a.editor.project.lights[0].evaluated_direction(&globals);
    assert!(before.distance(after) > 0.1);
    a.select_light(0);
    a.action(Action::LightTool(ui::LightTool::StopTracking));
    assert!(a.editor.project.lights[0].target.is_none());
    assert!(a.editor.project.lights[0].direction.distance(after) < 1e-5);
    a.action(Action::Undo);
    assert!(a.editor.project.lights[0].target.is_some());
    a.editor.delete_node(node).unwrap();
    assert!(a.editor.project.lights[0].target.is_none());
    assert!(a.editor.project.lights[0].direction.distance(after) < 1e-5);
    a.action(Action::Undo);
    assert!(a.editor.project.lights[0].target.is_some());
    a.aim_light_at(0, node, false);
    assert!(a.editor.project.lights[0].target.is_none());
}
#[test]
fn demo_and_light_inspector_have_no_removed_controls() {
    let demo = Project::demo();
    assert!(!demo.nodes.iter().any(|n| n.name == "Платформа"));
    let mut a = app();
    a.select_light(0);
    for scroll in [0., 300., 650., 1000.] {
        a.inspector_scroll = scroll;
        let ui = a.build_ui();
        assert!(!ui.texts.iter().any(|t| matches!(
            t.text.as_str(),
            "TXT"
                | "Список костей"
                | "Добавить источник"
                | "Направить в центр вида"
                | "Расположить перед камерой"
        )));
        assert!(!ui
            .hits
            .iter()
            .any(|h| h.action == Action::SurfaceDeleteLight));
    }
}

#[test]
fn tracked_direction_handle_noop_keeps_binding_fallback_and_undo() {
    let mut a = app();
    let node = a.editor.selected.unwrap();
    a.frame_all();
    a.editor.project.lights[0].kind = LightKind::Spot;
    a.editor.project.lights[0].position = light_controls::placement_in_view(&a.view()).0;
    a.aim_light_at(0, node, true);
    a.editor
        .project
        .node_mut(node)
        .unwrap()
        .transform
        .translation
        .x += 0.3;
    a.select_light(0);
    let before = a.editor.project.clone();
    let undo = a.editor.undo_len();
    let point = a
        .light_shape_handles()
        .into_iter()
        .find(|(grip, _)| *grip == light_shape_controls::LightGrip::Direction)
        .unwrap()
        .1;
    a.mouse = point;
    assert!(a.light_shape_pointer_down());
    let Some(Drag::LightShape(drag)) = a.drag.take() else {
        panic!("direction drag")
    };
    a.update_light_shape_drag(drag, point);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), undo);
}
