//! Native modal workspaces for character mapping and keyframe authoring.
//! These functions only emit UI actions; the editor owns all data changes.
use crate::ui::{Action, SliderRange, Ui};
use crate::workspace::shortened;
use poseforge_core::{BoneRole, DetectionReport, Project, RigProfile};
use std::collections::BTreeSet;

pub const NODE_PAGE_SIZE: usize = 12;
pub const CLIP_PAGE_SIZE: usize = 6;

/// Shared by the picker and its wheel handler so filtering and scrolling agree.
/// `Some(None)` selects a root; `Some(Some(index))` assigns a body role.
pub fn candidate_nodes(
    project: &Project,
    profile: &RigProfile,
    picker: Option<Option<usize>>,
    query: &str,
) -> Vec<u64> {
    let Some(role_index) = picker else {
        return Vec::new();
    };
    if role_index.is_some_and(|index| index >= BoneRole::ALL.len()) {
        return Vec::new();
    }
    let query = query.trim().to_lowercase();
    let mut nodes: Vec<_> = project
        .nodes
        .iter()
        .filter(|node| {
            node.name.to_lowercase().contains(&query)
                && (role_index.is_none()
                    || (node.is_joint && within_scope(project, node.id, profile.root)))
        })
        .collect();
    nodes.sort_by_cached_key(|node| (node.name.to_lowercase(), node.id));
    nodes.into_iter().map(|node| node.id).collect()
}

fn within_scope(project: &Project, node: u64, root: Option<u64>) -> bool {
    let Some(root) = root else {
        return true;
    };
    let mut cursor = Some(node);
    let mut visited = BTreeSet::new();
    while let Some(id) = cursor {
        if id == root {
            return true;
        }
        if !visited.insert(id) {
            return false;
        }
        cursor = project.node(id).and_then(|node| node.parent);
    }
    false
}

fn modal(u: &mut Ui, width: f32, height: f32, size: [f32; 2]) -> [f32; 2] {
    let p = u.palette();
    u.hits.clear();
    u.texts.clear();
    u.rect([0., 0., width, height], [0.02, 0.025, 0.035, 0.64]);
    let x = (width - size[0]) / 2.;
    let y = (height - size[1]) / 2.;
    u.outlined_surface([x, y, size[0], size[1]], p.panel, p.border);
    [x, y]
}

