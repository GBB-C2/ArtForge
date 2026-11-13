//! Floating workspace and camera-bound navigation interaction contracts.
use super::*;
use poseforge_core::{box_mesh, Material, Node};

fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.editor = Editor::new(Project::default());
    a.editor.mode = EditorMode::Scene;
    a.width = 1100.;
    a.height = 760.;
    a
}
fn camera_app() -> (App, u64) {
    let mut a = app();
    a.action(Action::CreateCamera);
    let id = a.editor.selected.unwrap();
    a.action(Action::CameraView);
    a.action(Action::Studio(ui::StudioAction::CameraLock));
    a.ui = a.build_ui();
    a.mouse = [520., 400.];
    (a, id)
}
fn hit(a: &mut App, action: Action) -> [f32; 4] {
    a.ui = a.build_ui();
    a.ui.hits
        .iter()
        .rev()
        .find(|h| h.action == action)
        .unwrap_or_else(||panic!("Missing {action:?}"))
        .rect
}
fn click(a: &mut App, action: Action) {
    let r = hit(a, action);
    a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
    a.click_ui_control();
}
fn material_app() -> App {
    let mut a = app();
    for id in 1..21 {
        a.editor.project.materials.push(Material {
            id,
            name: format!("Материал {id:02}"),
            in_library: true,
            ..Material::default()
        });
    }
    a.editor
        .project
        .meshes
        .push(box_mesh(21, "Меш", Vec3::ONE, Some(1)));
    for id in [22, 23] {
        let mut n = Node::new(id, "Объект");
        n.mesh = Some(21);
        a.editor.project.nodes.push(n);
    }
    a.editor.selected = Some(22);
    a.preferences.right_panel = true;
    a.surface.tab = 1;
    a
}

#[test]
fn panels_overlay_a_stable_viewport_and_collapse_independently() {
    let mut a = app();
    let viewport = a.viewport();
    let before = a.editor.project.clone();
    for scale in [1., 2.] {
        a.scale = scale;
        for objects in [false, true] {
            for tools in [false, true] {
                for properties in [false, true] {
                    a.preferences.left_panel = objects;
                    a.preferences.tools_expanded = tools;
                    a.preferences.right_panel = properties;
                    assert_eq!(a.viewport(), viewport);
                    let u = a.build_ui();
                    assert!(u.hits.iter().any(|h| h.action == Action::ToggleLeftPanel));
                    assert_eq!(
                        u.hits.iter().any(|h| h.action == Action::ToggleTools),
                        false
                    );
                    assert_eq!(
                        u.hits.iter().any(|h| h.action == Action::Move),
                        objects
                    );
                    assert_eq!(
                        u.hits.iter().any(|h| h.action == Action::SearchScene),
                        objects
                    );
                    if objects { assert!(a.rail_rect()[0] >= a.left_panel_rect()[0] + a.left_panel_rect()[2] + 8.); }
                    else { assert_eq!(a.rail_rect()[2], 0.); }
                    a.ui = u;
                    let r = a.left_panel_rect();
                    assert_eq!(a.canvas_input_at([r[0] + 2., r[1] + 2.]), !objects);
                }
            }
        }
    }
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
    a.menu = Some(crate::chrome::Menu::Project);
    assert!(!a
        .build_ui()
        .texts
        .iter()
        .any(|t| t.text.contains("Тема") || t.text.contains("Ctrl+T")));
}

#[test]
fn expanded_animation_dock_does_not_cover_object_rows_or_tools() {
    let mut a = app();
    a.editor.mode = EditorMode::Animation;
    a.timeline.expanded = true;
    a.tool = Tool::Move;
    a.preferences.right_panel = true;
    for scale in [1., 2.] {
        a.scale = scale;
        let ui = a.build_ui();
        let dock = a.animation_dock_rect();
        let rail = a.rail_rect();
        assert!(rail[1] + rail[3] <= dock[1] - 12.);
        for action in [
            Action::Move,
            Action::Rotate,
            Action::Scale,
            Action::ToggleGizmoSpace,
        ] {
            let r = ui.hits.iter().find(|h| h.action == action).unwrap().rect;
            assert!(r[1] + r[3] <= rail[1] + rail[3]);
        }
    }
}

