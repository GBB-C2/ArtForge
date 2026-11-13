//! Exercise authoring through the actual panel commands, fields and hit targets.
use super::*;
use poseforge_core::{box_mesh, Material, MaterialPreset, Node};

fn material_app() -> App {
    let mut project = Project::default();
    project.materials.push(Material {
        id: 1,
        name: "Общий материал".into(),
        in_library: true,
        ..Material::default()
    });
    project
        .meshes
        .push(box_mesh(2, "Общий меш", Vec3::ONE, Some(1)));
    for id in [3, 4] {
        let mut node = Node::new(id, format!("Объект {id}"));
        node.mesh = Some(2);
        project.nodes.push(node);
    }
    let mut app = App::new(None);
    app.editor = Editor::new(project);
    app.editor.selected = Some(3);
    app.preferences = Preferences::default();
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app.surface.tab = 1;
    app.saved_snapshot = app.editor.project.clone();
    app
}

fn click(app: &mut App, action: Action) {
    if action == Action::SurfaceMaterialList {
        assert!(app.material_key(&Key::Named(NamedKey::ArrowDown), false, true, false));
        return;
    }
    if action == Action::SurfaceMaterialName {
        let id=app.selected_material_id().unwrap(); app.surface.material_click=None;
        click(app,Action::MaterialSelect(id)); click(app,Action::MaterialSelect(id)); return;
    }
    if action == Action::DropdownOpen(ui::DropdownKind::MaterialCreate) {
        click(app,Action::DropdownOpen(ui::DropdownKind::MaterialActions)); app.choose_dropdown(10); return;
    }
    app.ui = app.build_ui();
    if !app.ui.hits.iter().any(|h| h.action == action) && !app.has_modal() && app.input.is_none() {
        show_npr_action(app, &action);
    }
    let hit = app
        .ui
        .hits
        .iter()
        .rev()
        .find(|hit| hit.action == action)
        .unwrap_or_else(|| panic!("Not visible: {action:?}"))
        .rect;
    app.mouse_move([hit[0] + hit[2] * 0.5, hit[1] + hit[3] * 0.5]);
    app.click_ui_control();
}

fn draft(app: &mut App, value: &str) {
    let input = app.input.as_mut().unwrap();
    input.select_all();
    input.insert(value);
}

fn slider(app: &mut App, field: Action) -> [f32; 4] {
    show_npr_action(app, &field);
    let (rect, range, value) = app
        .ui
        .hits
        .iter()
        .rev()
        .find_map(|hit| match &hit.action {
            Action::Slide(action, range, value) if **action == field => {
                Some((hit.rect, *range, *value))
            }
            _ => None,
        })
        .unwrap();
    app.mouse_move([
        rect[0] + rect[2] * range.ratio(value),
        rect[1] + rect[3] * 0.5,
    ]);
    app.mouse_down(MouseButton::Left);
    assert!(matches!(app.drag, Some(Drag::Slider(_))));
    rect
}

#[test]
fn new_material_and_inline_unicode_rename_preserve_other_instances_and_undo() {
    let mut app = material_app();
    let before = app.editor.project.clone();
    click(&mut app, Action::SurfaceCreateMaterial);
    let mesh = app.editor.project.node(3).unwrap().mesh.unwrap();
    let material = app.selected_material_id().unwrap();
    assert_eq!(mesh, 2);
    assert_ne!(material, 1);
    assert_eq!(app.editor.project.node(4).unwrap().mesh, Some(2));
    assert_eq!(app.editor.project.mesh(2).unwrap().material, Some(1));
    assert_eq!(app.editor.undo_len(), 1);
    click(&mut app, Action::SurfaceMaterialName);
    draft(&mut app, "");
    app.action(Action::ConfirmInput);
    assert!(app.input.is_some() && app.surface.input_error.is_some());
    assert_eq!(app.editor.undo_len(), 1);
    draft(&mut app, "Лак — тёплая керамика");
    app.action(Action::ConfirmInput);
    assert_eq!(
        app.editor.project.material(material).unwrap().name,
        "Лак — тёплая керамика"
    );
    assert_eq!(app.editor.undo_len(), 2);
    click(&mut app, Action::SurfaceMaterialName);
    app.action(Action::ConfirmInput);
    assert_eq!(app.editor.undo_len(), 2, "untouched name is not an edit");
    app.action(Action::Undo);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.selected_material_id(), Some(1));
}

