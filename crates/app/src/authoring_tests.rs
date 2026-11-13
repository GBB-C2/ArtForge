use super::*;
use crate::ui::AuthorAction as A;
use crate::ui::MeshSelection as S;
use poseforge_core::{box_mesh, Node, PhysicsSettings};

fn app() -> App {
    let mut p = Project::default();
    p.meshes.push(box_mesh(2, "Cube", Vec3::ONE, None));
    let mut n = Node::new(1, "Cube");
    n.mesh = Some(2);
    n.transform.translation.y = 1.;
    p.nodes.push(n);
    let mut a = App::new(None);
    a.editor = Editor::new(p);
    a.editor.selected = Some(1);
    a.editor.mode = EditorMode::Scene;
    a.authoring.xray = true;
    a.preferences = Preferences::default();
    a.preferences.left_panel = true;
    a.preferences.right_panel = true;
    a.width = 1100.;
    a.height = 760.;
    a.frame_all();
    a
}
fn click(a: &mut App, action: Action) {
    if matches!(action, Action::Author(A::Coordinate(_))) {
        a.action(Action::ModelingTool(ui::ModelingTool::Move));
        a.inspector_scroll = a.inspector_max_scroll();
    }
    a.ui = a.build_ui();
    let r =
        a.ui.hits
            .iter()
            .rev()
            .find(|h| h.action == action)
            .expect("visible control")
            .rect;
    a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
    a.click_ui_control();
}
fn first_vertex(a: &App) -> [f32; 2] {
    let id = a.authoring.editing.unwrap();
    let mesh = a.editor.editable_mesh(id).unwrap();
    let globals = a.editor.project.global_transforms().unwrap();
    mesh.vertices
        .iter()
        .filter_map(|v| {
            a.project_point(globals[&id].transform_point3(Vec3::from_array(v.position)))
        })
        .find(|p| a.canvas_input_at(*p) && a.ui.hit(*p).is_none())
        .unwrap()
}
fn finish_job(a: &mut App) {
    let start = Instant::now();
    while a.job.is_some() {
        a.poll_jobs();
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn new_cube_uses_twelve_edges_and_both_triangle_halves_select_the_same_quad() {
    for ortho in [false, true] {
        let mut a = app();
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        a.target = Vec3::ZERO;
        a.create_primitive(0);
        a.orthographic = ortho;
        a.frame_all();
        let id = a.editor.selected.unwrap();
        let mesh = a.editor.editable_mesh(id).unwrap().clone();
        assert_eq!(authoring_controls::mesh_edges(&mesh).len(), 12);
        a.action(Action::Author(A::SelectMode(S::Face)));
        a.ui = a.build_ui();
        assert!(!a
            .ui
            .texts
            .iter()
            .any(|t| t.text.contains("Редактировать меш")));
        let model = a.editor.project.global_transforms().unwrap()[&id];
        let eye = a.view().eye;
        let face = (0..mesh.face_count())
            .find(|&f| {
                let corners = mesh.face_vertices(f).unwrap();
                let center = corners
                    .iter()
                    .map(|&i| {
                        model.transform_point3(Vec3::from_array(mesh.vertices[i as usize].position))
                    })
                    .sum::<Vec3>()
                    * 0.25;
                let normal = model
                    .transform_vector3(Vec3::from_array(mesh.vertices[corners[0] as usize].normal));
                normal.dot(eye - center) > 0.1
                    && mesh.indices[f * 6..f * 6 + 6].chunks(3).all(|t| {
                        let p = t
                            .iter()
                            .map(|&i| {
                                model.transform_point3(Vec3::from_array(
                                    mesh.vertices[i as usize].position,
                                ))
                            })
                            .sum::<Vec3>()
                            / 3.;
                        a.project_point(p).is_some_and(|p| a.canvas_input_at(p))
                    })
            })
            .unwrap();
        for t in mesh.indices[face * 6..face * 6 + 6].chunks(3) {
            a.authoring.faces.clear();
            let world = t
                .iter()
                .map(|&i| {
                    model.transform_point3(Vec3::from_array(mesh.vertices[i as usize].position))
                })
                .sum::<Vec3>()
                / 3.;
            a.mouse_move(a.project_point(world).unwrap());
            a.mouse_down(MouseButton::Left);
            a.mouse_up(MouseButton::Left);
            assert_eq!(a.authoring.faces, BTreeSet::from([face]));
        }
        a.action(Action::Author(A::SelectAll));
        assert_eq!(a.authoring.faces.len(), 6);
        a.action(Action::Author(A::SelectMode(S::Edge)));
        assert_eq!(a.authoring.edges.len(), 12);
        a.editor.project.validate().unwrap();
    }
}

#[test]
fn component_toolbar_enters_all_modes_and_scene_exits_without_editing_history() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app();
            a.preferences.theme = theme;
            a.scale = scale;
            a.preferences.right_width = Some(280.);
            let before = a.editor.project.clone();
            click(&mut a, Action::Author(A::SelectMode(S::Vertex)));
            assert_eq!(a.authoring.editing, Some(1));
            a.action(Action::Author(A::SelectAll));
            click(&mut a, Action::Author(A::SelectMode(S::Edge)));
            assert_eq!(a.authoring.selection_mode, S::Edge);
            assert_eq!(
                a.authoring.edges.len(),
                18,
                "cube edges include six triangle diagonals and weld seams"
            );
            assert!(a.authoring_key(&Key::Character("3".into()), false, false));
            assert_eq!(a.authoring.selection_mode, S::Face);
            assert_eq!(a.authoring.faces.len(), 12);
            assert!(a.authoring_key(&Key::Character("2".into()), false, false));
            assert_eq!(a.authoring.selection_mode, S::Edge);
            a.inspector_scroll = a.inspector_max_scroll();
            a.ui = a.build_ui();
            a.inspector_scroll = 0.;
            a.ui = a.build_ui();
            let bounds = a.creation_toolbar_rect();
            let mut rows = vec![];
            for action in [
                Action::Mode(0),
                Action::Author(A::SelectMode(S::Vertex)),
                Action::Author(A::SelectMode(S::Edge)),
                Action::Author(A::SelectMode(S::Face)),
            ] {
                let h =
                    a.ui.hits
                        .iter()
                        .rev()
                        .find(|h| h.action == action && ui::contains(bounds, [h.rect[0], h.rect[1]]))
                        .unwrap();
                assert!(h.rect[1] >= bounds[1] && h.rect[3] >= 30.);
                assert!(h.rect[1] + h.rect[3] < bounds[1] + bounds[3]);
                rows.push(h.rect);
            }
            assert!(rows.windows(2).all(|r| r[0][0] + r[0][2] < r[1][0]));
            click(&mut a, Action::Mode(0));
            assert!(a.authoring.editing.is_none());
            assert!(a.authoring.edges.is_empty());
            assert_eq!(a.editor.project, before);
            assert_eq!(a.editor.undo_len(), 0);
            a.editor.project.node_mut(1).unwrap().locked = true;
            a.action(Action::Author(A::SelectMode(S::Edge)));
            assert!(a.authoring.editing.is_none());
        }
    }
}