#[test]
fn six_axis_widget_has_dpi_independent_targets_and_cancel_restores_view() {
    let mut a = app();
    a.preferences.right_panel = true;
    for scale in [1., 2.] {
        a.scale = scale;
        let u = a.build_ui();
        for axis in 0..6 {
            let h = u
                .hits
                .iter()
                .find(|h| h.action == Action::ViewAxis(axis))
                .unwrap();
            assert!(ui::contains(
                a.view_widget_rect(),
                [h.rect[0] + 12., h.rect[1] + 12.]
            ));
            a.action(Action::ViewAxis(axis));
            let direction = (a.view().eye - a.view().target).normalize();
            let expected = [
                Vec3::X,
                Vec3::NEG_X,
                Vec3::Y,
                Vec3::NEG_Y,
                Vec3::Z,
                Vec3::NEG_Z,
            ][axis as usize];
            assert!(direction.dot(expected) > 0.999);
            assert!(a.view().orthographic.is_none());
        }
        let original = (a.yaw, a.pitch, a.target);
        a.mouse = [600., 100.];
        a.action(Action::ViewOrbit);
        a.mouse_move([640., 120.]);
        assert_ne!(a.yaw, original.0);
        a.action(Action::Cancel);
        assert_eq!((a.yaw, a.pitch, a.target), original);
        assert_eq!(a.editor.undo_len(), 0);
    }
}

