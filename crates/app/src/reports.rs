//! Copy complete report data, independently of visual wrapping and pagination.
use super::{Action, App, Icon, Ui};
use poseforge_core::Project;
use poseforge_io::RigManifest;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use winit::keyboard::Key;

const COPIED: &str = "Отчёт целиком скопирован в буфер обмена.";
const CLIPBOARD_UNAVAILABLE: &str = "Буфер обмена недоступен. Отчёт не скопирован.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ReportCopyFeedback {
    Copied,
    Failed(String),
}

impl ReportCopyFeedback {
    fn message(&self) -> &str {
        match self {
            Self::Copied => COPIED,
            Self::Failed(message) => message,
        }
    }
}

/// Preserve every manifest row, its full name/path and every comparison
/// diagnostic. The on-screen list's scroll offset and shortened labels do not
/// participate in this representation.
fn rig_report_text(manifest: &RigManifest, project: &Project) -> String {
    let report = manifest.compare(project);
    let matched: BTreeMap<_, _> = report
        .matched
        .iter()
        .map(|(name, id)| (name.as_str(), *id))
        .collect();
    let ambiguous: BTreeSet<_> = report.ambiguous.iter().map(String::as_str).collect();
    let mut text = format!(
        "Проверка списка костей\nArmature: {}\nBones: {}\n{}\n\n\
         TXT содержит имена и связи. Модель и управляющий риг загружаются отдельно.\n\n\
         Все кости:\nРезультат\tИмя кости\tРодитель в списке\tУзел сцены\tПуть в списке\n",
        manifest.armature,
        manifest.bones.len(),
        report.summary(),
    );
    for bone in &manifest.bones {
        let node = matched.get(bone.name.as_str());
        let state = if node.is_some() {
            "Найдено"
        } else if ambiguous.contains(bone.name.as_str()) {
            "Несколько"
        } else {
            "Нет в сцене"
        };
        // Writing into a String cannot fail. No display length limits apply.
        let _ = writeln!(
            text,
            "{}\t{}\t{}\t{}\t{}",
            state,
            bone.name,
            bone.parent.as_deref().unwrap_or("корень"),
            node.map(|id| id.to_string()).unwrap_or_else(|| "—".into()),
            bone.path.join(" > "),
        );
    }
    text.push_str("\nРазличия иерархии:\n");
    if report.hierarchy_mismatches.is_empty() {
        text.push_str("Нет.\n");
    } else {
        for mismatch in &report.hierarchy_mismatches {
            let _ = writeln!(text, "{mismatch}");
        }
    }
    text
}

fn clipboard_error(error: arboard::Error) -> String {
    match error {
        arboard::Error::ClipboardOccupied => {
            "Буфер обмена занят. Повторите копирование отчёта.".into()
        }
        arboard::Error::ClipboardNotSupported => CLIPBOARD_UNAVAILABLE.into(),
        arboard::Error::ConversionFailure => {
            "Не удалось передать текст отчёта в буфер обмена.".into()
        }
        _ => "Не удалось записать отчёт в буфер обмена. Повторите попытку.".into(),
    }
}

fn write_system_clipboard(
    clipboard: &mut Option<arboard::Clipboard>,
    text: String,
) -> Result<(), String> {
    clipboard
        .as_mut()
        .ok_or_else(|| CLIPBOARD_UNAVAILABLE.to_owned())?
        .set_text(text)
        .map_err(clipboard_error)
}

impl App {
    pub(super) fn open_details_report(&mut self, text: String) {
        self.details_text = text;
        self.details_page = 0;
        self.details = true;
        self.rig_details = false;
        self.report_copy_feedback = None;
    }

    pub(super) fn open_rig_report(&mut self) {
        self.rig_details = true;
        self.rig_scroll = 0;
        self.details = false;
        self.report_copy_feedback = None;
    }

    fn can_copy_report(&self) -> bool {
        if self.rig_details {
            self.rig_manifest.is_some()
        } else {
            self.details
        }
    }

