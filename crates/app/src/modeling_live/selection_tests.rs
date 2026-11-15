use super::*;
use crate::ui::ModelingTool as M;
use poseforge_core::{Node, PrimitiveKind, PrimitiveSettings};

fn app(mode: S) -> App {
    let mut node = Node::new(1, "Model");
    node.mesh = Some(2);
    let mut a = App::new(None);
    a.editor = Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    });
    a.preferences = Preferences {
        animation_visible: false,
        ..Default::default()
    };
    a.width = 1440.;
    a.height = 900.;
    a.editor.selected = Some(1);
    a.editor.mode = EditorMode::Scene;
    a.frame_all();
    a.action(Action::Author(A::SelectMode(mode)));
    a.authoring.xray = true;
    a.authoring.vertices = BTreeSet::from([0]);
    a.authoring.edges = BTreeSet::from([(0, 1)]);
    a.authoring.faces = BTreeSet::from([0]);
    a
}

// Choose a real projected component, clear of floating UI and navigation controls.
fn component_point(a: &mut App) -> [f32; 2] {
    a.ui = a.build_ui();
    let mesh = a.editor.editable_mesh(1).unwrap();
    let cache = a.mesh_cache(mesh);
    let p = |v: usize| Vec3::from_array(mesh.vertices[v].position);
    let candidates: Vec<_> = match a.authoring.selection_mode {
        S::Vertex => (0..mesh.vertices.len())
            .filter(|&i| cache.graph.canonical[i] == i && !a.authoring.vertices.contains(&i))
            .map(p)
            .collect(),
        S::Edge => cache
            .graph
            .edges
            .iter()
            .filter(|e| !a.authoring.edges.contains(e))
            .map(|&(v, w)| (p(v) + p(w)) * 0.5)
            .collect(),
        S::Face => (0..mesh.face_count())
            .map(|f| {
                let face = mesh.face_vertices(f).unwrap();
                face.iter().map(|&v| p(v as usize)).sum::<Vec3>() / face.len() as f32
            })
            .collect(),
    };
    candidates
        .into_iter()
        .filter_map(|world| a.project_point(world))
        .find(|&point| {
            if !a.canvas_input_at(point) {
                return false;
            }
            if a.authoring.selection_mode != S::Face {
                return true;
            }
            let (o, d) =
                authoring_controls::ray(a.view().view_projection().inverse(), a.viewport(), point)
                    .unwrap();
            cache
                .hit(o, d)
                .and_then(|(t, _)| mesh.triangle_face(t))
                .is_some_and(|f| !a.authoring.faces.contains(&f))
        })
        .expect("Visible unselected component")
}

fn selection_count(a: &App) -> usize {
    match a.authoring.selection_mode {
        S::Vertex => a.authoring.vertices.len(),
        S::Edge => a.authoring.edges.len(),
        S::Face => a.authoring.faces.len(),
    }
}

#[test]
fn array_selects_source_and_copies_in_each_mode_and_has_no_rotation_controls() {
    for mode in [S::Vertex, S::Edge, S::Face] {
        let mut a = app(mode);
        a.authoring.workflow.values[3] = 2.5;
        // Settings from an earlier array cannot rotate copies behind hidden fields.
        a.authoring.workflow.values[6..9].fill(45.);
        let source = a.editor.project.clone();
        a.action(Action::ModelingTool(M::Array));
        assert_eq!(selection_count(&a), 3);
        let operation = a.modeling_live.as_ref().unwrap();
        assert!(
            matches!(operation.edit(operation.values),MeshEdit::Array{rotation,..} if rotation==Vec3::ZERO)
        );
        a.ui = a.build_ui();
        for i in 4..7 {
            assert!(a.ui.action_point(&Action::Author(A::LiveDrag(i))).is_none());
            assert!(a
                .ui
                .action_point(&Action::Author(A::LiveField(i)))
                .is_none());
        }
        assert!(a
            .apply_field_value(Action::Author(A::LiveField(4)), "90")
            .is_err());
        a.apply_field_value(Action::Author(A::LiveField(0)), "4")
            .unwrap();
        assert_eq!(selection_count(&a), 4);
        a.apply_field_value(Action::Author(A::LiveField(0)), "2")
            .unwrap();
        assert_eq!(selection_count(&a), 2);
        a.modeling_modifiers_changed(ModifiersState::SHIFT);
        assert_eq!(selection_count(&a), 1);
        a.modeling_modifiers_changed(ModifiersState::empty());
        assert_eq!(selection_count(&a), 2);
        a.action(Action::Author(A::LiveAccept));
        assert_eq!(selection_count(&a), 2);
        assert_eq!(a.editor.undo_len(), 1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, source);
    }
}

