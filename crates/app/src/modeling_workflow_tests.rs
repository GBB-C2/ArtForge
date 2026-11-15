//! Actual pointer, keyboard, field and undo routing for the unified modeling tools.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S, ModelingTool as M};
fn app() -> App {
    let mut n = poseforge_core::Node::new(1, "Model");
    n.mesh = Some(2);
    let mut a = App::new(None);
    a.editor = Editor::new(Project {
        nodes: vec![n],
        meshes: vec![
            poseforge_core::PrimitiveSettings::new(poseforge_core::PrimitiveKind::Cube)
                .mesh(2, None)
                .unwrap(),
        ],
        ..Default::default()
    });
    a.preferences = Preferences {
        animation_visible: false,
        right_panel: true,
        ..Default::default()
    };
    a.width = 1440.;
    a.height = 900.;
    a.editor.selected = Some(1);
    a.editor.mode = EditorMode::Scene;
    a.frame_all();
    a.action(Action::Author(A::SelectMode(S::Face)));
    a.authoring.faces = BTreeSet::from([0]);
    a
}
fn point(a: &mut App, action: Action) -> [f32; 2] {
    a.ui = a.build_ui();
    let h =
        a.ui.hits
            .iter()
            .rev()
            .find(|h| h.action == action)
            .unwrap_or_else(|| panic!("Missing {action:?}"));
    [h.rect[0] + h.rect[2] * 0.5, h.rect[1] + h.rect[3] * 0.5]
}
fn click(a: &mut App, action: Action) {
    let p = point(a, action);
    a.mouse_move(p);
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
}
#[test]
fn workflow_palette_has_one_bridge_and_extrude_and_no_removed_tools() {
    let mut a = app();
    a.set_properties_tab(7);
    let mut u = Ui::new(1.);
    a.draw_modeling_palette(&mut u, 0., 400.);
    let tools: Vec<_> = u
        .hits
        .iter()
        .filter_map(|h| {
            if let Action::ModelingTool(t) = h.action {
                Some(t)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(tools.len(), 29);
    for t in [M::Weld, M::Inflate, M::BridgeFaces, M::ExtrudeIndividual] {
        assert!(!tools.contains(&t));
    }
    assert!(tools.contains(&M::Array));
    assert_eq!(M::Dissolve.title(), "Растворить");
}
#[test]
fn tool_click_starts_preview_and_waits_for_selection_without_a_start_button() {
    for tool in [M::Extrude,M::Inset,M::Bevel,M::Slide,M::Array] {
        let mut a = app();
        a.authoring.faces.clear();
        a.action(Action::ModelingTool(tool));
        assert!(a.modeling_live.is_none());
        assert_eq!(a.authoring.workflow.pending_preview,Some(tool));
        assert!(!a.build_ui().texts.iter().any(|t|t.text.contains("Начать предпросмотр")));
        // Selection can arrive through a viewport gesture after choosing the tool.
        a.authoring.faces.insert(0);
        a.mouse_up(MouseButton::Left);
        assert!(a.modeling_live.is_some(),"{tool:?}: {}",a.status);
        assert!(a.editor.has_active_edit());
        assert!(!a.authoring.workflow.collapsed);
        assert_eq!(a.editor.undo_len(),0);
        a.action(Action::Author(A::LiveCancel));
        a.mouse_up(MouseButton::Left);
        assert!(a.modeling_live.is_none());
        assert!(!a.editor.has_active_edit());
    }
}
#[test]
fn bridge_waits_for_two_components_and_cut_cancel_does_not_restart() {
    let mut a=app();
    a.action(Action::ModelingTool(M::Bridge));
    assert!(a.modeling_live.is_none());
    assert_eq!(a.authoring.workflow.pending_preview,Some(M::Bridge));
    a.mouse_up(MouseButton::Left);
    assert!(a.modeling_live.is_none());
    a.authoring.faces.insert(1);
    a.mouse_up(MouseButton::Left);
    assert!(a.modeling_live.is_some());
    a.action(Action::Author(A::LiveCancel));
    a.action(Action::ModelingTool(M::LoopCut));
    assert!(a.authoring.knife.is_some());
    a.action(Action::Author(A::KnifeCancel));
    a.mouse_up(MouseButton::Left);
    assert!(a.authoring.knife.is_none());
    assert!(!a.editor.has_active_edit());
}
#[test]
fn initially_invalid_inset_can_be_corrected_without_restarting_the_tool() {
    let mut a=app();
    a.authoring.faces=(0..6).collect();
    let source=a.editor.project.clone();
    a.action(Action::ModelingTool(M::Inset));
    assert!(a.modeling_live.is_some());
    assert_eq!(a.editor.project,source);
    assert!(a.apply_field_value(Action::Author(A::LiveField(1)),"0.1").is_err());
    a.action(Action::Author(A::FaceToolMode(1)));
    assert_ne!(a.editor.project,source);
    assert_eq!(a.editor.undo_len(),0);
    a.action(Action::Author(A::LiveAccept));
    assert_eq!(a.editor.undo_len(),1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project,source);
}
#[test]
fn inset_and_bevel_modes_keep_one_preview_and_independent_preferences() {
    let mut a = app();
    let source = a.editor.project.clone();
    for tool in [M::Inset,M::Bevel] {
        a.action(Action::ModelingTool(tool));
        a.apply_field_value(Action::Author(A::LiveField(1)),"0.12").unwrap();
        for mode in [2,1,0,2] {
            click(&mut a,Action::Author(A::FaceToolMode(mode)));
            assert!(a.modeling_live.is_some());
            assert_eq!(a.editor.undo_len(),0);
            a.editor.project.validate().unwrap();
        }
        a.action(Action::Author(A::LiveAccept));
        assert_eq!(a.editor.undo_len(),1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project,source);
    }
    assert_eq!(a.authoring.workflow.inset_mode,2);
    assert_eq!(a.authoring.workflow.bevel_mode,2);
    a.action(Action::ModelingTool(M::Inset));
    click(&mut a,Action::Author(A::FaceToolMode(1)));
    assert_eq!(a.authoring.workflow.bevel_mode,2);
    a.action(Action::Author(A::LiveCancel));
    assert_eq!(a.editor.project,source);
}
#[test]
fn floating_tool_colors_match_primitive_panels_in_every_theme_and_accent() {
    let colors_at = |u: &Ui, point| u.rects.iter().filter(|r|ui::contains(r.rect,point))
        .map(|r|r.color).collect::<Vec<_>>();
    for theme in [ui::Theme::Dark,ui::Theme::Light] {
        for accent in ui::Accent::ALL {
            let mut a = app();
            a.preferences.theme=theme; a.preferences.accent=accent;
            a.action(Action::ModelingTool(M::Extrude));
            let mut primitive=app();
            primitive.authoring=Default::default();primitive.primitive_panel=Some(1);
            for transparency in [0.1,0.2,0.7,0.9] {
                a.preferences.tool_panel_transparency=transparency;
                primitive.preferences.tool_panel_transparency=transparency;
                let mut u = Ui::with_theme(1.,theme,[0.,0.]); u.accent=accent;
                let mut reference = Ui::with_theme(1.,theme,[0.,0.]); reference.accent=accent;
                a.draw_tool_panel(&mut u);
                primitive.draw_primitive_panel(&mut reference);
                let rect=a.tool_panel_rect().unwrap();let other=primitive.primitive_panel_rect().unwrap();
                let background=colors_at(&u,[rect[0]+rect[2]-46.,rect[1]+32.]);
                assert_eq!(background,colors_at(&reference,[other[0]+other[2]-46.,other[1]+32.]));
                assert!((background[0][3]-(1.-transparency)).abs()<1e-6);
                for (tool,creation,field) in [
                    (Action::Author(A::LiveAccept),Action::Primitive(ui::PrimitiveAction::Close),false),
                    (Action::Author(A::LiveField(0)),Action::Primitive(ui::PrimitiveAction::Field(0)),true),
                ] {
                    let sample=|ui:&Ui,action:Action| {
                        let r=ui.hits.iter().find(|h|h.action==action && (!matches!(action,Action::Primitive(ui::PrimitiveAction::Close)) || h.rect[2]>36.)).unwrap().rect;
                        [r[0]+if field {r[2]-8.}else{12.},r[1]+8.]
                    };
                    assert_eq!(colors_at(&u,sample(&u,tool)),colors_at(&reference,sample(&reference,creation)));
                }
                assert_eq!(u.texts.iter().find(|t|t.text=="Применить").unwrap().color,
                    reference.texts.iter().find(|t|t.text=="Готово").unwrap().color);
                assert!(u.texts.iter().all(|t|t.color[3]==1.));
            }
        }
    }
}
#[test]
fn panel_transparency_slider_and_reset_do_not_edit_the_project() {
    let mut a=app();
    let source=a.editor.project.clone();
    a.set_properties_tab(9);
    a.apply_field_value(Action::ToolPanelTransparency,"65").unwrap();
    assert_eq!(a.preferences.tool_panel_transparency,0.65);
    assert!(a.apply_field_value(Action::ToolPanelTransparency,"NaN").is_err());
    assert!(a.apply_field_value(Action::ToolPanelTransparency,"101").is_err());
    a.ui=a.build_ui();
    let h=a.ui.hits.iter().find(|h|matches!(&h.action,Action::Slide(v,_,_) if **v==Action::ToolPanelTransparency)).unwrap().clone();
    a.mouse=[h.rect[0]+h.rect[2]*0.5,h.rect[1]+h.rect[3]*0.5];
    a.action(h.action);
    a.mouse_move([a.mouse[0]+12.,a.mouse[1]]);
    assert_ne!(a.preferences.tool_panel_transparency,0.65);
    a.cancel_drag();
    assert_eq!(a.preferences.tool_panel_transparency,0.65);
    a.reset_number(Action::ToolPanelTransparency);
    assert_eq!(a.preferences.tool_panel_transparency,0.1);
    assert_eq!(a.editor.project,source);
    assert_eq!(a.editor.undo_len(),0);
}
#[test]
fn workflow_extrude_modes_rebuild_frozen_source_and_commit_once() {
    let mut a = app();
    let before = a.editor.project.clone();
    a.action(Action::ModelingTool(M::Extrude));
    for mode in [1, 2, 0] {
        click(&mut a, Action::Author(A::ExtrudeMode(mode)));
        assert!(a.modeling_live.is_some());
        assert_eq!(a.editor.undo_len(), 0);
        a.editor.project.validate().unwrap();
    }
    click(&mut a, Action::Author(A::LiveAccept));
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn manifold_preview_crosses_the_opposite_face_and_keeps_the_moving_cap_selected() {
    let mut a=app();let before=a.editor.project.clone();
    a.action(Action::ModelingTool(M::Extrude));
    click(&mut a,Action::Author(A::ExtrudeMode(3)));
    for distance in ["-0.4","-2","-2.4","0.4"] {
        a.apply_field_value(Action::Author(A::LiveField(0)),distance).unwrap();
        assert_eq!(a.authoring.faces.len(),1,"{distance}: {}",a.status);
        assert_eq!(a.editor.undo_len(),0);
        a.editor.project.validate().unwrap();
    }
    click(&mut a,Action::Author(A::LiveAccept));
    assert_eq!(a.editor.undo_len(),1);assert!(a.tool_panel_rect().is_none());
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
}
#[test]
fn workflow_floating_panel_drag_collapse_and_cancel_keep_geometry_transaction() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let mut a = app();
        a.preferences.theme = theme;
        let original = a.editor.project.clone();
        a.action(Action::ModelingTool(M::Extrude));
        let preview = a.editor.project.clone();
        let before = a.tool_panel_rect().unwrap();
        let mut panel = Ui::with_theme(1., theme, [0., 0.]);
        a.draw_tool_panel(&mut panel);
        let sample = [before[0] + before[2] - 46., before[1] + 32.];
        let transmission: f32 = panel.rects.iter()
            .filter(|r| ui::contains(r.rect, sample))
            .map(|r| 1. - r.color[3]).product();
        assert!((transmission-a.preferences.tool_panel_transparency).abs()<1e-6, "Panel has an opaque backing");
        a.mouse = point(&mut a, Action::Author(A::ToolPanelDrag));
        a.mouse_down(MouseButton::Left);
        assert!(matches!(a.drag, Some(Drag::ToolPanel { .. })));
        a.mouse_move([a.mouse[0] + 90., a.mouse[1] - 70.]);
        a.mouse_up(MouseButton::Left);
        let after = a.tool_panel_rect().unwrap();
        assert_ne!([before[0], before[1]], [after[0], after[1]]);
        assert_eq!(a.editor.project, preview);
        assert!(a.editor.has_active_edit());
        click(&mut a, Action::Author(A::ToolPanelToggle));
        assert_eq!(a.tool_panel_rect().unwrap()[3], 38.);
        click(&mut a, Action::Author(A::LiveCancel));
        assert_eq!(a.editor.project, original);
        assert_eq!(a.editor.undo_len(), 0);
    }
}
#[test]
fn floating_properties_scroll_reaches_all_array_fields_without_zoom_or_geometry_changes() {
    let mut a = app();
    a.height = 400.;
    let original = a.editor.project.clone();
    a.action(Action::ModelingTool(M::Array));
    let before = a.editor.project.clone();
    let distance = a.distance;
    let rect = a.tool_panel_rect().unwrap();
    assert!(a.tool_panel_scroll_max() > 0.);
    a.ui = a.build_ui();
    a.mouse = [rect[0] + 40., rect[1] + 80.];
    a.scroll(-100.);
    assert_eq!(a.authoring.workflow.panel_scroll, a.tool_panel_scroll_max());
    let field = point(&mut a, Action::Author(A::LiveField(3)));
    assert!(matches!(a.ui.hit(field), Some(Action::Slide(action, _, _))
        if *action == Action::Author(A::LiveField(3))));
    // Numeric sliders enter exact input on a double click.
    click(&mut a, Action::Author(A::LiveField(3)));
    click(&mut a, Action::Author(A::LiveField(3)));
    a.input.as_mut().unwrap().select_all();
    a.input.as_mut().unwrap().insert("0.45");
    a.action(Action::ConfirmInput);
    assert_eq!(a.authoring.workflow.values[5], 0.45);
    assert_eq!(a.distance, distance);
    assert_ne!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
    a.mouse = point(&mut a, Action::Author(A::ToolPanelScroll));
    a.mouse_down(MouseButton::Left);
    assert!(matches!(a.drag, Some(Drag::ToolPanelScroll { .. })));
    a.mouse_move([a.mouse[0], rect[1] + 38.]);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.authoring.workflow.panel_scroll, 0.);
    assert!(a.build_ui().action_point(&Action::Author(A::LiveField(0))).is_some());
    a.action(Action::ModelingTool(M::Move));
    assert_eq!(a.authoring.workflow.panel_scroll, 0.);
    assert_eq!(a.editor.project, original);
}
#[test]
fn knife_panel_move_collapse_backdrop_and_scroll_keep_the_active_cut_session() {
    for tool in [M::Knife, M::LoopCut] {
        let mut a = app();
        a.height = 360.;
        a.action(Action::ModelingTool(tool));
        let before = a.editor.project.clone();
        click(&mut a, Action::Author(A::ToolPanelToggle));
        assert!(a.authoring.workflow.collapsed);
        assert!(a.authoring.knife.is_some());
        click(&mut a, Action::Author(A::ToolPanelToggle));
        a.mouse = point(&mut a, Action::Author(A::ToolPanelDrag));
        a.mouse_down(MouseButton::Left);
        a.mouse_move([a.mouse[0] + 40., a.mouse[1]]);
        a.mouse_up(MouseButton::Left);
        let rect = a.tool_panel_rect().unwrap();
        a.ui = a.build_ui();
        a.mouse = [rect[0] + 3., rect[1] + rect[3] * 0.5];
        assert_eq!(a.ui.hit(a.mouse), Some(Action::Author(A::LiveBackdrop)));
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
        a.scroll(-100.);
        assert!(a.authoring.workflow.panel_scroll > 0.);
        a.mouse = point(&mut a, Action::Author(A::ToolPanelScroll));
        a.mouse_down(MouseButton::Left);
        a.mouse_move([a.mouse[0], rect[1] + 38.]);
        a.cancel_drag();
        assert!(a.authoring.knife.is_some());
        assert!(a.editor.has_active_edit());
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), 0);
        a.mouse = [rect[0] + 3., rect[1] + 60.];
        a.scroll(-100.);
        click(&mut a, Action::Author(A::KnifeCancel));
        assert!(a.authoring.knife.is_none());
        assert!(!a.editor.has_active_edit());
        assert_eq!(a.editor.project, before);
    }
}
#[test]
fn workflow_gap_slider_does_not_commit_or_cancel_knife_transaction() {
    let mut a = app();
    a.action(Action::ModelingTool(M::Knife));
    let source = a.editor.project.clone();
    a.ui = a.build_ui();
    let h=a.ui.hits.iter().rev().find(|h|matches!(&h.action,Action::Slide(field,_,_)if **field==Action::Author(A::ToolField(0)))).unwrap();
    let p = [h.rect[0] + h.rect[2] * 0.5, h.rect[1] + h.rect[3] * 0.5];
    a.mouse_move(p);
    a.mouse_down(MouseButton::Left);
    a.mouse_move([p[0] + 25., p[1]]);
    a.mouse_up(MouseButton::Left);
    assert!(a.authoring.workflow.values[0] > 0.);
    assert!(a.authoring.knife.is_some());
    assert!(a.editor.has_active_edit());
    assert_eq!(a.editor.undo_len(), 0);
    click(
        &mut a,
        Action::ResetNumber(Box::new(Action::Author(A::ToolField(0)))),
    );
    assert_eq!(a.authoring.workflow.values[0], 0.);
    assert!(a.editor.has_active_edit());
    a.action(Action::Author(A::KnifeCancel));
    assert_eq!(a.editor.project, source);
}
#[test]
fn workflow_loop_cut_works_in_all_component_modes_and_one_undo() {
    for mode in [S::Vertex, S::Edge, S::Face] {
        let mut a = app();
        a.action(Action::Author(A::SelectMode(mode)));
        a.authoring.vertices.clear();
        a.authoring.edges.clear();
        a.authoring.faces.clear();
        let original = a.editor.project.clone();
        a.action(Action::ModelingTool(M::LoopCut));
        assert!(a.authoring.knife.as_ref().unwrap().loop_mode);
        assert_eq!(a.editor.project, original);
        a.authoring.xray = true;
        let mesh = a.editor.editable_mesh(1).unwrap();
        let p = (Vec3::from_array(mesh.vertices[0].position)
            + Vec3::from_array(mesh.vertices[1].position))
            * 0.5;
        a.mouse = a.project_point(p).unwrap();
        a.knife_click();
        assert!(a.editor.project.meshes[0].face_count() > 6, "{}", a.status);
        assert_eq!(a.editor.undo_len(), 0);
        a.action(Action::Author(A::KnifeAccept));
        assert_eq!(a.editor.undo_len(), 1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, original);
    }
}
#[test]
fn workflow_array_fields_apply_reset_and_undo() {
    let mut a = app();
    let before = a.editor.project.clone();
    a.action(Action::ModelingTool(M::Array));
    assert!(a.modeling_live.is_some());
    assert_eq!(a.editor.project.meshes[0].face_count(), 8);
    a.apply_field_value(Action::Author(A::LiveField(0)), "4")
        .unwrap();
    a.apply_field_value(Action::Author(A::LiveField(1)), "2")
        .unwrap();
    a.apply_field_value(Action::Author(A::LiveField(3)), "1.5")
        .unwrap();
    assert!(a
        .apply_field_value(Action::Author(A::LiveField(0)), "2.4")
        .is_err());
    assert_eq!(a.editor.undo_len(), 0);
    a.reset_number(Action::Author(A::LiveField(3)));
    assert_eq!(a.authoring.workflow.values[5], 0.);
    click(&mut a, Action::Author(A::LiveAccept));
    assert_eq!(a.editor.project.meshes[0].face_count(), 9, "{}", a.status);
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn workflow_tooltips_are_small_and_do_not_capture_input() {
    let mut a = app();
    a.action(Action::ModelingTool(M::Array));
    a.mouse = point(&mut a, Action::ModelingTool(M::Array));
    a.ui = a.build_ui();
    assert_eq!(a.ui.hit(a.mouse), Some(Action::ModelingTool(M::Array)));
    assert!(a
        .ui
        .texts
        .iter()
        .any(|t| t.text == "Копирует выделение с заданным шагом"));
}
#[test]
#[ignore = "Exports actual editor UI for CPU inspection"]
fn workflow_capture_ui() {
    let dir = PathBuf::from(std::env::var_os("ARTFORGE_UI_CAPTURE_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for width in [900., 1440.] {
            for (tool, name) in [
                (M::Extrude, "extrude"),
                (M::Inset, "inset"),
                (M::Bevel, "bevel"),
                (M::Bridge, "bridge"),
                (M::Knife, "knife"),
                (M::LoopCut, "loop"),
                (M::Array, "array"),
                (M::Subdivide,"subdivide"),
                (M::Align,"align"),
                (M::Move, "move"),
                (M::Rotate, "rotate"),
                (M::Scale, "scale"),
            ] {
                let mut a = app();
                a.width = width;
                a.preferences.theme = theme;
                if let Ok(height) = std::env::var("ARTFORGE_UI_CAPTURE_HEIGHT") { a.height = height.parse().unwrap(); }
                if let Ok(accent) = std::env::var("ARTFORGE_UI_CAPTURE_ACCENT") { a.preferences.accent = ui::Accent::from_name(&accent); }
                if let Ok(transparency) = std::env::var("ARTFORGE_UI_CAPTURE_TRANSPARENCY") { a.preferences.tool_panel_transparency = transparency.parse().unwrap(); }
                a.action(Action::ModelingTool(tool));
                if std::env::var_os("ARTFORGE_UI_CAPTURE_MERGE").is_some() && a.tool_merge_visible() {
                    a.action(Action::Author(A::ToolMerge));
                }
                a.mouse = [0., 0.];
                let u = a.build_ui();
                let data = serde_json::json!({"width":a.width,"height":a.height,"background":u.palette().viewport,"source":"Actual App::build_ui primitives; CPU rendering", "rectangles":u.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),"texts":u.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
                std::fs::write(
                    dir.join(format!(
                        "workflow-{name}-{}-{}.json",
                        if theme == ui::Theme::Dark {
                            "dark"
                        } else {
                            "light"
                        },
                        width as u32
                    )),
                    serde_json::to_vec(&data).unwrap(),
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn workflow_exact_entry_uses_only_the_floating_panel() {
    let mut a = app();
    a.action(Action::ModelingTool(M::Extrude));
    a.inspector_scroll = a.inspector_max_scroll();
    a.ui = a.build_ui();
    let fields: Vec<_> =
        a.ui.hits
            .iter()
            .filter(|h| h.action == Action::Author(A::LiveField(0)))
            .map(|h| h.rect)
            .collect();
    assert_eq!(fields.len(), 1);
    assert!(ui::contains(a.tool_panel_rect().unwrap(), [fields[0][0], fields[0][1]]));
    for r in fields {
        a.mouse = [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5];
        a.action(Action::Author(A::LiveField(0)));
        assert_eq!(a.active_field_rect(), Some(r));
        a.mouse = [0., 0.];
        assert_eq!(a.active_field_rect(), Some(r));
        a.action(Action::Cancel);
        assert!(a.modeling_live.is_some());
    }
}

#[test]
fn workflow_transform_sliders_preview_frozen_source_cancel_and_one_undo() {
    for (tool, command) in [
        (M::Move, A::Coordinate(0)),
        (M::Rotate, A::RotateDegrees(0)),
        (M::Scale, A::ScaleFactor(3)),
    ] {
        let mut a = app();
        a.authoring.constraint = if tool == M::Rotate { Some(0) } else { None };
        a.action(Action::ModelingTool(tool));
        let original = a.editor.project.clone();
        a.ui = a.build_ui();
        let h = a
            .ui
            .hits
            .iter()
            .rev()
            .find(
                |h| matches!(&h.action,Action::Slide(field,_,_)if **field==Action::Author(command)),
            )
            .unwrap();
        let p = [h.rect[0] + h.rect[2] * 0.5, h.rect[1] + h.rect[3] * 0.5];
        a.mouse_move(p);
        a.mouse_down(MouseButton::Left);
        a.mouse_move([p[0] + 20., p[1]]);
        let first = a.editor.project.clone();
        assert_ne!(first, original);
        a.mouse_move([p[0] + 40., p[1]]);
        a.mouse_move([p[0] + 20., p[1]]);
        assert_eq!(a.editor.project, first);
        a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.undo_len(), 0);
        a.action(Action::Author(A::ToolApply));
        assert_eq!(a.editor.undo_len(), 1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, original);
        a.action(Action::ModelingTool(tool));
        a.ui=a.build_ui();
        a.mouse_move(p);
        a.mouse_down(MouseButton::Left);
        a.mouse_move([p[0] + 30., p[1]]);
        a.cancel_drag();
        assert_eq!(a.editor.project, original);
    }
}

#[test]
fn workflow_loop_cut_can_start_inside_a_face_without_edge_selection() {
    let mut a = app();
    a.authoring.faces.clear();
    a.action(Action::ModelingTool(M::LoopCut));
    a.authoring.workflow.collapsed = true;
    a.ui = a.build_ui();
    let p = a.project_point(Vec3::ZERO).unwrap();
    a.mouse_move(p);
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert!(a.editor.project.meshes[0].face_count() > 6, "{}", a.status);
    a.action(Action::Author(A::KnifeCancel));
    assert_eq!(a.editor.project.meshes[0].face_count(), 6);
    assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn workflow_knife_dropdown_keeps_its_floating_anchor_and_transaction() {
    let mut a = app();
    a.action(Action::ModelingTool(M::Knife));
    let rect = a.tool_panel_rect().unwrap();
    click(&mut a, Action::DropdownOpen(ui::DropdownKind::KnifeAngle));
    assert_eq!(a.tool_panel_rect(), Some(rect));
    assert!(a.authoring.knife.is_some());
    a.choose_dropdown(3);
    assert_eq!(a.authoring.knife_angle, 3);
    assert!(a.editor.has_active_edit());
    a.action(Action::Author(A::KnifeCancel));
    assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn preview_bridge_fields_orbit_pan_zoom_and_cancel_preserve_single_transaction() {
    let mut a=app();
    // Two separate cube caps form closed boundary loops for a bridge.
    let first=a.editor.project.meshes[0].clone();let mut second=first.clone();
    for v in &mut second.vertices {v.position[0]+=4.;}
    let count=first.vertices.len() as u32;let mesh=&mut a.editor.project.meshes[0];
    mesh.vertices.extend(second.vertices.iter().copied());
    mesh.topology.as_mut().unwrap().faces.extend(second.topology.unwrap().faces.iter().map(|f|f.iter().map(|v|v+count).collect::<Vec<_>>()));
    mesh.rebuild_polygon_indices().unwrap();a.frame_all();a.authoring.faces=BTreeSet::from([0,6]);
    let original=a.editor.project.clone();let undo=a.editor.undo_len();
    a.action(Action::ModelingTool(M::Bridge));assert!(a.modeling_live.is_some(),"{}",a.status);
    a.apply_field_value(Action::Author(A::LiveField(0)),"3").unwrap();
    let preview=a.editor.project.clone();a.ui=a.build_ui();
    let area=a.modeling_area();let point=[area[0]+area[2]*0.55,area[1]+area[3]*0.3];
    for button in [MouseButton::Right,MouseButton::Middle] {
        a.mouse=point;a.mouse_down(button);assert!(a.drag.is_some(),"navigation {button:?}");
        a.mouse_move([point[0]+24.,point[1]+12.]);a.mouse_up(button);
        assert_eq!(a.editor.project,preview);assert!(a.modeling_live.is_some());assert_eq!(a.editor.undo_len(),undo);
    }
    a.mouse=point;let distance=a.distance;a.scroll(1.);assert_ne!(a.distance,distance);assert_eq!(a.editor.project,preview);
    a.action(Action::ViewAxis(2));assert!(a.modeling_live.is_some());
    a.action(Action::Author(A::LiveAccept));assert_eq!(a.editor.undo_len(),undo+1);
    a.action(Action::Undo);assert_eq!(a.editor.project,original);
}
#[test]
fn knife_gap_recomputes_existing_cut_and_count_without_accumulation() {
    let mut a=app();a.authoring.xray=true;a.action(Action::ModelingTool(M::LoopCut));
    let original=a.editor.project.clone();
    let mesh=&original.meshes[0];let g=poseforge_core::MeshConnectivity::new(mesh);let &(i,j)=g.edges.first().unwrap();
    let p=Vec3::from_array(mesh.vertices[i].position).lerp(Vec3::from_array(mesh.vertices[j].position),0.35);
    a.mouse=a.project_point(p).unwrap();a.knife_click();let cut=a.editor.project.clone();assert_ne!(cut,original);
    a.apply_field_value(Action::Author(A::ToolField(1)),"0.1").unwrap();let gap=a.editor.project.clone();assert_ne!(gap,cut);
    a.apply_field_value(Action::Author(A::ToolField(1)),"0.2").unwrap();assert_ne!(a.editor.project,gap);
    a.apply_field_value(Action::Author(A::ToolField(1)),"0").unwrap();assert_eq!(a.editor.project,cut);
    a.apply_field_value(Action::Author(A::ToolField(9)),"3").unwrap();assert!(a.editor.project.meshes[0].face_count()>cut.meshes[0].face_count());
    a.apply_field_value(Action::Author(A::ToolField(9)),"1").unwrap();assert_eq!(a.editor.project,cut);
    a.action(Action::Author(A::KnifeCancel));assert_eq!(a.editor.project,original);
}
#[test]
fn new_scene_confirmation_and_mesh_preferences_do_not_silently_discard_work() {
    let mut a=app();a.saved_snapshot=a.editor.project.clone();let before=a.editor.project.clone();
    let undo=a.editor.undo_len();a.action(Action::New);assert!(a.confirm_new);assert_eq!(a.editor.project,before);
    a.action(Action::Cancel);assert!(!a.confirm_new);assert_eq!(a.editor.project,before);
    a.apply_field_value(Action::MeshOpacity,"32").unwrap();assert_eq!(a.preferences.mesh_opacity,0.32);
    a.action(Action::MeshColor);assert!(a.color_picker.is_some());
    a.color_picker.as_mut().unwrap().picker.draft.set_rgba([0.9,0.2,0.1,1.]).unwrap();a.preview_color().unwrap();a.cancel_color_picker();
    assert_eq!(a.preferences.mesh_color,Preferences::default().mesh_color);assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
    a.dirty=true;a.action(Action::New);assert!(a.build_ui().texts.iter().any(|t|t.text.contains("потеряны")));
    a.action(Action::ConfirmNew);assert!(a.editor.project.nodes.is_empty());assert!(!a.dirty);assert_eq!(a.path,None);
}

#[test]
fn mesh_vertex_markers_are_round_and_not_visible_in_edge_mode() {
    let mut a=app();a.authoring.selection_mode=S::Vertex;a.authoring.vertices.clear();
    a.preferences.mesh_color=[0.9,0.15,0.7];a.preferences.mesh_opacity=0.8;
    let mut u=Ui::new(1.);a.draw_mesh_overlay(&mut u);
    let dots:Vec<_>=u.rects.iter().filter(|r|r.segment.is_none()&&r.triangle.is_none()).collect();
    assert!(!dots.is_empty());assert!(dots.iter().all(|r|r.rect[2]<4.&&r.rect[3]<4.));
    assert!(dots.iter().all(|r|r.color[..3]==a.preferences.mesh_color));
    a.authoring.selection_mode=S::Edge;let mut edges=Ui::new(1.);a.draw_mesh_overlay(&mut edges);
    assert!(edges.rects.iter().all(|r|r.segment.is_some()));
}
#[test]
fn color_popup_excludes_underlying_diagonal_strokes() {
    let mut a=app();a.action(Action::MeshColor);let r=a.color_layout().unwrap().panel;
    let mut u=Ui::new(1.);u.mesh_segment([r[0]-10.,r[1]-10.],[r[0]+r[2]+10.,r[1]+r[3]+10.],2.,[1.;4]);
    u.mesh_segment([r[0]-10.,r[1]+r[3]+10.],[r[0]+r[2]+10.,r[1]-10.],2.,[1.;4]);
    a.draw_color_picker(&mut u);
    assert!(u.rects.iter().filter(|q|q.segment.is_some()).all(|q|intersect_rect(q.rect,r).is_none()));
}
#[test]
#[ignore="Exports actual UI primitives for visual review"]
fn viewport_update_capture() {
    let dir=PathBuf::from(std::env::var_os("ARTFORGE_UI_CAPTURE_DIR").unwrap());std::fs::create_dir_all(&dir).unwrap();
    for theme in [ui::Theme::Dark,ui::Theme::Light] {for width in [900.,1440.] {
        for case in ["camera","settings","color","knife","loop","extrude","ribbon","new","bevel-in","bevel-out","sections-edit"] {
            let mut a=app();a.width=width;a.preferences.theme=theme;
            match case {
                "camera"=>{a.action(Action::Author(A::ToggleEdit));a.action(Action::CreateCamera);
                    a.preferences.collapsed_sections.extend([1000,1001,1002,1003]);
                    for field in 4..=10 {a.action(Action::CameraTool(ui::CameraTool::AddEffect(field)));}
                    a.camera_effects.scroll=0.;a.inspector_scroll=0.;},
                "settings"|"color"=>{a.set_properties_tab(9);if case=="color"{a.action(Action::MeshColor);}},
                "knife"=>a.action(Action::ModelingTool(M::Knife)),
                "loop"=>a.action(Action::ModelingTool(M::LoopCut)),
                "sections-edit"=>{create_two_sections(&mut a);a.action(Action::Author(A::SectionMode(true)));a.apply_field_value(Action::Author(A::ToolField(1)),"0.08").unwrap();a.action(Action::Author(A::KnifeCloseGap));},
                "extrude"=>{a.action(Action::ModelingTool(M::Extrude));a.action(Action::Author(A::ExtrudeMode(3)));},
                "bevel-in"|"bevel-out"=>{a.action(Action::ModelingTool(M::Bevel));a.modeling_live_confirm(&Action::Author(A::LiveField(2)),"5").unwrap();a.modeling_live_confirm(&Action::Author(A::LiveField(3)),if case=="bevel-in" {"-75"} else {"75"}).unwrap();},
                "ribbon"=>{a.action(Action::Author(A::ToggleEdit));a.create_curve(0);a.curve_action(ui::CurveAction::AddGeometry);a.curve_action(ui::CurveAction::Profile(1));},
                _=>{a.dirty=true;a.action(Action::New);}
            }
            a.mouse=[0.,0.];let u=a.build_ui();
            let data=serde_json::json!({"width":a.width,"height":a.height,"background":u.palette().viewport,"source":"Actual App::build_ui primitives; CPU rendering", "rectangles":u.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),"texts":u.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
            std::fs::write(dir.join(format!("viewport-{case}-{}-{}.json",if theme==ui::Theme::Dark{"dark"}else{"light"},width as u32)),serde_json::to_vec(&data).unwrap()).unwrap();
        }
    }}
}
#[test]
fn camera_effect_search_and_inner_scroll_keep_project_and_border_ownership() {
    let mut a=app();a.action(Action::Author(A::ToggleEdit));a.action(Action::CreateCamera);
    a.preferences.collapsed_sections.extend([1000,1001,1002,1003]);
    for field in 4..=10 {a.action(Action::CameraTool(ui::CameraTool::AddEffect(field)));}
    a.camera_effects.scroll=0.;a.inspector_scroll=0.;a.ui=a.build_ui();
    let original=a.editor.project.clone();let undo=a.editor.undo_len();
    let r=a.ui.hits.iter().find(|h|h.action==Action::CameraEffectScroll).unwrap().rect;
    a.mouse=[r[0]-20.,r[1]+r[3]*0.5];a.scroll(-2.);assert!(a.camera_effects.scroll>0.);
    let before=a.camera_effects.scroll;a.mouse=[r[0]+r[2]*0.5,r[1]+20.];a.mouse_down(MouseButton::Left);
    a.mouse_move([a.mouse[0],r[1]+r[3]-20.]);a.mouse_up(MouseButton::Left);assert!(a.camera_effects.scroll>before);
    a.action(Action::CameraEffectSearch);a.input.as_mut().unwrap().value="Зерно".into();
    let u=a.build_ui();assert!(u.hits.iter().any(|h|h.action==Action::CameraNumber(9)));
    assert!(!u.hits.iter().any(|h|h.action==Action::CameraNumber(4)));
    a.confirm_input();assert_eq!(a.editor.project,original);assert_eq!(a.editor.undo_len(),undo);
}
#[test]
fn mesh_color_context_undo_redo_restores_preferences_without_undoing_the_scene() {
    let mut a=app();a.set_properties_tab(9);
    let project=a.editor.project.clone();let undo=a.editor.undo_len();let original=a.preferences.mesh_color;
    a.apply_swatch(Action::MeshColor,[0.8,0.2,0.4,1.]);
    let changed=a.preferences.mesh_color;assert_ne!(original,changed);
    for (command,expected) in [(1,original),(2,changed)] {
        a.mouse=point(&mut a,Action::OpenColorPicker(Box::new(Action::MeshColor)));
        a.mouse_down(MouseButton::Right);a.mouse_up(MouseButton::Right);
        assert!(a.swatch_menu_open());a.action(Action::SwatchCommand(command));
        assert_eq!(a.preferences.mesh_color,expected);
        assert_eq!(a.editor.project,project);assert_eq!(a.editor.undo_len(),undo);
    }
}

#[test]
fn polygon_bevel_has_signed_profile_segments_and_exact_undo() {
 let mut a=app();let original=a.editor.project.clone();let history=a.editor.undo_len();
 a.action(Action::ModelingTool(M::Bevel));assert!(a.modeling_live.is_some());
 for (field,value) in [(2,"5"),(3,"-75"),(1,"0")] {
   a.action(Action::Author(A::LiveField(field)));a.input.as_mut().unwrap().value=value.into();a.confirm_input();assert!(a.input.is_none());
 }
 let inward=a.editor.project.clone();
 a.action(Action::Author(A::LiveField(3)));a.input.as_mut().unwrap().value="75".into();a.confirm_input();assert_ne!(a.editor.project,inward);
 a.ui=a.build_ui();for i in 0..4 {assert!(a.ui.hits.iter().any(|h|h.action==Action::Author(A::LiveField(i))));}
 a.action(Action::Author(A::LiveAccept));assert_eq!(a.editor.undo_len(),history+1);a.action(Action::Undo);assert_eq!(a.editor.project,original);
}

#[test]
fn face_bevel_starts_on_a_newly_created_cube() {
    let mut a=app();a.action(Action::Author(A::ToggleEdit));
    a.create_primitive(0);
    let id=a.editor.selected.unwrap();
    a.action(Action::Author(A::SelectMode(S::Face)));
    a.action(Action::Author(A::SelectAll));
    let before=a.editor.project.clone();let history=a.editor.undo_len();
    a.action(Action::ModelingTool(M::Bevel));
    assert!(a.modeling_live.is_some(),"{}",a.status);
    assert_ne!(a.editor.editable_mesh(id).unwrap().face_count(),6);
    a.action(Action::Author(A::LiveAccept));assert_eq!(a.editor.undo_len(),history+1);
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
}

#[test]
fn face_bevel_preview_applies_to_a_new_knife_face_with_profile_and_undo() {
    let mut a=app();
    let mesh=a.editor.editable_mesh(1).unwrap();let ring=mesh.face_vertices(0).unwrap();
    let p=|i:usize|Vec3::from_array(mesh.vertices[ring[i] as usize].position);
    let cuts=vec![poseforge_core::KnifeCut{face:0,first:p(0).lerp(p(1),0.37).to_array(),second:p(3).lerp(p(2),0.37).to_array()}];
    a.editor.edit_mesh(1,poseforge_core::MeshEdit::KnifeStroke{cuts}).unwrap();
    a.invalidate_geometry();a.authoring.faces=BTreeSet::from([1]);
    let before=a.editor.project.clone();let history=a.editor.undo_len();
    click(&mut a,Action::ModelingTool(M::Bevel));
    assert!(a.modeling_live.is_some(),"{}",a.status);
    for (field,value) in [(2,"5"),(3,"-75"),(3,"75")] {
        a.modeling_live_confirm(&Action::Author(A::LiveField(field)),value).unwrap();
        let report=poseforge_core::inspect_topology(a.editor.editable_mesh(1).unwrap());
        assert_eq!((report.boundary_edges,report.nonmanifold_edges,report.degenerate_faces),(0,0,0));
    }
    a.action(Action::Author(A::LiveAccept));let after=a.editor.project.clone();
    assert_ne!(after,before);assert_eq!(a.editor.undo_len(),history+1);
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
    a.action(Action::Redo);assert_eq!(a.editor.project,after);
}

fn create_two_sections(a:&mut App) {
    a.authoring.xray=true;a.action(Action::ModelingTool(M::LoopCut));
    a.apply_field_value(Action::Author(A::ToolField(9)),"2").unwrap();
    let m=a.editor.editable_mesh(1).unwrap();
    let p=Vec3::from_array(m.vertices[0].position).lerp(Vec3::from_array(m.vertices[1].position),0.5);
    a.mouse=a.project_point(p).unwrap();a.knife_click();
    assert!(a.editor.project.meshes[0].face_count()>6,"{}",a.status);
}
#[test]
fn section_modes_edit_individual_planes_and_commit_one_undo_step() {
    let mut a=app();let original=a.editor.project.clone();create_two_sections(&mut a);
    click(&mut a,Action::Author(A::SectionMode(true)));
    a.action(Action::Author(A::SectionSelect(0)));
    a.apply_field_value(Action::Author(A::ToolField(10)),"0.1").unwrap();
    a.apply_field_value(Action::Author(A::ToolField(11)),"15").unwrap();
    a.apply_field_value(Action::Author(A::ToolField(1)),"0.04").unwrap();
    a.action(Action::Author(A::KnifeCloseGap));
    let first=a.editor.project.clone();assert_eq!(a.section_value(10),Some(0.1));
    a.action(Action::Author(A::SectionSelect(1)));
    assert_eq!(a.section_value(10),Some(0.));assert_eq!(a.section_value(1),Some(0.));
    a.apply_field_value(Action::Author(A::ToolField(12)),"-12").unwrap();
    assert_ne!(a.editor.project,first);
    let valid=a.editor.project.clone();
    assert!(a.apply_field_value(Action::Author(A::ToolField(1)),"1000").is_err());
    assert_eq!(a.editor.project,valid);assert_eq!(a.section_value(1),Some(0.));
    a.action(Action::Author(A::SectionSelect(0)));assert_eq!(a.section_value(10),Some(0.1));
    a.action(Action::ResetNumber(Box::new(Action::Author(A::ToolField(10)))));assert_eq!(a.section_value(10),Some(0.));
    assert_eq!(a.editor.undo_len(),0);assert!(a.editor.has_active_edit());
    let after=a.editor.project.clone();a.action(Action::Author(A::KnifeAccept));assert_eq!(a.editor.undo_len(),1);
    a.action(Action::Undo);assert_eq!(a.editor.project,original);a.action(Action::Redo);assert_eq!(a.editor.project,after);
}
#[test]
fn section_delete_last_restores_source_and_cancel_restores_selection() {
    let mut a=app();let original=a.editor.project.clone();create_two_sections(&mut a);
    a.action(Action::Author(A::SectionMode(true)));
    a.action(Action::Author(A::SectionDelete));assert!(a.authoring.knife.is_some());
    a.action(Action::Author(A::SectionDelete));assert_eq!(a.editor.project,original);
    a.action(Action::Author(A::KnifeCancel));assert_eq!(a.authoring.faces,BTreeSet::from([0]));assert_eq!(a.editor.undo_len(),0);
}
#[test]
fn section_editing_keeps_navigation_and_both_theme_controls_available() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
        let mut a=app();a.scale=scale;a.preferences.theme=theme;create_two_sections(&mut a);
        click(&mut a,Action::Author(A::SectionMode(true)));
        let u=a.build_ui();
        assert!(u.rects.iter().any(|r|r.triangle.is_some()&&r.color[3]>0.&&r.color[3]<0.2));
        for field in [1,10,11,12] {assert!(u.hits.iter().any(|h|h.action==Action::Author(A::ToolField(field))));}
        let before=a.editor.project.clone();let area=a.modeling_area();let point=[area[0]+area[2]*0.7,area[1]+area[3]*0.65];
        for button in [MouseButton::Right,MouseButton::Middle] {a.mouse=point;a.mouse_down(button);a.mouse_move([point[0]+20.,point[1]+10.]);a.mouse_up(button);assert!(a.authoring.knife.is_some());assert_eq!(a.editor.project,before);}
        a.mouse=point;let distance=a.distance;a.scroll(1.);assert_ne!(a.distance,distance);assert_eq!(a.editor.project,before);
        a.action(Action::Author(A::KnifeCancel));
    }}
}

#[test]
fn selected_section_plane_drags_without_adding_cuts_or_undo_entries() {
    let mut a=app();let before=a.editor.project.clone();create_two_sections(&mut a);
    a.action(Action::Author(A::SectionMode(true)));a.mouse=a.project_point(Vec3::ZERO).unwrap();
    a.knife_click();assert!(a.authoring.knife.as_ref().unwrap().press.is_some());
    let faces=a.editor.project.meshes[0].face_count();let start=a.mouse;
    a.mouse_down(MouseButton::Right);assert!(a.authoring.knife.is_some());assert!(a.drag.is_none());
    a.mouse_move([start[0]+30.,start[1]+10.]);assert_ne!(a.section_value(10),Some(0.));
    a.mouse_up(MouseButton::Left);assert!(a.authoring.knife.as_ref().unwrap().press.is_none());
    assert_eq!(a.editor.project.meshes[0].face_count(),faces);assert_eq!(a.editor.undo_len(),0);
    a.action(Action::Author(A::KnifeCancel));assert_eq!(a.editor.project,before);
}
#[test]
fn face_bevel_legacy_action_respects_face_mode_on_an_open_surface() {
    let mut a=app();a.editor.project.meshes[0]=poseforge_core::PrimitiveSettings::new(poseforge_core::PrimitiveKind::Plane).mesh(2,None).unwrap();
    a.authoring.faces=BTreeSet::from([0]);a.authoring.selection_mode=S::Face;
    let before=a.editor.project.clone();a.action(Action::Author(A::EdgeBevel));
    assert!(a.modeling_live.is_some(),"{}",a.status);assert_ne!(a.editor.project,before);
    a.action(Action::Author(A::LiveCancel));assert_eq!(a.editor.project,before);
}

#[test]
fn region_bevel_preview_preserves_shared_edge_and_mode_switches_stay_one_edit() {
    let mut a=app();
    a.authoring.faces=BTreeSet::from([0,2]);
    let source=a.editor.project.clone();
    a.action(Action::ModelingTool(M::Bevel));
    let joined=|a:&App|poseforge_core::MeshConnectivity::new(a.editor.editable_mesh(1).unwrap())
        .edge_faces.values().any(|faces|faces.contains(&0)&&faces.contains(&2));
    assert!(joined(&a),"{}",a.status);
    a.apply_field_value(Action::Author(A::LiveField(2)),"5").unwrap();
    a.apply_field_value(Action::Author(A::LiveField(3)),"-75").unwrap();
    assert!(joined(&a));
    click(&mut a,Action::Author(A::FaceToolMode(2)));
    assert!(!joined(&a));
    click(&mut a,Action::Author(A::FaceToolMode(0)));
    assert!(joined(&a));
    assert_eq!(a.editor.undo_len(),0);
    click(&mut a,Action::Author(A::LiveAccept));
    assert_eq!(a.editor.undo_len(),1);
    assert!(a.tool_panel_rect().is_none());
    a.action(Action::Undo);assert_eq!(a.editor.project,source);
}

#[test]
fn modeling_options_cancel_closes_every_live_panel_and_pending_preview() {
    for tool in [M::Array,M::Inset,M::Bevel,M::Extrude,M::Slide,M::Bridge,M::Knife,M::LoopCut] {
        let mut a=app();let original=a.editor.project.clone();
        a.action(Action::ModelingTool(tool));
        let cancel=if a.authoring.knife.is_some(){A::KnifeCancel}else if a.modeling_live.is_some(){A::LiveCancel}else{A::ToolCancel};
        a.action(Action::Author(cancel));a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.project,original,"{tool:?}");assert!(a.tool_panel_rect().is_none(),"{tool:?}");
        assert!(a.authoring.workflow.pending_preview.is_none());assert!(!a.editor.has_active_edit());assert_eq!(a.editor.undo_len(),0);
    }
}
#[test]
fn modeling_options_merge_fields_keep_preview_selection_and_one_undo() {
    for tool in [M::Array,M::Inset,M::Bevel] {
        let mut a=app();let before=a.editor.project.clone();
        a.action(Action::ModelingTool(tool));
        let unmerged=a.editor.project.clone();
        click(&mut a,Action::Author(A::ToolMerge));
        assert!(a.tool_merge_enabled());assert!(a.modeling_live.is_some());assert_eq!(a.editor.undo_len(),0);
        let mesh=a.editor.editable_mesh(1).unwrap();assert!(a.authoring.faces.iter().all(|&f|f<mesh.face_count()));
        click(&mut a,Action::Author(A::ToolMerge));assert_eq!(a.editor.project,unmerged);
        a.action(Action::Author(A::ToolMerge));let after=a.editor.project.clone();
        a.action(Action::Author(A::LiveAccept));assert_eq!(a.editor.undo_len(),1);assert!(a.tool_panel_rect().is_none());
        a.action(Action::Undo);assert_eq!(a.editor.project,before);a.action(Action::Redo);assert_eq!(a.editor.project,after);
    }
}
#[test]
fn modeling_options_subdivide_slider_rebuilds_source_and_resets() {
    let mut a=app();let before=a.editor.project.clone();
    a.action(Action::ModelingTool(M::Subdivide));
    assert_eq!(a.editor.project.meshes[0].face_count(),9);assert_eq!(a.editor.undo_len(),0);
    a.apply_field_value(Action::Author(A::SubdivideCount),"3").unwrap();assert_eq!(a.editor.project.meshes[0].face_count(),14);
    a.apply_field_value(Action::Author(A::SubdivideCount),"5").unwrap();assert_eq!(a.editor.project.meshes[0].face_count(),30);
    click(&mut a,Action::ResetNumber(Box::new(Action::Author(A::SubdivideCount))));assert_eq!(a.editor.project.meshes[0].face_count(),9);
    let after=a.editor.project.clone();click(&mut a,Action::Author(A::ToolApply));
    assert_eq!(a.editor.undo_len(),1);assert!(a.tool_panel_rect().is_none());
    a.action(Action::Undo);assert_eq!(a.editor.project,before);a.action(Action::Redo);assert_eq!(a.editor.project,after);
    a.action(Action::ModelingTool(M::Subdivide));click(&mut a,Action::Author(A::ToolCancel));assert_eq!(a.editor.project,after);assert!(a.tool_panel_rect().is_none());
}
#[test]
fn modeling_options_transform_fields_cancel_as_a_whole_tool() {
    for tool in [M::Move,M::Rotate,M::Scale] {
        let mut a=app();let before=a.editor.project.clone();let selection=a.authoring.faces.clone();
        a.action(Action::ModelingTool(tool));
        let (field,value)=match tool {M::Move=>(A::Coordinate(0),"2"),M::Rotate=>(A::RotateDegrees(2),"15"),_=>(A::ScaleFactor(3),"1.25")};
        a.apply_field_value(Action::Author(field),value).unwrap();
        assert_ne!(a.editor.project,before);assert_eq!(a.editor.undo_len(),0);
        click(&mut a,Action::Author(A::ToolCancel));assert_eq!(a.editor.project,before);assert_eq!(a.authoring.faces,selection);assert!(!a.editor.has_active_edit());
        a.action(Action::ModelingTool(tool));a.apply_field_value(Action::Author(field),value).unwrap();let after=a.editor.project.clone();
        click(&mut a,Action::Author(A::ToolApply));assert_eq!(a.editor.undo_len(),1);a.action(Action::Undo);assert_eq!(a.editor.project,before);a.action(Action::Redo);assert_eq!(a.editor.project,after);
    }
}
#[test]
fn modeling_options_every_tool_has_confirmation_controls_in_both_themes() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {for dpi in [1.,2.] {
        for tool in [M::Select(0),M::Select(1),M::Select(2),M::Move,M::Rotate,M::Scale,M::Merge,M::Dissolve,M::DeleteFaces,M::Subdivide,M::FillHoles,M::Triangulate,M::Separate,M::Smooth,M::Flat,M::Align,M::Relax,M::Sphere,M::Circle,M::Twist,M::Inspect] {
            let mut a=app();a.preferences.theme=theme;a.scale=dpi;a.height=1300.;
            let before=a.editor.project.clone();a.action(Action::ModelingTool(tool));
            let u=a.build_ui();assert!(u.texts.iter().any(|t|t.text=="Применить"),"{tool:?}");
            assert!(u.hits.iter().any(|h|h.action==Action::Author(A::ToolCancel)),"{tool:?}");
            assert_eq!(a.editor.undo_len(),0,"{tool:?}");
            a.action(Action::Author(A::ToolCancel));assert_eq!(a.editor.project,before,"{tool:?}");assert!(a.tool_panel_rect().is_none());
        }
    }}
}
#[test]
fn modeling_options_pending_subdivide_and_shift_selection_preview_use_source() {
    let mut a=app();a.authoring.faces.clear();let before=a.editor.project.clone();
    a.action(Action::ModelingTool(M::Subdivide));assert_eq!(a.editor.project,before);
    a.authoring.faces.insert(0);a.mouse_up(MouseButton::Left);assert_eq!(a.editor.project.meshes[0].face_count(),9);
    a.modeling_modifiers_changed(ModifiersState::SHIFT);assert_eq!(a.editor.project,before);
    a.authoring.faces=BTreeSet::from([0,1]);a.modeling_modifiers_changed(ModifiersState::empty());
    assert_eq!(a.editor.project.meshes[0].face_count(),12);
    a.action(Action::Author(A::ToolCancel));assert_eq!(a.editor.project,before);assert!(a.authoring.faces.is_empty());
}
#[test]
fn modeling_options_cancel_ignores_invalid_text_and_closes_collapsed_panels() {
    for tool in [M::Bevel,M::Subdivide,M::Move,M::Knife] {
        let mut a=app();let before=a.editor.project.clone();a.action(Action::ModelingTool(tool));
        let field=match tool {M::Bevel=>A::LiveField(0),M::Subdivide=>A::SubdivideCount,M::Move=>A::Coordinate(0),_=>A::ToolField(0)};
        a.action(Action::Author(field));a.input.as_mut().unwrap().select_all();a.input.as_mut().unwrap().insert("nan");
        let cancel=if a.modeling_live.is_some(){A::LiveCancel}else if a.authoring.knife.is_some(){A::KnifeCancel}else{A::ToolCancel};
        a.action(Action::Author(cancel));assert!(a.input.is_none());assert!(a.tool_panel_rect().is_none());assert_eq!(a.editor.project,before);
        a.action(Action::ModelingTool(tool));a.action(Action::Author(A::ToolPanelToggle));
        click(&mut a,Action::Author(cancel));assert!(a.tool_panel_rect().is_none());assert_eq!(a.editor.project,before);
    }
}
#[test]
fn modeling_options_background_result_stays_pending_until_apply_and_cancel_discards_worker() {
    for cancel in [false,true] {
        let mut a=app();let source=a.editor.project.clone();a.action(Action::ModelingTool(M::Move));
        let mut result=Editor::new(source.clone());result.edit_mesh(1,poseforge_core::MeshEdit::Translate{vertices:BTreeSet::from([0]),delta:Vec3::X}).unwrap();
        let after=result.project.clone();let (tx,rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));
        a.job=Some(Job{receiver:rx,cancel:stop.clone(),label:"Обработка сетки".into()});
        a.apply_modeling_session();assert!(a.authoring.workflow.session.is_some());assert_eq!(a.editor.undo_len(),0);
        if cancel {
            a.action(Action::Author(A::ToolCancel));assert!(stop.load(Ordering::Relaxed));
            assert!(tx.send(JobMessage::MeshToolReady(Box::new(source.clone()),Box::new(after),1,None)).is_err());
            a.poll_jobs();assert_eq!(a.editor.project,source);assert!(a.tool_panel_rect().is_none());
        } else {
            tx.send(JobMessage::MeshToolReady(Box::new(source.clone()),Box::new(after.clone()),1,None)).unwrap();
            a.poll_jobs();assert_eq!(a.editor.project,after);assert_eq!(a.editor.undo_len(),0);
            a.action(Action::Author(A::ToolApply));assert_eq!(a.editor.undo_len(),1);a.action(Action::Undo);assert_eq!(a.editor.project,source);
        }
    }
}

#[test]
fn merge_distance_subdivide_pointer_drag_updates_geometry_and_keeps_capture() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {for dpi in [1.,2.] {
        let mut a=app();a.preferences.theme=theme;a.scale=dpi;
        let before=a.editor.project.clone();a.action(Action::ModelingTool(M::Subdivide));
        a.ui=a.build_ui();let field=Action::Author(A::SubdivideCount);
        let rect=a.ui.hits.iter().find(|h|matches!(&h.action,Action::Slide(f,_,_)if **f==field)).unwrap().rect;
        let point=[rect[0]+rect[2]*0.5,rect[1]+rect[3]*0.5];
        a.mouse_move(point);a.mouse_down(MouseButton::Left);assert!(matches!(a.drag,Some(Drag::Slider(_))));
        a.mouse_move([point[0]+rect[2]*0.2,point[1]]);
        let count=a.authoring.workflow.subdivide_segments;assert!(count>2,"{}",a.status);
        assert_eq!(a.editor.project.meshes[0].face_count(),5+(count*count) as usize);
        assert!(matches!(a.drag,Some(Drag::Slider(_))));assert_eq!(a.editor.undo_len(),0);
        a.mouse_up(MouseButton::Left);assert_eq!(a.editor.undo_len(),0);
        a.action(Action::Author(A::ToolApply));assert_eq!(a.editor.undo_len(),1);
        a.action(Action::Undo);assert_eq!(a.editor.project,before);
    }}
}

#[test]
fn merge_distance_pointer_drag_reset_and_entry_keep_array_preview_pending() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {for dpi in [1.,2.] {
        let mut a=app();a.preferences.theme=theme;a.scale=dpi;a.height=1400.;
        a.authoring.faces=(0..6).collect();let before=a.editor.project.clone();
        a.action(Action::ModelingTool(M::Array));
        a.apply_field_value(Action::Author(A::LiveField(0)),"2").unwrap();
        a.apply_field_value(Action::Author(A::LiveField(1)),"2.02").unwrap();
        click(&mut a,Action::Author(A::ToolMerge));
        let field=Action::Author(A::ToolMergeDistance);a.ui=a.build_ui();
        let rect=a.ui.hits.iter().find(|h|matches!(&h.action,Action::Slide(f,_,_)if **f==field)).unwrap().rect;
        let p=[rect[0]+rect[2]*0.5,rect[1]+rect[3]*0.5];
        a.mouse_move(p);a.mouse_down(MouseButton::Left);
        a.mouse_move([p[0]+rect[2]*0.04,p[1]]);
        assert!(a.tool_merge_distance()>0.02,"{}",a.status);
        assert_eq!(a.editor.project.meshes[0].face_count(),10);
        assert!(matches!(a.drag,Some(Drag::Slider(_))));
        a.mouse_move([p[0]+rect[2]*0.005,p[1]]);
        assert_eq!(a.editor.project.meshes[0].face_count(),12);
        a.mouse_up(MouseButton::Left);
        assert!(a.modeling_live.is_some());assert!(a.editor.has_active_edit());assert_eq!(a.editor.undo_len(),0);
        let zoom=a.distance;let old=a.tool_merge_distance();a.ui=a.build_ui();a.scroll(1.);
        assert!(a.tool_merge_distance()>old);assert_eq!(a.distance,zoom);
        click(&mut a,Action::ResetNumber(Box::new(field.clone())));
        assert_eq!(a.tool_merge_distance(),super::modeling_workflow::DEFAULT_MERGE_DISTANCE);
        a.mouse=point(&mut a,field.clone());a.click_ui_control();a.input.as_mut().unwrap().select_all();a.input.as_mut().unwrap().insert("0.03");
        a.action(Action::ConfirmInput);assert!(a.input.is_none());
        assert_eq!(a.editor.project.meshes[0].face_count(),10);
        let preview=a.editor.project.clone();
        assert!(a.authoring.faces.iter().all(|&f|f<10));assert_eq!(a.authoring.faces.len(),10);
        a.mouse=point(&mut a,field);a.click_ui_control();a.focus_next_field(false);
        assert!(matches!(a.input.as_ref().unwrap().purpose,Action::Author(A::LiveField(_))));
        a.action(Action::ConfirmInput);
        click(&mut a,Action::Author(A::LiveAccept));assert!(a.tool_panel_rect().is_none());assert_eq!(a.editor.undo_len(),1);
        a.action(Action::Undo);assert_eq!(a.editor.project,before);a.action(Action::Redo);assert_eq!(a.editor.project,preview);
    }}
}

#[test]
fn merge_distance_bevel_inset_cancel_and_per_tool_values() {
    let mut a=app();let before=a.editor.project.clone();
    for (tool,value) in [(M::Bevel,0.2),(M::Inset,0.3),(M::Array,0.03)] {
        a.action(Action::ModelingTool(tool));assert_eq!(a.tool_merge_distance(),0.001);
        click(&mut a,Action::Author(A::ToolMerge));
        let field=Action::Author(A::ToolMergeDistance);
        a.apply_field_value(field.clone(),&value.to_string()).unwrap();
        let result=a.editor.project.clone();
        for invalid in ["-1","nan","10001"] {
            assert!(a.apply_field_value(field.clone(),invalid).is_err());assert_eq!(a.editor.project,result);
        }
        click(&mut a,Action::Author(A::ToolMerge));
        assert!(!a.build_ui().hits.iter().any(|h|h.action==field));
        click(&mut a,Action::Author(A::ToolMerge));assert_eq!(a.editor.project,result);
        a.mouse=point(&mut a,field);a.click_ui_control();a.input.as_mut().unwrap().select_all();a.input.as_mut().unwrap().insert("nan");
        a.action(Action::Author(A::LiveCancel));assert!(a.input.is_none());assert!(a.tool_panel_rect().is_none());
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),0);
    }
    for (tool,value) in [(M::Bevel,0.2),(M::Inset,0.3),(M::Array,0.03)] {
        a.action(Action::ModelingTool(tool));assert_eq!(a.tool_merge_distance(),value);assert!(a.tool_merge_enabled());
        a.action(Action::Author(A::LiveCancel));
    }
}

