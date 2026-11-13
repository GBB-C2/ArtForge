//! Deterministic CPU reference implementation of Position Based Fluids.
//!
//! This is an experimental preview solver, not the GPU/final-surface implementation
//! required by the full application specification. It solves compression constraints
//! with Poly6 density, Spiky gradients and XSPH viscosity. A uniform spatial hash
//! restricts neighbor lookup to 27 cells; hash-table iteration order is never used.
//! See Macklin & Müller, https://mmacklin.com/pbf_sig_preprint.pdf.

use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::{AtomicBool, Ordering},
};
use thiserror::Error;

pub mod deform;
pub mod fields;
pub mod fx_flow;
pub mod fx_collision;
pub mod mesh_contact;
pub mod mesh_obstacles;
pub mod shard_contact;
pub mod surface;
pub use surface::{extract_surface, extract_surface_cancellable, extract_surface_render_cancellable, SurfaceMesh};

pub const SOLVER_VERSION: &str = "artforge-cpu-pbf-7";
pub const MAX_ADVANCE_STEPS: usize = 64;
pub const MAX_REALTIME_BACKLOG: f64 = 2.0;
pub const MAX_PARTICLES: usize = 16_384;

#[derive(Debug, Error)]
pub enum FluidError {
    #[error("Расчёт жидкости отменён")]
    Cancelled,
    #[error("Некорректные параметры жидкости: {0}")]
    InvalidConfig(String),
    #[error("Некорректное состояние жидкости: {0}")]
    InvalidState(String),
    #[error("Кэш жидкости устарел: параметры или исходные данные изменились")]
    StaleCache,
    #[error("Шаг {0} отсутствует в кэше; требуется последовательный расчёт")]
    NotCached(u64),
    #[error("Ошибка кэша жидкости: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ошибка формата кэша: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FluidConfig {
    pub time_step: f32,
    pub smoothing_radius: f32,
    pub particle_radius: f32,
    pub rest_density: f32,
    pub particle_mass: f32,
    pub gravity: Vec3,
    pub iterations: u32,
    /// XSPH coefficient, 0..=0.2.
    pub viscosity: f32,
    /// Strength of velocity-postprocess vorticity confinement (m/s).
    pub vorticity: f32,
    /// Tangential contact damping per reference 1/120 s interval.
    pub boundary_friction: f32,
    /// Deterministic adaptive subdivisions inside one fixed physical tick.
    pub max_substeps: u32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    /// Numerical preview safety limit; not a physically calibrated parameter.
    pub max_speed: f32,
}

impl Default for FluidConfig {
    fn default() -> Self {
        Self {
            time_step: 1.0 / 120.0,
            smoothing_radius: 0.14,
            particle_radius: 0.035,
            rest_density: 1000.0,
            particle_mass: 0.512,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            iterations: 4,
            viscosity: 0.04,
            vorticity: 0.1,
            boundary_friction: 0.05,
            max_substeps: 4,
            bounds_min: Vec3::new(-1.0, 0.0, -0.6),
            bounds_max: Vec3::new(1.0, 1.8, 0.6),
            max_speed: 8.0,
        }
    }
}