#[test]
fn material_dropdown_navigation_requires_explicit_commit_and_scopes_assignment() {
    let mut app = material_app();
    for id in 10..19 {
        app.editor.project.materials.push(Material {
            id,
            name: format!("Материал {id}"),
            in_library: true,
            ..MaterialPreset::Ceramic.create()
        });
    }
    let before = app.editor.project.clone();
    assert!(app.material_key(&Key::Named(NamedKey::ArrowDown), false, true, false));
    assert!(app.material_picker_active());
    assert!(app.material_key(&Key::Named(NamedKey::End), false, false, false));
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    app.ui = app.build_ui();
    assert!(app.material_popup_visible_ids().contains(&18));
    assert!(app.material_key(&Key::Named(NamedKey::Escape), false, false, false));
    assert!(!app.material_picker_active());
    assert_eq!(app.editor.project, before);
    click(&mut app, Action::SurfaceMaterialList);
    app.material_key(&Key::Named(NamedKey::End), false, false, false);
    app.material_key(&Key::Named(NamedKey::Enter), false, false, false);
    let first = app.selected_material_id().unwrap();
    assert_ne!(first, 18); assert!(app.material_list_ids().contains(&first));
    assert_eq!(app.editor.project.material(first).unwrap().roughness, before.material(18).unwrap().roughness);
    assert_eq!(app.editor.project.meshes,before.meshes); assert_eq!(app.editor.undo_len(),1);
    assert!(!app.material_picker_active());
    click(&mut app,Action::SurfaceMaterialList);
    app.material_key(&Key::Named(NamedKey::End),false,false,false);
    app.material_key(&Key::Named(NamedKey::Enter),false,false,false);
    let second = app.selected_material_id().unwrap(); assert_ne!(first,second);
    assert_eq!(app.editor.project.node(3).unwrap().material_layers.len(),2);
    assert!(app.material_list_ids().contains(&first) && app.material_list_ids().contains(&second));
    assert_eq!(app.editor.undo_len(),2);
    app.action(Action::Undo); app.action(Action::Undo); assert_eq!(app.editor.project,before);
}