#[test]
fn merge_distance_subdivide_fine_drag_rounds_to_whole_steps_and_escape_restores() {
    let mut a=app();let before=a.editor.project.clone();a.action(Action::ModelingTool(M::Subdivide));
    a.ui=a.build_ui();let preview=a.editor.project.clone();
    let field=Action::Author(A::SubdivideCount);
    let rect=a.ui.hits.iter().find(|h|matches!(&h.action,Action::Slide(f,_,_)if **f==field)).unwrap().rect;
    let p=[rect[0]+rect[2]*0.5,rect[1]+rect[3]*0.5];
    a.mouse_move(p);a.mouse_down(MouseButton::Left);a.modifiers=ModifiersState::SHIFT;
    a.mouse_move([p[0]+rect[2]*0.4,p[1]]);
    assert!(a.authoring.workflow.subdivide_segments>2,"{}",a.status);
    assert_eq!(a.editor.project.meshes[0].face_count(),5+a.authoring.workflow.subdivide_segments.pow(2) as usize);
    a.action(Action::Cancel);assert!(a.drag.is_none());assert_eq!(a.authoring.workflow.subdivide_segments,2);
    assert_eq!(a.editor.project,preview);assert!(a.authoring.workflow.session.is_some());
    a.modifiers=ModifiersState::empty();a.action(Action::Author(A::ToolCancel));assert_eq!(a.editor.project,before);
}

