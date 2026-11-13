//! Small contextual menus keep secondary controls off the canvas.
use crate::icons::Icon;
use crate::ui::{Action, Hit, Ui};
use poseforge_core::{BoneRole, Project, RigProfile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Project,
    View,
    Body,
}

/// All coordinates are logical pixels, matching the rest of the native UI.
#[allow(clippy::too_many_arguments)]
pub fn draw_menu(
    u: &mut Ui,
    size: [f32; 2],
    viewport: [f32; 4],
    menu: Menu,
    project: &Project,
    profile: Option<&RigProfile>,
    selected: Option<u64>,
    orthographic: bool,
    grid: bool,
    skeleton: bool,
    preview: [bool; 3],
) {
    let mut panel = menu_rect(size, viewport, menu);
    if menu == Menu::View {
        if let Some(anchor) = u
            .hits
            .iter()
            .rev()
            .find(|h| h.action == Action::ToggleViewMenu)
            .map(|h| h.rect)
        {
            panel[0] =
                (anchor[0] + anchor[2] - panel[2]).clamp(12., (size[0] - panel[2] - 12.).max(12.));
            panel[1] = (anchor[1] + anchor[3] + 6.).clamp(12., (size[1] - panel[3] - 12.).max(12.));
        }
    }
    popover(u, size, panel);
    match menu {
        Menu::Project => project_menu(u, panel, project),
        Menu::View => view_menu(
            u,
            panel,
            project,
            selected,
            orthographic,
            grid,
            skeleton,
            preview,
        ),
        Menu::Body => body_menu(u, panel, project, profile, selected),
    }
}

fn menu_rect(size: [f32; 2], viewport: [f32; 4], menu: Menu) -> [f32; 4] {
    let [width, height] = match menu {
        Menu::Project => [320., 508.],
        Menu::View => [288., 496.],
        Menu::Body => [288., 382.],
    };
    let (requested_x, requested_y) = match menu {
        Menu::Project => (12., 58.),
        Menu::View => (viewport[0] + viewport[2] - 300., viewport[1] + 156.),
        Menu::Body => {
            let right = (viewport[0] + viewport[2] - width - 12.).max(viewport[0] + 12.);
            (f32::min(viewport[0] + 116., right), viewport[1] + 12.)
        }
    };
    [
        requested_x.clamp(12., (size[0] - width - 12.).max(12.)),
        requested_y.clamp(12., (size[1] - height - 12.).max(12.)),
        width,
        height,
    ]
}

fn popover(u: &mut Ui, size: [f32; 2], panel: [f32; 4]) {
    let p = u.palette();
    // A dismissal click must never reach the viewport or an underlying control.
    u.hits.clear();
    u.hits.push(Hit {
        rect: [0., 0., size[0], size[1]],
        action: Action::DismissMenu,
    });
    u.hits.push(Hit {
        rect: panel,
        action: Action::DismissMenu,
    });

    // Text is drawn after all rectangles. Remove only background labels that
    // intersect the popup, otherwise they would show through its opaque surface.
    // The conservative width estimate also covers Cyrillic labels and wide glyphs.
    let [px, py, pw, ph] = panel.map(|value| value * u.scale);
    u.texts.retain(|text| {
        let width = text.text.chars().count() as f32 * text.size;
        text.x >= px + pw
            || text.x + width <= px
            || text.y >= py + ph
            || text.y + text.size * 1.5 <= py
    });

    let [x, y, width, height] = panel;
    u.surface([x + 3., y + 4., width, height], [0., 0., 0., 0.16]);
    u.outlined_surface(panel, p.panel, p.border);
}

fn heading(u: &mut Ui, panel: [f32; 4], title: &str) {
    let p = u.palette();
    let [x, y, width, _] = panel;
    u.label(x + 16., y + 16., title, 18., p.text);
    u.quiet_button(
        [x + width - 44., y + 12., 32., 30.],
        "×",
        Action::DismissMenu,
        false,
    );
}

fn separator(u: &mut Ui, panel: [f32; 4], offset: f32) {
    u.rect(
        [panel[0] + 16., panel[1] + offset, panel[2] - 32., 1.],
        u.palette().border,
    );
}

