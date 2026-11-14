//! Context, undo and persisted settings for the viewport/light workflow.
use super::*;
use poseforge_core::{Light, Material, Node, PrimitiveKind, PrimitiveSettings, VolumeLight};

fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    a.width = 1440.;
    a.height = 1000.;
    let mut node = Node::new(1, "Cube");
    node.mesh = Some(2);
    a.editor = Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2, Some(3)).unwrap()],
        materials: vec![Material { id: 3, ..Default::default() }],
        lights: vec![Light::default(), Light::default()],
        ..Default::default()
    });
    a.editor.selected = Some(1);
    a.editor.mode = EditorMode::Scene;
    a.saved_snapshot = a.editor.project.clone();
    a
}

#[test]
fn shadow_only_is_an_undoable_mesh_property_independent_of_the_scene_eye() {
    let mut a = app();
    a.set_properties_tab(0);
    let original = a.editor.project.clone();
    a.ui = a.build_ui();
    let point = a.ui.action_point(&Action::SceneShadowOnly(1)).unwrap();
    a.mouse_move(point); a.click_ui_control();
    let node = a.editor.project.node(1).unwrap();
    assert!(node.shadow_only && node.visible && node.render_visible);
    assert!(a.node_selectable(1));
    assert_eq!(a.editor.project.meshes, original.meshes);
    assert_eq!(a.editor.project.materials, original.materials);
    assert_eq!(a.editor.undo_len(), 1);
    let shadow = a.editor.project.clone();
    a.primitive_panel = Some(1);
    let mut overlay = Ui::new(1.);
    a.draw_mesh_overlay(&mut overlay);
    assert!(overlay.rects.is_empty() && overlay.hits.is_empty());
    a.action(Action::Undo);
    assert_eq!(a.editor.project, original);
    a.action(Action::Redo);
    assert_eq!(a.editor.project, shadow);
    a.editor.selected = None;
    a.action(Action::SceneShadowOnly(1));
    assert_eq!(a.editor.project, shadow, "a stale control cannot toggle another object");
    a.action(Action::Select(1));
    assert_eq!(a.editor.selected, Some(1));
    a.action(Action::SceneToggleNode(1));
    assert!(a.editor.project.visible_node_ids(false).is_empty());
    assert!(a.editor.project.node(1).unwrap().shadow_only);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, shadow);
    a.action(Action::SceneShadowOnly(1));
    assert!(!a.editor.project.node(1).unwrap().shadow_only);
}

#[test]
fn uv_opens_its_inspector_and_light_selection_closes_mesh_context() {
    let mut a = app();
    a.action(Action::Uv(ui::UvAction::Open));
    assert!(a.uv.open);
    assert_eq!(a.surface.tab, 12);
    let u = a.build_ui();
    let right = a.right_panel_rect()[0];
    assert!(u.hits.iter().any(|h| h.rect[0] > right && h.action == Action::Uv(ui::UvAction::All)));
    let before = a.editor.project.clone();
    a.select_light(0);
    assert!(!a.uv.open);
    assert!(a.authoring.editing.is_none());
    assert_eq!(a.active_light_index(), Some(0));
    let u = a.build_ui();
    assert!(!u.hits.iter().any(|h| matches!(h.action, Action::Uv(_))));
    a.action(Action::Uv(ui::UvAction::Open));
    assert!(!a.uv.open);
    assert_eq!(a.editor.project, before);
    a.action(Action::SceneTool(ui::SceneTool::Clear));
    assert!(a.editor.selected.is_none());
    assert!(a.active_light_index().is_none());
    assert!(a.object_selection().is_empty());
}