#[test]
fn shift_release_during_panel_drag_resumes_preview_and_cancel_restores_original_selection() {
    let mut a = app(S::Face);
    let source = a.editor.project.clone();
    let selection = a.authoring.faces.clone();
    a.action(Action::ModelingTool(M::Inset));
    a.modeling_modifiers_changed(ModifiersState::SHIFT);
    let point = component_point(&mut a);
    click_component(&mut a, point);
    assert_ne!(a.authoring.faces, selection);
    a.action(Action::Author(A::ToolPanelDrag));
    assert!(a.drag.is_some());
    a.modeling_modifiers_changed(ModifiersState::empty());
    assert!(!a.modeling_live.as_ref().unwrap().selecting);
    assert!(
        a.modeling_live.as_ref().unwrap().error.is_none(),
        "{}",
        a.status
    );
    assert_ne!(a.editor.project, source);
    a.cancel_drag();
    a.action(Action::Author(A::LiveCancel));
    assert_eq!(a.authoring.faces, selection);
    assert_eq!(a.editor.project, source);
    assert_eq!(a.editor.undo_len(), 0);
}

fn click_component(a: &mut App, point: [f32; 2]) {
    a.mouse_move(point);
    a.mouse_down(MouseButton::Left);
    assert!(a.drag.is_none(), "Shift click must not move geometry");
    a.mouse_up(MouseButton::Left);
}

#[test]
fn shift_pointer_retargets_all_component_modes_without_stacking_previews() {
    for mode in [S::Vertex, S::Edge, S::Face] {
        let mut a = app(mode);
        let source = a.editor.project.clone();
        let original = SelectionSnapshot::capture(&a.authoring);
        a.action(Action::ModelingTool(M::Array));
        a.apply_field_value(Action::Author(A::LiveField(0)), "4")
            .unwrap();
        a.apply_field_value(Action::Author(A::LiveField(1)), "1.25")
            .unwrap();
        let values = a.modeling_live.as_ref().unwrap().values;
        let first_preview = a.editor.project.clone();
        a.modeling_modifiers_changed(ModifiersState::SHIFT);
        assert_eq!(a.editor.project, source);
        let point = component_point(&mut a);
        click_component(&mut a, point);
        assert_eq!(selection_count(&a), 2, "{mode:?}");
        assert_eq!(a.editor.project, source);
        assert!(a.editor.has_active_edit());
        assert_eq!(a.editor.undo_len(), 0);
        a.modeling_modifiers_changed(ModifiersState::empty());
        assert_eq!(a.modeling_live.as_ref().unwrap().values, values);
        assert!(
            a.modeling_live.as_ref().unwrap().error.is_none(),
            "{}",
            a.status
        );
        assert_ne!(a.editor.project, first_preview);
        a.editor.project.validate().unwrap();
        // Repeating the gesture removes the component and exactly restores the first preview.
        a.modeling_modifiers_changed(ModifiersState::SHIFT);
        a.ui = a.build_ui();
        click_component(&mut a, point);
        assert_eq!(selection_count(&a), 1);
        a.modeling_modifiers_changed(ModifiersState::empty());
        assert_eq!(a.editor.project, first_preview);
        a.action(Action::Author(A::LiveCancel));
        assert_eq!(a.editor.project, source);
        assert_eq!(a.authoring.vertices, original.vertices);
        assert_eq!(a.authoring.edges, original.edges);
        assert_eq!(a.authoring.faces, original.faces);
        assert_eq!(a.editor.undo_len(), 0);
    }
}

#[test]
fn retargeting_shared_mesh_commits_once_and_ctrl_cannot_click_through() {
    let mut a = app(S::Face);
    let mut instance = a.editor.project.node(1).unwrap().clone();
    instance.id = 3;
    a.editor.project.nodes.push(instance);
    let source = a.editor.project.clone();
    a.action(Action::ModelingTool(M::Array));
    a.modeling_modifiers_changed(ModifiersState::SHIFT);
    let point = component_point(&mut a);
    click_component(&mut a, point);
    assert!(a.modeling_control_key(&Key::Named(NamedKey::Control), false));
    assert!(a.modeling_live.is_none());
    assert!(a.tool_panel_rect().is_none());
    assert!(a.authoring.modeling_tool.is_none());
    assert_eq!(a.editor.undo_len(), 1);
    assert_eq!(a.editor.editable_mesh(3).unwrap(), &source.meshes[0]);
    assert_eq!(a.editor.project.meshes.len(), 2);
    let accepted = a.editor.project.clone();
    a.modeling_modifiers_changed(ModifiersState::CONTROL);
    a.ui = a.build_ui();
    click_component(&mut a, point);
    assert_eq!(a.editor.project, accepted);
    assert_eq!(a.editor.undo_len(), 1);
    a.modeling_modifiers_changed(ModifiersState::empty());
    a.action(Action::Undo);
    assert_eq!(a.editor.project, source);
    a.action(Action::ModelingTool(M::Array));
    assert!(a.tool_panel_rect().is_some());
}