#[test]
fn material_picker_preserves_ime_and_rejects_stale_modal_busy_or_locked_targets() {
    let mut app = material_app();
    let before = app.editor.project.clone();
    click(&mut app, Action::SurfaceMaterialName);
    app.input.as_mut().unwrap().preedit = "новое".into();
    assert!(app.material_key(&Key::Named(NamedKey::ArrowDown), false, true, false));
    assert!(!app.material_picker_active());
    assert_eq!(app.input.as_ref().unwrap().preedit, "новое");
    app.action(Action::Cancel);
    click(&mut app, Action::SurfaceMaterialList);
    app.editor.selected = Some(4);
    assert!(!app.material_picker_active());
    app.action(Action::SurfaceAssignMaterial(1));
    assert_eq!(app.editor.project, before);
    for busy in [false, true] {
        if busy {
            let (_sender, receiver) = mpsc::channel();
            app.job = Some(Job {
                receiver,
                cancel: Arc::new(AtomicBool::new(false)),
                label: "Экспорт PNG".into(),
            });
        } else {
            app.help = true;
        }
        app.action(Action::SurfaceCreateMaterial);
        assert!(!app.material_key(&Key::Character("n".into()), false, true, false));
        assert_eq!(app.editor.project, before);
        app.job = None;
        app.help = false;
    }
    app.editor.project.nodes.push(Node::new(20, "Locked group"));
    app.editor.project.node_mut(20).unwrap().locked = true;
    app.editor.project.node_mut(4).unwrap().parent = Some(20);
    let locked = app.editor.project.clone();
    app.action(Action::SurfaceCreateMaterial);
    app.action(Action::SurfaceUniqueMaterial);
    assert_eq!(app.editor.project, locked);
    assert!(app.selected_material_id().is_none());
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn new_surface_values_use_inline_validation_and_coat_slider_has_atomic_undo_and_cancel() {
    let mut app = material_app();
    app.inspector_scroll = app.property_y(390. + app.material_list_height()).unwrap()
        - properties::CONTENT_SHIFT
        - 200.;
    let before = app.editor.project.clone();
    for (field, invalid) in [(7, "0.9"), (7, "NaN"), (8, "1.1"), (9, "0.02")] {
        click(&mut app, Action::SurfaceMaterialNumber(field));
        draft(&mut app, invalid);
        app.action(Action::ConfirmInput);
        assert!(app.input.is_some());
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
        app.action(Action::Cancel);
    }
    click(&mut app, Action::SurfaceMaterialNumber(7));
    draft(&mut app, "1,75");
    app.action(Action::ConfirmInput);
    assert_eq!(app.editor.project.material(1).unwrap().ior, 1.75);
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    let rect = slider(&mut app, Action::SurfaceMaterialNumber(8));
    for fraction in [0.2, 0.5, 0.8] {
        app.mouse_move([rect[0] + rect[2] * fraction, rect[1] + rect[3] * 0.5]);
        assert_eq!(app.editor.undo_len(), 0);
    }
    assert!(app.editor.project.material(1).unwrap().coat_weight > 0.5);
    app.action(Action::Cancel);
    assert_eq!(app.editor.project, before);
    assert!(!app.dirty);
    let rect = slider(&mut app, Action::SurfaceMaterialNumber(8));
    app.mouse_move([rect[0] + rect[2] * 0.7, rect[1] + rect[3] * 0.5]);
    app.mouse_up(MouseButton::Left);
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
}

#[test]
fn material_library_presets_add_local_layers_and_undo_preserves_the_source_mesh() {
    let mut app = material_app();
    app.editor.project.materials[0].base_color = [0.3, 0.2, 0.1, 1.];
    app.editor.project.materials[0].emissive = [4., 2., 1.];
    let original = app.editor.project.clone();
    for (index, preset) in MaterialPreset::ALL.into_iter().enumerate() {
        click(&mut app, Action::SurfaceMaterialList);
        app.input.as_mut().unwrap().insert(preset.label());
        app.ui = app.build_ui();
        assert!(app.material_popup_visible_presets().contains(&index));
        assert_eq!(app.editor.project, original);
        assert_eq!(app.editor.undo_len(), 0);
        if index % 2 == 0 {
            click(&mut app, Action::DropdownChoose(0));
        } else {
            app.material_key(&Key::Named(NamedKey::Enter), false, false, false);
        }
        let id = app.selected_material_id().unwrap();
        assert_ne!(id, 1);
        let created = app.editor.project.material(id).unwrap();
        assert!(!created.in_library);
        let mut expected = preset.create(); expected.id = id; expected.in_library = false;
        assert_eq!(created, &expected);
        assert_eq!(app.editor.project.meshes, original.meshes);
        assert_eq!(app.editor.project.node(3).unwrap().material_layers.len(), 1);
        assert_eq!(app.editor.project.node(3).unwrap().material_layers[0].material, id);
        assert!(app.editor.project.node(3).unwrap().material_layers[0].visible);
        assert_eq!(app.editor.project.node(4), original.node(4));
        assert!(app.visible_material_ids().contains(&id));
        assert_eq!(app.editor.project.material(1), original.material(1));
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, original);
    }
    assert!(MaterialPreset::Glass.create().transmission > 0.);
}

