//! Reproducible CPU-only preview measurements; these are not renderer FPS.
//! Run: cargo run -p poseforge-simulation --example profile --release

use poseforge_simulation::{extract_surface, FluidConfig, FluidError, FluidWorld};
use std::time::Instant;

fn main() -> Result<(), FluidError> {
    println!(
        "profile,particles,ticks,substeps,mean_tick_ms,p95_tick_ms,max_tick_ms,max_density_ratio,max_speed,speed_limit_events,substep_budget_events"
    );
    let profiles = [
        ("default", FluidConfig::default()),
        (
            "quality",
            FluidConfig {
                iterations: 8,
                max_substeps: 8,
                ..Default::default()
            },
        ),
    ];
    for (name, config) in profiles {
        let mut world = FluidWorld::demo_with_config(config)?;
        let mut timings = Vec::with_capacity(120);
        let mut substeps = 0;
        let mut max_density = 0.0_f32;
        let mut max_speed = 0.0_f32;
        let mut speed_limits = 0;
        let mut substep_limits = 0;
        for _ in 0..120 {
            let start = Instant::now();
            let report = world.step();
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
            substeps += report.substeps;
            max_density = max_density.max(report.max_density_ratio);
            speed_limits += report.speed_limited_particles;
            substep_limits += usize::from(report.substep_limit_reached);
            for (position, velocity) in world.positions().iter().zip(world.velocities()) {
                if !position.is_finite()
                    || !velocity.is_finite()
                    || position.cmplt(world.config().bounds_min).any()
                    || position.cmpgt(world.config().bounds_max).any()
                {
                    return Err(FluidError::InvalidState(
                        "profile encountered an invalid particle".into(),
                    ));
                }
                max_speed = max_speed.max(velocity.length());
            }
        }
        timings.sort_by(f64::total_cmp);
        let mean_ms = timings.iter().sum::<f64>() / timings.len() as f64;
        println!(
            "{name},{},{},{substeps},{mean_ms:.3},{:.3},{:.3},{max_density:.5},{max_speed:.5},{speed_limits},{substep_limits}",
            world.positions().len(),
            world.tick(),
            timings[113],
            timings[119],
        );
        for cell_size in [0.045, 0.025] {
            let start = Instant::now();
            let mesh = extract_surface(&world, cell_size, 0.5)?;
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            let bytes = mesh.positions.len() * 24 + mesh.indices.len() * 4;
            // The preceding solver's triangle soup emitted three vertex+normal
            // pairs for each triangle. Compare representation of this same mesh.
            let unshared_bytes = mesh.indices.len() * 28;
            println!(
                "surface,{name},cell={cell_size:.3},ms={elapsed_ms:.3},vertices={},triangles={},bytes={bytes},unshared_equivalent_bytes={unshared_bytes},saved_percent={:.2}",
                mesh.positions.len(),
                mesh.indices.len() / 3,
                100.0 * (1.0 - bytes as f64 / unshared_bytes.max(1) as f64),
            );
        }
    }
    Ok(())
}