#[test]
fn volume_sources_are_independent_and_toggle_is_one_undo() {
    let mut a = app();
    a.select_light(0);
    let original = a.editor.project.clone();
    let undo = a.editor.undo_len();
    a.action(Action::SurfaceLightVolume);
    assert!(a.editor.project.lights[0].volume.enabled);
    assert!(!a.editor.project.lights[1].volume.enabled);
    assert!(a.editor.project.render_settings.atmosphere.density > 0.);
    assert_eq!(a.editor.undo_len(), undo + 1);
    a.editor.undo();
    assert_eq!(a.editor.project, original);
    a.editor.redo();
    a.surface_confirm(Action::SurfaceLightNumber(16), "2.5").unwrap();
    assert_eq!(a.editor.project.lights[0].volume.strength, 2.5);
    assert_eq!(a.editor.project.lights[1].volume.strength, 1.);
    let before = a.editor.project.clone();
    assert!(a.surface_confirm(Action::SurfaceLightNumber(16), "NaN").is_err());
    assert_eq!(a.editor.project, before);
}

#[test]
fn preview_modes_preserve_authored_scene_and_have_distinct_behavior() {
    let mut a = app();
    a.editor.project.materials[0].metallic = 0.9;
    a.editor.project.materials[0].visible = false;
    a.editor.project.lights[1].volume = VolumeLight { enabled: true, strength: 2., tint: [1., 0., 0.] };
    a.editor.project.render_settings.atmosphere.density = 0.1;
    let original = a.editor.project.clone();
    for mode in 0..=3 {
        a.action(Action::ViewShading(mode));
        let mut preview = a.editor.project.clone();
        a.apply_viewport_shading(&mut preview);
        preview.validate().unwrap();
        assert_eq!(a.editor.project, original);
        assert!(!preview.materials[0].visible);
        if mode < 3 {
            assert_eq!(preview.render_settings.atmosphere.density, 0.);
            assert_eq!(preview.lights.len(), 1);
        } else {
            let mut expected = original.clone(); a.apply_viewport_backfaces(&mut expected);
            assert_eq!(preview, expected);
        }
        if mode <= 1 { assert_eq!(preview.materials[0].metallic, 0.); }
        else { assert_eq!(preview.materials[0].metallic, 0.9); }
    }
}

#[test]
fn material_eye_is_reversible_and_cross_preserves_the_library() {
    let mut a = app();
    let original = a.editor.project.clone();
    a.action(Action::SceneMaterialVisible(3));
    assert!(!a.editor.project.material(3).unwrap().visible);
    a.editor.undo();
    assert_eq!(a.editor.project, original);
    a.action(Action::SceneMaterialDetach(1, 3));
    assert!(a.editor.project.mesh(2).unwrap().assigned_materials().is_empty());
    assert!(a.editor.project.material(3).is_some());
    a.editor.undo();
    assert_eq!(a.editor.project, original);
    let layer=a.editor.add_material_layer(1,None).unwrap();
    a.editor.project.materials.iter_mut().find(|m|m.id==layer).unwrap().visible=false;
    let hidden=a.editor.project.clone();
    a.action(Action::MaterialLayerVisible(1,layer));
    assert!(a.editor.project.material(layer).unwrap().visible);
    assert!(a.editor.project.node(1).unwrap().material_layers[0].visible);
    a.action(Action::Undo);assert_eq!(a.editor.project,hidden);
}

#[test]
fn primitives_ignore_the_panned_view_target() {
    let mut a = app();
    a.target = Vec3::new(9., -4., 12.);
    for kind in [0, 1, 2, 4] {
        a.create_primitive(kind);
        let node = a.editor.project.node(a.editor.selected.unwrap()).unwrap();
        assert_eq!(node.transform.translation, Vec3::ZERO);
        assert!(node.parent.is_none());
    }
}

#[test]
fn view_controls_fit_both_themes_and_dpi_without_overlapping() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
          for width in [900.,1100.,1440.] {
            let mut a = app();
            a.width = width;
            a.height = 760.;
            a.scale = scale;
            a.preferences.theme = theme;
            let u = a.build_ui();
            let mut rects = vec![];
            for mode in 0..=3 {
                let r = u.hits.iter().find(|h| h.action == Action::ViewShading(mode)).unwrap().rect;
                assert!(r[0] >= a.viewport()[0]);
                assert!(r[0] + r[2] < a.view_widget_rect()[0]);
                rects.push(r);
            }
            for pair in rects.windows(2) { assert!(pair[0][0] + pair[0][2] < pair[1][0]); }
            assert_eq!(a.view_menu_rect()[2], 26.);
            for action in [Action::Author(ui::AuthorAction::SnapToggle),Action::DropdownOpen(ui::DropdownKind::SnapMode),Action::ToggleViewMenu] {
                let r=u.hits.iter().find(|h|h.action==action).unwrap().rect;
                assert!(intersect_rect(r,a.left_panel_rect()).is_none());
                assert!(intersect_rect(r,a.rail_rect()).is_none());
                assert!(intersect_rect(r,a.view_widget_rect()).is_none());
            }
          }
        }
    }
}

