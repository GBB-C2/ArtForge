//! Exercise the graph and environment through real hit targets, gestures and history.
use super::*;
use crate::ui::{NodeAction as N, WorldAction as W};
use poseforge_core::{
    box_mesh, EnvironmentMap, Material, MaterialGraph, MaterialNode, MaterialNodeKind as K, Node,
};

fn app() -> App {
    let mut app = App::new(None);
    let mut p = Project::default();
    p.materials.push(Material {
        id: 1,
        ..Material::default()
    });
    p.meshes.push(box_mesh(2, "Cube", Vec3::ONE, Some(1)));
    let mut node = Node::new(3, "Cube");
    node.mesh = Some(2);
    p.nodes.push(node);
    app.editor = Editor::new(p);
    app.editor.selected = Some(3);
    app.preferences = Preferences::default();
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app.surface.tab = 1;
    app.saved_snapshot = app.editor.project.clone();
    app
}
fn hit(app: &mut App, action: Action) -> [f32; 2] {
    app.ui = app.build_ui();
    let r = app
        .ui
        .hits
        .iter()
        .rev()
        .find(|h| h.action == action)
        .unwrap_or_else(|| panic!("Missing hit {action:?}"))
        .rect;
    [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]
}
fn click(app: &mut App, action: Action) {
    let p = hit(app, action);
    app.mouse_move(p);
    app.click_ui_control();
}
fn graph(app: &App) -> &MaterialGraph {
    app.editor
        .project
        .material(1)
        .unwrap()
        .graph
        .as_ref()
        .unwrap()
}
fn setup_graph(app: &mut App) {
    app.editor.project.materials[0].graph = Some(MaterialGraph {
        nodes: vec![
            MaterialNode::new(1, K::Color, [20., 20.]),
            MaterialNode::new(2, K::Mix, [220., 20.]),
        ],
        output_position: [450., 20.],
        ..MaterialGraph::default()
    });
    click(app, Action::Node(N::Open));
}
fn draft(app: &mut App, text: &str) {
    let i = app.input.as_mut().unwrap();
    i.select_all();
    i.insert(text);
}