fn show_npr_action(app: &mut App, action: &Action) {
    let max = app.inspector_max_scroll();
    for step in 0..=(max / 80.).ceil() as usize {
        app.inspector_scroll = (step as f32 * 80.).min(max);
        app.ui = app.build_ui();
        let field_visible = app.ui.hits.iter().any(|h| h.action == *action);
        let slider_visible = !matches!(action, Action::SurfaceMaterialNumber(_))
            || app.ui.hits.iter().any(|h| {
                matches!(&h.action,
                Action::Slide(inner, _, _) if **inner == *action)
            });
        if field_visible && slider_visible {
            return;
        }
    }
    panic!("NPR control is unreachable: {action:?}");
}

#[test]
fn npr_creation_dropdown_and_numeric_validation_undo_and_toggle() {
    for index in 10..13 {
        let mut app = material_app();
        let before = app.editor.project.clone();
        click(
            &mut app,
            Action::DropdownOpen(ui::DropdownKind::MaterialCreate),
        );
        app.dropdown_key(&Key::Named(NamedKey::Home), false, false, false);
        for _ in 0..index {
            app.dropdown_key(&Key::Named(NamedKey::ArrowDown), false, false, false);
        }
        app.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
        let id = app.selected_material_id().unwrap();
        assert!(app.editor.project.material(id).unwrap().npr.enabled);
        assert_eq!(app.editor.project.node(4).unwrap().mesh, Some(2));
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        app.action(Action::Redo);

        let field = Action::SurfaceMaterialNumber(19);
        show_npr_action(&mut app, &field);
        click(&mut app, field);
        for invalid in ["1", "9", "3.5", "NaN"] {
            draft(&mut app, invalid);
            app.action(Action::ConfirmInput);
            assert!(app.input.is_some());
            assert_eq!(app.editor.undo_len(), 1);
        }
        draft(&mut app, "6");
        app.action(Action::ConfirmInput);
        assert!(app.input.is_none());
        assert_eq!(app.editor.project.material(id).unwrap().npr.bands, 6);
        assert_eq!(app.editor.undo_len(), 2);
        let settings = app.editor.project.material(id).unwrap().npr;
        show_npr_action(&mut app, &Action::SurfaceNprToggle);
        click(&mut app, Action::SurfaceNprToggle);
        assert!(!app.editor.project.material(id).unwrap().npr.enabled);
        show_npr_action(&mut app, &Action::SurfaceNprToggle);
        click(&mut app, Action::SurfaceNprToggle);
        assert_eq!(app.editor.project.material(id).unwrap().npr, settings);
    }
}

#[test]
fn npr_colors_preview_cancel_and_commit_one_history_entry() {
    for field in 0..5 {
        let mut app = material_app();
        MaterialPreset::Anime.apply(&mut app.editor.project.materials[0]);
        let before = app.editor.project.clone();
        let action = Action::OpenColorPicker(Box::new(Action::SurfaceNprColor(field)));
        show_npr_action(&mut app, &action);
        click(&mut app, action.clone());
        assert!(app.color_picker.is_some());
        app.action(Action::ColorPicker(crate::color_picker::ColorControl::Hex));
        draft(&mut app, "#39A2E8");
        app.action(Action::ConfirmInput);
        assert_ne!(app.editor.project, before);
        app.action(Action::Cancel);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
        show_npr_action(&mut app, &action);
        click(&mut app, action);
        app.action(Action::ColorPicker(crate::color_picker::ColorControl::Hex));
        draft(&mut app, "#39A2E8");
        app.action(Action::ConfirmInput);
        app.action(Action::ColorPicker(
            crate::color_picker::ColorControl::Apply,
        ));
        assert_eq!(app.editor.undo_len(), 1);
        let changed = app.editor.project.clone();
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        app.action(Action::Redo);
        assert_eq!(app.editor.project, changed);
    }
}

