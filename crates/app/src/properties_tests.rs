//! Navigate the actual property rail, preserving selection, gestures and context.
use super::*;
use poseforge_core::{box_mesh, CameraLens, Material, Node};

fn app() -> App {
    let mut p = Project {
        materials: vec![
            Material {
                id: 1,
                ..Material::default()
            },
            Material {
                id: 2,
                ..Material::default()
            },
        ],
        meshes: vec![
            box_mesh(10, "Первый меш", Vec3::ONE, Some(1)),
            box_mesh(11, "Второй меш", Vec3::ONE, Some(2)),
        ],
        ..Project::default()
    };
    let mut first = Node::new(20, "Первый объект");
    first.mesh = Some(10);
    let mut second = Node::new(21, "Второй объект");
    second.mesh = Some(11);
    let mut camera = Node::new(22, "Камера");
    camera.camera = Some(CameraLens::default());
    p.nodes = vec![first, second, camera];
    let mut a = App::new(None);
    a.editor = Editor::new(p);
    a.editor.selected = Some(20);
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    a.preferences.left_panel = true;
    a.width = 1100.;
    a.height = 760.;
    a
}
fn click(a: &mut App, action: Action) {
    a.ui = a.build_ui();
    let r =
        a.ui.hits
            .iter()
            .rev()
            .find(|h| h.action == action)
            .expect("visible rail control")
            .rect;
    a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
    a.click_ui_control();
}
#[test]
fn rail_is_vertical_fixed_and_separate_from_scrolled_content_in_both_themes() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            for (width, height) in [(1100., 620.), (1440., 900.)] {
                let mut a = app();
                a.width = width;
                a.height = height;
                a.scale = scale;
                a.preferences.theme = theme;
                for tab in [0, 1, 3, 6, 7] {
                    click(&mut a, Action::SurfaceTab(tab));
                    a.inspector_scroll = a.inspector_max_scroll();
                    let u = a.build_ui();
                    let toolbar = a.creation_toolbar_rect();
                    // Inspect the same navigation paint used by build_ui: title,
                    // context and collapse control must be below both creation rows.
                    let mut navigation = Ui::with_theme(a.scale, a.preferences.theme, a.mouse);
                    a.draw_properties_navigation(&mut navigation);
                    let captions: Vec<_> = navigation.texts.iter().filter(|t| !t.text.is_empty() && t.text != "−").collect();
                    assert_eq!(captions.len(), 2);
                    for text in captions {
                        assert!(text.y / a.scale >= toolbar[1] + toolbar[3]);
                        assert!((text.y + text.size) / a.scale <= a.properties_content_rect()[1]);
                    }
                    let collapse = navigation.hits.iter().find(|h| h.action == Action::ToggleRightPanel).unwrap();
                    assert!(collapse.rect[1] >= toolbar[1] + toolbar[3]);
                    let mut rows = vec![];
                    for t in [3, 6, 7, 0, 1] {
                        let hits: Vec<_> = u
                            .hits
                            .iter()
                            .filter(|h| h.action == Action::SurfaceTab(t))
                            .collect();
                        let h = hits
                            .iter()
                            .find(|h| h.rect[0] < a.right_panel_rect()[0] + properties::RAIL_WIDTH)
                            .unwrap();
                        assert!(h.rect[2] >= 28. && h.rect[3] >= 20.);
                        rows.push(h.rect);
                    }
                    assert!(rows
                        .windows(2)
                        .all(|r| r[0][0] == r[1][0] && r[0][1] + r[0][3] <= r[1][1]));
                    let bounds = a.properties_content_rect();
                    for h in u
                        .hits
                        .iter()
                        .filter(|h| h.action.is_field() && h.rect[0] >= a.right_panel_rect()[0])
                    {
                        assert!(
                            h.rect[0] >= bounds[0] && h.rect[1] >= bounds[1] - 0.1,
                            "clipped field {:?}",
                            h.action
                        );
                        assert!(h.rect[1] + h.rect[3] <= bounds[1] + bounds[3] + 0.1);
                    }
                }
            }
        }
    }
}
#[test]
fn rail_switch_restores_each_scroll_without_touching_document_selection_or_undo() {
    let mut a = app();
    let before = a.editor.project.clone();
    click(&mut a, Action::SurfaceTab(1));
    a.inspector_scroll = 310.;
    click(&mut a, Action::SurfaceTab(3));
    a.inspector_scroll = 120.;
    click(&mut a, Action::SurfaceTab(1));
    assert_eq!(a.inspector_scroll, 310.);
    click(&mut a, Action::SurfaceTab(3));
    assert_eq!(a.inspector_scroll, 120.);
    assert_eq!(a.editor.selected, Some(20));
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
    assert!(a.properties_key(&Key::Named(NamedKey::PageUp), true));
    assert_eq!(a.surface.tab, 9);
    assert!(a.properties_key(&Key::Named(NamedKey::PageDown), true));
    assert_eq!(a.surface.tab, 3);
    a.mouse = [a.right_panel_rect()[0] + 15., 100.];
    a.modifiers = ModifiersState::CONTROL;
    a.scroll(-1.);
    assert_eq!(a.surface.tab, 6);
    a.modifiers = ModifiersState::empty();
    a.scroll(-1.);
    assert_eq!(a.surface.tab, 6);
    assert_eq!(a.editor.project, before);
}
#[test]
fn material_context_follows_selected_mesh_and_camera_light_empty_cannot_edit_other_objects() {
    let mut a = app();
    click(&mut a, Action::SurfaceTab(1));
    assert_eq!(a.selected_material_id(), Some(1));
    a.action(Action::SurfaceNextMesh);
    assert_eq!(a.selected_material_id(), Some(1));
    a.action(Action::Select(21));
    assert_eq!(a.selected_material_id(), Some(2));
    a.surface.mesh = Some(10);
    a.surface.selection = Some(21);
    assert_eq!(
        a.selected_material_id(),
        Some(2),
        "stale remembered mesh outside selection"
    );
    a.inspector_scroll = a.inspector_max_scroll();
    a.action(Action::Select(22));
    a.action(Action::SurfaceTab(1));
    assert!(a.surface_mesh_id().is_none());
    assert_eq!(a.surface.tab, 0);
    assert!(!a
        .build_ui()
        .hits
        .iter()
        .any(|h| h.action == Action::SurfaceTab(1)));
    let before = a.editor.project.clone();
    for action in [
        Action::SurfaceCreateMaterial,
        Action::SurfaceUniqueMaterial,
        Action::SurfacePreset(7),
        Action::SurfaceNextMesh,
    ] {
        a.action(action);
    }
    assert_eq!(a.editor.project, before);
    a.editor.selected = None;
    assert!(a.surface_mesh_id().is_none());
    a.select_light(0);
    a.action(Action::SurfaceTab(1));
    assert_eq!(a.surface.tab, 0);
    assert!(a.surface_mesh_id().is_none());
    assert_eq!(a.editor.project, before);
}
#[test]
fn grouped_models_cycle_only_descendant_meshes_and_retain_selected_root() {
    let mut a = app();
    let root = Node::new(30, "Группа");
    a.editor.project.nodes.push(root);
    a.editor.project.node_mut(20).unwrap().parent = Some(30);
    a.editor.selected = Some(30);
    assert_eq!(a.surface_mesh_ids(), vec![10]);
    a.action(Action::SurfaceNextMesh);
    assert_eq!(a.surface_mesh_id(), Some(10));
    a.editor.project.node_mut(21).unwrap().parent = Some(30);
    a.action(Action::SurfaceNextMesh);
    assert_eq!(a.surface_mesh_id(), Some(11));
    assert_eq!(a.editor.selected, Some(30));
    a.action(Action::SurfaceNextMesh);
    assert_eq!(a.surface_mesh_id(), Some(10));
}
#[test]
fn pending_numeric_input_owns_keyboard_and_invalid_field_blocks_tab_switch() {
    let mut a = app();
    click(&mut a, Action::SurfaceTab(1));
    a.action(Action::SurfaceMaterialNumber(1));
    assert!(a.input.is_some());
    assert!(!a.properties_key(&Key::Named(NamedKey::PageDown), true));
    let input = a.input.as_mut().unwrap();
    input.select_all();
    input.insert("bad");
    click(&mut a, Action::SurfaceTab(3));
    assert_eq!(a.surface.tab, 1);
    assert!(a.input.is_some());
    a.action(Action::Cancel);
    click(&mut a, Action::SurfaceTab(3));
    assert_eq!(a.surface.tab, 3);
    assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn light_selection_preserves_material_scroll_and_scrollbar_drag_reaches_last_field() {
    let mut a = app();
    click(&mut a, Action::SurfaceTab(1));
    a.inspector_scroll = 270.;
    a.select_light(0);
    assert_eq!(a.surface.tab, 0);
    assert_eq!(a.inspector_scroll, 0.);
    a.action(Action::Select(20));
    click(&mut a, Action::SurfaceTab(1));
    assert_eq!(a.inspector_scroll, 270.);
    let track = a.properties_scrollbar_rect();
    a.mouse_move([track[0] + 6., track[1] + track[3] * 0.5]);
    a.mouse_down(MouseButton::Left);
    assert!(matches!(a.drag, Some(Drag::Scrollbar { .. })));
    a.mouse_move([track[0] + 6., track[1] + track[3] + 100.]);
    a.mouse_up(MouseButton::Left);
    assert!((a.inspector_scroll - a.inspector_max_scroll()).abs() < 0.1);
    let ui = a.build_ui();
    assert!(ui
        .hits
        .iter()
        .any(|h| h.action == Action::SurfaceMaterialNumber(13)));
    assert_eq!(a.editor.undo_len(), 0);
}
