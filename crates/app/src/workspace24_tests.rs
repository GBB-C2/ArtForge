use super::*;
fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.preferences.left_panel = true;
    a.preferences.right_panel = true;
    a.width = 1280.;
    a.height = 800.;
    a.editor = Editor::new(Project::default());
    a.editor.mode = EditorMode::Scene;
    a.action(Action::Studio(ui::StudioAction::Mesh(0)));
    a.set_properties_tab(1);
    a
}
fn click(a: &mut App, action: Action) {
    if action==Action::DropdownOpen(ui::DropdownKind::MaterialCreate) {
        a.ui=a.build_ui();a.open_dropdown(ui::DropdownKind::MaterialCreate);return;
    }
    if action==Action::SurfaceMaterialName {
        let id=a.selected_material_id().unwrap();a.surface.material_click=None;
        if !a.material_list_ids().contains(&id) {
            click(a,Action::DropdownOpen(ui::DropdownKind::MaterialActions));
            a.choose_dropdown(1); return;
        }
        click(a,Action::MaterialSelect(id));click(a,Action::MaterialSelect(id));return;
    }
    a.ui = a.build_ui();
    let r =
        a.ui.hits
            .iter()
            .find(|h| h.action == action)
            .unwrap_or_else(||panic!("Missing {action:?}; library={}, selected={:?}", a.surface.material_library, a.selected_material_id()))
            .rect;
    a.mouse_move([r[0] + r[2] / 2., r[1] + r[3] / 2.]);
    a.click_ui_control();
}
#[test]
fn local_create_preserves_geometry_and_library_membership_is_explicit() {
    let mut a=app(); let before=a.editor.project.clone();
    click(&mut a,Action::SurfaceCreateMaterial);let id=a.selected_material_id().unwrap();
    assert_eq!(a.editor.project.meshes,before.meshes);
    assert!(!a.editor.project.material(id).unwrap().in_library);
    let local=a.editor.project.clone();click(&mut a,Action::MaterialSelect(id));assert_eq!(a.editor.project,local);
    a.action(Action::MaterialLibraryToggle);assert!(a.editor.project.material(id).unwrap().in_library);
    a.action(Action::Undo);assert_eq!(a.editor.project,local);
    a.action(Action::Undo);assert_eq!(a.editor.project,before);
}
#[test]
fn library_material_can_be_applied_only_to_selected_polygons() {
    let mut a = app();
    a.action(Action::Author(ui::AuthorAction::SelectMode(
        ui::MeshSelection::Face,
    )));
    a.authoring.faces = BTreeSet::from([0]);
    a.set_properties_tab(1);
    a.action(Action::SurfaceCreateMaterialPreset(10));
    let id = a.selected_material_id().unwrap();
    let before = a.editor.project.clone();
    a.action(Action::MaterialApplyFaces);
    let m = a.editor.project.mesh(a.surface_mesh_id().unwrap()).unwrap();
    assert_eq!(m.face_material(0), Some(id));
    assert_ne!(m.face_material(1), Some(id));
    assert_eq!(m.face_count(), 6);
    a.action(Action::MaterialBrowse(false));
    assert!(a.visible_material_ids().contains(&id));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
}
#[test]
fn axis_clicks_and_drags_preserve_projection_and_cancel_restores_view() {
    for ortho in [false, true] {
        for scale in [1., 2.] {
            let mut a = app();
            a.scale = scale;
            a.orthographic = ortho;
            a.free_view_fov = Some(0.9);
            click(&mut a, Action::ViewAxis(0));
            assert_eq!(a.orthographic, ortho);
            assert_eq!(a.free_view_fov, Some(0.9));
            a.yaw = 0.7;
            a.pitch = 0.4;
            a.ui = a.build_ui();
            let r =
                a.ui.hits
                    .iter()
                    .find(|h| h.action == Action::ViewAxis(4))
                    .unwrap()
                    .rect;
            let p = [r[0] + r[2] / 2., r[1] + r[3] / 2.];
            let before = (a.yaw, a.pitch, a.target, a.distance);
            a.mouse_move(p);
            a.mouse_down(MouseButton::Left);
            assert_eq!(a.yaw, before.0);
            a.mouse_move([p[0] + 30., p[1] + 10.]);
            assert_eq!(a.orthographic, ortho);
            assert!((a.yaw - (before.0 - 0.24)).abs() < 0.001);
            a.action(Action::Cancel);
            assert_eq!((a.yaw, a.pitch, a.target, a.distance), before);
            assert_eq!(a.orthographic, ortho);
        }
    }
}

#[test]
fn empty_scene_library_has_usable_creation_and_material_properties() {
    let mut a = app();
    a.editor.selected = None;
    a.action(Action::MaterialBrowse(true));
    click(
        &mut a,
        Action::DropdownOpen(ui::DropdownKind::MaterialCreate),
    );
    a.action(Action::DropdownChoose(5));
    let id = a.selected_material_id().unwrap();
    click(&mut a, Action::SurfaceMaterialName);
    a.input.as_mut().unwrap().select_all();
    a.input.as_mut().unwrap().insert("Стекло в библиотеке");
    a.action(Action::ConfirmInput);
    assert_eq!(
        a.editor.project.material(id).unwrap().name,
        "Стекло в библиотеке"
    );
    assert!(!a
        .build_ui()
        .texts
        .iter()
        .any(|t| t.text == "Объекту" || t.text == "Граням"));
    assert!(a.inspector_max_scroll() > 100.);
}