#[test]
fn npr_controls_remain_reachable_at_small_widths_and_integer_slider_cancels() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let mut app = material_app();
        app.preferences.theme = theme;
        app.width = 900.;
        app.height = 640.;
        MaterialPreset::Toon.apply(&mut app.editor.project.materials[0]);
        for field in 19..=26 {
            show_npr_action(&mut app, &Action::SurfaceMaterialNumber(field));
        }
        show_npr_action(&mut app, &Action::SurfaceMaterialNumber(19));
        let before = app.editor.project.clone();
        app.modifiers = ModifiersState::SHIFT;
        let rect = slider(&mut app, Action::SurfaceMaterialNumber(19));
        app.mouse_move([rect[0] + rect[2], rect[1] + rect[3] * 0.5]);
        app.action(Action::Cancel);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
    }
}

#[test]
fn point_spot_and_sun_shadow_checkbox_and_filter_edit_use_existing_history() {
    for kind in [
        poseforge_core::LightKind::Directional,
        poseforge_core::LightKind::Point,
        poseforge_core::LightKind::Spot,
    ] {
        let mut app = material_app();
        app.select_light(0);
        app.editor.project.lights[0].kind = kind;
        let before = app.editor.project.clone();
        click(&mut app, Action::SurfaceToggleLightShadow);
        assert!(!app.editor.project.lights[0].casts_shadow);
        assert!(app.editor.project.lights[0].enabled);
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        // Undo releases indexed light ownership because history may reorder lights.
        assert_eq!(app.active_light_index(), None);
        app.select_light(0);
        assert!(app.material_key(&Key::Character("s".into()), false, true, false));
        assert!(!app.editor.project.lights[0].casts_shadow);
        app.action(Action::Undo);
        app.surface.tab = 3;
        click(&mut app, Action::SurfaceRenderNumber(4));
        draft(&mut app, "2.5");
        app.action(Action::ConfirmInput);
        assert_eq!(app.editor.project.render_settings.shadow_filter_radius, 2.5);
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
    }
}

#[test]
fn material_authoring_layout_is_accessible_in_both_themes_and_dpi_with_long_names() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 1.5, 2.] {
            let mut app = material_app();
            app.action(Action::MaterialBrowse(true));
            app.preferences.theme = theme;
            app.scale = scale;
            app.editor.project.materials[0].name =
                "Длинное название — керамический материал персонажа".into();
            let top = app.build_ui();
            assert!(top
                .hits
                .iter()
                .any(|hit| hit.action == Action::DropdownOpen(ui::DropdownKind::MaterialLibrary)));
            assert!(!top
                .hits
                .iter()
                .any(|hit| hit.action == Action::SurfaceMaterialList));
            click(
                &mut app,
                Action::DropdownOpen(ui::DropdownKind::MaterialLibrary),
            );
            let popup = app.build_ui();
            for (index, caption) in ["Матовый", "Пластик", "Металл", "Керамика", "Резина"]
                .iter()
                .enumerate()
            {
                assert!(
                    popup.texts.iter().any(|text| text.text == *caption),
                    "Material preset caption must be complete: {caption}, scale {scale}"
                );
                assert!(popup
                    .hits
                    .iter()
                    .any(|hit| hit.action == Action::DropdownChoose(index)));
            }
            app.action(Action::DropdownDismiss);
            for maps in [false, true] {
                app.surface.maps = maps;
                let mut reached = std::collections::BTreeSet::new();
                let max = app.inspector_max_scroll();
                for index in 0..=((max / 120.).ceil() as usize) {
                    app.inspector_scroll = (index as f32 * 120.).min(max);
                    let ui = app.build_ui();
                    for hit in ui.hits.iter().filter(|hit| {
                        matches!(
                            hit.action,
                            Action::SurfaceMaterialName
                                | Action::SurfaceMaterialNumber(_)
                                | Action::SurfaceMaterialList
                                | Action::SurfaceCreateMaterial
                                | Action::SurfaceUniqueMaterial
                                | Action::DropdownOpen(
                                    ui::DropdownKind::MaterialAlpha
                                )
                                | Action::SurfaceImportTexture(_)
                        )
                    }) {
                        assert!(hit.rect[0] >= app.right_panel_rect()[0] + 16.);
                        assert!(hit.rect[0] + hit.rect[2] <= app.width - 16.);
                        assert!(
                            hit.rect[1] >= 116. && hit.rect[1] + hit.rect[3] <= app.height - 44.
                        );
                        if let Action::SurfaceMaterialNumber(field) = hit.action {
                            reached.insert(field);
                        }
                    }
                }
                assert_eq!(reached, (0..19).collect());
            }
            app.inspector_scroll = 0.;
            app.action(Action::SurfaceMaterialList);
            let ui = app.build_ui();
            assert!(ui
                .hits
                .iter()
                .any(|hit| matches!(hit.action, Action::DropdownChoose(_))));
            assert!(ui
                .texts
                .iter()
                .all(|text| text.x.is_finite() && text.y.is_finite()));
        }
    }
}

