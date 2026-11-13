use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S, ModelingTool as M, ShapeAction};
use poseforge_core::{Node, PrimitiveKind, PrimitiveSettings};

fn app() -> App {
    let mut a = App::new(None);
    let mut n = Node::new(1, "Model");
    n.mesh = Some(2);
    a.editor = Editor::new(Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    });
    a.preferences = Preferences {
        right_panel: true,
        ..Default::default()
    };
    a.width = 1440.;
    a.height = 1100.;
    a.editor.mode = EditorMode::Scene;
    a.editor.selected = Some(1);
    a.frame_all();
    a.action(Action::Author(A::SelectMode(S::Vertex)));
    a
}
fn point(a: &mut App, action: Action) -> [f32; 2] {
    a.ui = a.build_ui();
    let max = a.inspector_max_scroll();
    for step in 0..=(max / 24.).ceil() as usize {
        a.inspector_scroll = (step as f32 * 24.).min(max);
        a.ui = a.build_ui();
        if let Some(h) =
            a.ui.hits
                .iter()
                .rev()
                .find(|h| h.action == action && h.rect[3] >= 24.)
        {
            return [h.rect[0] + h.rect[2] * 0.5, h.rect[1] + h.rect[3] * 0.5];
        }
    }
    panic!("Missing {action:?}");
}
fn click(a: &mut App, action: Action) {
    a.mouse = point(a, action);
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
}

