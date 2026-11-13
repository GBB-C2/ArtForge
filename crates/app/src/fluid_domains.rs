//! Object-owned liquid worlds and continuously shaded, cancellable surface previews.
use super::*;
use poseforge_core::{FluidDomainSettings, Mesh, Vertex};
use poseforge_simulation::{mesh_contact::{MeshVolume, Triangle}, mesh_obstacles::MeshObstacles, FluidConfig};

pub(super) fn mesh_bounds(mesh: &Mesh) -> (Vec3, Vec3) {
    let min = mesh.vertices.iter().map(|v| Vec3::from_array(v.position))
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let max = mesh.vertices.iter().map(|v| Vec3::from_array(v.position))
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    (min, max)
}

#[derive(Clone, PartialEq)]
struct DomainSpec {
    fields: poseforge_simulation::fields::Fields,
    vertices: poseforge_core::SharedBuffer<Vertex>,
    indices: poseforge_core::SharedBuffer<u32>,
    min: Vec3,
    max: Vec3,
    fill: f32,
    config: FluidConfig,
    colliders: Vec<scene_colliders::Input>,
    spacing: f32,
}
impl DomainSpec {
    fn new(project: &Project, id: u64, globals: &BTreeMap<u64, Mat4>, render: bool) -> Result<Self, String> {
        let node = project.node(id).ok_or("Контейнер удалён")?;
        let settings = node.fluid.ok_or("Жидкость не назначена")?;
        settings.validate().map_err(|e| e.to_string())?;
        let mesh = project.mesh(node.mesh.ok_or("Контейнер требует меш")?).ok_or("Меш удалён")?;
        let (min, max) = mesh_bounds(mesh);
        let size = max - min;
        if !min.is_finite() || !max.is_finite() || size.min_element() < 0.08 {
            return Err("Для жидкости нужен объёмный примитив: каждая сторона от 0.08".into());
        }
        let model = *globals.get(&id).ok_or("Нет трансформации контейнера")?;
        if !model.is_finite() || model.determinant().abs() < 1e-8 {
            return Err("Масштаб контейнера не должен быть нулевым".into());
        }
        let inverse = model.inverse();
        let volume = size.x * size.y * size.z * settings.fill;
        let mut spacing = (volume / 2500.).cbrt().max(0.01);
        let counts = |step: f32| {
            let extent = Vec3::new(size.x, size.y * settings.fill, size.z);
            extent.to_array().map(|v| (v / step).ceil().max(1.) as usize + 1)
        };
        while counts(spacing).into_iter().try_fold(1usize, |n, v| n.checked_mul(v))
            .is_none_or(|n| n > 6000)
        {
            spacing *= 1.1;
        }
        if spacing * 1.76 >= size.min_element() {
            return Err("Контейнер слишком тонкий для жидкости; увеличьте меньшую сторону".into());
        }
        let s = settings.simulation;
        let ratio = spacing / 0.08;
        let config = FluidConfig {
            smoothing_radius: 0.14 * ratio,
            particle_radius: 0.035 * ratio,
            particle_mass: 0.512 * ratio.powi(3),
            bounds_min: min,
            bounds_max: max,
            gravity: inverse.transform_vector3(Vec3::new(0., -s.gravity, 0.)),
            viscosity: s.viscosity,
            vorticity: s.vorticity,
            boundary_friction: s.boundary_friction,
            iterations: s.iterations,
            max_substeps: s.max_substeps,
            ..Default::default()
        };
        config.validate().map_err(|e| e.to_string())?;
        let colliders = if settings.mesh_collision {
            scene_colliders::inputs(project, globals, render, Some(id), inverse, None, node.collision_targets.fluid.as_deref())
        } else { Vec::new() };
        Ok(Self { fields:scene_fields::collect(project,globals,render,0,model), vertices: mesh.vertices.clone(), indices: mesh.indices.clone(), min, max, fill: settings.fill, config, colliders, spacing })
    }

    fn world(&self, cancel: &AtomicBool) -> Result<FluidWorld, String> {
        if cancel.load(Ordering::Relaxed) { return Err("Подготовка жидкости отменена".into()); }
        let triangles = self.indices.chunks_exact(3).map(|t| Triangle([Vec3::from_array(self.vertices[t[0] as usize].position), Vec3::from_array(self.vertices[t[1] as usize].position), Vec3::from_array(self.vertices[t[2] as usize].position)])).collect();
        let shape = MeshVolume::new(triangles)?;
        let obstacles = if self.colliders.is_empty() { None } else {
            Some(Arc::new(MeshObstacles::new(scene_colliders::triangles(&self.colliders, cancel)?, cancel)?))
        };
        let mut contacts = Vec::new();
        let radius = self.config.particle_radius;
        let lo = self.min + Vec3::splat(radius * 1.1);
        let mut hi = self.max - Vec3::splat(radius * 1.1);
        hi.y = (self.min.y + (self.max.y - self.min.y) * self.fill - radius)
            .max(lo.y).min(hi.y);
        let extent = (hi - lo).max(Vec3::ZERO);
        let count = extent.to_array().map(|v| (v / self.spacing).floor() as usize + 1);
        let mut points = Vec::new();
        for y in 0..count[1] {
            for z in 0..count[2] {
                for x in 0..count[0] {
                    if cancel.load(Ordering::Relaxed) { return Err("Подготовка жидкости отменена".into()); }
                    let offset = Vec3::new(x as f32, y as f32, z as f32) * self.spacing;
                    let p = lo + offset;
                    if shape.accepts_particle(p, radius * 1.01)
                        && obstacles.as_ref().is_none_or(|c| c.accepts_particle(p, radius * 1.01, &mut contacts)) {
                        points.push(p);
                    }
                }
            }
        }
        if points.is_empty() || points.len() > 6000 {
            return Err("Нет свободного объёма для жидкости; уменьшите коллайдер или заполнение".into());
        }
        let mut world = FluidWorld::new(self.config.clone(), points).map_err(|e| e.to_string())?;
        world.set_container(shape).map_err(|e| e.to_string())?;
        world.set_mesh_obstacles(obstacles);
        world.set_fields(self.fields.clone());
        Ok(world)
    }