#[test]
fn graph_click_and_drag_links_preserve_redraw_and_reject_cycles() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app();
            a.preferences.theme = theme;
            a.scale = scale;
            setup_graph(&mut a);
            click(&mut a, Action::Node(N::Output(1)));
            a.ui = a.build_ui();
            assert_eq!(a.nodes.source, Some(1));
            click(&mut a, Action::Node(N::Input(2, 0)));
            assert_eq!(graph(&a).nodes[1].inputs[0], Some(1));
            let from = hit(&mut a, Action::Node(N::Output(2)));
            let to = hit(&mut a, Action::Node(N::Input(0, 0)));
            a.mouse_move(from);
            a.mouse_down(MouseButton::Left);
            a.mouse_move(to);
            a.mouse_up(MouseButton::Left);
            assert_eq!(graph(&a).outputs[0], Some(2));
            assert_eq!(a.editor.undo_len(), 2);
            click(&mut a, Action::Node(N::Output(2)));
            click(&mut a, Action::Node(N::Input(2, 1)));
            assert_eq!(graph(&a).nodes[1].inputs[1], None);
            assert_eq!(a.editor.undo_len(), 2);
            a.action(Action::Cancel);
            assert!(a.nodes_open());
            assert_eq!(a.nodes.source, None);
            a.action(Action::Undo);
            assert_eq!(graph(&a).outputs[0], None);
            a.action(Action::Redo);
            assert_eq!(graph(&a).outputs[0], Some(2));
        }
    }
}
#[test]
fn node_move_is_one_history_entry_and_escape_rolls_back_preview() {
    let mut a = app();
    setup_graph(&mut a);
    let before = a.editor.project.clone();
    let from = hit(&mut a, Action::Node(N::Move(1)));
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    for dx in [10., 20., 60.] {
        a.mouse_move([from[0] + dx, from[1] + 20.]);
    }
    assert_eq!(a.editor.undo_len(), 0);
    a.action(Action::Cancel);
    assert!(a.editor.project == before);
    assert!(a.drag.is_none());
    let from = hit(&mut a, Action::Node(N::Move(1)));
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move([from[0] + 40., from[1] + 15.]);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.undo_len(), 1);
    assert_ne!(graph(&a).nodes[0].position, [20., 20.]);
    a.action(Action::Undo);
    assert!(a.editor.project == before);
}
#[test]
fn node_fields_keep_invalid_draft_and_scrollbar_exposes_delete_without_editing() {
    let mut a = app();
    setup_graph(&mut a);
    click(&mut a, Action::Node(N::Select(1)));
    click(&mut a, Action::Node(N::Value(0)));
    draft(&mut a, "NaN");
    a.action(Action::ConfirmInput);
    assert!(a.input.is_some() && a.surface.input_error.is_some());
    assert_eq!(a.editor.undo_len(), 0);
    draft(&mut a, "0,25");
    a.action(Action::ConfirmInput);
    assert_eq!(graph(&a).nodes[0].value[0], 0.25);
    click(&mut a, Action::Node(N::Value(4)));
    draft(&mut a, "#808080");
    a.action(Action::ConfirmInput);
    assert!((graph(&a).nodes[0].value[0] - 0.21586).abs() < 0.0001);
    let count = a.editor.undo_len();
    let from = hit(&mut a, Action::Node(N::InspectorScroll));
    a.mouse_move(from);
    a.mouse_down(MouseButton::Left);
    a.mouse_move([from[0], from[1] + 200.]);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.undo_len(), count);
    click(&mut a, Action::Node(N::Delete));
    assert_eq!(graph(&a).nodes.len(), 1);
    a.action(Action::Undo);
    assert_eq!(graph(&a).nodes.len(), 2);
}
#[test]
fn graph_menu_keyboard_and_lock_pruning_do_not_edit_the_wrong_material() {
    let mut a = app();
    click(&mut a, Action::Node(N::Open));
    let count = a.editor.undo_len();
    click(&mut a, Action::Node(N::AddMenu));
    a.ui = a.build_ui();
    assert!(a.nodes.add_menu);
    a.node_key(&Key::Named(NamedKey::ArrowDown), false, false);
    a.node_key(&Key::Named(NamedKey::Enter), false, false);
    assert_eq!(graph(&a).nodes[0].kind, K::Value);
    assert_eq!(a.editor.undo_len(), count + 1);
    a.editor.project.nodes[0].locked = true;
    a.ui = a.build_ui();
    assert!(!a.nodes_open());
    let count = a.editor.undo_len();
    a.action(Action::Node(N::Add(0)));
    assert_eq!(a.editor.undo_len(), count);
}
#[test]
fn environment_controls_and_quality_dropdown_commit_once_and_validate_numeric_range() {
    for scale in [1., 2.] {
        let mut a = app();
        a.scale = scale;
        a.action(Action::SurfaceTab(3));
        a.inspector_scroll = 650.;
        click(&mut a, Action::World(W::Reflections));
        assert!(a.editor.project.render_settings.ray_reflections);
        click(
            &mut a,
            Action::DropdownOpen(ui::DropdownKind::ReflectionSamples),
        );
        a.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
        a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
        assert_eq!(a.editor.project.render_settings.reflection_samples, 16);
        assert_eq!(a.editor.undo_len(), 2);
        click(&mut a, Action::SurfaceRenderNumber(8));
        draft(&mut a, "181");
        a.action(Action::ConfirmInput);
        assert!(a.input.is_some());
        assert_eq!(a.editor.project.render_settings.hdri_rotation, 0.);
        draft(&mut a, "-90");
        a.action(Action::ConfirmInput);
        assert_eq!(a.editor.project.render_settings.hdri_rotation, -90.);
        a.action(Action::Undo);
        assert_eq!(a.editor.project.render_settings.hdri_rotation, 0.);
    }
}
#[test]
fn environment_job_applies_one_embedded_undo_and_cancel_discards_result() {
    let mut a = app();
    let before = a.editor.project.clone();
    let map = EnvironmentMap {
        name: "Studio".into(),
        width: 1,
        height: 1,
        rgb: vec![[4., 2., 1.]],
    };
    for cancelled in [true, false] {
        let (sender, receiver) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(cancelled));
        a.job = Some(Job {
            receiver,
            cancel,
            label: "Загрузка HDRI".into(),
        });
        sender
            .send(JobMessage::EnvironmentLoaded(map.clone()))
            .unwrap();
        a.poll_jobs();
        assert_eq!(a.editor.project.environment_map.is_some(), !cancelled);
    }
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert!(a.editor.project == before);
    for ext in ["hdr", "HDR", "png", "jpg", "jpeg"] {
        assert!(file_extension_allowed(
            FilePurpose::Environment,
            Path::new(&format!("studio.{ext}"))
        ));
    }
    assert!(!file_extension_allowed(
        FilePurpose::Environment,
        Path::new("studio.exr")
    ));
}
