use super::*;
use crate::ui::{DropdownKind, StudioAction as S};
use poseforge_core::{BackgroundMode, MaterialPreset};
fn app() -> App {
    let mut a = App::new(None);
    a.editor = Editor::new(Project::default());
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    a.preferences.left_panel = true;
    a.width = 1280.;
    a.height = 900.;
    a
}
#[test]
fn mesh_creation_selection_translation_and_history() {
    let mut a = app();
    a.preferences.right_panel = false;
    a.surface.tab = 3;
    let before = a.editor.project.clone();
    a.ui = a.build_ui();
    let trigger =
        a.ui.hits
            .iter()
            .find(|h| h.action == Action::DropdownOpen(DropdownKind::MeshCreate))
            .unwrap()
            .rect;
    assert!(ui::contains(
        a.creation_toolbar_rect(),
        [trigger[0], trigger[1]]
    ));
    a.mouse = [trigger[0] + 13., trigger[1] + 13.];
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert!(a.dropdown.is_some());
    a.ui = a.build_ui();
    // Cancel consumes the viewport click and cannot create or move a mesh.
    a.mouse = [600., 350.];
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project, before);
    a.ui = a.build_ui();
    a.action(Action::DropdownOpen(DropdownKind::MeshCreate));
    a.dropdown_key(&Key::Named(NamedKey::ArrowDown), false, false, false);
    a.dropdown_key(&Key::Named(NamedKey::ArrowDown), false, false, false);
    a.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
    let id = a.editor.selected.unwrap();
    assert!(a.editor.project.node(id).unwrap().mesh.is_some());
    assert!(a.tool == Tool::Move);
    let transform = Transform::from_translation(Vec3::new(2., 1., -3.));
    a.editor.set_transform(id, transform).unwrap();
    assert_eq!(
        a.editor.evaluated().unwrap().global[&id].w_axis.truncate(),
        transform.translation
    );
    assert!(a.editor.undo());
    assert!(a.editor.undo());
    assert_eq!(a.editor.project, before);
}

