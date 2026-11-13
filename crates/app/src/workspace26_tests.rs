use super::*;
use crate::ui::{AuthorAction, MeshSelection, StudioAction};

fn app() -> App {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app.editor = Editor::new(Project::default());
    app.editor.mode = EditorMode::Scene;
    app.action(Action::Studio(StudioAction::Mesh(0)));
    app.primitive_panel = None;
    app.set_properties_tab(1);
    app
}

#[test]
fn material_selection_is_read_only_and_face_assignment_is_explicit() {
    let mut a=app();
    a.action(Action::Author(AuthorAction::SelectMode(MeshSelection::Face)));
    a.action(Action::SurfaceTab(1)); a.authoring.faces=BTreeSet::from([0,2]);
    a.action(Action::SurfaceCreateMaterialPreset(2)); let id=a.selected_material_id().unwrap();
    let before=a.editor.project.clone(); a.action(Action::MaterialSelect(id));
    assert_eq!(a.editor.project,before);
    a.action(Action::MaterialApplyFaces);
    let mesh=a.editor.project.mesh(a.surface_mesh_id().unwrap()).unwrap();
    assert_eq!(mesh.face_material(0),Some(id)); assert_eq!(mesh.face_material(2),Some(id));
    assert_ne!(mesh.face_material(1),Some(id));
    assert!(a.editor.project.node(a.editor.selected.unwrap()).unwrap().material_layers.is_empty());
    a.action(Action::Undo); assert_eq!(a.editor.project,before);
    a.action(Action::MaterialLibraryToggle); a.ui=a.build_ui(); a.open_dropdown(ui::DropdownKind::MaterialLibrary);
    a.input.as_mut().unwrap().insert("ничего-не-найдено"); a.ui=a.build_ui();
    assert!(a.material_popup_visible_ids().is_empty());
    let name=a.editor.project.material(id).unwrap().name.clone();
    a.input.as_mut().unwrap().select_all();a.input.as_mut().unwrap().insert(&name);a.ui=a.build_ui();
    assert!(a.material_popup_visible_ids().contains(&id)); a.close_material_dropdown();
}

#[test]
fn panel_toggles_remove_hits_preserve_document_and_recover_from_topbar() {
    let mut a = app();
    let before = a.editor.project.clone();
    a.action(Action::ToggleLeftPanel);
    let ui = a.build_ui();
    assert!(!ui.hits.iter().any(|h| h.action == Action::SearchScene));
    assert!(!ui.hits.iter().any(|h| h.action == Action::ToggleTools));
    assert!(ui.hits.iter().any(|h| h.action == Action::ToggleLeftPanel));
    a.action(Action::ToggleAnimationPanel);
    let point = [a.width * 0.5, a.height - 100.];
    assert!(!a.canvas_input_at(point));
    a.action(Action::ToggleAnimationLayers);
    a.action(Action::Studio(StudioAction::LayerAdd));
    let authored = a.editor.project.clone();
    a.action(Action::ToggleAnimationPanel);
    a.ui = a.build_ui();
    assert!(a.canvas_input_at(point));
    assert!(!a
        .ui
        .hits
        .iter()
        .any(|h| h.action == Action::Studio(StudioAction::LayerAdd)));
    a.action(Action::ToggleAnimationPanel);
    assert!(a.animation_layers_rect().is_some());
    assert_eq!(a.editor.project, authored);
    a.action(Action::ToggleLeftPanel);
    assert!(a
        .build_ui()
        .hits
        .iter()
        .any(|h| h.action == Action::SearchScene));
    assert_eq!(a.editor.project.meshes, before.meshes);
}