#[test]
fn merge_distance_region_bevel_distance_accepts_signed_values_without_changing_on_apply() {
    for shift in ["-0.3","0.3"] {
        let mut a=app();a.authoring.faces=BTreeSet::from([0,2]);let before=a.editor.project.clone();
        a.action(Action::ModelingTool(M::Bevel));
        a.apply_field_value(Action::Author(A::LiveField(2)),"4").unwrap();
        a.apply_field_value(Action::Author(A::LiveField(3)),"-80").unwrap();
        a.apply_field_value(Action::Author(A::LiveField(1)),shift).unwrap();
        let preview=a.editor.project.clone();let mesh=&preview.meshes[0];let graph=poseforge_core::MeshConnectivity::new(mesh);
        assert!(graph.edge_faces.values().any(|adj|adj.contains(&0)&&adj.contains(&2)));
        let report=poseforge_core::inspect_topology(mesh);assert_eq!((report.boundary_edges,report.nonmanifold_edges,report.winding_edges),(0,0,0));
        click(&mut a,Action::Author(A::LiveAccept));assert_eq!(a.editor.project,preview);assert!(a.tool_panel_rect().is_none());assert_eq!(a.editor.undo_len(),1);
        a.action(Action::Undo);assert_eq!(a.editor.project,before);
    }
}