#[test]
fn ctrl_click_applies_and_invalid_selection_keeps_panel_open_until_corrected() {
    let mut a = app(S::Face);
    a.action(Action::ModelingTool(M::Bevel));
    a.apply_field_value(Action::Author(A::LiveField(2)), "5")
        .unwrap();
    a.apply_field_value(Action::Author(A::LiveField(3)), "-50")
        .unwrap();
    a.modeling_modifiers_changed(ModifiersState::SHIFT);
    a.authoring.faces.clear();
    a.modeling_modifiers_changed(ModifiersState::empty());
    assert!(a.modeling_live.as_ref().unwrap().error.is_some());
    assert!(a.modeling_control_key(&Key::Named(NamedKey::Control), false));
    assert!(a.tool_panel_rect().is_some());
    assert_eq!(a.editor.undo_len(), 0);
    a.modeling_modifiers_changed(ModifiersState::SHIFT);
    let point = component_point(&mut a);
    click_component(&mut a, point);
    a.modeling_modifiers_changed(ModifiersState::empty());
    assert_eq!(a.modeling_live.as_ref().unwrap().values[2..4], [5., -50.]);
    assert!(
        a.modeling_live.as_ref().unwrap().error.is_none(),
        "{}",
        a.status
    );
    a.ui = a.build_ui();
    a.mouse = [700., 700.];
    assert!(a.canvas_input_at(a.mouse));
    a.modeling_modifiers_changed(ModifiersState::CONTROL);
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert!(a.modeling_live.is_none());
    assert!(a.tool_panel_rect().is_none());
    assert_eq!(a.editor.undo_len(), 1);
}

#[test]
fn shift_selection_does_not_cut_and_ctrl_closes_knife_or_cut_panel() {
    for tool in [M::Knife, M::LoopCut] {
        let mut a = app(S::Face);
        let source = a.editor.project.clone();
        a.action(Action::ModelingTool(tool));
        a.modeling_modifiers_changed(ModifiersState::SHIFT);
        let point = component_point(&mut a);
        click_component(&mut a, point);
        assert_eq!(selection_count(&a), 2);
        assert_eq!(a.editor.project, source);
        assert!(a.authoring.knife.as_ref().unwrap().press.is_none());
        a.modeling_modifiers_changed(ModifiersState::empty());
        assert!(a.modeling_control_key(&Key::Named(NamedKey::Control), false));
        assert!(a.authoring.knife.is_none());
        assert!(a.tool_panel_rect().is_none());
        assert_eq!(a.editor.undo_len(), 0);
    }
}

#[test]
fn modifier_keys_preserve_slider_fine_snap_and_text_input_ownership() {
    let mut a = app(S::Face);
    a.action(Action::ModelingTool(M::Extrude));
    a.ui = a.build_ui();
    a.mouse = a.ui.action_point(&Action::Author(A::LiveDrag(0))).unwrap();
    a.action(Action::Author(A::LiveDrag(0)));
    assert!(a.drag.is_some());
    a.modeling_modifiers_changed(ModifiersState::SHIFT | ModifiersState::CONTROL);
    assert!(!a.modeling_control_key(&Key::Named(NamedKey::Control), false));
    assert!(!a.modeling_live.as_ref().unwrap().selecting);
    a.mouse_move([a.mouse[0] + 25., a.mouse[1] + 10.]);
    assert!(a.modeling_live.is_some());
    assert_eq!(a.editor.undo_len(), 0);
    a.cancel_drag();
    a.modeling_modifiers_changed(ModifiersState::empty());
    a.action(Action::Author(A::LiveField(0)));
    assert!(a.input.is_some());
    assert!(!a.modeling_control_key(&Key::Named(NamedKey::Control), false));
    a.modeling_modifiers_changed(ModifiersState::SHIFT);
    assert!(!a.modeling_live.as_ref().unwrap().selecting);
    a.action(Action::Cancel);
    a.modeling_modifiers_changed(ModifiersState::empty());
    assert!(!a.modeling_control_key(&Key::Named(NamedKey::Control), true));
    assert!(a.modeling_live.is_some());
    a.action(Action::Author(A::LiveAccept));
    assert!(a.tool_panel_rect().is_none());
}

#[test]
fn pending_preview_waits_for_shift_release_and_one_shot_tools_close_on_apply() {
    let mut a = app(S::Face);
    a.authoring.faces.clear();
    a.action(Action::ModelingTool(M::Inset));
    a.modeling_modifiers_changed(ModifiersState::SHIFT);
    let point = component_point(&mut a);
    click_component(&mut a, point);
    assert!(a.modeling_live.is_none());
    a.modeling_modifiers_changed(ModifiersState::empty());
    assert!(a.modeling_live.is_some());
    a.action(Action::Author(A::LiveAccept));
    assert!(a.tool_panel_rect().is_none());
    a.action(Action::ModelingTool(M::Subdivide));
    assert!(a.tool_panel_rect().is_some());
    assert!(a.modeling_control_key(&Key::Named(NamedKey::Control), false));
    assert!(a.tool_panel_rect().is_none());
    assert_eq!(a.editor.undo_len(), 2);
}