#[test]
fn fiber_controls_scroll_into_view_validate_and_commit_single_slider_history() {
    let mut app = material_app();
    app.inspector_scroll = app.inspector_max_scroll();
    let before = app.editor.project.clone();
    for (field, invalid, valid) in [
        (10, "1", "0.85"),
        (11, "181", "-45"),
        (12, "NaN", "0.8"),
        (13, "0.01", "0.25"),
    ] {
        click(&mut app, Action::SurfaceMaterialNumber(field));
        draft(&mut app, invalid);
        app.action(Action::ConfirmInput);
        assert!(app.input.is_some() && app.surface.input_error.is_some());
        assert_eq!(app.editor.project, before);
        draft(&mut app, valid);
        app.action(Action::ConfirmInput);
        assert!(app.input.is_none());
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        let rect = slider(&mut app, Action::SurfaceMaterialNumber(field));
        for fraction in [0.2, 0.4, 0.6] {
            app.mouse_move([rect[0] + rect[2] * fraction, rect[1] + rect[3] * 0.5]);
        }
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
    }
}

#[test]
fn creation_menu_uses_preset_and_ignores_stale_selection() {
    let mut app = material_app();
    let before = app.editor.project.clone();
    click(
        &mut app,
        Action::DropdownOpen(ui::DropdownKind::MaterialCreate),
    );
    // A real popup must survive redraws before keyboard or pointer selection.
    for _ in 0..2 {
        app.ui = app.build_ui();
        assert!(app.dropdown_is_open(ui::DropdownKind::MaterialCreate));
        assert!(app
            .ui
            .hits
            .iter()
            .any(|hit| hit.action == Action::DropdownChoose(0)));
    }
    app.dropdown_key(&Key::Named(NamedKey::Home), false, false, false);
    for _ in 0..6 {
        app.dropdown_key(&Key::Named(NamedKey::ArrowDown), false, false, false);
        app.ui = app.build_ui();
        assert!(app.dropdown_is_open(ui::DropdownKind::MaterialCreate));
    }
    click(&mut app, Action::DropdownChoose(6));
    let material = app
        .editor
        .project
        .material(app.selected_material_id().unwrap())
        .unwrap();
    assert_eq!(material.name, "Вода");
    assert_eq!(material.ior, 1.333);
    assert_eq!(material.transmission, 1.);
    assert_eq!(
        app.editor.project.material(1).unwrap(),
        &before.materials[0]
    );
    assert_eq!(app.editor.undo_len(), 1);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
    click(
        &mut app,
        Action::DropdownOpen(ui::DropdownKind::MaterialCreate),
    );
    app.editor.selected = Some(4);
    app.choose_dropdown(5);
    assert_eq!(app.editor.project, before);
}