#[test]
fn blank_viewport_click_deselects_without_editing_the_document() {
    for tool in [Tool::Move, Tool::Orbit] {
        let mut a = app();
        a.tool = tool;
        a.preferences.skeleton = false;
        a.editor.project.lights.clear();
        a.ui = a.build_ui();
        let [x, y, w, h] = a.viewport();
        let eval = a.editor.evaluated().unwrap();
        let point = (1..10).flat_map(|ix| (2..9).map(move |iy| [x + w * ix as f32 / 10., y + h * iy as f32 / 10.]))
            .find(|point| a.canvas_input_at(*point) && crate::picking::pick_mesh(&a.editor.project, &eval,
                a.view().view_projection(), a.viewport(), *point).is_none()
                && a.selected_gizmo().is_none_or(|g| g.hit(*point).is_none()))
            .unwrap();
        let original = a.editor.project.clone();
        a.mouse_move(point);
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
        assert!(a.editor.selected.is_none());
        assert!(a.object_selection().is_empty());
        assert_eq!(a.editor.project, original);
    }
}

#[test]
fn uv_inspector_exact_entry_stays_in_the_right_panel() {
    let mut a = app();
    a.action(Action::Uv(ui::UvAction::Open));
    a.action(Action::Uv(ui::UvAction::All));
    a.inspector_scroll = 200.;
    a.ui = a.build_ui();
    a.action(Action::Uv(ui::UvAction::PanelNumber(0)));
    a.ui = a.build_ui();
    let r = a.active_field_rect().unwrap();
    assert!(r[0] > a.right_panel_rect()[0]);
    let original = a.editor.project.clone();
    a.action(Action::Cancel);
    assert_eq!(a.editor.project, original);
}

#[test]
fn liquid_outliner_row_has_independent_visibility_and_reversible_removal() {
    let mut a = app();
    a.action(Action::FluidObject(ui::FluidObjectAction::Attach));
    a.wait_for_background_jobs();
    assert!(a.scene_tools.material_expanded.contains(&1));
    let u = a.build_ui();
    for action in [Action::SceneFluid(1), Action::SceneFluidVisible(1), Action::SceneFluidRemove(1)] {
        assert!(u.hits.iter().any(|hit| hit.action == action));
    }
    let original = a.editor.project.clone();
    a.action(Action::SceneFluidVisible(1));
    assert!(!a.editor.project.node(1).unwrap().fluid.unwrap().visible);
    assert!(a.editor.project.node(1).unwrap().fluid.unwrap().enabled);
    a.editor.undo();
    assert_eq!(a.editor.project, original);
    a.action(Action::SceneFluidRemove(1));
    assert!(a.editor.project.node(1).unwrap().fluid.is_none());
    assert_eq!(a.editor.project.meshes, original.meshes);
    assert_eq!(a.editor.project.materials, original.materials);
    a.editor.undo();
    assert_eq!(a.editor.project, original);
}

#[test]
fn removed_hex_accents_restore_the_existing_default() {
    assert_eq!(ui::Accent::ALL.len(), 6);
    for name in ["5582ff", "13ff86", "#5582FF", "#13FF86"] {
        assert_eq!(ui::Accent::from_name(name), ui::Accent::Blue);
        assert_eq!(Preferences::from_json(&serde_json::json!({"accent": name})).accent, ui::Accent::Blue);
    }
}