impl FluidConfig {
    pub fn validate(&self) -> Result<(), FluidError> {
        let scalars = [
            self.time_step,
            self.smoothing_radius,
            self.particle_radius,
            self.rest_density,
            self.particle_mass,
            self.viscosity,
            self.vorticity,
            self.boundary_friction,
            self.max_speed,
        ];
        if scalars.iter().any(|x| !x.is_finite())
            || !self.gravity.is_finite()
            || !self.bounds_min.is_finite()
            || !self.bounds_max.is_finite()
        {
            return Err(FluidError::InvalidConfig("NaN или бесконечность".into()));
        }
        if !(1.0 / 1000.0..=1.0 / 30.0).contains(&self.time_step)
            || !(0.005..=2.0).contains(&self.smoothing_radius)
            || self.particle_radius <= 0.0
            || self.particle_radius >= self.smoothing_radius * 0.5
            || !(1.0..=100_000.0).contains(&self.rest_density)
            || !(0.000001..=1000.0).contains(&self.particle_mass)
            || !(1..=16).contains(&self.iterations)
            || !(0.0..=0.2).contains(&self.viscosity)
            || !(0.0..=5.0).contains(&self.vorticity)
            || !(0.0..=1.0).contains(&self.boundary_friction)
            || !(1..=8).contains(&self.max_substeps)
            || !(0.1..=100.0).contains(&self.max_speed)
            || !self.gravity.length().is_finite()
            || self.gravity.length() > 1000.0
            || (self.bounds_max - self.bounds_min).min_element() <= self.particle_radius * 4.0
        {
            return Err(FluidError::InvalidConfig(
                "параметр вне допустимого диапазона".into(),
            ));
        }
        // Coordinates must be small enough for well-defined integer cell addresses.
        if self.bounds_min.abs().max_element() > 10_000.0
            || self.bounds_max.abs().max_element() > 10_000.0
        {
            return Err(FluidError::InvalidConfig(
                "область расчёта слишком далеко от начала координат".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ColliderMotion {
    Static,
    Linear {
        velocity: Vec3,
    },
    /// The center is origin + amplitude * sin(2π * frequency_hz * t + phase).
    Oscillate {
        amplitude: Vec3,
        frequency_hz: f32,
        phase: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SphereCollider {
    pub center: Vec3,
    pub radius: f32,
    pub motion: ColliderMotion,
}

impl SphereCollider {
    pub fn center_at(&self, time: f64) -> Vec3 {
        match self.motion {
            ColliderMotion::Static => self.center,
            ColliderMotion::Linear { velocity } => self.center + velocity * time as f32,
            ColliderMotion::Oscillate {
                amplitude,
                frequency_hz,
                phase,
            } => {
                self.center
                    + amplitude
                        * (std::f64::consts::TAU * frequency_hz as f64 * time + phase as f64).sin()
                            as f32
            }
        }
    }

    pub fn velocity_at(&self, time: f64) -> Vec3 {
        match self.motion {
            ColliderMotion::Static => Vec3::ZERO,
            ColliderMotion::Linear { velocity } => velocity,
            ColliderMotion::Oscillate {
                amplitude,
                frequency_hz,
                phase,
            } => {
                amplitude
                    * (std::f32::consts::TAU * frequency_hz)
                    * (std::f64::consts::TAU * frequency_hz as f64 * time + phase as f64).cos()
                        as f32
            }
        }
    }

    fn validate(&self) -> Result<(), FluidError> {
        if !self.center.is_finite()
            || !self.radius.is_finite()
            || self.radius <= 0.0
            || self.radius > 1000.0
            || !self.center_at(1.0).is_finite()
            || !self.velocity_at(1.0).is_finite()
        {
            return Err(FluidError::InvalidConfig(
                "некорректный сферический коллайдер".into(),
            ));
        }
        if let ColliderMotion::Oscillate {
            frequency_hz,
            phase,
            ..
        } = self.motion
        {
            if !frequency_hz.is_finite()
                || !phase.is_finite()
                || !(0.0..=10.0).contains(&frequency_hz)
            {
                return Err(FluidError::InvalidConfig(
                    "некорректная частота коллайдера".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StepReport {
    pub steps: usize,
    /// Number of completed internal PBF solves across those fixed ticks.
    pub substeps: usize,
    /// True when requested displacement-based subdivisions exceed the budget.
    pub substep_limit_reached: bool,
    /// A cancellable advance stopped between fully completed fixed ticks.
    pub cancelled: bool,
    /// Unprocessed real-time preview work, bounded by MAX_REALTIME_BACKLOG.
    pub backlog_seconds: f64,
    /// Wall-clock time discarded by the real-time backlog limit; offline step/bake
    /// never discards simulation time.
    pub discarded_seconds: f64,
    pub max_vorticity: f32,
    pub speed_limited_particles: usize,
    pub max_density_ratio: f32,
    pub neighbor_candidates: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FluidSnapshot {
    pub solver_version: String,
    pub signature: u64,
    pub tick: u64,
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
}

#[derive(Clone)]
pub struct FluidWorld {
    fields: fields::Fields,
    config: FluidConfig,
    initial_positions: Vec<Vec3>,
    positions: Vec<Vec3>,
    velocities: Vec<Vec3>,
    sphere: Option<SphereCollider>,
    colliders: Vec<deform::Collider>,
    container: Option<std::sync::Arc<mesh_contact::MeshVolume>>,
    mesh_obstacles: Option<std::sync::Arc<mesh_obstacles::MeshObstacles>>,
    tick: u64,
    accumulator: f64,
    signature: u64,
    report: StepReport,
    scratch: StepScratch,
}

impl FluidWorld {
    pub fn new(config: FluidConfig, initial_positions: Vec<Vec3>) -> Result<Self, FluidError> {
        config.validate()?;
        if initial_positions.is_empty()
            || initial_positions.len() > MAX_PARTICLES
            || initial_positions.iter().any(|p| !p.is_finite())
        {
            return Err(FluidError::InvalidState(format!(
                "требуется от 1 до {MAX_PARTICLES} конечных позиций"
            )));
        }
        let min = config.bounds_min + Vec3::splat(config.particle_radius);
        let max = config.bounds_max - Vec3::splat(config.particle_radius);
        if initial_positions
            .iter()
            .any(|p| p.cmplt(min).any() || p.cmpgt(max).any())
        {
            return Err(FluidError::InvalidState(
                "исходная частица вне области расчёта".into(),
            ));
        }
        let mut world = Self {
            fields: Default::default(),
            positions: initial_positions.clone(),
            velocities: vec![Vec3::ZERO; initial_positions.len()],
            initial_positions,
            config,
            sphere: None,
            colliders: vec![],
            container: None,
            mesh_obstacles: None,
            tick: 0,
            accumulator: 0.0,
            signature: 0,
            report: StepReport::default(),
            scratch: StepScratch::default(),
        };
        world.refresh_signature();
        Ok(world)
    }

    /// A falling rectangular water volume, 576 particles, with a moving obstacle.
    pub fn demo() -> Self {
        Self::demo_with_config(FluidConfig::default()).expect("constant demo configuration")
    }

    /// Rebuild the deterministic demonstration volume using validated solver inputs.
    pub fn demo_with_config(config: FluidConfig) -> Result<Self, FluidError> {
        let mut particles = Vec::new();
        for y in 0..8 {
            for z in 0..6 {
                for x in 0..12 {
                    particles.push(Vec3::new(
                        -0.8 + x as f32 * 0.08,
                        0.45 + y as f32 * 0.08,
                        -0.2 + z as f32 * 0.08,
                    ));
                }
            }
        }
        let mut world = Self::new(config, particles)?;
        world.set_sphere(Some(SphereCollider {
            center: Vec3::new(0.2, 0.25, 0.0),
            radius: 0.17,
            motion: ColliderMotion::Oscillate {
                amplitude: Vec3::new(0.3, 0.0, 0.0),
                frequency_hz: 0.35,
                phase: 0.0,
            },
        }))?;
        Ok(world)
    }

    pub fn container(&self) -> Option<&mesh_contact::MeshVolume> { self.container.as_deref() }
    pub fn set_container(&mut self, volume: mesh_contact::MeshVolume) -> Result<(), FluidError> {
        if self.initial_positions.iter().any(|p| !volume.accepts_particle(*p, self.config.particle_radius * 0.99)) {
            return Err(FluidError::InvalidState("Частица вне формы контейнера".into()));
        }
        self.container = Some(std::sync::Arc::new(volume));
        self.refresh_signature();
        self.reset();
        Ok(())
    }
    pub fn colliders(&self) -> &[deform::Collider] {
        &self.colliders
    }
    pub fn set_mesh_obstacles(&mut self, obstacles: Option<std::sync::Arc<mesh_obstacles::MeshObstacles>>) {
        self.mesh_obstacles = obstacles;
        self.refresh_signature();
    }
    pub fn set_colliders(&mut self, colliders: Vec<deform::Collider>) -> Result<(), FluidError> {
        if colliders.len() > 64 || colliders.iter().any(|c| !c.valid()) {
            return Err(FluidError::InvalidConfig("коллайдеры".into()));
        }
        self.colliders = colliders;
        self.refresh_signature();
        Ok(())
    }
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }
    pub fn velocities(&self) -> &[Vec3] {
        &self.velocities
    }
    pub fn config(&self) -> &FluidConfig {
        &self.config
    }
    pub fn sphere(&self) -> Option<&SphereCollider> {
        self.sphere.as_ref()
    }
    pub fn tick(&self) -> u64 {
        self.tick
    }
    pub fn time(&self) -> f64 {
        self.tick as f64 * self.config.time_step as f64
    }
    pub fn signature(&self) -> u64 {
        self.signature
    }
    pub fn last_report(&self) -> StepReport {
        self.report
    }

    /// Changing solver inputs restarts the simulation and invalidates old cache signatures.
    pub fn set_config(&mut self, config: FluidConfig) -> Result<(), FluidError> {
        config.validate()?;
        let min = config.bounds_min + Vec3::splat(config.particle_radius);
        let max = config.bounds_max - Vec3::splat(config.particle_radius);
        if self
            .initial_positions
            .iter()
            .any(|p| p.cmplt(min).any() || p.cmpgt(max).any()
                || self.container.as_ref().is_some_and(|shape| !shape.accepts_particle(*p, config.particle_radius * 0.99)))
        {
            return Err(FluidError::InvalidConfig(
                "новая область исключает исходные частицы".into(),
            ));
        }
        if self.config == config {
            return Ok(());
        }
        self.config = config;
        self.refresh_signature();
        self.reset();
        Ok(())
    }

    /// Atomically validate new inputs, then reset to the original particle state.
    /// Equal inputs are a no-op. This never silently retargets a physics cache.
    pub fn reconfigure(&mut self, config: FluidConfig) -> Result<(), FluidError> {
        self.set_config(config)
    }

    pub fn set_sphere(&mut self, sphere: Option<SphereCollider>) -> Result<(), FluidError> {
        if let Some(sphere) = &sphere {
            sphere.validate()?;
        }
        self.sphere = sphere;
        self.refresh_signature();
        self.reset();
        Ok(())
    }

    pub fn reset(&mut self) {
        self.positions.clone_from(&self.initial_positions);
        self.velocities.fill(Vec3::ZERO);
        self.tick = 0;
        self.accumulator = 0.0;
        self.report = StepReport::default();
    }

    pub fn set_fields(&mut self,fields:fields::Fields) {if self.fields!=fields {self.fields=fields;self.reset();self.refresh_signature();}}

    fn refresh_signature(&mut self) {
        // Deterministic JSON tuples include all physics inputs and initial state.
        let bytes = serde_json::to_vec(&(
            SOLVER_VERSION,
            &self.config,
            &self.fields,
            &self.initial_positions,
            &self.sphere,
            &self.colliders,
            self.container.as_ref().map(|c| c.signature()),
            self.mesh_obstacles.as_ref().map(|c| c.signature()),
        ))
        .expect("validated finite solver inputs serialize");
        self.signature = bytes.iter().fold(0xcbf29ce484222325u64, |hash, &byte| {
            (hash ^ byte as u64).wrapping_mul(0x100000001b3)
        });
    }

    /// Advance one fixed physical tick. The adaptive microstep count depends
    /// solely on physics state, so frame cadence and cache replay remain identical.
    pub fn step(&mut self) -> StepReport {
        let cfg = &self.config;
        let dt = cfg.time_step;
        let particle_speed = self
            .velocities
            .iter()
            .map(|v| v.length())
            .fold(0.0_f32, f32::max)
            + (cfg.gravity.length()+self.fields.acceleration_limit(cfg.max_speed)) * dt;
        let collider_speed = self
            .sphere
            .as_ref()
            .map_or(0.0, |sphere| match sphere.motion {
                ColliderMotion::Static => 0.0,
                ColliderMotion::Linear { velocity } => velocity.length(),
                ColliderMotion::Oscillate {
                    amplitude,
                    frequency_hz,
                    ..
                } => amplitude.length() * std::f32::consts::TAU * frequency_hz,
            });
        let travel_limit = cfg.particle_radius.max(cfg.smoothing_radius * 0.1);
        let requested =
            (((particle_speed + collider_speed) * dt / travel_limit).ceil() as u32).max(1);
        let subdivisions = requested.min(cfg.max_substeps);
        let mut report = StepReport {
            steps: 1,
            substeps: subdivisions as usize,
            substep_limit_reached: requested > cfg.max_substeps,
            ..StepReport::default()
        };
        let start_time = self.time();
        let sub_dt = dt / subdivisions as f32;
        // Scratch vectors and the packed spatial index are retained across ticks.
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.resize(self.positions.len());
        for substep in 0..subdivisions {
            let time = start_time + substep as f64 * dt as f64 / subdivisions as f64;
            self.substep(sub_dt, time, &mut scratch, &mut report);
        }
        self.scratch = scratch;
        self.tick += 1;
        self.report = report;
        report
    }

    fn substep(
        &mut self,
        dt: f32,
        start_time: f64,
        work: &mut StepScratch,
        report: &mut StepReport,
    ) {
        let cfg = &self.config;
        let n = self.positions.len();
        let next_time = start_time + dt as f64;
        let kernel = Kernel::new(cfg.smoothing_radius);
        let min = cfg.bounds_min + Vec3::splat(cfg.particle_radius);
        let max = cfg.bounds_max - Vec3::splat(cfg.particle_radius);
        for i in 0..n {
            let acceleration=cfg.gravity+self.fields.acceleration(self.positions[i],self.velocities[i]);
            self.velocities[i] += acceleration * dt;
            if self.velocities[i].length_squared() > cfg.max_speed * cfg.max_speed {
                self.velocities[i] = self.velocities[i].normalize_or_zero() * cfg.max_speed;
                report.speed_limited_particles += 1;
            }
            let predicted = self.positions[i] + self.velocities[i] * dt;
            work.predicted[i] = swept_collision(
                self.positions[i],
                predicted,
                min,
                max,
                cfg.particle_radius,
                self.sphere.as_ref(),
                start_time,
                next_time,
            );
            for c in &self.colliders {
                work.predicted[i] =
                    c.project(self.positions[i], work.predicted[i], cfg.particle_radius);
            }
            work.container_normals[i] = Vec3::ZERO;
            work.mesh_normals[i] = Vec3::ZERO;
            if let Some(obstacles) = &self.mesh_obstacles {
                let before = work.predicted[i];
                work.predicted[i] = obstacles.project(self.positions[i], before, cfg.particle_radius, &mut work.contact_candidates);
                work.mesh_normals[i] = (work.predicted[i] - before).normalize_or_zero();
            }
            if let Some(container) = &self.container {
                let before = work.predicted[i];
                work.predicted[i] = container.project(self.positions[i], before, cfg.particle_radius, &mut work.contact_candidates);
                work.container_normals[i] = (work.predicted[i] - before).normalize_or_zero();
            }
        }
        let mass_over_density = cfg.particle_mass / cfg.rest_density;
        for _ in 0..cfg.iterations {
            // Rebuilding after correction prevents particles crossing cells from
            // escaping their constraints. Packed neighbors avoid per-particle heaps.
            report.neighbor_candidates += work
                .neighbors
                .rebuild(&work.predicted, cfg.smoothing_radius);
            for i in 0..n {
                let mut density = cfg.particle_mass * kernel.poly6(0.0);
                let mut gradient_sum = Vec3::ZERO;
                let mut sum_gradient_squared = 0.0;
                for &j in work.neighbors.get(i) {
                    let difference = work.predicted[i] - work.predicted[j];
                    density += cfg.particle_mass * kernel.poly6(difference.length_squared());
                    let gradient =
                        constraint_gradient(difference, i, j, &kernel) * mass_over_density;
                    sum_gradient_squared += gradient.length_squared();
                    gradient_sum += gradient;
                }
                sum_gradient_squared += gradient_sum.length_squared();
                let ratio = density / cfg.rest_density;
                report.max_density_ratio = report.max_density_ratio.max(ratio);
                // Compression-only constraints retain the stable free-surface model.
                work.lambdas[i] = -(ratio - 1.0).max(0.0) / (sum_gradient_squared + 0.001);
            }
            for i in 0..n {
                let mut correction = Vec3::ZERO;
                for &j in work.neighbors.get(i) {
                    correction +=
                        constraint_gradient(work.predicted[i] - work.predicted[j], i, j, &kernel)
                            * ((work.lambdas[i] + work.lambdas[j]) * mass_over_density);
                }
                work.deltas[i] = correction.clamp_length_max(cfg.smoothing_radius * 0.25);
            }
            for i in 0..n {
                work.predicted[i] = project_collision(
                    work.predicted[i] + work.deltas[i],
                    min,
                    max,
                    cfg.particle_radius,
                    self.sphere.as_ref(),
                    next_time,
                );
                for c in &self.colliders {
                    work.predicted[i] =
                        c.project(self.positions[i], work.predicted[i], cfg.particle_radius);
                }
                if let Some(obstacles) = &self.mesh_obstacles {
                    let before = work.predicted[i];
                    work.predicted[i] = obstacles.project(self.positions[i], before, cfg.particle_radius, &mut work.contact_candidates);
                    let normal = (work.predicted[i] - before).normalize_or_zero();
                    if normal != Vec3::ZERO { work.mesh_normals[i] = normal; }
                }
                if let Some(container) = &self.container {
                    let before = work.predicted[i];
                    work.predicted[i] = container.project(self.positions[i], before, cfg.particle_radius, &mut work.contact_candidates);
                    let normal = (work.predicted[i] - before).normalize_or_zero();
                    if normal != Vec3::ZERO { work.container_normals[i] = normal; }
                }
            }
        }
        for i in 0..n {
            work.new_velocities[i] = (work.predicted[i] - self.positions[i]) / dt;
        }
        if cfg.vorticity > 0.0 || cfg.viscosity > 0.0 {
            report.neighbor_candidates += work
                .neighbors
                .rebuild(&work.predicted, cfg.smoothing_radius);
        }
        if cfg.vorticity > 0.0 {
            let maximum = confine_vorticity(
                &work.predicted,
                &mut work.new_velocities,
                &work.neighbors,
                &kernel,
                mass_over_density,
                cfg.vorticity,
                dt,
                cfg.max_speed,
                &mut work.vorticities,
            );
            report.max_vorticity = report.max_vorticity.max(maximum);
        }
        if cfg.viscosity > 0.0 {
            // XSPH is specified per reference 1/120 s, avoiding stronger damping
            // simply because collision safety requested additional microsteps.
            let viscosity = 1.0 - (1.0 - cfg.viscosity).powf(dt * 120.0);
            diffuse_velocity(
                &work.predicted,
                &mut work.new_velocities,
                &work.neighbors,
                &kernel,
                mass_over_density,
                viscosity,
                &mut work.prior_velocities,
                &mut work.weight_sums,
            );
        }
        let friction = 1.0 - (1.0 - cfg.boundary_friction).powf(dt * 120.0);
        for i in 0..n {
            let mut velocity = work.new_velocities[i];
            if work.mesh_normals[i] != Vec3::ZERO {
                velocity = contact_velocity(velocity, work.mesh_normals[i], Vec3::ZERO, friction);
            }
            if work.container_normals[i] != Vec3::ZERO {
                velocity = contact_velocity(velocity, work.container_normals[i], Vec3::ZERO, friction);
            }
            for axis in 0..3 {
                let mut normal = Vec3::ZERO;
                if work.predicted[i][axis] <= min[axis] + 1e-5 {
                    normal[axis] = 1.0;
                }
                if work.predicted[i][axis] >= max[axis] - 1e-5 {
                    normal[axis] = -1.0;
                }
                if normal != Vec3::ZERO {
                    velocity = contact_velocity(velocity, normal, Vec3::ZERO, friction);
                }
            }
            if let Some(sphere) = &self.sphere {
                let offset = work.predicted[i] - sphere.center_at(next_time);
                if offset.length() <= sphere.radius + cfg.particle_radius + 1e-4 {
                    velocity = contact_velocity(
                        velocity,
                        offset.normalize_or_zero(),
                        sphere.velocity_at(next_time),
                        friction,
                    );
                }
            }
            if velocity.length_squared() > cfg.max_speed * cfg.max_speed {
                velocity = velocity.normalize_or_zero() * cfg.max_speed;
                report.speed_limited_particles += 1;
            }
            self.velocities[i] = velocity;
        }
        self.positions.copy_from_slice(&work.predicted);
    }

    /// Bounded real-time catch-up. At most 64 fixed ticks are solved per call,
    /// and no more than two seconds are queued. Discarded wall time is reported;
    /// deterministic offline stepping and cache baking do not use this policy.
    pub fn advance(&mut self, real_seconds: f64, max_steps: usize) -> StepReport {
        self.advance_cancellable(real_seconds, max_steps, &AtomicBool::new(false))
    }

    /// Cooperative worker cancellation never exposes a half-solved physical tick.
    pub fn advance_cancellable(
        &mut self,
        real_seconds: f64,
        max_steps: usize,
        cancelled: &AtomicBool,
    ) -> StepReport {
        let mut result = StepReport::default();
        if cancelled.load(Ordering::Relaxed) {
            result.cancelled = true;
            result.backlog_seconds = self.accumulator;
            self.report = result;
            return result;
        }
        if real_seconds.is_finite() && real_seconds > 0.0 {
            let room = (MAX_REALTIME_BACKLOG - self.accumulator).max(0.0);
            let accepted = real_seconds.min(room);
            self.accumulator += accepted;
            result.discarded_seconds = real_seconds - accepted;
        }
        let dt = self.config.time_step as f64;
        let budget = max_steps.min(MAX_ADVANCE_STEPS);
        while self.accumulator + 1e-12 >= dt
            && result.steps < budget
            && !cancelled.load(Ordering::Relaxed)
        {
            let step = self.step();
            result.steps += 1;
            result.substeps += step.substeps;
            result.substep_limit_reached |= step.substep_limit_reached;
            result.neighbor_candidates += step.neighbor_candidates;
            result.speed_limited_particles += step.speed_limited_particles;
            result.max_density_ratio = result.max_density_ratio.max(step.max_density_ratio);
            result.max_vorticity = result.max_vorticity.max(step.max_vorticity);
            self.accumulator -= dt;
        }
        self.accumulator = self.accumulator.max(0.0);
        result.backlog_seconds = self.accumulator;
        result.cancelled = cancelled.load(Ordering::Relaxed);
        self.report = result;
        result
    }

    pub fn snapshot(&self) -> FluidSnapshot {
        FluidSnapshot {
            solver_version: SOLVER_VERSION.into(),
            signature: self.signature,
            tick: self.tick,
            positions: self.positions.clone(),
            velocities: self.velocities.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &FluidSnapshot) -> Result<(), FluidError> {
        if snapshot.solver_version != SOLVER_VERSION || snapshot.signature != self.signature {
            return Err(FluidError::StaleCache);
        }
        let min = self.config.bounds_min + Vec3::splat(self.config.particle_radius - 1e-4);
        let max = self.config.bounds_max - Vec3::splat(self.config.particle_radius - 1e-4);
        if snapshot.positions.len() != self.positions.len()
            || snapshot.velocities.len() != self.velocities.len()
            || snapshot
                .positions
                .iter()
                .any(|v| !v.is_finite() || v.cmplt(min).any() || v.cmpgt(max).any()
                    || self.container.as_ref().is_some_and(|shape| !shape.contains(*v)))
            || snapshot
                .velocities
                .iter()
                .any(|v| !v.is_finite() || v.length() > self.config.max_speed + 1e-3)
            || snapshot.tick > 1_000_000_000
        {
            return Err(FluidError::InvalidState("повреждённый блок кэша".into()));
        }
        // Validate completely before replacing the current state.
        self.positions.clone_from(&snapshot.positions);
        self.velocities.clone_from(&snapshot.velocities);
        self.tick = snapshot.tick;
        self.accumulator = 0.0;
        self.report = StepReport::default();
        Ok(())
    }
}

/// Continuous contact for linearly moving centers during one microstep. Curved
/// collider motion is linearized per microstep; adaptive subdivisions reduce error.
#[allow(clippy::too_many_arguments)]
fn swept_collision(
    start: Vec3,
    end: Vec3,
    min: Vec3,
    max: Vec3,
    particle_radius: f32,
    sphere: Option<&SphereCollider>,
    start_time: f64,
    end_time: f64,
) -> Vec3 {
    let mut point = end;
    if let Some(sphere) = sphere {
        let center_start = sphere.center_at(start_time);
        let center_end = sphere.center_at(end_time);
        let relative_start = start - center_start;
        let relative_motion = end - start - (center_end - center_start);
        let radius = sphere.radius + particle_radius;
        let a = relative_motion.length_squared();
        let b = relative_start.dot(relative_motion);
        let c = relative_start.length_squared() - radius * radius;
        let discriminant = b * b - a * c;
        if c <= 0.0 && a > 1e-12 {
            // A contact can begin exactly on the surface (or slightly inside it
            // after roundoff). Retain the original side when a fast obstacle
            // advances through that existing contact during the whole interval.
            let normal = if relative_start.length_squared() > 1e-12 {
                relative_start.normalize()
            } else {
                -relative_motion.normalize_or_zero()
            };
            if relative_motion.dot(normal) < 0.0 {
                let sliding = relative_motion - normal * relative_motion.dot(normal);
                point = center_end + normal * radius + sliding;
            }
        } else if a > 1e-12 && b < 0.0 && discriminant >= 0.0 {
            let t = (-b - discriminant.sqrt()) / a;
            if (0.0..=1.0).contains(&t) {
                let normal = (relative_start + relative_motion * t).normalize_or_zero();
                let remaining = relative_motion * (1.0 - t);
                let sliding = remaining - normal * remaining.dot(normal).min(0.0);
                point = center_end + normal * radius + sliding;
            }
        }
    }
    project_collision(point, min, max, particle_radius, sphere, end_time)
}

fn contact_velocity(velocity: Vec3, normal: Vec3, surface_velocity: Vec3, friction: f32) -> Vec3 {
    let relative = velocity - surface_velocity;
    let approach = relative.dot(normal);
    if approach > 0.0 {
        return velocity;
    }
    surface_velocity + (relative - normal * approach) * (1.0 - friction)
}

/// Equal-and-opposite pair exchanges preserve the bulk momentum of the liquid.
/// A symmetric row-sum bound keeps every updated velocity a convex combination
/// of the old velocities, including dense and irregular neighborhoods. Unlike
/// one-sided XSPH normalization, this does not accelerate the center of mass.
#[allow(clippy::too_many_arguments)]
fn diffuse_velocity(
    positions: &[Vec3],
    velocities: &mut [Vec3],
    neighbors: &NeighborSearch,
    kernel: &Kernel,
    particle_volume: f32,
    amount: f32,
    previous: &mut [Vec3],
    weight_sums: &mut [f32],
) {
    previous.copy_from_slice(velocities);
    for i in 0..positions.len() {
        weight_sums[i] = neighbors
            .get(i)
            .iter()
            .map(|&j| {
                particle_volume * kernel.poly6((positions[i] - positions[j]).length_squared())
            })
            .sum::<f32>()
            .max(1.0);
    }
    for i in 0..positions.len() {
        for &j in neighbors.get(i) {
            if j <= i {
                continue;
            }
            let weight = particle_volume
                * kernel.poly6((positions[i] - positions[j]).length_squared())
                / weight_sums[i].max(weight_sums[j]);
            let exchange = (previous[j] - previous[i]) * (amount * weight);
            velocities[i] += exchange;
            velocities[j] -= exchange;
        }
    }
}

/// Coincident particles otherwise have a zero Spiky gradient and can remain
/// permanently compressed. Assign an antisymmetric, deterministic direction
/// only within numerical coincidence; ordinary particles keep the exact kernel.
fn constraint_gradient(difference: Vec3, i: usize, j: usize, kernel: &Kernel) -> Vec3 {
    if difference.length_squared() >= 1e-12 {
        return kernel.spiky_gradient(difference);
    }
    let low = i.min(j) as u64;
    let high = i.max(j) as u64;
    let hash = low.wrapping_mul(0x9e3779b97f4a7c15) ^ high.wrapping_mul(0xbf58476d1ce4e5b9);
    // Several non-axis-aligned directions prevent an overlapping cluster from
    // receiving a single preferred direction; the pair order only flips sign.
    let direction = Vec3::new(
        (hash & 1023) as f32 - 511.5,
        ((hash >> 10) & 1023) as f32 - 511.5,
        ((hash >> 20) & 1023) as f32 - 511.5,
    )
    .normalize();
    let sign = if i < j { 1.0 } else { -1.0 };
    kernel.spiky_gradient(direction * (kernel.h * 0.001 * sign))
}

/// PBF section5, equations15–16, with particle-volume weighting and a bounded
/// acceleration. Uniform flow and zero curl receive no artificial rotational force.
#[allow(clippy::too_many_arguments)]
fn confine_vorticity(
    positions: &[Vec3],
    velocities: &mut [Vec3],
    neighbors: &NeighborSearch,
    kernel: &Kernel,
    particle_volume: f32,
    strength: f32,
    dt: f32,
    max_speed: f32,
    omega: &mut [Vec3],
) -> f32 {
    let mut maximum = 0.0_f32;
    for i in 0..positions.len() {
        let mut curl = Vec3::ZERO;
        for &j in neighbors.get(i) {
            let gradient = kernel.spiky_gradient(positions[i] - positions[j]) * particle_volume;
            curl += gradient.cross(velocities[j] - velocities[i]);
        }
        omega[i] = curl;
        maximum = maximum.max(curl.length());
    }
    for i in 0..positions.len() {
        let mut gradient = Vec3::ZERO;
        for &j in neighbors.get(i) {
            gradient += kernel.spiky_gradient(positions[i] - positions[j])
                * ((omega[j].length() - omega[i].length()) * particle_volume);
        }
        let force = gradient.normalize_or_zero().cross(omega[i]) * strength;
        velocities[i] += (force * dt).clamp_length_max(max_speed * 0.25);
    }
    maximum
}

fn project_collision(
    mut point: Vec3,
    min: Vec3,
    max: Vec3,
    particle_radius: f32,
    sphere: Option<&SphereCollider>,
    time: f64,
) -> Vec3 {
    point = point.clamp(min, max);
    if let Some(sphere) = sphere {
        let center = sphere.center_at(time);
        let delta = point - center;
        let radius = sphere.radius + particle_radius;
        if delta.length_squared() < radius * radius {
            let normal = if delta.length_squared() > 1e-12 {
                delta.normalize()
            } else {
                Vec3::Y
            };
            point = (center + normal * radius).clamp(min, max);
        }
    }
    point
}

struct Kernel {
    h: f32,
    h_squared: f32,
    poly6_factor: f32,
    spiky_factor: f32,
}
impl Kernel {
    fn new(h: f32) -> Self {
        Self {
            h,
            h_squared: h * h,
            poly6_factor: 315.0 / (64.0 * std::f32::consts::PI * h.powi(9)),
            spiky_factor: -45.0 / (std::f32::consts::PI * h.powi(6)),
        }
    }
    fn poly6(&self, distance_squared: f32) -> f32 {
        if distance_squared >= self.h_squared {
            0.0
        } else {
            self.poly6_factor * (self.h_squared - distance_squared).powi(3)
        }
    }
    fn spiky_gradient(&self, difference: Vec3) -> Vec3 {
        let length = difference.length();
        if length < 1e-6 || length >= self.h {
            Vec3::ZERO
        } else {
            difference * (self.spiky_factor * (self.h - length).powi(2) / length)
        }
    }
}

type Cell = (i32, i32, i32);
fn cell(point: Vec3, h: f32) -> Cell {
    let grid = (point / h).floor().as_ivec3();
    (grid.x, grid.y, grid.z)
}
#[derive(Clone, Default)]
struct NeighborSearch {
    order: Vec<(Cell, usize)>,
    cells: HashMap<Cell, std::ops::Range<usize>>,
    offsets: Vec<usize>,
    indices: Vec<usize>,
}
impl NeighborSearch {
    fn rebuild(&mut self, positions: &[Vec3], h: f32) -> usize {
        self.order.clear();
        self.order
            .extend(positions.iter().enumerate().map(|(i, &p)| (cell(p, h), i)));
        self.order.sort_unstable();
        self.cells.clear();
        let mut start = 0;
        while start < self.order.len() {
            let key = self.order[start].0;
            let mut end = start + 1;
            while end < self.order.len() && self.order[end].0 == key {
                end += 1;
            }
            self.cells.insert(key, start..end);
            start = end;
        }
        self.offsets.clear();
        self.indices.clear();
        self.offsets.push(0);
        let mut candidates = 0;
        for (i, &position) in positions.iter().enumerate() {
            let (x, y, z) = cell(position, h);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(range) = self.cells.get(&(x + dx, y + dy, z + dz)) {
                            for k in range.clone() {
                                candidates += 1;
                                let j = self.order[k].1;
                                if i != j && (positions[j] - position).length_squared() < h * h {
                                    self.indices.push(j);
                                }
                            }
                        }
                    }
                }
            }
            self.offsets.push(self.indices.len());
        }
        candidates
    }
    fn get(&self, particle: usize) -> &[usize] {
        &self.indices[self.offsets[particle]..self.offsets[particle + 1]]
    }
}

#[derive(Clone, Default)]
struct StepScratch {
    predicted: Vec<Vec3>,
    lambdas: Vec<f32>,
    deltas: Vec<Vec3>,
    new_velocities: Vec<Vec3>,
    prior_velocities: Vec<Vec3>,
    weight_sums: Vec<f32>,
    vorticities: Vec<Vec3>,
    neighbors: NeighborSearch,
    container_normals: Vec<Vec3>,
    mesh_normals: Vec<Vec3>,
    contact_candidates: Vec<usize>,
}
impl StepScratch {
    fn resize(&mut self, count: usize) {
        self.predicted.resize(count, Vec3::ZERO);
        self.container_normals.resize(count, Vec3::ZERO);
        self.mesh_normals.resize(count, Vec3::ZERO);
        self.lambdas.resize(count, 0.0);
        self.deltas.resize(count, Vec3::ZERO);
        self.new_velocities.resize(count, Vec3::ZERO);
        self.prior_velocities.resize(count, Vec3::ZERO);
        self.weight_sums.resize(count, 0.0);
        self.vorticities.resize(count, Vec3::ZERO);
    }
}

#[cfg(test)]
fn build_neighbors(positions: &[Vec3], h: f32) -> (Vec<Vec<usize>>, usize) {
    let mut search = NeighborSearch::default();
    let candidates = search.rebuild(positions, h);
    (
        (0..positions.len())
            .map(|i| search.get(i).to_vec())
            .collect(),
        candidates,
    )
}

/// A cache stores complete restart states; rendering may consume positions alone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheTimeline {
    pub solver_version: String,
    pub signature: u64,
    snapshots: BTreeMap<u64, FluidSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BakeReport {
    pub completed_steps: u64,
    pub cancelled: bool,
    pub stored_states: usize,
}

impl CacheTimeline {
    pub fn new(world: &FluidWorld) -> Self {
        let mut cache = Self {
            solver_version: SOLVER_VERSION.into(),
            signature: world.signature(),
            snapshots: BTreeMap::new(),
        };
        cache.snapshots.insert(world.tick(), world.snapshot());
        cache
    }
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
    pub fn ticks(&self) -> impl Iterator<Item = u64> + '_ {
        self.snapshots.keys().copied()
    }
    pub fn get(&self, tick: u64) -> Option<&FluidSnapshot> {
        self.snapshots.get(&tick)
    }
    pub fn is_valid_for(&self, world: &FluidWorld) -> bool {
        self.signature == world.signature() && self.solver_version == SOLVER_VERSION
    }
    pub fn clear(&mut self, world: &FluidWorld) {
        *self = Self::new(world);
    }
    pub fn record(&mut self, world: &FluidWorld) -> Result<(), FluidError> {
        if !self.is_valid_for(world) {
            return Err(FluidError::StaleCache);
        }
        self.snapshots.insert(world.tick(), world.snapshot());
        Ok(())
    }
    pub fn seek(&self, world: &mut FluidWorld, tick: u64) -> Result<(), FluidError> {
        if !self.is_valid_for(world) {
            return Err(FluidError::StaleCache);
        }
        world.restore(
            self.snapshots
                .get(&tick)
                .ok_or(FluidError::NotCached(tick))?,
        )
    }
    pub fn bake(
        &mut self,
        world: &mut FluidWorld,
        steps: u64,
        stride: u64,
        cancellation: &AtomicBool,
    ) -> Result<BakeReport, FluidError> {
        if !self.is_valid_for(world) {
            return Err(FluidError::StaleCache);
        }
        if stride == 0 {
            return Err(FluidError::InvalidConfig("нулевой шаг кэша".into()));
        }
        let mut completed = 0;
        while completed < steps && !cancellation.load(Ordering::Relaxed) {
            world.step();
            completed += 1;
            if world.tick().is_multiple_of(stride) {
                self.record(world)?;
            }
        }
        // Cancellation only occurs between full physics steps. The latest complete
        // restart state is preserved even when it falls between regular blocks.
        self.record(world)?;
        Ok(BakeReport {
            completed_steps: completed,
            cancelled: completed < steps,
            stored_states: self.len(),
        })
    }
    /// Rewind to the nearest preceding checkpoint, then simulate forwards only.
    pub fn seek_or_simulate(
        &mut self,
        world: &mut FluidWorld,
        tick: u64,
        cancellation: &AtomicBool,
    ) -> Result<bool, FluidError> {
        if !self.is_valid_for(world) {
            return Err(FluidError::StaleCache);
        }
        let (_, checkpoint) = self
            .snapshots
            .range(..=tick)
            .next_back()
            .ok_or(FluidError::NotCached(tick))?;
        world.restore(checkpoint)?;
        while world.tick() < tick {
            if cancellation.load(Ordering::Relaxed) {
                self.record(world)?;
                return Ok(false);
            }
            world.step();
        }
        self.record(world)?;
        Ok(true)
    }

    /// Write a new immutable cache file. Existing files are deliberately not overwritten.
    /// A failed or cancelled write cannot damage a previously saved cache file.
    pub fn save_new(&self, path: impl AsRef<std::path::Path>) -> Result<(), FluidError> {
        use std::io::Write;
        let data = serde_json::to_vec(self)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&data)?;
        file.sync_all()?;
        Ok(())
    }
    pub fn load(path: impl AsRef<std::path::Path>, world: &FluidWorld) -> Result<Self, FluidError> {
        let path = path.as_ref();
        if std::fs::metadata(path)?.len() > 256 * 1024 * 1024 {
            return Err(FluidError::InvalidState(
                "кэш больше предела 256 МиБ".into(),
            ));
        }
        let cache: Self = serde_json::from_reader(std::fs::File::open(path)?)?;
        if !cache.is_valid_for(world) {
            return Err(FluidError::StaleCache);
        }
        let mut validation_world = world.clone();
        for (&tick, snapshot) in &cache.snapshots {
            if tick != snapshot.tick {
                return Err(FluidError::InvalidState(
                    "индекс кэша не совпадает с блоком".into(),
                ));
            }
            validation_world.restore(snapshot)?;
        }
        Ok(cache)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact_world() -> FluidWorld {
        let mut positions = Vec::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    positions.push(Vec3::new(
                        -0.3 + x as f32 * 0.08,
                        0.3 + y as f32 * 0.08,
                        -0.12 + z as f32 * 0.08,
                    ));
                }
            }
        }
        FluidWorld::new(FluidConfig::default(), positions).unwrap()
    }

    #[test]
    fn realtime_work_and_backlog_are_bounded_and_cancellation_is_atomic() {
        let mut world = compact_world();
        let before = world.snapshot();
        let stopped = world.advance_cancellable(100.0, usize::MAX, &AtomicBool::new(true));
        assert!(stopped.cancelled);
        assert_eq!(stopped.steps, 0);
        assert_eq!(world.tick(), before.tick);
        assert_eq!(world.positions(), before.positions);
        let report = world.advance(f64::MAX, usize::MAX);
        assert_eq!(report.steps, MAX_ADVANCE_STEPS);
        assert!(report.discarded_seconds.is_finite() && report.discarded_seconds > 1.0);
        assert!((0.0..=MAX_REALTIME_BACKLOG).contains(&report.backlog_seconds));
        assert!(report.substeps <= MAX_ADVANCE_STEPS * world.config().max_substeps as usize);
        assert!(world.positions().iter().all(|p| p.is_finite()));
    }

    #[test]
    fn adaptive_microsteps_preserve_fixed_time_and_catch_fast_motion() {
        let config = FluidConfig {
            gravity: Vec3::ZERO,
            viscosity: 0.0,
            vorticity: 0.0,
            time_step: 1.0 / 30.0,
            max_speed: 100.0,
            max_substeps: 8,
            bounds_min: Vec3::splat(-5.0),
            bounds_max: Vec3::splat(5.0),
            ..Default::default()
        };
        let mut world = FluidWorld::new(config, vec![Vec3::ZERO]).unwrap();
        world.velocities[0] = Vec3::new(6.0, 0.0, 0.0);
        let report = world.step();
        assert!(report.substeps > 1 && report.substeps <= 8);
        assert_eq!(world.tick(), 1);
        assert_eq!(world.time(), world.config().time_step as f64);
        assert!((world.positions()[0].x - 0.2).abs() < 1e-6);
        world.velocities[0] = Vec3::new(100.0, 0.0, 0.0);
        assert!(world.step().substep_limit_reached);
    }

    #[test]
    fn moving_sphere_cannot_pass_through_a_stationary_particle_between_samples() {
        let sphere = SphereCollider {
            center: Vec3::new(-0.5, 0.5, 0.0),
            radius: 0.08,
            motion: ColliderMotion::Linear {
                velocity: Vec3::new(30.0, 0.0, 0.0),
            },
        };
        let start = Vec3::new(0.0, 0.5, 0.0);
        let end_time = 1.0 / 30.0;
        let result = swept_collision(
            start,
            start,
            Vec3::splat(-2.0),
            Vec3::splat(2.0),
            0.035,
            Some(&sphere),
            0.0,
            end_time,
        );
        assert!(result.x >= sphere.center_at(end_time).x + sphere.radius + 0.035 - 1e-5);
        let config = FluidConfig {
            gravity: Vec3::ZERO,
            time_step: end_time as f32,
            viscosity: 0.0,
            vorticity: 0.0,
            max_substeps: 1,
            max_speed: 100.0,
            bounds_min: Vec3::splat(-2.0),
            bounds_max: Vec3::splat(2.0),
            ..Default::default()
        };
        let mut world = FluidWorld::new(config, vec![start]).unwrap();
        world.set_sphere(Some(sphere.clone())).unwrap();
        world.step();
        assert!(world.positions()[0].x > sphere.center_at(world.time()).x);
        let touching = sphere.center + Vec3::X * (sphere.radius + 0.035);
        let continuing_contact = swept_collision(
            touching,
            touching,
            Vec3::splat(-2.0),
            Vec3::splat(2.0),
            0.035,
            Some(&sphere),
            0.0,
            end_time,
        );
        assert!(
            continuing_contact.x >= sphere.center_at(end_time).x + sphere.radius + 0.035 - 1e-5
        );
    }

    #[test]
    fn wall_friction_damps_tangential_motion_without_creating_normal_velocity() {
        let base = FluidConfig {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            viscosity: 0.0,
            vorticity: 0.0,
            boundary_friction: 0.0,
            ..Default::default()
        };
        let initial = Vec3::new(0.0, base.bounds_min.y + base.particle_radius, 0.0);
        let mut smooth = FluidWorld::new(base.clone(), vec![initial]).unwrap();
        let mut sticky = FluidWorld::new(
            FluidConfig {
                boundary_friction: 0.5,
                ..base
            },
            vec![initial],
        )
        .unwrap();
        smooth.velocities[0] = Vec3::X;
        sticky.velocities[0] = Vec3::X;
        smooth.step();
        sticky.step();
        assert!((smooth.velocities()[0].x - 1.0).abs() < 1e-5);
        assert!(sticky.velocities()[0].x < smooth.velocities()[0].x * 0.6);
        assert_eq!(sticky.velocities()[0].y, 0.0);
    }

    #[test]
    fn vorticity_confinement_preserves_uniform_flow_and_strengthens_a_vortex() {
        let positions: Vec<_> = (-3..=3)
            .flat_map(|x| {
                (-3..=3).flat_map(move |y| {
                    (-3..=3).map(move |z| Vec3::new(x as f32, y as f32, z as f32) * 0.06)
                })
            })
            .collect();
        let mut neighbors = NeighborSearch::default();
        neighbors.rebuild(&positions, 0.14);
        let kernel = Kernel::new(0.14);
        let mut uniform = vec![Vec3::new(0.7, -0.2, 0.3); positions.len()];
        let original = uniform.clone();
        let mut omega = vec![Vec3::ZERO; positions.len()];
        let maximum = confine_vorticity(
            &positions,
            &mut uniform,
            &neighbors,
            &kernel,
            0.000512,
            0.5,
            1.0 / 120.0,
            8.0,
            &mut omega,
        );
        assert_eq!(maximum, 0.0);
        assert_eq!(uniform, original);
        let mut vortex: Vec<_> = positions
            .iter()
            .map(|p| Vec3::new(-p.z, 0.0, p.x) * (-(p.x * p.x + p.z * p.z) * 50.0).exp() * 10.0)
            .collect();
        let before: f32 = vortex.iter().map(|v| v.length_squared()).sum();
        let maximum = confine_vorticity(
            &positions,
            &mut vortex,
            &neighbors,
            &kernel,
            0.000512,
            0.5,
            1.0 / 120.0,
            8.0,
            &mut omega,
        );
        let after: f32 = vortex.iter().map(|v| v.length_squared()).sum();
        assert!(maximum > 0.0);
        assert!(
            after > before,
            "confinement should restore swirling energy: {before} -> {after}"
        );
        assert!(vortex.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn symmetric_viscosity_preserves_momentum_and_dissipates_energy() {
        // An uneven cluster exercises the different normalization totals that
        // previously made viscosity move the entire liquid without a force.
        let positions = vec![
            Vec3::ZERO,
            Vec3::new(0.025, 0.0, 0.0),
            Vec3::new(0.04, 0.03, 0.0),
            Vec3::new(0.11, -0.02, 0.01),
            Vec3::new(0.21, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
        ];
        let initial = vec![
            Vec3::new(2.0, 0.3, -0.1),
            Vec3::new(-0.2, -1.0, 0.5),
            Vec3::new(0.7, 1.1, -0.3),
            Vec3::new(-1.0, 0.4, 0.8),
            Vec3::new(0.1, -0.5, -0.9),
            Vec3::new(3.0, 2.0, 1.0),
        ];
        let mut neighbors = NeighborSearch::default();
        neighbors.rebuild(&positions, 0.14);
        let kernel = Kernel::new(0.14);
        let momentum: Vec3 = initial.iter().copied().sum();
        let energy: f32 = initial.iter().map(|v| v.length_squared()).sum();
        for particle_volume in [0.000512, 0.00512] {
            for amount in [0.04, 0.2, 0.6, 1.0] {
                let mut velocities = initial.clone();
                let mut previous = vec![Vec3::ZERO; positions.len()];
                let mut weights = vec![0.0; positions.len()];
                diffuse_velocity(
                    &positions,
                    &mut velocities,
                    &neighbors,
                    &kernel,
                    particle_volume,
                    amount,
                    &mut previous,
                    &mut weights,
                );
                let after_momentum: Vec3 = velocities.iter().copied().sum();
                let after_energy: f32 = velocities.iter().map(|v| v.length_squared()).sum();
                assert!((after_momentum - momentum).length() < 2e-6);
                assert!(after_energy < energy);
                assert_eq!(
                    velocities[5], initial[5],
                    "isolated particles must not damp"
                );
            }
        }
        let mut uniform = vec![Vec3::new(0.4, -0.8, 1.2); positions.len()];
        let expected = uniform.clone();
        diffuse_velocity(
            &positions,
            &mut uniform,
            &neighbors,
            &kernel,
            0.00512,
            0.6,
            &mut vec![Vec3::ZERO; positions.len()],
            &mut vec![0.0; positions.len()],
        );
        assert_eq!(uniform, expected, "uniform flow must be unchanged");
    }

    #[test]
    fn coincident_particles_separate_deterministically_without_bulk_drift() {
        let config = FluidConfig {
            particle_mass: 1.3,
            gravity: Vec3::ZERO,
            viscosity: 0.0,
            vorticity: 0.0,
            ..Default::default()
        };
        let center = Vec3::new(0.0, 0.8, 0.0);
        let mut world = FluidWorld::new(config, vec![center; 2]).unwrap();
        let mut repeated = world.clone();
        let report = world.step();
        repeated.step();
        assert!(report.max_density_ratio > 1.0);
        assert!((world.positions()[0] - world.positions()[1]).length() > 0.001);
        assert!(((world.positions()[0] + world.positions()[1]) * 0.5 - center).length() < 1e-6);
        assert_eq!(world.positions(), repeated.positions());
        assert_eq!(world.velocities(), repeated.velocities());
        let kernel = Kernel::new(0.14);
        assert_eq!(
            constraint_gradient(Vec3::ZERO, 0, 1, &kernel),
            -constraint_gradient(Vec3::ZERO, 1, 0, &kernel)
        );
        let normal_difference = Vec3::new(0.03, 0.01, 0.0);
        assert_eq!(
            constraint_gradient(normal_difference, 0, 1, &kernel),
            kernel.spiky_gradient(normal_difference)
        );
    }

    #[test]
    fn old_config_defaults_new_controls_and_reconfigure_is_atomic() {
        let mut value = serde_json::to_value(FluidConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        for key in ["vorticity", "boundary_friction", "max_substeps"] {
            object.remove(key);
        }
        assert_eq!(
            serde_json::from_value::<FluidConfig>(value).unwrap(),
            FluidConfig::default()
        );
        let mut world = FluidWorld::demo();
        world.step();
        let before = world.snapshot();
        world.reconfigure(world.config().clone()).unwrap();
        assert_eq!(world.tick(), before.tick);
        assert_eq!(world.positions(), before.positions);
        let mut invalid = world.config().clone();
        invalid.vorticity = f32::NAN;
        assert!(world.reconfigure(invalid).is_err());
        assert_eq!(world.positions(), before.positions);
        let mut changed = world.config().clone();
        changed.vorticity = 0.25;
        world.reconfigure(changed).unwrap();
        assert_eq!(world.tick(), 0);
        assert_ne!(world.signature(), before.signature);
        assert!(matches!(
            world.restore(&before),
            Err(FluidError::StaleCache)
        ));
    }

    #[test]
    fn ten_seconds_stays_finite_and_inside_container() {
        let mut world = compact_world();
        world
            .set_sphere(Some(SphereCollider {
                center: Vec3::new(0.0, 0.15, 0.0),
                radius: 0.08,
                motion: ColliderMotion::Oscillate {
                    amplitude: Vec3::new(0.25, 0.0, 0.0),
                    frequency_hz: 0.7,
                    phase: 0.0,
                },
            }))
            .unwrap();
        let count = world.positions().len();
        for _ in 0..1200 {
            world.step();
            let min =
                world.config().bounds_min + Vec3::splat(world.config().particle_radius - 1e-5);
            let max =
                world.config().bounds_max - Vec3::splat(world.config().particle_radius - 1e-5);
            for position in world.positions() {
                assert!(position.is_finite());
                assert!(position.cmpge(min).all() && position.cmple(max).all());
            }
            assert!(world.velocities().iter().all(|v| v.is_finite()));
            assert_eq!(world.positions().len(), count);
        }
        assert!((world.time() - 10.0).abs() < 1e-5);
    }

    #[test]
    fn fixed_steps_are_independent_of_render_cadence() {
        let mut one = compact_world();
        let mut several = one.clone();
        let dt = one.config().time_step as f64;
        one.advance(dt * 120.0, 200);
        while one.tick() < 120 {
            one.advance(0.0, 200);
        }
        for _ in 0..30 {
            several.advance(dt * 4.0, 200);
        }
        assert_eq!(one.tick(), several.tick());
        assert_eq!(one.positions(), several.positions());
        assert_eq!(one.velocities(), several.velocities());
        let mut capped = compact_world();
        let result = capped.advance(dt * 120.0, 2);
        assert_eq!(result.steps, 2);
        assert!(result.backlog_seconds > 0.9);
    }

    #[test]
    fn density_solver_expands_compressed_volume() {
        let config = FluidConfig {
            gravity: Vec3::ZERO,
            ..Default::default()
        };
        let mut positions = Vec::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    positions.push(Vec3::new(
                        x as f32 * 0.04,
                        0.5 + y as f32 * 0.04,
                        z as f32 * 0.04,
                    ));
                }
            }
        }
        let mut world = FluidWorld::new(config, positions).unwrap();
        let bounds = |p: &[Vec3]| {
            p.iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(min, max), &p| (min.min(p), max.max(p)),
            )
        };
        let (min_before, max_before) = bounds(world.positions());
        let report = world.step();
        let (min_after, max_after) = bounds(world.positions());
        assert!(
            report.max_density_ratio > 1.5,
            "must exercise the density constraint"
        );
        assert!(
            (max_after - min_after).length() > (max_before - min_before).length() * 1.1,
            "density projection must separate compressed particles"
        );
    }

    #[test]
    fn sparse_hash_does_not_visit_every_pair() {
        let mut positions = Vec::new();
        for x in 0..20 {
            for z in 0..20 {
                positions.push(Vec3::new(x as f32 * 0.3, 0.0, z as f32 * 0.3));
            }
        }
        let (neighbors, candidates) = build_neighbors(&positions, 0.14);
        assert!(neighbors.iter().all(Vec::is_empty));
        assert!(
            candidates < positions.len() * 10,
            "sparse neighbor lookup should scale with local occupancy"
        );
    }

    #[test]
    fn cache_rewind_resume_matches_uninterrupted_simulation() {
        let mut world = compact_world();
        let mut cache = CacheTimeline::new(&world);
        let cancellation = AtomicBool::new(false);
        cache.bake(&mut world, 60, 12, &cancellation).unwrap();
        let final_positions = world.positions().to_vec();
        let final_velocities = world.velocities().to_vec();
        cache.seek(&mut world, 24).unwrap();
        for _ in 24..60 {
            world.step();
        }
        assert_eq!(world.positions(), final_positions);
        assert_eq!(world.velocities(), final_velocities);
        assert!(cache
            .seek_or_simulate(&mut world, 37, &cancellation)
            .unwrap());
        let mut reference = compact_world();
        for _ in 0..37 {
            reference.step();
        }
        assert_eq!(reference.positions(), world.positions());
    }

    #[test]
    fn cancellation_retains_complete_cache_and_input_change_invalidates_it() {
        let mut world = compact_world();
        let mut cache = CacheTimeline::new(&world);
        let cancellation = AtomicBool::new(false);
        cache.bake(&mut world, 10, 4, &cancellation).unwrap();
        let count = cache.len();
        cancellation.store(true, Ordering::Relaxed);
        let report = cache.bake(&mut world, 500, 4, &cancellation).unwrap();
        assert!(report.cancelled);
        assert_eq!(report.completed_steps, 0);
        assert_eq!(cache.len(), count);
        cache.seek(&mut world, 8).unwrap();
        let mut config = world.config().clone();
        config.gravity.y = -3.0;
        world.set_config(config).unwrap();
        assert!(!cache.is_valid_for(&world));
        assert!(matches!(
            cache.seek(&mut world, 8),
            Err(FluidError::StaleCache)
        ));
    }

    #[test]
    fn invalid_snapshot_does_not_mutate_current_state() {
        let mut world = compact_world();
        world.step();
        let before = world.positions().to_vec();
        let mut snapshot = world.snapshot();
        snapshot.positions[0].x = f32::NAN;
        assert!(world.restore(&snapshot).is_err());
        assert_eq!(world.positions(), before);
        let mut config = world.config().clone();
        config.time_step = f32::NAN;
        assert!(world.set_config(config).is_err());
        assert_eq!(world.positions(), before);
    }

    #[test]
    fn collider_moves_at_physics_time() {
        let sphere = SphereCollider {
            center: Vec3::new(0.0, 0.5, 0.0),
            radius: 0.2,
            motion: ColliderMotion::Linear { velocity: Vec3::X },
        };
        assert_eq!(sphere.center_at(0.5), Vec3::new(0.5, 0.5, 0.0));
        let point = project_collision(
            Vec3::new(0.5, 0.5, 0.0),
            Vec3::splat(-2.0),
            Vec3::splat(2.0),
            0.035,
            Some(&sphere),
            0.5,
        );
        assert!((point.distance(sphere.center_at(0.5)) - 0.235).abs() < 1e-5);
    }

    #[test]
    fn disk_cache_restores_full_restart_state_and_never_overwrites() {
        let mut world = compact_world();
        let mut cache = CacheTimeline::new(&world);
        cache
            .bake(&mut world, 8, 4, &AtomicBool::new(false))
            .unwrap();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join(format!(".fluid-test-{}-{unique}.json", std::process::id()));
        cache.save_new(&path).unwrap();
        assert!(
            cache.save_new(&path).is_err(),
            "an existing cache must never be silently replaced"
        );
        let loaded = CacheTimeline::load(&path, &world).unwrap();
        std::fs::remove_file(&path).unwrap();
        world.reset();
        loaded.seek(&mut world, 8).unwrap();
        assert_eq!(world.positions(), cache.get(8).unwrap().positions);
        assert_eq!(world.velocities(), cache.get(8).unwrap().velocities);
    }
}

pub mod gas;