    fn matches_source(&self, mesh: &Mesh) -> bool {
        self.vertices == mesh.vertices && self.indices == mesh.indices
    }

}

pub(super) struct PreparedDomain(Domain);

/// Run on a worker: validation, seeding and initial surface extraction can be expensive.
pub(super) fn prepare_domain(project: &Project, id: u64, cancel: &AtomicBool) -> Result<PreparedDomain, String> {
    let globals = project.global_transforms().map_err(|e| e.to_string())?;
    Domain::new(DomainSpec::new(project, id, &globals, false)?, cancel).map(PreparedDomain)
}

fn surface_mesh(surface: poseforge_simulation::SurfaceMesh) -> Mesh {
    Mesh {
        id: 0,
        name: "Жидкость · живая поверхность".into(),
        vertices: surface.positions.iter().zip(surface.normals).map(|(p, n)| Vertex {
            position: *p, normal: n, uv: [p[0], p[2]], ..Default::default()
        }).collect(),
        indices: surface.indices.into(),
        material: None,
        topology: None,
        morph_targets: vec![],
    }
}

fn cell_size(spec: &DomainSpec, settings: FluidDomainSettings) -> f32 {
    // Bound live extraction to roughly 64 cells along its longest axis.
    settings.simulation.surface_cell_size.max(spec.spacing * 0.45)
        .max((spec.max - spec.min).max_element() / 62.)
}

fn render_cell_size(spec: &DomainSpec, settings: FluidDomainSettings) -> f32 {
    // Respect authored detail down to the final-render budget, independently of
    // the 64-cell live preview. Padding is accounted for by the 124-cell bound.
    settings.simulation.surface_cell_size.max(0.005).max(spec.spacing * 0.25)
        .max((spec.max - spec.min).max_element() / 124.)
}

// Permits belong to the actual thread, including canceled jobs that are still
// retiring. Clearing a domain therefore cannot bypass the global work limit.
const MAX_SURFACES: usize = 2;
#[derive(Default)]
struct SurfaceBudget(Arc<std::sync::atomic::AtomicUsize>);
struct SurfacePermit(Arc<std::sync::atomic::AtomicUsize>);
impl SurfaceBudget {
    fn acquire(&self) -> Option<SurfacePermit> {
        self.0.try_update(Ordering::Relaxed, Ordering::Relaxed,
            |n| (n < MAX_SURFACES).then_some(n + 1)).ok().map(|_| SurfacePermit(self.0.clone()))
    }
    fn active(&self) -> usize { self.0.load(Ordering::Relaxed) }
}
impl Drop for SurfacePermit {
    fn drop(&mut self) { self.0.fetch_sub(1, Ordering::Relaxed); }
}

type SurfaceStamp = (u64, u32, u32);
struct SurfaceJob {
    receiver: Receiver<Result<Mesh, String>>,
    cancel: Arc<AtomicBool>,
    stamp: SurfaceStamp,
}
impl Drop for SurfaceJob {
    fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}
