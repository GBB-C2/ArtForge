//! Bounded, lazy GPU previews. A dedicated worker owns its renderer; UI never waits on readback.
use super::*;
use poseforge_core::{
    Material, MaterialGraph, MaterialNode, MaterialNodeKind, Mesh, Node, Texture, Vertex,
};
use poseforge_render::UiRect;
use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    sync::mpsc::{SyncSender, TryRecvError},
};

const SIZE: u32 = 80;
const CACHE_LIMIT: usize = 64;
const TEXTURE_LIMIT: u32 = 256;
type Preview = Result<Vec<UiRect>, String>;
struct Request {
    key: u64,
    content: Content,
}
enum Content {
    Material(Material, Vec<Texture>),
    Asset(super::asset_thumbnails::AssetRequest),
}
struct Worker {
    tx: Option<SyncSender<Request>>,
    rx: Option<Receiver<(u64, Preview)>>,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Disconnect both channels, including a worker blocked publishing a
        // result. Never wait on an in-flight GPU readback from the UI thread.
        self.tx.take();
        self.rx.take();
        if let Some(join) = self.join.take() {
            if join.is_finished() { let _ = join.join(); }
            // Dropping an unfinished handle detaches it. The worker owns its
            // renderer and exits after the current readback observes stop.
        }
    }
}
#[derive(Default)]
pub(super) struct MaterialPreviews {
    worker: Option<Worker>,
    wake: Option<winit::event_loop::EventLoopProxy<()>>,
    cache: BTreeMap<u64, Preview>,
    order: VecDeque<u64>,
    pending: BTreeSet<u64>,
    failed: bool,
}
fn slots(m: &Material) -> [Option<u64>; 5] {
    [
        m.base_color_texture,
        m.metallic_roughness_texture,
        m.normal_texture,
        m.occlusion_texture,
        m.emissive_texture,
    ]
}
pub(super) fn uses_texture(m: &Material, id: u64) -> bool {
    slots(m).contains(&Some(id))
        || m.graph
            .as_ref()
            .is_some_and(|g| g.enabled && g.nodes.iter().any(|n| n.texture == Some(id)))
}
fn fingerprint(m: &Material, textures: &[Texture]) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(m).unwrap_or_default().hash(&mut hash);
    // Revisions detect in-place edits even when the buffer has a single owner.
    for t in textures.iter().filter(|t| uses_texture(m, t.id)) {
        (t.id, t.width, t.height, t.rgba8.revision(), t.rgba8.len()).hash(&mut hash);
    }
    hash.finish()
}
pub(super) fn small_texture(t: &Texture) -> Texture {
    let ratio = (TEXTURE_LIMIT as f64 / t.width.max(t.height).max(1) as f64).min(1.);
    let width = (t.width as f64 * ratio).round().max(1.) as u32;
    let height = (t.height as f64 * ratio).round().max(1.) as u32;
    let mut rgba8 = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let sx = (x as u64 * t.width as u64 / width as u64) as usize;
            let sy = (y as u64 * t.height as u64 / height as u64) as usize;
            let index = (sy * t.width as usize + sx) * 4;
            rgba8.extend_from_slice(t.rgba8.get(index..index + 4).unwrap_or(&[255, 0, 255, 255]));
        }
    }
    Texture {
        id: t.id,
        name: t.name.clone(),
        width,
        height,
        rgba8: rgba8.into(),
    }
}
fn material_content(material: &Material, textures: &[Texture]) -> Content {
    Content::Material(material.clone(), textures.iter().filter(|t| uses_texture(material,t.id)).cloned().collect())
}
fn preview_project(material: Material, textures: Vec<Texture>) -> Project {
    let mut p = Project {
        materials: vec![material],
        textures,
        ..Project::default()
    };
    let id = p.next_id();
    p.meshes
        .push(super::studio_controls::sphere(id, p.materials[0].id));
    let mut node = Node::new(id + 1, "Material preview");
    node.mesh = Some(id);
    p.nodes.push(node);
    let mut graph = MaterialGraph::default();
    let mut light = MaterialNode::new(1, MaterialNodeKind::Color, [0., 0.]);
    light.value = [0.65, 0.65, 0.65, 1.];
    let mut dark = MaterialNode::new(2, MaterialNodeKind::Color, [0., 0.]);
    dark.value = [0.11, 0.13, 0.16, 1.];
    let mut checker = MaterialNode::new(3, MaterialNodeKind::Checker, [0., 0.]);
    checker.value[0] = 8.;
    checker.inputs = [Some(1), Some(2), None];
    graph.nodes = vec![light, dark, checker];
    graph.outputs[0] = Some(3);
    p.materials.push(Material {
        id: id + 2,
        name: "Preview backdrop".into(),
        roughness: 1.,
        graph: Some(graph),
        ..Material::default()
    });
    let vertices = [
        [-2., -2., -0.8],
        [2., -2., -0.8],
        [2., 2., -0.8],
        [-2., 2., -0.8],
    ]
    .into_iter()
    .zip([[0., 0.], [1., 0.], [1., 1.], [0., 1.]])
    .map(|(position, uv)| Vertex {
        position,
        normal: [0., 0., 1.],
        uv,
        ..Vertex::default()
    })
    .collect();
    p.meshes.push(Mesh {
        id: id + 3,
        name: "Preview backdrop".into(),
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3].into(),
        material: Some(id + 2),
        topology: None,
        morph_targets: vec![],
    });
    let mut backdrop = Node::new(id + 4, "Preview backdrop");
    backdrop.mesh = Some(id + 3);
    p.nodes.push(backdrop);
    p.lights = vec![
        poseforge_core::Light {
            direction: Vec3::new(0.45, -0.65, -0.8).normalize(),
            intensity: 3.,
            casts_shadow: false,
            ..Default::default()
        },
        poseforge_core::Light {
            direction: Vec3::new(-0.8, 0.1, -0.5).normalize(),
            intensity: 0.7,
            color: [0.72, 0.82, 1.],
            casts_shadow: false,
            ..Default::default()
        },
    ];
    p.render_settings.shadows = false;
    p.render_settings.sky_color = [0.5, 0.55, 0.65];
    p.render_settings.ground_color = [0.07, 0.065, 0.06];
    p
}
fn render_preview(renderer: &mut Renderer, request: Request) -> Preview {
    let (material, textures) = match request.content {
        Content::Material(material,textures) => (material,textures.iter().map(small_texture).collect()),
        Content::Asset(request) => return super::asset_thumbnails::render(renderer,request),
    };
    let p = preview_project(material, textures);
    let globals = p.nodes.iter().map(|n| (n.id, Mat4::IDENTITY)).collect();
    let view = View {
        eye: Vec3::new(0., 0., 3.),
        target: Vec3::ZERO,
        orthographic: Some(0.5),
        viewport: [0., 0., SIZE as f32, SIZE as f32],
        ..View::default()
    };
    renderer.invalidate_assets();
    let rgba = renderer.render_rgba(&p, &globals, &view, SIZE, SIZE)?;
    Ok(pixel_runs(&rgba))
}
fn pixel_runs(rgba: &[u8]) -> Vec<UiRect> { image_runs(rgba,SIZE,true) }
pub(super) fn image_runs(rgba: &[u8], size: u32, circular: bool) -> Vec<UiRect> {
    let mut rects = vec![];
    for y in 0..size {
        let mut x = 0;
        while x < size {
            let pixel = |x: u32| {
                let i = ((y * size + x) * 4) as usize;
                let dx = x as f32 + 0.5 - size as f32 * 0.5;
                let dy = y as f32 + 0.5 - size as f32 * 0.5;
                let coverage = (size as f32 * 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0., 1.);
                [
                    rgba[i] & 0xfc,
                    rgba[i + 1] & 0xfc,
                    rgba[i + 2] & 0xfc,
                    if circular { (coverage * 255.) as u8 } else { 255 },
                ]
            };
            let color = pixel(x);
            let start = x;
            x += 1;
            while x < size && pixel(x) == color {
                x += 1;
            }
            if color[3] > 0 {
                rects.push(UiRect {
                    triangle: None,
                    segment: None,
                    depth: None,
                    rect: [
                        start as f32 / size as f32,
                        y as f32 / size as f32,
                        (x - start) as f32 / size as f32,
                        1. / size as f32,
                    ],
                    color: color.map(|v| v as f32 / 255.),
                });
            }
        }
    }
    rects
}
struct WakeOnDrop(Option<winit::event_loop::EventLoopProxy<()>>);
impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        if let Some(wake) = &self.0 {
            let _ = wake.send_event(());
        }
    }
}
impl MaterialPreviews {
    pub(super) fn set_waker(&mut self, wake: winit::event_loop::EventLoopProxy<()>) {
        self.wake = Some(wake);
    }
    #[cfg(test)]
    pub(super) fn busy(&self) -> bool {
        !self.pending.is_empty()
    }
    fn poll(&mut self) {
        if let Some(worker) = &self.worker {
            loop {
                match worker.rx.as_ref().unwrap().try_recv() {
                    Ok((key, preview)) => {
                        self.pending.remove(&key);
                        self.cache.insert(key, preview);
                        self.order.push_back(key);
                        while self.order.len() > CACHE_LIMIT {
                            if let Some(old) = self.order.pop_front() {
                                self.cache.remove(&old);
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.failed = true;
                        self.pending.clear();
                        break;
                    }
                }
            }
        }
    }
    pub(super) fn request(&mut self, material: &Material, textures: &[Texture]) {
        self.enqueue(fingerprint(material,textures), || material_content(material, textures));
    }
    pub(super) fn request_asset(&mut self, source: &super::asset_thumbnails::AssetImage, project: &Project) {
        self.enqueue(source.key(project), || Content::Asset(source.request(project)));
    }
    fn enqueue(&mut self, key: u64, content: impl FnOnce() -> Content) {
        if self.failed
            || self.cache.contains_key(&key)
            || self.pending.contains(&key)
            || self.pending.len() >= 4
        {
            return;
        }
        if self.worker.is_none() {
            let (tx, rx) = mpsc::sync_channel::<Request>(4);
            let (results, received) = mpsc::sync_channel(4);
            let wake = self.wake.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let cancelled = stop.clone();
            match std::thread::Builder::new()
                .name("material-previews".into())
                .stack_size(poseforge_render::RENDERER_STACK_BYTES)
                .spawn(move || {
                    let _wake_on_exit = WakeOnDrop(wake.clone());
                    if cancelled.load(Ordering::Relaxed) {
                        return;
                    }
                    let mut renderer = pollster::block_on(Renderer::headless(SIZE, SIZE));
                    if let Ok(renderer) = &mut renderer {
                        renderer.set_fx_cancel(cancelled.clone());
                    }
                    while let Ok(request) = rx.recv() {
                        if cancelled.load(Ordering::Relaxed) {
                            break;
                        }
                        let key = request.key;
                        let result = match &mut renderer {
                            Ok(renderer) => render_preview(renderer, request),
                            Err(error) => Err(error.clone()),
                        };
                        if results.send((key, result)).is_err() {
                            break;
                        }
                        if let Some(wake) = &wake {
                            let _ = wake.send_event(());
                        }
                    }
                }) {
                Ok(join) => {
                    self.worker = Some(Worker {
                        tx: Some(tx),
                        rx: Some(received),
                        stop,
                        join: Some(join),
                    })
                }
                Err(_) => {
                    self.failed = true;
                    return;
                }
            }
        }
        if self
            .worker
            .as_ref()
            .unwrap()
            .tx
            .as_ref()
            .unwrap()
            .try_send(Request {
                key,
                content: content(),
            })
            .is_ok()
        {
            self.pending.insert(key);
        }
    }
    pub(super) fn draw_asset(&self, u: &mut Ui, r: [f32;4], source: &super::asset_thumbnails::AssetImage, project: &Project) {
        match self.cache.get(&source.key(project)) {
            Some(Ok(runs)) => for run in runs { u.rect([r[0]+run.rect[0]*r[2],r[1]+run.rect[1]*r[3],run.rect[2]*r[2],run.rect[3]*r[3]],run.color); },
            result => {
                let p = u.palette(); u.rounded_rect(r,p.bg,6.);
                u.fitted_label([r[0]+4.,r[1]+r[3]*0.5-8.,r[2]-8.,20.],
                    if result.is_some() || self.failed { "Нет превью" } else { "Загрузка…" },10.,p.muted);
            }
        }
    }
    pub(super) fn draw(&self, u: &mut Ui, r: [f32; 4], material: &Material, textures: &[Texture]) {
        match self.cache.get(&fingerprint(material, textures)) {
            Some(Ok(runs)) => {
                for run in runs {
                    u.rect(
                        [
                            r[0] + run.rect[0] * r[2],
                            r[1] + run.rect[1] * r[3],
                            run.rect[2] * r[2],
                            run.rect[3] * r[3],
                        ],
                        run.color,
                    );
                }
            }
            result => {
                let p = u.palette();
                u.rounded_rect(r, p.elevated, r[2] * 0.5);
                u.label(
                    r[0] + r[2] * 0.5 - 4.,
                    r[1] + r[3] * 0.5 - 8.,
                    if result.is_some() || self.failed {
                        "!"
                    } else {
                        "…"
                    },
                    12.,
                    p.muted,
                );
            }
        }
    }
}
impl App {
    pub(super) fn material_previews_visible(&self) -> bool {
        self.surface.tab == 1
            && self.preferences.right_panel
            && (self.surface.material_library || self.surface_mesh_id().is_some())
            && !self.material_locked()
            && self
                .property_y(material_controls::MATERIAL_LIST_Y + 74.)
                .is_some()
    }
    pub(super) fn prepare_material_previews(&mut self) {
        self.material_previews.poll();
        if self.window.is_none() && std::env::var_os("ARTFORGE_QA_UI_GPU").is_none() {
            return;
        }
        self.prepare_asset_previews();
        let mut ids = if self.material_previews_visible() {
            self.visible_material_ids()
        } else {
            vec![]
        };
        ids.extend(self.material_popup_visible_ids());
        for index in self.material_popup_visible_presets() {
            if let Some(preset) = poseforge_core::MaterialPreset::ALL.get(index) {
                self.material_previews.request(&preset.create(), &[]);
            }
        }
        if self.preferences.left_panel {
            ids.extend(
                self.outliner_rows()
                    .iter()
                    .skip(self.scene_scroll)
                    .take(self.scene_capacity())
                    .filter_map(|row| {
                        if let super::scene_tree::SceneEntry::Material(_, material, _) = row {
                            *material
                        } else {
                            None
                        }
                    }),
            );
        }
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            if let Some(material) = self.editor.project.material(id) {
                self.material_previews
                    .request(material, &self.editor.project.textures);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn in_place_texture_edits_invalidate_preview_and_requests_share_source_storage() {
        let material = Material { base_color_texture:Some(1), ..Default::default() };
        let mut textures = vec![Texture { id:1,name:"Map".into(),width:1024,height:512,rgba8:vec![128;1024*512*4].into() }];
        let pointer = textures[0].rgba8.as_ptr();let before = fingerprint(&material, &textures);
        textures[0].rgba8[0] = 255;
        assert_eq!(textures[0].rgba8.as_ptr(), pointer);
        assert_ne!(fingerprint(&material, &textures), before);
        let Content::Material(_, request) = material_content(&material, &textures) else { panic!() };
        assert_eq!((request[0].width,request[0].height), (1024,512));
        assert!(request[0].rgba8.shares_storage(&textures[0].rgba8));
        let reduced = small_texture(&request[0]);
        assert_eq!((reduced.width,reduced.height), (256,128));assert_eq!(reduced.rgba8[0],255);
    }
    #[test]
    fn preview_key_tracks_material_texture_replacement_and_undo_snapshot() {
        let original = Material {
            id: 1,
            base_color_texture: Some(2),
            ..Material::default()
        };
        let texture = Texture {
            id: 2,
            name: "Map".into(),
            width: 1,
            height: 1,
            rgba8: vec![255, 0, 0, 255].into(),
        };
        let textures = vec![texture];
        let key = fingerprint(&original, &textures);
        let mut changed = original.clone();
        changed.roughness = 0.1;
        assert_ne!(key, fingerprint(&changed, &textures));
        changed = original.clone();
        changed.transmission = 0.9;
        assert_ne!(key, fingerprint(&changed, &textures));
        assert_eq!(key, fingerprint(&original.clone(), &textures));
        let mut replaced = textures.clone();
        replaced[0].rgba8 = vec![0, 0, 255, 255].into();
        assert_ne!(key, fingerprint(&original, &replaced));
    }
    #[test]
    fn preview_request_downsizes_only_referenced_maps_and_bounds_memory() {
        let t = Texture {
            id: 4,
            name: "Long map".into(),
            width: 1024,
            height: 512,
            rgba8: vec![128; 1024 * 512 * 4].into(),
        };
        let small = small_texture(&t);
        assert_eq!((small.width, small.height), (256, 128));
        assert_eq!(small.rgba8.len(), 256 * 128 * 4);
        assert_eq!(small.id, t.id);
        let p = preview_project(
            Material {
                id: 2,
                base_color_texture: Some(4),
                ..Material::default()
            },
            vec![small],
        );
        p.validate().unwrap();
        assert_eq!(p.meshes.len(), 2);
        assert!(p.materials[0].base_color_texture.is_some());
    }
    #[test]
    fn sphere_pixel_runs_are_circular_and_do_not_exceed_tile_bounds() {
        let pixels = vec![128; (SIZE * SIZE * 4) as usize];
        let runs = pixel_runs(&pixels);
        assert!(runs.len() < SIZE as usize * 5);
        for run in runs {
            let [x, y, w, h] = run.rect;
            assert!(x >= 0. && y >= 0. && x + w <= 1.0001 && y + h <= 1.0001);
            assert!(run.color[3] > 0.);
        }
    }
    #[test]
    #[ignore = "Requires a Vulkan GPU or software adapter"]
    fn gpu_spheres_distinguish_color_roughness_graphs_and_glass() {
        let mut renderer = pollster::block_on(Renderer::headless(SIZE, SIZE)).unwrap();
        let base = Material {
            id: 1,
            base_color: [0.7, 0.015, 0.01, 1.],
            roughness: 0.2,
            ..Material::default()
        };
        let render = |renderer: &mut Renderer, m: Material| {
            let p = preview_project(m, vec![]);
            p.validate().unwrap();
            let globals = p.nodes.iter().map(|n| (n.id, Mat4::IDENTITY)).collect();
            renderer.invalidate_assets();
            renderer
                .render_rgba(
                    &p,
                    &globals,
                    &View {
                        eye: Vec3::new(0., 0., 3.),
                        target: Vec3::ZERO,
                        orthographic: Some(0.5),
                        viewport: [0., 0., SIZE as f32, SIZE as f32],
                        ..View::default()
                    },
                    SIZE,
                    SIZE,
                )
                .unwrap()
        };
        let red = render(&mut renderer, base.clone());
        let mut blue = base.clone();
        blue.base_color = [0.01, 0.03, 0.7, 1.];
        let blue = render(&mut renderer, blue);
        let center = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
        assert!(red[center] > red[center + 2] + 40);
        assert!(blue[center + 2] > blue[center] + 40);
        let difference = |a: &[u8], b: &[u8]| {
            a.iter()
                .zip(b)
                .map(|(a, b)| a.abs_diff(*b) as u64)
                .sum::<u64>()
        };
        let mut rough = base.clone();
        rough.metallic = 1.;
        rough.roughness = 0.9;
        let mut smooth = rough.clone();
        smooth.roughness = 0.04;
        assert!(
            difference(
                &render(&mut renderer, rough),
                &render(&mut renderer, smooth)
            ) > 30000
        );
        let mut glass = base.clone();
        glass.base_color = [0.9, 0.97, 1., 1.];
        glass.transmission = 1.;
        glass.roughness = 0.05;
        assert!(difference(&red, &render(&mut renderer, glass)) > 100000);
        let mut graph = MaterialGraph::default();
        let mut color = MaterialNode::new(1, MaterialNodeKind::Color, [0., 0.]);
        color.value = [0.01, 0.7, 0.01, 1.];
        graph.nodes.push(color);
        graph.outputs[0] = Some(1);
        let mut node_material = base;
        node_material.graph = Some(graph);
        let green = render(&mut renderer, node_material);
        assert!(green[center + 1] > green[center] + 40);
    }
}

#[cfg(test)]
mod worker_shutdown_tests {
    use super::*;
    #[test]
    fn shutdown_unblocks_a_full_result_channel() {
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (results, rx) = mpsc::sync_channel(1);
        let (started, running) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let observed = stop.clone();
        let join = std::thread::spawn(move || {
            results.send((1, Ok(vec![]))).unwrap();
            started.send(()).unwrap();
            let _ = results.send((2, Ok(vec![])));
            assert!(observed.load(Ordering::Relaxed));
            drop(requests);
            finished.send(()).unwrap();
        });
        running.recv().unwrap();
        drop(Worker {
            tx: Some(tx),
            rx: Some(rx),
            stop,
            join: Some(join),
        });
        done.recv_timeout(Duration::from_secs(2)).unwrap();
    }

    #[test]
    fn dropping_preview_does_not_wait_for_a_busy_renderer() {
        let (tx, _requests) = mpsc::sync_channel::<Request>(1);
        let (_results, rx) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let observed = stop.clone();
        let join = std::thread::spawn(move || {
            wait.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(observed.load(Ordering::Relaxed));
            finished.send(()).unwrap();
        });
        drop(Worker { tx: Some(tx), rx: Some(rx), stop, join: Some(join) });
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
    }
}
