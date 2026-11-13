//! Bounded, cancellable fluid preview work. The UI owns only completed snapshots.
use super::*;
use poseforge_core::FluidSettings;
use poseforge_simulation::{FluidConfig, StepReport};
use std::sync::mpsc::SyncSender;

// Covers 2× simulation playback at 30 Hz while keeping work off the UI thread.
// Cancellation is checked between each completed fixed tick.
const MAX_PREVIEW_TICKS: usize = 8;

struct Request {
    generation: u64,
    world: FluidWorld,
    elapsed: f64,
    cancel: Arc<AtomicBool>,
    bake: Option<[f64; 3]>,
}
enum WorkerMessage {
    Advanced(Box<Completed>),
    Progress(u64, usize, usize),
    Baked(u64, Result<super::fluid_cache::FluidCache, String>),
}
struct Completed {
    generation: u64,
    world: FluidWorld,
    report: StepReport,
}

#[derive(Default)]
pub(super) struct FluidPreview {
    sender: Option<SyncSender<Request>>,
    waiting: Option<Request>,
    receiver: Option<Receiver<WorkerMessage>>,
    pending: Option<Arc<AtomicBool>>,
    generation: u64,
    target: f64,
    pub report: StepReport,
    pub cache: super::fluid_cache::FluidCache,
    pub baking: Option<(usize, usize)>,
}
impl FluidPreview {
    fn start_worker(&mut self) -> Result<(), String> {
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (completed, receiver) = mpsc::channel();
        std::thread::Builder::new().name("fluid-preview".into()).spawn(move || {
            for mut request in requests {
                if request.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                if let Some([duration, fps, speed]) = request.bake {
                    let result = super::fluid_cache::bake(
                        &request.world,
                        duration,
                        fps,
                        speed,
                        &request.cancel,
                        |done, total| {
                            let _ = completed.send(WorkerMessage::Progress(
                                request.generation,
                                done,
                                total,
                            ));
                        },
                    );
                    let _ = completed.send(WorkerMessage::Baked(request.generation, result));
                    continue;
                }
                // Small work batches keep pause/reset responsive even with a
                // high quality solver. Fixed-step backlog stays inside the world.
                let report = request.world.advance_cancellable(
                    request.elapsed,
                    MAX_PREVIEW_TICKS,
                    &request.cancel,
                );
                if request.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                if completed
                    .send(WorkerMessage::Advanced(Box::new(Completed {
                        generation: request.generation,
                        world: request.world,
                        report,
                    })))
                    .is_err()
                {
                    break;
                }
            }
        }).map_err(|e| format!("Не удалось запустить расчёт жидкости: {e}"))?;
        self.sender = Some(sender);
        self.receiver = Some(receiver);
        Ok(())
    }
    fn submit(&mut self, request: Request) -> Result<(), String> {
        self.pending = Some(request.cancel.clone());
        self.waiting = Some(request);
        self.flush_waiting()
    }
    fn flush_waiting(&mut self) -> Result<(), String> {
        let Some(request) = self.waiting.take() else { return Ok(()); };
        match self.sender.as_ref().unwrap().try_send(request) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(request)) => {
                // Keep one latest request locally while canceled work retires.
                self.waiting = Some(request); Ok(())
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.cancel(); self.sender = None; self.receiver = None;
                Err("Расчёт жидкости остановлен; повторите запуск".into())
            }
        }
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn cancel(&mut self) {
        if let Some(cancel) = self.pending.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.waiting = None;
        self.generation = self.generation.wrapping_add(1);
        self.baking = None;
    }

    pub fn advance(&mut self, world: &FluidWorld, elapsed: f64) -> Result<(), String> {
        if !elapsed.is_finite() || elapsed < 0. {
            return Ok(());
        }
        if self.pending.is_some() {
            return Ok(());
        }
        if self.sender.is_none() {
            self.start_worker()?;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let request = Request {
            generation: self.generation,
            world: world.clone(),
            elapsed,
            cancel: cancel.clone(),
            bake: None,
        };
        self.submit(request)
    }

    pub fn start_bake(
        &mut self,
        world: &FluidWorld,
        duration: f64,
        fps: f64,
        speed: f64,
    ) -> Result<(), String> {
        self.cancel();
        if self.sender.is_none() {
            self.start_worker()?;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.submit(Request {
                generation: self.generation,
                world: world.clone(),
                elapsed: 0.,
                cancel: cancel.clone(),
                bake: Some([duration, fps, speed]),
            })?;
        self.baking = Some((0, (duration * fps).ceil() as usize + 1));
        Ok(())
    }

    pub fn poll(&mut self) -> Result<Option<FluidWorld>, String> {
        self.flush_waiting()?;
        let Some(receiver) = &self.receiver else {
            return Ok(None);
        };
        let mut latest = None;
        loop {
            match receiver.try_recv() {
                Ok(WorkerMessage::Advanced(result)) if result.generation == self.generation => {
                    self.pending = None;
                    self.report = result.report;
                    self.cache.record(&result.world);
                    latest = Some(result.world);
                }
                Ok(WorkerMessage::Progress(generation, done, total))
                    if generation == self.generation =>
                {
                    self.baking = Some((done, total));
                }
                Ok(WorkerMessage::Baked(generation, result)) if generation == self.generation => {
                    self.pending = None;
                    self.baking = None;
                    self.cache = result?;
                }
                Ok(_) => {} // A reset or cancelled edit invalidated this snapshot.
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.cancel();
                    self.sender = None;
                    self.receiver = None;
                    return Err(
                        "Расчёт жидкости остановлен. Повторите запуск предпросмотра.".into(),
                    );
                }
            }
        }
        Ok(latest)
    }

    /// Follow absolute timeline time. Rewinds invalidate in-flight snapshots;
    /// forward seeks catch up in small batches without dropping timeline time.
    pub fn seek(&mut self, world: &mut FluidWorld, target: f64) -> Result<(), String> {
        if !target.is_finite() || target < 0. {
            return Ok(());
        }
        self.cache.reset_for(world);
        self.cache.record(world);
        if self.baking.is_some() {
            let _ = self.poll()?;
            if self.baking.is_some() {
                return Ok(());
            }
        }
        let dt = world.config().time_step as f64;
        let target_tick = ((target + 1e-10) / dt).floor() as u64;
        if target + 1e-10 < self.target || target_tick < world.tick() {
            self.cancel();
            if !self.cache.restore_nearest(world, target_tick) {
                world.reset();
            }
            self.report = StepReport::default();
        } else if self
            .cache
            .nearest_tick(target_tick)
            .is_some_and(|tick| tick > world.tick())
        {
            self.cancel();
            self.cache.restore_nearest(world, target_tick);
        }
        self.target = target;
        if let Some(completed) = self.poll()? {
            *world = completed;
        }
        let ticks = target_tick
            .saturating_sub(world.tick())
            .min(MAX_PREVIEW_TICKS as u64);
        self.report.backlog_seconds = (target - world.time()).max(0.);
        if ticks > 0 && !self.is_pending() {
            let elapsed = (ticks as f64 * dt - world.last_report().backlog_seconds).max(0.);
            self.advance(world, elapsed)?;
        }
        Ok(())
    }
}
impl Drop for FluidPreview {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub(super) fn simulation_config(settings: FluidSettings) -> FluidConfig {
    let spacing = super::physics_preview::source_spacing(settings);
    let ratio = spacing / 0.08;
    let center = Vec3::from_array(settings.source_center);
    let size = Vec3::from_array(settings.source_size);
    FluidConfig {
        smoothing_radius: 0.14 * ratio,
        particle_radius: 0.035 * ratio,
        particle_mass: 0.512 * ratio.powi(3),
        bounds_min: Vec3::new(-1., 0., -0.6).min(center - size * 0.5 - Vec3::splat(spacing)),
        bounds_max: Vec3::new(1., 1.8, 0.6).max(center + size * 0.5 + Vec3::splat(0.5)),
        gravity: Vec3::new(0., -settings.gravity, 0.),
        viscosity: settings.viscosity,
        vorticity: settings.vorticity,
        boundary_friction: settings.boundary_friction,
        iterations: settings.iterations,
        max_substeps: settings.max_substeps,
        ..Default::default()
    }
}

impl App {
    pub(super) fn fluid_edit_active(&self) -> bool {
        self.input.as_ref().is_some_and(|input| {
            matches!(
                input.purpose,
                Action::FluidNumber(_) | Action::Studio(crate::ui::StudioAction::Field(20..=25))
            )
        }) || matches!(&self.drag, Some(Drag::Slider(slider)) if matches!(slider.action, Action::FluidNumber(_)|Action::Studio(crate::ui::StudioAction::Field(20..=25))))
    }