#[test]
fn edge_pointer_selection_move_cancel_undo_and_numeric_entry_follow_welded_endpoints() {
    for ortho in [false, true] {
        let mut a = app();
        a.orthographic = ortho;
        a.action(Action::Author(A::SelectMode(S::Edge)));
        a.ui = a.build_ui();
        let mesh = a.editor.editable_mesh(1).unwrap();
        let model = a.editor.project.global_transforms().unwrap()[&1];
        let point = authoring_controls::mesh_edges(mesh)
            .into_iter()
            .find_map(|(i, j)| {
                let first = a.project_point(
                    model.transform_point3(Vec3::from_array(mesh.vertices[i].position)),
                )?;
                let second = a.project_point(
                    model.transform_point3(Vec3::from_array(mesh.vertices[j].position)),
                )?;
                let midpoint = [(first[0] + second[0]) * 0.5, (first[1] + second[1]) * 0.5];
                (a.canvas_input_at(midpoint)
                    && (first[0] - second[0]).hypot(first[1] - second[1]) > 30.)
                    .then_some(midpoint)
            })
            .unwrap();
        let before = a.editor.project.clone();
        a.mouse_move(point);
        a.mouse_down(MouseButton::Left);
        assert_eq!(a.authoring.edges.len(), 1);
        assert!(matches!(a.drag, Some(Drag::Mesh(_))));
        a.mouse_move([point[0] + 24., point[1] - 12.]);
        assert_ne!(a.editor.project, before);
        a.action(Action::Cancel);
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), 0);
        a.mouse_move(point);
        a.mouse_down(MouseButton::Left);
        a.mouse_move([point[0] + 24., point[1] - 12.]);
        a.mouse_up(MouseButton::Left);
        let after = a.editor.project.clone();
        assert_eq!(a.editor.undo_len(), 1);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
        a.action(Action::Redo);
        assert_eq!(a.editor.project, after);
        a.authoring_confirm(&Action::Author(A::Coordinate(0)), "0.25")
            .unwrap();
        assert_eq!(a.editor.undo_len(), 2);
        a.editor.project.validate().unwrap();
        let unchanged = a.editor.project.clone();
        a.action(Action::Author(A::DeleteFaces));
        assert_ne!(a.editor.project, unchanged);
        a.action(Action::Undo);
        assert_eq!(a.editor.project, unchanged);
        a.action(Action::Author(A::Clear));
        a.modifiers = ModifiersState::SHIFT;
        a.action(Action::Undo);
        a.action(Action::Undo);
        a.ui = a.build_ui();
        a.mouse_move(point);
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
        assert_eq!(a.authoring.edges.len(), 1);
        assert!(a.drag.is_none());
        a.mouse_down(MouseButton::Left);
        a.mouse_up(MouseButton::Left);
        assert!(a.authoring.edges.is_empty());
    }
}