#[test]
fn leaving_uv_restores_the_panel_and_other_tabs_close_the_canvas() {
    for visible in [false, true] {
        let mut a = app(); a.set_properties_tab(1); a.preferences.right_panel = visible;
        let original = a.editor.project.clone();
        a.action(Action::Uv(ui::UvAction::Open));
        assert!(a.uv.open && a.preferences.right_panel);
        a.action(Action::Uv(ui::UvAction::Close));
        assert!(!a.uv.open); assert_eq!(a.surface.tab, 1);
        assert_eq!(a.preferences.right_panel, visible);
        for tab in [0, 1, 3, 6, 7, 9, 13] {
            a.action(Action::Uv(ui::UvAction::Open));
            a.action(Action::SurfaceTab(tab));
            assert!(!a.uv.open); assert_eq!(a.surface.tab, tab);
            assert!(a.preferences.right_panel);
        }
        assert_eq!(a.editor.project, original);
    }
}

#[test]
fn viewport_pan_and_zoom_are_cancellable_and_do_not_edit_the_scene() {
    let mut a = app();
    let original = a.editor.project.clone();
    let target = a.target; let distance = a.distance;
    for action in [Action::ViewPan, Action::ViewZoom] {
        a.ui = a.build_ui();
        let r = a.ui.hits.iter().find(|h| h.action == action).unwrap().rect;
        a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
        let start = a.mouse;
        a.mouse_down(MouseButton::Left);
        assert!(matches!(a.drag, Some(Drag::ViewWidget { .. })));
        a.mouse_move([start[0] + 50., start[1] - 45.]);
        if action == Action::ViewPan { assert!(a.target.distance(target) > 0.01); }
        else { assert!(a.distance < distance); }
        a.action(Action::Cancel);
        assert_eq!(a.target, target); assert_eq!(a.distance, distance);
        assert_eq!(a.editor.project, original);
    }
}

#[test]
fn physics_contains_destruction_and_fields_cancel_without_document_edits() {
    let mut a = app();
    a.editor.project.node_mut(1).unwrap().destruction = Some(Default::default());
    a.action(Action::SurfaceTab(13));
    let u = a.build_ui();
    for tab in [4, 11, 13] { assert!(u.hits.iter().any(|h| h.action == Action::SurfaceTab(tab))); }
    let original = a.editor.project.clone();
    let action = Action::Destruction(ui::DestructionAction::Field(0));
    let before = a.destruction_settings().value(0).unwrap();
    let history = a.editor.undo_len();
    a.ui = u;
    let r = a.ui.hits.iter().find(|h| h.action == action).unwrap().rect;
    a.mouse = [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5];
    a.mouse_down(MouseButton::Left); a.mouse_move([a.mouse[0] + 30., a.mouse[1]]);
    assert_ne!(a.destruction_settings().value(0).unwrap(), before);
    a.action(Action::Cancel); assert_eq!(a.destruction_settings().value(0).unwrap(), before);
    assert_eq!(a.editor.project, original);
    assert_eq!(a.editor.undo_len(), history);
}

#[test]
fn navigation_buttons_obey_camera_lock_and_atomic_undo() {
    let mut a = app();
    a.action(Action::CameraView);
    assert!(a.camera_view.is_some());
    a.camera_navigation_lock = true;
    let original = a.editor.project.clone();
    for action in [Action::ViewPan, Action::ViewZoom] {
        a.mouse = [900., 250.];
        a.action(action.clone());
        assert!(matches!(a.drag, Some(Drag::CameraNavigate(_))));
        a.mouse_move([940., 210.]);
        assert_ne!(a.editor.project, original);
        a.action(Action::Cancel);
        assert_eq!(a.editor.project, original);
        let history = a.editor.undo_len();
        a.action(action);
        a.mouse_move([980., 170.]);
        a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.undo_len(), history + 1);
        a.editor.undo();
        assert_eq!(a.editor.project, original);
    }
}

#[test]
fn backface_visibility_is_a_view_preference_and_does_not_edit_materials_or_history() {
    let mut a = app();
    let original = a.editor.project.clone();
    for show in [false, true] {
        a.preferences.show_backfaces = show;
        for mode in 1..=3 {
            a.preferences.viewport_shading = mode;
            let mut preview = original.clone(); a.apply_viewport_shading(&mut preview);
            assert!(preview.materials.iter().all(|m| m.double_sided == show));
            assert!(a.picking_project().materials.iter().all(|m| m.double_sided == show));
            assert_eq!(a.editor.project, original);
            assert_eq!(a.editor.undo_len(), 0);
        }
    }
    a.action(Action::ToggleBackfaces);
    assert!(!a.preferences.show_backfaces);
    assert_eq!(a.editor.project, original);
}

