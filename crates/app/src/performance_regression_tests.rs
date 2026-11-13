//! Repeatable CPU probes. Run explicitly with --ignored --nocapture; timings are
//! diagnostic, never pass/fail thresholds tied to the machine running the suite.
use super::*;
use std::hint::black_box;

fn large_scene(count: usize) -> App {
    let mut app = App::new(None);
    app.editor = Editor::new(Project {
        nodes: (1..=count as u64)
            .map(|id| poseforge_core::Node::new(id, format!("Object {id}")))
            .collect(),
        lights: vec![],
        ..Default::default()
    });
    app.editor.mode = EditorMode::Scene;
    app.editor.selected = None;
    app.preferences = Preferences::default();
    app.preferences.skeleton = false;
    app.width = 1440.;
    app.height = 900.;
    app.reset_workflow_selection();
    app
}

fn measure(mut f: impl FnMut()) -> f64 {
    f();
    let mut samples = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        f();
        samples.push(start.elapsed().as_secs_f64() * 1000.);
    }
    samples.sort_by(f64::total_cmp);
    samples[2]
}

#[test]
fn closing_keeps_the_saved_scene_frozen_until_a_write_error_is_reported() {
    let mut app = large_scene(2);
    let before = app.editor.project.clone();
    let (release, wait) = mpsc::channel();
    app.close_recovery.start(move || {
        wait.recv().unwrap();
        Err("disk full".into())
    }).unwrap();
    // A queued scene-changing result must not commit after the closing snapshot
    // was taken, even if a redraw arrives while the disk worker is still busy.
    let (tx, rx) = mpsc::channel();
    tx.send(JobMessage::Loaded(Project::default(), PathBuf::from("later.artforge"))).unwrap();
    app.job = Some(Job { receiver: rx, cancel: Arc::new(AtomicBool::new(false)), label: "Открытие".into() });
    app.tick();
    assert_eq!(app.editor.project, before);
    assert!(app.job.is_some());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match app.close_recovery.poll() {
            Err(message) => { assert_eq!(message, "disk full"); break; }
            Ok(false) => { assert!(Instant::now() < deadline); std::thread::yield_now(); }
            Ok(true) => panic!("failed write must never permit exit"),
        }
    }
    assert!(!app.close_recovery.is_pending());
    assert_eq!(app.editor.project, before);
    // The open editor resumes normal polling after the error has been shown.
    app.tick();
    assert!(app.job.is_none());
    assert!(app.editor.project.nodes.is_empty());
}

#[test]
#[ignore = "manual CPU benchmark; no GPU or wall-clock assertions"]
fn large_scene_cpu_benchmark() {
    for count in [1_000, 5_000, 10_000] {
        let mut app = large_scene(count);
        let tree = measure(|| {
            assert_eq!(black_box(app.outliner_rows()).len(), count);
        });
        let evaluate = measure(|| {
            black_box(app.editor.evaluated().unwrap());
        });
        let ui = measure(|| {
            black_box(app.build_ui());
        });
        println!("nodes={count} outliner_ms={tree:.3} evaluate_ms={evaluate:.3} ui_ms={ui:.3}");
    }
    for scale in [1., 2.] {
        let elapsed = measure(|| {
            let mut ui = Ui::new(scale);
            for i in 0..500 {
                let rect = [0., i as f32 * 30., 200., 26.];
                ui.surface(rect, [0.5; 4]);
                ui.surface_outline(rect, [0.8; 4]);
            }
            black_box(ui);
        });
        println!("rounded_controls=500 scale={scale} rounded_ms={elapsed:.3}");
    }
}