#[test]
fn removed_creation_commands_cannot_change_document_or_history() {
    let mut a = app();
    a.action(Action::Studio(S::Mesh(0)));
    let before = a.editor.project.clone();
    let history = a.editor.undo_len();
    for command in [
        S::Mesh(11),
        S::Physics(4),
        S::Background(2),
        S::Background(3),
    ] {
        a.action(Action::Studio(command));
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), history);
    }
    assert!(a
        .studio_confirm(&Action::Studio(S::Field(14)), "45")
        .is_err());
    a.action(Action::RenderColor(3));
    assert!(a.color_picker.is_none() && a.input.is_none());
}
#[test]
fn transmission_background_and_layer_fields_share_validation_and_undo() {
    let mut a = app();
    a.action(Action::Studio(S::Mesh(0)));
    a.studio_confirm(&Action::Studio(S::Field(0)), "0.7")
        .unwrap();
    let id = a.selected_material_id().unwrap();
    assert_eq!(a.editor.project.material(id).unwrap().transmission, 0.7);
    assert!(a.studio_confirm(&Action::Studio(S::Field(0)), "2").is_err());
    a.editor.undo();
    assert_eq!(a.editor.project.material(id).unwrap().transmission, 0.);
    a.action(Action::Studio(S::Background(1)));
    assert_eq!(
        a.editor.project.render_settings.background_mode,
        BackgroundMode::Solid
    );
    a.action(Action::Studio(S::LayerAdd));
    a.action(Action::Studio(S::LayerAdd));
    a.studio_confirm(&Action::Studio(S::Field(60)), "0.25")
        .unwrap();
    a.action(Action::Studio(S::LayerMove(true)));
    assert_eq!(a.studio.layer, 0);
    assert_eq!(a.editor.project.animation_layers[0].weight, 0.25);
    a.editor.project.validate().unwrap();
}
#[test]
fn material_menu_wheel_and_vertical_thumb_do_not_edit_until_commit() {
    let mut a = app();
    a.action(Action::Studio(S::Mesh(0)));
    a.surface.tab = 1;
    a.ui = a.build_ui();
    a.open_dropdown(DropdownKind::MaterialLibrary);
    a.ui = a.build_ui();
    let before = a.editor.project.clone();
    let row =
        a.ui.hits
            .iter()
            .find(|h| h.action == Action::DropdownChoose(0))
            .unwrap()
            .rect;
    a.mouse = [row[0] + 40., row[1] + 10.];
    a.scroll(-2.);
    a.ui = a.build_ui();
    assert!(a
        .ui
        .hits
        .iter()
        .any(|h| h.action == Action::DropdownChoose(6)));
    assert_eq!(a.editor.project, before);
    let track =
        a.ui.hits
            .iter()
            .find(|h| h.action == Action::ScrollPanel(2))
            .unwrap()
            .rect;
    a.mouse = [track[0] + 6., track[1] + 4.];
    a.mouse_down(MouseButton::Left);
    a.mouse_move([track[0] + 6., track[1] + track[3] - 2.]);
    a.mouse_up(MouseButton::Left);
    a.input.as_mut().unwrap().insert("Стекло");
    a.ui = a.build_ui();
    a.choose_dropdown(0);
    assert_eq!(
        a.editor
            .project
            .material(a.selected_material_id().unwrap())
            .unwrap()
            .transmission,
        MaterialPreset::Glass.create().transmission
    );
    let owner = a.editor.selected.unwrap();
    assert_eq!(a.editor.project.node(owner).unwrap().material_layers.len(), before.node(owner).unwrap().material_layers.len() + 1);
    assert_eq!(a.editor.project.meshes, before.meshes);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn fluid_source_has_particles_at_minimum_size_and_bounds_for_every_shape() {
    for kind in 0..3 {
        let s = poseforge_core::FluidSettings {
            source_kind: kind,
            source_size: [0.08; 3],
            source_center: [1., 2., 3.],
            ..Default::default()
        };
        let w = physics_preview::source_world(s).unwrap();
        assert!(!w.positions().is_empty());
        assert!(w.positions().iter().all(|p| (*p - Vec3::new(1., 2., 3.))
            .abs()
            .cmple(Vec3::splat(0.041))
            .all()));
    }
}
#[test]
fn fluid_source_from_mesh_and_physics_assignments_are_reversible() {
    let mut a = app();
    a.action(Action::Studio(S::Mesh(0)));
    let before = a.editor.project.clone();
    a.action(Action::Studio(S::Physics(1)));
    let (min, max) = physics_preview::node_bounds(&a.editor.project, a.editor.selected.unwrap()).unwrap();
    a.action(Action::Studio(S::FluidFromMesh));
    assert_eq!(a.editor.project.fluid_settings.source_center, ((min + max) * 0.5).to_array());
    assert!(!a.fluid.positions().is_empty());
    assert!(a.editor.undo());
    assert!(a.editor.undo());
    assert_eq!(a.editor.project, before);
}
#[test]
fn camera_lock_moves_camera_and_unlock_restores_free_navigation() {
    let mut a = app();
    a.action(Action::CreateCamera);
    a.action(Action::CameraView);
    assert!(a.camera_view.is_some());
    a.action(Action::Studio(S::CameraLock));
    a.ui = a.build_ui();
    assert!(a
        .ui
        .hits
        .iter()
        .any(|h| h.action == Action::Studio(S::CameraLock)));
    a.mouse = [600., 400.];
    let eye = a.view().eye;
    a.scroll(2.);
    a.mouse_down(MouseButton::Middle);
    a.mouse_move([630., 420.]);
    a.mouse_up(MouseButton::Middle);
    assert!(a.view().eye.distance(eye) > 0.01);
    assert!(a.camera_view.is_some());
    a.action(Action::Studio(S::CameraLock));
    a.scroll(2.);
    assert!(a.camera_view.is_none());
}
#[test]
fn physics_worker_generates_deformation_and_bake_undo_restores_source() {
    use poseforge_core::{Channel, Clip, Interpolation, Keyframe, Property};
    let mut a = app();
    a.action(Action::Studio(S::Mesh(3)));
    let id = a.editor.selected.unwrap();
    a.studio_confirm(&Action::Studio(S::Field(43)), "4")
        .unwrap();
    let position = a.editor.project.node(id).unwrap().transform.translation.to_array().to_vec();
    let mut clip = Clip::new("Existing keys", 4.);
    clip.channels.push(Channel { node: id, property: Property::Translation, interpolation: Interpolation::Linear,
        keys: vec![Keyframe::new(0., position.clone()), Keyframe::new(4., position)] });
    a.editor.project.clips.push(clip); a.editor.active_clip = Some(0); a.editor.time = 1.25;
    let before = a.editor.project.clone();
    let undo = a.editor.undo_len(); let mode = a.editor.mode;
    a.editor.playing = true;
    a.action(Action::Studio(S::PhysicsPlay));
    assert!(!a.editor.playing);
    for _ in 0..60 {
        a.tick_physics(1. / 60.);
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    a.action(Action::Studio(S::PhysicsPlay));
    let deadline = Instant::now() + Duration::from_secs(10);
    while a.studio.physics.is_pending() && Instant::now() < deadline {
        a.tick_physics(0.); std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!a.studio.physics.is_pending());
    assert!(!a.studio.physics.snapshots.is_empty());
    let mut preview = before.clone();
    a.apply_physics_preview(&mut preview);
    assert_ne!(preview, before);
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), undo); assert_eq!(a.editor.time, 1.25);
    assert_eq!(a.editor.active_clip, Some(0)); assert!(a.editor.mode == mode);
    a.tick_physics(1.); let mut paused = before.clone(); a.apply_physics_preview(&mut paused);
    assert_eq!(paused, preview);
    a.bake_physics();
    assert!(a.editor.project.node(id).unwrap().physics.is_none());
    assert!(a.editor.undo());
    assert_eq!(a.editor.project, before);
    a.action(Action::Studio(S::PhysicsReset)); assert!(a.studio.physics.snapshots.is_empty());
    assert_eq!(a.editor.project, before); assert_eq!(a.editor.time, 1.25);
}

#[test]
fn thin_large_fluid_sources_stay_bounded_and_scene_boxes_stop_particles() {
    let s = poseforge_core::FluidSettings {
        source_size: [10., 0.08, 10.],
        ..Default::default()
    };
    let world = physics_preview::source_world(s).unwrap();
    assert!(world.positions().len() <= 6000);
    let mut world = FluidWorld::new(
        poseforge_simulation::FluidConfig::default(),
        vec![Vec3::new(0., 1.4, 0.)],
    )
    .unwrap();
    let original = world.signature();
    world
        .set_colliders(vec![poseforge_simulation::deform::Collider {
            min: Vec3::new(-0.5, 0.5, -0.5),
            max: Vec3::new(0.5, 0.8, 0.5),
        }])
        .unwrap();
    assert_ne!(world.signature(), original);
    for _ in 0..240 {
        world.step();
    }
    assert!(world.positions()[0].y > 0.8);
}
