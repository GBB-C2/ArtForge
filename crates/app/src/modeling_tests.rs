use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S};
use poseforge_core::{Node, PrimitiveKind, PrimitiveSettings};
fn app(kind: PrimitiveKind) -> App {
    let mut n = Node::new(1, "Model");
    n.mesh = Some(2);
    let mut a = App::new(None);
    a.editor = Editor::new(Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(kind).mesh(2, None).unwrap()],
        ..Default::default()
    });
    a.editor.selected = Some(1);
    a.editor.mode = EditorMode::Scene;
    a.preferences = Preferences::default();
    a.width = 1440.;
    a.height = 900.;
    a.target = Vec3::ZERO;
    a.frame_all();
    a.action(Action::Author(A::SelectMode(S::Vertex)));
    a
}
fn click(a: &mut App, action: Action) {
    use crate::ui::ModelingTool as M;
    let action = match action {
        Action::Author(A::Bevel) => Action::ModelingTool(M::Bevel),
        Action::Author(A::EdgeBevel) => Action::ModelingTool(M::Bevel),
        Action::Author(A::LoopCut) => Action::ModelingTool(M::LoopCut),
        other => other,
    };
    let context = match action {
        Action::Author(A::Grow | A::Shrink | A::Invert | A::Connected) => Some(M::Select(0)),
        Action::Author(A::RotateDegrees(_)) => Some(M::Rotate),
        Action::Author(A::Weld) => Some(M::Weld),
        Action::Author(A::Inspect) => Some(M::Inspect),
        _ => None,
    };
    if let Some(tool) = context { a.action(Action::ModelingTool(tool)); }
    for step in 0..24 {
        a.inspector_scroll = (step as f32 * 70.).min(a.inspector_max_scroll());
        a.ui = a.build_ui();
        if let Some(r) =
            a.ui.hits
                .iter()
                .rev()
                .find(|h| h.action == action)
                .map(|h| h.rect)
        {
            a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
            a.click_ui_control();
            return;
        }
    }
    panic!("control unavailable: {action:?}");
}
#[test]
fn mesh_cache_reuses_adjacency_and_dense_overlay_bounds_gpu_instances() {
    let mut a = app(PrimitiveKind::Sphere);
    let mut settings = PrimitiveSettings::new(PrimitiveKind::Sphere);
    settings.segments = [48; 3];
    a.editor.project.meshes[0] = settings.mesh(2, None).unwrap();
    let mesh = &a.editor.project.meshes[0];
    let first = a.mesh_cache(mesh);
    let second = a.mesh_cache(mesh);
    assert!(Arc::ptr_eq(&first, &second));
    a.authoring.selection_mode = S::Face;
    let start = Instant::now();
    let mut u = Ui::new(1.);
    a.draw_mesh_overlay(&mut u);
    let edges = first.graph.edges.len();
    let lines = u.rects.iter().filter(|r| r.segment.is_some()).count();
    let shown = edges.div_ceil(edges.div_ceil(24_000).max(1));
    assert!(lines > shown / 2 && lines <= shown);
    assert!(lines <= 24_000);
    assert!(u.rects.len() < edges + 1000);
    eprintln!(
        "Dense mesh: {} faces, {} edges, {} UI instances, {:?} CPU overlay",
        mesh.face_count(),
        edges,
        u.rects.len(),
        start.elapsed()
    );
    a.editor
        .edit_mesh(
            1,
            poseforge_core::MeshEdit::Transform {
                vertices: BTreeSet::from([0]),
                matrix: Mat4::from_translation(Vec3::Y * 0.02),
            },
        )
        .unwrap();
    let changed = a.mesh_cache(&a.editor.project.meshes[0]);
    assert!(!Arc::ptr_eq(&first, &changed));
}
#[test]
fn edge_selection_handles_a_high_valence_fan_without_pairwise_neighbor_allocation() {
    let mut a = app(PrimitiveKind::Cube);
    let count = 10_000usize;
    let mut vertices = vec![poseforge_core::Vertex::default()];
    for i in 0..count {
        let angle = i as f32 * std::f32::consts::TAU / count as f32;
        vertices.push(poseforge_core::Vertex {
            position: [angle.cos(), 0., angle.sin()],
            ..Default::default()
        });
    }
    let mesh = &mut a.editor.project.meshes[0];
    mesh.vertices = vertices.into();
    mesh.indices = (0..count)
        .flat_map(|i| [0, (i + 1) as u32, ((i + 1) % count + 1) as u32])
        .collect();
    mesh.topology = None;
    a.authoring.selection_mode = S::Edge;
    a.authoring.edges = BTreeSet::from([(0, 1)]);
    a.action(Action::Author(A::Grow));
    assert_eq!(a.authoring.edges.len(), count + 2);
    a.action(Action::Author(A::Shrink));
    assert_eq!(a.authoring.edges, BTreeSet::from([(0, 1)]));
    a.action(Action::Author(A::Connected));
    assert_eq!(a.authoring.edges.len(), count * 2);
    a.action(Action::Author(A::Invert));
    assert!(a.authoring.edges.is_empty());
}
#[test]
fn xray_controls_hidden_vertex_selection_in_both_projections() {
    for ortho in [false, true] {
        let mut a = app(PrimitiveKind::Cube);
        a.orthographic = ortho;
        a.frame_all();
        a.ui = a.build_ui();
        let cache = a.mesh_cache(&a.editor.project.meshes[0]);
        let mesh = a.editor.project.meshes[0].clone();
        let mut hidden = None;
        for (i, v) in mesh.vertices.iter().enumerate() {
            if cache.graph.canonical[i] != i {
                continue;
            }
            let world = Vec3::from_array(v.position);
            let Some(point) = a.project_point(world) else {
                continue;
            };
            if !a.canvas_input_at(point) {
                continue;
            }
            a.authoring.vertices.clear();
            a.authoring.xray = false;
            a.mouse_move(point);
            a.mesh_pointer_down();
            let picked = a.authoring.vertices.contains(&i);
            a.cancel_drag();
            if !picked {
                hidden = Some((i, point));
                break;
            }
        }
        let (i, point) = hidden.expect("cube has an occluded vertex");
        a.authoring.vertices.clear();
        a.action(Action::Author(A::Xray));
        a.mouse_move(point);
        a.mesh_pointer_down();
        assert!(a.authoring.vertices.contains(&i));
        a.cancel_drag();
    }
}
#[test]
fn rotate_scale_drag_is_absolute_cancellable_and_one_undo_with_precision_switch() {
    for tool in [Tool::Rotate, Tool::Scale] {
        for ortho in [false, true] {
            let mut a = app(PrimitiveKind::Cube);
            a.orthographic = ortho;
            a.frame_all();
            a.action(Action::Author(A::SelectAll));
            a.tool = tool;
            a.authoring.constraint = Some(2);
            a.mouse = [720., 450.];
            let before = a.editor.project.clone();
            let count = a.editor.undo_len();
            a.begin_mesh_drag();
            a.mouse_move([750., 440.]);
            let coarse = a.editor.project.clone();
            assert_ne!(coarse, before);
            a.modifiers = ModifiersState::SHIFT;
            a.mouse_move([750., 440.]);
            assert_eq!(a.editor.project, coarse);
            a.mouse_move([780., 440.]);
            a.action(Action::Cancel);
            assert_eq!(a.editor.project, before);
            assert_eq!(a.editor.undo_len(), count);
            a.modifiers = ModifiersState::empty();
            a.mouse = [720., 450.];
            a.begin_mesh_drag();
            a.mouse_move([760., 450.]);
            a.mouse_up(MouseButton::Left);
            assert_eq!(a.editor.undo_len(), count + 1);
            a.action(Action::Undo);
            assert_eq!(a.editor.project, before);
            a.action(Action::Redo);
            a.editor.project.validate().unwrap();
        }
    }
}
#[test]
fn selection_tools_and_polygon_commands_work_from_scrolled_panels_in_both_themes() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app(PrimitiveKind::Cube);
            a.preferences.theme = theme;
            a.scale = scale;
            a.width = 1100.;
            a.height = 620.;
            a.action(Action::Author(A::SelectMode(S::Face)));
            a.authoring.faces.insert(0);
            click(&mut a, Action::Author(A::Grow));
            assert_eq!(a.authoring.faces.len(), 5);
            click(&mut a, Action::Author(A::Shrink));
            assert_eq!(a.authoring.faces, BTreeSet::from([0]));
            click(&mut a, Action::Author(A::Invert));
            assert_eq!(a.authoring.faces.len(), 5);
            click(&mut a, Action::Author(A::Connected));
            assert_eq!(a.authoring.faces.len(), 6);
            a.authoring.faces = BTreeSet::from([0]);
            click(&mut a, Action::Author(A::Bevel));
            click(&mut a, Action::Author(A::LiveAccept));
            // Four edge strips with three segments, without extra side patches.
            assert_eq!(a.editor.project.meshes[0].face_count(), 18);
            let report=poseforge_core::inspect_topology(&a.editor.project.meshes[0]);
            assert_eq!((report.boundary_edges,report.nonmanifold_edges),(0,0));
            a.action(Action::Undo);
            a.action(Action::Author(A::SelectMode(S::Edge)));
            a.authoring.edges = BTreeSet::from([(0, 1)]);
            click(&mut a, Action::Author(A::LoopCut));
            a.authoring.xray=true;
            let mesh=a.editor.editable_mesh(1).unwrap();
            let midpoint=(Vec3::from_array(mesh.vertices[0].position)+Vec3::from_array(mesh.vertices[1].position))*0.5;
            a.mouse=a.project_point(midpoint).unwrap();a.knife_click();
            click(&mut a, Action::Author(A::KnifeAccept));
            assert_eq!(a.editor.project.meshes[0].face_count(), 10);
            a.editor.project.validate().unwrap();
            a.action(Action::Mode(0));
            a.ui = a.build_ui();
            assert!(!a
                .ui
                .texts
                .iter()
                .any(|t| t.text.contains("Редактировать меш")));
        }
    }
}
#[test]
fn exact_transform_fields_validate_keep_focus_and_respect_pivot() {
    let mut a = app(PrimitiveKind::Cube);
    a.action(Action::Author(A::SelectAll));
    a.authoring_confirm(&Action::Author(A::ScaleFactor(0)), "2")
        .unwrap();
    assert!(a.editor.project.meshes[0]
        .vertices
        .iter()
        .all(|v| v.position[0].abs() == 2.));
    let before = a.editor.project.clone();
    let count = a.editor.undo_len();
    click(&mut a, Action::Author(A::RotateDegrees(2)));
    let input = a.input.as_mut().unwrap();
    input.select_all();
    input.insert("NaN");
    a.action(Action::ConfirmInput);
    assert!(a.input.is_some());
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), count);
    a.action(Action::Cancel);
    a.authoring_confirm(&Action::Author(A::RotateDegrees(2)), "90")
        .unwrap();
    assert!((a.editor.project.meshes[0].vertices[0].position[1].abs() - 2.).abs() < 1e-5);
}

