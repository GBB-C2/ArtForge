mod animation_overlay;
mod animation_timeline;
mod app;
mod bones;
mod chrome;
mod cli;
mod color_picker;
mod file_picker;
mod gizmo;
mod icons;
mod picking;
mod ui;
mod workflows;
mod workspace;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::run(&args) {
        Ok(true) => return,
        Ok(false) => {}
        Err(e) => {
            eprintln!("ArtForge Studio: {e}");
            std::process::exit(1);
        }
    }
    if let Err(e) = app::run(args.first().map(std::path::PathBuf::from)) {
        eprintln!("ArtForge Studio: {e}");
        std::process::exit(1);
    }
}