struct Domain {
    spec: DomainSpec,
    world: FluidWorld,
    preview: super::fluid_preview::FluidPreview,
    mesh: Mesh,
    initial_mesh: Mesh,
    surface_stamp: Option<SurfaceStamp>,
    job: Option<SurfaceJob>,
    error: Option<String>,
    simulation_error: Option<String>,
}
impl Domain {
    fn new(spec: DomainSpec, cancel: &AtomicBool) -> Result<Self, String> {
        let world = spec.world(cancel)?;
        // A bounded coarse surface gives immediate shape feedback while the finer
        // cancellable preview is prepared; it is never a bounding-box substitute.
        let cell = ((spec.max - spec.min).max_element() / 24.).max(spec.spacing * 0.75);
        let surface = poseforge_simulation::extract_surface_cancellable(&world, cell, 0.6, cancel).map_err(|e| e.to_string())?;
        let mesh = surface_mesh(surface);
        Ok(Self {
            world, initial_mesh: mesh.clone(), mesh, spec,
            preview: Default::default(), surface_stamp: None, job: None, error: None, simulation_error: None,
        })
    }
    fn stamp(&self, settings: FluidDomainSettings) -> SurfaceStamp {
        (self.world.tick(), cell_size(&self.spec, settings).to_bits(), settings.simulation.surface_iso_density.to_bits())
    }
    fn poll_surface(&mut self, settings: FluidDomainSettings) {
        let stamp = self.stamp(settings);
        if self.surface_stamp.is_some_and(|old| old.0 > stamp.0) {
            // Never leave a future frame on screen after rewinding/resetting time.
            self.mesh = self.initial_mesh.clone(); self.surface_stamp = None;
        }
        if self.job.as_ref().is_some_and(|job| job.stamp.0 > stamp.0
            || job.stamp.1 != stamp.1 || job.stamp.2 != stamp.2) {
            self.job = None;
        }
        if let Some(job) = &self.job {
            match job.receiver.try_recv() {
                Ok(result) => {
                    self.surface_stamp = Some(job.stamp);
                    match result {
                        Ok(mesh) => { self.mesh = mesh; self.error = None; }
                        Err(error) => self.error = Some(error),
                    }
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.surface_stamp = Some(job.stamp);
                    self.error = Some("Расчёт поверхности остановлен".into());
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }
    fn queue_surface(&mut self, settings: FluidDomainSettings, budget: &SurfaceBudget) -> bool {
        let stamp = self.stamp(settings);
        if self.job.is_some() || self.surface_stamp == Some(stamp) { return false; }
        let Some(permit) = budget.acquire() else { return false; };
        // Clone particle buffers only after a real worker slot is available.
        let world = self.world.clone();
        let cell = f32::from_bits(stamp.1); let iso = f32::from_bits(stamp.2);
        let cancel = Arc::new(AtomicBool::new(false)); let c = cancel.clone();
        let (tx, receiver) = mpsc::channel();
        match std::thread::Builder::new().name("fluid-surface".into()).spawn(move || {
            let _permit = permit;
            let result = poseforge_simulation::extract_surface_cancellable(&world, cell, iso, &c)
                .map(surface_mesh).map_err(|e| e.to_string());
            if !c.load(Ordering::Relaxed) { let _ = tx.send(result); }
        }) {
            Ok(_) => { self.job = Some(SurfaceJob { receiver, cancel, stamp }); true }
            Err(error) => {
                self.error = Some(format!("Не удалось запустить расчёт поверхности: {error}"));
                self.surface_stamp = Some(stamp); false
            }
        }
    }

}

const MAX_INITIALIZATIONS: usize = 2;
struct Initialization {
    spec: DomainSpec,
    receiver: Receiver<Result<PreparedDomain, String>>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Initialization {
    fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}

#[derive(Default)]
struct Playback {
    time: f64,
    running: bool,
}

#[derive(Default)]
pub(super) struct FluidDomains {
    playback: BTreeMap<u64, Playback>,
    domains: BTreeMap<u64, Domain>,
    errors: BTreeMap<u64, String>,
    failed: BTreeMap<u64, DomainSpec>,
    initializing: BTreeMap<u64, Initialization>,
    retiring: Vec<Initialization>,
    surface_budget: SurfaceBudget,
    last_surface: Option<u64>,
}
impl FluidDomains {
    fn retire(&mut self, job: Initialization) {
        job.cancel.store(true, Ordering::Relaxed);
        self.retiring.push(job);
    }
    fn poll_initializations(&mut self) {
        self.retiring.retain(|job| matches!(job.receiver.try_recv(), Err(mpsc::TryRecvError::Empty)));
        let completed: Vec<_> = self.initializing.iter().filter_map(|(&id, job)| {
            match job.receiver.try_recv() {
                Ok(result) => Some((id, result)),
                Err(mpsc::TryRecvError::Disconnected) => Some((id, Err("Подготовка жидкости остановлена".into()))),
                Err(mpsc::TryRecvError::Empty) => None,
            }
        }).collect();
        for (id, result) in completed {
            let job = self.initializing.remove(&id).unwrap();
            if job.cancel.load(Ordering::Relaxed) { continue; }
            match result {
                Ok(PreparedDomain(domain)) => { self.domains.insert(id, domain); self.errors.remove(&id); self.failed.remove(&id); }
                Err(error) => { self.errors.insert(id, error); self.failed.insert(id, job.spec.clone()); }
            }
        }
    }
    pub fn clear(&mut self) {
        self.playback.clear();
        self.domains.clear(); self.errors.clear(); self.failed.clear();
        let jobs = std::mem::take(&mut self.initializing);
        for (_, job) in jobs { self.retire(job); }
        self.poll_initializations();
    }
    pub fn reset(&mut self, id: u64) {
        self.domains.remove(&id); self.errors.remove(&id); self.failed.remove(&id);
        if let Some(job) = self.initializing.remove(&id) { self.retire(job); }
    }
    pub fn install(&mut self, id: u64, prepared: PreparedDomain) {
        self.reset(id);
        self.domains.insert(id, prepared.0);
    }
    pub fn initializing(&self, id: u64) -> bool { self.initializing.contains_key(&id) }
    pub fn playing(&self, id: u64) -> bool { self.playback.get(&id).is_some_and(|p| p.running) }
    pub fn any_playing(&self) -> bool { self.playback.values().any(|p| p.running) }
    pub fn pause_playback(&mut self) { for clock in self.playback.values_mut() { clock.running = false; } }
    pub fn toggle_playback(&mut self, id: u64) {
        let restart = self.errors.contains_key(&id) || self.domains.get(&id).is_some_and(|d|
            d.simulation_error.is_some() || d.error.is_some() || (!self.playback.contains_key(&id) && d.world.time() > 0.));
        if restart { self.reset(id); }
        let clock = self.playback.entry(id).or_default();
        if restart { clock.time = 0.; }
        clock.running = !clock.running;
    }
    pub fn reset_playback(&mut self, id: u64) {
        self.playback.insert(id, Playback::default());
        self.reset(id);
    }
    pub fn follow_timeline(&mut self) { self.playback.clear(); }
    pub fn advance_playback(&mut self, project: &Project, dt: f64) {
        if !dt.is_finite() || dt < 0. { return; }
        let visible = project.visible_node_ids(false);
        self.playback.retain(|id, _| project.node(*id).is_some_and(|n| n.fluid.is_some()));
        for (&id, clock) in &mut self.playback {
            let Some(settings) = project.node(id).and_then(|n| n.fluid) else { continue; };
            if !settings.enabled || !visible.contains(&id) || self.errors.contains_key(&id) { clock.running = false; }
            if !clock.running { continue; }
            // Preparation and slow solvers cannot accumulate an unbounded catch-up.
            if let Some(domain) = self.domains.get(&id) {
                if domain.simulation_error.is_some() || domain.error.is_some() { clock.running = false; continue; }
                let calculated = domain.world.time() / settings.simulation.time_scale as f64;
                clock.time = (clock.time + dt.min(0.1)).min(calculated + 0.25);
            }
        }
    }
    pub fn surface_pending(&self, id: u64, settings: FluidDomainSettings) -> bool {
        self.domains.get(&id).is_none_or(|d| d.job.is_some() || d.surface_stamp != Some(d.stamp(settings)))
    }
    pub fn is_pending(&self) -> bool {
        !self.initializing.is_empty() || !self.retiring.is_empty() || self.surface_budget.active() > 0
            || self.domains.values().any(|d| d.job.is_some() || d.preview.is_pending())
    }
    pub fn info(&self, id: u64) -> (usize, f64, Option<&str>) {
        self.domains.get(&id).map_or((0, 0., self.errors.get(&id).map(String::as_str)), |d| {
            (d.world.positions().len(), d.world.time(), d.simulation_error.as_deref().or(d.error.as_deref()))
        })
    }
    pub fn update(&mut self, project: &Project, globals: &BTreeMap<u64, Mat4>, time: f64) {
        self.poll_initializations();
        let visible = project.visible_node_ids(false);
        self.domains.retain(|id, _| project.node(*id).is_some_and(|n| n.fluid.is_some()) && visible.contains(id));
        self.errors.retain(|id, _| project.node(*id).is_some_and(|n| n.fluid.is_some()));
        self.failed.retain(|id, _| project.node(*id).is_some_and(|n| n.fluid.is_some()));
        let obsolete: Vec<_> = self.initializing.keys().copied().filter(|id| {
            !visible.contains(id) || project.node(*id).is_none_or(|n| n.fluid.is_none())
        }).collect();
        for id in obsolete { if let Some(job) = self.initializing.remove(&id) { self.retire(job); } }
        for node in project.nodes.iter().filter(|n| n.fluid.is_some() && visible.contains(&n.id)) {
            let settings = node.fluid.unwrap();
            let spec = match DomainSpec::new(project, node.id, globals, false) {
                Ok(spec) => spec,
                Err(error) => {
                    self.reset(node.id); self.errors.insert(node.id, error); continue;
                }
            };
            if self.domains.get(&node.id).is_none_or(|d| d.spec != spec) {
                if let Some(clock) = self.playback.get_mut(&node.id) { clock.time = 0.; }
                self.domains.remove(&node.id);
                if self.failed.get(&node.id) == Some(&spec) { continue; }
                if let Some(job) = self.initializing.get(&node.id) {
                    if job.spec != spec { job.cancel.store(true, Ordering::Relaxed); }
                    continue;
                }
                if self.initializing.len() + self.retiring.len() >= MAX_INITIALIZATIONS { continue; }
                self.errors.remove(&node.id); self.failed.remove(&node.id);
                let work = spec.clone();
                let cancel = Arc::new(AtomicBool::new(false)); let c = cancel.clone();
                let (tx, receiver) = mpsc::channel();
                match std::thread::Builder::new().name("fluid-initialization".into()).spawn(move || {
                    let _ = tx.send(Domain::new(work, &c).map(PreparedDomain));
                }) {
                    Ok(_) => { self.initializing.insert(node.id, Initialization { spec, receiver, cancel }); }
                    Err(error) => { self.errors.insert(node.id, error.to_string()); self.failed.insert(node.id, spec); }
                }
                continue;
            }
            let domain = self.domains.get_mut(&node.id).unwrap();
            if settings.enabled {
                let time = self.playback.get(&node.id).map_or(time, |clock| clock.time);
                domain.simulation_error = domain.preview.seek(&mut domain.world, time * settings.simulation.time_scale as f64).err();
            } else {
                domain.preview.cancel();
                domain.world.reset();
            }
            domain.preview.cache.trim_to(16 * 1024 * 1024);
            domain.poll_surface(settings);
        }
        // Round-robin avoids starving later objects when the first liquid is
        // continuously changing during playback.
        let mut order: Vec<_> = self.domains.keys().copied().collect();
        if let Some(last) = self.last_surface {
            let start = order.partition_point(|&id| id <= last);
            order.rotate_left(start);
        }
        for id in order {
            let Some(settings) = project.node(id).and_then(|n| n.fluid) else { continue; };
            if self.domains.get_mut(&id).unwrap().queue_surface(settings, &self.surface_budget) {
                self.last_surface = Some(id);
            }
            if self.surface_budget.active() >= MAX_SURFACES { break; }
        }
    }
    pub fn apply(&self, project: &mut Project, editing: Option<u64>) {
        hide_fluid_surfaces(project);
        for (&id, domain) in &self.domains {
            if editing == Some(id) { continue; }
            let Some(node) = project.node(id).filter(|n| n.fluid.is_some()) else { continue };
            let Some(source) = node.mesh.and_then(|m| project.mesh(m)) else { continue };
            if !domain.spec.matches_source(source) { continue; }
            let mut mesh = domain.mesh.clone();
            mesh.id = project.next_id();
            mesh.material = source.material;
            project.node_mut(id).unwrap().mesh = Some(mesh.id);
            project.meshes.push(mesh);
        }
    }
}

// Remove only the component geometry in the render snapshot: child objects stay
// visible and the authored primitive, settings and simulation state are retained.
fn hide_fluid_surfaces(project: &mut Project) {
    for node in &mut project.nodes {
        if node.fluid.is_some_and(|fluid| !fluid.visible) {
            node.mesh = None;
            node.material_layers.clear();
            node.fluid = None;
        }
    }
}

// Export reconstructs each frame automatically, without changing document meshes.
#[derive(Default)]
pub(crate) struct ExportDomains(BTreeMap<u64, (DomainSpec, FluidWorld)>);
impl ExportDomains {
    pub fn apply(&mut self, project: &mut Project, globals: &BTreeMap<u64, Mat4>, time: f64, cancel: &AtomicBool) -> Result<(), String> {
        hide_fluid_surfaces(project);
        let visible = project.visible_node_ids(true);
        let ids: Vec<_> = project.nodes.iter().filter(|n| n.fluid.is_some() && visible.contains(&n.id)).map(|n| n.id).collect();
        for id in ids {
            if cancel.load(Ordering::Relaxed) { return Err("Рендер отменён".into()); }
            let settings = project.node(id).unwrap().fluid.unwrap();
            let spec = DomainSpec::new(project, id, globals, true)?;
            if self.0.get(&id).is_none_or(|(old, _)| old != &spec) {
                self.0.insert(id, (spec.clone(), spec.world(cancel)?));
            }
            let (_, world) = self.0.get_mut(&id).unwrap();
            let target = if settings.enabled { time * settings.simulation.time_scale as f64 } else { 0. };
            let dt = world.config().time_step as f64;
            let tick = ((target + 1e-10) / dt).floor() as u64;
            if world.tick() > tick { world.reset(); }
            while world.tick() < tick {
                if cancel.load(Ordering::Relaxed) { return Err("Рендер отменён".into()); }
                let steps = (tick - world.tick()).min(8) as usize;
                world.advance_cancellable(steps as f64 * dt, steps, cancel);
            }
            let surface = poseforge_simulation::extract_surface_render_cancellable(world, render_cell_size(&spec, settings), settings.simulation.surface_iso_density, cancel)
                .map_err(|e| e.to_string())?;
            let source = project.node(id).unwrap().mesh.and_then(|m| project.mesh(m)).unwrap();
            let mut mesh = surface_mesh(surface);
            mesh.id = project.next_id();
            mesh.material = source.material;
            project.node_mut(id).unwrap().mesh = Some(mesh.id);
            project.meshes.push(mesh);
        }
        Ok(())
    }
}

impl App {
    pub(super) fn tick_fluid_playback(&mut self, dt: f64) {
        if self.editor.playing { self.fluid_domains.follow_timeline(); return; }
        if self.job.is_none() && self.drag.is_none() && self.input.is_none() && !self.has_modal() {
            self.fluid_domains.advance_playback(&self.editor.project, dt);
        }
    }
    pub(super) fn tick_fluid_domains(&mut self) {
        if !self.editor.project.nodes.iter().any(|n| n.fluid.is_some()) {
            self.fluid_domains.clear();
            return;
        }
        let editing = self.input.as_ref().is_some_and(|i| matches!(i.purpose, Action::FluidObject(ui::FluidObjectAction::Field(_))|Action::Helper(ui::HelperAction::Number(_))))
            || matches!(&self.drag, Some(Drag::Slider(d)) if matches!(d.action, Action::FluidObject(ui::FluidObjectAction::Field(_))|Action::Helper(ui::HelperAction::Number(_))));
        if editing { return; }
        if let Ok(eval) = self.editor.evaluated() {
            self.fluid_domains.update(&self.editor.project, &eval.global, self.editor.time);
        }
    }
    pub(super) fn draw_fluid_container(&self, ui: &mut Ui) {
        let Some(node) = self.editor.selected.and_then(|id| self.editor.project.node(id)).filter(|n| n.fluid.is_some()) else { return };
        if !self.editor.project.node_visible(node.id, false) { return; }
        let Some(mesh) = node.mesh.and_then(|id| self.editor.project.mesh(id)) else { return };
        let Ok(eval) = self.editor.evaluated() else { return };
        let Some(model) = eval.global.get(&node.id) else { return };
        if node.fluid.is_some_and(|fluid| !fluid.visible) { return; }
        let mut edges = BTreeSet::new();
        for face in 0..mesh.face_count() {
            let Some(vertices) = mesh.face_vertices(face) else { continue; };
            for i in 0..vertices.len() {
                let a = vertices[i]; let b = vertices[(i + 1) % vertices.len()];
                edges.insert((a.min(b), a.max(b)));
            }
        }
        let start = (ui.rects.len(), ui.texts.len(), ui.hits.len());
        let mut color = ui.palette().accent; color[3] *= 0.35;
        for (a, b) in edges.into_iter().take(12000) {
            let a = model.transform_point3(Vec3::from_array(mesh.vertices[a as usize].position));
            let b = model.transform_point3(Vec3::from_array(mesh.vertices[b as usize].position));
            if let (Some(a), Some(b)) = (self.project_point(a), self.project_point(b)) {
                ui.mesh_segment(a, b, 1., color);
            }
        }
        clip_scrolled(ui, start, self.viewport(), 0.);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{box_mesh, Material, Node, PhysicsKind, PhysicsSettings};

    pub(super) fn project() -> Project {
        let mut p = Project::default();
        p.materials.push(Material { id: 3, ..Default::default() });
        p.meshes.push(box_mesh(2, "Container", Vec3::splat(0.3), Some(3)));
        let mut node = Node::new(1, "Liquid");
        node.mesh = Some(2);
        node.fluid = Some(FluidDomainSettings::default());
        node.transform.translation = Vec3::new(3., 2., -4.);
        p.nodes.push(node);
        p
    }

    #[test]
    fn independent_clock_waits_for_preparation_and_bounds_solver_catch_up() {
        let p = project(); let mut runtime = FluidDomains::default();
        runtime.toggle_playback(1); runtime.advance_playback(&p, 50.);
        assert_eq!(runtime.playback[&1].time, 0.);
        runtime.install(1, prepare_domain(&p, 1, &AtomicBool::new(false)).unwrap());
        for _ in 0..100 { runtime.advance_playback(&p, 1.); }
        assert_eq!(runtime.playback[&1].time, 0.25);
        runtime.toggle_playback(1); runtime.advance_playback(&p, 1.);
        assert_eq!(runtime.playback[&1].time, 0.25); assert!(!runtime.any_playing());
        runtime.reset_playback(1); assert_eq!(runtime.playback[&1].time, 0.);
        runtime.toggle_playback(1); runtime.errors.insert(1, "Preparation failed".into());
        runtime.advance_playback(&p, 0.1); assert!(!runtime.playing(1));
        runtime.toggle_playback(1); assert!(runtime.playing(1)); assert!(!runtime.errors.contains_key(&1));
        runtime.follow_timeline(); assert!(runtime.playback.is_empty());
    }

    #[test]
    fn liquid_controls_preserve_document_time_and_existing_keys() {
        use crate::ui::FluidObjectAction as F;
        use poseforge_core::{Channel, Clip, Interpolation, Keyframe, Property};
        let mut p = project(); let mut clip = Clip::new("Existing keys", 4.);
        clip.channels.push(Channel { node: 1, property: Property::Translation, interpolation: Interpolation::Linear,
            keys: vec![Keyframe::new(0., vec![3., 2., -4.]), Keyframe::new(4., vec![3., 2., -4.])] });
        p.clips.push(clip);
        let mut a = App::new(None); a.editor = Editor::new(p); a.reset_workflow_selection();
        a.preferences = Default::default(); a.editor.selected = Some(1);
        a.editor.active_clip = Some(0); a.editor.time = 1.25;
        let before = a.editor.project.clone(); let undo = a.editor.undo_len(); let mode = a.editor.mode;
        a.editor.playing = true; a.fluid_object_action(F::Play);
        assert!(a.fluid_domains.playing(1)); assert!(!a.editor.playing);
        a.fluid_object_action(F::Play); assert!(!a.fluid_domains.playing(1));
        a.fluid_object_action(F::Play); a.fluid_object_action(F::Reset);
        assert!(!a.fluid_domains.playing(1)); assert_eq!(a.fluid_domains.playback[&1].time, 0.);
        assert_eq!(a.editor.project, before); assert_eq!(a.editor.undo_len(), undo);
        assert_eq!(a.editor.time, 1.25); assert_eq!(a.editor.active_clip, Some(0)); assert!(a.editor.mode == mode);
    }

    #[test]
    fn local_container_bounds_fill_and_gravity_follow_the_object() {
        let mut p = project();
        p.nodes[0].transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let globals = p.global_transforms().unwrap();
        let spec = DomainSpec::new(&p, 1, &globals, false).unwrap();
        let world = spec.world(&AtomicBool::new(false)).unwrap();
        assert!(!world.positions().is_empty());
        assert!(world.positions().len() <= 6000);
        assert!(spec.config.gravity.x < -9.8 && spec.config.gravity.y.abs() < 0.001);
        assert!(world.positions().iter().all(|v| v.cmpgt(spec.min).all() && v.cmplt(spec.max).all()
            && v.y <= spec.min.y + (spec.max.y - spec.min.y) * spec.fill));
        let domain = Domain::new(spec.clone(), &AtomicBool::new(false)).unwrap();
        assert!(!domain.mesh.indices.is_empty());
        let (min, max) = mesh_bounds(&domain.mesh);
        assert!(min.cmpge(spec.min).all() && max.cmple(spec.max).all());
    }

    #[test]
    fn geometry_edits_with_unchanged_bounds_invalidate_the_domain() {
        let mut p = project();
        let globals = p.global_transforms().unwrap();
        let before = DomainSpec::new(&p, 1, &globals, false).unwrap();
        p.meshes[0].vertices[0].position[0] *= 0.9;
        let after = DomainSpec::new(&p, 1, &globals, false).unwrap();
        assert_eq!((before.min, before.max), (after.min, after.max));
        assert!(before != after);
        assert!(!before.matches_source(&p.meshes[0]));
    }

    #[test]
    fn mesh_obstacles_exclude_initial_liquid_volume_and_switch_is_part_of_spec() {
        let mut p = project();
        let mut obstacle = Node::new(4, "Solid obstacle"); obstacle.mesh = Some(5);
        obstacle.transform = p.nodes[0].transform;
        obstacle.physics = Some(PhysicsSettings { kind: PhysicsKind::Collider, ..Default::default() });
        p.nodes.push(obstacle); p.meshes.push(box_mesh(5, "Solid", Vec3::splat(0.12), None));
        let globals = p.global_transforms().unwrap();
        let on = DomainSpec::new(&p, 1, &globals, false).unwrap();
        let world = on.world(&AtomicBool::new(false)).unwrap();
        assert!(world.positions().iter().all(|point| point.abs().max_element() >= 0.12));
        p.nodes[0].fluid.as_mut().unwrap().mesh_collision = false;
        let off = DomainSpec::new(&p, 1, &globals, false).unwrap();
        assert!(off.colliders.is_empty()); assert!(on != off);
        let free = off.world(&AtomicBool::new(false)).unwrap();
        assert!(free.positions().len() > world.positions().len());
        assert_ne!(free.signature(), world.signature());
        p.nodes[0].fluid.as_mut().unwrap().mesh_collision = true;
        p.node_mut(4).unwrap().physics.as_mut().unwrap().visible = false;
        assert!(DomainSpec::new(&p, 1, &globals, false).unwrap().colliders.is_empty());
    }
    #[test]
    fn per_receiver_list_changes_fluid_volume_without_global_collider_component() {
        let mut p=project();let mut obstacle=Node::new(4,"Explicit obstacle");obstacle.mesh=Some(5);
        obstacle.transform=p.nodes[0].transform;p.nodes.push(obstacle);
        p.meshes.push(box_mesh(5,"Solid",Vec3::splat(0.12),None));
        p.node_mut(1).unwrap().collision_targets.fluid=Some(vec![poseforge_core::CollisionTarget {node:4,enabled:true}]);
        let globals=p.global_transforms().unwrap();
        let on=DomainSpec::new(&p,1,&globals,false).unwrap();let world=on.world(&AtomicBool::new(false)).unwrap();
        assert_eq!(on.colliders.len(),1);
        assert!(world.positions().iter().all(|point|point.abs().max_element()>=0.12));
        p.node_mut(1).unwrap().collision_targets.fluid.as_mut().unwrap()[0].enabled=false;
        let off=DomainSpec::new(&p,1,&globals,false).unwrap();assert!(off.colliders.is_empty());assert!(on!=off);
        assert!(off.world(&AtomicBool::new(false)).unwrap().positions().len()>world.positions().len());
        assert!(p.node(4).unwrap().physics.is_none());assert!(p.node(4).unwrap().visible);
    }

    #[test]
    fn failed_initialization_is_not_retried_until_input_changes() {
        let mut p = project();
        let globals = p.global_transforms().unwrap();
        let spec = DomainSpec::new(&p, 1, &globals, false).unwrap();
        let (tx, receiver) = mpsc::channel();
        assert!(tx.send(Err("No closed interior".into())).is_ok());
        let mut runtime = FluidDomains::default();
        runtime.initializing.insert(1, Initialization {
            spec, receiver, cancel: Arc::new(AtomicBool::new(false)),
        });
        for _ in 0..5 { runtime.update(&p, &globals, 0.); }
        assert!(runtime.domains.is_empty() && runtime.initializing.is_empty());
        assert_eq!(runtime.info(1).2, Some("No closed interior"));
        p.nodes[0].fluid.as_mut().unwrap().fill = 0.5;
        runtime.update(&p, &globals, 0.);
        assert!(runtime.initializing.contains_key(&1));
    }

    #[test]
    fn deleted_container_cancels_and_discards_its_late_result() {
        let mut p = project();
        let globals = p.global_transforms().unwrap();
        let spec = DomainSpec::new(&p, 1, &globals, false).unwrap();
        let (tx, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut runtime = FluidDomains::default();
        runtime.initializing.insert(1, Initialization { spec, receiver, cancel: cancel.clone() });
        p.nodes.clear();
        runtime.update(&p, &globals, 0.);
        assert!(cancel.load(Ordering::Relaxed));
        assert!(runtime.initializing.is_empty() && runtime.is_pending());
        assert!(tx.send(Err("Cancelled worker completed".into())).is_ok());
        runtime.update(&p, &globals, 0.);
        assert!(!runtime.is_pending());
        assert!(runtime.domains.is_empty() && runtime.errors.is_empty());
    }

    #[test]
    fn many_containers_have_a_bounded_initialization_queue() {
        let mut p = project();
        for id in 4..10 {
            let mut node = p.nodes[0].clone(); node.id = id;
            p.nodes.push(node);
        }
        let mut runtime = FluidDomains::default();
        runtime.update(&p, &p.global_transforms().unwrap(), 0.);
        assert_eq!(runtime.initializing.len(), MAX_INITIALIZATIONS);
        runtime.clear();
        assert!(runtime.retiring.iter().all(|job| job.cancel.load(Ordering::Relaxed)));
    }

    #[test]
    fn live_surface_is_automatic_material_edits_keep_state_and_deletion_cancels_jobs() {
        let mut p = project();
        let globals = p.global_transforms().unwrap();
        let original = p.clone();
        let mut runtime = FluidDomains::default();
        runtime.update(&p, &globals, 0.);
        assert!(runtime.is_pending());
        let deadline = Instant::now() + Duration::from_secs(20);
        while runtime.is_pending() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
            runtime.update(&p, &globals, 0.);
        }
        let domain = &runtime.domains[&1];
        assert!(domain.surface_stamp.is_some() && domain.error.is_none());
        assert!(!domain.mesh.indices.is_empty());
        let mut preview = p.clone();
        runtime.apply(&mut preview, None);
        assert_eq!(p, original);
        let rendered = preview.mesh(preview.node(1).unwrap().mesh.unwrap()).unwrap();
        assert_eq!(rendered.material, Some(3));
        assert_ne!(rendered.id, 2);
        p.materials[0].roughness = 0.2;
        runtime.update(&p, &globals, 0.);
        assert!(!runtime.is_pending(), "Material edits must not recreate the simulation or surface");
        runtime.update(&p, &globals, 0.05);
        assert!(runtime.is_pending());
        p.nodes.clear();
        runtime.update(&p, &globals, 0.05);
        assert!(runtime.domains.is_empty());
        assert!(runtime.initializing.is_empty());
    }

    #[test]
    fn domains_are_independent_and_hidden_parent_colliders_are_ignored() {
        let mut p = project();
        let mut second = p.nodes[0].clone();
        second.id = 4;
        second.transform.translation.x = 10.;
        second.fluid.as_mut().unwrap().fill = 0.3;
        p.nodes.push(second);
        let mut parent = Node::new(5, "Hidden group");
        parent.visible = false;
        p.nodes.push(parent);
        let mut collider = Node::new(6, "Hidden collider");
        collider.parent = Some(5);
        collider.mesh = Some(2);
        collider.transform = p.nodes[0].transform;
        collider.physics = Some(PhysicsSettings { kind: PhysicsKind::Collider, ..Default::default() });
        p.nodes.push(collider);
        let globals = p.global_transforms().unwrap();
        assert!(DomainSpec::new(&p, 1, &globals, false).unwrap().colliders.is_empty());
        let mut runtime = FluidDomains::default();
        runtime.update(&p, &globals, 0.);
        let deadline = Instant::now() + Duration::from_secs(20);
        while runtime.domains.len() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2)); runtime.update(&p, &globals, 0.);
        }
        assert_eq!(runtime.domains.len(), 2);
        assert_ne!(runtime.domains[&1].spec.fill, runtime.domains[&4].spec.fill);
        let before = runtime.domains[&4].world.positions().to_vec();
        runtime.reset(1);
        assert_eq!(runtime.domains[&4].world.positions(), before);
    }

    #[test]
    fn export_uses_liquid_surface_and_honors_cancellation() {
        let p = project();
        let globals = p.global_transforms().unwrap();
        let mut output = p.clone();
        let mut export = ExportDomains::default();
        assert!(export.apply(&mut output, &globals, 0., &AtomicBool::new(true)).is_err());
        assert_eq!(output, p);
        export.apply(&mut output, &globals, 0., &AtomicBool::new(false)).unwrap();
        let mesh = output.mesh(output.node(1).unwrap().mesh.unwrap()).unwrap();
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.material, Some(3));
        assert_eq!(p.node(1).unwrap().mesh, Some(2));
    }
}

#[cfg(test)]
mod shape_tests {
    use super::*;
    use super::tests::project;
    use poseforge_core::{Node, PrimitiveKind, PrimitiveSettings};
    #[test]
    fn sphere_seeding_and_initial_preview_preserve_the_primitive_shape() {
        let mut node = Node::new(1, "Sphere");
        node.mesh = Some(2);
        node.fluid = Some(FluidDomainSettings::default());
        let project = Project {
            nodes: vec![node],
            meshes: vec![PrimitiveSettings::new(PrimitiveKind::Sphere).mesh(2, None).unwrap()],
            ..Default::default()
        };
        let globals = project.global_transforms().unwrap();
        let spec = DomainSpec::new(&project, 1, &globals, false).unwrap();
        let domain = Domain::new(spec.clone(), &AtomicBool::new(false)).unwrap();
        let shape = domain.world.container().unwrap();
        assert!(domain.world.positions().iter().all(|p| shape.contains(*p)));
        let center = (spec.min + spec.max) * 0.5;
        let corner = center + (spec.max - spec.min) * 0.4;
        assert!(!shape.contains(corner));
        assert!(!domain.mesh.indices.is_empty());
        assert!(domain.mesh.vertices.iter().all(|v| {
            let p = Vec3::from_array(v.position);
            shape.contains(p) || p.distance(shape.closest(p)) < 1e-4
        }));
    }
    #[test]
    fn hiding_the_component_preserves_children_and_the_authored_document() {
        let mut p = Project::default();
        let mut node = Node::new(1, "Container");
        node.mesh = Some(2);
        node.fluid = Some(FluidDomainSettings { visible: false, ..Default::default() });
        let mut child = Node::new(3, "Child");
        child.parent = Some(1);
        p.nodes = vec![node, child];
        p.meshes.push(PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2, None).unwrap());
        let original = p.clone();
        let mut preview = p.clone();
        FluidDomains::default().apply(&mut preview, None);
        assert!(preview.node(1).unwrap().mesh.is_none());
        assert!(preview.node_visible(3, false));
        assert_eq!(p, original);
        preview.validate().unwrap();
    }

