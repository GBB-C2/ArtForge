use super::*;
use crate::ui::{AuthorAction as A, ConstructionAction as C, ModelingTool as M};
use poseforge_core::{Node, PrimitiveKind as K, PrimitiveSettings};
fn app() -> App {
    let mut a = App::new(None);
    let mut n = Node::new(1, "Cube");
    n.mesh = Some(2);
    let mut b = Node::new(3, "Cutter");
    b.mesh = Some(4);
    b.transform.translation = Vec3::X;
    a.editor = Editor::new(Project {
        nodes: vec![n, b],
        meshes: vec![
            PrimitiveSettings::new(K::Cube).mesh(2, None).unwrap(),
            PrimitiveSettings::new(K::Cube).mesh(4, None).unwrap(),
        ],
        ..Default::default()
    });
    a.editor.selected = Some(1);
    a.scene_tools.selected = BTreeSet::from([1, 3]);
    a.width = 1440.;
    a.height = 1000.;
    a.preferences = Preferences {
        animation_visible: false,
        right_panel: true,
        ..Default::default()
    };
    a.editor.mode = EditorMode::Scene;
    a.frame_all();
    a
}
fn click(a: &mut App, action: Action) {
    a.ui = a.build_ui();
    let r =
        a.ui.hits
            .iter()
            .find(|h| h.action == action)
            .unwrap_or_else(|| panic!("Missing {action:?}"))
            .rect;
    a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
}
fn await_boolean(a: &mut App) {
    for _ in 0..300 {
        a.poll_construction();
        if a.status.contains("готов") {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("Boolean did not finish: {}", a.status);
}
#[test]
fn construction_boolean_modes_preview_apply_cancel_and_undo() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let mut a = app();
        a.preferences.theme = theme;
        let before = a.editor.project.clone();
        a.action(Action::Construction(C::Boolean));
        await_boolean(&mut a);
        assert_ne!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), 0);
        for mode in [0, 2, 1] {
            click(&mut a, Action::Construction(C::Mode(mode)));
            await_boolean(&mut a);
            a.editor.project.validate().unwrap();
        }
        click(&mut a, Action::Construction(C::Cancel));
        assert_eq!(a.editor.project, before);
        assert!(a.construction.session.is_none());
        a.action(Action::Construction(C::Boolean));
        await_boolean(&mut a);
        let preview = a.editor.project.clone();
        click(&mut a, Action::Construction(C::Apply));
        assert_eq!(a.editor.project, preview);
        assert_eq!(a.editor.undo_len(), 1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
    }
}
#[test]
fn construction_cable_parameters_physics_and_cancel_are_one_transaction() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app();
            a.preferences.theme = theme;
            a.scale = scale;
            let before = a.editor.project.clone();
            a.action(Action::Construction(C::Cable));
            let id = a.editor.selected.unwrap();
            assert!(a.editor.project.node(id).unwrap().cable.is_some());
            for (i, v) in [
                (0, "-2"),
                (1, "3"),
                (3, "2"),
                (4, "3"),
                (6, "0.08"),
                (7, "32"),
            ] {
                a.apply_field_value(Action::Construction(C::Field(i)), v)
                    .unwrap();
            }
            let input = physics_preview::inputs(&a.editor.project, false)
                .into_iter()
                .find(|b| b.id == id)
                .unwrap();
            let mut world = input.world().unwrap();
            for _ in 0..40 {
                world.step(input.config(), &[]);
            }
            let m = input.snapshot(&world);
            assert_eq!(m.vertices.len(), input.mesh.vertices.len());
            assert_ne!(m.vertices, input.mesh.vertices);
            assert_eq!(a.editor.undo_len(), 0);
            click(&mut a, Action::Construction(C::Apply));
            assert_eq!(a.editor.undo_len(), 1);
            let saved = a.editor.project.clone();
            a.action(Action::Construction(C::Cable));
            a.apply_field_value(Action::Construction(C::Field(6)), "0.1")
                .unwrap();
            click(&mut a, Action::Construction(C::Cancel));
            assert_eq!(a.editor.project, saved);
            a.action(Action::Undo);
            assert_eq!(a.editor.project, before);
        }
    }
}
#[test]
fn construction_density_modes_and_mirror_rebuild_the_original() {
    let mut a = app();
    a.action(Action::Author(A::SelectMode(ui::MeshSelection::Face)));
    a.authoring.faces = (0..6).collect();
    let before = a.editor.project.clone();
    a.action(Action::ModelingTool(M::Subdivide));
    let divided = a.editor.project.clone();
    click(&mut a, Action::Author(A::DensityMode(true)));
    click(&mut a, Action::Author(A::DensityMode(false)));
    assert_eq!(a.editor.project, divided);
    click(&mut a, Action::Author(A::ToolCancel));
    assert_eq!(a.editor.project, before);
    a.action(Action::ModelingTool(M::Mirror));
    a.apply_field_value(Action::Author(A::MirrorField(0)), "2")
        .unwrap();
    assert_ne!(a.editor.project, before);
    click(&mut a, Action::Author(A::MirrorAxis(2)));
    click(&mut a, Action::Author(A::ToolCancel));
    assert_eq!(a.editor.project, before);
}
#[test]
fn construction_shift_drag_creates_a_primitive_on_a_surface_with_one_undo() {
    let mut a = app();
    a.scene_tools.selected = BTreeSet::from([1]);
    a.editor.project.nodes.retain(|n| n.id == 1);
    a.frame_all();
    let before = a.editor.project.clone();
    a.modifiers = ModifiersState::SHIFT;
    a.action(Action::Studio(ui::StudioAction::Mesh(0)));
    assert_eq!(a.editor.project, before);
    a.ui = a.build_ui();
    let p = a.project_point(Vec3::ZERO).unwrap();
    a.mouse_move(p);
    a.mouse_down(MouseButton::Left);
    assert!(
        matches!(a.drag, Some(Drag::PrimitiveDraw(_))),
        "{}",
        a.status
    );
    a.mouse_move([p[0] + 90., p[1] + 55.]);
    a.mouse_up(MouseButton::Left);
    assert!(a.primitive_height.is_some());
    assert_eq!(a.editor.undo_len(), 0);
    a.mouse_move([p[0] + 90., p[1] - 30.]);
    a.mouse_down(MouseButton::Left);
    assert!(a.primitive_height.is_none());
    assert_eq!(a.editor.project.nodes.len(), before.nodes.len() + 1);
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn construction_shift_drag_cancel_restores_scene_and_selection() {
    let mut a = app();
    let before = a.editor.project.clone();
    a.modifiers = ModifiersState::SHIFT;
    a.action(Action::Studio(ui::StudioAction::Mesh(7)));
    a.ui = a.build_ui();
    a.mouse_move([650., 260.]);
    a.mouse_down(MouseButton::Left);
    assert!(matches!(a.drag, Some(Drag::PrimitiveDraw(_))));
    a.mouse_move([730., 310.]);
    a.mouse_up(MouseButton::Left);
    assert!(a.primitive_height.is_some());
    a.action(Action::Cancel);
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.selected, Some(1));
    assert_eq!(a.editor.undo_len(), 0);
}
#[test]
fn construction_cable_sliders_handles_and_scroll_keep_cancel_atomic() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let mut a = app();
        a.preferences.theme = theme;
        a.action(Action::Construction(C::Cable));
        let id = a.editor.selected.unwrap();
        let initial = a.editor.project.clone();
        for index in [6, 7, 8, 9] {
            a.ui = a.build_ui();
            let r=a.ui.hits.iter().find(|h|matches!(&h.action,Action::Slide(f,..) if **f==Action::Construction(C::Field(index)))).unwrap().rect;
            let p = [r[0] + r[2] * 0.4, r[1] + r[3] * 0.5];
            a.mouse_move(p);
            a.mouse_down(MouseButton::Left);
            assert!(matches!(a.drag, Some(Drag::Slider(_))));
            a.mouse_move([p[0] + 17., p[1]]);
            assert_ne!(a.editor.project, initial);
            a.action(Action::Cancel);
            assert!(a.construction.session.is_some());
            assert_eq!(a.editor.project, initial);
            assert_eq!(a.editor.undo_len(), 0);
        }
        a.ui = a.build_ui();
        let r =
            a.ui.hits
                .iter()
                .find(|h| h.action == Action::Construction(C::Handle(1)))
                .unwrap()
                .rect;
        let p = [r[0] + 9., r[1] + 9.];
        a.mouse_move(p);
        a.mouse_down(MouseButton::Left);
        assert!(matches!(a.drag, Some(Drag::Construction(_))));
        a.mouse_move([p[0] + 40., p[1] + 15.]);
        assert_ne!(
            a.editor.project.node(id).unwrap().cable,
            initial.node(id).unwrap().cable
        );
        a.action(Action::Cancel);
        assert_eq!(a.editor.project, initial);
        let panel = a.construction_rect().unwrap();
        let area = a.modeling_area();
        assert!(panel[0] >= area[0] && panel[0] + panel[2] <= area[0] + area[2]);
        a.width = 900.;
        a.height = 540.;
        a.ui = a.build_ui();
        let r = a.construction_rect().unwrap();
        a.mouse = [r[0] + r[2] * 0.5, r[1] + 60.];
        a.construction_scroll(-100.);
        a.ui = a.build_ui();
        assert!(a.ui.action_point(&Action::Construction(C::Play)).is_some());
        assert!(a.ui.action_point(&Action::Construction(C::Apply)).is_some());
        assert!(a
            .ui
            .action_point(&Action::Construction(C::Cancel))
            .is_some());
        click(&mut a, Action::Construction(C::Collapse));
        a.ui = a.build_ui();
        assert!(a.ui.action_point(&Action::Construction(C::Apply)).is_none());
        click(&mut a, Action::Construction(C::Collapse));
        click(&mut a, Action::Construction(C::Apply));
        assert_eq!(a.editor.undo_len(), 1);
        assert!(a.construction.session.is_none());
    }
}
#[test]
fn construction_shift_plane_lies_on_the_clicked_surface() {
    let mut a = app();
    a.editor.project.nodes.retain(|n| n.id == 1);
    a.frame_all();
    a.action(Action::Top);
    a.modifiers = ModifiersState::SHIFT;
    a.action(Action::Studio(ui::StudioAction::Mesh(1)));
    a.ui = a.build_ui();
    let p = a.project_point(Vec3::Y).unwrap();
    a.mouse_move(p);
    let (_, normal) = a.surface_anchor().unwrap();
    a.mouse_down(MouseButton::Left);
    a.mouse_move([p[0] + 35., p[1] + 45.]);
    a.mouse_up(MouseButton::Left);
    let n = a.editor.project.node(a.editor.selected.unwrap()).unwrap();
    let m = a.editor.project.mesh(n.mesh.unwrap()).unwrap();
    let model = n.transform.matrix();
    let origin = model.transform_point3(Vec3::from_array(m.vertices[0].position));
    for v in &m.vertices {
        let p = model.transform_point3(Vec3::from_array(v.position));
        assert!((p - origin).dot(normal).abs() < 1e-5);
    }
    assert_eq!(a.editor.undo_len(), 1);
}
#[test]
fn construction_cable_animation_bake_keeps_tube_topology() {
    let mut a = app();
    a.action(Action::Construction(C::Cable));
    a.apply_field_value(Action::Construction(C::Field(1)), "4")
        .unwrap();
    a.apply_field_value(Action::Construction(C::Field(4)), "4")
        .unwrap();
    let id = a.editor.selected.unwrap();
    a.action(Action::Construction(C::Pin(1)));
    a.action(Action::Construction(C::Apply));
    let before = a.editor.project.clone();
    let baked = physics_bake::bake(&before, id, 8, &AtomicBool::new(false), |_| {}).unwrap();
    let n = baked.node(id).unwrap();
    assert!(n.cable.is_none() && n.physics.is_none());
    let mesh = baked.mesh(n.mesh.unwrap()).unwrap();
    assert_eq!(mesh.morph_targets.len(), 8);
    assert!(mesh
        .morph_targets
        .iter()
        .all(|m| m.positions.len() == mesh.vertices.len()));
    assert!(mesh.morph_targets[7]
        .positions
        .iter()
        .any(|p| Vec3::from_array(*p).length() > 0.01));
    assert_eq!(a.editor.project, before);
}
#[test]
#[ignore = "Exports actual UI primitives for CPU visual inspection"]
fn construction_capture_ui() {
    let dir = PathBuf::from(std::env::var_os("ARTFORGE_UI_CAPTURE_DIR").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for (width, height) in [(1440., 1000.), (900., 620.)] {
            for name in ["cable", "boolean", "mirror", "density"] {
                let mut a = app();
                a.width = width;
                a.height = height;
                a.preferences.theme = theme;
                match name {
                    "cable" => a.action(Action::Construction(C::Cable)),
                    "boolean" => {
                        a.action(Action::Construction(C::Boolean));
                        await_boolean(&mut a);
                    }
                    "mirror" => a.action(Action::Construction(C::Mirror)),
                    _ => {
                        a.action(Action::Author(A::SelectMode(ui::MeshSelection::Face)));
                        a.authoring.faces = (0..6).collect();
                        a.action(Action::ModelingTool(M::Subdivide));
                    }
                }
                let u = a.build_ui();
                let data = serde_json::json!({"width":width,"height":height,"background":u.palette().viewport,"source":"Actual App::build_ui primitives; CPU preview, not native window capture","rectangles":u.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),"texts":u.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
                std::fs::write(
                    dir.join(format!(
                        "construction-{name}-{}-{}.json",
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
fn construction_object_tools_do_not_overlap_morph_controls() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        let mut a = app();
        a.width = 900.;
        a.height = 620.;
        a.preferences.theme = theme;
        a.action(Action::Construction(C::Boolean));
        await_boolean(&mut a);
        a.ui = a.build_ui();
        let cable = a.ui.action_point(&Action::Construction(C::Cable)).unwrap();
        assert_eq!(a.ui.hit(cable), Some(Action::Construction(C::Cable)));
        assert!(a.morph_panel_start() > 320.);
        a.action(Action::Construction(C::Cancel));
    }
}

#[test]
fn construction_cable_preserves_collision_choices_and_rejects_locked_edits() {
    let mut a = app();
    a.action(Action::Construction(C::Cable));
    let id = a.editor.selected.unwrap();
    a.action(Action::Construction(C::Collisions));
    a.action(Action::Construction(C::Apply));
    assert!(a
        .editor
        .project
        .node(id)
        .unwrap()
        .collision_targets
        .cloth
        .as_ref()
        .unwrap()
        .is_empty());
    a.action(Action::Construction(C::Cable));
    assert!(a
        .editor
        .project
        .node(id)
        .unwrap()
        .collision_targets
        .cloth
        .as_ref()
        .unwrap()
        .is_empty());
    a.action(Action::Construction(C::Cancel));
    a.editor.project.node_mut(id).unwrap().locked = true;
    let before = a.editor.project.clone();
    a.action(Action::Construction(C::Cable));
    assert!(a.construction.session.is_none());
    assert_eq!(a.editor.project, before);
}