#[test]
fn merge_distance_small_panels_scroll_to_confirmation_without_changing_preview() {
    for theme in [ui::Theme::Dark,ui::Theme::Light] {for dpi in [1.,2.] {for tool in [M::Array,M::Bevel,M::Inset] {
        let mut a=app();a.preferences.theme=theme;a.scale=dpi;a.width=900.;a.height=600.;
        let before=a.editor.project.clone();a.action(Action::ModelingTool(tool));
        // The checkbox can itself be below the initial clipped viewport at high DPI.
        a.action(Action::Author(A::ToolMerge));let preview=a.editor.project.clone();let zoom=a.distance;
        let rect=a.tool_panel_rect().unwrap();a.ui=a.build_ui();a.mouse=[rect[0]+8.,rect[1]+55.];
        a.scroll(-100.);assert_eq!(a.authoring.workflow.panel_scroll,a.tool_panel_scroll_max());
        for action in [Action::Author(A::LiveAccept),Action::Author(A::LiveCancel)] {
            let p=point(&mut a,action.clone());assert!(ui::contains(rect,p));assert_eq!(a.ui.hit(p),Some(action));
        }
        assert_eq!(a.distance,zoom);assert_eq!(a.editor.project,preview);
        click(&mut a,Action::Author(A::LiveCancel));assert_eq!(a.editor.project,before);assert!(a.tool_panel_rect().is_none());
    }}}
}