    fn full_open_report(&self) -> Result<String, String> {
        // The rig panel is drawn above details. Never copy a report hidden by it.
        if self.rig_details {
            self.rig_manifest
                .as_ref()
                .map(|manifest| rig_report_text(manifest, &self.editor.project))
                .ok_or_else(|| "Сначала откройте список костей TXT для сравнения.".into())
        } else if self.details {
            // Preserve whitespace, Unicode, long paths and the final newline.
            Ok(self.details_text.clone())
        } else {
            Err("Сначала откройте отчёт, который нужно скопировать.".into())
        }
    }

    pub(super) fn copy_open_report(&mut self) {
        self.copy_open_report_with(write_system_clipboard);
    }

    /// The injected writer lets checks cover failure and success without
    /// changing the developer's system clipboard or requiring a desktop.
    fn copy_open_report_with(
        &mut self,
        write: impl FnOnce(&mut Option<arboard::Clipboard>, String) -> Result<(), String>,
    ) {
        let result = self
            .full_open_report()
            .and_then(|text| write(&mut self.clipboard, text));
        let feedback = match result {
            Ok(()) => ReportCopyFeedback::Copied,
            Err(error) => ReportCopyFeedback::Failed(error),
        };
        self.status = feedback.message().to_owned();
        self.report_copy_feedback = Some(feedback);
    }