#[allow(clippy::too_many_arguments)]
pub fn draw_character_dialog(
    u: &mut Ui,
    width: f32,
    height: f32,
    project: &Project,
    profile: &RigProfile,
    report: Option<&DetectionReport>,
    picker: Option<Option<usize>>,
    node_query: &str,
    node_offset: usize,
    status: &str,
) {
    let p = u.palette();
    let [x, y] = modal(u, width, height, [960., 600.]);
    let root_name = profile
        .root
        .and_then(|id| project.node(id))
        .map(|node| node.name.as_str())
        .unwrap_or("Выбрать персонажа…");
    if let Some(role_index) = picker {
        let role = role_index.and_then(|index| BoneRole::ALL.get(index).copied());
        u.label(
            x + 24.,
            y + 20.,
            role.map(|role| format!("Назначить: {}", role.label()))
                .unwrap_or_else(|| "Корень персонажа".into()),
            21.,
            p.text,
        );
        u.label(
            x + 24.,
            y + 52.,
            if role_index.is_some() {
                "Выберите кость для этой части тела."
            } else {
                "Выберите общий родительский объект скелета."
            },
            12.,
            p.muted,
        );
        u.label(x + 24., y + 88., "Поиск", 12., p.muted);
        u.field(
            [x + 84., y + 80., 852., 32.],
            node_query,
            Action::SearchBones,
        );
        u.label(
            x + 24.,
            y + 123.,
            if role_index.is_some() {
                shortened(root_name, 100)
            } else {
                "Вся сцена".into()
            },
            11.,
            p.muted,
        );
        let nodes = candidate_nodes(project, profile, picker, node_query);
        let offset = node_offset.min(nodes.len().saturating_sub(NODE_PAGE_SIZE));
        for (row, &id) in nodes.iter().skip(offset).take(NODE_PAGE_SIZE).enumerate() {
            let Some(node) = project.node(id) else {
                continue;
            };
            let parent = node
                .parent
                .and_then(|id| project.node(id))
                .map(|node| node.name.as_str())
                .unwrap_or("корень сцены");
            let suggestion = role
                .and_then(|role| report.and_then(|report| report.candidates.get(&role)))
                .and_then(|candidates| candidates.iter().find(|candidate| candidate.node == id));
            let kind = suggestion
                .map(|candidate| candidate.confidence.label())
                .unwrap_or(if node.is_joint {
                    "кость"
                } else {
                    "объект"
                });
            let label = format!("{}  ·  #{}  ·  {}  ·  {}", node.name, id, kind, parent);
            let selected = role
                .and_then(|role| profile.assignments.get(&role).copied())
                .or(if role_index.is_none() {
                    profile.root
                } else {
                    None
                });
            u.button(
                [x + 24., y + 150. + row as f32 * 28., 912., 25.],
                &label,
                if role_index.is_none() {
                    Action::SetRigRoot(id)
                } else {
                    Action::AssignBone(id)
                },
                selected == Some(id),
            );
        }
        if nodes.is_empty() {
            u.label(x + 34., y + 178., "Подходящих узлов нет", 15., p.text);
            u.label(
                x + 34.,
                y + 208.,
                "Измените поиск или выберите другой корень.",
                12.,
                p.muted,
            );
        }
        u.label(
            x + 24.,
            y + 500.,
            format!(
                "{}–{} из {}",
                if nodes.is_empty() { 0 } else { offset + 1 },
                (offset + NODE_PAGE_SIZE).min(nodes.len()),
                nodes.len()
            ),
            11.,
            p.muted,
        );
        u.label(x + 24., y + 526., shortened(status, 122), 11., p.warn);
        u.button(
            [x + 24., y + 552., 126., 32.],
            "Назад",
            Action::BackToRig,
            false,
        );
        return;
    }

    u.label(x + 24., y + 20., "Персонаж", 21., p.text);
    u.button(
        [x + 836., y + 20., 100., 30.],
        "Закрыть",
        Action::Cancel,
        false,
    );
    u.button(
        [x + 24., y + 72., 690., 32.],
        &format!("Корень: {root_name}"),
        Action::PickRigRoot,
        false,
    );
    u.button(
        [x + 726., y + 72., 210., 32.],
        "Распознать кости",
        Action::DetectSkeleton,
        profile.assignments.is_empty(),
    );
    u.label(
        x + 24.,
        y + 120.,
        format!(
            "Соответствия  ·  {} из {}",
            profile.assignments.len(),
            BoneRole::ALL.len()
        ),
        11.,
        p.muted,
    );
    for (index, role) in BoneRole::ALL.iter().copied().enumerate() {
        let column = index / 9;
        let row = index % 9;
        let rx = x + 24. + column as f32 * 468.;
        let ry = y + 150. + row as f32 * 36.;
        let assigned = profile
            .assignments
            .get(&role)
            .and_then(|id| project.node(*id));
        u.label(rx, ry + 7., shortened(role.label(), 18), 12., p.muted);
        let candidates = report
            .and_then(|report| report.candidates.get(&role))
            .map_or(0, Vec::len);
        let caption = assigned.map(|node| node.name.clone()).unwrap_or_else(|| {
            if candidates > 1 {
                format!("Выбрать · {candidates}")
            } else {
                "Назначить…".into()
            }
        });
        u.button(
            [rx + 132., ry, 212., 30.],
            &caption,
            Action::PickBone(index),
            false,
        );
        if assigned.is_some() {
            u.button(
                [rx + 352., ry, 52., 30.],
                "Поза",
                Action::SelectRole(index),
                false,
            );
            u.button(
                [rx + 412., ry, 32., 30.],
                "×",
                Action::ClearBone(index),
                false,
            );
        }
    }
    let warning = report
        .and_then(|report| report.warnings.first())
        .map(String::as_str);
    if let Some(warning) = warning {
        u.label(x + 24., y + 488., shortened(warning, 122), 11., p.warn);
    }
    u.label(x + 24., y + 514., shortened(status, 122), 11., p.warn);
    let saved = project.rig_profiles.iter().any(|saved| saved == profile);
    u.label(
        x + 24.,
        y + 560.,
        if saved {
            "Сохранено в проекте"
        } else {
            "Изменения не сохранены"
        },
        12.,
        p.muted,
    );
    if profile.root.is_some() && !profile.assignments.is_empty() {
        u.button(
            [x + 726., y + 552., 210., 32.],
            "Сохранить",
            Action::SaveRigProfile,
            !saved,
        );
    } else {
        u.disabled_button([x + 726., y + 552., 210., 32.], "Сохранить");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_animation_dialog(
    u: &mut Ui,
    width: f32,
    height: f32,
    project: &Project,
    active_clip: Option<usize>,
    time: f64,
    clip_offset: usize,
    status: &str,
) {
    let p = u.palette();
    let [x, panel_y] = modal(u, width, height, [760., 692.]);
    let y = panel_y + 52.;
    let current = active_clip.and_then(|index| project.clips.get(index));
    u.label(x + 24., panel_y + 20., "Анимация", 21., p.text);
    u.button(
        [x + 636., panel_y + 20., 100., 30.],
        "К сцене",
        Action::Cancel,
        false,
    );
    u.number_field(
        [x + 24., panel_y + 68., 340., 30.],
        "Кадров/с",
        project.fps as f64,
        Action::EditFps,
        SliderRange::new(1., 240., 1.),
    );
    u.number_field(
        [x + 376., panel_y + 68., 360., 30.],
        "Длительность, с",
        project.duration,
        Action::EditDuration,
        SliderRange::new(0., (project.duration * 2.).max(60.), 0.1),
    );
    u.label(x + 24., y + 76., "Клипы", 14., p.text);
    u.button(
        [x + 590., y + 68., 146., 30.],
        "Статичная поза",
        Action::SetClip(project.clips.len()),
        active_clip.is_none(),
    );
    let offset = clip_offset.min(project.clips.len().saturating_sub(CLIP_PAGE_SIZE));
    for (index, clip) in project
        .clips
        .iter()
        .enumerate()
        .skip(offset)
        .take(CLIP_PAGE_SIZE)
    {
        u.button(
            [x + 24., y + 110. + (index - offset) as f32 * 32., 712., 28.],
            &format!(
                "{}  ·  {:.2} с  ·  {} ключевых кадров",
                clip.name,
                clip.duration,
                clip.key_times(None).len()
            ),
            Action::SetClip(index),
            active_clip == Some(index),
        );
    }
    if project.clips.is_empty() {
        u.label(
            x + 24.,
            y + 174.,
            "Создайте клип, чтобы записать движение.",
            14.,
            p.muted,
        );
    }
    let name_width = (712. - 24.) / 3.;
    let create_x = x + 24. + name_width + 12.;
    let duplicate_x = create_x + name_width + 12.;
    u.label(x + 24., y + 316., "Имя выбранного клипа", 11., p.muted);
    u.label(create_x, y + 316., "Новый клип", 11., p.muted);
    u.label(duplicate_x, y + 316., "Копия клипа", 11., p.muted);
    if let Some(clip) = current {
        u.field(
            [x + 24., y + 338., name_width, 32.],
            &clip.name,
            Action::RenameClip,
        );
        u.field(
            [duplicate_x, y + 338., name_width, 32.],
            &format!("{} — копия", clip.name),
            Action::DuplicateClip,
        );
        u.label(x + 24., y + 379., "Enter — переименовать", 11., p.muted);
        u.label(duplicate_x, y + 379., "Enter — создать копию", 11., p.muted);
    } else {
        u.disabled_button([x + 24., y + 338., name_width, 32.], "Выберите клип");
        u.disabled_button([duplicate_x, y + 338., name_width, 32.], "Выберите клип");
    }
    u.field(
        [create_x, y + 338., name_width, 32.],
        &format!("Движение {}", project.clips.len() + 1),
        Action::CreateClip,
    );
    u.label(create_x, y + 379., "Enter — создать клип", 11., p.muted);
    if project.clips.len() > CLIP_PAGE_SIZE {
        u.label(
            x + 260.,
            y + 77.,
            format!(
                "{}–{} из {}",
                offset + 1,
                (offset + CLIP_PAGE_SIZE).min(project.clips.len()),
                project.clips.len()
            ),
            11.,
            p.muted,
        );
    }
    u.rect([x + 24., y + 408., 712., 1.], p.border);
    u.label(x + 24., y + 426., "Кадр", 14., p.text);
    u.button(
        [x + 24., y + 450., 44., 32.],
        "−1",
        Action::PrevFrame,
        false,
    );
    let frame = (time.max(0.) * project.fps as f64).round();
    let last_frame = (project.duration * project.fps as f64)
        .round()
        .max(1.)
        .min(u32::MAX as f64);
    u.number_field(
        [x + 76., y + 450., 324., 44.],
        "Номер кадра",
        frame,
        Action::EditFrame,
        SliderRange::new(0., last_frame, 1.),
    );
    u.button(
        [x + 408., y + 450., 44., 32.],
        "+1",
        Action::NextFrame,
        false,
    );
    u.label(x + 468., y + 458., format!("{time:.3} с"), 12., p.muted);
    u.button(
        [x + 590., y + 450., 146., 32.],
        "Играть / пауза",
        Action::Play,
        false,
    );
    if project.nodes.is_empty() {
        u.disabled_button([x + 24., y + 508., 180., 32.], "Ключ персонажа");
        u.disabled_button([x + 216., y + 508., 180., 32.], "Ключ выбранного");
    } else {
        u.button(
            [x + 24., y + 508., 180., 32.],
            "Ключ персонажа",
            Action::KeyPose,
            true,
        );
        u.button(
            [x + 216., y + 508., 180., 32.],
            "Ключ выбранного",
            Action::Key,
            false,
        );
    }
    u.label(x + 24., y + 566., "Ключи выбранного", 12., p.muted);
    if current.is_some() {
        u.button(
            [x + 216., y + 558., 90., 30.],
            "−1 кадр",
            Action::ShiftKeys(-1),
            false,
        );
        u.button(
            [x + 318., y + 558., 90., 30.],
            "+1 кадр",
            Action::ShiftKeys(1),
            false,
        );
        u.button(
            [x + 420., y + 558., 136., 30.],
            "Удалить ключ",
            Action::DeleteKey,
            false,
        );
    } else {
        u.disabled_button([x + 216., y + 558., 90., 30.], "−1 кадр");
        u.disabled_button([x + 318., y + 558., 90., 30.], "+1 кадр");
        u.disabled_button([x + 420., y + 558., 136., 30.], "Удалить ключ");
    }
    u.label(x + 24., y + 608., shortened(status, 96), 11., p.warn);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Theme;
    use poseforge_core::Node;

    fn two_characters() -> Project {
        let mut project = Project::default();
        for (id, name, parent, joint) in [
            (1, "Персонаж А", None, false),
            (2, "Кисть.L", Some(1), true),
            (3, "Персонаж Б", None, false),
            (4, "Кисть.L", Some(3), true),
        ] {
            let mut node = Node::new(id, name);
            node.parent = parent;
            node.is_joint = joint;
            project.nodes.push(node);
        }
        project
    }

    #[test]
    fn bone_picker_never_offers_a_joint_from_another_character() {
        let project = two_characters();
        let profile = RigProfile {
            root: Some(1),
            ..RigProfile::default()
        };
        assert_eq!(
            candidate_nodes(&project, &profile, Some(Some(0)), "КИСТЬ"),
            vec![2]
        );
        assert_eq!(
            candidate_nodes(&project, &profile, Some(None), "КИСТЬ"),
            vec![2, 4]
        );
        assert!(candidate_nodes(&project, &profile, Some(Some(99)), "").is_empty());
    }

    #[test]
    fn workflow_modals_replace_background_hits_and_fit_small_window() {
        let project = two_characters();
        let profile = RigProfile {
            root: Some(1),
            ..RigProfile::default()
        };
        for theme in [Theme::Dark, Theme::Light] {
            for picker in [None, Some(None), Some(Some(0))] {
                let mut ui = Ui::with_theme(1.5, theme, [0., 0.]);
                ui.button([0., 0., 80., 28.], "Фон", Action::New, false);
                draw_character_dialog(
                    &mut ui, 1100., 760., &project, &profile, None, picker, "", 0, "",
                );
                assert!(!ui.hits.iter().any(|hit| hit.action == Action::New));
                assert!(ui.hits.iter().all(|hit| {
                    let [x, y, w, h] = hit.rect;
                    x >= 0. && y >= 0. && x + w <= 1100. && y + h <= 760.
                }));
            }
            let mut ui = Ui::with_theme(1.5, theme, [0., 0.]);
            draw_animation_dialog(&mut ui, 1100., 760., &project, None, 0., 0, "");
            assert!(!ui
                .hits
                .iter()
                .any(|hit| matches!(hit.action, Action::RenameClip | Action::DuplicateClip)));
            assert!(ui.hits.iter().all(|hit| {
                let [x, y, w, h] = hit.rect;
                x >= 0. && y >= 0. && x + w <= 1100. && y + h <= 760.
            }));
        }
    }
}