#[test]
fn mesh_pointer_drag_and_escape_use_one_transaction_in_both_projections() {
    for ortho in [false, true] {
        let mut a = app();
        a.orthographic = ortho;
        click(&mut a, Action::SurfaceTab(7));
        click(&mut a, Action::Author(A::SelectMode(S::Vertex)));
        a.ui = a.build_ui();
        let point = first_vertex(&a);
        let before = a.editor.project.clone();
        let count = a.editor.undo_len();
        a.mouse_move(point);
        a.mouse_down(MouseButton::Left);
        assert!(matches!(a.drag, Some(Drag::Mesh(_))));
        a.mouse_move([point[0] + 24., point[1] - 18.]);
        assert_ne!(a.editor.project, before);
        a.action(Action::Cancel);
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), count);
        a.ui = a.build_ui();
        a.mouse_move(point);
        a.mouse_down(MouseButton::Left);
        a.mouse_move([point[0] + 32., point[1] + 12.]);
        a.mouse_up(MouseButton::Left);
        assert_eq!(a.editor.undo_len(), count + 1);
        assert!(a.editor.undo());
        assert_eq!(a.editor.project, before);
    }
}
#[test]
fn mesh_numeric_edit_validates_and_keyboard_never_escapes_text_field() {
    let mut a = app();
    a.action(Action::Author(A::ToggleEdit));
    a.action(Action::Author(A::SelectAll));
    a.ui = a.build_ui();
    click(&mut a, Action::Author(A::Coordinate(0)));
    assert!(a.input.is_some());
    assert!(!a.authoring_key(&Key::Character("e".into()), false, false));
    let before = a.editor.project.clone();
    a.input.as_mut().unwrap().select_all();
    a.input.as_mut().unwrap().insert("nan");
    a.action(Action::ConfirmInput);
    assert!(a.input.is_some());
    assert_eq!(a.editor.project, before);
    a.input.as_mut().unwrap().select_all();
    a.input.as_mut().unwrap().insert("0.5");
    a.action(Action::ConfirmInput);
    assert!(a.input.is_none());
    assert_ne!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 1);
}
#[test]
fn fluid_and_cloth_are_children_of_physics_and_values_support_undo() {
    let mut a = app();
    a.editor.project.node_mut(1).unwrap().physics = Some(PhysicsSettings::default());
    click(&mut a, Action::SurfaceTab(6));
    let ui = a.build_ui();
    let fluid = ui
        .hits
        .iter()
        .find(|h| h.action == Action::SurfaceTab(4))
        .unwrap()
        .rect;
    let physics = ui
        .hits
        .iter()
        .find(|h| h.action == Action::SurfaceTab(6))
        .unwrap()
        .rect;
    assert!(fluid[0] > physics[0] + physics[2]);
    let cloth = ui.hits.iter().find(|h| h.action == Action::SurfaceTab(11)).unwrap().rect;
    assert_eq!(fluid[1], cloth[1]);
    assert!(cloth[0] > fluid[0]);
    a.studio_confirm(&Action::Studio(ui::StudioAction::Field(48)), "0.8")
        .unwrap();
    assert_eq!(
        a.editor.project.node(1).unwrap().physics.unwrap().friction,
        0.8
    );
    assert!(a.editor.undo());
    assert_eq!(
        a.editor.project.node(1).unwrap().physics.unwrap().friction,
        0.35
    );
    assert!(a
        .studio_confirm(&Action::Studio(ui::StudioAction::Field(47)), "0")
        .is_err());
    a.inspector_scroll = a.inspector_max_scroll();
    a.ui = a.build_ui();
    assert!(a
        .ui
        .hits
        .iter()
        .any(|h| h.action == Action::Studio(ui::StudioAction::Field(49))));
    a.action(Action::SurfaceTab(0));
    let ui = a.build_ui();
    assert!(!ui
        .hits
        .iter()
        .any(|h| h.action == Action::DropdownOpen(ui::DropdownKind::PhysicsType)));
}
#[test]
fn skeleton_creation_binding_job_and_unbind_preserve_history_and_shape() {
    let mut a = app();
    a.action(Action::Author(A::CreateSkeleton(true)));
    let root = a.authoring.skeleton.unwrap();
    assert_eq!(a.editor.selected, Some(root));
    assert_eq!(a.editor.mode, EditorMode::Scene);
    assert_eq!(a.surface.tab, 0);
    assert_eq!(a.object_selection().len(), poseforge_core::HUMANOID_JOINT_COUNT + 1);
    assert_eq!(a.authoring.bind_target, Some(1));
    assert_eq!(a.editor.project.rig_profiles[0].root, Some(root));
    let before = a.editor.project.clone();
    a.action(Action::Author(A::Bind));
    assert!(a.job.is_some());
    assert!(a.scene_write_pending());
    finish_job(&mut a);
    assert!(a.editor.project.node(1).unwrap().skin.is_some());
    assert_eq!(a.editor.undo_len(), 2);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::Author(A::Bind));
    a.job
        .as_ref()
        .unwrap()
        .cancel
        .store(true, Ordering::Relaxed);
    finish_job(&mut a);
    assert_eq!(a.editor.project, before);
}
#[test]
fn mesh_selection_changes_leave_edit_mode_and_locked_object_cannot_enter() {
    let mut a = app();
    a.action(Action::Author(A::ToggleEdit));
    assert_eq!(a.authoring.editing, Some(1));
    a.editor.selected = None;
    a.build_ui();
    assert_eq!(a.authoring.editing, None);
    a.editor.selected = Some(1);
    a.editor.project.node_mut(1).unwrap().locked = true;
    a.action(Action::Author(A::ToggleEdit));
    assert_eq!(a.authoring.editing, None);
    assert_eq!(a.editor.undo_len(), 0);
}
#[test]
fn bone_extension_numeric_confirmation_does_not_reenter_input_resolution() {
    let mut a = app();
    a.action(Action::Author(A::CreateSkeleton(false)));
    let root = a.authoring.skeleton.unwrap();
    a.editor.selected = Some(root + 3);
    a.action(Action::Author(A::ExtendJoint));
    a.input = Some(TextEdit::new("0.4".into(), Action::Author(A::ExtendJoint)));
    a.action(Action::ConfirmInput);
    assert!(a.input.is_none());
    let bone = a.editor.project.node(a.editor.selected.unwrap()).unwrap();
    assert_eq!(bone.parent, Some(root + 3));
    assert_eq!(bone.transform.translation, Vec3::Y * 0.4);
    assert_eq!(a.editor.undo_len(), 2);
}