    /// Document settings are authoritative. A numerical draft/drag does not
    /// restart physics until it commits; Escape preserves the old preview.
    pub(super) fn sync_fluid_settings(&mut self) {
        if self.fluid_edit_active() || self.fluid_applied == self.editor.project.fluid_settings {
            return;
        }
        let settings = self.editor.project.fluid_settings;
        if settings.source_kind != self.fluid_applied.source_kind
            || settings.source_center != self.fluid_applied.source_center
            || settings.source_size != self.fluid_applied.source_size
        {
            self.rebuild_fluid_source();
            return;
        }
        let config = simulation_config(settings);
        if config != *self.fluid.config() {
            self.fluid_preview.cancel();
            match self.fluid.reconfigure(config) {
                Ok(()) => self.fluid_preview.report = StepReport::default(),
                Err(error) => {
                    self.fluid_on = false;
                    self.status = error.to_string();
                    return;
                }
            }
        }
        self.fluid_applied = settings;
        self.fluid_preview.cache.reset_for(&self.fluid);
    }

    pub(super) fn reset_fluid_preview(&mut self) {
        self.fluid_preview.cancel();
        self.sync_fluid_settings();
        self.fluid.reset();
        self.fluid_preview.report = StepReport::default();
    }

    pub(super) fn tick_fluid_preview(&mut self) {
        self.sync_fluid_settings();
        if self.fluid_edit_active() {
            if self.fluid_preview.is_pending() {
                self.fluid_preview.cancel();
            }
            return;
        }
        if self.fluid_on {
            if let Err(error) = self.fluid_preview.seek(
                &mut self.fluid,
                self.editor.time * self.editor.project.fluid_settings.time_scale as f64,
            ) {
                self.fluid_on = false;
                self.status = error;
            }
        }
    }
}

impl super::App {
    pub(super) fn fluid_cache_file(&mut self, path: std::path::PathBuf, save: bool) {
        if self.job.is_some() {
            return;
        }
        self.editor.playing = false;
        self.fluid_preview.cancel();
        self.sync_fluid_settings();
        self.fluid_preview.cache.reset_for(&self.fluid);
        let cache = self.fluid_preview.cache.clone();
        let world = self.fluid.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = cancel.clone();
        std::thread::spawn(move || {
            let result = if save {
                cache.save(&path, &c).map(|()| None)
            } else {
                super::fluid_cache::FluidCache::load(&path, &world, &c).map(Some)
            };
            let msg = match result {
                Ok(cache) => super::JobMessage::FluidCacheReady(cache, world.signature()),
                Err(e) => super::JobMessage::Error(e),
            };
            let _ = tx.send(msg);
        });
        self.job = Some(super::Job {
            receiver: rx,
            cancel,
            label: "Кэш жидкости".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_returns_a_completed_world_without_mutating_the_ui_snapshot() {
        let world = FluidWorld::demo();
        let original = world.positions().to_vec();
        let mut preview = FluidPreview::default();
        preview
            .advance(&world, 2. * world.config().time_step as f64)
            .unwrap();
        assert!(preview.is_pending());
        assert_eq!(world.time(), 0.);
        assert_eq!(world.positions(), original);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let completed = loop {
            if let Some(completed) = preview.poll().unwrap() {
                break completed;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "fluid worker did not finish"
            );
            std::thread::sleep(Duration::from_millis(2));
        };
        assert!(!preview.is_pending());
        assert_eq!(preview.report.steps, 2);
        assert!(completed.time() > 0.);
        assert_eq!(world.positions(), original);
        assert_ne!(completed.positions(), original);
    }

    #[test]
    fn worker_can_keep_up_with_double_speed_at_thirty_frames_per_second() {
        let world =
            FluidWorld::new(FluidConfig::default(), vec![Vec3::new(0.0, 0.8, 0.0)]).unwrap();
        let mut preview = FluidPreview::default();
        // Eight exact ticks model 2x playback at a 30 Hz display. The former
        // two-tick cap silently limited playback and left a growing backlog.
        preview
            .advance(&world, 8.0 * world.config().time_step as f64)
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let completed = loop {
            if let Some(world) = preview.poll().unwrap() {
                break world;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(completed.tick(), 8);
        assert_eq!(preview.report.steps, 8);
        assert!(preview.report.backlog_seconds < 1e-8);
    }

    #[test]
    fn cancelled_worker_results_cannot_replace_a_reset_snapshot() {
        let mut preview = FluidPreview::default();
        let (sender, receiver) = mpsc::channel();
        preview.receiver = Some(receiver);
        preview.pending = Some(Arc::new(AtomicBool::new(false)));
        let generation = preview.generation;
        preview.cancel();
        sender
            .send(WorkerMessage::Advanced(Box::new(Completed {
                generation,
                world: FluidWorld::demo(),
                report: StepReport::default(),
            })))
            .unwrap();
        assert!(preview.poll().unwrap().is_none());
        assert!(!preview.is_pending());
    }

    #[test]
    fn cancelling_a_settings_drag_preserves_completed_fluid_state() {
        let mut app = App::new(None);
        app.preferences = Preferences::default();
        app.preferences.right_panel = true;
        app.surface.tab = 4;
        app.fluid.step();
        let before = app.fluid.positions().to_vec();
        let time = app.fluid.time();
        // Legacy global settings still share the solver, but are no longer a property tab.
        let mut controls = Ui::new(1.);
        app.draw_fluid_settings(&mut controls, 0., 300.);
        app.ui = controls;
        let (rect, range, value) = app
            .ui
            .hits
            .iter()
            .find_map(|hit| match &hit.action {
                Action::Slide(field, range, value) if **field == Action::FluidNumber(0) => {
                    Some((hit.rect, *range, *value))
                }
                _ => None,
            })
            .unwrap();
        app.mouse = [rect[0] + range.ratio(value) * rect[2], rect[1] + 6.];
        app.mouse_down(MouseButton::Left);
        app.mouse_move([rect[0] + rect[2] * 0.8, rect[1] + 6.]);
        app.tick_fluid_preview();
        assert_eq!(app.fluid.positions(), before);
        app.action(Action::Cancel);
        assert_eq!(app.fluid.positions(), before);
        assert_eq!(app.fluid.time(), time);
        assert_eq!(app.editor.project.fluid_settings, FluidSettings::default());
    }

    fn settle(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            app.tick_fluid_preview();
            if !app.fluid_preview.is_pending() {
                break;
            }
            assert!(Instant::now() < deadline, "timeline fluid did not settle");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn timeline_play_pause_speed_seek_loop_and_reset_control_fluid() {
        let mut app = App::new(None);
        app.preferences = Preferences::default();
        app.editor = Editor::new(Project {
            duration: 6.,
            ..Default::default()
        });
        app.fluid = FluidWorld::new(
            simulation_config(app.editor.project.fluid_settings),
            vec![Vec3::new(0., 0.8, 0.)],
        )
        .unwrap();
        app.action(Action::Fluid);
        assert_eq!(app.editor.mode, EditorMode::Animation);
        assert!(!app.editor.playing);
        settle(&mut app);
        assert_eq!(app.fluid.tick(), 0);
        app.toggle_animation_playback();
        app.animation.speed = 2.;
        app.animation.loop_enabled = false;
        app.tick_animation(0.5);
        settle(&mut app);
        let dt = app.fluid.config().time_step as f64;
        assert!((app.fluid.time() - app.editor.time).abs() < dt);
        assert!(app.fluid.time() > 0.9);
        app.toggle_animation_playback();
        let paused = app.fluid.snapshot();
        app.tick_animation(1.);
        settle(&mut app);
        assert_eq!(app.fluid.tick(), paused.tick);
        assert_eq!(app.fluid.positions(), paused.positions);
        app.set_time(0.25);
        settle(&mut app);
        let sought = app.fluid.snapshot();
        let mut reference = app.fluid.clone();
        reference.reset();
        for _ in 0..sought.tick {
            reference.step();
        }
        assert_eq!(app.fluid.positions(), reference.positions());
        app.set_time(4.); // More than the old two-second real-time backlog limit.
        app.tick_fluid_preview();
        assert!(app.fluid_preview.is_pending());
        app.set_time(0.); // Cancel that in-flight seek before its result can replace frame zero.
        settle(&mut app);
        assert_eq!(app.fluid.tick(), 0);
        app.set_time(4.);
        settle(&mut app);
        assert!((app.fluid.time() - 4.).abs() < dt);
        app.editor.project.fluid_settings.time_scale = 0.5;
        settle(&mut app);
        assert!((app.fluid.time() - 2.).abs() < dt);
        app.animation.loop_enabled = true;
        app.toggle_animation_playback();
        app.tick_animation(2.); // Wrap the six-second timeline back to its beginning.
        settle(&mut app);
        assert!((app.fluid.time() - app.editor.time * 0.5).abs() < dt);
        app.action(Action::ResetFluid);
        settle(&mut app);
        assert!(!app.editor.playing);
        assert_eq!(app.editor.time, 0.);
        assert_eq!(app.fluid.tick(), 0);
        assert!(app.fluid_on);
    }

    #[test]
    fn repeated_scrubbing_keeps_only_one_queued_and_one_latest_request() {
        let world = FluidWorld::demo();
        let (sender, requests) = mpsc::sync_channel(1);
        let mut preview = FluidPreview::default(); preview.sender = Some(sender);
        preview.advance(&world, 0.01).unwrap();
        let old = preview.pending.clone().unwrap();
        for _ in 0..32 {
            preview.cancel(); preview.advance(&world, 0.02).unwrap();
            assert!(preview.waiting.is_some());
        }
        assert!(old.load(Ordering::Relaxed));
        assert!(requests.try_recv().unwrap().cancel.load(Ordering::Relaxed));
        assert!(requests.try_recv().is_err());
        preview.flush_waiting().unwrap();
        let latest = requests.try_recv().unwrap();
        assert_eq!(latest.generation, preview.generation);
        assert!(!latest.cancel.load(Ordering::Relaxed));
        assert!(preview.waiting.is_none());
    }
    #[test]
    fn disconnected_fluid_queue_clears_bake_and_allows_a_fresh_worker() {
        let (sender, requests) = mpsc::sync_channel(1); drop(requests);
        let mut preview = FluidPreview::default(); preview.sender = Some(sender);
        assert!(preview.advance(&FluidWorld::demo(), 0.01).is_err());
        assert!(!preview.is_pending() && preview.sender.is_none() && preview.waiting.is_none());
    }

}
