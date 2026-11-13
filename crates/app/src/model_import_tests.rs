//! The .blend import preference is workspace state; background progress must
//! distinguish Blender's unknown duration from measured model parsing.
use super::*;

fn import_app() -> App {
    let mut app = App::new(None);
    app.preferences = Preferences::default();
    app.width = 1100.;
    app.height = 760.;
    app.dialog = Some(FileDialog {
        purpose: FilePurpose::Import,
        path: "/__poseforge_qa__/Модели/Персонаж.blend".into(),
        entries: (0..9)
            .map(|i| {
                (
                    PathBuf::from(format!("/__poseforge_qa__/Модели/Персонаж {i}.blend")),
                    false,
                )
            })
            .collect(),
        offset: 0,
    });
    app
}

fn checkbox_rect(ui: &Ui) -> [f32; 4] {
    let hits: Vec<_> = ui
        .hits
        .iter()
        .filter(|hit| hit.action == Action::ToggleBlendImportAnimations)
        .collect();
    assert_eq!(hits.len(), 1);
    hits[0].rect
}

#[test]
fn blend_checkbox_fits_small_window_themes_and_dpi_with_a_shared_caption_hit() {
    for theme in [ui::Theme::Dark, ui::Theme::Light] {
        for scale in [1., 1.5, 2.] {
            let mut app = import_app();
            app.preferences.theme = theme;
            app.scale = scale;
            let ui = app.build_ui();
            let row = checkbox_rect(&ui);
            for x in [row[0] + 12., row[0] + 220., row[0] + row[2] - 8.] {
                assert_eq!(
                    ui.hit([x, row[1] + row[3] * 0.5]),
                    Some(Action::ToggleBlendImportAnimations)
                );
            }
            assert!(ui.texts.iter().any(|text| {
                text.text == "Импортировать готовую анимацию (.blend)"
                    && text.y >= row[1] * scale
                    && text.y + text.size <= (row[1] + row[3]) * scale
            }));
            for hit in &ui.hits {
                assert!(hit.rect[0] >= 160. && hit.rect[1] >= 50.);
                assert!(hit.rect[0] + hit.rect[2] <= 940.);
                assert!(hit.rect[1] + hit.rect[3] <= 710.);
                if hit.action != Action::ToggleBlendImportAnimations {
                    assert!(
                        intersect_rect(row, hit.rect).is_none(),
                        "overlap with {:?}",
                        hit.action
                    );
                }
            }
            assert_eq!(
                ui.hits
                    .iter()
                    .filter(|hit| matches!(hit.action, Action::FileEntry(_)))
                    .count(),
                9
            );
        }
    }
}

#[test]
fn clicking_checkbox_and_caption_changes_only_workspace_preference() {
    let mut app = import_app();
    let project = app.editor.project.clone();
    let selected = app.editor.selected;
    for (offset, checked) in [(12., true), (260., false)] {
        app.ui = app.build_ui();
        let row = checkbox_rect(&app.ui);
        app.mouse_move([row[0] + offset, row[1] + row[3] * 0.5]);
        app.mouse_down(MouseButton::Left);
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.preferences.blend_import_animations, checked);
        assert_eq!(app.editor.project, project);
        assert_eq!(app.editor.selected, selected);
        assert_eq!(app.editor.undo_len(), 0);
        assert!(!app.dirty);
    }
}

#[test]
fn import_keyboard_reaches_checkbox_and_preserves_text_input_and_ime() {
    let mut app = import_app();
    for _ in 0..12 {
        assert!(app.import_dialog_key(&Key::Named(NamedKey::Tab), false, false));
        if app.import_focus == Some(Action::ToggleBlendImportAnimations) {
            break;
        }
    }
    assert_eq!(app.import_focus, Some(Action::ToggleBlendImportAnimations));
    assert!(app.input.is_none());
    let ui = app.build_ui();
    let row = checkbox_rect(&ui);
    // The focused control has a complete rounded ring. Check the visible
    // result on all four sides instead of requiring a square top-edge quad.
    for point in [
        [row[0] + row[2] * 0.5, row[1] + 0.5],
        [row[0] + row[2] * 0.5, row[1] + row[3] - 0.5],
        [row[0] + 0.5, row[1] + row[3] * 0.5],
        [row[0] + row[2] - 0.5, row[1] + row[3] * 0.5],
    ] {
        let point = point.map(|value| value * ui.scale);
        let visible = ui
            .rects
            .iter()
            .rev()
            .find(|rect| crate::ui::contains(rect.rect, point));
        assert_eq!(
            visible.map(|rect| rect.color),
            Some(app.preferences.palette().accent),
            "keyboard focus must be visible around the entire control"
        );
    }
    assert!(app.import_dialog_key(&Key::Named(NamedKey::Space), false, false));
    assert!(app.preferences.blend_import_animations);
    assert!(app.import_dialog_key(&Key::Named(NamedKey::Space), false, true));
    assert!(
        app.preferences.blend_import_animations,
        "held space must not toggle repeatedly"
    );
    assert!(app.import_dialog_key(&Key::Named(NamedKey::Enter), false, false));
    assert!(!app.preferences.blend_import_animations);
    assert!(app.import_dialog_key(&Key::Named(NamedKey::Tab), true, false));
    assert_eq!(app.import_focus, Some(Action::DetectBlender));

    app.action(Action::EditPath);
    assert!(!app.import_dialog_key(&Key::Named(NamedKey::Space), false, false));
    assert!(!app.import_dialog_key(&Key::Named(NamedKey::Enter), false, false));
    app.input.as_mut().unwrap().preedit = "Персонаж".into();
    assert!(app.import_dialog_key(&Key::Named(NamedKey::Tab), false, false));
    assert_eq!(app.input.as_ref().unwrap().purpose, Action::EditPath);
    assert_eq!(app.input.as_ref().unwrap().preedit, "Персонаж");
    assert_eq!(app.editor.undo_len(), 0);
    assert!(!app.dirty);
}