    #[test]
    fn surface_slots_remain_owned_until_canceled_threads_really_exit() {
        let budget = SurfaceBudget::default();
        let first = budget.acquire().unwrap(); let second = budget.acquire().unwrap();
        assert_eq!(budget.active(), MAX_SURFACES); assert!(budget.acquire().is_none());
        drop(first); assert_eq!(budget.active(), 1);
        let next = budget.acquire().unwrap(); assert!(budget.acquire().is_none());
        drop(second); drop(next); assert_eq!(budget.active(), 0);
    }
    #[test]
    fn rewind_rejects_future_surface_and_parameter_edits_cancel_old_extraction() {
        let p = project(); let settings = p.node(1).unwrap().fluid.unwrap();
        let spec = DomainSpec::new(&p, 1, &p.global_transforms().unwrap(), false).unwrap();
        let mut domain = Domain::new(spec, &AtomicBool::new(false)).unwrap();
        let stamp = domain.stamp(settings);
        domain.surface_stamp = Some((10, stamp.1, stamp.2));
        let original = domain.initial_mesh.vertices.revision();
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        domain.job = Some(SurfaceJob { receiver, cancel: cancel.clone(), stamp: (11, stamp.1, stamp.2) });
        sender.send(Ok(Mesh { name: "stale".into(), ..domain.mesh.clone() })).unwrap();
        domain.poll_surface(settings);
        assert!(cancel.load(Ordering::Relaxed)); assert!(domain.job.is_none());
        assert_eq!(domain.mesh.vertices.revision(), original); assert!(domain.surface_stamp.is_none());
        let (_sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        domain.job = Some(SurfaceJob { receiver, cancel: cancel.clone(), stamp });
        let mut edited = settings; edited.simulation.surface_iso_density = 0.8;
        domain.poll_surface(edited); assert!(cancel.load(Ordering::Relaxed));
    }
    #[test]
    fn final_render_surface_detail_is_independent_of_the_interactive_cap() {
        let p = project(); let spec = DomainSpec::new(&p, 1, &p.global_transforms().unwrap(), false).unwrap();
        let mut settings = p.node(1).unwrap().fluid.unwrap(); settings.simulation.surface_cell_size = 0.005;
        assert!(render_cell_size(&spec, settings) < cell_size(&spec, settings));
        assert!(render_cell_size(&spec, settings) >= 0.005);
    }

    #[test]
    fn domain_fields_change_cache_identity_and_transform_world_wind_to_local_space() {
        let mut p=project();let globals=p.global_transforms().unwrap();let initial=DomainSpec::new(&p,1,&globals,false).unwrap();
        let id=p.next_id();let mut field=Node::new(id,"Wind");field.empty=true;field.force_field=Some(poseforge_core::ForceFieldSettings{kind:poseforge_core::ForceFieldKind::Wind,radius:100.,falloff:0.,..Default::default()});p.nodes.push(field);
        let globals=p.global_transforms().unwrap();let wind=DomainSpec::new(&p,1,&globals,false).unwrap();assert!(wind!=initial);assert!(wind.fields.acceleration(Vec3::ZERO,Vec3::ZERO).z>9.);
        p.node_mut(id).unwrap().force_field.as_mut().unwrap().enabled=false;let disabled=DomainSpec::new(&p,1,&p.global_transforms().unwrap(),false).unwrap();assert!(disabled==initial);
    }

}
