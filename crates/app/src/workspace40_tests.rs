//! The actual controls retain context, preview ownership and undo boundaries.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S, ModelingTool as M};
use poseforge_core::{Node, PrimitiveKind, PrimitiveSettings};

fn app() -> App {
    let mut node = Node::new(1, "Cube"); node.mesh = Some(2);
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    a.preferences.animation_visible = false;
    a.width = 1100.; a.height = 760.;
    a.editor = Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2, None).unwrap()],
        ..Default::default()
    });
    a.editor.selected = Some(1); a.editor.mode = EditorMode::Scene;
    a.set_properties_tab(7); a.frame_all(); a
}
fn click(a: &mut App, action: Action) {
    for offset in 0..40 {
        a.inspector_scroll = (offset as f32 * 40.).min(a.inspector_max_scroll());
        a.ui = a.build_ui();
        if let Some(point) = a.ui.action_point(&action) {
            a.mouse_move(point); a.click_ui_control(); return;
        }
    }
    panic!("Control is not reachable: {action:?}");
}
#[test]
fn property_rail_is_vertical_in_order_and_creation_only_creates() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app(); a.scale = scale; a.preferences.theme = theme;
            a.set_properties_tab(0);
            let u = a.build_ui();
            let rows: Vec<_> = [6, 7, 0].into_iter().map(|tab| {
                u.hits.iter().find(|h| h.action == Action::SurfaceTab(tab)).unwrap().rect
            }).collect();
            assert!(rows.windows(2).all(|r| r[0][0] == r[1][0] && r[0][1] + r[0][3] <= r[1][1]));
            let creation = a.creation_toolbar_rect();
            assert_eq!(u.hits.iter().filter(|h| ui::contains(creation, [h.rect[0], h.rect[1]])).count(), 12);
            assert!(!u.hits.iter().any(|h| matches!(h.action, Action::Ik | Action::CopyPose | Action::PastePose | Action::Poses)));
            assert!(!u.texts.iter().any(|t| t.text.contains("Поза и действия")));
            a.set_properties_tab(6); a.properties_cycle(1); assert_eq!(a.surface.tab, 7);
            a.set_properties_tab(13); a.properties_cycle(1); assert_eq!(a.surface.tab, 7);
        }
    }
}
#[test]
fn icons_reveal_only_the_chosen_properties_without_changing_the_mesh() {
    for width in [280., 336., 440.] {
        let mut a = app(); a.preferences.right_width = Some(width);
        click(&mut a, Action::Author(A::SelectMode(S::Vertex)));
        let original = a.editor.project.clone();
        assert!(a.property_y(a.modeling_properties_start()).is_none());
        let mut u = Ui::new(1.); a.draw_authoring_panel(&mut u, 0., width - properties::RAIL_WIDTH);
        let tools: Vec<_> = u.hits.iter().filter(|h| matches!(h.action, Action::ModelingTool(_))).collect();
        assert_eq!(tools.len(), 28);
        for h in tools {
            assert!(h.action.hint().is_some());
            assert!(!u.texts.iter().any(|t| ui::contains(h.rect, [t.x, t.y])));
        }
        click(&mut a, Action::ModelingTool(M::Weld));
        u = Ui::new(1.); a.draw_authoring_panel(&mut u, 0., width - properties::RAIL_WIDTH);
        assert!(u.hits.iter().any(|h| h.action == Action::Author(A::WeldDistance)));
        assert!(!u.hits.iter().any(|h| h.action == Action::Author(A::BridgeSegments)));
        click(&mut a, Action::ModelingTool(M::Bridge));
        u = Ui::new(1.); a.draw_authoring_panel(&mut u, 0., width - properties::RAIL_WIDTH);
        assert!(u.hits.iter().any(|h| h.action == Action::Author(A::BridgeSegments)));
        assert!(!u.hits.iter().any(|h| h.action == Action::Author(A::WeldDistance)));
        assert_eq!(a.editor.project, original); assert_eq!(a.editor.undo_len(), 0);
    }
}
#[test]
fn collapsing_scrolling_and_numeric_entry_preserve_the_same_live_transaction() {
    let mut a = app();
    click(&mut a, Action::Author(A::SelectMode(S::Face)));
    a.authoring.faces.insert(0);
    let original = a.editor.project.clone(); let history = a.editor.undo_len();
    click(&mut a, Action::ModelingTool(M::Extrude));
    assert!(a.modeling_live.is_some());
    let preview = a.editor.project.clone();
    click(&mut a, Action::ToggleSection(modeling_palette::PROPERTIES_SECTION));
    assert!(a.modeling_live.is_some()); assert_eq!(a.editor.project, preview);
    assert!(a.property_y(a.modeling_properties_start()).is_none());
    click(&mut a, Action::ToggleSection(modeling_palette::PROPERTIES_SECTION));
    a.ui = a.build_ui(); a.mouse = [a.properties_content_rect()[0] + 40., 300.];
    a.scroll(-1.); assert!(a.modeling_live.is_some()); assert_eq!(a.editor.project, preview);
    click(&mut a, Action::Author(A::LiveField(0)));
    a.input.as_mut().unwrap().select_all(); a.input.as_mut().unwrap().insert("0.4");
    a.action(Action::ConfirmInput);
    assert_eq!(a.editor.undo_len(), history);
    click(&mut a, Action::Author(A::LiveAccept));
    assert_eq!(a.editor.undo_len(), history + 1);
    a.action(Action::Undo); assert_eq!(a.editor.project, original);
    a.authoring.faces = BTreeSet::from([0]);
    click(&mut a, Action::ModelingTool(M::Extrude));
    click(&mut a, Action::ModelingTool(M::Move));
    assert!(a.modeling_live.is_none()); assert_eq!(a.editor.project, original);
}
#[test]
fn invalid_numeric_entry_blocks_switching_tools_and_escape_retains_context() {
    let mut a = app();
    click(&mut a, Action::Author(A::SelectMode(S::Vertex)));
    click(&mut a, Action::ModelingTool(M::Weld));
    click(&mut a, Action::Author(A::WeldDistance));
    a.input.as_mut().unwrap().select_all(); a.input.as_mut().unwrap().insert("bad");
    a.action(Action::ModelingTool(M::Bridge));
    assert!(a.input.is_some()); assert_eq!(a.authoring.modeling_tool, Some(M::Weld));
    a.action(Action::Cancel); assert!(a.input.is_none());
    assert_eq!(a.authoring.modeling_tool, Some(M::Weld)); assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn mesh_modes_are_fixed_below_creation_and_divider_follows_geometry() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            for width in [280., 336., 440.] {
                let mut a = app();
                a.preferences.theme = theme; a.scale = scale;
                a.preferences.right_width = Some(width);
                for shown in [true, false] {
                    a.preferences.right_panel = shown;
                    for tab in [0, 1, 3, 6, 7, 8, 9] {
                        a.set_properties_tab(tab);
                        a.inspector_scroll = a.inspector_max_scroll();
                        let u = a.build_ui();
                        let toolbar = a.creation_toolbar_rect();
                        for action in [Action::Mode(0), Action::Author(A::SelectMode(S::Vertex)), Action::Author(A::SelectMode(S::Edge)), Action::Author(A::SelectMode(S::Face))] {
                            let hits: Vec<_> = u.hits.iter().filter(|h| h.action == action && ui::contains(toolbar, [h.rect[0], h.rect[1]])).collect();
                            assert_eq!(hits.len(), 1);
                            let rect = hits[0].rect;
                            assert!(rect[1] >= toolbar[1] + 38.);
                            assert!(rect[1] + rect[3] <= toolbar[1] + toolbar[3]);
                            assert!(rect[1] + rect[3] <= a.properties_content_rect()[1]);
                            assert!(!a.canvas_input_at([rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5]));
                        }
                        if shown {
                            let geometry = u.hits.iter().find(|h| h.action == Action::SurfaceTab(7)).unwrap().rect;
                            assert!(u.hits.iter().any(|h| h.action == Action::SurfaceTab(8)));
                            let object = u.hits.iter().find(|h| h.action == Action::SurfaceTab(0)).unwrap().rect;
                            assert!(object[1] - (geometry[1] + geometry[3]) >= 16.);
                            assert!(u.rects.iter().any(|r| r.segment.is_none() && r.rect[2] == 20. * scale && r.rect[3] == scale
                                && r.rect[0] == (toolbar[0] + 10.) * scale
                                && r.rect[1] > (geometry[1] + geometry[3]) * scale && r.rect[1] < object[1] * scale));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn one_bevel_button_uses_each_selection_context_and_cancels_on_mode_switch() {
    for (mode, fields) in [(S::Vertex, 1), (S::Edge, 3), (S::Face, 2)] {
        let mut a = app();
        click(&mut a, Action::Author(A::SelectMode(mode)));
        a.authoring.vertices = BTreeSet::from([0]);
        a.authoring.edges = BTreeSet::from([*poseforge_core::MeshConnectivity::new(&a.editor.project.meshes[0]).edges.first().unwrap()]);
        a.authoring.faces = BTreeSet::from([0]);
        let original = a.editor.project.clone();
        click(&mut a, Action::ModelingTool(M::Bevel));
        assert!(a.modeling_live.is_some(), "{}", a.status);
        assert_ne!(a.editor.project, original);
        let mut u = Ui::new(1.);
        a.draw_authoring_panel(&mut u, 0., 300.);
        assert_eq!(u.hits.iter().filter(|h| h.action == Action::ModelingTool(M::Bevel)).count(), 1);
        assert_eq!(u.hits.iter().filter(|h| matches!(h.action, Action::Author(A::LiveField(_)))).count(), fields);
        let target = if mode == S::Face { S::Vertex } else { S::Face };
        click(&mut a, Action::Author(A::SelectMode(target)));
        assert!(a.modeling_live.is_none()); assert_eq!(a.editor.project, original);
        assert_eq!(a.authoring.modeling_tool, Some(M::Bevel));
        assert_eq!(a.authoring.selection_mode, target); assert_eq!(a.editor.undo_len(), 0);
    }
}

#[test]
fn switching_component_types_keeps_transform_tool_and_selection() {
    for tool in [M::Move, M::Rotate, M::Scale] {
        let mut a = app();
        click(&mut a, Action::Author(A::SelectMode(S::Face)));
        a.authoring.faces = BTreeSet::from([0]);
        click(&mut a, Action::ModelingTool(tool));
        let original = a.editor.project.clone();
        let vertices = a.edit_vertices();
        for mode in [S::Edge, S::Vertex, S::Face] {
            click(&mut a, Action::Author(A::SelectMode(mode)));
            assert_eq!(a.authoring.modeling_tool, Some(tool));
            assert!(matches!((tool, a.tool), (M::Move, Tool::Move) | (M::Rotate, Tool::Rotate) | (M::Scale, Tool::Scale)));
            let mesh = &a.editor.project.meshes[0];
            let positions = |vs: BTreeSet<usize>| vs.into_iter().map(|i| poseforge_core::position_key(mesh.vertices[i].position)).collect::<BTreeSet<_>>();
            assert_eq!(positions(a.edit_vertices()), positions(vertices.clone()));
            assert_eq!(a.editor.project, original); assert_eq!(a.editor.undo_len(), 0);
        }
        click(&mut a, Action::Mode(0)); assert!(a.authoring.editing.is_none());
    }
}