#[test]
fn layer_drawer_fits_minimum_window_and_scrolls_without_changing_timeline() {
    let mut a = app();
    for _ in 0..8 {
        a.action(Action::Studio(StudioAction::LayerAdd));
    }
    a.timeline.expanded = true;
    for scale in [1., 2.] {
        a.scale = scale;
        let ui = a.build_ui();
        let drawer = a.animation_layers_rect().unwrap();
        assert!(a.rail_rect()[1] + a.rail_rect()[3] <= drawer[1]);
        for hit in ui.hits.iter().filter(|h| {
            matches!(
                h.action,
                Action::Studio(StudioAction::LayerSelect(_))
                    | Action::Studio(StudioAction::LayerAdd)
                    | Action::Studio(StudioAction::Field(60))
            )
        }) {
            assert_eq!(intersect_rect(hit.rect, drawer), Some(hit.rect));
        }
        for tab in [0, 1, 3, 6, 7] {
            let hit = ui
                .hits
                .iter()
                .find(|h| h.action == Action::SurfaceTab(tab))
                .unwrap();
            assert!(hit.rect[1] + hit.rect[3] < drawer[1]);
        }
    }
    let frame = a.editor.time;
    let top = a.animation_layers_rect().unwrap()[1];
    a.mouse = [40., top + 60.];
    a.animation_layers_scroll(10.);
    assert_eq!(a.animation.layers_offset, 0);
    a.animation_layers_scroll(-10.);
    assert_eq!(a.animation.layers_offset, 5);
    assert_eq!(a.editor.time, frame);
    let before = a.editor.project.clone();
    a.action(Action::Studio(StudioAction::LayerEnabled(7)));
    assert!(!a.editor.project.animation_layers[7].enabled);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::ToggleAnimationLayers);
    assert!(a.animation_panel_visible());
    assert!(a.animation_layers_rect().is_none());
}

#[test]
fn repeated_camera_look_keeps_horizon_and_translation_keeps_authored_rotation() {
    let mut a = app();
    a.action(Action::CreateCamera);
    let id = a.selected_camera_id().unwrap();
    a.camera_view = Some(id);
    a.camera_navigation_lock = true;
    for n in 0..32 {
        a.mouse = [600., 300.];
        assert!(a.begin_camera_navigation(false, MouseButton::Right));
        let Some(Drag::CameraNavigate(drag)) = a.drag.clone() else {
            panic!("camera drag");
        };
        a.update_camera_navigation(drag, [620., if n % 2 == 0 { 312. } else { 288. }]);
        a.mouse_up(MouseButton::Right);
        let camera = a
            .editor
            .project
            .camera_from_node(id, &a.editor.evaluated().unwrap().global)
            .unwrap();
        let right = (camera.target - camera.eye)
            .normalize()
            .cross(camera.up)
            .normalize();
        assert!(right.y.abs() < 0.00002, "accumulated roll {n}: {right:?}");
    }
    let parent = a.editor.project.next_id();
    let mut node = poseforge_core::Node::new(parent, "Camera parent");
    node.transform.rotation = glam::Quat::from_rotation_y(0.6);
    node.transform.scale = Vec3::splat(1.7);
    a.editor.project.nodes.push(node);
    let camera = a.editor.project.node_mut(id).unwrap();
    camera.parent = Some(parent);
    camera.transform.rotation *= glam::Quat::from_rotation_z(0.31);
    camera.transform.scale = Vec3::new(1.2, 0.9, 1.1);
    let before = a.editor.project.clone();
    let transform = a.editor.project.node(id).unwrap().transform;
    a.mouse = [600., 300.];
    a.begin_camera_navigation(true, MouseButton::Middle);
    let Some(Drag::CameraNavigate(drag)) = a.drag.clone() else {
        panic!("camera pan");
    };
    a.update_camera_navigation(drag, [625., 313.]);
    a.mouse_up(MouseButton::Middle);
    let changed = a.editor.project.node(id).unwrap().transform;
    assert_eq!(changed.rotation, transform.rotation);
    assert_eq!(changed.scale, transform.scale);
    assert_ne!(changed.translation, transform.translation);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.camera_scroll(1.);
    assert_eq!(
        a.editor.project.node(id).unwrap().transform.rotation,
        transform.rotation
    );
}