#[test]
fn texture_mapping_numeric_edits_and_reset_are_atomic_and_validate() {
    let mut app = material_app();
    let before = app.editor.project.clone();
    for (field, value) in [(14, "3"), (15, "2"), (16, "-0.2"), (17, "0.3"), (18, "45")] {
        assert_eq!(
            app.surface_confirm(Action::SurfaceMaterialNumber(field), value),
            Ok(true)
        );
    }
    let authored = app.editor.project.clone();
    assert_eq!(authored.materials[0].uv_scale, [3., 2.]);
    assert_eq!(authored.materials[0].uv_rotation, 45.);
    for (field, bad) in [(14, "0"), (15, "nan"), (16, "1001"), (18, "181")] {
        assert!(app
            .surface_confirm(Action::SurfaceMaterialNumber(field), bad)
            .is_err());
        assert_eq!(app.editor.project, authored);
    }
    app.action(Action::SurfaceResetMapping);
    assert_eq!(app.editor.project.materials[0], before.materials[0]);
    app.action(Action::Undo);
    assert_eq!(app.editor.project, authored);
}

#[test]
fn npr_palette_hatching_fields_are_reachable_and_undoable() {
    let mut app = material_app();
    app.action(Action::SurfaceNprToggle);
    for (field, value) in [
        (27, "0.8"),
        (28, "0.4"),
        (29, "0.6"),
        (30, "33"),
        (31, "-70"),
        (32, "0.5"),
    ] {
        let a = Action::SurfaceMaterialNumber(field);
        show_npr_action(&mut app, &a);
        let before = app.editor.project.clone();
        let count = app.editor.undo_len();
        click(&mut app, a);
        draft(&mut app, value);
        app.action(Action::ConfirmInput);
        assert!(app.input.is_none(), "{}", app.status);
        assert_eq!(app.editor.undo_len(), count + 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        app.action(Action::Redo);
    }
    app.editor.project.validate().unwrap();
}


#[test]
fn artistic_paint_fields_are_visible_editable_validated_and_undoable() {
    let mut app=material_app();
    app.editor.project.materials[0].artistic =
        poseforge_core::ArtisticSettings::for_style(poseforge_core::ArtisticStyle::Watercolor);
    let original=app.editor.project.clone();
    assert!(app.artistic_enabled());
    for (index,value) in ["0.8","0.7","0.6","0.5","0.4","123"].into_iter().enumerate() {
        let action=Action::SurfaceMaterialNumber(44+index);
        show_npr_action(&mut app,&action);
        let before=app.editor.project.clone();
        click(&mut app,action); draft(&mut app,value); app.action(Action::ConfirmInput);
        assert!(app.input.is_none(),"{}",app.status);
        assert_ne!(app.editor.project,before);
        app.action(Action::Undo); assert_eq!(app.editor.project,before);
    }
    let valid=app.editor.project.clone();
    for (field,bad) in [(37,"0"),(38,"nan"),(39,"1.1"),(49,"0.5"),(49,"65536")] {
        assert!(app.surface_confirm(Action::SurfaceMaterialNumber(field),bad).is_err());
        assert_eq!(app.editor.project,valid);
    }
    assert_eq!(app.editor.project,original);
}

#[test]
fn artistic_style_reset_toggle_and_lock_preserve_authored_data() {
    let mut app=material_app(); app.action(Action::SurfaceArtisticToggle);
    app.surface_confirm(Action::SurfaceMaterialNumber(37),"17").unwrap();
    let before=app.editor.project.clone();
    app.action(Action::SurfaceArtisticNextStyle);
    let m=&app.editor.project.materials[0];
    assert_eq!(m.artistic.style,poseforge_core::ArtisticStyle::Clay);
    assert_eq!(m.artistic.scale,17.); assert_eq!(m.base_color,before.materials[0].base_color);
    app.action(Action::SurfaceArtisticReset);
    assert_eq!(app.editor.project.materials[0].artistic,
        poseforge_core::ArtisticSettings::for_style(poseforge_core::ArtisticStyle::Clay));
    let authored=app.editor.project.materials[0].clone();
    app.action(Action::SurfaceArtisticToggle); app.action(Action::SurfaceArtisticToggle);
    assert_eq!(app.editor.project.materials[0],authored);
    app.editor.project.node_mut(3).unwrap().locked=true;
    let locked=app.editor.project.clone();
    for action in [Action::SurfaceArtisticToggle,Action::SurfaceArtisticNextStyle,Action::SurfaceArtisticReset] {
        app.action(action); assert_eq!(app.editor.project,locked);
    }
}

#[test]
fn artistic_presets_create_through_existing_assignment_actions() {
    for index in 13..poseforge_core::MaterialPreset::ALL.len() {
        let mut app=material_app(); let before=app.editor.project.clone();
        app.action(Action::SurfaceCreateMaterialPreset(index));
        let m=app.editor.project.material(app.selected_material_id().unwrap()).unwrap();
        assert!(m.artistic.enabled && m.artistic.valid());
        app.action(Action::Undo); assert_eq!(app.editor.project,before);
    }
}

#[test]
fn artistic_surface_and_sculpt_sections_are_removed_without_gaps() {
    for npr in [false,true] { for maps in [false,true] {
        let mut app=material_app(); app.surface.maps=maps;
        app.editor.project.materials[0].npr.enabled=npr;
        // Obsolete saved collapse flags must not hide or renumber the paint group.
        app.preferences.collapsed_sections.extend([180,181]);
        for enabled in [false,true] {
            app.editor.project.materials[0].artistic.enabled=enabled;
            let before=app.editor.project.clone();
            let mut panel=Ui::new(1.);
            app.draw_material_panel(&mut panel,0.,320.);
            assert!(!panel.hits.iter().any(|h| matches!(h.action,
                Action::SurfaceArtisticToggle | Action::SurfaceArtisticNextStyle
                | Action::SurfaceArtisticReset | Action::SurfaceMaterialNumber(37..=43))));
            assert!(!app.property_sections().iter().any(|s| matches!(s.id,180|181)));
            assert!(!panel.texts.iter().any(|t| t.text.contains("художественную поверхность")
                || t.text.contains("Микрорельеф") || t.text.contains("Следы лепки")));
            if enabled {
                assert!(app.property_sections().iter().any(|s| s.id==182));
                assert!(app.material_panel_bottom()-app.material_legacy_bottom()<360.);
                for field in [44,49] { show_npr_action(&mut app,&Action::SurfaceMaterialNumber(field)); }
            } else {
                assert!(!app.property_sections().iter().any(|s| s.id==182));
                assert_eq!(app.material_panel_bottom(),app.material_legacy_bottom());
            }
            assert_eq!(app.editor.project,before);
        }
    }}
}

#[test]
fn artistic_numeric_reset_uses_the_current_style_and_is_undoable() {
    let mut app=material_app();
    app.editor.project.materials[0].artistic =
        poseforge_core::ArtisticSettings::for_style(poseforge_core::ArtisticStyle::Watercolor);
    app.surface_confirm(Action::SurfaceMaterialNumber(37),"19").unwrap();
    let before=app.editor.project.clone();
    assert_eq!(app.number_default(&Action::SurfaceMaterialNumber(37)),Some(3.));
    app.action(Action::ResetNumber(Box::new(Action::SurfaceMaterialNumber(37))));
    assert_eq!(app.editor.project.materials[0].artistic.scale,3.);
    app.action(Action::Undo); assert_eq!(app.editor.project,before);
    for field in 37..=49 {
        let default=app.number_default(&Action::SurfaceMaterialNumber(field)).unwrap();
        assert_eq!(default,poseforge_core::ArtisticSettings::for_style(
            poseforge_core::ArtisticStyle::Watercolor).number(field-37) as f64);
    }
}
