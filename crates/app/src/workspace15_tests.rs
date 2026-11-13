use super::*;
use poseforge_core::{box_mesh, Material, Node};
fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    let mut p = Project {
        materials: (1..=9)
            .map(|id| Material {
                id,
                name: format!("Материал {id}"),
                in_library: true,
                ..Material::default()
            })
            .collect(),
        ..Project::default()
    };
    p.meshes.push(box_mesh(20, "Cube", Vec3::ONE, Some(1)));
    let mut n = Node::new(30, "Объект");
    n.mesh = Some(20);
    p.nodes.push(n);
    a.editor = Editor::new(p);
    a.editor.selected = Some(30);
    a.width = 1100.;
    a.height = 760.;
    a
}
fn click(a: &mut App, action: Action) {
    a.ui = a.build_ui();
    let h =
        a.ui.hits
            .iter()
            .rev()
            .find(|h| h.action == action)
            .unwrap()
            .rect;
    a.mouse_move([h[0] + h[2] / 2., h[1] + h[3] / 2.]);
    a.click_ui_control();
}
#[test]
fn creation_toolbar_stays_first_in_every_tab_mode_and_collapsed_panel() {
    let mut a = app();
    for scale in [1., 2.] {
        a.scale = scale;
        for shown in [false, true] {
            a.preferences.right_panel = shown;
            for mode in [EditorMode::Scene, EditorMode::Render, EditorMode::Animation] {
                a.editor.mode = mode;
                for tab in [0, 1, 3, 4, 5, 6, 7, 8, 9] {
                    a.set_properties_tab(tab);
                    a.inspector_scroll = a.inspector_max_scroll();
                    let u = a.build_ui();
                    let bounds = a.creation_toolbar_rect();
                    for action in [
                        Action::DropdownOpen(ui::DropdownKind::MeshCreate),
                        Action::DropdownOpen(ui::DropdownKind::LightCreate),
                    ] {
                        let hits: Vec<_> = u
                            .hits
                            .iter()
                            .filter(|h| {
                                h.action == action && ui::contains(bounds, [h.rect[0], h.rect[1]])
                            })
                            .collect();
                        assert_eq!(hits.len(), 1, "{tab} {action:?}");
                        let r = hits[0].rect;
                        assert!(r[1] + r[3] <= a.properties_content_rect()[1]);
                        assert!(!a.canvas_input_at([r[0] + r[2] / 2., r[1] + r[3] / 2.]));
                    }
                }
            }
        }
    }
}
#[test]
fn creation_toolbar_commands_create_real_entities_from_render_tab() {
    let mut a = app();
    a.set_properties_tab(3);
    a.editor.mode = EditorMode::Render;
    click(&mut a, Action::DropdownOpen(ui::DropdownKind::MeshCreate));
    a.action(Action::DropdownChoose(2));
    assert_eq!(a.editor.project.meshes.len(), 2);
    a.set_properties_tab(1);
    click(&mut a, Action::CreateCamera);
    assert!(a.selected_camera_id().is_some());
    assert_eq!(a.editor.undo_len(), 2);
}
#[test]
fn clear_search_inside_field_wins_over_text_selection_and_ime() {
    for scale in [1., 2.] {
        for search in [
            Action::SearchScene,
            Action::MaterialSearch,
            Action::SearchBones,
        ] {
            let mut a = app();
            a.scale = scale;
            if search == Action::MaterialSearch {
                a.set_properties_tab(1);
                a.ui = a.build_ui();
                a.open_dropdown(ui::DropdownKind::MaterialLibrary);
            }
            if search == Action::SearchBones {
                a.character_panel = true;
                a.rig_picker = Some(None);
            }
            a.input(
                search.clone(),
                "Очень длинный поисковый запрос для проверки прокрутки текста".into(),
            );
            a.input.as_mut().unwrap().preedit = "текст IME".into();
            a.ui = a.build_ui();
            let field =
                a.ui.hits
                    .iter()
                    .rev()
                    .find(|h| h.action == search)
                    .unwrap()
                    .rect;
            let clear = search.search_clear().unwrap();
            let cross =
                a.ui.hits
                    .iter()
                    .rev()
                    .find(|h| h.action == clear)
                    .unwrap()
                    .rect;
            assert!(ui::contains(field, [cross[0], cross[1]]));
            assert!(cross[0] + cross[2] <= field[0] + field[2]);
            let before = a.editor.project.clone();
            click(&mut a, clear);
            if search == Action::MaterialSearch { assert!(a.input.as_ref().is_some_and(|i| i.value.is_empty() && i.preedit.is_empty())); }
            else { assert!(a.input.is_none()); }
            assert!(!a.text_selecting);
            assert_eq!(a.editor.project, before);
            assert!(
                a.scene_query.is_empty()
                    && a.surface.material_query.is_empty()
                    && a.bone_query.is_empty()
            );
        }
    }
}
#[test]
fn collapsing_material_library_removes_body_hits_and_preserves_document() {
    let mut a = app();
    a.set_properties_tab(1);
    let before = a.editor.project.clone();
    let max = a.inspector_max_scroll();
    click(&mut a, Action::ToggleSection(100));
    let u = a.build_ui();
    assert!(!u.hits.iter().any(|h| matches!(
        h.action,
        Action::MaterialSelect(..) | Action::MaterialSearch | Action::MaterialScroll
    )));
    assert!(a.inspector_max_scroll() < max);
    let camera_before = a.view().eye;
    a.mouse = [a.right_panel_rect()[0] + 100., 400.];
    a.scroll(-1.);
    assert_eq!(a.view().eye, camera_before);
    a.inspector_scroll = 0.;
    click(&mut a, Action::ToggleSection(100));
    assert!(a
        .build_ui()
        .hits
        .iter()
        .any(|h| matches!(h.action, Action::MaterialSelect(..))));
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
}
#[test]
fn section_state_survives_tabs_and_clamps_scroll_when_collapsed() {
    let mut a = app();
    a.set_properties_tab(3);
    let ids: Vec<_> = a.property_sections().iter().map(|s| s.id).collect();
    for id in &ids {
        a.toggle_property_section(*id);
    }
    assert_eq!(a.inspector_max_scroll(), 0.);
    let u = a.build_ui();
    assert!(!u
        .hits
        .iter()
        .any(|h| matches!(h.action, Action::SurfaceRenderNumber(_))));
    a.set_properties_tab(1);
    assert!(a.property_y(316.).is_some());
    a.set_properties_tab(3);
    assert_eq!(a.preferences.collapsed_sections.len(), ids.len());
    for id in ids {
        a.toggle_property_section(id);
    }
}
#[test]
fn orientation_widget_uses_round_targets_and_repeated_click_reverses_axis() {
    let mut a = app();
    let u = a.build_ui();
    let orbit = u
        .hits
        .iter()
        .find(|h| h.action == Action::ViewOrbit)
        .unwrap()
        .rect;
    assert!(!ui::circle_contains(orbit, [orbit[0], orbit[1]]));
    for axis in 0..6 {
        a.orient_view(axis);
        let direction = (a.view().eye - a.view().target).normalize();
        a.action(Action::ViewAxis(axis));
        let reversed = (a.view().eye - a.view().target).normalize();
        assert!(direction.dot(reversed) < -0.999);
    }
    a.yaw = 0.7;
    a.pitch = 0.4;
    let u = a.build_ui();
    for h in u
        .hits
        .iter()
        .filter(|h| matches!(h.action, Action::ViewAxis(_)))
    {
        assert!(ui::circle_contains(
            h.rect,
            [h.rect[0] + h.rect[2] / 2., h.rect[1] + h.rect[3] / 2.]
        ));
        assert!(!ui::circle_contains(
            h.rect,
            [h.rect[0] + 0.1, h.rect[1] + 0.1]
        ));
    }
}
