use super::*;
use crate::ui::CameraTool as C;
use poseforge_core::{box_mesh, camera_transform, Camera, CameraLens, Material, Node, Project};
fn camera_app() -> App {
    let mut app = App::new(None);
    let mut p = Project::default();
    p.materials.push(Material {
        id: 3,
        ..Material::default()
    });
    p.meshes.push(box_mesh(2, "Target", Vec3::ONE, Some(3)));
    let mut n = Node::new(1, "Target");
    n.mesh = Some(2);
    p.nodes.push(n);
    let c = Camera {
        eye: Vec3::new(0., 0., 4.),
        target: Vec3::ZERO,
        ..Camera::default()
    };
    let mut node = Node::new(4, "Camera");
    node.camera = Some(CameraLens::from_camera(&c));
    node.transform = camera_transform(&c);
    p.nodes.push(node);
    p.active_camera = Some(4);
    app.editor = Editor::new(p);
    app.editor.selected = Some(4);
    app.camera_view = Some(4);
    app.preferences.collapsed_sections.clear();
    app
}
#[test]
fn focus_click_measures_surface_depth_and_undo_restores_lens() {
    let mut a = camera_app();
    let before = a.editor.project.clone();
    a.action(Action::CameraTool(C::PickFocus));
    a.mouse = a.project_point(Vec3::ZERO).unwrap();
    a.ui = a.build_ui();
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    let lens = a.editor.project.node(4).unwrap().camera.unwrap();
    assert!(lens.depth_of_field.enabled);
    assert!((lens.depth_of_field.focus_distance - 3.).abs() < 0.01);
    assert!(a.focus_pick.is_none());
    assert_eq!(a.editor.undo_len(), 1);
    assert_eq!(a.editor.selected, Some(4));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::CameraTool(C::PickFocus));
    a.action(Action::Cancel);
    assert!(a.focus_pick.is_none());
    assert_eq!(a.editor.project, before);
}
#[test]
fn saved_view_restores_pose_and_optics_without_moving_scene_camera() {
    let mut a = camera_app();
    a.camera_confirm(&Action::CameraNumber(2), "2.25").unwrap();
    a.camera_confirm(&Action::CameraNumber(3), "1.4").unwrap();
    a.action(Action::CameraTool(C::Dof));
    let expected = a.camera_snapshot();
    a.action(Action::CameraTool(C::SaveView));
    a.input.as_mut().unwrap().insert("Крупный план");
    assert!(a.confirm_input());
    let saved = a.editor.project.clone();
    assert_eq!(saved.saved_views.len(), 1);
    a.leave_camera_view();
    a.yaw += 1.;
    a.action(Action::CameraTool(C::RecallView(0)));
    assert_eq!(a.editor.project, saved);
    assert_eq!(a.camera_snapshot(), expected);
    a.action(Action::CameraTool(C::DeleteView));
    assert!(a.editor.project.saved_views.is_empty());
    a.action(Action::Undo);
    assert_eq!(a.editor.project, saved);
}
#[test]
fn material_drop_assigns_only_the_hit_instance_and_cancels_outside_viewport() {
    let mut a = camera_app();
    a.editor.selected = Some(1);
    a.surface.tab = 1;
    a.editor.project.materials.push(Material {
        id: 20,
        name: "Drop".into(),
        ..Material::default()
    });
    let mut copy = Node::new(21, "Instance");
    copy.mesh = Some(2);
    copy.transform.translation = Vec3::new(3., 0., 0.);
    a.editor.project.nodes.push(copy);
    let before = a.editor.project.clone();
    let action = Action::MaterialAssign(20, 2, Some(1));
    a.mouse = [a.width - 80., 350.];
    assert!(a.begin_material_drag(&action));
    a.mouse = a.project_point(Vec3::ZERO).unwrap();
    a.mouse_up(MouseButton::Left);
    let mesh = a.editor.project.node(1).unwrap().mesh.unwrap();
    assert_ne!(mesh, 2);
    assert!((0..a.editor.project.mesh(mesh).unwrap().face_count()).all(|f|a.editor.project.mesh(mesh).unwrap().face_material(f)==Some(20)));
    assert_eq!(a.editor.project.mesh(2).unwrap().material, Some(3));
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.mouse = [a.width - 80., 350.];
    assert!(a.begin_material_drag(&action));
    a.mouse = [-10., -10.];
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project, before);
}
#[test]
fn area_handles_aim_target_and_solo_keep_undo_and_document_ownership() {
    use crate::ui::LightTool as L;
    let mut a = camera_app();
    a.editor.project.lights[0] = poseforge_core::Light {
        kind: poseforge_core::LightKind::Area,
        position: Vec3::ZERO,
        direction: Vec3::NEG_Z,
        area_size: [1., 1.],
        ..Default::default()
    };
    a.editor.selected = Some(1);
    a.select_light(0);
    let original = a.editor.project.clone();
    let right = a.editor.project.lights[0].area_basis().0;
    a.mouse = a.project_point(right * 0.5).unwrap();
    assert!(a.area_pointer_down());
    let end = a.project_point(right * 0.9).unwrap();
    a.mouse_move(end);
    a.mouse_up(MouseButton::Left);
    assert!((a.editor.project.lights[0].area_size[0] - 1.8).abs() < 0.02);
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, original);
    a.select_light(0);
    a.mouse = a.project_point(right * 0.5).unwrap();
    assert!(a.area_pointer_down());
    a.mouse_move(end);
    a.action(Action::Cancel);
    assert_eq!(a.editor.project, original);
    a.editor.project.lights[0].position = Vec3::new(3., 2., 2.);
    a.last_light_target = Some(1);
    a.action(Action::LightTool(L::AimLast));
    let light = &a.editor.project.lights[0];
    assert!(light.direction.distance((-light.position).normalize()) < 1e-5);
    let before = a.editor.project.clone();
    a.action(Action::LightTool(L::Solo));
    let mut preview = before.clone();
    App::apply_solo_light(&mut preview, a.solo_light.unwrap());
    assert_eq!(a.editor.project, before);
    assert_eq!(preview.render_settings.ambient_intensity, 0.);
    assert!(!preview.render_settings.hdri_enabled);
    a.action(Action::LightTool(L::SoloOff));
    assert!(a.solo_light.is_none());
    assert_eq!(a.editor.project, before);
}
#[test]
fn panorama_library_swaps_assets_without_duplicates_and_modal_blocks_scene_navigation() {
    use crate::ui::WorldAction as W;
    use poseforge_core::EnvironmentMap;
    let mut a = camera_app();
    let first = Arc::new(EnvironmentMap {
        name: "Studio".into(),
        width: 2,
        height: 1,
        rgb: vec![[8., 1., 0.5]; 2],
    });
    let second = Arc::new(EnvironmentMap {
        name: "Night".into(),
        width: 2,
        height: 1,
        rgb: vec![[0.01, 0.1, 1.]; 2],
    });
    a.editor.project.environment_map = Some(first.clone());
    a.editor.project.environment_library.push(second.clone());
    let before = a.editor.project.clone();
    a.action(Action::World(W::Library));
    a.ui = a.build_ui();
    assert!(a.has_modal());
    assert!(!a.canvas_input_at([600., 400.]));
    assert!(a
        .ui
        .hits
        .iter()
        .any(|h| h.action == Action::World(W::ChooseEnvironment(1))));
    a.action(Action::World(W::ChooseEnvironment(1)));
    assert!(!a.environments.open);
    assert!(Arc::ptr_eq(
        a.editor.project.environment_map.as_ref().unwrap(),
        &second
    ));
    assert!(Arc::ptr_eq(
        &a.editor.project.environment_library[0],
        &first
    ));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::World(W::ForgetEnvironment(1)));
    assert!(a.editor.project.environment_library.is_empty());
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn shift_selection_group_drag_skips_nested_nodes_and_is_one_undo() {
    use crate::ui::SceneTool as S;
    let mut a = camera_app();
    a.editor.mode = EditorMode::Scene;
    a.editor.selected = None;
    let mut child = Node::new(10, "Child");
    child.parent = Some(1);
    child.transform.translation = Vec3::Y;
    let mut other = Node::new(11, "Other");
    other.mesh = Some(2);
    other.transform.translation = Vec3::X * 2.;
    a.editor.project.nodes.extend([child, other]);
    a.action(Action::Select(1));
    a.modifiers = ModifiersState::SHIFT;
    a.action(Action::Select(10));
    a.action(Action::Select(11));
    a.modifiers = ModifiersState::empty();
    assert_eq!(a.object_selection(), BTreeSet::from([1, 10, 11]));
    let before = a.editor.project.clone();
    a.tool = Tool::Move;
    let gizmo = a.group_gizmo().unwrap();
    a.mouse = a.project_point(gizmo.origin).unwrap();
    assert!(a.begin_group_drag(crate::gizmo::Handle::Center));
    let end = a.project_point(gizmo.origin + Vec3::X * 0.5).unwrap();
    a.mouse_move(end);
    a.mouse_up(MouseButton::Left);
    assert!((a.editor.project.node(1).unwrap().transform.translation.x - 0.5).abs() < 0.01);
    assert_eq!(
        a.editor.project.node(10).unwrap().transform.translation,
        Vec3::Y
    );
    assert!((a.editor.project.node(11).unwrap().transform.translation.x - 2.5).abs() < 0.01);
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.action(Action::SceneTool(S::Isolate));
    let preview = a.picking_project();
    assert!(preview.node(4).unwrap().camera.is_none());
    assert_eq!(a.editor.project, before);
    a.action(Action::SceneTool(S::Isolate));
    assert!(a.scene_tools.isolated.is_empty());
    a.action(Action::Duplicate);
    assert_eq!(a.editor.project.nodes.len(), before.nodes.len() + 3);
    assert_eq!(a.object_selection().len(), 2);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn collections_and_resize_are_accessible_without_reparenting_or_scene_edits() {
    use crate::ui::{DropdownKind as D, SceneTool as S};
    let mut a = camera_app();
    a.editor.selected = Some(1);
    let parent = a.editor.project.node(1).unwrap().parent;
    a.action(Action::SceneTool(S::NewCollection));
    assert!(a.input.is_some());
    let id = a.scene_tools.collection.unwrap();
    a.ui = a.build_ui();
    assert!(a.active_field_rect().is_some());
    a.input.as_mut().unwrap().insert("Предметы");
    assert!(a.confirm_input());
    assert_eq!(a.editor.project.node(1).unwrap().parent, parent);
    assert_eq!(a.editor.project.node_collection(1), Some(id));
    a.action(Action::SceneTool(S::CollectionLock(id)));
    assert!(!a.node_selectable(1));
    assert!(a.material_locked());
    a.action(Action::Undo);
    assert!(a.node_selectable(1));
    a.action(Action::SceneTool(S::CollectionVisible(id)));
    assert!(!a.editor.project.node_visible(1, false));
    assert!(a.editor.project.node_visible(1, true));
    a.action(Action::Undo);
    a.ui = a.build_ui();
    a.action(Action::DropdownOpen(D::SceneActions));
    a.action(Action::DropdownChoose(4));
    a.ui = a.build_ui();
    assert!(a.dropdown_is_open(D::Collections));
    a.action(Action::Cancel);
    let document = a.editor.project.clone();
    let width = a.left_width();
    let original = a.preferences.left_width;
    a.mouse = [a.left_panel_rect()[0] + width, 200.];
    a.begin_panel_resize(false);
    a.mouse_move([a.mouse[0] + 80., a.mouse[1]]);
    assert_eq!(a.left_width(), width + 80.);
    assert_eq!(a.editor.project, document);
    a.action(Action::Cancel);
    assert_eq!(a.preferences.left_width, original);
    assert_eq!(a.editor.project, document);
}

#[test]
fn bone_weight_brush_respects_hidden_surface_and_undo_and_escape() {
    use crate::ui::AuthorAction as A;
    let mut a = camera_app();
    a.editor.mode = EditorMode::Scene;
    a.editor.selected = Some(1);
    let mesh = a
        .editor
        .project
        .meshes
        .iter_mut()
        .find(|m| m.id == 2)
        .unwrap();
    for v in &mut mesh.vertices {
        v.position = Vec3::from_array(v.position)
            .mul_add(Vec3::splat(0.3), Vec3::ZERO)
            .to_array();
        v.joints = [0; 4];
        v.weights = [1., 0., 0., 0.];
    }
    for id in [10, 11] {
        let mut n = Node::new(id, format!("Bone {id}"));
        n.is_joint = true;
        a.editor.project.nodes.push(n);
    }
    a.editor.project.node_mut(1).unwrap().skin = Some(poseforge_core::Skin {
        joints: vec![10, 11],
        inverse_bind_matrices: vec![Mat4::IDENTITY; 2],
    });
    a.editor.project.validate().unwrap();
    a.action(Action::Author(A::Paint));
    a.action(Action::Author(A::PaintJoint(1)));
    a.weights.radius = 200.;
    a.mouse = a.project_point(Vec3::ZERO).unwrap();
    let before = a.editor.project.clone();
    assert!(a.weight_pointer_down());
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.undo_len(), 1);
    let mesh = a.editor.project.mesh(2).unwrap();
    assert!(mesh
        .vertices
        .iter()
        .filter(|v| v.position[2] > 0.)
        .any(|v| v.weights[0] > 0. && v.joints[0] == 1));
    assert!(mesh
        .vertices
        .iter()
        .filter(|v| v.position[2] < 0.)
        .all(|v| v.weights[0] == 1. && v.joints[0] == 0));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    assert!(a.weight_pointer_down());
    a.cancel_drag();
    assert_eq!(a.editor.project, before);
}

#[test]
fn collapsed_surface_keeps_visible_material_spheres_loading() {
    let mut a = camera_app();
    a.preferences.right_panel = true;
    a.editor.selected = Some(1);
    a.set_properties_tab(1);
    a.preferences.collapsed_sections.insert(101);
    assert!(a.material_previews_visible());
    a.preferences.collapsed_sections.insert(100);
    assert!(!a.material_previews_visible());
    a.preferences.collapsed_sections.remove(&100);
    assert!(a.material_previews_visible());
}