#[test]
fn blend_checkbox_obeys_modal_and_busy_boundaries() {
    let mut app = import_app();
    app.import_focus = Some(Action::ToggleBlendImportAnimations);
    app.help = true;
    assert!(!app.import_dialog_key(&Key::Named(NamedKey::Space), false, false));
    app.action(Action::ToggleBlendImportAnimations);
    assert!(!app.preferences.blend_import_animations);
    app.help = false;

    let (_tx, receiver) = mpsc::channel();
    app.job = Some(Job {
        receiver,
        cancel: Arc::new(AtomicBool::new(false)),
        label: "Импорт".into(),
    });
    assert!(!app
        .build_ui()
        .hits
        .iter()
        .any(|hit| hit.action == Action::ToggleBlendImportAnimations));
    assert!(!app.import_dialog_key(&Key::Named(NamedKey::Space), false, false));
    app.action(Action::ToggleBlendImportAnimations);
    assert!(!app.preferences.blend_import_animations);
    app.job = None;

    for purpose in [
        FilePurpose::Open,
        FilePurpose::Texture,
        FilePurpose::Save,
        FilePurpose::Rig,
        FilePurpose::Export,
        FilePurpose::Sequence,
    ] {
        app.dialog.as_mut().unwrap().purpose = purpose;
        assert!(!app
            .build_ui()
            .hits
            .iter()
            .any(|hit| hit.action == Action::ToggleBlendImportAnimations));
        assert!(!app.import_dialog_key(&Key::Named(NamedKey::Space), false, false));
        app.action(Action::ToggleBlendImportAnimations);
        assert!(!app.preferences.blend_import_animations);
    }
    app.dialog = None;
    assert!(!app.import_dialog_key(&Key::Named(NamedKey::Tab), false, false));
}

#[test]
fn indeterminate_import_keeps_phase_and_elapsed_visible_until_measured_progress() {
    let phase = "Blender: экспорт модели · прошло 37 с";
    for value in [-1., f32::NAN, f32::INFINITY] {
        assert_eq!(
            import_progress_label(value, phase),
            format!("Импорт · {phase}")
        );
    }
    assert_eq!(
        import_progress_label(0.72, "Чтение GLB"),
        "Импорт 72% · Чтение GLB"
    );
    assert_eq!(import_progress_label(1., "Готово"), "Импорт 100% · Готово");

    let mut app = import_app();
    app.dialog = None;
    app.ui = app.build_ui();
    let hover = app
        .ui
        .hits
        .iter()
        .find(|hit| hit.action.hint().is_some())
        .unwrap()
        .rect;
    app.mouse = [hover[0] + hover[2] * 0.5, hover[1] + hover[3] * 0.5];
    let (tx, receiver) = mpsc::channel();
    app.job = Some(Job {
        receiver,
        cancel: Arc::new(AtomicBool::new(false)),
        label: "Импорт".into(),
    });
    for (value, message) in [(-1., phase), (0.72, "Чтение GLB")] {
        let label = import_progress_label(value, message);
        tx.send(JobMessage::Progress(label.clone())).unwrap();
        app.poll_jobs();
        assert_eq!(app.status, label);
        assert!(app
            .build_ui()
            .texts
            .iter()
            .any(|text| text.text == label && text.y == app.height - 21.));
        assert!(app.job.is_some());
    }
}