#[test]
fn compact_light_sections_preserve_pointer_mapping_for_every_visible_field() {
    let mut a = app(); a.preferences.right_panel = true;
    for kind in [poseforge_core::LightKind::Point, poseforge_core::LightKind::Directional, poseforge_core::LightKind::Spot, poseforge_core::LightKind::Area] {
        a.editor.project.lights[0].kind = kind;
        a.editor.project.lights[0].target = None;
        a.select_light(0);
        for y in [200., 490., 559., 635., 690., 730., 774., 1026., 1094., 1302., 1640.] {
            let mapped = a.property_y(y).unwrap();
            assert!((a.property_cursor_y(mapped) - y).abs() < 0.001);
        }
        if kind == poseforge_core::LightKind::Point {
            assert!(a.property_y(1026.).unwrap() - a.property_y(774.).unwrap() <= 44.1);
        }
    }
}

#[test]
fn curve_creation_parameters_and_point_edit_use_atomic_undo() {
    let mut a = app();
    let before = a.editor.project.clone();
    a.action(Action::Studio(ui::StudioAction::Mesh(5)));
    let source = a.editor.selected.unwrap();
    let initial = a.editor.project.clone();
    let curve = a.selected_curve().unwrap().1;
    assert_eq!(curve.interpolation, poseforge_core::CurveInterpolation::Bezier);
    assert!(curve.geometry.is_none());
    assert_eq!(a.editor.project.node(source).unwrap().transform.translation, Vec3::ZERO);
    a.curve_action(ui::CurveAction::Point(1));
    a.curve_confirm(&Action::Curve(ui::CurveAction::Field(2)), "0.75").unwrap();
    assert_eq!(a.selected_curve().unwrap().1.points[1][2], 0.75);
    a.editor.undo(); assert_eq!(a.editor.project, initial);
    a.editor.undo(); assert_eq!(a.editor.project, before);
    a.action(Action::Studio(ui::StudioAction::Mesh(6)));
    assert!(a.selected_curve().unwrap().1.closed);
}

#[test]
fn icon_tools_are_right_of_scene_and_omit_pose_and_text() {
    let mut a = app(); a.preferences.left_panel = true;
    a.editor.mode = EditorMode::Scene; a.tool = Tool::Move;
    for width in [1000., 1440.] {
        a.width = width;
        let u = a.build_ui(); let rail = a.rail_rect(); let scene = a.left_panel_rect();
        assert!(rail[0] >= scene[0] + scene[2] + 8.);
        assert_eq!(rail[1],scene[1]);
        let hits: Vec<_> = u.hits.iter().filter(|h| ui::contains(rail, [h.rect[0] + h.rect[2] * 0.5, h.rect[1] + h.rect[3] * 0.5])).collect();
        assert!(hits.iter().any(|h| h.action == Action::Move));
        assert!(!hits.iter().any(|h| matches!(h.action, Action::Poses | Action::ToggleTools)));
        assert!(!u.texts.iter().any(|t| ui::contains(rail, [t.x / u.scale, t.y / u.scale])));
    }
}

fn with_destruction() -> App {
    let mut a = app();
    let source = a.editor.project.node_mut(1).unwrap();
    source.destruction = Some(poseforge_core::DestructionSettings { result: Some(4), ..Default::default() });
    source.visible = false;
    source.render_visible = false;
    a.editor.project.nodes.push(Node::new(4, "Осколки"));
    a.scene_tools.material_expanded.insert(1);
    a.editor.project.validate().unwrap();
    a
}