#[test]
fn component_extrude_and_inset_work_from_vertices_with_atomic_preview_and_cancel() {
    for command in [A::Extrude, A::Inset] {
        let mut a = app();
        a.authoring.vertices = BTreeSet::from([0]);
        let before = a.editor.project.clone();
        a.action(Action::Author(command));
        assert!(a.modeling_live.is_some(), "{}", a.status);
        assert_ne!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), 0);
        a.action(Action::Cancel);
        assert_eq!(a.editor.project, before);
        assert_eq!(a.authoring.vertices, BTreeSet::from([0]));
        a.action(Action::Author(command));
        a.action(Action::Author(A::LiveAccept));
        assert_eq!(a.editor.undo_len(), 1);
        a.editor.project.validate().unwrap();
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
    }
    let mut a = app();
    a.authoring.vertices = a.editor.project.meshes[0]
        .face_vertices(0)
        .unwrap()
        .iter()
        .map(|&v| v as usize)
        .collect();
    let selection = a.authoring.vertices.clone();
    a.action(Action::Author(A::Inset));
    assert!(a.modeling_live.is_some(), "{}", a.status);
    assert_eq!(a.authoring.selection_mode, S::Face);
    assert_eq!(a.authoring.faces, BTreeSet::from([0]));
    a.action(Action::Cancel);
    assert_eq!(a.authoring.selection_mode, S::Vertex);
    assert_eq!(a.authoring.vertices, selection);
}
#[test]
fn component_face_slide_and_selected_subdivision_use_only_the_active_selection() {
    let mut a = app();
    a.action(Action::Author(A::SelectMode(S::Face)));
    a.authoring.faces = BTreeSet::from([0]);
    let before = a.editor.project.clone();
    let selected = a.edit_vertices();
    a.action(Action::Author(A::Slide));
    assert!(a.modeling_live.is_some(), "{}", a.status);
    a.apply_field_value(Action::Author(A::LiveField(0)), "25")
        .unwrap();
    assert_ne!(a.editor.project, before);
    let selected_positions: BTreeSet<_> = selected
        .iter()
        .map(|&v| poseforge_core::position_key(before.meshes[0].vertices[v].position))
        .collect();
    for (old, new) in before.meshes[0]
        .vertices
        .iter()
        .zip(&a.editor.project.meshes[0].vertices)
    {
        if !selected_positions.contains(&poseforge_core::position_key(old.position)) {
            assert_eq!(old.position, new.position);
        }
    }
    a.action(Action::Cancel);
    assert_eq!(a.editor.project, before);
    a.action(Action::Author(A::Subdivide));
    assert_eq!(a.editor.project.meshes[0].face_count(), 9);
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.authoring.faces.clear();
    a.action(Action::Author(A::Subdivide));
    assert_eq!(a.editor.project, before);
}
#[test]
fn component_delete_uses_vertex_edge_or_face_mode() {
    for mode in [S::Vertex, S::Edge, S::Face] {
        let mut a = app();
        a.action(Action::Author(A::SelectMode(mode)));
        match mode {
            S::Vertex => {
                a.authoring.vertices.insert(0);
            }
            S::Edge => {
                a.authoring.edges.insert(
                    *a.mesh_cache(&a.editor.project.meshes[0])
                        .graph
                        .edges
                        .first()
                        .unwrap(),
                );
            }
            S::Face => {
                a.authoring.faces.insert(0);
            }
        }
        let before = a.editor.project.clone();
        a.action(Action::Author(A::DeleteFaces));
        assert_ne!(a.editor.project, before, "{mode:?}: {}", a.status);
        a.editor.project.validate().unwrap();
        assert_eq!(a.editor.undo_len(), 1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
    }
}
#[test]
fn component_modifiers_tab_opens_from_mesh_mode_and_exposes_array_parameters() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let mut a = app();
        a.preferences.theme = theme;
        let before = a.editor.project.clone();
        click(&mut a, Action::SurfaceTab(8));
        assert_eq!(a.surface.tab, 8);
        assert_eq!(a.editor.project, before);
        click(&mut a, Action::Author(A::ToggleEdit));
        assert!(a.authoring.editing.is_none());
        assert_eq!(a.surface.tab, 8);
        click(&mut a, Action::DropdownOpen(ui::DropdownKind::ModifierAdd));
        assert!(a.dropdown.is_some());
        a.action(Action::DropdownChoose(1));
        let modifier = a.editor.project.node(1).unwrap().modifiers[0].clone();
        assert_eq!(
            modifier.settings.kind(),
            poseforge_core::ModifierKind::Array
        );
        for field in 1..=4 {
            let _ = point(
                &mut a,
                Action::Modifier(ui::ModifierAction::Field(1, modifier.id, field)),
            );
        }
        a.apply_field_value(
            Action::Modifier(ui::ModifierAction::Field(1, modifier.id, 1)),
            "4",
        )
        .unwrap();
        assert!(matches!(
            a.editor.project.node(1).unwrap().modifiers[0].settings,
            poseforge_core::ModifierSettings::Array { count: 4, .. }
        ));
        a.action(Action::Undo);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
    }
}
#[test]
fn component_alignment_is_one_tool_with_three_functional_modes() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for width in [900., 1440.] {
            let mut a = app();
            a.width = width;
            a.preferences.theme = theme;
            a.authoring.vertices = (0..a.editor.project.meshes[0].vertices.len()).collect();
            a.action(Action::ModelingTool(M::Align));
            let before = a.editor.project.clone();
            for mode in 0..3 {
                click(&mut a, Action::DropdownOpen(ui::DropdownKind::Alignment));
                a.action(Action::DropdownChoose(mode));
                assert_eq!(a.mesh_tools.alignment_mode, mode as u8);
                assert_eq!(a.editor.project, before);
                let command = match mode {
                    0 => ShapeAction::Flatten(1),
                    1 => ShapeAction::Straighten,
                    _ => ShapeAction::Space,
                };
                let _ = point(&mut a, Action::Shape(command));
            }
            assert_eq!(a.authoring.modeling_tool, Some(M::Align));
        }
    }
}
#[test]
fn component_knife_three_clicks_continue_across_two_faces_in_one_transaction() {
    let mut a = app();
    let mut settings = PrimitiveSettings::new(PrimitiveKind::Plane);
    settings.segments = [2, 1, 1];
    a.editor.project.meshes[0] = settings.mesh(2, None).unwrap();
    a.authoring.xray = true;
    a.frame_all();
    let original = a.editor.project.clone();
    let m = &original.meshes[0];
    let graph = poseforge_core::MeshConnectivity::new(m);
    let shared = *graph
        .edge_faces
        .iter()
        .find(|(_, f)| f.len() == 2)
        .unwrap()
        .0;
    let mut path = Vec::new();
    for f in 0..2 {
        let face = m.face_vertices(f).unwrap();
        let j = (0..4)
            .find(|&i| {
                let x = graph.canonical[face[i] as usize];
                let y = graph.canonical[face[(i + 1) % 4] as usize];
                (x.min(y), x.max(y)) == shared
            })
            .unwrap();
        let p = |v: u32| Vec3::from_array(m.vertices[v as usize].position);
        if f == 0 {
            path.push((p(face[(j + 2) % 4]) + p(face[(j + 3) % 4])) * 0.5);
            path.push((p(face[j]) + p(face[(j + 1) % 4])) * 0.5);
        } else {
            path.push((p(face[(j + 2) % 4]) + p(face[(j + 3) % 4])) * 0.5);
        }
    }
    for accept in [false, true] {
        a.action(Action::Author(A::Knife));
        for p in &path {
            a.mouse = a.project_point(*p).unwrap();
            a.knife_click();
        }
        assert_eq!(a.editor.project.meshes[0].face_count(), 4, "{}", a.status);
        assert_eq!(a.editor.undo_len(), 0);
        if accept {
            a.action(Action::Author(A::KnifeAccept));
            assert_eq!(a.editor.undo_len(), 1);
            a.action(Action::Undo);
        } else {
            a.action(Action::Cancel);
        }
        assert_eq!(a.editor.project, original);
    }
}

