use super::*;
use poseforge_core::{
    CameraLens, Collection, Light, Material, Node, PrimitiveKind, PrimitiveSettings, SceneItem,
};
use scene_organization::Destination;
use scene_tree::SceneEntry;

fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.width = 1180.;
    a.height = 800.;
    a.preferences.left_panel = true;
    a.preferences.right_panel = true;
    let mut one = Node::new(1, "Куб");
    one.mesh = Some(10);
    let mut two = Node::new(2, "Второй куб");
    two.mesh = Some(11);
    let mut bone = Node::new(3, "Кость");
    bone.is_joint = true;
    bone.parent = Some(1);
    let mut camera = Node::new(4, "Камера");
    camera.camera = Some(CameraLens::default());
    a.editor = Editor::new(Project {
        nodes: vec![one, two, bone, camera],
        meshes: vec![
            PrimitiveSettings::new(PrimitiveKind::Cube)
                .mesh(10, Some(20))
                .unwrap(),
            PrimitiveSettings::new(PrimitiveKind::Cube)
                .mesh(11, Some(20))
                .unwrap(),
        ],
        materials: vec![
            Material {
                id: 20,
                name: "Основной".into(),
                ..Default::default()
            },
            Material {
                id: 21,
                name: "Акцент".into(),
                ..Default::default()
            },
            Material {
                id: 22,
                name: "Стекло".into(),
                ..Default::default()
            },
        ],
        lights: vec![
            Light::default(),
            Light {
                name: "Второй свет".into(),
                ..Default::default()
            },
        ],
        collections: vec![Collection {
            id: 100,
            name: "Папка".into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    a.editor.mode = EditorMode::Scene;
    a.editor.selected = Some(1);
    a.reset_workflow_selection();
    a.saved_snapshot = a.editor.project.clone();
    a.dirty = false;
    a
}
fn point(a: &mut App, action: Action) -> [f32; 2] {
    a.ui = a.build_ui();
    let r =
        a.ui.hits
            .iter()
            .rev()
            .find(|h| h.action == action)
            .unwrap_or_else(|| panic!("Missing hit: {action:?}"))
            .rect;
    [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]
}
fn click(a: &mut App, action: Action) {
    let p = point(a, action);
    a.mouse_move(p);
    a.click_ui_control();
}
fn inside(folder: Option<u64>) -> Destination {
    Destination {
        anchor: None,
        folder,
        after: true,
    }
}

#[test]
fn filter_popup_is_multi_select_keyboard_operable_and_does_not_edit_project() {
    let mut a = app();
    let before = a.editor.project.clone();
    click(&mut a, Action::DropdownOpen(ui::DropdownKind::SceneFilter));
    for i in [1, 2, 3] {
        a.action(Action::DropdownChoose(i));
        assert!(a.dropdown_is_open(ui::DropdownKind::SceneFilter));
    }
    assert!(a.outliner_rows().iter().all(|r| !matches!(
        r,
        SceneEntry::Light(_) | SceneEntry::Node(SceneRow { id: 3 | 4, .. })
    )));
    a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
    assert!(a.dropdown.is_none());
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
    click(&mut a, Action::DropdownOpen(ui::DropdownKind::SceneFilter));
    a.dropdown_key(&Key::Named(NamedKey::Home), false, false, false);
    a.dropdown_key(&Key::Named(NamedKey::Space), false, false, false);
    assert_eq!(a.scene_tools.hidden_types, 15);
}

#[test]
fn closed_filter_consumes_outside_click_before_delete_underneath() {
    let mut a = app();
    let delete = point(&mut a, Action::SceneDelete(SceneItem::Node(4)));
    click(&mut a, Action::DropdownOpen(ui::DropdownKind::SceneFilter));
    a.mouse_move(delete);
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert!(a.dropdown.is_none());
    assert!(a.editor.project.node(4).is_some());
}

#[test]
fn row_controls_are_separate_in_both_themes_and_dpi_without_visibility_checkbox() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app();
            a.preferences.theme = theme;
            a.scale = scale;
            let u = a.build_ui();
            assert!(!u.hits.iter().any(|h| h.action == Action::ToggleVisible));
            for id in [1, 2, 3, 4] {
                let eye = u
                    .hits
                    .iter()
                    .find(|h| h.action == Action::SceneToggleNode(id))
                    .unwrap()
                    .rect;
                let close = u
                    .hits
                    .iter()
                    .find(|h| h.action == Action::SceneDelete(SceneItem::Node(id)))
                    .unwrap()
                    .rect;
                assert!(eye[0] + eye[2] <= close[0]);
            }
            assert!(u
                .hits
                .iter()
                .any(|h| h.action == Action::SceneTool(ui::SceneTool::NewCollection)));
        }
    }
}

#[test]
fn row_delete_preserves_unrelated_selection_and_undo_restores_branch() {
    let mut a = app();
    let before = a.editor.project.clone();
    click(&mut a, Action::SceneDelete(SceneItem::Node(2)));
    assert_eq!(a.editor.selected, Some(1));
    assert!(a.editor.project.node(2).is_none());
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.selected, Some(1));
    click(&mut a, Action::SceneDelete(SceneItem::Node(1)));
    assert!(a.editor.project.node(1).is_none() && a.editor.project.node(3).is_none());
    assert!(a.editor.selected.is_none());
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}