fn wait_for_import(app: &mut App) -> (usize, Duration) {
    let started = Instant::now();
    let mut turns = 0;
    let mut longest = Duration::ZERO;
    while app.job.is_some() {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "import timed out: {}",
            app.status
        );
        let tick = Instant::now();
        app.poll_jobs();
        longest = longest.max(tick.elapsed());
        // Exercise the actual UI builder between worker events without a GPU.
        assert!(!app.build_ui().texts.is_empty());
        turns += 1;
        std::thread::sleep(Duration::from_millis(1));
    }
    (turns, longest)
}

#[test]
fn real_glb_background_import_preserves_animation_and_is_one_undo() {
    let mut app = import_app();
    app.dialog = None;
    app.preferences.skeleton = false;
    let before = app.editor.project.clone();
    app.start_load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/skinned_triangle.glb"),
        true,
    );
    let (turns, longest) = wait_for_import(&mut app);
    assert!(turns > 0);
    assert_eq!(
        app.editor.project.meshes.len(),
        before.meshes.len() + 1,
        "{}",
        app.status
    );
    assert_eq!(app.editor.project.clips.len(), before.clips.len() + 1);
    assert!(
        app.preferences.skeleton,
        "imported skeleton must be visible even if hidden before import"
    );
    assert_eq!(
        app.editor
            .project
            .nodes
            .iter()
            .filter(|n| n.is_joint)
            .count(),
        before.nodes.iter().filter(|n| n.is_joint).count() + 2
    );
    assert_eq!(app.editor.project.textures.len(), before.textures.len() + 1);
    assert_eq!(app.editor.undo_len(), 1);
    let imported = app.editor.project.clone();
    assert!(app.editor.undo());
    assert_eq!(app.editor.project, before);
    assert!(app.editor.redo());
    assert_eq!(app.editor.project, imported);
    eprintln!("GLB import UI turns: {turns}; longest poll: {longest:?}");
}

#[test]
fn real_dense_obj_background_import_keeps_processing_ui_events() {
    use std::fmt::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid.obj");
    let side = 128;
    let mut obj = String::from("\u{feff}s 1\n");
    for y in 0..=side {
        for x in 0..=side {
            writeln!(obj, "v {x} 0 {y}").unwrap();
        }
    }
    for y in 0..side {
        for x in 0..side {
            let a = y * (side + 1) + x + 1;
            writeln!(obj, "f {a} {} {} {}", a + 1, a + side + 2, a + side + 1).unwrap();
        }
    }
    std::fs::write(&path, obj).unwrap();
    let mut app = import_app();
    app.preferences.skeleton = false;
    app.dialog = None;
    let before = app.editor.project.clone();
    app.start_load(path, true);
    let (turns, longest) = wait_for_import(&mut app);
    assert!(!app.preferences.skeleton);
    assert!(app
        .last_import_report
        .contains("OBJ не хранит кости и веса"));
    assert_eq!(
        app.editor.project.meshes.len(),
        before.meshes.len() + 1,
        "{}",
        app.status
    );
    let mesh = app.editor.project.meshes.last().unwrap();
    assert_eq!(mesh.vertices.len(), (side + 1) * (side + 1));
    assert_eq!(mesh.indices.len(), side * side * 6);
    assert_eq!(app.editor.undo_len(), 1);
    eprintln!("OBJ 32768 triangles UI turns: {turns}; longest poll: {longest:?}");
}

#[test]
fn failed_and_cancelled_background_imports_leave_scene_and_history_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.obj");
    std::fs::write(&path, "v 0 0 0\nf 1 2 3\n").unwrap();
    let mut app = import_app();
    app.dialog = None;
    let before = app.editor.project.clone();
    app.start_load(path.clone(), true);
    wait_for_import(&mut app);
    assert!(app.status.contains("индекс"), "{}", app.status);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
    std::fs::write(&path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    app.start_load(path, true);
    app.action(Action::Cancel);
    wait_for_import(&mut app);
    assert!(app.status.contains("отменена"), "{}", app.status);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo_len(), 0);
}

#[test]
fn progress_queue_is_bounded_per_ui_turn_and_does_not_hide_cancellation() {
    let mut app = import_app();
    app.dialog = None;
    let (tx, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    app.job = Some(Job {
        receiver,
        cancel: cancel.clone(),
        label: "Импорт".into(),
    });
    for i in 0..1000 {
        tx.send(JobMessage::Progress(format!("Phase {i}"))).unwrap();
    }
    app.poll_jobs();
    assert_eq!(app.status, "Phase 63");
    cancel.store(true, Ordering::Relaxed);
    drop(tx);
    wait_for_import(&mut app);
    assert!(app.status.contains("отменена"));
    assert_eq!(app.editor.undo_len(), 0);
    let (tx, receiver) = mpsc::channel();
    app.job = Some(Job {
        receiver,
        cancel: Arc::new(AtomicBool::new(false)),
        label: "Импорт".into(),
    });
    drop(tx);
    app.poll_jobs();
    assert!(app.status.contains("без результата"));
}