#[test]
fn destruction_component_rows_survive_reload_and_eye_remove_have_atomic_undo() {
    let mut a = with_destruction();
    let saved = serde_json::to_string(&a.editor.project).unwrap();
    a.editor = Editor::new(serde_json::from_str(&saved).unwrap());
    a.action(Action::SceneDestruction(1));
    assert_eq!(a.editor.selected, Some(1));
    assert_eq!(a.surface.tab, 13);
    let ui = a.build_ui();
    for action in [Action::SceneDestruction(1), Action::SceneDestructionVisible(1), Action::SceneDestructionRemove(1)] {
        assert!(ui.hits.iter().any(|hit| hit.action == action));
    }
    let before = a.editor.project.clone();
    let history = a.editor.undo_len();
    a.action(Action::SceneDestructionVisible(1));
    assert!(a.editor.project.node(1).unwrap().visible);
    assert!(!a.editor.project.node(4).unwrap().visible);
    assert!(!a.editor.project.node(1).unwrap().destruction.unwrap().visible);
    assert_eq!(a.editor.undo_len(), history + 1);
    a.editor.undo(); assert_eq!(a.editor.project, before);
    a.action(Action::SceneDestructionRemove(1));
    assert!(a.editor.project.node(1).unwrap().destruction.is_none());
    assert!(a.editor.project.node(1).unwrap().visible);
    assert!(a.editor.project.node(4).is_none());
    assert_eq!(a.editor.project.mesh(2), before.mesh(2));
    a.editor.undo(); assert_eq!(a.editor.project, before);
}

#[test]
fn component_replacement_and_cloth_visibility_preserve_the_mesh_and_material() {
    for cloth in [true, false] {
        let mut a = with_destruction();
        let before = a.editor.project.clone();
        let history = a.editor.undo_len();
        if cloth { a.action(Action::Studio(ui::StudioAction::Physics(2))); }
        else { a.action(Action::FluidObject(ui::FluidObjectAction::Attach)); a.wait_for_background_jobs(); }
        let node = a.editor.project.node(1).unwrap();
        assert!(node.destruction.is_none() && node.visible);
        assert!(a.editor.project.node(4).is_none());
        assert_eq!(node.physics.is_some(), cloth);
        assert_eq!(node.fluid.is_some(), !cloth);
        assert_eq!(a.editor.undo_len(), history + 1);
        a.editor.project.validate().unwrap();
        if cloth {
            let attached = a.editor.project.clone();
            let ui = a.build_ui();
            for action in [Action::SceneCloth(1), Action::SceneClothVisible(1), Action::SceneClothRemove(1)] {
                assert!(ui.hits.iter().any(|hit| hit.action == action));
            }
            a.action(Action::SceneClothVisible(1));
            assert!(physics_preview::inputs(&a.editor.project, false).is_empty());
            assert!(a.editor.project.node(1).unwrap().visible);
            assert_eq!(a.editor.project.mesh(2), attached.mesh(2));
            a.editor.undo(); assert_eq!(a.editor.project, attached);
            a.action(Action::SceneClothRemove(1));
            assert!(a.editor.project.node(1).unwrap().physics.is_none());
            assert_eq!(a.editor.project.materials, attached.materials);
            a.editor.undo(); assert_eq!(a.editor.project, attached);
        }
        a.editor.undo(); assert_eq!(a.editor.project, before);
    }
}

#[test]
fn destruction_parameters_are_per_object_and_generated_groups_resolve_to_owner() {
    let mut a = with_destruction();
    let mut second = Node::new(5, "Второй примитив");
    second.mesh = Some(2); second.destruction = Some(Default::default());
    a.editor.project.nodes.push(second);
    a.destruction_confirm(&Action::Destruction(ui::DestructionAction::Field(0)), "7").unwrap();
    assert_eq!(a.editor.project.node(1).unwrap().destruction.unwrap().values[0], 7.);
    assert_eq!(a.editor.project.node(5).unwrap().destruction.unwrap().values[0], 12.);
    a.action(Action::Select(4));
    assert_eq!(a.destruction_source(), Some(1));
    assert_eq!(a.destruction_settings().value(0), Some(7.));
    a.action(Action::Select(5));
    assert_eq!(a.destruction_settings().value(0), Some(12.));
    let saved = serde_json::to_string(&a.editor.project).unwrap();
    let loaded: Project = serde_json::from_str(&saved).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded, a.editor.project);
}