    /// Call after the text editor has handled its own keyboard input.
    pub(super) fn report_shortcut(&self, key: &Key, ctrl: bool) -> Option<Action> {
        (self.input.is_none()
            && !self.help
            && self.can_copy_report()
            && ctrl
            && matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("c")))
        .then_some(Action::CopyReport)
    }

    pub(super) fn draw_report_copy_button(&self, ui: &mut Ui, rect: [f32; 4]) {
        if self.can_copy_report() {
            ui.icon_button(
                rect,
                "Копировать отчёт",
                Action::CopyReport,
                false,
                Icon::Copy,
            );
        } else {
            ui.disabled_icon_button(rect, "Копировать отчёт", Icon::Copy);
        }
    }

    pub(super) fn draw_report_copy_feedback(&self, ui: &mut Ui, rect: [f32; 4]) {
        let palette = ui.palette();
        let (message, color) = match &self.report_copy_feedback {
            Some(ReportCopyFeedback::Copied) => (COPIED, palette.accent),
            Some(ReportCopyFeedback::Failed(message)) => (message.as_str(), palette.danger),
            None if self.can_copy_report() => ("Ctrl+C — скопировать весь отчёт", palette.muted),
            None => return,
        };
        ui.fitted_label(rect, message, 11., color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::TextEdit;
    use poseforge_core::{Editor, Node};
    use poseforge_io::RigBone;

    fn app() -> App {
        let mut app = App::new(None);
        // These checks never read or write the operating-system clipboard.
        app.clipboard = None;
        app
    }

    fn manifest(names: &[&str]) -> RigManifest {
        RigManifest {
            armature: "Персонаж с длинным именем 骨格".into(),
            declared_count: names.len(),
            bones: names
                .iter()
                .map(|name| RigBone {
                    name: (*name).into(),
                    parent: None,
                    path: vec![(*name).into()],
                })
                .collect(),
        }
    }

    #[test]
    fn copy_preserves_the_complete_original_report_on_every_page_and_after_feedback() {
        let mut app = app();
        let text = format!(
            "Заголовок\r\n\n{}\nПуть:\tC:\\Модели с пробелами\\{}\\текстура.png\n",
            (0..80)
                .map(|i| format!("Предупреждение {i}: полный текст 骨格"))
                .collect::<Vec<_>>()
                .join("\n"),
            "д".repeat(240),
        );
        let project = app.editor.project.clone();
        let undo = app.editor.undo_len();
        let dirty = app.dirty;
        app.open_details_report(text.clone());
        for page in [0, 2, 5, 999] {
            app.details_page = page;
            let mut copied = None;
            app.copy_open_report_with(|_, text| {
                copied = Some(text);
                Ok(())
            });
            assert_eq!(copied.as_deref(), Some(text.as_str()));
            assert_eq!(app.details_text, text);
            assert_eq!(app.details_page, page);
            assert_eq!(app.status, COPIED);
            assert_eq!(app.report_copy_feedback, Some(ReportCopyFeedback::Copied));
        }
        assert_eq!(app.editor.project, project);
        assert_eq!(app.editor.undo_len(), undo);
        assert_eq!(app.dirty, dirty);
    }

    #[test]
    fn rig_copy_includes_all_rows_paths_states_and_hierarchy_diagnostics() {
        let mut app = app();
        let long_name = format!("Отсутствующая_{}", "кость".repeat(70));
        let mut manifest = manifest(&["Root", "Child", "Duplicate", &long_name]);
        manifest.bones[1].parent = Some("Root".into());
        manifest.bones[1].path = vec!["Root".into(), "Child".into()];
        for i in 0..40 {
            let name = format!("Дополнительная кость {i}");
            manifest.bones.push(RigBone {
                name: name.clone(),
                parent: Some("Root".into()),
                path: vec!["Root".into(), name],
            });
        }
        manifest.declared_count = manifest.bones.len();
        let project = Project {
            nodes: vec![
                Node::new(1, "Root"),
                Node::new(2, "Child"), // Missing the expected Root parent.
                Node::new(3, "Duplicate"),
                Node::new(4, "Duplicate"),
            ],
            ..Project::default()
        };
        app.editor = Editor::new(project);
        let before = app.editor.project.clone();
        app.rig_manifest = Some(manifest);
        app.open_rig_report();
        app.rig_scroll = 30;
        let mut copied = String::new();
        app.copy_open_report_with(|_, text| {
            copied = text;
            Ok(())
        });
        assert!(copied.contains("Armature: Персонаж с длинным именем 骨格"));
        assert!(copied.contains("Bones: 44"));
        assert!(copied.contains("Найдено\tRoot\tкорень\t1\tRoot"));
        assert!(copied.contains("Найдено\tChild\tRoot\t2\tRoot > Child"));
        assert!(copied.contains("Несколько\tDuplicate\tкорень\t—\tDuplicate"));
        assert!(copied.contains(&format!("Нет в сцене\t{long_name}\t")));
        assert!(copied.contains("Child: ожидается Root, в сцене корень"));
        for i in 0..40 {
            assert!(copied.contains(&format!("\tRoot > Дополнительная кость {i}\n")));
        }
        assert_eq!(app.rig_scroll, 30);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
    }

    #[test]
    fn failed_copy_replaces_previous_success_without_changing_report() {
        let mut app = app();
        app.open_details_report("Исходная диагностика\n".into());
        app.copy_open_report_with(|_, _| Ok(()));
        app.copy_open_report_with(|_, text| {
            assert_eq!(text, "Исходная диагностика\n");
            Err(CLIPBOARD_UNAVAILABLE.into())
        });
        assert_eq!(app.status, CLIPBOARD_UNAVAILABLE);
        assert_eq!(
            app.report_copy_feedback,
            Some(ReportCopyFeedback::Failed(CLIPBOARD_UNAVAILABLE.into()))
        );
        assert_eq!(app.details_text, "Исходная диагностика\n");
    }

    #[test]
    fn missing_clipboard_reports_failure_through_the_production_writer() {
        let mut app = app();
        app.open_details_report("Полный отчёт".into());
        app.copy_open_report();
        assert_eq!(app.status, CLIPBOARD_UNAVAILABLE);
        assert!(matches!(
            app.report_copy_feedback,
            Some(ReportCopyFeedback::Failed(_))
        ));
        assert_eq!(app.details_text, "Полный отчёт");
    }

    #[test]
    fn no_report_or_missing_manifest_never_calls_the_clipboard_writer() {
        let mut app = app();
        app.copy_open_report_with(|_, _| panic!("There is no open report"));
        assert!(app.status.contains("Сначала откройте отчёт"));
        app.details_text = "Скрытый отчёт нельзя скопировать".into();
        app.details = true;
        app.rig_details = true;
        app.copy_open_report_with(|_, _| panic!("There is no rig manifest"));
        assert!(app.status.contains("список костей TXT"));
    }

    #[test]
    fn opening_another_report_resets_copy_feedback_and_the_previous_panel() {
        let mut app = app();
        app.open_details_report("Первый отчёт".into());
        app.copy_open_report_with(|_, _| Ok(()));
        app.details_page = 4;
        app.open_details_report("Следующий отчёт".into());
        assert_eq!(app.report_copy_feedback, None);
        assert_eq!(app.details_page, 0);
        app.rig_scroll = 30;
        app.open_rig_report();
        assert!(!app.details);
        assert!(app.rig_details);
        assert_eq!(app.rig_scroll, 0);
        assert_eq!(app.report_copy_feedback, None);
        app.open_details_report("Статус операции".into());
        assert!(app.details);
        assert!(!app.rig_details);
    }

    #[test]
    fn report_shortcut_is_scoped_to_open_reports_and_never_steals_field_copy() {
        let mut app = app();
        let copy_key = Key::Character("c".into());
        assert_eq!(app.report_shortcut(&copy_key, true), None);
        app.open_details_report("Диагностика".into());
        assert_eq!(
            app.report_shortcut(&copy_key, true),
            Some(Action::CopyReport)
        );
        assert_eq!(app.report_shortcut(&copy_key, false), None);
        assert_eq!(app.report_shortcut(&Key::Character("v".into()), true), None);
        app.help = true;
        assert_eq!(app.report_shortcut(&copy_key, true), None);
        app.help = false;
        app.input = Some(TextEdit::new("Имя".into(), Action::EditTitle));
        assert_eq!(app.report_shortcut(&copy_key, true), None);
        app.input = None;
        app.open_rig_report();
        assert_eq!(app.report_shortcut(&copy_key, true), None);
        app.rig_manifest = Some(manifest(&["Root"]));
        assert_eq!(
            app.report_shortcut(&copy_key, true),
            Some(Action::CopyReport)
        );
    }

    #[test]
    fn copy_control_and_feedback_stay_in_the_report_surface() {
        let mut app = app();
        app.open_details_report("Диагностика".into());
        let mut ui = Ui::new(1.);
        app.draw_report_copy_button(&mut ui, [24., 50., 270., 34.]);
        app.draw_report_copy_feedback(&mut ui, [24., 20., 752., 18.]);
        assert!(ui.hits.iter().any(|hit| hit.action == Action::CopyReport));
        assert!(ui.texts.iter().any(|text| text.text.contains("весь отчёт")));
        app.copy_open_report();
        ui.texts.clear();
        app.draw_report_copy_feedback(&mut ui, [24., 20., 752., 18.]);
        assert!(ui
            .texts
            .iter()
            .any(|text| text.text == CLIPBOARD_UNAVAILABLE));
        app.open_rig_report();
        let mut ui = Ui::new(1.);
        app.draw_report_copy_button(&mut ui, [24., 50., 270., 34.]);
        assert!(ui.hits.is_empty());
    }

    #[test]
    fn clipboard_errors_are_readable_and_never_claim_success() {
        let occupied = clipboard_error(arboard::Error::ClipboardOccupied);
        assert!(occupied.contains("занят"));
        assert!(occupied.contains("Повторите"));
        assert_eq!(
            clipboard_error(arboard::Error::ClipboardNotSupported),
            CLIPBOARD_UNAVAILABLE
        );
        assert!(clipboard_error(arboard::Error::ConversionFailure).starts_with("Не удалось"));
        assert!(clipboard_error(arboard::Error::Unknown {
            description: "backend unavailable".into()
        })
        .starts_with("Не удалось"));
    }
}
