//! Artwork workflow: view-only camera controls, contextual light ownership and files.
use super::*;

fn artwork_app() -> App {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.preferences.left_panel = true;
    app.preferences.right_panel = true;
    app.width = 1100.;
    app.height = 760.;
    app
}

#[test]
fn camera_overlay_toggles_without_editing_camera_at_both_scales_and_panel_layouts() {
    for scale in [1., 2.] {
        for panels in [false, true] {
            let mut app = artwork_app();
            app.scale = scale;
            app.action(Action::CreateCamera);
            let id = app.selected_camera_id().unwrap();
            app.preferences.left_panel = panels;
            app.preferences.right_panel = panels;
            let original = app.editor.project.clone();
            let undo = app.editor.undo_len();
            for expected in [Some(id), None] {
                app.ui = app.build_ui();
                let vp = app.viewport();
                let rect = app
                    .ui
                    .hits
                    .iter()
                    .find(|hit| {
                        hit.action == Action::CameraView
                            && ui::contains(vp, [hit.rect[0], hit.rect[1]])
                    })
                    .expect("Camera control over viewport")
                    .rect;
                assert!((app.navigation_right() - rect[0] - rect[2] - 10.).abs() < 0.01);
                assert!((rect[1] - 184.).abs() < 0.01);
                app.mouse_move([rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.]);
                app.mouse_down(MouseButton::Left);
                app.mouse_up(MouseButton::Left);
                assert_eq!(app.camera_view, expected);
                assert_eq!(app.editor.project, original);
                assert_eq!(app.editor.undo_len(), undo);
                assert!(app.drag.is_none());
            }
        }
    }
}

#[test]
fn light_selection_survives_inspector_tabs_and_material_actions_cannot_edit_a_mesh() {
    let mut app = artwork_app();
    app.select_light(0);
    let original = app.editor.project.clone();
    assert_eq!(app.surface.tab, 0);
    for tab in [3, 4, 1, 0] {
        app.action(Action::SurfaceTab(tab));
        assert_eq!(app.active_light_index(), Some(0));
        assert_eq!(app.editor.selected, None);
        assert_eq!(app.editor.project, original);
        app.ui = app.build_ui();
        if tab == 1 {
            assert_ne!(app.surface.tab, 1);
            assert!(!app
                .ui
                .hits
                .iter()
                .any(|h| h.action == Action::SurfaceTab(1)));
            assert_eq!(app.selected_material_id(), None);
            app.action(Action::SurfaceMaterialColor(false));
            assert!(app.input.is_none());
        }
    }
    app.action(Action::DropdownOpen(ui::DropdownKind::LightKind));
    assert!(app.dropdown.is_some());
    app.action(Action::DropdownChoose(3));
    assert_eq!(
        app.editor.project.lights[0].kind,
        poseforge_core::LightKind::Area
    );
    app.action(Action::Undo);
    assert_eq!(app.editor.project, original);
}

#[test]
fn artwork_and_legacy_files_roundtrip_camera_lights_and_transparency() {
    let mut app = artwork_app();
    app.action(Action::CreateCamera);
    let before = app.editor.project.clone();
    app.action(Action::Transparent);
    assert!(app.editor.project.render_settings.transparent_background);
    let directory =
        std::env::temp_dir().join(format!("artforge-workflow07-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for name in ["artwork.artforge", "legacy.poseforge", "UPPER.ARTFORGE"] {
        let path = directory.join(name);
        assert!(file_extension_allowed(FilePurpose::Save, &path));
        assert!(file_extension_allowed(FilePurpose::Open, &path));
        poseforge_io::save_project(&app.editor.project, &path).unwrap();
        assert_eq!(
            poseforge_io::load_project(&path).unwrap(),
            app.editor.project
        );
    }
    assert!(!file_extension_allowed(
        FilePurpose::Save,
        Path::new("image.png")
    ));
    app.action(Action::Undo);
    assert_eq!(app.editor.project, before);
    std::fs::remove_dir_all(directory).unwrap();
}
