//! Import exactly the supplied local files and report stages, without a GPU.
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};
fn main() {
    for argument in std::env::args_os().skip(1) {
        let path = PathBuf::from(argument);
        let start = Instant::now();
        let imported = poseforge_io::import_model_with_progress(
            &path,
            &AtomicBool::new(false),
            |progress, label| {
                eprintln!(
                    "{:.3}s {:.0}% {label}",
                    start.elapsed().as_secs_f64(),
                    progress * 100.
                )
            },
        );
        match imported {
            Ok(i) => {
                let import_ms = start.elapsed().as_secs_f64() * 1000.;
                let mut p = poseforge_core::Project::default();
                let start = Instant::now();
                match poseforge_io::merge_project(&mut p, i.project) {
                    Ok(ids) => println!(
                        "{}",
                        serde_json::json!({"file":path,"import_ms":import_ms,"merge_ms":start.elapsed().as_secs_f64()*1000.,"roots":ids.len(),"report":i.report})
                    ),
                    Err(e) => {
                        eprintln!("MERGE: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                std::process::exit(1);
            }
        }
    }
}