fn project_menu(u: &mut Ui, panel: [f32; 4], project: &Project) {
    let p = u.palette();
    let [x, y, width, _] = panel;
    heading(u, panel, "Проект");
    u.label(x + 16., y + 54., "Имя проекта", 11., p.muted);
    u.field(
        [x + 12., y + 74., width - 24., 34.],
        &project.title,
        Action::EditTitle,
    );

    for (row, (label, action, icon)) in [
        ("Новый проект", Action::New, Icon::File),
        ("Открыть…", Action::Open, Icon::Folder),
        ("Импорт модели…", Action::Import, Icon::Import),
        ("Сохранить", Action::Save, Icon::Save),
        ("Сохранить как…", Action::SaveAs, Icon::Save),
    ]
    .into_iter()
    .enumerate()
    {
        u.quiet_icon_button(
            [x + 12., y + 122. + row as f32 * 38., width - 24., 32.],
            label,
            action,
            false,
            icon,
        );
    }
    separator(u, panel, 320.);
    u.quiet_icon_button(
        [x + 12., y + 334., width - 24., 32.],
        "Сцена",
        Action::Mode(0),
        false,
        Icon::Grid,
    );
    u.quiet_icon_button(
        [x + 12., y + 372., width - 24., 32.],
        "Рендер и экспорт…",
        Action::Mode(3),
        false,
        Icon::Camera,
    );
    u.quiet_icon_button(
        [x + 12., y + 422., width - 24., 32.],
        "Справка и клавиши",
        Action::Help,
        false,
        Icon::Help,
    );
    u.quiet_icon_button(
        [x + 12., y + 460., width - 24., 32.],
        "Отчёт последнего импорта",
        Action::ImportReport,
        false,
        Icon::File,
    );
}

#[allow(clippy::too_many_arguments)] // Menu receives independent workspace toggles.
fn view_menu(
    u: &mut Ui,
    panel: [f32; 4],
    project: &Project,
    selected: Option<u64>,
    orthographic: bool,
    grid: bool,
    skeleton: bool,
    preview: [bool; 3],
) {
    let [x, y, width, _] = panel;
    heading(u, panel, "Вид");
    let cell_width = (width - 36.) / 3.;
    for (column, (label, action)) in [
        ("Спереди", Action::Front),
        ("Сбоку", Action::Side),
        ("Сверху", Action::Top),
    ]
    .into_iter()
    .enumerate()
    {
        u.quiet_button(
            [
                x + 12. + column as f32 * (cell_width + 6.),
                y + 58.,
                cell_width,
                34.,
            ],
            label,
            action,
            false,
        );
    }
    let frame = [x + 12., y + 104., width - 24., 34.];
    if selected.and_then(|id| project.node(id)).is_some() {
        u.quiet_icon_button(
            frame,
            "Приблизить выбранное",
            Action::FrameSelected,
            false,
            Icon::Frame,
        );
    } else {
        u.disabled_icon_button(frame, "Приблизить выбранное", Icon::Frame);
    }
    u.checkbox(
        [x + 12., y + 146., width - 24., 34.],
        "Перспектива",
        Action::Perspective,
        !orthographic,
    );
    separator(u, panel, 192.);
    u.checkbox(
        [x + 12., y + 206., width - 24., 32.],
        "Сетка",
        Action::ToggleGrid,
        grid,
    );
    u.checkbox(
        [x + 12., y + 248., width - 24., 32.],
        "Скелет",
        Action::ToggleSkeleton,
        skeleton,
    );
    u.checkbox(
        [x + 12., y + 288., width - 24., 32.],
        "Адаптивное качество",
        Action::AdaptivePreview,
        preview[0],
    );
    u.checkbox(
        [x + 12., y + 328., width - 24., 32.],
        "Статистика просмотра",
        Action::PerformanceOverlay,
        preview[1],
    );
    u.checkbox([x + 12., y + 368., width - 24., 30.], "Отображать задние грани", Action::ToggleBackfaces, preview[2]);
    u.button(
        [x + 12., y + 408., width - 24., 30.],
        "Сохранить ракурс…",
        Action::CameraTool(crate::ui::CameraTool::SaveView),
        false,
    );
    u.button(
        [x + 12., y + 448., width - 24., 30.],
        "Сохранённые ракурсы…",
        Action::DropdownOpen(crate::ui::DropdownKind::SavedViews),
        false,
    );
}