#[test]
fn mesh_framing_fits_transformed_geometry_in_narrow_and_wide_views() {
    for ortho in [false, true] {
        for width in [900., 1700.] {
            let mut a = app();
            a.width = width;
            a.orthographic = ortho;
            a.distance = 1.;
            let node = a.editor.project.node_mut(1).unwrap();
            node.transform.scale = Vec3::new(5., 0.5, 2.);
            node.transform.translation = Vec3::new(8., 3., -4.);
            node.transform.rotation = glam::Quat::from_rotation_y(0.7);
            a.action(Action::Author(A::ToggleEdit));
            let before = a.editor.project.clone();
            a.action(Action::FrameSelected);
            let model = a.editor.evaluated().unwrap().global[&1];
            let vp = a.view().view_projection();
            for vertex in &a.editor.editable_mesh(1).unwrap().vertices {
                let clip = vp
                    * model
                        .transform_point3(Vec3::from_array(vertex.position))
                        .extend(1.);
                let ndc = clip.truncate() / clip.w;
                assert!(ndc.x.abs() < 0.91 && ndc.y.abs() < 0.91 && (0. ..1.).contains(&ndc.z));
            }
            assert_eq!(a.editor.project, before);
            assert_eq!(a.editor.undo_len(), 0);
        }
    }
}