#[test]
fn live_edge_bevel_handles_fields_cancel_and_single_undo_in_both_themes() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app(PrimitiveKind::Cube);
            a.preferences.theme = theme;
            a.scale = scale;
            a.authoring.selection_mode = S::Edge;
            a.authoring.edges = BTreeSet::from([(0, 1)]);
            let before = a.editor.project.clone();
            let selected = a.authoring.edges.clone();
            let history = a.editor.undo_len();
            click(&mut a, Action::Author(A::EdgeBevel));
            assert!(a.modeling_live.is_some());
            assert_eq!(a.editor.undo_len(), history);
            click(&mut a, Action::Author(A::LiveField(1)));
            a.input.as_mut().unwrap().select_all();
            a.input.as_mut().unwrap().insert("3.5");
            a.action(Action::ConfirmInput);
            assert!(a.input.is_some());
            assert_eq!(a.editor.undo_len(), history);
            a.input.as_mut().unwrap().select_all();
            a.input.as_mut().unwrap().insert("4");
            a.action(Action::ConfirmInput);
            assert!(a.input.is_none());
            a.ui = a.build_ui();
            a.mouse_move(a.ui.action_point(&Action::Author(A::LiveDrag(0))).unwrap());
            a.mouse_down(MouseButton::Left);
            assert!(matches!(a.drag, Some(Drag::LiveParameter(_))));
            a.mouse_move([a.mouse[0] + 8., a.mouse[1]]);
            a.mouse_up(MouseButton::Left);
            assert!(a.modeling_live.is_some());
            assert_eq!(a.editor.undo_len(), history);
            click(&mut a, Action::Author(A::LiveCancel));
            assert_eq!(a.editor.project, before);
            assert_eq!(a.authoring.edges, selected);
            click(&mut a, Action::Author(A::EdgeBevel));
            click(&mut a, Action::Author(A::LiveAccept));
            assert_eq!(a.editor.undo_len(), history + 1);
            a.editor.project.validate().unwrap();
            a.action(Action::Undo);
            assert_eq!(a.editor.project, before);
            a.action(Action::Redo);
            assert_ne!(a.editor.project, before);
        }
    }
}
#[test]
fn live_face_tools_and_loop_preview_from_frozen_mesh_and_cancel_on_tool_change() {
    for command in [A::Extrude, A::Inset, A::Bevel, A::LoopCut] {
        let mut a = app(PrimitiveKind::Cube);
        let before = a.editor.project.clone();
        if command == A::LoopCut {
            a.authoring.selection_mode = S::Edge;
            a.authoring.edges = BTreeSet::from([(0, 1)]);
        } else {
            a.authoring.selection_mode = S::Face;
            a.authoring.faces = BTreeSet::from([0]);
        }
        if command==A::LoopCut {a.modeling_live_action(command);}else{a.action(Action::Author(command));}
        assert!(a.modeling_live.is_some(), "{command:?}: {}", a.status);
        a.authoring_confirm(
            &Action::Author(A::LiveField(0)),
            if command == A::Extrude { "0.25" } else { "25" },
        )
        .unwrap();
        let once = a.editor.project.clone();
        a.authoring_confirm(
            &Action::Author(A::LiveField(0)),
            if command == A::Extrude { "0.5" } else { "40" },
        )
        .unwrap();
        a.authoring_confirm(
            &Action::Author(A::LiveField(0)),
            if command == A::Extrude { "0.25" } else { "25" },
        )
        .unwrap();
        assert_eq!(a.editor.project, once);
        let preview = a.editor.project.clone();
        assert!(a
            .authoring_confirm(&Action::Author(A::LiveField(0)), "NaN")
            .is_err());
        assert_eq!(a.editor.project, preview);
        a.action(Action::Move);
        assert!(a.modeling_live.is_none());
        assert!(!a.editor.has_active_edit());
        assert_eq!(a.editor.project, before);
    }
}
#[test]
fn parameter_grips_stay_reachable_and_cancel_after_fine_drag_in_small_and_uv_views() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for dpi in [1., 2.] {
            for (width, height, uv) in [
                (900., 600., false),
                (1440., 900., false),
                (1440., 900., true),
            ] {
                let mut a = app(PrimitiveKind::Cube);
                a.width = width;
                a.height = height;
                a.scale = dpi;
                a.preferences.theme = theme;
                if uv {
                    a.action(Action::Uv(ui::UvAction::Open));
                }
                // A 1440px framing can put the selected edge offscreen after
                // resizing to 900px. These cases exercise visible geometry.
                a.frame_all();
                a.action(Action::Author(A::SelectMode(S::Edge)));
                a.authoring.edges = BTreeSet::from([(0, 1)]);
                a.action(Action::Author(A::EdgeBevel));
                let preview = a.editor.project.clone();
                let history = a.editor.undo_len();
                a.ui = a.build_ui();
                let area = a.modeling_area();
                let grips: Vec<_> =
                    a.ui.hits
                        .iter()
                        .filter(|h| matches!(h.action, Action::Author(A::LiveDrag(_))))
                        .cloned()
                        .collect();
                assert_eq!(grips.len(), 1, "{width} x {height}, UV {uv}");
                for h in &grips {
                    let r = h.rect;
                    assert!(r[2] >= 16. && r[3] >= 16.);
                    assert!(r[0] >= area[0] && r[1] >= area[1]);
                    assert!(r[0] + r[2] <= area[0] + area[2] && r[1] + r[3] <= area[1] + area[3]);
                    assert_eq!(
                        a.ui.hit(a.ui.action_point(&h.action).unwrap()),
                        Some(h.action.clone())
                    );
                }
                for h in
                    a.ui.hits
                        .iter()
                        .filter(|h| matches!(h.action, Action::Author(A::LiveDrag(_))))
                {
                    for nav in a.ui.hits.iter().filter(|h| {
                        matches!(
                            h.action,
                            Action::ToggleViewMenu
                                | Action::ViewOrbit
                                | Action::CameraView
                                | Action::Studio(ui::StudioAction::CameraLock)
                        )
                    }) {
                        let (r, n) = (h.rect, nav.rect);
                        assert!(
                            r[0] + r[2] <= n[0]
                                || n[0] + n[2] <= r[0]
                                || r[1] + r[3] <= n[1]
                                || n[1] + n[3] <= r[1],
                            "Modeling control overlaps {:?} at {width} x {height}, UV {uv}",
                            nav.action
                        );
                    }
                }
                a.mouse_move(a.ui.action_point(&grips[0].action).unwrap());
                a.mouse_down(MouseButton::Left);
                a.mouse_move([a.mouse[0] + 12., a.mouse[1]]);
                assert_ne!(a.editor.project, preview);
                let before_fine = a.editor.project.clone();
                a.modifiers = ModifiersState::SHIFT;
                a.mouse_move(a.mouse);
                assert_eq!(a.editor.project, before_fine);
                a.mouse_move([a.mouse[0] + 4., a.mouse[1]]);
                a.cancel_drag();
                assert_eq!(a.editor.project, preview);
                assert_eq!(a.editor.undo_len(), history);
            }
        }
    }
}
#[test]
fn slide_slider_and_ctrl_drag_use_signed_percentages_independent_of_mesh_size() {
    for scale in [1., 10.] {
        let mut a = app(PrimitiveKind::Cube);
        for v in &mut a.editor.project.meshes[0].vertices {
            v.position = (Vec3::from_array(v.position) * scale).to_array();
        }
        a.invalidate_geometry();
        a.frame_all();
        a.authoring.vertices = BTreeSet::from([0]);
        let before = a.editor.project.clone();
        a.action(Action::Author(A::Slide));
        a.ui = a.build_ui();
        let range = a
            .ui
            .hits
            .iter()
            .find_map(|h| match &h.action {
                Action::Slide(field, range, _) if **field == Action::Author(A::LiveField(0)) => {
                    Some(*range)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!((range.min, range.max), (-99., 99.));
        a.modifiers = ModifiersState::CONTROL;
        a.mouse_move(a.ui.action_point(&Action::Author(A::LiveDrag(0))).unwrap());
        a.mouse_down(MouseButton::Left);
        a.mouse_move([a.mouse[0] - 28., a.mouse[1]]);
        a.mouse_up(MouseButton::Left);
        let mut expected = Editor::new(before.clone());
        expected
            .edit_mesh(
                1,
                poseforge_core::MeshEdit::Slide {
                    vertices: BTreeSet::from([0]),
                    edges: BTreeSet::new(),
                    amount: -0.05,
                },
            )
            .unwrap();
        assert_eq!(a.editor.project, expected.project);
        a.action(Action::Author(A::LiveCancel));
        assert_eq!(a.editor.project, before);
    }
}
#[test]
fn box_lasso_selection_capture_cancellation_and_subtraction() {
    for tool in [1, 2] {
        for mode in [S::Vertex, S::Edge, S::Face] {
            let mut a = app(PrimitiveKind::Cube);
            a.authoring.selection_mode = mode;
            a.authoring.xray = true;
            a.action(Action::Author(A::SelectTool(tool)));
            a.ui = a.build_ui();
            let v = a.viewport();
            let (x, y, w, h) = (v[0] + 4., v[1] + 4., v[2] - 8., v[3] - 8.);
            let before = a.editor.project.clone();
            a.mouse_move([x, y]);
            a.mouse_down(MouseButton::Left);
            assert!(matches!(a.drag, Some(Drag::ComponentSelection(_))));
            for p in [[x + w, y], [x + w, y + h], [x, y + h], [x, y]] {
                if tool == 2 {
                    a.mouse_move(p);
                }
            }
            if tool == 1 {
                a.mouse_move([x + w, y + h]);
            }
            a.mouse_up(MouseButton::Left);
            let count = match mode {
                S::Vertex => a.authoring.vertices.len(),
                S::Edge => a.authoring.edges.len(),
                S::Face => a.authoring.faces.len(),
            };
            assert_eq!(
                count,
                match mode {
                    S::Vertex => 8,
                    S::Edge => 12,
                    S::Face => 6,
                }
            );
            a.mouse_move([x, y]);
            a.mouse_down(MouseButton::Left);
            a.mouse_move([x + w, y + h]);
            a.action(Action::Cancel);
            let count_after = match mode {
                S::Vertex => a.authoring.vertices.len(),
                S::Edge => a.authoring.edges.len(),
                S::Face => a.authoring.faces.len(),
            };
            assert_eq!(count, count_after);
            a.modifiers = winit::keyboard::ModifiersState::CONTROL;
            a.mouse_move([x, y]);
            a.mouse_down(MouseButton::Left);
            for p in [[x + w, y], [x + w, y + h], [x, y + h], [x, y]] {
                if tool == 2 {
                    a.mouse_move(p);
                }
            }
            if tool == 1 {
                a.mouse_move([x + w, y + h]);
            }
            a.mouse_up(MouseButton::Left);
            assert!(a.edit_vertices().is_empty());
            assert_eq!(a.editor.project, before);
            assert_eq!(a.editor.undo_len(), 0);
        }
    }
}
#[test]
fn topology_controls_weld_validate_fields_and_report_real_geometry() {
    let mut a = app(PrimitiveKind::Cube);
    a.action(Action::Author(A::SelectAll));
    let before = a.editor.project.clone();
    click(&mut a, Action::Author(A::Weld));
    assert_eq!(a.editor.project.meshes[0].vertices.len(), 8);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    for (field, value) in [
        (A::WeldDistance, "0"),
        (A::WeldDistance, "NaN"),
        (A::BridgeSegments, "2.5"),
        (A::BridgeSegments, "33"),
    ] {
        assert!(a.authoring_confirm(&Action::Author(field), value).is_err());
    }
    click(&mut a, Action::Author(A::Inspect));
    let report = &a.authoring.diagnostics.as_ref().unwrap().1;
    assert_eq!(report.boundary_edges, 0);
    assert!(report.problem_faces.is_empty());
}

#[test]
fn all_edit_components_use_scene_depth_unless_xray_is_enabled() {
    for ortho in [false, true] {
        for scale in [1., 2.] {
            for mode in [S::Vertex, S::Edge, S::Face] {
                let mut a = app(PrimitiveKind::Cube);
                a.orthographic = ortho;
                a.scale = scale;
                a.authoring.selection_mode = mode;
                a.authoring.faces.insert(0);
                a.authoring.edges.insert((0, 1));
                let mut u = Ui::new(scale);
                a.draw_mesh_overlay(&mut u);
                let components: Vec<_> = u.rects.iter().filter_map(|r| r.depth).collect();
                assert!(components.len() >= 12);
                assert!(components
                    .iter()
                    .flatten()
                    .all(|z| z.is_finite() && (0. ..=1.).contains(z)));
                assert!(u
                    .rects
                    .iter()
                    .filter(|r| r.segment.is_some())
                    .all(|r| r.depth.is_some()));
                if mode == S::Vertex {
                    assert!(
                        u.rects
                            .iter()
                            .filter(|r| r.depth.is_some() && r.segment.is_none())
                            .count()
                            >= 8
                    );
                }
                a.authoring.xray = true;
                let mut xray = Ui::new(scale);
                a.draw_mesh_overlay(&mut xray);
                assert_eq!(u.rects.len(), xray.rects.len());
                assert!(xray.rects.iter().all(|r| r.depth.is_none()));
            }
        }
    }
}