fn body_menu(
    u: &mut Ui,
    panel: [f32; 4],
    project: &Project,
    profile: Option<&RigProfile>,
    selected: Option<u64>,
) {
    let p = u.palette();
    let [x, y, width, _] = panel;
    heading(u, panel, "Части тела");
    let profile = profile.filter(|profile| profile.validate(project).is_ok());
    u.label(
        x + 16.,
        y + 52.,
        if profile.is_some() {
            "Выберите часть тела"
        } else {
            "Сначала настройте персонажа"
        },
        12.,
        p.muted,
    );
    for (row, (role, label)) in [
        (BoneRole::Pelvis, "Таз"),
        (BoneRole::Head, "Голова"),
        (BoneRole::LeftHand, "Левая кисть"),
        (BoneRole::RightHand, "Правая кисть"),
        (BoneRole::LeftFoot, "Левая стопа"),
        (BoneRole::RightFoot, "Правая стопа"),
    ]
    .into_iter()
    .enumerate()
    {
        let index = BoneRole::ALL
            .iter()
            .position(|candidate| *candidate == role)
            .unwrap();
        let id = profile.and_then(|profile| profile.assignments.get(&role).copied());
        let enabled = id
            .and_then(|id| project.node(id))
            .is_some_and(|node| node.is_joint && node.visible && !node.locked);
        let row_y = y + 84. + row as f32 * 39.;
        let select = [x + 12., row_y, width - 84., 32.];
        if enabled {
            u.quiet_button(select, label, Action::SelectRole(index), selected == id);
        } else {
            u.disabled_button(select, label);
        }
        if role.is_ik_end() {
            let ik = [x + width - 66., row_y, 54., 32.];
            let chain_enabled = enabled
                && profile
                    .and_then(|profile| profile.chain(role, project).ok())
                    .is_some_and(|chain| {
                        chain.iter().all(|id| {
                            project
                                .node(*id)
                                .is_some_and(|node| node.is_joint && node.visible && !node.locked)
                        })
                    });
            if chain_enabled {
                u.quiet_button(ik, "IK", Action::IkRole(index), false);
            } else {
                u.disabled_button(ik, "IK");
            }
        }
    }
    separator(u, panel, 320.);
    u.quiet_button(
        [x + 12., y + 334., width - 24., 32.],
        "Настроить персонажа…",
        Action::CharacterPanel,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Theme;
    use poseforge_core::Node;

    fn body_fixture() -> (Project, RigProfile) {
        let mut project = Project::default();
        let mut profile = RigProfile {
            root: Some(1),
            ..RigProfile::default()
        };
        for (id, parent, role) in [
            (1, None, BoneRole::Pelvis),
            (2, Some(1), BoneRole::Head),
            (3, Some(1), BoneRole::LeftUpperArm),
            (4, Some(3), BoneRole::LeftForearm),
            (5, Some(4), BoneRole::LeftHand),
            (6, Some(1), BoneRole::RightUpperArm),
            (7, Some(6), BoneRole::RightForearm),
            (8, Some(7), BoneRole::RightHand),
            (9, Some(1), BoneRole::LeftThigh),
            (10, Some(9), BoneRole::LeftShin),
            (11, Some(10), BoneRole::LeftFoot),
            (12, Some(1), BoneRole::RightThigh),
            (13, Some(12), BoneRole::RightShin),
            (14, Some(13), BoneRole::RightFoot),
        ] {
            let mut node = Node::new(id, role.label());
            node.parent = parent;
            node.is_joint = true;
            project.nodes.push(node);
            profile.assignments.insert(role, id);
        }
        (project, profile)
    }

    fn menu_ui(menu: Menu, project: &Project, profile: Option<&RigProfile>) -> Ui {
        let mut ui = Ui::new(1.);
        draw_menu(
            &mut ui,
            [1100., 760.],
            [244., 58., 578., 556.],
            menu,
            project,
            profile,
            Some(5),
            false,
            true,
            true,
            [true, false, true],
        );
        ui
    }

    #[test]
    fn menu_captures_background_clicks_without_dimming_canvas() {
        let mut ui = Ui::new(1.);
        ui.button([900., 600., 100., 40.], "Удалить", Action::Delete, false);
        ui.label(500., 50., "Остаётся видимым", 13., ui.palette().text);
        let old_rects = ui.rects.len();
        draw_menu(
            &mut ui,
            [1100., 760.],
            [0., 58., 1100., 556.],
            Menu::Project,
            &Project::default(),
            None,
            None,
            false,
            true,
            true,
            [true, false, true],
        );
        assert_eq!(ui.hit([950., 620.]), Some(Action::DismissMenu));
        assert_eq!(ui.hit([16., 110.]), Some(Action::DismissMenu));
        assert!(!ui.hits.iter().any(|hit| hit.action == Action::Delete));
        assert!(ui.texts.iter().any(|text| text.text == "Остаётся видимым"));
        assert!(!ui.rects[old_rects..]
            .iter()
            .any(|rect| rect.rect == [0., 0., 1100., 760.]));
        for action in [
            Action::EditTitle,
            Action::New,
            Action::Open,
            Action::Import,
            Action::Save,
            Action::SaveAs,
            Action::Mode(0),
            Action::Mode(3),
            Action::Help,
        ] {
            assert!(ui.hits.iter().any(|hit| hit.action == action), "{action:?}");
        }
    }

    #[test]
    fn menus_fit_minimum_window_with_both_side_panels_at_any_scale() {
        let (project, profile) = body_fixture();
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 1.5, 2.] {
                for menu in [Menu::Project, Menu::View, Menu::Body] {
                    let mut ui = Ui::with_theme(scale, theme, [0., 0.]);
                    draw_menu(
                        &mut ui,
                        [1100., 760.],
                        [244., 58., 578., 556.],
                        menu,
                        &project,
                        Some(&profile),
                        Some(5),
                        false,
                        true,
                        true,
                        [true, false, true],
                    );
                    for hit in &ui.hits {
                        let [x, y, w, h] = hit.rect;
                        assert!(
                            x >= 0. && y >= 0. && x + w <= 1100. && y + h <= 760.,
                            "{menu:?}: {:?}",
                            hit.action
                        );
                    }
                    if menu != Menu::Project {
                        let panel = ui.hits[1].rect;
                        assert!(panel[0] >= 244. && panel[0] + panel[2] <= 822.);
                    }
                }
            }
        }
    }

    #[test]
    fn body_menu_disables_unavailable_bones_and_invalid_ik_chains() {
        let (mut project, profile) = body_fixture();
        let ui = menu_ui(Menu::Body, &project, Some(&profile));
        assert_eq!(
            ui.hits
                .iter()
                .filter(|hit| matches!(hit.action, Action::SelectRole(_)))
                .count(),
            6
        );
        assert_eq!(
            ui.hits
                .iter()
                .filter(|hit| matches!(hit.action, Action::IkRole(_)))
                .count(),
            4
        );
        project.node_mut(5).unwrap().visible = false;
        project.node_mut(7).unwrap().locked = true;
        project.node_mut(10).unwrap().parent = Some(1);
        let ui = menu_ui(Menu::Body, &project, Some(&profile));
        assert_eq!(
            ui.hits
                .iter()
                .filter(|hit| matches!(hit.action, Action::SelectRole(_)))
                .count(),
            5
        );
        assert_eq!(
            ui.hits
                .iter()
                .filter(|hit| matches!(hit.action, Action::IkRole(_)))
                .count(),
            1
        );
        let ui = menu_ui(Menu::Body, &project, None);
        assert!(!ui
            .hits
            .iter()
            .any(|hit| matches!(hit.action, Action::SelectRole(_) | Action::IkRole(_))));
        assert!(ui
            .hits
            .iter()
            .any(|hit| hit.action == Action::CharacterPanel));
    }

    #[test]
    fn frame_selected_requires_an_existing_selection() {
        let mut ui = Ui::new(1.);
        draw_menu(
            &mut ui,
            [1100., 760.],
            [0., 58., 1100., 556.],
            Menu::View,
            &Project::default(),
            None,
            Some(999),
            true,
            false,
            false,
            [true, false, true],
        );
        assert!(!ui
            .hits
            .iter()
            .any(|hit| hit.action == Action::FrameSelected));
        for action in [
            Action::Front,
            Action::Side,
            Action::Top,
            Action::Perspective,
            Action::ToggleGrid,
            Action::ToggleSkeleton,
        ] {
            assert!(ui.hits.iter().any(|hit| hit.action == action));
        }
    }
}
