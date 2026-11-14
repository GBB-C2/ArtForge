use super::*;
use crate::ui::{AuthorAction as A, DropdownKind as D, MeshSelection as S, ModifierAction as M};
use poseforge_core::{ModifierKind, ModifierSettings, Node, PrimitiveKind, PrimitiveSettings};
fn app() -> App {
    let mut a = App::new(None);
    let mut node = Node::new(1, "Model");
    node.mesh = Some(2);
    a.editor = Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, None)
            .unwrap()],
        ..Default::default()
    });
    a.preferences = Preferences {
        right_panel: true,
        animation_visible: false,
        ..Default::default()
    };
    a.width = 1440.;
    a.height = 1050.;
    a.editor.mode = EditorMode::Scene;
    a.editor.selected = Some(1);
    a.frame_all();
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
fn advanced_array_and_bevel_controls_edit_reset_and_undo() {
    let mut a = app();
    a.set_properties_tab(8);
    a.action(Action::Modifier(M::Add(ModifierKind::Array)));
    let id = a.procedural.selected.unwrap().1;
    click(&mut a, Action::DropdownOpen(D::ArrayMode(1, id)));
    a.choose_dropdown(1);
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        ModifierSettings::RadialArray { .. }
    ));
    a.apply_field_value(Action::Modifier(M::Field(1, id, 2)), "4.5")
        .unwrap();
    a.apply_field_value(Action::Modifier(M::Field(1, id, 3)), "180")
        .unwrap();
    click(&mut a, Action::Modifier(M::Reset(1, id)));
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        ModifierSettings::RadialArray { radius: 2., .. }
    ));
    a.action(Action::Undo);
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        ModifierSettings::RadialArray { radius: 4.5, .. }
    ));
    let mut curve = Node::new(3, "Curve");
    curve.curve = Some(poseforge_core::CurveData {
        points: vec![[0., 0., 0.], [10., 0., 0.]],
        interpolation: poseforge_core::CurveInterpolation::Polyline,
        ..Default::default()
    });
    a.editor.project.nodes.push(curve);
    click(&mut a, Action::DropdownOpen(D::ArrayMode(1, id)));
    a.choose_dropdown(2);
    click(&mut a, Action::DropdownOpen(D::ArrayCurve(1, id)));
    a.choose_dropdown(1);
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[0].settings,
        ModifierSettings::CurveArray { curve: Some(3), .. }
    ));
    a.action(Action::Modifier(M::Add(ModifierKind::Bevel)));
    let id = a.procedural.selected.unwrap().1;
    for (field, value) in [(1, "0.2"), (2, "5"), (3, "25"), (4, "40")] {
        point(&mut a, Action::Modifier(M::Field(1, id, field)));
        a.apply_field_value(Action::Modifier(M::Field(1, id, field)), value)
            .unwrap();
    }
    assert!(matches!(
        a.editor.project.node(1).unwrap().modifiers[1].settings,
        ModifierSettings::Bevel {
            width: 0.2,
            segments: 5,
            profile: 0.25,
            ..
        }
    ));
    a.editor.project.validate().unwrap();
}
#[test]
fn modifier_preview_updates_from_source_edits_and_keeps_a_visible_cage() {
    let mut a = app();
    a.editor.add_modifier(1, ModifierKind::Solidify).unwrap();
    a.editor.evaluated().unwrap();
    a.action(Action::Author(A::SelectMode(S::Vertex)));
    a.authoring.vertices = BTreeSet::from([0]);
    assert!(a
        .evaluated_interactive()
        .unwrap()
        .modifier_meshes
        .contains_key(&1));
    let before = a.editor.project.clone();
    a.apply_field_value(Action::Author(A::Coordinate(0)), "2")
        .unwrap();
    a.editor.evaluated().unwrap();
    let eval = a.evaluated_interactive().unwrap();
    assert_ne!(eval.modifier_meshes[&1], before.meshes[0]);
    assert_eq!(a.editor.project.meshes[0].face_count(), 6);
    let mut u = ui::Ui::new(1.);
    a.draw_mesh_overlay(&mut u);
    assert!(u
        .rects
        .iter()
        .filter(|r| r.segment.is_some())
        .all(|r| r.depth.is_none()));
    a.action(Action::Author(A::ModifierPreview));
    assert!(!a
        .evaluated_interactive()
        .unwrap()
        .modifier_meshes
        .contains_key(&1));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn smart_knife_one_drag_cuts_multiple_faces_and_enter_commits_once() {
    let mut a = app();
    let mut settings = PrimitiveSettings::new(PrimitiveKind::Plane);
    settings.segments = [3, 1, 1];
    a.editor.project.meshes[0] = settings.mesh(2, None).unwrap();
    a.frame_all();
    a.action(Action::Author(A::SelectMode(S::Face)));
    a.authoring.xray = true;
    let source = a.editor.project.clone();
    let mesh = &source.meshes[0];
    let g = poseforge_core::MeshConnectivity::new(mesh);
    let shared = *g.edge_faces.iter().find(|(_, fs)| fs.len() == 2).unwrap().0;
    let p = |i: usize| Vec3::from_array(mesh.vertices[i].position);
    let across = (p(shared.1) - p(shared.0))
        .normalize()
        .cross(Vec3::Y)
        .normalize();
    let middle = (p(shared.0) + p(shared.1)) * 0.5;
    let lo = mesh
        .vertices
        .iter()
        .map(|v| (Vec3::from_array(v.position) - middle).dot(across))
        .fold(f32::INFINITY, f32::min);
    let hi = mesh
        .vertices
        .iter()
        .map(|v| (Vec3::from_array(v.position) - middle).dot(across))
        .fold(f32::NEG_INFINITY, f32::max);
    for accept in [false, true] {
        a.action(Action::Author(A::Knife));
        a.mouse = a.project_point(middle + across * (lo - 0.2)).unwrap();
        a.mouse_down(MouseButton::Left);
        a.mouse_move(a.project_point(middle + across * (hi + 0.2)).unwrap());
        a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.project.meshes[0].face_count(), 6, "{}", a.status);
        assert_eq!(a.editor.undo_len(), 0);
        if accept {
            a.action(Action::Author(A::KnifeAccept));
            assert_eq!(a.editor.undo_len(), 1);
            a.action(Action::Undo);
        } else {
            a.action(Action::Cancel);
        }
        assert_eq!(a.editor.project, source);
    }
}
#[test]
fn repair_selection_and_additional_tools_have_one_undo_per_operation() {
    let mut a = app();
    a.action(Action::Author(A::SelectMode(S::Face)));
    a.authoring.faces = BTreeSet::from([0]);
    a.action(Action::Author(A::DeleteFaces));
    let source = a.editor.project.clone();
    a.action(Action::Author(A::RepairMode(1)));
    a.action(Action::Author(A::SelectIssue));
    assert_eq!(a.authoring.edges.len(), 4);
    let undo = a.editor.undo_len();
    a.action(Action::Author(A::RepairIssue));
    assert_eq!(a.editor.project.meshes[0].face_count(), 6, "{}", a.status);
    assert_eq!(a.editor.undo_len(), undo + 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, source);
    a.action(Action::Author(A::SelectMode(S::Face)));
    a.authoring.faces = BTreeSet::from([0]);
    a.action(Action::Author(A::ExtrudeIndividual));
    assert!(a.modeling_live.is_some(), "{}", a.status);
    a.action(Action::Cancel);
    assert_eq!(a.editor.project, source);
    a.authoring.faces = BTreeSet::from([0]);
    a.action(Action::Author(A::Triangulate));
    assert_eq!(a.editor.project.meshes[0].face_count(), 6);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, source);
    a.authoring.faces = BTreeSet::from([0]);
    a.action(Action::Author(A::Separate));
    assert_eq!(a.editor.project.nodes.len(), 2);
    assert_ne!(a.editor.selected, Some(1));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, source);
}
#[test]
fn snapping_grid_component_drag_respects_world_frame_and_cancel() {
    let mut a = app();
    let mut parent = Node::new(3, "Parent");
    parent.transform.scale = Vec3::new(2., 3., 1.);
    parent.transform.translation = Vec3::new(0.13, 0.27, 0.);
    a.editor.project.nodes.push(parent);
    a.editor.project.node_mut(1).unwrap().parent = Some(3);
    a.frame_all();
    a.action(Action::Author(A::SelectMode(S::Vertex)));
    a.authoring.vertices = BTreeSet::from([0]);
    click(&mut a, Action::DropdownOpen(D::SnapMode));
    a.choose_dropdown(5);
    a.apply_field_value(Action::Author(A::SnapStep), "0.5")
        .unwrap();
    let before = a.editor.project.clone();
    let model = a.editor.project.global_transforms().unwrap()[&1];
    let start = model.transform_point3(a.edit_center().unwrap());
    a.mouse = a.project_point(start).unwrap();
    a.begin_mesh_drag();
    a.mouse_move([a.mouse[0] + 37., a.mouse[1] + 19.]);
    let result = model.transform_point3(a.edit_center().unwrap());
    assert!((result * 2. - (result * 2.).round()).length() < 0.0001);
    a.cancel_drag();
    assert_eq!(a.editor.project, before);
    a.mouse = a.project_point(start).unwrap();
    a.begin_mesh_drag();
    a.mouse_move([a.mouse[0] + 37., a.mouse[1] + 19.]);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
#[ignore = "Exports actual UI primitives for CPU visual inspection"]
fn advanced_capture_ui() {
    let directory = PathBuf::from(std::env::var_os("ARTFORGE_UI_CAPTURE_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for width in [900., 1440.] {
            for case in [
                "bevel", "radial", "curve", "bridge", "knife", "repair", "camera", "snapping",
            ] {
                let mut a = app();
                a.width = width;
                a.preferences.theme = theme;
                match case {
                    "bevel" | "radial" | "curve" => {
                        a.set_properties_tab(8);
                        a.action(Action::Modifier(M::Add(if case == "bevel" {
                            ModifierKind::Bevel
                        } else {
                            ModifierKind::Array
                        })));
                        let id = a.procedural.selected.unwrap().1;
                        if case != "bevel" {
                            a.action(Action::Modifier(M::ArrayMode(
                                1,
                                id,
                                if case == "radial" { 1 } else { 2 },
                            )));
                        }
                    }
                    "camera" => {
                        a.action(Action::CreateCamera);
                        a.preferences
                            .collapsed_sections
                            .extend([1000, 1001, 1002, 1003]);
                        for effect in [8, 9, 10, 11] {
                            a.action(Action::CameraTool(ui::CameraTool::AddEffect(effect)));
                        }
                        point(&mut a, Action::CameraNumber(8));
                    }
                    "snapping" => {
                        a.tool = Tool::Move;
                        a.authoring.snapping.mode = 6;
                        a.mouse = point(&mut a, Action::DropdownOpen(D::SnapMode));
                        a.action(Action::DropdownOpen(D::SnapMode));
                    }
                    _ => {
                        a.action(Action::Author(A::SelectMode(S::Face)));
                        a.authoring.faces = BTreeSet::from([0, 1]);
                        a.action(Action::ModelingTool(match case {
                            "bridge" => ui::ModelingTool::BridgeFaces,
                            "knife" => ui::ModelingTool::Knife,
                            _ => ui::ModelingTool::Inspect,
                        }));
                        if case == "knife" {
                            a.action(Action::Author(A::Knife));
                        }
                    }
                }
                let u = a.build_ui();
                let data = serde_json::json!({"width":a.width,"height":a.height,"background":u.palette().viewport,"source":"Actual App::build_ui primitives; CPU preview", "rectangles":u.rects.iter().map(|r|serde_json::json!({"rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),"texts":u.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
                std::fs::write(
                    directory.join(format!(
                        "advanced-{case}-{}-{}.json",
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