#[test]
fn component_zero_extrusion_preserves_selection_and_history() {
    let mut a = app();
    a.authoring.vertices = BTreeSet::from([0]);
    let before = a.editor.project.clone();
    a.action(Action::Author(A::Extrude));
    a.apply_field_value(Action::Author(A::LiveField(0)), "0")
        .unwrap();
    assert_eq!(a.authoring.vertices, BTreeSet::from([0]));
    a.accept_modeling_live();
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
    let mut mesh = a.editor.project.meshes[0].clone();
    let graph = a.mesh_cache(&mesh);
    let b = (1..mesh.vertices.len())
        .find(|&b| graph.graph.canonical[b] == b && !graph.graph.edges.contains(&(0, b)))
        .unwrap();
    mesh.topology
        .as_mut()
        .unwrap()
        .loose_edges
        .push([0, b as u32]);
    let updated = a.mesh_cache(&mesh);
    assert!(!Arc::ptr_eq(&graph, &updated));
    assert!(updated.graph.edges.contains(&(0, b)));
}

#[test]
#[ignore = "requires ARTFORGE_UI_CAPTURE_DIR; exports actual UI primitives for CPU visual review"]
fn component_modeling_capture_ui() {
    let directory =
        PathBuf::from(std::env::var_os("ARTFORGE_UI_CAPTURE_DIR").expect("capture directory"));
    std::fs::create_dir_all(&directory).unwrap();
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for width in [900., 1440.] {
            for case in ["align", "modifiers", "array", "bridge", "extrude"] {
                let mut a = app();
                a.width = width;
                a.preferences.theme = theme;
                if matches!(case, "modifiers" | "array") {
                    a.action(Action::SurfaceTab(8));
                    if case == "array" {
                        a.action(Action::Author(A::ToggleEdit));
                        a.mouse =
                            point(&mut a, Action::DropdownOpen(ui::DropdownKind::ModifierAdd));
                        a.action(Action::DropdownOpen(ui::DropdownKind::ModifierAdd));
                        a.action(Action::DropdownChoose(1));
                    }
                } else if case == "align" {
                    a.action(Action::ModelingTool(M::Align));
                    a.mouse = point(&mut a, Action::DropdownOpen(ui::DropdownKind::Alignment));
                    a.action(Action::DropdownOpen(ui::DropdownKind::Alignment));
                } else if case == "bridge" {
                    a.action(Action::Author(A::SelectMode(S::Face)));
                    a.authoring.faces = BTreeSet::from([0, 1]);
                    a.action(Action::ModelingTool(M::BridgeFaces));
                } else {
                    a.authoring.vertices = BTreeSet::from([0]);
                    a.action(Action::Author(A::Extrude));
                }
                let u = a.build_ui();
                let data = serde_json::json!({"width":a.width,"height":a.height,"background":u.palette().viewport,
                    "source":"Actual App::build_ui output; CPU preview, not a native window capture",
                    "rectangles":u.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),
                    "texts":u.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
                let name = if theme == ui::Theme::Dark {
                    "dark"
                } else {
                    "light"
                };
                std::fs::write(
                    directory.join(format!("modeling-{case}-{name}-{}.json", width as u32)),
                    serde_json::to_vec(&data).unwrap(),
                )
                .unwrap();
            }
        }
    }
}
