use super::*;
use crate::ui::DropdownKind as D;
use poseforge_core::{box_mesh, Material, Node};
fn app() -> App {
    let mut a = App::new(None);
    a.preferences = Preferences::default();
    a.preferences.right_panel = true;
    let mut p = Project {
        materials: vec![
            Material {
                id: 1,
                in_library: true,
                ..Material::default()
            },
            Material {
                id: 2,
                in_library: true,
                ..Material::default()
            },
        ],
        ..Project::default()
    };
    p.meshes.push(box_mesh(10, "Box", Vec3::ONE, Some(1)));
    for id in [20, 21] {
        let mut n = Node::new(id, "Object");
        n.mesh = Some(10);
        p.nodes.push(n);
    }
    a.editor = Editor::new(p);
    a.editor.selected = Some(20);
    a.width = 1100.;
    a.height = 760.;
    a.set_properties_tab(1);
    a
}
fn click(a: &mut App, action: Action) {
    if action==Action::DropdownOpen(D::MaterialCreate) {
        click(a,Action::DropdownOpen(D::MaterialActions)); a.choose_dropdown(10); return;
    }
    a.ui = a.build_ui();
    let r =
        a.ui.hits
            .iter()
            .rev()
            .find(|h| h.action == action)
            .unwrap_or_else(||panic!("Missing {action:?}"))
            .rect;
    a.mouse_move([r[0] + r[2] * 0.5, r[1] + r[3] * 0.5]);
    a.click_ui_control();
}
#[test]
fn library_icon_sits_below_more_and_opens_search_above_results() {
    for scale in [1., 2.] {
        let mut a = app(); a.scale = scale;
        let u = a.build_ui();
        let more = u.hits.iter().find(|h| h.action == Action::DropdownOpen(D::MaterialActions)).unwrap().rect;
        let library = u.hits.iter().rev().find(|h| h.action == Action::DropdownOpen(D::MaterialLibrary)).unwrap().rect;
        assert_eq!(library[0], more[0]); assert!(library[1] >= more[1] + more[3]);
        a.ui = u; a.mouse = [library[0] + 13., library[1] + 14.]; a.click_ui_control();
        let u = a.build_ui();
        let search = u.hits.iter().find(|h| h.action == Action::MaterialSearch).unwrap().rect;
        assert!(u.hits.iter().filter(|h| matches!(h.action, Action::DropdownChoose(_))).all(|h| h.rect[1] > search[1] + search[3]));
    }
}

#[test]
fn row_detach_plus_and_more_edit_materials_without_a_minus_button() {
    let mut a = app();
    let before = a.editor.project.clone();
    assert!(!a.build_ui().hits.iter().any(|h| h.action == Action::SurfaceUnassignMaterial));
    let node = a.editor.selected.unwrap(); let material = a.selected_material_id().unwrap();
    click(&mut a, Action::SceneMaterialDetach(node, material));
    assert!(a.selected_material_id().is_none());
    assert_eq!(a.editor.project.mesh(10).unwrap().material, Some(1));
    assert_eq!(a.editor.project.materials, before.materials);
    assert!(!a
        .build_ui()
        .hits
        .iter()
        .any(|h| h.action == Action::SurfaceUnassignMaterial));
    a.action(Action::Undo);
    assert_eq!(a.editor.project, before);
    a.open_dropdown(D::MaterialCreate);
    a.action(Action::DropdownChoose(5));
    assert!(
        a.editor
            .project
            .material(a.selected_material_id().unwrap())
            .unwrap()
            .transmission
            > 0.9
    );
    let before_menu = a.editor.project.clone();
    click(&mut a, Action::DropdownOpen(D::MaterialActions));
    a.ui = a.build_ui();
    a.mouse = [500., 400.];
    a.mouse_down(MouseButton::Left);
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.editor.project, before_menu);
    assert!(a.dropdown.is_none());
    assert!(a.drag.is_none());
    click(&mut a, Action::DropdownOpen(D::MaterialActions));
    a.action(Action::DropdownChoose(0));
    assert_eq!(
        a.editor.project.materials.len(),
        before_menu.materials.len() + 1
    );
}
#[test]
fn material_menu_reveals_rename_field_and_rejects_stale_or_locked_targets() {
    let mut a = app();
    a.preferences.collapsed_sections.insert(100);
    assert!(a.build_ui().action_point(&Action::DropdownOpen(D::MaterialActions)).is_none());
    click(&mut a, Action::ToggleSection(100));
    click(&mut a, Action::DropdownOpen(D::MaterialActions));
    a.action(Action::DropdownChoose(1));
    assert!(a.input.is_some());
    assert!(!a.preferences.collapsed_sections.contains(&100));
    assert!(a
        .build_ui()
        .hits
        .iter()
        .any(|h| h.action == Action::SurfaceMaterialName));
    a.action(Action::Cancel);
    click(&mut a, Action::DropdownOpen(D::MaterialActions));
    a.editor.selected = Some(21);
    let before = a.editor.project.clone();
    a.action(Action::DropdownChoose(0));
    assert_eq!(a.editor.project, before);
    a.editor.project.node_mut(21).unwrap().locked = true;
    a.action(Action::SurfaceUnassignMaterial);
    assert_eq!(a.editor.project.mesh(10).unwrap().material, Some(1));
}
#[test]
fn camera_button_is_round_and_view_menu_is_left_of_upper_gizmo() {
    for scale in [1., 2.] {
        let mut a = app();
        a.scale = scale;
        a.action(Action::CreateCamera);
        let id = a.selected_camera_id().unwrap();
        let u = a.build_ui();
        let camera = u
            .hits
            .iter()
            .find(|h| h.action == Action::CameraView)
            .unwrap()
            .rect;
        assert_eq!(camera[2], camera[3]);
        assert_ne!(
            u.hit([camera[0] + 1., camera[1] + 1.]),
            Some(Action::CameraView)
        );
        let menu = u
            .hits
            .iter()
            .find(|h| h.action == Action::ToggleViewMenu)
            .unwrap()
            .rect;
        let gizmo = a.view_widget_rect();
        assert!(menu[0] + menu[2] < gizmo[0] && menu[1] < gizmo[1] + 20.);
        click(&mut a, Action::CameraView);
        assert_eq!(a.camera_view, Some(id));
        assert!(a
            .build_ui()
            .hits
            .iter()
            .any(|h| h.action == Action::Studio(ui::StudioAction::CameraLock)));
        click(&mut a, Action::ToggleViewMenu);
        assert_eq!(a.menu, Some(crate::chrome::Menu::View));
        let popup = a
            .build_ui()
            .hits
            .into_iter()
            .find(|h| h.action == Action::DismissMenu && h.rect[2] < a.width)
            .unwrap()
            .rect;
        assert!((popup[1] - (menu[1] + menu[3] + 6.)).abs() < 0.01);
        assert!((popup[0] + popup[2] - (menu[0] + menu[2])).abs() < 0.01);
    }
}