#[test]
fn lookdev_controls_remain_reachable_in_both_themes_and_dpi_scales() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 2.] {
            let mut a = app();
            a.preferences.theme = theme;
            a.scale = scale;
            a.editor.project.lights.push(poseforge_core::Light {
                kind: poseforge_core::LightKind::Area,
                use_temperature: true,
                ..poseforge_core::Light::default()
            });
            a.select_light(a.editor.project.lights.len() - 1);
            for tab in [0, 3] {
                a.action(Action::SurfaceTab(tab));
                let mut fields = BTreeSet::new();
                let max = a.inspector_max_scroll();
                for offset in 0..=((max / 100.).ceil() as usize) {
                    a.inspector_scroll = (offset as f32 * 100.).min(max);
                    let u = a.build_ui();
                    for hit in &u.hits {
                        match hit.action {
                            Action::SurfaceLightNumber(field) if tab == 0 => {
                                fields.insert(field);
                            }
                            Action::SurfaceRenderNumber(field) if tab == 3 => {
                                fields.insert(field);
                            }
                            _ => {}
                        }
                    }
                }
                if tab == 0 {
                    assert!(fields.contains(&12) && fields.contains(&13));
                } else {
                    assert!(fields.contains(&5) && fields.contains(&6));
                }
            }
            a.action(Action::RenderQuality(1));
            assert_eq!(a.editor.project.render_settings.ambient_occlusion, 0.7);
            assert!(a
                .scene_settings_confirm(Action::SurfaceRenderNumber(6), "0")
                .is_err());
        }
    }
}