#[test]
fn camera_look_pan_and_dolly_commit_atomic_edits_and_undo() {
    let (mut a, id) = camera_app();
    let before = a.editor.project.clone();
    let count = a.editor.undo_len();
    let eye = a.view().eye;
    a.mouse_down(MouseButton::Right);
    assert!(matches!(a.drag, Some(Drag::CameraNavigate(_))));
    for p in [[535., 405.], [555., 410.], [570., 420.]] {
        a.mouse_move(p);
    }
    assert!(
        a.view().eye.distance(eye) < 1e-4,
        "looking turns about the camera eye"
    );
    a.mouse_up(MouseButton::Right);
    assert_eq!(a.editor.undo_len(), count + 1);
    assert_eq!(a.camera_view, Some(id));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.mouse_down(MouseButton::Middle);
    a.mouse_move([620., 450.]);
    assert!(a.view().eye.distance(eye) > 0.01);
    a.mouse_up(MouseButton::Middle);
    assert_eq!(a.editor.undo_len(), count + 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.scroll(1.);
    assert_eq!(a.camera_view, Some(id));
    assert!(a.view().eye.distance(eye) > 0.01);
    assert_eq!(a.editor.undo_len(), count + 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}

#[test]
fn camera_cancel_locks_and_rotated_parent_keep_world_pose_safe() {
    let (mut a, id) = camera_app();
    let mut parent = Node::new(100, "Родитель");
    parent.transform.translation = Vec3::new(2., 1., -3.);
    parent.transform.rotation = glam::Quat::from_rotation_y(0.7);
    parent.transform.scale = Vec3::splat(2.);
    a.editor.project.nodes.push(parent);
    a.editor.project.node_mut(id).unwrap().parent = Some(100);
    let before = a.editor.project.clone();
    let eye = a.view().eye;
    a.mouse_down(MouseButton::Right);
    a.mouse_move([560., 430.]);
    assert!(a.view().eye.distance(eye) < 1e-4);
    a.cancel_drag();
    assert_eq!(a.editor.project, before);
    for blocked_parent in [false, true] {
        a.editor
            .project
            .node_mut(if blocked_parent { 100 } else { id })
            .unwrap()
            .locked = true;
        let locked = a.editor.project.clone();
        a.scroll(2.);
        a.mouse_down(MouseButton::Middle);
        assert!(a.drag.is_none());
        a.action(Action::ViewAxis(0));
        assert_eq!(a.editor.project, locked);
        a.editor
            .project
            .node_mut(if blocked_parent { 100 } else { id })
            .unwrap()
            .locked = false;
    }
    a.editor.project.node_mut(100).unwrap().transform.scale = Vec3::new(1., 2., 1.);
    let nonuniform = a.editor.project.clone();
    a.scroll(2.);
    assert_eq!(a.editor.project, nonuniform);
}

#[test]
fn locked_camera_projection_and_widget_work_without_camera_selection() {
    let (mut a, id) = camera_app();
    a.editor.selected = None;
    let before = a.editor.project.clone();
    a.action(Action::Perspective);
    assert!(a.view().orthographic.is_some());
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::ViewAxis(1));
    assert!(
        (a.view().eye - a.view().target)
            .normalize()
            .dot(Vec3::NEG_X)
            > 0.999
    );
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.camera_navigation_lock = false;
    let view = a.view();
    a.action(Action::ViewOrbit);
    a.mouse_move([550., 420.]);
    a.action(Action::Cancel);
    assert_eq!(a.camera_view, Some(id));
    assert!(a.view().eye.distance(view.eye) < 1e-5);
}

#[test]
fn material_library_popup_search_scroll_cancel_and_assignment_use_existing_history() {
    let mut a = material_app();
    let before = a.editor.project.clone();
    click(&mut a, Action::DropdownOpen(ui::DropdownKind::MaterialLibrary));
    a.input.as_mut().unwrap().insert("Материал"); a.ui = a.build_ui();
    let r = hit(&mut a, Action::ScrollPanel(2));
    a.mouse = [r[0] + 6., r[1] + 8.]; a.scroll(-3.);
    assert!(a.material_popup_visible_ids().contains(&12));
    assert_eq!(a.inspector_scroll, 0.);
    a.mouse_move([r[0] + 6., r[1] + 10.]); a.mouse_down(MouseButton::Left);
    a.mouse_move([r[0] + 6., r[1] + r[3] + 20.]); a.mouse_up(MouseButton::Left);
    assert!(a.material_popup_visible_ids().contains(&20));
    a.input.as_mut().unwrap().select_all(); a.input.as_mut().unwrap().insert("МАТЕРИАЛ 12");
    a.ui = a.build_ui();
    assert_eq!(a.material_popup_visible_ids(), vec![12]);
    a.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
    assert!(a.input.is_none() && a.dropdown.is_none());
    assert_eq!(a.editor.project, before); assert_eq!(a.editor.undo_len(), 0);
    click(&mut a, Action::DropdownOpen(ui::DropdownKind::MaterialLibrary));
    a.input.as_mut().unwrap().insert("материал 12"); a.ui = a.build_ui();
    a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
    let id = a.selected_material_id().unwrap(); assert_ne!(id,12);
    assert!(a.material_list_ids().contains(&id));
    assert_eq!(a.editor.project.meshes,before.meshes); assert_eq!(a.editor.undo_len(),1);
    a.action(Action::Undo); assert_eq!(a.editor.project,before);
}

#[test]
fn stale_material_rows_do_not_rebind_a_new_selection() {
    let mut a = material_app();
    let stale = Action::MaterialAssign(3, 21, Some(22));
    a.editor.selected = Some(23);
    let before = a.editor.project.clone();
    a.action(stale);
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn camera_animation_noop_cancel_staging_and_autokey_use_existing_history() {
    for auto_key in [false, true] {
        let (mut a, id) = camera_app();
        a.editor.mode = EditorMode::Animation;
        a.editor.auto_key = auto_key;
        a.ui = a.build_ui();
        a.mouse = [520., 300.];
        let before = a.editor.project.clone();
        let history = a.editor.undo_len();
        a.mouse_down(MouseButton::Right);
        a.mouse_move([550., 310.]);
        a.mouse_move([520., 300.]);
        a.mouse_up(MouseButton::Right);
        assert_eq!(a.editor.project, before);
        assert!(!a.editor.has_staged());
        assert_eq!(a.editor.undo_len(), history);
        a.mouse_down(MouseButton::Middle);
        a.mouse_move([560., 310.]);
        a.mouse_up(MouseButton::Middle);
        assert_eq!(a.editor.undo_len(), history + 1);
        assert_eq!(a.camera_view, Some(id));
        if auto_key {
            assert_ne!(a.editor.project, before);
            assert!(!a.editor.has_staged());
        } else {
            assert_eq!(a.editor.project, before);
            assert!(a.editor.has_staged());
        }
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
        assert!(!a.editor.has_staged());
    }
}

#[test]
fn object_scrollbar_reaches_last_object_without_camera_or_document_changes() {
    let mut a = app();
    for id in 1..60 {
        a.editor
            .project
            .nodes
            .push(Node::new(id, format!("Object {id}")));
    }
    let before = a.editor.project.clone();
    let view = a.view();
    let r = hit(&mut a, Action::ObjectScroll);
    a.mouse_move([r[0] + 6., r[1] + 6.]);
    a.mouse_down(MouseButton::Left);
    a.mouse_move([r[0] + 6., r[1] + r[3] + 100.]);
    a.mouse_up(MouseButton::Left);
    assert!(a
        .build_ui()
        .hits
        .iter()
        .any(|h| h.action == Action::Select(59)));
    assert_eq!(a.editor.project, before);
    assert_eq!(a.view().eye, view.eye);
    assert_eq!(a.editor.undo_len(), 0);
}