#[test]
fn deleting_stale_light_row_cannot_delete_replacement() {
    let mut a = app();
    let action = Action::SceneDeleteLight(0, Box::new(a.editor.project.lights[0].clone()));
    a.editor.project.lights.swap(0, 1);
    let before = a.editor.project.clone();
    a.action(action);
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn node_and_light_folder_moves_are_atomic_and_folder_delete_ungroups() {
    let mut a = app();
    let before = a.editor.project.clone();
    a.move_scene_items(
        &[SceneItem::Node(1), SceneItem::Light(1)],
        inside(Some(100)),
    );
    assert_eq!(a.editor.project.node_collection(3), Some(100));
    assert_eq!(a.editor.project.light_collection(1), Some(100));
    assert_eq!(a.editor.project.node(3).unwrap().parent, Some(1));
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::Redo);
    click(
        &mut a,
        Action::SceneTool(ui::SceneTool::CollectionVisible(100)),
    );
    assert!(!a.editor.project.light_visible(1, false));
    assert!(a.editor.project.light_visible(1, true));
    click(&mut a, Action::SceneDelete(SceneItem::Folder(100)));
    assert_eq!(a.editor.project.nodes, before.nodes);
    assert_eq!(a.editor.project.lights, before.lights);
    assert!(a.editor.project.node_collection(1).is_none());
    assert!(a.editor.project.light_visible(1, false));
}

#[test]
fn folder_creation_button_creates_names_and_groups_selected_light() {
    let mut a = app();
    a.select_light(1);
    click(&mut a, Action::SceneTool(ui::SceneTool::NewCollection));
    let id = a.scene_tools.collection.unwrap();
    assert_eq!(a.editor.project.light_collection(1), Some(id));
    assert!(a.input.is_some());
    a.action(Action::Cancel);
    a.action(Action::Undo);
    assert_eq!(a.editor.project.collections.len(), 1);
}

#[test]
fn node_drag_reorders_on_release_and_escape_cancels() {
    let mut a = app();
    let before = a.editor.project.clone();
    let from = point(&mut a, Action::Select(2));
    let mut to = point(&mut a, Action::Select(1));
    to[1] -= 8.;
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move(to);
    assert_eq!(a.editor.project, before);
    a.action(Action::Cancel);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move(to);
    a.mouse_up(MouseButton::Left);
    assert!(a.scene_node_offset(2) < a.scene_node_offset(1));
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}

#[test]
fn drag_outside_and_locked_destination_leave_project_unchanged() {
    let mut a = app();
    a.editor.project.collections[0].locked = true;
    let before = a.editor.project.clone();
    a.move_scene_items(&[SceneItem::Node(1)], inside(Some(100)));
    assert_eq!(a.editor.project, before);
    let from = point(&mut a, Action::Select(2));
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move([900., 400.]);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project, before);
}

#[test]
fn material_rows_reorder_without_changing_polygon_assignments() {
    let mut a = app();
    a.editor
        .assign_face_material(10, &[1], &BTreeSet::from([0, 2]), 21)
        .unwrap();
    // Parent meshes initially expose their children and their material rows.
    let before = a.editor.project.clone();
    let undo = a.editor.undo_len();
    let from = point(&mut a, Action::SceneMaterial(1, 21));
    let mut to = point(&mut a, Action::SceneMaterial(1, 20));
    to[1] -= 7.;
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move(to);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.scene_material_ids(1), vec![21, 20]);
    assert_eq!(a.editor.project.meshes, before.meshes);
    assert_eq!(a.editor.undo_len(), undo + 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}

#[test]
fn material_drag_between_primitives_assigns_target_and_keeps_source() {
    let mut a = app();
    a.editor
        .assign_face_material(10, &[1], &BTreeSet::from([0]), 21)
        .unwrap();
    let source = a.editor.project.mesh(10).unwrap().clone();
    let undo = a.editor.undo_len();
    let from = point(&mut a, Action::SceneMaterial(1, 21));
    let to = point(&mut a, Action::Select(2));
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move(to);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project.mesh(11).unwrap().material, Some(21));
    assert_eq!(a.editor.project.mesh(10), Some(&source));
    assert_eq!(a.editor.undo_len(), undo + 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project.mesh(11).unwrap().material, Some(20));
}

#[test]
fn expanded_material_click_selects_assigned_material_without_reassigning() {
    let mut a = app();
    a.editor
        .assign_face_material(10, &[1], &BTreeSet::from([0]), 21)
        .unwrap();
    let before = a.editor.project.clone();
    click(&mut a, Action::SceneMaterial(1, 21));
    assert_eq!(a.editor.selected, Some(1));
    assert_eq!(a.selected_material_id(), Some(21));
    assert!(!a.surface.material_library);
    assert_eq!(a.editor.project, before);
}

#[test]
fn saved_order_and_folder_membership_round_trip() {
    let mut a = app();
    a.move_scene_items(
        &[SceneItem::Node(2), SceneItem::Light(1)],
        inside(Some(100)),
    );
    let restored: Project =
        serde_json::from_slice(&serde_json::to_vec(&a.editor.project).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, a.editor.project);
    let rows: Vec<_> = a.outliner_rows().iter().map(SceneEntry::item).collect();
    a.editor = Editor::new(restored);
    assert_eq!(
        rows,
        a.outliner_rows()
            .iter()
            .map(SceneEntry::item)
            .collect::<Vec<_>>()
    );
}

#[test]
fn edge_autoscroll_waits_before_scrolling_and_resets_after_leaving_edge() {
    let mut a = app();
    for id in 200..220 {
        a.editor
            .project
            .nodes
            .push(Node::new(id, format!("Extra {id}")));
    }
    let from = point(&mut a, Action::Select(1));
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    let r = a.scene_list_rect();
    a.mouse_move([r[0] + 80., r[1] + r[3] - 4.]);
    assert_eq!(a.scene_scroll, 0);
    a.scene_tools.edge_scroll_since = Some(Instant::now() - Duration::from_millis(300));
    a.scroll_scene_drag();
    assert_eq!(a.scene_scroll, 1);
    a.mouse_move([r[0] + 80., r[1] + 80.]);
    assert!(a.scene_tools.edge_scroll_since.is_none());
    a.action(Action::Cancel);
    assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn folders_nest_from_scene_drag_and_cannot_form_a_cycle() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {
        let mut a=app();a.preferences.theme=theme;
        a.editor.project.collections.push(Collection{id:101,name:"Inner".into(),members:vec![2],light_members:vec![0],..Default::default()});
        let before=a.editor.project.clone();let undo=a.editor.undo_len();
        let source=point(&mut a,Action::SceneTool(ui::SceneTool::CollectionSelect(101)));
        let target=point(&mut a,Action::SceneTool(ui::SceneTool::CollectionSelect(100)));
        a.mouse_move(source);a.mouse_down(MouseButton::Left);a.mouse_move(target);a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.project.collection_chain(101),vec![101,100]);assert_eq!(a.editor.undo_len(),undo+1);
        let rows=a.outliner_rows();let outer=rows.iter().position(|r|matches!(r,SceneEntry::Collection(100))).unwrap();
        let inner=rows.iter().position(|r|matches!(r,SceneEntry::Collection(101))).unwrap();assert!(inner>outer);
        a.collapsed.insert(100);assert!(!a.outliner_rows().iter().any(|r|matches!(r,SceneEntry::Collection(101))));
        a.scene_query="Inner".into();assert!(a.outliner_rows().iter().any(|r|matches!(r,SceneEntry::Collection(100))));a.scene_query.clear();
        let nested=a.editor.project.clone();a.move_scene_items(&[SceneItem::Folder(100)],inside(Some(101)));assert_eq!(a.editor.project,nested);
        a.action(Action::Undo);assert_eq!(a.editor.project,before);
        a.move_scene_items(&[SceneItem::Folder(101)],inside(Some(100)));
        a.editor.project.collections[0].locked=true;let locked=a.editor.project.clone();
        a.move_scene_items(&[SceneItem::Folder(101)],inside(None));assert_eq!(a.editor.project,locked);
    }
}

#[test]
fn folder_menu_moves_the_folder_and_object_selection_moves_only_the_object() {
    let mut a=app();
    a.editor.project.collections.push(Collection{id:101,name:"Inner".into(),members:vec![2],light_members:vec![0],..Default::default()});
    let before=a.editor.project.clone();
    click(&mut a,Action::SceneTool(ui::SceneTool::CollectionSelect(101)));
    assert_eq!(a.editor.selected,Some(2));
    a.action(Action::SceneTool(ui::SceneTool::AssignCollection(Some(100))));
    assert_eq!(a.editor.project.collection_chain(101),vec![101,100]);
    assert_eq!(a.editor.project.node_collection(2),Some(101));
    assert_eq!(a.editor.project.light_collection(0),Some(101));
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
    a.action(Action::Select(2));
    a.action(Action::SceneTool(ui::SceneTool::AssignCollection(Some(100))));
    assert_eq!(a.editor.project.collection_chain(101),vec![101]);
    assert_eq!(a.editor.project.node_collection(2),Some(100));
    assert_eq!(a.editor.project.light_collection(0),Some(101));
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
}

#[test]
fn removing_the_last_material_does_not_add_a_default_outliner_row() {
    let mut a=app();a.scene_tools.material_expanded.insert(1);
    click(&mut a,Action::SceneMaterialDetach(1,20));
    assert!(a.editor.project.mesh(10).unwrap().material.is_none());
    assert!(!a.outliner_rows().iter().any(|row|matches!(row,SceneEntry::Material(1,_,_))));
    a.action(Action::Undo);assert!(a.outliner_rows().iter().any(|row|matches!(row,SceneEntry::Material(1,Some(20),_))));
}
