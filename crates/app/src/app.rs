#[cfg(test)]
#[path = "viewport33_tests.rs"]
mod viewport33_tests;
#[cfg(test)]
#[path = "performance_regression_tests.rs"]
mod performance_regression_tests;
#[path = "close_recovery.rs"]
mod close_recovery;
#[path = "node_editor.rs"]
mod node_editor;
#[path = "viewport_controls.rs"]
mod viewport_controls;
#[path = "world_controls.rs"]
mod world_controls;
use crate::icons::Icon;
use crate::ui::{self, Action, SliderRange, TextEdit, Ui};
#[path = "advanced_tools.rs"]
mod advanced_tools;
#[path = "animation_controls.rs"]
mod animation_controls;
#[cfg(test)]
#[path = "animation_interaction_tests.rs"]
mod animation_interaction_tests;
#[path = "camera_controls.rs"]
mod camera_controls;
#[path = "curve_controls.rs"]
mod curve_controls;
#[path = "curve_binding_controls.rs"]
mod curve_binding_controls;
#[path = "curve_binding_edit.rs"]
mod curve_binding_edit;
#[path = "force_preview_controls.rs"]
mod force_preview_controls;
#[path = "fx_controls.rs"]
mod fx_controls;
#[path = "fx_advanced_controls.rs"]
mod fx_advanced_controls;
#[path = "pyro_controls.rs"]
mod pyro_controls;
#[cfg(test)]
#[path = "curve_binding_tests.rs"]
mod curve_binding_tests;
#[cfg(test)]
#[path = "curve_translation_tests.rs"]
mod curve_translation_tests;
#[cfg(test)]
#[path = "runtime_polish_tests.rs"]
mod runtime_polish_tests;
#[cfg(test)]
#[path = "camera_object_tests.rs"]
mod camera_object_tests;
#[path = "color_controls.rs"]
mod color_controls;
#[cfg(test)]
#[path = "color_interaction_tests.rs"]
mod color_interaction_tests;
#[path = "controls.rs"]
mod controls;
#[path = "welcome.rs"]
mod welcome;
#[cfg(test)]
#[path = "controls_tests.rs"]
mod controls_tests;
#[path = "dropdown.rs"]
mod dropdown;
#[path = "environment_library.rs"]
mod environment_library;
#[path = "fluid_cache.rs"]
mod fluid_cache;
#[path = "fluid_domains.rs"]
mod fluid_domains;
pub(crate) use fluid_domains::ExportDomains;
#[path = "fluid_object_controls.rs"]
mod fluid_object_controls;
#[path = "destruction_controls.rs"]
mod destruction_controls;
#[path = "destruction_overlay.rs"]
mod destruction_overlay;
#[path = "destruction_materials.rs"]
mod destruction_materials;
#[path = "number_defaults.rs"]
mod number_defaults;
#[path = "fluid_preview.rs"]
mod fluid_preview;
#[path = "lens_controls.rs"]
mod lens_controls;
#[path = "camera_effect_controls.rs"]
mod camera_effect_controls;
#[path = "light_controls.rs"]
mod light_controls;
#[cfg(test)]
#[path = "light_interaction_tests.rs"]
mod light_interaction_tests;
#[path = "light_shape_controls.rs"]
mod light_shape_controls;
#[path = "lighting_tools.rs"]
mod lighting_tools;
#[path = "model_import.rs"]
mod model_import;
#[cfg(test)]
#[path = "model_import_tests.rs"]
mod model_import_tests;
#[path = "physics_bake.rs"]
mod physics_bake;
#[path = "physics_preview.rs"]
mod physics_preview;
#[path = "destruction_preview.rs"]
mod destruction_preview;
#[path = "scene_colliders.rs"]
mod scene_colliders;
#[path = "text_controls.rs"]
mod text_controls;
#[path = "empty_controls.rs"]
mod empty_controls;
#[path = "scene_fields.rs"]
mod scene_fields;
#[path = "collision_controls.rs"]
mod collision_controls;
#[path = "reports.rs"]
mod reports;
#[path = "scene_controls.rs"]
mod scene_controls;
#[cfg(test)]
#[path = "scene_light_tests.rs"]
mod scene_light_tests;
#[path = "scene_organization.rs"]
mod scene_organization;
#[cfg(test)]
#[path = "scene_organization_tests.rs"]
mod scene_organization_tests;
#[path = "scene_tools.rs"]
mod scene_tools;
#[path = "scene_tree.rs"]
mod scene_tree;
#[path = "studio_controls.rs"]
mod studio_controls;
#[path = "uv_editor.rs"]
mod uv_editor;
#[path = "weight_paint.rs"]
mod weight_paint;
#[cfg(test)]
#[path = "workflow06_tests.rs"]
mod workflow06_tests;
#[cfg(test)]
#[path = "workflow07_tests.rs"]
mod workflow07_tests;
use crate::workspace::{self, shortened, Preferences, SceneRow};
#[path = "animation_layers.rs"]
mod animation_layers;
#[path = "authoring_controls.rs"]
mod authoring_controls;
#[cfg(test)]
#[path = "authoring_tests.rs"]
mod authoring_tests;
#[path = "component_selection.rs"]
mod component_selection;
#[path = "face_overlay.rs"]
mod face_overlay;
#[path = "frame_preparation.rs"]
mod frame_preparation;
#[path = "renderer_startup.rs"]
mod renderer_startup;
#[path = "material_controls.rs"]
mod material_controls;
#[path = "material_users.rs"]
mod material_users;
#[path = "asset_library.rs"]
mod asset_library;
#[path = "asset_thumbnails.rs"]
mod asset_thumbnails;
#[cfg(test)]
#[path = "workspace44_tests.rs"]
mod workspace44_tests;
#[cfg(test)]
#[path = "workspace45_tests.rs"]
mod workspace45_tests;

#[cfg(test)]
#[path = "workspace42_tests.rs"]
mod workspace42_tests;
#[cfg(test)]
#[path = "material_interaction_tests.rs"]
mod material_interaction_tests;
#[path = "material_library.rs"]
mod material_library;
#[path = "material_preview.rs"]
mod material_preview;
#[path = "mesh_pick.rs"]
mod mesh_pick;
#[path = "material_media.rs"]
mod material_media;
#[path = "morph_controls.rs"]
mod morph_controls;
#[path = "video_preview.rs"]
mod video_preview;
#[path = "mesh_tools_controls.rs"]
mod mesh_tools_controls;
#[path = "modeling_controls.rs"]
mod modeling_controls;
#[path = "modeling_palette.rs"]
mod modeling_palette;
#[cfg(test)]
#[path = "workspace40_tests.rs"]
mod workspace40_tests;
#[path = "modeling_live.rs"]
mod modeling_live;
#[cfg(test)]
#[path = "modeling_tests.rs"]
mod modeling_tests;
#[cfg(test)]
#[path = "component_modeling_tests.rs"]
mod component_modeling_tests;
#[path = "npr_controls.rs"]
mod npr_controls;
#[path = "artistic_controls.rs"]
mod artistic_controls;
#[path = "performance.rs"]
mod performance;
#[path = "primitive_controls.rs"]
mod primitive_controls;
#[path = "program_settings.rs"]
mod program_settings;
#[path = "bone_display_controls.rs"]
mod bone_display_controls;
#[path = "color_swatches.rs"]
mod color_swatches;
#[path = "modifier_controls.rs"]
mod modifier_controls;
#[cfg(test)]
#[path = "modifier_controls_tests.rs"]
mod modifier_controls_tests;
#[cfg(test)]
#[path = "object_selection_tests.rs"]
mod object_selection_tests;
#[cfg(test)]
#[path = "bone_display_controls_tests.rs"]
mod bone_display_controls_tests;
#[cfg(test)]
#[path = "appearance_workflow_tests.rs"]
mod appearance_workflow_tests;
#[path = "properties.rs"]
mod properties;
#[cfg(test)]
#[path = "properties_tests.rs"]
mod properties_tests;
#[path = "property_sections.rs"]
mod property_sections;
#[path = "rig_pose_controls.rs"]
mod rig_pose_controls;
#[path = "rig_workflow.rs"]
mod rig_workflow;
#[path = "rig_references.rs"]
mod rig_references;
#[path = "surfaces.rs"]
mod surfaces;
#[path = "topology_controls.rs"]
mod topology_controls;
#[cfg(test)]
#[path = "workspace15_tests.rs"]
mod workspace15_tests;
#[cfg(test)]
#[path = "workspace16_tests.rs"]
mod workspace16_tests;
use glam::{EulerRot, Mat4, Quat, Vec3};
use poseforge_core::{
    BoneRole, DetectionReport, Editor, EditorMode, Project, RigProfile, StagedResolution, Transform,
};
use poseforge_render::{Renderer, View};
use poseforge_simulation::FluidWorld;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};

pub fn run(initial: Option<PathBuf>) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App::new(initial);
    if app.initial.is_none() {
        app.action(Action::New);
        app.status.clear();
    }
    app.material_previews.set_waker(event_loop.create_proxy());
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    if let Some(error)=app.startup_error.take() {Err(error)} else {Ok(())}
}

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Orbit,
    Move,
    Rotate,
    Scale,
}
#[derive(Clone)]
enum Drag {
    Swatch(color_swatches::SwatchDrag),
    CreationPanel { start: [f32; 2], origin: [f32; 2], original: Option<[f32; 2]>, moved: bool },
    Curve(curve_controls::CurveDrag),
    RigPole(rig_pose_controls::PoleDrag),
    Uv(uv_editor::UvDrag),
    UvPan(uv_editor::UvPan),
    ComponentSelection(component_selection::ComponentSelection),
    LiveParameter(modeling_live::ParameterDrag),
    Weights(weight_paint::WeightStroke),
    Group(scene_tools::GroupDrag),
    Panel(scene_tools::PanelDrag),
    Area(lighting_tools::AreaDrag),
    LightShape(light_shape_controls::LightShapeDrag),
    Material(material_library::MaterialDrag),
    Scene(scene_organization::SceneDrag),
    Node(node_editor::NodeDrag),
    NodePan(node_editor::NodePan),
    ObjectScroll {
        start: f32,
        value: usize,
    },
    AnimationLayersScroll {
        start: f32,
        value: usize,
    },
    MaterialScroll {
        start: f32,
        value: usize,
    },
    CameraNavigate(viewport_controls::CameraNavigation),
    ViewWidget {
        navigation: u8,
        target: Vec3,
        distance: f32,
        start: [f32; 2],
        yaw: f32,
        pitch: f32,
        saved_view: (f32, f32, Vec3, f32, bool, Option<f32>),
        recalled_view: Option<poseforge_core::Camera>,
        last_saved_view: Option<usize>,
        camera_view: Option<u64>,
    },
    NodeScroll {
        start: f32,
        value: f32,
    },
    Mesh(authoring_controls::MeshDrag),
    Scrollbar {
        start: f32,
        value: f32,
    },
    Orbit {
        start: [f32; 2],
        yaw: f32,
        pitch: f32,
    },
    Pan {
        start: [f32; 2],
        target: Vec3,
    },
    Gizmo {
        id: u64,
        gesture: crate::gizmo::GizmoDrag,
        curve: Option<Arc<poseforge_core::CurveTranslation>>,
        ik_chain: Option<[u64; 3]>,
        pole: Vec3,
    },
    Timeline,
    Keys(animation_controls::KeyDrag),
    KeyBox(animation_controls::KeyBoxDrag),
    Light(light_controls::LightDrag),
    Slider(controls::SliderDrag),
    Color(color_controls::ColorDrag),
}
#[derive(Clone, Copy, PartialEq)]
enum FilePurpose {
    CacheSave,
    CacheLoad,
    Environment,
    Import,
    Texture,
    Rig,
    Open,
    Save,
    Export,
    Sequence,
}
fn file_extension_allowed(purpose: FilePurpose, path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match purpose {
        FilePurpose::CacheSave | FilePurpose::CacheLoad => extension == "afcache",
        FilePurpose::Environment => matches!(extension.as_str(), "hdr" | "png" | "jpg" | "jpeg"),
        FilePurpose::Import => matches!(extension.as_str(), "glb" | "gltf" | "obj"),
        FilePurpose::Texture => matches!(
            extension.as_str(),
            "png" | "jpg" | "jpeg" | "tga" | "bmp" | "dds" | "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" | "ogv" | "gif"
        ),
        FilePurpose::Rig => extension == "txt",
        FilePurpose::Open => matches!(extension.as_str(), "artforge" | "poseforge" | "json"),
        FilePurpose::Save => matches!(extension.as_str(), "artforge" | "poseforge"),
        FilePurpose::Export => extension == "png",
        FilePurpose::Sequence => false,
    }
}

fn import_progress_label(progress: f32, text: &str) -> String {
    // Blender cannot report a fraction while it opens/evaluates/exports a scene.
    // Preserve its real phase and elapsed time instead of showing a stuck 10%.
    if progress.is_finite() && progress >= 0. {
        format!("Импорт {:.0}% · {}", progress.clamp(0., 1.) * 100., text)
    } else {
        format!("Импорт · {text}")
    }
}

fn import_focus_action(action: &Action) -> bool {
    matches!(
        action,
        Action::EditPath
            | Action::PastePath
            | Action::BrowsePath
            | Action::ParentDir
            | Action::EditBlenderPath
            | Action::PasteBlenderPath
            | Action::BrowseBlenderPath
            | Action::DetectBlender
            | Action::ToggleBlendImportAnimations
            | Action::FileEntry(_)
            | Action::Cancel
            | Action::FileSubmit
    )
}

fn assign_loaded_texture(
    project: &mut Project,
    mut texture: poseforge_core::Texture,
    material: u64,
    slot: usize,
) -> bool {
    if slot >= 5 || project.material(material).is_none() {
        return false;
    }
    let id = project.next_id();
    texture.id = id;
    project.textures.push(texture);
    let material = project
        .materials
        .iter_mut()
        .find(|m| m.id == material)
        .unwrap();
    match slot {
        0 => material.base_color_texture = Some(id),
        1 => material.metallic_roughness_texture = Some(id),
        2 => material.normal_texture = Some(id),
        3 => material.occlusion_texture = Some(id),
        4 => material.emissive_texture = Some(id),
        _ => unreachable!(),
    }
    true
}
fn wrapped_report(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            for fragment in word.chars().collect::<Vec<_>>().chunks(88) {
                let fragment: String = fragment.iter().collect();
                if line.chars().count() + fragment.chars().count() + 1 > 90 {
                    lines.push(std::mem::take(&mut line));
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(&fragment);
            }
        }
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push("Нет данных.".into());
    }
    lines
}
struct FileDialog {
    purpose: FilePurpose,
    path: String,
    entries: Vec<(PathBuf, bool)>,
    offset: usize,
}
impl FileDialog {
    fn new(purpose: FilePurpose, path: PathBuf) -> Self {
        let mut s = Self {
            purpose,
            path: path.to_string_lossy().into(),
            entries: vec![],
            offset: 0,
        };
        s.refresh();
        s
    }
    fn directory(&self) -> PathBuf {
        let p = PathBuf::from(&self.path);
        if p.is_dir() {
            p
        } else {
            p.parent().unwrap_or(Path::new(".")).to_path_buf()
        }
    }
    fn refresh(&mut self) {
        self.entries = std::fs::read_dir(self.directory())
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| (e.path(), e.path().is_dir()))
            .filter(|(p, d)| *d || file_extension_allowed(self.purpose, p))
            .collect();
        self.entries.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.file_name().cmp(&b.0.file_name()))
        });
        self.offset = 0;
    }
}
enum JobMessage {
    LibrarySceneReady(Box<poseforge_core::PreparedProjectEdit>, Vec<u64>, String),
    FluidCacheReady(Option<fluid_cache::FluidCache>, u64),
    UvReady(Box<Project>, Box<Project>),
    MeshToolReady(Box<Project>, Box<Project>, u64, Option<BTreeSet<usize>>),
    PhysicsBaked(Box<Project>, Box<Project>),
    FluidAttached(Box<Project>, Box<Project>, u64, Box<fluid_domains::PreparedDomain>),
    DestructionReady(Box<Project>, Box<Project>, u64, u64, bool, bool),
    EnvironmentLoaded(poseforge_core::EnvironmentMap),
    Bound(Box<Project>, Box<Project>, u64, u64),
    Progress(String),
    Imported(Box<model_import::PreparedImport>),
    TextureLoaded(poseforge_core::Texture, u64, usize),
    VideoLoaded(poseforge_core::Texture, poseforge_core::VideoTexture, u64, video_preview::Target),
    NodeImageLoaded(poseforge_core::Texture, u64, u32),
    Loaded(Project, PathBuf),
    Saved(PathBuf, Project),
    Autosaved(PathBuf),
    Surface(poseforge_simulation::SurfaceMesh),
    Exported(usize, PathBuf),
    Error(String),
}
struct Job {
    receiver: Receiver<JobMessage>,
    cancel: Arc<AtomicBool>,
    label: String,
}
impl Drop for Job {
    fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}

struct App {
    welcome: welcome::Welcome,
    procedural: modifier_controls::State,
    nodes: node_editor::NodeEditor,
    uv: uv_editor::UvState,
    authoring: authoring_controls::AuthoringState,
    modeling_live: Option<modeling_live::LiveOperation>,
    primitive_panel: Option<u64>,
    curves: curve_controls::CurveState,
    morphs: morph_controls::MorphState,
    assets: asset_library::AssetLibrary,
    primitive_collapsed: bool,
    bone_display_collapsed: bool,
    weights: weight_paint::WeightPaint,
    window: Option<Arc<Window>>,
    // Stop the thumbnail renderer before dropping the window renderer.
    material_previews: material_preview::MaterialPreviews,
    renderer: Option<Renderer>,
    renderer_startup: Option<renderer_startup::Startup<Renderer>>,
    startup_error: Option<String>,
    video_preview: video_preview::VideoPreview,
    fx_preview: fx_controls::Preview,
    editor: Editor,
    initial: Option<PathBuf>,
    width: f32,
    height: f32,
    scale: f32,
    ui: Ui,
    mouse: [f32; 2],
    slider_cursor_locked: bool,
    modifiers: ModifiersState,
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
    orthographic: bool,
    environments: environment_library::EnvironmentLibrary,
    scene_tools: scene_tools::SceneTools,
    camera_view: Option<u64>,
    last_light_target: Option<u64>,
    light_aim: Option<(usize, poseforge_core::Light, bool)>,
    solo_light: Option<usize>,
    recalled_view: Option<poseforge_core::Camera>,
    last_saved_view: Option<usize>,
    focus_pick: Option<u64>,
    camera_effects: camera_effect_controls::State,
    path_pick: Option<curve_binding_controls::BindingTarget>,
    path_edit: Option<curve_binding_edit::EditTarget>,
    camera_navigation_lock: bool,
    studio: studio_controls::StudioState,
    destruction: destruction_controls::State,
    collisions: collision_controls::State,
    free_view_fov: Option<f32>,
    tool: Tool,
    axis: usize,
    gizmo_local: bool,
    ik: bool,
    rig_pose: rig_pose_controls::RigPoseState,
    mesh_tools: mesh_tools_controls::MeshToolsState,
    drag: Option<Drag>,
    drag_button: Option<MouseButton>,
    orbit_click: Option<[f32; 2]>,
    slider_click: Option<controls::SliderClick>,
    view_axis_press: Option<(u8, [f32; 2])>,
    input: Option<TextEdit>,
    color_picker: Option<color_controls::ColorSession>,
    swatches: color_swatches::Swatches,
    text_selecting: bool,
    native_picker: Option<controls::NativePicker>,
    clipboard: Option<arboard::Clipboard>,
    dialog: Option<FileDialog>,
    import_focus: Option<Action>,
    texture_target: Option<(u64, usize)>,
    node_texture_target: Option<(u64, u32)>,
    overwrite: Option<(PathBuf, FilePurpose)>,
    path: Option<PathBuf>,
    status: String,
    job: Option<Job>,
    last_tick: Instant,
    scene_scroll: usize,
    preferences: Preferences,
    scene_query: String,
    joints_only: bool,
    collapsed: BTreeSet<u64>,
    inspector_scroll: f32,
    surface: surfaces::SurfaceState,
    pose_scroll: usize,
    rig_manifest: Option<poseforge_io::RigManifest>,
    rig_details: bool,
    rig_scroll: usize,
    character_panel: bool,
    animation_panel: bool,
    timeline: crate::animation_timeline::TimelineState,
    animation: animation_controls::AnimationSession,
    animation_overlay: crate::animation_overlay::AnimationOverlay,
    menu: Option<crate::chrome::Menu>,
    dropdown: Option<dropdown::Dropdown>,
    dropdown_focus: Option<ui::DropdownKind>,
    rig_draft: RigProfile,
    rig_draft_active: bool,
    detection: Option<DetectionReport>,
    rig_picker: Option<Option<usize>>,
    bone_query: String,
    bone_scroll: usize,
    clip_scroll: usize,
    active_rig_root: Option<u64>,
    copied_pose: Option<BTreeMap<u64, Transform>>,
    fluid: FluidWorld,
    fluid_applied: poseforge_core::FluidSettings,
    fluid_preview: fluid_preview::FluidPreview,
    fluid_on: bool,
    fluid_domains: fluid_domains::FluidDomains,
    help: bool,
    poses: bool,
    details: bool,
    details_text: String,
    details_page: usize,
    report_copy_feedback: Option<reports::ReportCopyFeedback>,
    last_import_report: String,
    pending_time: Option<f64>,
    dirty: bool,
    saved_snapshot: Project,
    last_autosave: Instant,
    performance: performance::Performance,
    recovery_id: String,
    close_recovery: close_recovery::CloseRecovery,
    smoke_frames: Option<u32>,
}
/// Shift and clip one panel's primitives and logical hit boxes together.
fn clip_scrolled(u: &mut Ui, start: (usize, usize, usize), bounds: [f32; 4], scroll: f32) {
    let physical = bounds.map(|v| v * u.scale);
    let mut rects = u.rects.split_off(start.0);
    for rect in &mut rects {
        rect.rect[1] -= scroll * u.scale;
        if let Some(points) = &mut rect.triangle {
            for point in points {
                point[1] -= scroll * u.scale;
            }
        }
        if let Some(line) = &mut rect.segment {
            line[1] -= scroll * u.scale;
            line[3] -= scroll * u.scale;
        }
    }
    for mut rect in rects {
        if let Some(r) = intersect_rect(rect.rect, physical) {
            rect.rect = r;
            u.rects.push(rect);
        }
    }
    let texts = u.texts.split_off(start.1);
    for mut text in texts {
        text.y -= scroll * u.scale;
        if text.y >= physical[1] && text.y + text.size * 1.25 <= physical[1] + physical[3] {
            u.texts.push(text);
        }
    }
    let hits = u.hits.split_off(start.2);
    for mut hit in hits {
        hit.rect[1] -= scroll;
        if let Some(r) = intersect_rect(hit.rect, bounds) {
            hit.rect = r;
            u.hits.push(hit);
        }
    }
}
fn intersect_rect(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    let right = (a[0] + a[2]).min(b[0] + b[2]);
    let bottom = (a[1] + a[3]).min(b[1] + b[3]);
    (right > x && bottom > y).then_some([x, y, right - x, bottom - y])
}
impl App {
    fn new(initial: Option<PathBuf>) -> Self {
        let p = Project::demo();
        let mut editor = Editor::new(p.clone());
        editor.mode = EditorMode::Scene;
        editor.selected = p.nodes.iter().find(|n| n.is_joint).map(|n| n.id);
        let saved_snapshot = editor.project.clone();
        Self {
            welcome: Default::default(),
            close_recovery: Default::default(),
            material_previews: material_preview::MaterialPreviews::default(),
            nodes: node_editor::NodeEditor::default(),
            uv: Default::default(),
            authoring: authoring_controls::AuthoringState::default(),
            modeling_live: None,
            primitive_panel: None,
            curves: Default::default(),
            morphs: Default::default(),
            assets: Default::default(),
            primitive_collapsed: false,
            bone_display_collapsed: false,
            weights: weight_paint::WeightPaint::default(),
            window: None,
            renderer: None,
            renderer_startup: None,
            startup_error: None,
            video_preview: Default::default(),
            fx_preview: Default::default(),
            editor,
            initial,
            width: 1440.,
            height: 900.,
            scale: 1.,
            ui: Ui::new(1.),
            mouse: [0.; 2],
            slider_cursor_locked: false,
            modifiers: ModifiersState::empty(),
            yaw: 0.48,
            pitch: 0.21,
            distance: 5.4,
            target: Vec3::new(0., 1., 0.),
            orthographic: false,
            environments: Default::default(),
            scene_tools: Default::default(),
            procedural: Default::default(),
            camera_view: None,
            last_light_target: None,
            light_aim: None,
            solo_light: None,
            recalled_view: None,
            last_saved_view: None,
            focus_pick: None,
            camera_effects: Default::default(),
            path_pick: None,
            path_edit: None,
            camera_navigation_lock: false,
            studio: studio_controls::StudioState::default(),
            destruction: Default::default(),
            collisions: Default::default(),
            free_view_fov: None,
            tool: Tool::Rotate,
            axis: 2,
            gizmo_local: false,
            ik: false,
            rig_pose: Default::default(),
            mesh_tools: Default::default(),
            drag: None,
            drag_button: None,
            orbit_click: None,
            slider_click: None,
            view_axis_press: None,
            input: None,
            color_picker: None,
            swatches: Default::default(),
            text_selecting: false,
            native_picker: None,
            clipboard: arboard::Clipboard::new().ok(),
            dialog: None,
            import_focus: None,
            texture_target: None,
            node_texture_target: None,
            overwrite: None,
            path: None,
            status: "Демонстрационная сцена. Выберите сустав и измените позу.".into(),
            job: None,
            last_tick: Instant::now(),
            scene_scroll: 0,
            preferences: if cfg!(test) {
                Preferences::default()
            } else {
                Preferences::load()
            },
            scene_query: String::new(),
            joints_only: false,
            collapsed: BTreeSet::new(),
            inspector_scroll: 0.,
            surface: surfaces::SurfaceState::default(),
            pose_scroll: 0,
            rig_manifest: None,
            rig_details: false,
            rig_scroll: 0,
            character_panel: false,
            animation_panel: false,
            timeline: crate::animation_timeline::TimelineState::default(),
            animation: animation_controls::AnimationSession::default(),
            animation_overlay: crate::animation_overlay::AnimationOverlay::default(),
            menu: None,
            dropdown: None,
            dropdown_focus: None,
            rig_draft: RigProfile {
                root: None,
                assignments: BTreeMap::new(),
            },
            rig_draft_active: false,
            detection: None,
            rig_picker: None,
            bone_query: String::new(),
            bone_scroll: 0,
            clip_scroll: 0,
            active_rig_root: None,
            copied_pose: None,
            fluid: FluidWorld::demo(),
            fluid_applied: poseforge_core::FluidSettings::default(),
            fluid_preview: fluid_preview::FluidPreview::default(),
            fluid_on: false,
            fluid_domains: Default::default(),
            help: false,
            poses: false,
            details: false,
            details_text: String::new(),
            details_page: 0,
            report_copy_feedback: None,
            last_import_report: "Модель ещё не импортирована. Проект → Импорт модели.".into(),
            pending_time: None,
            dirty: false,
            saved_snapshot,
            last_autosave: Instant::now(),
            performance: performance::Performance::default(),
            recovery_id: workspace::new_recovery_id(),
            smoke_frames: std::env::var("POSEFORGE_SMOKE_FRAMES")
                .ok()
                .and_then(|s| s.parse().ok()),
        }
    }
    fn left_width(&self) -> f32 {
        self.preferences
            .left_width
            .unwrap_or(if self.width < 1320. { 256. } else { 280. })
            .min((self.width * 0.4).max(220.))
    }
    fn right_width(&self) -> f32 {
        self.preferences
            .right_width
            .unwrap_or(if self.width < 1320. { 304. } else { 336. })
            .min((self.width - self.left_width() - 180.).max(280.))
    }
    fn scene_panel_visible(&self) -> bool {
        self.preferences.left_panel && !self.uv.open
    }
    fn viewport(&self) -> [f32; 4] {
        if self.uv.open {
            let r = self.uv_rect();
            let x = r[0] + r[2] + 8.;
            return [
                x,
                72.,
                (self.navigation_right() - x).max(80.),
                (self.panel_bottom() - 84.).max(80.),
            ];
        }
        let bottom = if self.nodes_open() {
            self.height - self.node_editor_rect()[1] + 8.
        } else {
            36.
        };
        if self.camera_view.is_some() {
            let left = self.left_panel_rect();
            let x = if self.preferences.left_panel {
                left[0] + left[2] + 12.
            } else {
                12.
            };
            let right = if self.preferences.right_panel {
                self.right_panel_rect()[0] - 12.
            } else {
                self.width - 12.
            };
            let bottom = (self.height - bottom).min(self.panel_bottom() - 12.);
            return [x, 72., (right - x).max(1.), (bottom - 72.).max(1.)];
        }
        [
            0.,
            60.,
            self.width.max(120.),
            (self.height - 60. - bottom).max(100.),
        ]
    }
    fn left_panel_rect(&self) -> [f32; 4] {
        if !self.scene_panel_visible() {
            return [12., 72., 0., 0.];
        }
        let y = 72.;
        let available = (self.panel_bottom() - y).max(80.);
        let height = if self.preferences.left_panel {
            available.clamp(120., 338.)
        } else {
            38.
        };
        [12., y, self.left_width(), height]
    }
    fn right_panel_rect(&self) -> [f32; 4] {
        [
            self.width - self.right_width() - 12.,
            64.,
            self.right_width(),
            self.panel_bottom() - 64.,
        ]
    }
    fn rail_rect(&self) -> [f32; 4] {
        if !self.scene_panel_visible() {
            return [12., 72., 0., 0.];
        }
        [20. + self.left_width(), 72., 44., self.tools_height()]
    }
    fn canvas_input_at(&self, point: [f32; 2]) -> bool {
        ui::contains(self.viewport(), point)
            && !ui::contains(self.left_panel_rect(), point)
            && !(self.preferences.right_panel && ui::contains(self.right_panel_rect(), point))
            && !ui::contains(self.creation_toolbar_rect(), point)
            && !ui::contains(self.rail_rect(), point)
            && !(self.animation_panel_visible() && ui::contains(self.animation_dock_rect(), point))
            && !self
                .primitive_panel_rect()
                .is_some_and(|r| ui::contains(r, point))
            && self.ui.hit(point).is_none()
    }
    fn scene_rows(&self) -> Vec<SceneRow> {
        workspace::scene_rows_filtered(
            &self.editor.project,
            &self.scene_query,
            self.joints_only,
            &self.collapsed,
            self.scene_tools.hidden_types,
        )
    }
    fn scene_capacity(&self) -> usize {
        ((self.left_panel_rect()[3] - 80.) / 27.).max(1.) as usize
    }
    fn inspector_content_bottom(&self) -> f32 {
        if let Some(bottom) = self.path_panel_bottom() { return bottom; }
        match self.surface.tab {
            8 => self.modifier_panel_bottom(),
            12 => 784.,
            13 => if self.destruction_source().and_then(|id| self.editor.project.node(id)).is_some_and(|n| n.destruction.is_some()) { destruction_controls::PANEL_BOTTOM + self.collision_layout_shift(poseforge_core::CollisionKind::Destruction) } else { 370. },
            4 => if self.selected_fluid().is_some() { fluid_object_controls::PANEL_BOTTOM + 44. + self.collision_layout_shift(poseforge_core::CollisionKind::Fluid) } else { 350. },
            6 if self.selected_fluid().is_some() => fluid_object_controls::PANEL_BOTTOM + 44. + self.collision_layout_shift(poseforge_core::CollisionKind::Fluid),
            6 => self.physics_panel_bottom().max(300.) + 44.,
            11 => if self.editor.selected.and_then(|id| self.editor.project.node(id))
                .and_then(|n| n.physics).is_some_and(|p| p.kind == poseforge_core::PhysicsKind::Cloth)
                { self.physics_panel_bottom() + 44. } else { 350. },
            7 => self.modeling_panel_bottom(),
            0 if self.selected_text().is_some() => text_controls::PANEL_BOTTOM,
            0 if self.selected_helper().is_some() => self.helper_panel_bottom(),
            0 if self.rig_context() => self.rig_object_bottom(),
            9 => 430.,
            10 => self.normals_panel_bottom(),
            1 if !self.surface.material_library && self.surface_mesh_id().is_none()
                || self.material_locked() =>
            {
                300.
            }
            1 if self.selected_material_id().is_none() => {
                material_controls::MATERIAL_LIST_Y + self.material_list_height() + 60.
            }
            1 => self.material_panel_bottom(),
            2 => 1040.,
            3 => scene_controls::RENDER_PANEL_BOTTOM,
            _ => {
                if self.active_light_index().is_some() {
                    1690.
                } else if self.selected_camera_id().is_some() {
                    self.camera_panel_bottom()
                } else if let Some((_, curve)) = self.selected_curve() {
                    curve.geometry.map_or(908., |g| if g.profile == poseforge_core::CurveProfile::Tube { 1130. } else { 1038. })
                } else if self.editor.selected.and_then(|id| self.editor.project.node(id)).is_some_and(|n| n.mesh.is_some()) { 602. }
                else { 526. }
            }
        }
    }
    fn inspector_max_scroll(&self) -> f32 {
        (self.collapsed_content_bottom() - properties::CONTENT_SHIFT - (self.panel_bottom() - 8.))
            .max(0.)
    }
    fn persist_preferences(&mut self) {
        if cfg!(test) {
            return;
        }
        if let Err(e) = self.preferences.save() {
            self.status = format!("Настройки не сохранены: {e}");
        }
    }
    fn update_dirty(&mut self) {
        self.performance.changed();
        self.dirty = self.editor.project != self.saved_snapshot || self.editor.has_staged();
    }
    fn has_modal(&self) -> bool {
        self.welcome.open || self.collision_popup_open() || self.environments.open
            || self.nodes_open()
            || self.dialog.is_some()
            || self.overwrite.is_some()
            || self.help
            || self.poses
            || self.details
            || self.rig_details
            || self.character_panel
            || self.animation_panel
            || self.pending_time.is_some()
            || self.menu.is_some()
            || self.dropdown.is_some()
            || self.color_picker.is_some()
    }
    fn current_profile(&self) -> Option<&RigProfile> {
        self.editor
            .project
            .rig_profiles
            .iter()
            .find(|profile| profile.root == self.active_rig_root)
    }
    fn is_descendant(&self, id: u64, root: u64) -> bool {
        let mut cursor = Some(id);
        let mut visited = BTreeSet::new();
        while let Some(id) = cursor {
            if id == root {
                return true;
            }
            if !visited.insert(id) {
                return false;
            }
            cursor = self.editor.project.node(id).and_then(|n| n.parent);
        }
        false
    }
    fn saved_root_for(&self, id: u64) -> Option<u64> {
        if let Some(root) = self.current_profile().and_then(|profile| profile.root) {
            if self.is_descendant(id, root) {
                return Some(root);
            }
        }
        let mut cursor = Some(id);
        let mut seen = BTreeSet::new();
        while let Some(node) = cursor {
            if !seen.insert(node) {
                break;
            }
            if self
                .editor
                .project
                .rig_profiles
                .iter()
                .any(|p| p.root == Some(node))
            {
                return Some(node);
            }
            cursor = self.editor.project.node(node).and_then(|n| n.parent);
        }
        None
    }
    fn sync_active_character(&mut self, id: u64) {
        // A manually chosen armature/container may sit above the detected
        // skeleton root. Keep that saved scope when one of its bones is picked.
        let root = self.saved_root_for(id).or_else(|| {
            poseforge_core::character_roots(&self.editor.project)
                .into_iter()
                .find(|root| self.is_descendant(id, *root))
        });
        if root.is_some() {
            self.active_rig_root = root;
        }
    }
    fn character_scope(&self) -> Result<Vec<u64>, String> {
        let profile = self
            .current_profile()
            .ok_or("Сначала сохраните соответствие костей в свойствах скелета.")?;
        let root = profile.root.ok_or("Выберите корень персонажа.")?;
        let nodes: Vec<_> = self
            .editor
            .project
            .nodes
            .iter()
            .filter(|n| n.is_joint && self.is_descendant(n.id, root))
            .map(|n| n.id)
            .collect();
        if nodes.is_empty() {
            return Err("В выбранном персонаже нет суставов.".into());
        }
        Ok(nodes)
    }
    fn prepare_character(&mut self) {
        let roots = poseforge_core::character_roots(&self.editor.project);
        let selected_root = self.editor.selected.and_then(|id| {
            self.rig_draft
                .root
                .filter(|root| self.rig_draft_active && self.is_descendant(id, *root))
                .or_else(|| self.saved_root_for(id))
                .or_else(|| {
                    roots
                        .iter()
                        .copied()
                        .find(|root| self.is_descendant(id, *root))
                })
        });
        let root = selected_root
            .or(self
                .active_rig_root
                .filter(|id| self.editor.project.node(*id).is_some()))
            .or_else(|| (roots.len() == 1).then(|| roots[0]));
        self.active_rig_root = root;
        let detected = poseforge_core::detect_humanoid(&self.editor.project, root);
        if !self.rig_draft_active || self.rig_draft.root != root {
            self.rig_draft = self
                .editor
                .project
                .rig_profiles
                .iter()
                .find(|p| p.root == root)
                .cloned()
                .unwrap_or_else(|| detected.resolved.clone());
        }
        self.rig_draft_active = true;
        self.detection = Some(detected);
        self.rig_picker = None;
        self.bone_query.clear();
        self.bone_scroll = 0;
        self.character_panel = true;
        self.animation_panel = false;
        self.editor.playing = false;
    }
    fn select_role(&mut self, index: usize, ik: bool) {
        self.sync_rig_pose();
        self.editor.playing = false;
        let Some(role) = BoneRole::ALL.get(index).copied() else {
            return;
        };
        let profile = if self.character_panel {
            Some(self.rig_draft.clone())
        } else {
            self.pose_profile().cloned()
        };
        let Some(profile) = profile else {
            self.status = "Сначала сохраните профиль персонажа.".into();
            return;
        };
        let Some(id) = profile.assignments.get(&role).copied() else {
            self.status = "Назначьте кость этой части тела.".into();
            return;
        };
        if !self.node_selectable(id) {
            self.status = "Кость скрыта, заблокирована или удалена.".into();
            return;
        }
        if ik {
            if let Err(e) = profile.chain(role, &self.editor.project) {
                self.status = e.to_string();
                return;
            }
            if profile
                .chain(role, &self.editor.project)
                .is_ok_and(|chain| chain.iter().any(|id| !self.node_selectable(*id)))
            {
                self.status = "Цепь IK содержит скрытую или заблокированную кость.".into();
                return;
            }
        }
        self.scene_tools.selected.clear();
        self.editor.selected = Some(id);
        self.active_rig_root = profile.root;
        self.reveal_scene_node(id);
        if self.editor.mode == EditorMode::Render {
            self.editor.mode = if self.editor.render_animated {
                EditorMode::Animation
            } else {
                EditorMode::Pose
            };
        } else if self.editor.mode == EditorMode::Scene {
            self.editor.mode = EditorMode::Pose;
        }
        self.ik = ik;
        self.tool = if ik { Tool::Move } else { Tool::Rotate };
        self.character_panel = false;
        self.animation_panel = false;
        self.preferences.skeleton = true;
        self.status = format!(
            "{} · {} · потяните {} гизмо",
            role.label(),
            if ik { "IK" } else { "FK" },
            if ik {
                "стрелку или центр"
            } else {
                "кольцо"
            }
        );
    }
    fn pose_chain(&self, id: u64) -> Result<[u64; 3], String> {
        if let Some(profile) = self.pose_profile() {
            if let Some((role, _)) = profile.assignments.iter().find(|(_, node)| **node == id) {
                let chain = profile
                    .chain(*role, &self.editor.project)
                    .map_err(|e| e.to_string())?;
                if chain.iter().any(|id| !self.node_selectable(*id)) {
                    return Err("Цепь IK содержит скрытую или заблокированную кость.".into());
                }
                return Ok(chain);
            }
        }
        let middle = self
            .editor
            .project
            .node(id)
            .and_then(|n| n.parent)
            .ok_or("Для IK выберите конечную кость цепи.")?;
        let root = self
            .editor
            .project
            .node(middle)
            .and_then(|n| n.parent)
            .ok_or("В цепи IK должно быть три сустава.")?;
        if [root, middle, id].iter().any(|id| {
            self.editor
                .project
                .node(*id)
                .is_none_or(|n| !n.is_joint || !self.node_selectable(*id))
        }) {
            return Err("IK требует три незаблокированных сустава.".into());
        }
        Ok([root, middle, id])
    }
    fn selected_gizmo(&self) -> Option<crate::gizmo::Gizmo> {
        if self.object_selection().iter().any(|id| self.preview_shard_or_child(*id)) { return None; }
        if self.object_selection().len() > 1 && self.group_transform_mode() {
            return self.group_gizmo();
        }
        if self.authoring.editing.is_some() {
            return None;
        }
        if self.editor.mode == EditorMode::Render || self.tool == Tool::Orbit || self.has_modal() {
            return None;
        }
        let id = self.editor.selected?;
        if self.destruction.preview.affects(id) || !self.node_selectable(id) {
            return None;
        }
        let mut cursor = Some(id);
        let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return None;
            }
            let node = self.editor.project.node(id)?;
            if node.locked || !node.visible {
                return None;
            }
            cursor = node.parent;
        }
        let evaluated = self.evaluated_interactive().ok()?;
        let parent = self.editor.project.transform_frame(id, &evaluated.global, self.editor.time).ok()?;
        if self.tool == Tool::Rotate && crate::gizmo::uniform_parent_rotation(parent).is_none() {
            return None;
        }
        let mode = match self.tool {
            Tool::Move => crate::gizmo::Mode::Move,
            Tool::Rotate => crate::gizmo::Mode::Rotate,
            Tool::Scale => crate::gizmo::Mode::Scale,
            Tool::Orbit => return None,
        };
        crate::gizmo::Gizmo::new(
            mode,
            *evaluated.global.get(&id)?,
            self.gizmo_local,
            self.view(),
            self.viewport(),
        )
    }
    fn begin_gizmo_drag(&mut self, handle: crate::gizmo::Handle) {
        if self.begin_group_drag(handle) {
            return;
        }
        let Some(id) = self.editor.selected else {
            return;
        };
        let Some(gizmo) = self.selected_gizmo() else {
            self.status = "Гизмо недоступно: проверьте блокировку и масштаб родителя. Числовые поля — в «Свойствах».".into();
            return;
        };
        let Ok(evaluated) = self.evaluated_interactive() else {
            return;
        };
        let Some(transform) = evaluated.local.get(&id).copied() else {
            return;
        };
        let Ok(parent) = self.editor.project.transform_frame(id, &evaluated.global, self.editor.time) else { return; };
        let world = gizmo.origin;
        let curve = if self.tool == Tool::Move {
            match self.object_curve_translation(id, &evaluated) {
                Ok(path) => path, Err(e) => { self.status = e.to_string(); return; }
            }
        } else { None };
        let Some(gesture) =
            crate::gizmo::GizmoDrag::begin(gizmo, handle, transform, parent, self.mouse)
        else {
            self.status =
                "Эта ось направлена в камеру. Измените ракурс или используйте числовые поля."
                    .into();
            return;
        };
        self.editor.playing = false;
        let mut chain = None;
        let mut pole = world + Vec3::Z;
        if self.ik
            && self.tool == Tool::Move
            && self.editor.project.node(id).is_some_and(|n| n.is_joint)
        {
            match self.pose_chain(id) {
                Ok(ids) => {
                    if ids
                        .iter()
                        .any(|id| self.editor.project.node(*id).is_none_or(|n| n.locked))
                    {
                        self.status = "Разблокируйте всю цепь IK перед позированием.".into();
                        return;
                    }
                    if let Ok(evaluated) = self.evaluated_interactive() {
                        let positions: Option<Vec<_>> = ids
                            .iter()
                            .map(|id| evaluated.global.get(id).map(|m| m.w_axis.truncate()))
                            .collect();
                        if let Some(p) = positions {
                            let axis = (p[2] - p[0]).try_normalize().unwrap_or(Vec3::Y);
                            let side = (p[1] - p[0]) - axis * (p[1] - p[0]).dot(axis);
                            let bend = side
                                .try_normalize()
                                .unwrap_or_else(|| axis.any_orthonormal_vector());
                            pole = p[1] + bend * (p[2] - p[0]).length().max(0.1);
                        }
                    }
                    chain = Some(ids);
                }
                Err(e) => {
                    self.status = e;
                    return;
                }
            }
        }
        if let crate::gizmo::Handle::Axis(axis) = handle {
            self.axis = axis;
        }
        self.editor.begin_edit(match self.tool {
            Tool::Move => "Перемещение",
            Tool::Rotate => "Поворот",
            Tool::Scale => "Масштаб",
            Tool::Orbit => "Трансформация",
        });
        self.drag = Some(Drag::Gizmo {
            id,
            gesture,
            curve,
            ik_chain: chain,
            pole,
        });
        self.drag_button = Some(MouseButton::Left);
    }
    fn cancel_drag(&mut self) {
        self.curves.drawing = false;
        self.slider_click = None;
        self.view_axis_press = None;
        if let Some(drag) = self.drag.take() {
            if matches!(drag, Drag::Mesh(_)) {
                self.authoring.last_click = None;
            }
            if let Drag::ComponentSelection(ref selection) = drag {
                selection.original.restore(&mut self.authoring);
            }
            if let Drag::LiveParameter(ref parameter) = drag {
                self.restore_live_parameter(parameter);
            }
            if let Drag::CreationPanel { original, .. } = &drag {
                self.preferences.creation_panel_position = *original;
            }
            if let Drag::Panel(ref panel) = drag {
                if panel.right {
                    self.preferences.right_width = panel.original;
                } else {
                    self.preferences.left_width = panel.original;
                }
            }
            self.cancel_uv_pan(&drag);
            self.cancel_node_pan(&drag);
            self.cancel_view_navigation(&drag);
            if let Drag::Color(ref color) = drag {
                self.restore_color_drag(color);
            } else if !matches!(drag, Drag::ComponentSelection(_) | Drag::LiveParameter(_)) {
                self.editor.cancel_edit();
                self.restore_animation_drag(&drag);
                if matches!(drag, Drag::Uv(_) | Drag::Mesh(_) | Drag::Weights(_) | Drag::Curve(_)) {
                    self.invalidate_geometry();
                }
                if let Drag::Slider(ref slider) = drag {
                    self.restore_slider_preferences(slider);
                }
            }
            self.update_dirty();
        }
        self.drag_button = None;
        self.release_drag_pointer();
    }
    fn reset_workflow_selection(&mut self) {
        self.fx_preview=Default::default();
        self.close_collision_selection();
        self.collisions = Default::default();
        self.authoring.reference_pick = None;
        self.curves = Default::default();
        self.assets = Default::default();
        self.destruction = Default::default();
        self.fluid_domains.clear();
        self.rig_pose = Default::default();
        self.mesh_tools = Default::default();
        self.uv = Default::default();
        self.scene_tools = Default::default();
        self.environments = Default::default();
        self.solo_light = None;
        self.light_aim = None;
        self.last_light_target = None;
        self.recalled_view = None;
        self.last_saved_view = None;
        self.focus_pick = None;
        self.path_pick = None;
        self.path_edit = None;
        self.nodes = node_editor::NodeEditor::default();
        self.cancel_modeling_live();
        self.authoring = authoring_controls::AuthoringState::default();
        self.primitive_panel = None;
        self.weights = weight_paint::WeightPaint::default();
        self.timeline = crate::animation_timeline::TimelineState::default();
        self.animation = animation_controls::AnimationSession::default();
        self.animation_overlay.invalidate();
        self.fluid_on = false;
        self.studio.physics_running = false;
        self.studio.physics.reset();
        self.studio.layer = 0;
        self.camera_navigation_lock = false;
        self.reset_fluid_preview();
        self.surface = surfaces::SurfaceState::default();
        self.inspector_scroll = 0.;
        self.active_rig_root = self
            .editor
            .project
            .rig_profiles
            .first()
            .and_then(|p| p.root);
        self.rig_draft = RigProfile {
            root: None,
            assignments: BTreeMap::new(),
        };
        self.rig_draft_active = false;
        self.detection = None;
        self.character_panel = false;
        self.animation_panel = false;
        self.menu = None;
        self.camera_view = None;
        self.free_view_fov = None;
        self.dropdown = None;
        self.dropdown_focus = None;
        self.ik = false;
    }
    fn focus_imported_character(&mut self, imported_roots: &[u64]) {
        let roots: Vec<_> = poseforge_core::character_roots(&self.editor.project)
            .into_iter()
            .filter(|id| {
                imported_roots
                    .iter()
                    .any(|root| self.is_descendant(*id, *root))
            })
            .collect();
        if roots.is_empty() {
            return;
        }
        self.active_rig_root = (roots.len() == 1).then(|| roots[0]);
        self.editor.selected = self.active_rig_root;
        self.rig_draft_active = false;
        self.prepare_character();
    }
    fn load_rig_manifest(&mut self, path: &Path) {
        let result = std::fs::metadata(path)
            .map_err(|e| e.to_string())
            .and_then(|m| {
                if m.len() > 16 * 1024 * 1024 {
                    return Err("Список костей превышает 16 МиБ".into());
                }
                std::fs::read_to_string(path).map_err(|e| e.to_string())
            })
            .and_then(|text| poseforge_io::parse_rig_manifest(&text).map_err(|e| e.to_string()));
        match result {
            Ok(manifest) => {
                self.status = format!(
                    "{}: {} костей. {}",
                    manifest.armature,
                    manifest.bones.len(),
                    manifest.compare(&self.editor.project).summary()
                );
                self.rig_manifest = Some(manifest);
                self.open_rig_report();
            }
            Err(e) => {
                self.status = format!("Список костей: {e}");
                self.open_details_report(self.status.clone());
            }
        }
    }
    fn open_dropped(&mut self, path: PathBuf) {
        if (self.has_modal() && !self.welcome_active()) || self.job.is_some() {
            self.status = "Завершите текущий диалог или операцию перед открытием файла.".into();
            return;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "txt" => self.load_rig_manifest(&path),
            "glb" | "gltf" | "obj" => self.start_load(path, true),
            "artforge" | "poseforge" | "json" if !self.dirty && !self.editor.has_staged() => {
                self.start_load(path, false)
            }
            "artforge" | "poseforge" | "json" => {
                self.status = "Сначала сохраните текущий проект и запишите временную позу.".into()
            }
            _ => self.status =
                "Поддерживаются GLB/glTF, OBJ, .artforge, .poseforge и список костей .txt."
                    .into(),
        }
    }
    fn view(&self) -> View {
        if let Some(view) = self.camera_object_view() {
            return view;
        }
        if let Some(camera) = &self.recalled_view {
            return camera_controls::view_from_camera(
                camera,
                self.viewport().map(|v| v * self.scale),
            );
        }
        let dir = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        );
        View {
            eye: self.target + dir * self.distance,
            target: self.target,
            up: Vec3::Y,
            fov_y: self
                .free_view_fov
                .unwrap_or(self.editor.project.camera.fov_y_radians),
            viewport: self.viewport().map(|v| v * self.scale),
            orthographic: self.orthographic.then_some(self.distance * 0.42),
            effects: Default::default(),
            depth_of_field: Default::default(),
        }
    }
    fn project_point(&self, p: Vec3) -> Option<[f32; 2]> {
        let v = self.view();
        let r = self.viewport();
        let clip = v.view_projection() * p.extend(1.);
        if clip.w <= 0. {
            return None;
        }
        let q = clip.truncate() / clip.w;
        if q.z < 0. || q.z > 1. {
            return None;
        }
        Some([
            r[0] + (q.x * 0.5 + 0.5) * r[2],
            r[1] + (0.5 - q.y * 0.5) * r[3],
        ])
    }
    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.set_cursor(
                (self.authoring.reference_pick.is_some() || self.collision_picking() || self.path_pick.is_some()).then_some(winit::window::CursorIcon::Crosshair)
                    .or_else(|| self.live_parameter_cursor())
                    .or_else(|| {
                        if self
                            .active_field_rect()
                            .is_some_and(|r| ui::contains(r, self.mouse))
                        {
                            Some(winit::window::CursorIcon::Text)
                        } else if matches!(self.drag, Some(Drag::Slider(_)))
                            || (self.drag.is_none()
                                && matches!(self.ui.hit(self.mouse), Some(Action::Slide(..))))
                        {
                            Some(winit::window::CursorIcon::EwResize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(winit::window::CursorIcon::Default),
            );
            w.request_redraw();
        }
    }
    fn result<T>(&mut self, r: Result<T, impl std::fmt::Display>) -> Option<T> {
        match r {
            Ok(v) => {
                self.update_dirty();
                Some(v)
            }
            Err(e) => {
                self.status = e.to_string();
                None
            }
        }
    }
    fn editing_transform(&self, id: u64) -> Option<Transform> {
        self.evaluated_interactive().ok()?.local.get(&id).copied()
    }
    fn input(&mut self, purpose: Action, value: String) {
        self.editor.playing = false;
        if matches!(purpose, Action::FluidNumber(_)) {
            self.fluid_preview.cancel();
        }
        self.surface.input_error = None;
        self.text_selecting = false;
        self.input = Some(TextEdit::new(value, purpose));
        if let Some(w) = &self.window {
            w.set_ime_allowed(true);
        }
    }
    fn end_input(&mut self) {
        self.input = None;
        self.text_selecting = false;
        self.surface.input_error = None;
        if let Some(w) = &self.window {
            w.set_ime_allowed(false);
        }
    }
    fn set_time(&mut self, t: f64) {
        self.editor.playing = false;
        match self
            .editor
            .set_time(t.clamp(0., self.editor.project.duration))
        {
            Ok(_) => {}
            Err(_) => {
                self.pending_time = Some(t);
            }
        }
    }
    fn show_files(&mut self, purpose: FilePurpose) {
        self.editor.playing = false;
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let p = match purpose {
            FilePurpose::CacheSave => cwd.join("fluid.afcache"),
            FilePurpose::Save => self.path.clone().unwrap_or(cwd.join("artwork.artforge")),
            FilePurpose::Export => cwd.join("output/frame.png"),
            FilePurpose::Sequence => cwd.join("output/sequence"),
            _ => self
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .unwrap_or(&cwd)
                .to_path_buf(),
        };
        self.dialog = Some(FileDialog::new(purpose, p));
        self.import_focus = None;
    }
    fn import_dialog_active(&self) -> bool {
        self.dialog
            .as_ref()
            .is_some_and(|dialog| dialog.purpose == FilePurpose::Import)
            && self.native_picker.is_none()
            && self.overwrite.is_none()
            && self.pending_time.is_none()
            && self.color_picker.is_none()
            && !self.help
            && !self.poses
            && !self.details
            && !self.rig_details
            && !self.character_panel
            && !self.animation_panel
            && self.menu.is_none()
    }
    fn import_dialog_key(&mut self, key: &Key, shift: bool, repeat: bool) -> bool {
        if !self.import_dialog_active() || self.job.is_some() {
            return false;
        }
        if *key == Key::Named(NamedKey::Tab) {
            if repeat {
                return true;
            }
            let current = self
                .input
                .as_ref()
                .map(|input| input.purpose.clone())
                .or_else(|| self.import_focus.clone());
            if !self.resolve_input() {
                self.redraw();
                return true;
            }
            // Derive focus order from the same enabled controls as pointer hits.
            // This also discards stale entries after changing directories.
            self.ui = self.build_ui();
            let mut actions = Vec::new();
            for hit in &self.ui.hits {
                if import_focus_action(&hit.action) && !actions.contains(&hit.action) {
                    actions.push(hit.action.clone());
                }
            }
            if !actions.is_empty() {
                let index = current.and_then(|action| actions.iter().position(|a| *a == action));
                let next = match (index, shift) {
                    (Some(i), false) => (i + 1) % actions.len(),
                    (Some(i), true) => (i + actions.len() - 1) % actions.len(),
                    (None, false) => 0,
                    (None, true) => actions.len() - 1,
                };
                let action = actions[next].clone();
                self.import_focus = Some(action.clone());
                if action.is_field() {
                    self.action(action);
                }
            }
            self.redraw();
            return true;
        }
        if self.input.is_none() && matches!(key, Key::Named(NamedKey::Enter | NamedKey::Space)) {
            if let Some(action) = self.import_focus.clone() {
                if !repeat {
                    self.ui = self.build_ui();
                    if self.ui.hits.iter().any(|hit| hit.action == action) {
                        self.action(action);
                    }
                }
                return true;
            }
        }
        false
    }
    fn invalidate(&mut self) {
        if let Some(r) = &mut self.renderer {
            r.invalidate_assets();
        }
    }
    fn invalidate_geometry(&mut self) {
        // The renderer keys geometry by SharedBuffer revisions. Keep texture,
        // material and unrelated drawable caches alive during a mesh gesture.
        self.redraw();
    }
    fn start_load(&mut self, path: PathBuf, import: bool) {
        if import && path.extension().is_some_and(|ext|ext.eq_ignore_ascii_case("blend")) {
            self.status="Для импорта модели используйте GLB, glTF или OBJ.".into();return;
        }
        if self.editor.has_staged() {
            self.status = "Запишите ключ или отбросьте временную позу перед загрузкой.".into();
            return;
        }
        if self.job.is_some() {
            self.status = "Дождитесь завершения текущей операции или отмените её.".into();
            return;
        }
        self.cancel_color_picker();
        self.cancel_drag();
        self.cancel_modeling_live();
        self.editor.playing = false;
        let before = self.editor.project.clone();
        let clip = self.editor.evaluated_clip();
        let time = self.editor.time;
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let blender_path = self.preferences.blender_path.clone();
        let blender_options = poseforge_io::BlendImportOptions {
            import_animations: self.preferences.blend_import_animations,
        };
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if import {
                    let mut last_progress = Instant::now();
                    let mut last_phase = String::new();
                    let progress = |progress: f32, text: &str| {
                        if text == last_phase
                            && progress < 1.
                            && last_progress.elapsed() < Duration::from_millis(50)
                        {
                            return;
                        }
                        last_progress = Instant::now();
                        last_phase.clear();
                        last_phase.push_str(text);
                        let progress = if progress >= 0. {
                            progress * 0.9
                        } else {
                            progress
                        };
                        let _ =
                            tx.send(JobMessage::Progress(import_progress_label(progress, text)));
                    };
                    let result = if path
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.eq_ignore_ascii_case("blend"))
                    {
                        poseforge_io::import_blend_with_options(
                            &path,
                            blender_path.as_deref(),
                            blender_options,
                            &c,
                            progress,
                        )
                    } else {
                        poseforge_io::import_model_with_progress(&path, &c, progress)
                    };
                    let result = result.and_then(|imported| {
                        model_import::prepare_import(
                            before,
                            imported,
                            clip,
                            time,
                            &c,
                            |value, text| {
                                let _ = tx
                                    .send(JobMessage::Progress(import_progress_label(value, text)));
                            },
                        )
                    });
                    if !c.load(Ordering::Relaxed) {
                        let _ = tx.send(match result {
                            Ok(i) => JobMessage::Imported(Box::new(i)),
                            Err(e) => JobMessage::Error(e.to_string()),
                        });
                    }
                } else {
                    let result = poseforge_io::load_project(&path);
                    if !c.load(Ordering::Relaxed) {
                        let _ = tx.send(match result {
                            Ok(p) => JobMessage::Loaded(p, path),
                            Err(e) => JobMessage::Error(e.to_string()),
                        });
                    }
                }
            }));
            if outcome.is_err() && !c.load(Ordering::Relaxed) {
                let _ = tx.send(JobMessage::Error(
                    "Не удалось обработать файл: внутренняя ошибка импортера. Сцена не изменена."
                        .into(),
                ));
            }
        });
        self.job = Some(Job {
            receiver: rx,
            cancel,
            label: if import {
                "Импорт"
            } else {
                "Открытие"
            }
            .into(),
        });
    }
    fn scene_write_pending(&self) -> bool {
        // Scene jobs may replace selection or capture a material ID. Resolve
        // fields before starting them and keep editing targets stable until commit.
        self.job.as_ref().is_some_and(|job| {
            matches!(
                job.label.as_str(),
                "Открытие"
                    | "Импорт"
                    | "Добавление сцены"
                    | "Загрузка текстуры"
                    | "Импорт видео"
                    | "Загрузка HDRI"
                    | "Поверхность"
                    | "Привязка скелета"
                    | "Развёртка UV"
                    | "Обработка сетки"
                    | "Разрушение"
                    | "Подготовка жидкости"
            )
        })
    }
    fn start_texture_load(&mut self, path: PathBuf) {
        if poseforge_io::is_video_path(&path) { self.start_video_load(path); return; }
        if self.node_texture_target.is_some() {
            self.start_node_image_load(path);
            return;
        }
        if self.job.is_some() {
            self.status = "Дождитесь текущей операции или отмените её.".into();
            return;
        }
        let Some((material, slot)) = self.texture_target.take() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        std::thread::spawn(move || {
            if c.load(Ordering::Relaxed) {
                return;
            }
            let result = poseforge_io::load_texture(&path);
            if !c.load(Ordering::Relaxed) {
                let message = match result {
                    Ok(texture) => JobMessage::TextureLoaded(texture, material, slot),
                    Err(error) => JobMessage::Error(error.to_string()),
                };
                let _ = tx.send(message);
            }
        });
        self.job = Some(Job {
            receiver: rx,
            cancel,
            label: "Загрузка текстуры".into(),
        });
        self.status = "Загрузка текстуры… Esc — отмена".into();
        self.editor.playing = false;
    }
    fn start_save(&mut self, path: PathBuf) {
        if self.editor.has_staged() {
            self.status = "Перед сохранением поставьте ключ или отбросьте временную позу.".into();
            return;
        }
        if self.job.is_some() {
            self.status = "Дождитесь текущей операции.".into();
            return;
        }
        let view = self.view();
        let r = self.editor.transact("Сохранить ракурс", |p| {
            p.camera.eye = view.eye;
            p.camera.target = view.target;
            p.camera.up = view.up;
            p.camera.fov_y_radians = view.fov_y.clamp(0.011, 3.129);
            p.camera.orthographic = view.orthographic.is_some();
            p.camera.ortho_size = view.orthographic.unwrap_or(3.);
        });
        if self.result(r).is_none() {
            return;
        }
        let project = self.editor.project.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        std::thread::spawn(move || {
            let msg = match poseforge_io::save_project(&project, &path) {
                Ok(_) => JobMessage::Saved(path, project),
                Err(e) => JobMessage::Error(e.to_string()),
            };
            let _ = tx.send(msg);
        });
        self.job = Some(Job {
            receiver: rx,
            cancel,
            label: "Сохранение".into(),
        });
    }
    fn start_export(&mut self, path: PathBuf, sequence: bool) {
        if self.job.is_some() {
            self.status = "Дождитесь текущей операции.".into();
            return;
        }
        if self.editor.has_staged() {
            self.status = "Поставьте ключ или отбросьте временную позу перед экспортом.".into();
            return;
        }
        let p = self.editor.project.clone();
        let clip = self.editor.evaluated_clip();
        let time = self.editor.time;
        let effect_time = self.fx_view_time();
        let width = self.editor.project.render_settings.output_width;
        let height = self.editor.project.render_settings.output_height;
        let mut view = self.view();
        view.viewport = [0., 0., width as f32, height as f32];
        let transparent = self.editor.project.render_settings.transparent_background;
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let spawned = std::thread::Builder::new().name("image-export".into())
            .stack_size(poseforge_render::RENDERER_STACK_BYTES).spawn(move || {
            let result = (|| -> Result<usize, String> {
                let mut renderer = pollster::block_on(Renderer::headless(width, height))?;
                renderer.set_fx_cancel(c.clone());
                let frames = if sequence {
                    (p.duration * p.fps as f64).round() as usize + 1
                } else {
                    1
                };
                if sequence {
                    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
                } else if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut completed = vec![];
                let mut fluids = fluid_domains::ExportDomains::default();
                let mut videos = poseforge_io::video::VideoFrameCache::default();
                for frame in 0..frames {
                    if c.load(Ordering::Relaxed) {
                        break;
                    }
                    let t = if sequence {
                        frame as f64 / p.fps as f64
                    } else {
                        time
                    };
                    let eval = p.evaluated(clip, t).map_err(|e| e.to_string())?;
                    let mut sampled = p.clone();
                    sampled.apply_text_meshes(&eval.text_meshes);
                    sampled.apply_text_meshes(&eval.modifier_meshes);
                    sampled.lights.clone_from(&eval.lights);
                    for n in &mut sampled.nodes {
                        if let Some(w) = eval.morph_weights.get(&n.id) {
                            n.morph_weights = w.clone();
                        }
                    }
                    fluids.apply(&mut sampled, &eval.global, t, &c)?;
                    videos.apply(&mut sampled, t, true, &c).map_err(|e| e.to_string())?;
                    let output = if sequence {
                        path.join(format!("frame_{frame:05}.png"))
                    } else {
                        path.clone()
                    };
                    let frame_view =
                        camera_controls::export_camera_view(&sampled, &eval.global, &view);
                    renderer.set_effect_time(if sequence {t}else{effect_time});
                    renderer.export_png(
                        &sampled,
                        &eval.global,
                        &frame_view,
                        &[],
                        &output,
                        width,
                        height,
                        transparent,
                    )?;
                    completed.push(frame);
                    if sequence {
                        let journal = serde_json::json!({"fps":p.fps,"width":width,"height":height,"completed":completed});
                        let tmp = path.join("progress.tmp");
                        std::fs::write(&tmp, serde_json::to_vec_pretty(&journal).unwrap())
                            .map_err(|e| e.to_string())?;
                        std::fs::rename(tmp, path.join("progress.json"))
                            .map_err(|e| e.to_string())?;
                    }
                    let _ = tx.send(JobMessage::Progress(format!(
                        "Рендер: {} / {}",
                        frame + 1,
                        frames
                    )));
                }
                Ok(completed.len())
            })();
            let _ = tx.send(match result {
                Ok(n) => JobMessage::Exported(n, path),
                Err(e) => JobMessage::Error(e),
            });
        });
        if let Err(error) = spawned {
            self.status = format!("Не удалось запустить рендер: {error}");
            return;
        }
        self.job = Some(Job {
            receiver: rx,
            cancel,
            label: "Рендер".into(),
        });
        self.editor.playing = false;
    }
    fn poll_jobs(&mut self) {
        let mut messages = vec![];
        let mut disconnected = false;
        if let Some(j) = &self.job {
            // Bound each event-loop turn, including a noisy external importer.
            for _ in 0..64 {
                match j.receiver.try_recv() {
                    Ok(m) => messages.push(m),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
        for msg in messages {
            if self
                .job
                .as_ref()
                .is_some_and(|job| job.cancel.load(Ordering::Relaxed))
                && matches!(
                    &msg,
                    JobMessage::Progress(_)
                        | JobMessage::LibrarySceneReady(..)
                        | JobMessage::Error(_)
                        | JobMessage::Imported(_)
                        | JobMessage::Loaded(_, _)
                        | JobMessage::TextureLoaded(_, _, _)
                        | JobMessage::VideoLoaded(..)
                        | JobMessage::NodeImageLoaded(_, _, _)
                        | JobMessage::EnvironmentLoaded(_)
                        | JobMessage::Surface(_)
                        | JobMessage::Bound(..)
                        | JobMessage::PhysicsBaked(..)
                        | JobMessage::FluidAttached(..)
                        | JobMessage::DestructionReady(..)
                        | JobMessage::UvReady(..)
                        | JobMessage::MeshToolReady(..)
                        | JobMessage::FluidCacheReady(..)
                )
            {
                if !matches!(msg, JobMessage::Progress(_)) {
                    self.job = None;
                }
                self.status = "Операция отменена; сцена сохранена без изменений.".into();
                continue;
            }
            match msg {
                JobMessage::FluidCacheReady(cache, signature) => {
                    self.job = None;
                    if let Some(cache) = cache {
                        if self.fluid.signature() != signature {
                            self.status = "Настройки жидкости изменились; кэш не загружен".into();
                            continue;
                        }
                        self.fluid_preview.cancel();
                        self.fluid_preview.cache = cache;
                    }
                    self.status = "Кэш жидкости · готово".into();
                }
                JobMessage::MeshToolReady(before, after, id, faces) => {
                    self.job = None;
                    if self.editor.project != *before {
                        self.status =
                            "Сцена изменилась во время расчёта; повторите операцию".into();
                        continue;
                    }
                    let result =
                        self.editor.transact("Обработка сетки", |p| *p = *after);
                    if self.result(result).is_some() {
                        self.mesh_tool_finished(id, faces);
                    }
                }
                JobMessage::UvReady(before, after) => {
                    self.job = None;
                    if self.editor.project != *before {
                        self.status =
                            "Сцена изменилась во время расчёта UV; повторите операцию".into();
                        continue;
                    }
                    let result = self.editor.transact("Развёртка UV", |p| *p = *after);
                    if self.result(result).is_some() {
                        self.uv.selected.clear();
                        self.invalidate_geometry();
                        self.update_dirty();
                        self.status = "UV · готово".into();
                    }
                }
                JobMessage::DestructionReady(before, after, source, root, bake, preview) => {
                    self.finish_destruction(*before, *after, source, root, bake, preview)
                }
                JobMessage::FluidAttached(before, after, id, ready) => {
                    self.finish_fluid_attachment(*before, *after, id, *ready)
                }
                JobMessage::PhysicsBaked(before, after) => {
                    self.finish_physics_bake(*before, *after)
                }
                JobMessage::Bound(before, after, target, root) => {
                    self.job = None;
                    if self.editor.project != *before {
                        self.status =
                            "Сцена изменилась во время привязки; запустите расчёт повторно.".into();
                        continue;
                    }
                    let r =
                        self.editor.transact("Привязать меш к скелету", |p| {
                            *p = *after
                        });
                    if self.result(r).is_some() {
                        self.authoring.bind_target = Some(target);
                        self.active_rig_root = Some(root);
                        self.authoring.editing = None;
                        self.invalidate();
                        self.update_dirty();
                        self.status = "Меш привязан. Выберите кость и режим «Поза»; шаблон поддерживает IK кистей и стоп.".into();
                    }
                }
                JobMessage::Progress(s) => self.status = s,
                JobMessage::Error(e) => {
                    self.status = e;
                    self.job = None;
                }
                JobMessage::Loaded(p, path) => {
                    self.cancel_color_picker();
                    self.cancel_drag();
                    self.editor = Editor::new(p);
                    self.saved_snapshot = self.editor.project.clone();
                    self.preferences.remember_project(&path);
                    self.persist_preferences();
                    self.welcome.open=false;
                    self.path = Some(path);
                    self.reset_workflow_selection();
                    self.dirty = false;
                    self.scene_scroll = 0;
                    self.collapsed.clear();
                    self.scene_query.clear();
                    self.invalidate();
                    self.restore_camera();
                    self.status = "Проект открыт.".into();
                    self.job = None;
                }
                JobMessage::LibrarySceneReady(edit, roots, title) => {
                    self.job = None;
                    let result = self.editor.commit_prepared("Добавить сцену из библиотеки", *edit);
                    if self.result(result).is_some() {
                        if let Some(&id) = roots.first() { self.action(Action::Select(id)); }
                        self.invalidate_geometry(); self.update_dirty();
                        self.status = format!("{title} добавлена · Ctrl+Z — отменить");
                    }
                }
                JobMessage::Imported(i) => {
                    let report = i.report;
                    self.last_import_report = report.clone();
                    match self.editor.commit_prepared("Импорт", i.edit) {
                        Ok(()) => {
                            self.welcome.open=false;
                            self.editor.selected = i.roots.first().copied();
                            if i.has_skeleton {
                                self.preferences.skeleton = true;
                            }
                            self.invalidate();
                            if let Some((target, distance)) = i.framing {
                                self.target = target;
                                self.distance = distance;
                            }
                            self.update_dirty();
                            self.status = format!(
                                "{} Подробности: Проект → Отчёт последнего импорта.",
                                report.lines().next().unwrap_or("Импорт завершён.")
                            );
                            if !self.has_modal() {
                                self.focus_imported_character(&i.roots);
                            }
                        }
                        Err(e) => {
                            self.status = e.to_string();
                            self.last_import_report
                                .push_str(&format!("\nДобавление в сцену отменено: {e}"));
                        }
                    };
                    self.job = None;
                }
                JobMessage::EnvironmentLoaded(map) => {
                    let cancelled = self
                        .job
                        .as_ref()
                        .is_none_or(|j| j.cancel.load(Ordering::Relaxed));
                    self.job = None;
                    if cancelled {
                        continue;
                    }
                    let r = self.editor.transact("Загрузить HDRI", |p| {
                        if let Some(previous) = p.environment_map.replace(Arc::new(map)) {
                            p.environment_library.push(previous);
                        }
                        p.render_settings.hdri_enabled = true;
                    });
                    if self.result(r).is_some() {
                        self.invalidate();
                        self.status = "HDRI загружено и встроено в проект.".into();
                    }
                }
                JobMessage::NodeImageLoaded(texture, material, node) => {
                    self.finish_node_image_load(texture, material, node)
                }
                JobMessage::VideoLoaded(texture, video, material, target) => self.finish_video_load(texture, video, material, target),
                JobMessage::TextureLoaded(texture, material, slot) => {
                    let cancelled = self
                        .job
                        .as_ref()
                        .is_none_or(|j| j.cancel.load(Ordering::Relaxed));
                    self.job = None;
                    if cancelled {
                        continue;
                    }
                    self.cancel_color_picker();
                    self.cancel_drag();
                    if self.editor.project.material(material).is_none() || slot >= 5 {
                        self.status = "Материал удалён; текстура не добавлена.".into();
                        continue;
                    }
                    let r = self.editor.transact(
                        "Загрузить карту материала",
                        |p| {
                            assign_loaded_texture(p, texture, material, slot);
                        },
                    );
                    if self.result(r).is_some() {
                        self.invalidate();
                        self.status =
                            "Карта материала добавлена. Изображение встроено в проект.".into();
                    }
                }
                JobMessage::Saved(path, saved) => {
                    self.preferences.remember_project(&path);
                    self.persist_preferences();
                    self.path = Some(path.clone());
                    self.dirty = self.editor.project != saved || self.editor.has_staged();
                    self.saved_snapshot = saved;
                    self.status = format!("Сохранено: {}", path.display());
                    self.job = None;
                }
                JobMessage::Autosaved(path) => {
                    self.status = format!("Автосохранение: {}", path.display());
                    self.job = None;
                }
                JobMessage::Surface(surface) => {
                    self.cancel_color_picker();
                    self.cancel_drag();
                    let mut id = 0;
                    let r =
                        self.editor.transact("Снимок поверхности воды", |p| {
                            id = p.next_id();
                            let material = id + 1;
                            let mesh = id + 2;
                            let vertices = surface
                                .positions
                                .iter()
                                .zip(&surface.normals)
                                .map(|(position, normal)| poseforge_core::Vertex {
                                    position: *position,
                                    normal: *normal,
                                    ..Default::default()
                                })
                                .collect();
                            p.materials.push(poseforge_core::Material {
                                id: material,
                                name: "Вода · снимок".into(),
                                base_color: [0.12, 0.46, 0.7, 1.],
                                metallic: 0.,
                                roughness: 0.08,
                                ..Default::default()
                            });
                            p.meshes.push(poseforge_core::Mesh {
                                id: mesh,
                                name: "Поверхность PBF".into(),
                                vertices,
                                indices: surface.indices.into(),
                                material: Some(material),
                                topology: None,
                                morph_targets: vec![],
                            });
                            let mut n = poseforge_core::Node::new(id, "Вода · снимок PBF");
                            n.mesh = Some(mesh);
                            p.nodes.push(n);
                        });
                    if self.result(r).is_some() {
                        self.editor.selected = Some(id);
                        self.invalidate();
                        self.status="Снимок поверхности добавлен в сцену: его можно сохранить и отрендерить.".into();
                    }
                    self.job = None;
                }
                JobMessage::Exported(n, path) => {
                    self.status = format!("Сохранено кадров: {n} · {}", path.display());
                    self.job = None;
                }
            }
        }
        if disconnected && self.job.is_some() {
            let cancelled = self.job.take().unwrap().cancel.load(Ordering::Relaxed);
            self.status = if cancelled {
                "Операция отменена; сцена сохранена без изменений."
            } else {
                "Фоновая операция завершилась без результата; повторите действие."
            }
            .into();
        }
    }
    fn restore_camera(&mut self) {
        self.camera_view = None;
        self.free_view_fov = None;
        let camera = &self.editor.project.camera;
        self.target = camera.target;
        let delta = camera.eye - camera.target;
        self.distance = delta.length().max(0.1);
        let dir = delta / self.distance;
        self.yaw = dir.x.atan2(dir.z);
        self.pitch = dir.y.clamp(-1., 1.).asin().clamp(-1.56, 1.56);
        self.orthographic = camera.orthographic;
        if self.orthographic {
            self.distance = camera.ortho_size / 0.42;
        }
    }
    fn dispatch_file(&mut self, purpose: FilePurpose, path: PathBuf) {
        match purpose {
            FilePurpose::CacheSave => self.fluid_cache_file(path, true),
            FilePurpose::CacheLoad => self.fluid_cache_file(path, false),
            FilePurpose::Environment => self.start_environment_load(path),
            FilePurpose::Import => self.start_load(path, true),
            FilePurpose::Texture => self.start_texture_load(path),
            FilePurpose::Rig => self.load_rig_manifest(&path),
            FilePurpose::Open => self.start_load(path, false),
            FilePurpose::Save => self.start_save(path),
            FilePurpose::Export => self.start_export(path, false),
            FilePurpose::Sequence => self.start_export(path, true),
        }
    }
    fn frame_all(&mut self) {
        let Ok(e) = self.evaluated_interactive() else {
            return;
        };
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for n in &self.editor.project.nodes {
            if let Some(g) = e.global.get(&n.id) {
                if let Some(mesh) = e.modifier_meshes.get(&n.id).or_else(||e.text_meshes.get(&n.id)).or_else(||n.mesh.and_then(|m| self.editor.project.mesh(m))) {
                    for v in &mesh.vertices {
                        let p = g.transform_point3(Vec3::from_array(v.position));
                        min = min.min(p);
                        max = max.max(p);
                    }
                } else if let Some(curve) = &n.curve {
                    for &point in &curve.points {
                        let p = g.transform_point3(Vec3::from_array(point)); min = min.min(p); max = max.max(p);
                    }
                } else {
                    let p = g.w_axis.truncate(); min = min.min(p); max = max.max(p);
                }
            }
        }
        if min.is_finite() {
            self.target = (min + max) * 0.5;
            self.distance = (max - min).length().max(0.5) * 1.8;
        }
    }
    fn action(&mut self, a: Action) {
        if self.swatch_menu_action(&a) { return; }
        if self.welcome_action(&a) {return;}
        if self.path_pick.is_some() {
            if let Action::Select(id)=a {self.pick_path_node(id);return;}
            self.path_pick=None;self.status="Выбор привязки отменён".into();self.redraw();return;
        }
        if let Some((index, expected, tracking)) = self.light_aim.clone() {
            if let Action::Select(id) = a {
                if self.editor.project.lights.get(index) == Some(&expected) && self.node_selectable(id) {
                    self.aim_light_at(index, id, tracking);
                }
                self.light_aim = None; self.redraw(); return;
            }
        }
        if self.route_collision_action(&a) { return; }
        if self.authoring.reference_pick.is_some() {
            if a == Action::Cancel {
                self.authoring.reference_pick = None;
                self.status = "Выбор пипеткой отменён.".into(); self.redraw(); return;
            }
            if let Action::Select(id) = &a { self.pick_rig_reference_node(*id); return; }
            if !matches!(a, Action::RigReferencePick(_) | Action::RigReferenceSet(..)) {
                self.authoring.reference_pick = None;
            }
        }
        if !matches!(a, Action::Slide(..)) {
            self.slider_click = None;
        }
        if a == Action::Cancel && self.light_aim.take().is_some() {
            self.status = "Выбор цели света отменён.".into();
            self.redraw();
            return;
        }
        if a == Action::Cancel && self.focus_pick.take().is_some() {
            self.status = "Выбор фокуса отменён.".into();
            self.redraw();
            return;
        }
        self.performance.changed();
        if self.dropdown.is_some() && a == Action::Cancel {
            self.close_asset_popup();
            self.close_material_dropdown();
            self.dropdown = None;
            self.redraw();
            return;
        }
        if !matches!(
            a,
            Action::DropdownChoose(_) | Action::DropdownOpen(_) | Action::DropdownDismiss
        ) {
            self.close_asset_popup();
            self.close_material_dropdown();
            self.dropdown = None;
            self.dropdown_focus = None;
        }
        if a == Action::ToggleBlendImportAnimations
            && ((!self.import_dialog_active() && self.surface.tab != 9) || self.job.is_some())
        {
            return;
        }
        if self.native_picker.is_some() {
            return;
        }
        if matches!(&a, Action::ResetNumber(field) | Action::ResetColor(field) if self.input.as_ref().is_some_and(|i| i.purpose == **field)) {
            self.end_input();
        }
        if self.action_with_color_picker(&a) {
            return;
        }
        if a == Action::Cancel && self.modeling_live.is_some() && self.input.is_none() {
            self.cancel_modeling_live();
            return;
        }
        if a == Action::Cancel && self.curves.drawing && self.drag.is_none() && self.input.is_none() {
            self.curves.drawing = false; self.status = "Рисование кривой отменено".into(); self.redraw(); return;
        }
        if a == Action::Cancel && self.drag.is_some() {
            self.cancel_drag();
            self.redraw();
            return;
        }
        if self
            .input
            .as_ref()
            .is_some_and(|input| input.purpose.search_clear().as_ref() == Some(&a))
        {
            self.end_input();
        }
        if let Some(input) = &self.input {
            if a == Action::Cancel {
                self.end_input();
                self.redraw();
                return;
            }
            if a == Action::ConfirmInput {
                self.confirm_input();
                self.redraw();
                return;
            }
            if a == input.purpose {
                return;
            }
            let replaces_path = matches!(
                (&a, &input.purpose),
                (Action::BrowsePath | Action::PastePath, Action::EditPath)
                    | (
                        Action::BrowseBlenderPath | Action::PasteBlenderPath,
                        Action::EditBlenderPath
                    )
            );
            if !replaces_path && !self.resolve_input() {
                self.redraw();
                return;
            }
        }
        if let Action::Slide(field, _, _) = &a {
            if self.slider_double_click(field) {
                self.action((**field).clone());
                return;
            }
        }
        if let Action::ResetNumber(field) = &a {
            if self.drag.is_some() { self.cancel_drag(); }
            self.reset_number((**field).clone());
            return;
        }
        if self.route_knife_action(&a) {
            self.redraw();
            return;
        }
        if self.route_live_action(&a) {
            return;
        }
        if a == Action::Cancel && self.primitive_panel.is_some() && self.drag.is_none() {
            self.primitive_panel = None;
            self.redraw();
            return;
        }
        if a == Action::Cancel && self.path_edit.take().is_some() {
            self.status = "Edit выключен · перемещение снова вдоль кривой.".into(); self.redraw(); return;
        }
        if a == Action::Cancel
            && self.nodes_open()
            && self.dialog.is_none()
            && self.overwrite.is_none()
        {
            self.node_key(&Key::Named(NamedKey::Escape), false, false);
            self.redraw();
            return;
        }
        // Changing tool, selection or opening a surface resolves ownership first.
        if self.drag.is_some() && a != Action::Cancel {
            self.cancel_drag();
        }
        if !a.is_field()
            && !matches!(
                a,
                Action::ToggleProjectMenu
                    | Action::ToggleViewMenu
                    | Action::ToggleBodyMenu
                    | Action::Slide(..)
                    | Action::Cancel
            )
        {
            self.menu = None;
        }
        if self.job.is_some() && matches!(a, Action::New | Action::Open | Action::SaveAs) {
            self.status = "Дождитесь текущей операции или отмените её.".into();
            return;
        }
        if self.scene_write_pending() && !matches!(a, Action::Cancel | Action::CopyReport) {
            self.status =
                "Фоновая операция: дождитесь результата или отмените её через Esc.".into();
            return;
        }
        if self.import_dialog_active() && import_focus_action(&a) {
            self.import_focus = Some(a.clone());
        }
        if self.scene_organization_action(&a) {
            self.redraw();
            return;
        }
        if self.scene_tree_action(&a) {
            self.redraw();
            return;
        }
        if self.group_action(&a) {
            self.redraw();
            return;
        }
        if self.light_action(&a) {
            self.redraw();
            return;
        }
        self.remember_modeling_tool(&a);
        match a {
            Action::Modifier(action) => self.modifier_action(action),
            Action::AssetCategory(_) | Action::AssetSearch | Action::AssetSearchClear
            | Action::AssetMaterial(_) | Action::AssetPreset(_) | Action::AssetScene(_) => self.asset_action(a),
            Action::Curve(action) => self.curve_action(action),
            Action::Path(action) => self.path_action(action),
            Action::Media(action) => self.media_action(action),
            Action::Morph(action) => self.morph_action(action),
            Action::ModelingTool(tool) => self.choose_modeling_tool(tool),
            Action::Rig(action) => self.rig_action(action),
            Action::RigStage(stage) => self.rig_stage_action(stage),
            Action::BoneDisplay(action) => self.bone_display_action(action),
            Action::SwatchCommand(_) | Action::SwatchDismiss => {},
            Action::RigReferencePick(reference) => self.begin_rig_reference_pick(reference),
            Action::RigReferenceSet(reference, id) => self.set_rig_reference(reference, id),
            Action::Normal(action) => self.normals_action(action),
            Action::Shape(action) => self.shape_action(action),
            Action::InterfaceTheme(theme) => {
                self.preferences.theme = theme;
                self.persist_preferences();
            }
            Action::Primitive(action) => self.primitive_action(action),
            Action::ToggleTools => {
                if !self.tools_expanded() && self.preferences.tools_expanded {
                    self.preferences.animation_layers_open = false;
                } else {
                    self.preferences.tools_expanded = !self.preferences.tools_expanded;
                }
                self.persist_preferences();
            }
            Action::ToggleSection(id) => self.toggle_property_section(id),
            Action::ViewAxis(axis) => self.click_view_axis(axis),
            Action::MaterialLibraryCategory(_) => {} // Owned by the modal library popup.
            Action::MaterialApplyFaces | Action::MaterialUnassignFaces | Action::MaterialUserDetach(..) | Action::MaterialLibraryToggle | Action::MaterialLibraryRemove(_)
            | Action::MaterialLayerVisible(..) | Action::MaterialLayerMove(_)
            | Action::MaterialBrowse(_) | Action::MaterialSelect(_) | Action::MaterialUsers(_) | Action::MaterialUserSelect(..) => {
                self.material_library_action(&a);
            }
            Action::MaterialFavorite
            | Action::MaterialFavorites
            | Action::MaterialCategory
            | Action::MaterialTags
            | Action::MaterialGrid
            | Action::MaterialSize => {
                self.material_library_action(&a);
            }
            Action::AdaptivePreview => {
                self.preferences.adaptive_preview = !self.preferences.adaptive_preview;
                self.persist_preferences();
            }
            Action::PerformanceOverlay => {
                self.preferences.performance_overlay = !self.preferences.performance_overlay;
                self.persist_preferences();
            }
            Action::ViewOrbit => self.begin_view_widget(),
            Action::ObjectScroll => self.start_object_scroll(),
            Action::MaterialSearch => self.input(a, self.surface.material_query.clone()),
            Action::MaterialSearchClear => {
                self.surface.material_query.clear();
                self.surface.material_offset = 0;
            }
            Action::MaterialScroll => self.start_material_scroll(),
            Action::MaterialAssign(material, mesh, selection) => {
                if self.editor.selected == selection && self.surface_mesh_id() == Some(mesh) {
                    self.material_action(&Action::MaterialAssign(material, mesh, selection));
                }
            }
            Action::World(command) => self.world_action(command),
            Action::Node(command) => self.node_action(command),
            Action::Uv(command) => self.uv_action(command),
            Action::Author(command) => self.authoring_action(command),
            Action::Studio(command) => self.studio_action(command),
            Action::ScrollPanel(0) => {
                let maximum = self.inspector_max_scroll();
                let [_, top_y, _, height] = self.properties_scrollbar_rect();
                let thumb = (height * height / (height + maximum)).max(28.).min(height);
                let top = top_y + (height - thumb) * self.inspector_scroll / maximum.max(1.);
                if self.mouse[1] < top || self.mouse[1] > top + thumb {
                    self.inspector_scroll = ((self.mouse[1] - top_y - thumb * 0.5)
                        / (height - thumb).max(1.)
                        * maximum)
                        .clamp(0., maximum);
                }
                self.drag = Some(Drag::Scrollbar {
                    start: self.mouse[1],
                    value: self.inspector_scroll,
                });
                self.drag_button = Some(MouseButton::Left);
            }
            Action::ScrollPanel(_) => {}
            Action::SceneSelectLight(..)
            | Action::SceneToggleLight(..)
            | Action::SceneToggleNode(_)
            | Action::SceneShadowOnly(_)
            | Action::SceneDelete(_)
            | Action::SceneDeleteLight(..)
            | Action::SceneExpand(_)
            | Action::SceneCloth(_) | Action::SceneClothVisible(_) | Action::SceneClothRemove(_)
            | Action::SceneDestruction(_) | Action::SceneDestructionVisible(_) | Action::SceneDestructionRemove(_)
            | Action::SceneFluid(_)
            | Action::SceneFluidVisible(_)
            | Action::SceneFluidRemove(_)
            | Action::SceneMaterial(..)
            | Action::SceneMaterialVisible(_)
            | Action::SceneMaterialDetach(..) => {}
            Action::CameraTool(tool) => self.lens_action(tool),
            Action::CameraEffect(id, tool) => { self.camera_effect_node_action(id, tool); },
            Action::LightTool(tool) => self.light_tool(tool),
            Action::SceneTool(tool) => self.scene_tool_action(tool),
            Action::PanelResize(right) => self.begin_panel_resize(right),
            Action::CreateCamera
            | Action::CameraView
            | Action::CameraExitView
            | Action::CameraSetActive
            | Action::CameraClearActive
            | Action::CameraProjection
            | Action::CameraAlignView
            | Action::CameraNumber(_) => self.camera_action(a),
            Action::DropdownOpen(kind) => self.open_dropdown(kind),
            Action::DropdownChoose(index) => self.choose_dropdown(index),
            Action::DropdownDismiss => { self.close_asset_popup(); self.close_material_dropdown(); self.dropdown = None; },
            Action::OpenColorPicker(source) => self.open_color_picker(*source),
            Action::ResetColor(source) => self.reset_color(*source),
            Action::ColorPicker(control) => self.color_action(control),
            Action::Animation(control) => self.animation_action(control),
            Action::FluidObject(action) => self.fluid_object_action(action),
            Action::Text(action) => self.text_action(action),
            Action::CreateEmpty => self.create_empty_object(),
            Action::Helper(action) => self.helper_action(action),
            Action::Fx(action) => self.fx_action(action),
            Action::Collision(action) => self.collision_action(action),
            Action::Destruction(action) => self.destruction_action(action),
            Action::ResetNumber(action) => self.reset_number(*action),
            Action::Slide(action, range, value) => {
                if let Action::ColorPicker(control) = *action {
                    self.begin_color_slider(control);
                } else {
                    self.begin_slider(*action, range, value);
                }
            }
            Action::BrowsePath => self.browse_path(false),
            Action::BrowseBlenderPath => self.browse_path(true),
            Action::PastePath => self.paste_path(false),
            Action::PasteBlenderPath => self.paste_path(true),
            Action::SurfaceRenderNumber(_)
            | Action::RenderAntialiasing(_)
            | Action::RenderToneMapping(_)
            | Action::RenderToggleShadows
            | Action::RenderShadowResolution(_)
            | Action::RenderQuality(_)
            | Action::RenderColor(_)
            | Action::LightingPreset(_)
            | Action::FluidNumber(_)
            | Action::FluidPreset(_)
            | Action::FluidResetSettings => self.scene_settings_action(a),
            Action::SurfaceTab(_)
            | Action::SurfaceNextMesh
            | Action::SurfaceMaps
            | Action::SurfaceCreateMaterial
            | Action::SurfaceCreateMaterialPreset(_)
            | Action::SurfaceResetMapping
            | Action::SurfaceUnassignMaterial
            | Action::SurfacePruneMaterials
            | Action::SurfaceUniqueMaterial
            | Action::SurfaceMaterialName
            | Action::SurfaceMaterialList
            | Action::SurfaceAssignMaterial(_)
            | Action::SurfaceMaterialColor(_)
            | Action::SurfaceAttenuationColor
            | Action::SurfaceNprToggle
            | Action::SurfaceArtisticToggle
            | Action::SurfaceArtisticNextStyle
            | Action::SurfaceArtisticReset
            | Action::SurfaceNprColor(_)
            | Action::SurfaceMaterialNumber(_)
            | Action::SurfaceEmissionVolume
            | Action::SurfaceDoubleSided
            | Action::SurfacePreset(_)
            | Action::SurfaceNextTexture(_)
            | Action::SurfaceClearTexture(_)
            | Action::SurfaceAddLight(_)
            | Action::SurfaceDeleteLight
            | Action::SurfaceToggleLight
            | Action::SurfaceToggleLightShadow
            | Action::SurfaceLightTemperature
            | Action::SurfacePointLightAtView
            | Action::SurfaceLightKind
            | Action::SurfaceAimLight
            | Action::SurfaceLightColor
            | Action::SurfaceLightVolumeColor
            | Action::SurfaceLightVolume
            | Action::SurfaceLightName
            | Action::SurfaceLightNumber(_) => self.surface_action(a),
            Action::ViewShading(mode) => {
                self.preferences.viewport_shading = mode.min(3);
                self.persist_preferences();
                self.redraw();
            }
            Action::ToggleProjectMenu | Action::ToggleViewMenu | Action::ToggleBodyMenu => {
                let menu = match a {
                    Action::ToggleProjectMenu => crate::chrome::Menu::Project,
                    Action::ToggleViewMenu => crate::chrome::Menu::View,
                    _ => crate::chrome::Menu::Body,
                };
                self.menu = if self.menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                self.editor.playing = false;
            }
            Action::DismissMenu => self.menu = None,
            Action::CharacterPanel => self.prepare_character(),
            Action::DetectSkeleton => {
                let report =
                    poseforge_core::detect_humanoid(&self.editor.project, self.rig_draft.root);
                self.rig_draft = report.resolved.clone();
                self.status = format!(
                    "Распознано {} из {} ролей. Проверьте соответствия перед сохранением.",
                    self.rig_draft.assignments.len(),
                    BoneRole::ALL.len()
                );
                self.detection = Some(report);
            }
            Action::SaveRigProfile => {
                if self.rig_draft.root.is_none() || self.rig_draft.assignments.is_empty() {
                    self.status =
                        "Выберите корень персонажа и назначьте хотя бы одну кость.".into();
                } else if let Err(e) = self.rig_draft.validate(&self.editor.project) {
                    self.status = e.to_string();
                } else {
                    let profile = self.rig_draft.clone();
                    let root = profile.root;
                    let r = self.editor.transact("Настроить персонажа", |p| {
                        p.rig_profiles.retain(|p| p.root != root);
                        p.rig_profiles.push(profile);
                    });
                    if self.result(r).is_some() {
                        self.active_rig_root = root;
                        self.character_panel = false;
                        self.status = "Профиль персонажа сохранён в проекте. Доступны быстрый выбор частей тела и ключ всей позы.".into();
                    }
                }
            }
            Action::PickRigRoot => {
                self.rig_picker = Some(None);
                self.bone_query.clear();
                self.bone_scroll = 0;
            }
            Action::PickBone(index) => {
                self.rig_picker = Some(Some(index));
                self.bone_query.clear();
                self.bone_scroll = 0;
            }
            Action::SetRigRoot(id) => {
                if self.editor.project.node(id).is_some() {
                    self.rig_draft = RigProfile {
                        root: Some(id),
                        assignments: BTreeMap::new(),
                    };
                    self.rig_picker = None;
                    self.detection = None;
                    self.status =
                        "Корень выбран. Нажмите «Распознать» или назначьте кости вручную.".into();
                }
            }
            Action::AssignBone(id) => {
                if let Some(Some(index)) = self.rig_picker {
                    if let Some(role) = BoneRole::ALL.get(index).copied() {
                        match self.rig_draft.assign(role, id, &self.editor.project) {
                            Ok(()) => {
                                self.rig_picker = None;
                                self.status = format!("Назначено: {}", role.label());
                            }
                            Err(e) => self.status = e.to_string(),
                        }
                    }
                }
            }
            Action::ClearBone(index) => {
                if let Some(role) = BoneRole::ALL.get(index) {
                    self.rig_draft.assignments.remove(role);
                }
            }
            Action::BackToRig => self.rig_picker = None,
            Action::SelectRole(index) => self.select_role(index, false),
            Action::IkRole(index) => self.select_role(index, true),
            Action::SearchBones => self.input(a, self.bone_query.clone()),
            Action::ClearBoneSearch => {
                self.bone_query.clear();
                self.bone_scroll = 0;
            }
            Action::AnimationPanel => {
                self.animation_panel = true;
                self.character_panel = false;
                self.clip_scroll = 0;
                self.editor.playing = false;
                self.editor.mode = EditorMode::Animation;
            }
            Action::CreateClip => self.input(
                a,
                format!("Движение {}", self.editor.project.clips.len() + 1),
            ),
            Action::RenameClip | Action::DuplicateClip => {
                if let Some(clip) = self
                    .editor
                    .active_clip
                    .and_then(|i| self.editor.project.clips.get(i))
                {
                    self.input(
                        a.clone(),
                        if a == Action::DuplicateClip {
                            format!("{} — копия", clip.name)
                        } else {
                            clip.name.clone()
                        },
                    );
                } else {
                    self.status = "Сначала выберите клип.".into();
                }
            }
            Action::KeyPose => match self.character_scope() {
                Ok(nodes) => {
                    let r = self.editor.key_nodes(&nodes);
                    if self.result(r).is_some() {
                        self.status = format!("Записана поза: {} суставов", nodes.len());
                    }
                }
                Err(e) => self.status = e,
            },
            Action::PrevFrame | Action::NextFrame => {
                self.editor.mode = EditorMode::Animation;
                let frame = (self.editor.time * self.editor.project.fps as f64).round();
                let step = if a == Action::PrevFrame { -1. } else { 1. };
                self.set_time((frame + step).max(0.) / self.editor.project.fps as f64);
            }
            Action::EditFrame => self.input(
                a,
                format!("{:.0}", self.editor.time * self.editor.project.fps as f64),
            ),
            Action::ShiftKeys(frames) => {
                if let Some(id) = self.editor.selected {
                    let r = self.editor.shift_current_keys(frames, &[id]);
                    if let Some(count) = self.result(r) {
                        self.status = format!("Сдвинуто ключей выбранного узла: {count}");
                    }
                } else {
                    self.status = "Выберите кость, ключи которой нужно сдвинуть.".into();
                }
            }
            Action::EditBlenderPath => self.input(
                a,
                self.preferences
                    .blender_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            Action::DetectBlender => {
                self.status =
                    match poseforge_io::locate_blender(self.preferences.blender_path.as_deref()) {
                        Ok(path) => format!("Blender найден: {}", path.display()),
                        Err(e) => e.to_string(),
                    };
            }
            Action::ToggleBlendImportAnimations => {
                if (self.import_dialog_active() || self.surface.tab == 9) && self.job.is_none() {
                    self.preferences.blend_import_animations =
                        !self.preferences.blend_import_animations;
                    self.persist_preferences();
                }
            }
            Action::New => {
                if self.dirty || self.editor.has_staged() {
                    self.status="Сначала сохраните проект. Для новой сцены после сохранения нажмите ещё раз.".into();
                } else {
                    self.welcome.open=false;
                    self.status="Новый проект".into();
                    self.editor = Editor::new(Project::default());
                    self.editor.mode = EditorMode::Scene;
                    self.tool = Tool::Orbit;
                    self.saved_snapshot = self.editor.project.clone();
                    self.path = None;
                    self.recovery_id = workspace::new_recovery_id();
                    self.last_autosave = Instant::now();
                    self.reset_workflow_selection();
                    self.scene_scroll = 0;
                    self.collapsed.clear();
                    self.scene_query.clear();
                    self.invalidate();
                    self.frame_all();
                }
            }
            Action::Import => self.show_files(FilePurpose::Import),
            Action::SurfaceImportTexture(slot) => {
                if self.job.is_some() {
                    self.status = "Дождитесь текущей операции или отмените её.".into();
                } else if slot < 5 {
                    if let Some(id) = self.selected_material_id() {
                        self.texture_target = Some((id, slot));
                        self.show_files(FilePurpose::Texture);
                    }
                }
            }
            Action::ImportRig => {
                self.rig_details = false;
                self.show_files(FilePurpose::Rig);
            }
            Action::SearchScene => {
                self.preferences.left_panel = true;
                self.input(a, self.scene_query.clone());
            }
            Action::ClearSearch => {
                self.scene_query.clear();
                self.scene_scroll = 0;
            }
            Action::ToggleLeftPanel => {
                if self.uv.open {
                    self.uv_action(ui::UvAction::Close);
                    self.preferences.left_panel = true;
                } else {
                    self.preferences.left_panel = !self.preferences.left_panel;
                }
                self.persist_preferences();
            }
            Action::ToggleAnimationPanel => {
                if self.animation_panel_visible() {
                    self.preferences.animation_visible = false;
                    self.animation.focused = false;
                } else {
                    if self.editor.mode != EditorMode::Animation {
                        self.action(Action::Mode(2));
                    }
                    self.preferences.animation_visible = true;
                }
                self.persist_preferences();
            }
            Action::ToggleAnimationLayers => {
                self.preferences.animation_layers_open = !self.preferences.animation_layers_open;
                if self.preferences.animation_layers_open {
                    self.show_animation_layers();
                }
                self.persist_preferences();
            }
            Action::AnimationLayersScroll => self.animation_layers_scroll_to_pointer(),
            Action::ToggleRightPanel => {
                self.preferences.right_panel = !self.preferences.right_panel;
                self.persist_preferences();
            }
            Action::ToggleBackfaces => {
                self.preferences.show_backfaces = !self.preferences.show_backfaces;
                self.persist_preferences(); self.performance.changed();
            }
            Action::ToggleGrid => {
                self.preferences.grid = !self.preferences.grid;
                self.persist_preferences();
            }
            Action::ToggleSkeleton => {
                self.preferences.skeleton = !self.preferences.skeleton;
                self.persist_preferences();
            }
            Action::Open => {
                if self.dirty {
                    self.status = "Сохраните текущий проект перед открытием другого.".into();
                } else {
                    self.show_files(FilePurpose::Open)
                }
            }
            Action::Save => {
                if let Some(p) = self.path.clone() {
                    self.start_save(p)
                } else {
                    self.show_files(FilePurpose::Save)
                }
            }
            Action::SaveAs => self.show_files(FilePurpose::Save),
            Action::Undo => {
                self.editor.undo();
                self.update_dirty();
                self.invalidate();
            }
            Action::Redo => {
                self.editor.redo();
                self.update_dirty();
                self.invalidate();
            }
            Action::Mode(i) => {
                if self.editor.has_staged() {
                    self.status = "Сначала поставьте ключ или отбросьте временную позу.".into();
                    return;
                }
                if i == 3 {
                    self.editor.render_animated = self.editor.mode == EditorMode::Animation
                        || (self.editor.mode == EditorMode::Render && self.editor.render_animated);
                }
                self.editor.mode = match i {
                    0 => EditorMode::Scene,
                    1 => EditorMode::Pose,
                    2 => EditorMode::Animation,
                    _ => EditorMode::Render,
                };
                self.scene_tools.folder_selected = false;
                self.editor.playing = false;
                self.authoring.editing = None;
                self.authoring.vertices.clear();
                self.authoring.edges.clear();
                self.authoring.faces.clear();
                if i == 2 {
                    self.preferences.animation_visible = true;
                }
                if i == 3 {
                    self.set_properties_tab(3);
                    self.preferences.right_panel = true;
                }
                if i == 0 {
                    self.weights.target = None;
                    self.preferences.left_panel = true;
                    if let Some(id) = self
                        .editor
                        .selected
                        .filter(|id| self.editor.project.node(*id).is_some_and(|n| n.is_joint))
                    {
                        let (primary, members) = self.skeleton_object_selection(id);
                        self.editor.selected = Some(primary);
                        self.scene_tools.selected = members;
                    }
                } else if i == 1 {
                    self.scene_tools.selected.clear();
                }
            }
            Action::Select(id) => {
                self.select_object(id);
                if self
                    .editor
                    .project
                    .node(id)
                    .is_some_and(|node| node.camera.is_some())
                {
                    self.set_properties_tab(0);
                    self.preferences.right_panel = true;
                    self.inspector_scroll = 0.;
                }
                self.sync_active_character(id);
                self.sync_rig_object_selection();
                if self.rig_context() && matches!(self.surface.tab, 0 | 7 | 10) {
                    self.set_properties_tab(0);
                }
                if self.rig_details {
                    self.preferences.left_panel = true;
                    self.scene_query.clear();
                    self.joints_only = false;
                    let mut parent = self.editor.project.node(id).and_then(|n| n.parent);
                    while let Some(p) = parent {
                        self.collapsed.remove(&p);
                        parent = self.editor.project.node(p).and_then(|n| n.parent);
                    }
                    self.scene_scroll = self.scene_node_offset(id);
                }
                self.rig_details = false;
            }
            Action::Play => self.toggle_animation_playback(),
            Action::Start => {
                let frame = self.animation_preview_range()[0];
                self.set_time(frame as f64 / self.editor.project.fps as f64);
            }
            Action::Timeline => {
                self.animation.focused = true;
                self.scrub();
                if self.pending_time.is_none() {
                    self.drag = Some(Drag::Timeline);
                    self.drag_button = Some(MouseButton::Left);
                }
            }
            Action::PreviousKey | Action::NextKey => {
                let dir = if a == Action::PreviousKey { -1. } else { 1. };
                let times = self.key_times();
                let next = if dir < 0. {
                    times
                        .iter()
                        .copied()
                        .filter(|t| *t < self.editor.time - 0.0001)
                        .max_by(f64::total_cmp)
                } else {
                    times
                        .iter()
                        .copied()
                        .filter(|t| *t > self.editor.time + 0.0001)
                        .min_by(f64::total_cmp)
                };
                if let Some(t) = next {
                    self.set_time(t);
                }
            }
            Action::Key => {
                if let Some(id) = self.editor.selected {
                    let r = self.editor.key_nodes(&[id]);
                    self.result(r);
                } else {
                    self.status = "Сначала выберите объект или сустав.".into();
                }
            }
            Action::AutoKey => self.editor.auto_key = !self.editor.auto_key,
            Action::Reset => {
                let r = self.editor.reset_selected();
                self.result(r);
            }
            Action::Orbit => self.tool = Tool::Orbit,
            Action::Move => self.tool = Tool::Move,
            Action::Rotate => {
                self.tool = Tool::Rotate;
                self.ik = false;
            }
            Action::Scale => {
                self.tool = Tool::Scale;
                self.ik = false;
            }
            Action::ToggleGizmoSpace => {
                self.gizmo_local = !self.gizmo_local;
                self.status = if self.tool == Tool::Scale {
                    "Масштабирование использует локальные оси объекта.".into()
                } else {
                    format!(
                        "Оси гизмо: {}",
                        if self.gizmo_local {
                            "локальные"
                        } else {
                            "мировые"
                        }
                    )
                };
            }
            Action::SetAxis(i) => {
                self.axis = i.min(2);
                self.status = format!(
                    "Ось {} выделена · потяните {} этой оси",
                    ["X", "Y", "Z"][self.axis],
                    if self.tool == Tool::Rotate {
                        "кольцо"
                    } else {
                        "рукоятку"
                    }
                );
            }
            Action::Ik => {
                self.ik = !self.ik;
                self.tool = Tool::Move;
                self.status =
                    "IK: выберите конечный сустав и потяните стрелку или центр гизмо.".into();
            }
            Action::FrameSelected => {
                if self.frame_modifier_selection(&self.object_selection()) {
                    self.status = "Результат модификаторов целиком в кадре".into();
                } else if self.frame_curve() {
                    self.status = "Кривая целиком в кадре".into();
                } else if self.frame_edit_mesh() {
                    self.status = "Меш целиком в кадре · F2 — режим объекта".into();
                } else if let Some(id) = self.editor.selected {
                    if let Ok(e) = self.evaluated_interactive() {
                        if let Some(g) = e.global.get(&id) {
                            self.target = g.w_axis.truncate();
                            self.distance = self.distance.clamp(1., 4.);
                        }
                    }
                } else {
                    self.frame_all();
                }
            }
            Action::Front => self.orient_view(4),
            Action::Side => self.orient_view(0),
            Action::Top => self.orient_view(2),
            Action::Perspective => self.toggle_view_projection(),
            Action::ViewPan => self.begin_view_navigation(1),
            Action::ViewZoom => self.begin_view_navigation(2),
            Action::EditTitle => self.input(a, self.editor.project.title.clone()),
            Action::EditName => {
                if let Some(n) = self
                    .editor
                    .selected
                    .and_then(|id| self.editor.project.node(id))
                {
                    self.input(a, n.name.clone());
                }
            }
            Action::EditTransform(i) => {
                if let Some(t) = self
                    .editor
                    .selected
                    .and_then(|id| self.editing_transform(id))
                {
                    let v = if i < 3 {
                        t.translation[i]
                    } else if i < 6 {
                        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        [x, y, z][i - 3].to_degrees()
                    } else {
                        t.scale[i - 6]
                    };
                    self.input(a, format!("{v:.4}"));
                }
            }
            Action::EditFps => self.input(a, self.editor.project.fps.to_string()),
            Action::EditDuration => self.input(a, format!("{:.2}", self.editor.project.duration)),
            Action::EditWidth => self.input(
                a,
                self.editor.project.render_settings.output_width.to_string(),
            ),
            Action::EditHeight => self.input(
                a,
                self.editor
                    .project
                    .render_settings
                    .output_height
                    .to_string(),
            ),
            Action::SetClip(i) => {
                self.editor.playing = false;
                if self.editor.has_staged() {
                    self.status = "Поставьте ключ перед сменой клипа.".into();
                } else {
                    if i >= self.editor.project.clips.len() {
                        self.editor.active_clip = None;
                        self.editor.render_animated = false;
                    } else {
                        self.editor.active_clip = Some(i);
                        if self.editor.mode == EditorMode::Render {
                            self.editor.render_animated = true;
                        } else {
                            self.editor.mode = EditorMode::Animation;
                        }
                    }
                }
            }
            Action::ToggleVisible => {
                if let Some(id) = self.editor.selected {
                    let r = self.editor.transact("Видимость", |p| {
                        if let Some(n) = p.node_mut(id) {
                            n.visible = !n.visible;
                        }
                    });
                    self.result(r);
                }
            }
            Action::Duplicate => {
                if let Some(id) = self.editor.selected {
                    let mut new_id = None;
                    let r = self.editor.transact("Дублировать ветвь", |p| {
                        new_id = p.duplicate_subtree(id).ok();
                    });
                    self.result(r);
                    if let Some(id) = new_id {
                        self.editor.selected = Some(id);
                        self.sync_active_character(id);
                    }
                    self.invalidate();
                }
            }
            Action::Delete => {
                let r = self.editor.delete_selected();
                self.result(r);
                self.invalidate();
            }
            Action::DeleteKey => {
                if let Some(id) = self.editor.selected {
                    let r = self.editor.delete_current_keys(&[id]);
                    if let Some(count) = self.result(r) {
                        self.status = format!("Удалено ключей выбранного узла: {count}");
                    }
                }
            }
            Action::Poses => {
                self.poses = true;
                self.pose_scroll = 0;
            }
            Action::SavePose => self.input(
                Action::SavePose,
                format!("Поза {}", self.editor.project.poses.len() + 1),
            ),
            Action::LoadPose(i) => {
                let r = self.editor.load_pose(i);
                self.result(r);
                self.poses = false;
            }
            Action::CopyReport => self.copy_open_report(),
            Action::Details => self.open_details_report(self.status.clone()),
            Action::ImportReport => self.open_details_report(self.last_import_report.clone()),
            Action::DetailsPage(step) => {
                let pages = wrapped_report(&self.details_text).len().div_ceil(12);
                self.details_page = (self.details_page as i64 + step as i64)
                    .clamp(0, pages.saturating_sub(1) as i64)
                    as usize;
            }
            Action::CopyPose => {
                if let Ok(e) = self.evaluated_interactive() {
                    self.copied_pose = Some(e.local);
                    self.status = "Поза скопирована.".into();
                }
            }
            Action::PastePose => {
                if let Some(pose) = self.copied_pose.clone() {
                    self.editor.begin_edit("Вставить позу");
                    for (id, t) in pose {
                        if self.editor.project.node(id).is_some() {
                            let r = self.editor.set_transform(id, t);
                            self.result(r);
                        }
                    }
                    let r = self.editor.commit_edit();
                    self.result(r);
                }
            }
            Action::Export => self.show_files(FilePurpose::Export),
            Action::ExportSequence => self.show_files(FilePurpose::Sequence),
            Action::Transparent => {
                let result = self.editor.transact("Прозрачный фон PNG", |p| {
                    p.render_settings.transparent_background =
                        !p.render_settings.transparent_background
                });
                self.result(result);
            }
            Action::FluidBake => {
                self.editor.playing = false;
                self.sync_fluid_settings();
                self.fluid_on = true;
                self.editor.mode = EditorMode::Animation;
                let result = self.fluid_preview.start_bake(
                    &self.fluid,
                    self.editor.project.duration,
                    self.editor.project.fps as f64,
                    self.editor.project.fluid_settings.time_scale as f64,
                );
                if let Err(error) = result {
                    self.status = error;
                }
            }
            Action::FluidCacheSave => self.show_files(FilePurpose::CacheSave),
            Action::FluidCacheLoad => self.show_files(FilePurpose::CacheLoad),
            Action::FluidCacheClear => {
                self.fluid_preview.cancel();
                self.fluid_preview.cache.clear();
                self.status = "Кэш жидкости очищен".into();
            }
            Action::FluidBakeCancel => {
                self.fluid_preview.cancel();
                self.status = "Запекание отменено; предыдущий кэш сохранён".into();
            }
            Action::Fluid => {
                self.fluid_preview.cancel();
                self.sync_fluid_settings();
                self.fluid_on = !self.fluid_on;
                if self.fluid_on {
                    self.editor.mode = EditorMode::Animation;
                }
                self.status = if self.fluid_on {
                    "Жидкость подключена к таймлайну. Нажмите ▶ или пробел для запуска."
                } else {
                    "Жидкость отключена от таймлайна."
                }
                .into();
            }
            Action::ResetFluid => {
                self.editor.playing = false;
                self.set_time(0.);
                self.reset_fluid_preview();
            }
            Action::FluidFrame => {
                let config = self.fluid.config();
                self.target = (config.bounds_min + config.bounds_max) * 0.5;
                self.distance = (config.bounds_max - config.bounds_min).length().max(0.5) * 1.5;
                self.pitch = 0.25;
                self.yaw = 0.55;
            }
            Action::FluidSurface => {
                if self.job.is_some() {
                    return;
                }
                self.editor.playing = false;
                self.fluid_preview.cancel();
                self.sync_fluid_settings();
                let settings = self.editor.project.fluid_settings;
                let world = self.fluid.clone();
                let (tx, rx) = mpsc::channel();
                let cancel = Arc::new(AtomicBool::new(false));
                let c = cancel.clone();
                std::thread::spawn(move || {
                    let result = poseforge_simulation::extract_surface_cancellable(
                        &world,
                        settings.surface_cell_size,
                        settings.surface_iso_density,
                        &c,
                    );
                    if !c.load(Ordering::Relaxed) {
                        let _ = tx.send(match result {
                            Ok(mesh) => JobMessage::Surface(mesh),
                            Err(e) => JobMessage::Error(e.to_string()),
                        });
                    }
                });
                self.job = Some(Job {
                    receiver: rx,
                    cancel,
                    label: "Поверхность".into(),
                });
            }
            Action::SetCamera => {
                let v = self.view();
                let r = self.editor.transact("Камера", |p| {
                    p.camera.eye = v.eye;
                    p.camera.target = v.target;
                    p.camera.up = v.up;
                    p.camera.fov_y_radians = v.fov_y.clamp(0.011, 3.129);
                    p.camera.orthographic = v.orthographic.is_some();
                    p.camera.ortho_size = v.orthographic.unwrap_or(3.);
                });
                self.result(r);
            }
            Action::FileEntry(i) => {
                if let Some(d) = &mut self.dialog {
                    if let Some((p, is_dir)) = d.entries.get(i).cloned() {
                        d.path = p.to_string_lossy().into();
                        if is_dir {
                            d.refresh();
                        }
                    }
                }
            }
            Action::ParentDir => {
                if let Some(d) = &mut self.dialog {
                    if let Some(p) = d.directory().parent() {
                        d.path = p.to_string_lossy().into();
                        d.refresh();
                    }
                }
            }
            Action::EditPath => {
                if let Some(d) = &self.dialog {
                    self.input(a, d.path.clone());
                }
            }
            Action::FileSubmit => {
                if let Some(d) = self.dialog.take() {
                    let p = PathBuf::from(&d.path);
                    let ext = p
                        .extension()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let valid = match d.purpose {
                        FilePurpose::Environment => {
                            matches!(ext.as_str(), "hdr" | "png" | "jpg" | "jpeg") && p.is_file()
                        }
                        FilePurpose::Import => {
                            matches!(ext.as_str(), "glb" | "gltf" | "blend" | "obj") && p.is_file()
                        }
                        FilePurpose::Texture => {
                            matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "tga" | "bmp" | "dds" | "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" | "ogv" | "gif")
                                && p.is_file()
                        }
                        FilePurpose::CacheSave => ext == "afcache",
                        FilePurpose::CacheLoad => ext == "afcache" && p.is_file(),
                        FilePurpose::Rig => ext == "txt" && p.is_file(),
                        FilePurpose::Open => {
                            matches!(ext.as_str(), "artforge" | "poseforge" | "json") && p.is_file()
                        }
                        FilePurpose::Save => matches!(ext.as_str(), "artforge" | "poseforge"),
                        FilePurpose::Export => ext == "png",
                        FilePurpose::Sequence => !p.is_file(),
                    };
                    if !valid {
                        self.status="Неподходящий файл: модель BLEND/GLB/glTF/OBJ, текстура PNG/JPEG/TGA/BMP/DDS, проект .artforge или .poseforge.".into();
                        self.dialog = Some(d);
                    } else if p.exists()
                        && matches!(
                            d.purpose,
                            FilePurpose::CacheSave
                                | FilePurpose::Save
                                | FilePurpose::Export
                                | FilePurpose::Sequence
                        )
                    {
                        self.overwrite = Some((p, d.purpose));
                    } else {
                        self.dispatch_file(d.purpose, p);
                    }
                }
            }
            Action::ConfirmOverwrite => {
                if let Some((p, purpose)) = self.overwrite.take() {
                    self.dispatch_file(purpose, p);
                }
            }
            Action::SetKey => {
                let r = self.editor.resolve_staged(StagedResolution::Commit);
                if self.result(r).is_some() {
                    if let Some(t) = self.pending_time.take() {
                        self.set_time(t);
                    }
                }
            }
            Action::Discard => {
                let r = self.editor.resolve_staged(StagedResolution::Discard);
                self.result(r);
                if let Some(t) = self.pending_time.take() {
                    self.set_time(t);
                }
            }
            Action::Cancel => {
                if self.fluid_preview.baking.is_some() {
                    self.fluid_preview.cancel();
                    self.status = "Запекание жидкости отменено".into();
                } else if self.input.is_some() {
                    self.end_input();
                } else if self.overwrite.is_some() {
                    self.overwrite = None;
                } else if self.dialog.is_some() {
                    self.dialog = None;
                    self.import_focus = None;
                } else if self.pending_time.is_some() {
                    self.pending_time = None;
                } else if self.character_panel {
                    if self.rig_picker.is_some() {
                        self.rig_picker = None;
                    } else {
                        self.character_panel = false;
                    }
                } else if self.animation_panel {
                    self.animation_panel = false;
                } else if self.menu.is_some() {
                    self.menu = None;
                } else if self.drag.is_some() {
                    self.cancel_drag();
                } else if let Some(j) = &self.job {
                    j.cancel.store(true, Ordering::Relaxed);
                }
                self.help = false;
                self.poses = false;
                self.details = false;
                self.rig_details = false;
                self.update_dirty();
                self.release_drag_pointer();
            }
            Action::ConfirmInput => {
                self.confirm_input();
            }
            Action::Help => self.help = true,
            Action::CloseHelp => self.help = false,
            Action::ShowWelcome | Action::CloseWelcome | Action::Language(_) | Action::OpenRecent(_) => unreachable!("welcome action routed earlier"),
        }
        self.prune_animation_selection();
        self.sync_fluid_settings();
        self.redraw();
    }
    fn key_times(&self) -> Vec<f64> {
        self.editor
            .active_clip
            .and_then(|i| self.editor.project.clips.get(i))
            .map(|clip| {
                clip.key_times(if self.animation.focused && !self.timeline.only_selected {
                    None
                } else {
                    self.editor.selected
                })
            })
            .unwrap_or_default()
    }
    fn scrub(&mut self) {
        let frame = self.timeline_layout().frame_at(self.mouse[0]).round();
        self.set_time(frame / self.editor.project.fps as f64);
    }
    fn mouse_down(&mut self, button: MouseButton) {
        if self.swatch_menu_pointer(button) { return; }
        if self.welcome_active() {
            if button==MouseButton::Left {if let Some(action)=self.ui.hit(self.mouse) {self.welcome.focus=Some(action.clone());self.action(action);}}
            return;
        }
        self.camera_effect_pointer_focus();
        // A second mouse button must not replace or finish the active gesture.
        if self.drag.is_some() || self.native_picker.is_some() {
            return;
        }
        if self.path_pick_pointer(button) {return;}
        if self.rig_reference_pointer(button) { return; }
        if self.collision_pointer(button) { return; }
        if self.dropdown_is_open(ui::DropdownKind::AssetLibrary) {
            self.asset_popup_pointer_down(button);
            return;
        }
        if button == MouseButton::Left { self.orbit_click = None; }
        if button != MouseButton::Left
            || !matches!(self.ui.hit(self.mouse), Some(Action::Slide(..)))
        {
            self.slider_click = None;
        }
        if self.authoring.knife.is_some() && button != MouseButton::Left {
            self.cancel_knife();
        }
        if self.uv_middle(button) {
            self.redraw();
            return;
        }
        if self.node_middle(button) {
            self.redraw();
            return;
        }
        if self.dropdown_pointer_down(button) {
            return;
        }
        if self.begin_swatch_pointer(button) { return; }
        if self.color_pointer_down(button) {
            self.redraw();
            return;
        }
        if button == MouseButton::Left {
            if self.extend_bone_selection_at_pointer() { return; }
            if !self.has_modal() {
                self.animation.focused = self.animation_panel_visible()
                    && ui::contains(self.timeline.dock_rect(self.width, self.height), self.mouse);
            }
            if let Some(action) = self.ui.hit(self.mouse).filter(|action| {
                self.input
                    .as_ref()
                    .is_some_and(|input| input.purpose.search_clear().as_ref() == Some(action))
            }) {
                self.action(action);
                return;
            }
            if let Some(rect) = self
                .active_field_rect()
                .filter(|rect| ui::contains(*rect, self.mouse))
            {
                if let Some(input) = &mut self.input {
                    input.click(rect, self.mouse, self.modifiers.shift_key());
                    self.text_selecting = true;
                }
                return;
            }
            if let Some(action) = self.ui.hit(self.mouse) {
                if let Action::ViewAxis(axis) = action {
                    self.action(Action::ViewOrbit);
                    if matches!(
                        self.drag,
                        Some(Drag::ViewWidget { .. } | Drag::CameraNavigate(_))
                    ) {
                        self.view_axis_press = Some((axis, self.mouse));
                    }
                    return;
                }
                if self.begin_creation_panel_drag(&action) { return; }
                if self.begin_scene_drag(&action) {
                    return;
                }
                if self.begin_material_drag(&action) {
                    return;
                }
                self.action(action);
                return;
            }
        }
        if self.input.is_some() {
            self.resolve_input();
            return;
        }
        if self.menu.is_some() || self.dropdown.is_some() || self.color_picker.is_some() {
            return;
        }
        if self.scene_write_pending() {
            return;
        }
        if self.dialog.is_some()
            || self.overwrite.is_some()
            || self.help
            || self.poses
            || self.details
            || self.rig_details
            || self.character_panel
            || self.animation_panel
            || self.pending_time.is_some()
        {
            return;
        }
        if button == MouseButton::Left && self.timeline_pointer_down() {
            return;
        }
        if self.modeling_live.is_some() {
            return;
        }
        if !self.canvas_input_at(self.mouse) {
            return;
        }
        if button == MouseButton::Left
            && (self.light_aim_pointer()
                || self.light_shape_pointer_down()
                || self.area_pointer_down())
        {
            self.redraw();
            return;
        }
        if button == MouseButton::Left && self.focus_pointer_down() {
            self.redraw();
            return;
        }
        if button == MouseButton::Left && self.weight_pointer_down() {
            self.redraw();
            return;
        }
        if button == MouseButton::Left && self.curve_pointer_down() { self.redraw(); return; }
        if button == MouseButton::Left && self.mesh_pointer_down() {
            self.redraw();
            return;
        }
        if button == MouseButton::Left && self.empty_pointer_down() {return;}
        if button == MouseButton::Left && self.camera_pointer_down() {
            return;
        }
        if button == MouseButton::Left && self.light_pointer_down() {
            return;
        }
        if button == MouseButton::Middle {
            if self.begin_camera_navigation(true, button) {
                return;
            }
            self.leave_camera_view();
            self.drag = Some(Drag::Pan {
                start: self.mouse,
                target: self.target,
            });
            self.drag_button = Some(button);
            return;
        }
        if button == MouseButton::Right || (button == MouseButton::Left && self.tool == Tool::Orbit)
        {
            self.orbit_click = (button == MouseButton::Left).then_some(self.mouse);
            if self.begin_camera_navigation(false, button) {
                return;
            }
            self.leave_camera_view();
            self.drag = Some(Drag::Orbit {
                start: self.mouse,
                yaw: self.yaw,
                pitch: self.pitch,
            });
            self.drag_button = Some(button);
            return;
        }
        if button != MouseButton::Left { return; }
        if self.editor.mode == EditorMode::Render {
            if let Ok(eval) = self.evaluated_viewport() {
                if crate::picking::pick_mesh(&self.picking_project(), &eval, self.view().view_projection(), self.viewport(), self.mouse).is_none()
                    && !self.modifiers.shift_key() && !self.modifiers.control_key() {
                    self.clear_scene_selection();
                }
            }
            return;
        }
        if let Some(handle) = self
            .selected_gizmo()
            .and_then(|gizmo| gizmo.hit(self.mouse))
        {
            self.begin_gizmo_drag(handle);
            return;
        }
        let Ok(eval) = self.evaluated_viewport() else {
            return;
        };
        if let Some(id) = self.pick_viewport_node(&eval) {
            self.select_in_viewport(id);
        } else if !self.modifiers.shift_key() && !self.modifiers.control_key() {
            self.clear_scene_selection();
        }
    }
    fn node_selectable(&self, id: u64) -> bool {
        if !self.editor.project.node_selectable(id) || !self.in_isolation(id) {
            return false;
        }
        let mut cursor = Some(id);
        let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return false;
            }
            let Some(node) = self.editor.project.node(id) else {
                return false;
            };
            if !node.visible || node.locked {
                return false;
            }
            cursor = node.parent;
        }
        true
    }
    fn select_in_viewport(&mut self, id: u64) {
        if !self.node_selectable(id) {
            return;
        }
        self.clear_light_selection();
        self.select_object(id);
        self.sync_active_character(id);
        self.sync_rig_object_selection();
        let name = self
            .editor
            .project
            .node(id)
            .map(|node| node.name.as_str())
            .unwrap_or("");
        self.status = format!(
            "{} · {}",
            name,
            if self.tool == Tool::Rotate {
                "потяните кольцо X/Y/Z или внешнее кольцо по виду"
            } else {
                "потяните рукоятку гизмо для изменения"
            }
        );
    }
    fn mouse_move(&mut self, p: [f32; 2]) {
        if self.orbit_click.is_some_and(|start| (p[0] - start[0]).hypot(p[1] - start[1]) > 3.) {
            self.orbit_click = None;
        }
        let pending_click = self.defer_slider_drag(p);
        self.move_slider_click(p);
        if self.drag.is_some() {
            self.performance.changed();
        }
        self.mouse = p;
        if self
            .view_axis_press
            .is_some_and(|(_, start)| (p[0] - start[0]).hypot(p[1] - start[1]) > 3.)
        {
            self.view_axis_press = None;
        }
        self.dropdown_pointer_move();
        self.collision_pointer_move();
        if pending_click {
            self.redraw();
            return;
        }
        if self.text_selecting {
            if let Some(rect) = self.active_field_rect() {
                if let Some(input) = &mut self.input {
                    input.drag(rect, p);
                }
            }
            self.redraw();
            return;
        }
        match self.drag.clone() {
            Some(Drag::Swatch(drag)) => self.update_swatch_drag(drag),
            Some(Drag::CreationPanel { start, origin, original, moved }) => self.update_creation_panel_drag(start, origin, original, moved),
            Some(Drag::Material(_)) => {}
            Some(Drag::Scene(_)) => self.scroll_scene_drag(),
            Some(Drag::CameraNavigate(drag)) => self.update_camera_navigation(drag, p),
            Some(Drag::ObjectScroll { start, value }) => {
                self.update_object_scroll(start, value, p[1])
            }
            Some(Drag::AnimationLayersScroll { start, value }) => {
                self.update_animation_layers_scroll(start, value, self.mouse[1]);
            }
            Some(Drag::MaterialScroll { start, value }) => {
                self.update_material_scroll(start, value, p[1])
            }
            Some(Drag::ViewWidget { navigation, start, yaw, pitch, target, distance, .. }) => {
                let fine = if self.modifiers.shift_key() { 0.15 } else { 1. };
                match navigation {
                    1 => {
                        let view = self.view();
                        let forward = (view.target - view.eye).normalize_or_zero();
                        let right = forward.cross(view.up).normalize_or_zero();
                        let up = right.cross(forward);
                        self.target = target + (-right * (p[0] - start[0]) + up * (p[1] - start[1])) * distance * 0.0015 * fine;
                    }
                    2 => self.distance = (distance * ((p[1] - start[1]) * 0.01 * fine).exp()).clamp(0.05, 10000.),
                    _ => {
                        self.yaw = yaw - (p[0] - start[0]) * 0.008;
                        self.pitch = (pitch + (p[1] - start[1]) * 0.008).clamp(-1.56, 1.56);
                    }
                }
            }
            Some(Drag::Node(drag)) => self.update_node_drag(drag, p),
            Some(Drag::NodePan(drag)) => self.update_node_pan(drag, p),
            Some(Drag::NodeScroll { start, value }) => self.update_node_scroll(start, value, p[1]),
            Some(Drag::Weights(drag)) => self.update_weight_stroke(drag, p),
            Some(Drag::Uv(drag)) => self.update_uv_drag(drag, p),
            Some(Drag::UvPan(drag)) => self.update_uv_pan(drag, p),
            Some(Drag::Curve(drag)) => self.update_curve_drag(drag, p),
            Some(Drag::Mesh(drag)) => self.update_mesh_drag(drag, p),
            Some(Drag::ComponentSelection(drag)) => self.update_component_selection(drag, p),
            Some(Drag::LiveParameter(drag)) => self.update_live_parameter(drag, p),
            Some(Drag::Color(drag)) => self.update_color_drag(drag, p),
            Some(Drag::Slider(drag)) => self.update_slider(drag, p),
            Some(Drag::Keys(drag)) => self.update_key_drag(drag, p),
            Some(Drag::KeyBox(drag)) => self.update_key_box(drag, p),
            Some(Drag::Light(drag)) => self.update_light_drag(drag, p),
            Some(Drag::Area(drag)) => self.update_area_drag(drag, p),
            Some(Drag::LightShape(drag)) => self.update_light_shape_drag(drag, p),
            Some(Drag::Group(drag)) => self.update_group_drag(drag, p),
            Some(Drag::Panel(drag)) => self.update_panel_resize(drag, p),
            Some(Drag::Orbit { start, yaw, pitch }) => {
                self.yaw = yaw - (p[0] - start[0]) * 0.008;
                self.pitch = (pitch + (p[1] - start[1]) * 0.008).clamp(-1.56, 1.56);
            }
            Some(Drag::Pan { start, target }) => {
                let v = self.view();
                let forward = (v.target - v.eye).normalize();
                let right = forward.cross(Vec3::Y).normalize();
                let up = right.cross(forward);
                self.target = target
                    + (-right * (p[0] - start[0]) + up * (p[1] - start[1]))
                        * self.distance
                        * 0.0015;
            }
            Some(Drag::Timeline) => self.scrub(),
            Some(Drag::RigPole(drag)) => self.update_rig_pole(drag, p),
            Some(Drag::Gizmo {
                id,
                mut gesture,
                curve,
                ik_chain,
                pole,
            }) => {
                if let Some((transform, target, feedback)) =
                    gesture.update(p, self.modifiers.shift_key(), self.modifiers.control_key())
                {
                    let result = if gesture.at_start(transform, target) {
                        // Returning to the initial pose also restores preexisting
                        // staged values; it must not create an identity key/Undo.
                        self.editor.cancel_edit();
                        self.editor.begin_edit(match self.tool {
                            Tool::Move => "Перемещение",
                            Tool::Rotate => "Поворот",
                            Tool::Scale => "Масштаб",
                            Tool::Orbit => "Трансформация",
                        });
                        Ok(())
                    } else if let Some([root, middle, end]) = ik_chain {
                        self.editor.apply_ik(root, middle, end, target, pole)
                    } else if let Some(path) = &curve {
                        self.translate_object_on_curve(id, path, target)
                    } else {
                        self.editor.set_transform(id, transform)
                    };
                    if self.result(result).is_some() {
                        self.status = if curve.is_some() {
                            let position = self.editor.project.node(id).and_then(|n| n.curve_binding).map_or(0., |b| b.position);
                            format!("Объект по кривой · {:.1} % · Esc — отмена", position * 100.)
                        } else { feedback };
                    }
                }
                self.drag = Some(Drag::Gizmo {
                    id,
                    gesture,
                    curve,
                    ik_chain,
                    pole,
                });
            }
            Some(Drag::Scrollbar { start, value }) => {
                let max = self.inspector_max_scroll();
                let track = self.properties_scrollbar_rect()[3];
                let thumb = (track * track / (track + max)).max(28.);
                self.inspector_scroll =
                    (value + (p[1] - start) * max / (track - thumb).max(1.)).clamp(0., max);
                self.drag = Some(Drag::Scrollbar { start, value });
            }
            None => {}
        }
        self.redraw();
    }
    fn mouse_up(&mut self, button: MouseButton) {
        self.node_pointer_up(button);
        self.dropdown_pointer_up();
        if button == MouseButton::Left {
            self.text_selecting = false;
            if let Some(input) = &mut self.input {
                input.end_drag();
            }
        }
        if self.drag_button.is_some_and(|owner| owner != button) {
            return;
        }
        if button == MouseButton::Left {
            if let Some((axis, _)) = self.view_axis_press.take() {
                self.cancel_drag();
                self.click_view_axis(axis);
                self.redraw();
                return;
            }
        }
        // Finish before releasing capture; a subsequent focus event is harmless.
        let drag = self.drag.take();
        self.drag_button = None;
        self.release_drag_pointer();
        let orbit_click = self.orbit_click.take().is_some();
        if button == MouseButton::Left && orbit_click
            && matches!(&drag, Some(Drag::Orbit { .. } | Drag::CameraNavigate(_)))
            && (self.editor.mode == EditorMode::Scene
                || (!self.modifiers.shift_key() && !self.modifiers.control_key()))
        {
            if let Ok(eval) = self.evaluated_viewport() {
                let hit = self.pick_viewport_node(&eval);
                if let Some(id) = hit { self.select_in_viewport(id); }
                else if !self.modifiers.shift_key() && !self.modifiers.control_key() {
                    self.clear_scene_selection();
                }
            }
        }
        match drag {
            Some(Drag::Swatch(drag)) => self.finish_swatch_drag(drag),
            Some(Drag::CreationPanel { moved, .. }) => {
                if moved { self.persist_preferences(); }
                else { self.primitive_action(ui::PrimitiveAction::Collapse); }
            }
            Some(Drag::Curve(drag)) => self.finish_curve_drag(drag),
            Some(Drag::ComponentSelection(drag)) => self.finish_component_selection(drag),
            Some(Drag::LiveParameter(_)) => {}
            Some(Drag::Panel(_)) => self.persist_preferences(),
            Some(Drag::Material(drag)) => self.finish_material_drag(drag),
            Some(Drag::Scene(drag)) => self.finish_scene_drag(drag),
            Some(Drag::Keys(drag)) => self.finish_key_drag(drag),
            Some(Drag::KeyBox(drag)) => self.finish_key_box(drag),
            Some(
                Drag::CameraNavigate(_)
                | Drag::Node(_)
                | Drag::Weights(_)
                | Drag::Uv(_)
                | Drag::Mesh(_)
                | Drag::Gizmo { .. }
                | Drag::RigPole(_)
                | Drag::Slider(_)
                | Drag::Light(_)
                | Drag::Area(_)
                | Drag::LightShape(_)
                | Drag::Group(_),
            ) => {
                let r = self.editor.commit_edit();
                self.result(r);
            }
            _ => {}
        }
        self.prune_animation_selection();
        self.sync_fluid_settings();
    }
    fn build_ui(&mut self) -> Ui {
        self.prune_path_edit();
        if self.camera_view.is_some_and(|id| {
            self.editor
                .project
                .node(id)
                .is_none_or(|n| n.camera.is_none())
        }) {
            self.camera_view = None;
        }
        self.prune_primitive_panel();
        self.prune_scene_tools();
        self.prepare_material_previews();
        self.nodes_prune();
        self.authoring_prune();
        self.uv_prune();
        if matches!(self.editor.mode, EditorMode::Pose | EditorMode::Animation) {
            self.sync_rig_pose();
        }
        self.sync_animation_context();
        // Contextual tools can shrink the inspector between input events.
        // Keep painting and inverse pointer mapping on the same scroll value.
        self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
        let palette = self.preferences.palette();
        let mut u = Ui::with_theme(self.scale, self.preferences.theme, self.mouse);
        u.accent = self.preferences.accent;
        u.language = self.preferences.language;
        if let Some(Drag::Slider(slider)) = &self.drag {
            u.set_slider_range(slider.action.clone(), slider.range);
        }
        if let Some((action, range)) = self.live_slider_ui_range() {
            u.set_slider_range(action, range);
        }
        let w = self.width;
        let h = self.height;
        let v = self.viewport();
        let [rx, _, rw, _] = self.right_panel_rect();
        // One quiet app bar; secondary actions live in anchored popovers.
        u.rect([0., 0., w, 60.], palette.panel);
        u.rect([0., 59., w, 1.], palette.border);
        u.quiet_icon_button(
            [12., 12., 92., 36.],
            "Проект",
            Action::ToggleProjectMenu,
            self.menu == Some(crate::chrome::Menu::Project),
            Icon::File,
        );
        for (x, enabled, action, icon) in [
            (112., self.editor.can_undo(), Action::Undo, Icon::Undo),
            (156., self.editor.can_redo(), Action::Redo, Icon::Redo),
        ] {
            if enabled {
                u.quiet_icon_button([x, 12., 36., 36.], "", action, false, icon);
            } else {
                u.disabled_icon_button([x, 12., 36., 36.], "", icon);
            }
        }
        let hx = w - 200.;
        u.raw_fitted_label(
            [208., 23., (hx - 224.).max(0.), 18.],
            &format!(
                "{}{}",
                self.editor.project.title,
                if self.dirty { " *" } else { "" }
            ),
            12.,
            palette.muted,
        );
        u.quiet_icon_button(
            [hx, 12., 36., 36.],
            "",
            Action::ToggleLeftPanel,
            self.scene_panel_visible(),
            Icon::Scene,
        );
        u.quiet_icon_button(
            [hx + 44., 12., 36., 36.],
            "",
            Action::ToggleAnimationPanel,
            self.animation_panel_visible(),
            Icon::Sequence,
        );
        u.quiet_icon_button(
            [hx + 88., 12., 100., 36.],
            "Свойства",
            Action::ToggleRightPanel,
            self.preferences.right_panel,
            Icon::Settings,
        );
        self.draw_mesh_overlay(&mut u);
        self.draw_fluid_container(&mut u);
        self.draw_weights(&mut u);

        if self.editor.mode == EditorMode::Animation {
            self.animation_overlay.draw(
                &mut u,
                &self.editor.project,
                self.editor.active_clip,
                self.editor.selected,
                self.active_rig_root,
                self.editor.time,
                self.animation_preview_range(),
                &self.view(),
                v,
            );
        }
        // Draw skeleton first so floating tools always remain readable.
        if self.preferences.skeleton && self.editor.mode != EditorMode::Render
            && self.editor.project.nodes.iter().any(|n| n.is_joint)
        {
            if let Ok(evaluated) = self.evaluated_interactive() {
                let bones = crate::bones::BoneOverlay::new(
                    &self.picking_project(),
                    &evaluated,
                    &self.view(),
                    v,
                );
                let hovered = if self.drag.is_none()
                    && self.canvas_input_at(self.mouse)
                    && self
                        .selected_gizmo()
                        .and_then(|g| g.hit(self.mouse))
                        .is_none()
                {
                    bones.hit(self.mouse)
                } else {
                    None
                };
                let selected = self.object_selection();
                bones.draw_selected(&mut u, &selected, hovered);
            }
        }
        self.draw_destruction_floor(&mut u);
        self.draw_curves(&mut u);
        self.draw_rig_handles(&mut u);
        self.draw_normal_directions(&mut u);
        if let Some(Drag::Gizmo { gesture, .. }) = &self.drag {
            if gesture.gizmo.mode == crate::gizmo::Mode::Rotate {
                // The drag solver owns a fixed basis. Keep its rings and angle
                // guide in that same basis while the selected joint is rotating.
                gesture.gizmo.draw(
                    &mut u,
                    None,
                    Some(gesture.handle),
                    self.axis,
                    self.rail_rect(),
                );
                gesture.draw_feedback(&mut u, self.rail_rect());
            } else if let Some(gizmo) = self.selected_gizmo() {
                gizmo.draw(
                    &mut u,
                    None,
                    Some(gesture.handle),
                    self.axis,
                    self.rail_rect(),
                );
            }
        } else if let Some(gizmo) = self.selected_gizmo() {
            let hovered = if self.canvas_input_at(self.mouse) {
                gizmo.hit(self.mouse)
            } else {
                None
            };
            gizmo.draw(&mut u, hovered, None, self.axis, self.rail_rect());
        }
        self.draw_empty_overlay(&mut u);
        self.draw_camera_overlay(&mut u);
        self.draw_light_controls(&mut u);
        self.draw_light_shape_handles(&mut u);
        self.draw_path_offset_guide(&mut u);
        let mut covers = vec![
            self.left_panel_rect(),
            self.rail_rect(),
            self.view_widget_rect(),
            self.creation_toolbar_rect(),
        ];
        if self.preferences.right_panel {
            covers.push(self.right_panel_rect());
        }
        for cover in covers {
            let [x, y, w, h] = cover.map(|v| v * u.scale);
            u.texts.retain(|t| {
                t.x >= x + w
                    || t.x + t.text.chars().count() as f32 * t.size <= x
                    || t.y >= y + h
                    || t.y + t.size * 1.5 <= y
            });
        }
        self.draw_scene_tree(&mut u);
        self.draw_tool_rail(&mut u);
        self.draw_view_widget(&mut u);
        if self.preferences.right_panel {
            u.surface(self.right_panel_rect(), palette.panel);
            self.draw_properties_navigation(&mut u);
            let rx = rx + properties::RAIL_WIDTH;
            let rw = rw - properties::RAIL_WIDTH;
            let content_start = (u.rects.len(), u.texts.len(), u.hits.len());
            let scroll = self.inspector_scroll.min(self.inspector_max_scroll());
            u.set_cursor([
                self.mouse[0],
                self.property_cursor_y(self.mouse[1] + scroll + properties::CONTENT_SHIFT),
            ]);
            if self.surface.tab == 8 {
                self.draw_modifier_panel(&mut u, rx, rw);
            } else if self.surface.tab == 12 {
                self.draw_uv_properties(&mut u, rx, rw);
            } else if self.surface.tab == 10 {
                self.draw_normals_panel(&mut u, rx, rw);
            } else if self.surface.tab == 9 {
                self.draw_program_settings(&mut u, rx, rw);
            } else if matches!(self.surface.tab, 4 | 6 | 11 | 13) {
                self.draw_physics_workspace(&mut u, rx, rw);
            } else if self.surface.tab == 7 {
                self.draw_authoring_panel(&mut u, rx, rw);
            } else if self.surface.tab == 3 {
                self.draw_render_settings(&mut u, rx, rw);
            } else if self.surface.tab != 0 || self.active_light_index().is_some() {
                self.draw_surface_panel(&mut u, rx, rw);
            } else if self.selected_camera_id().is_some() {
                self.draw_camera_panel(&mut u, rx, rw);
            } else {
                if let Some(n) = self
                    .editor
                    .selected
                    .and_then(|id| self.editor.project.node(id))
                {
                    u.field([rx + 16., 171., rw - 32., 32.], &n.name, Action::EditName);
                    u.label(
                        rx + 16.,
                        215.,
                        if n.is_joint {
                            "Сустав  /  локальные оси"
                        } else {
                            "Объект  /  локальные оси"
                        },
                        11.,
                        palette.muted,
                    );
                    if let Some(t) = self.editing_transform(n.id) {
                        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        let values = [
                            t.translation.to_array(),
                            [x.to_degrees(), y.to_degrees(), z.to_degrees()],
                            t.scale.to_array(),
                        ];
                        for (row, label) in ["Положение, м", "Вращение, градусы", "Масштаб"]
                            .iter()
                            .enumerate()
                        {
                            let y = 248. + row as f32 * 72.;
                            u.label(rx + 16., y, *label, 11., palette.muted);
                            for i in 0..3 {
                                let bx = rx + 16. + i as f32 * (rw - 26.) / 3.;
                                let value = values[row][i] as f64;
                                let range = match row {
                                    0 => SliderRange::new(
                                        -10_f64.max(value.abs() * 2.),
                                        10_f64.max(value.abs() * 2.),
                                        0.01,
                                    ),
                                    1 => SliderRange::new(-180., 180., 0.1),
                                    _ => SliderRange::new(0.001, 5_f64.max(value * 2.), 0.01),
                                };
                                u.number_field(
                                    [bx, y + 22., (rw - 42.) / 3., 44.],
                                    ["X", "Y", "Z"][i],
                                    value,
                                    Action::EditTransform(row * 3 + i),
                                    range,
                                );
                            }
                        }
                    }
                    u.label(rx + 16., 470., "Выделенная ось гизмо", 11., palette.muted);
                    for i in 0..3 {
                        u.button(
                            [
                                rx + 16. + i as f32 * (rw - 24.) / 3.,
                                492.,
                                (rw - 48.) / 3.,
                                28.,
                            ],
                            ["X", "Y", "Z"][i],
                            Action::SetAxis(i),
                            self.axis == i,
                        );
                    }
                    if n.curve.is_some() { self.draw_curve_properties(&mut u, rx, rw); }
                    if n.empty {self.draw_helper_properties(&mut u,rx,rw);}
                    if n.text.is_some() {self.draw_text_properties(&mut u,rx,rw);}
                    else if self.rig_context() {
                        self.draw_rig_workflow(&mut u, rx + 16., rig_workflow::RIG_OBJECT_START, rw - 32.);
                    } else if n.mesh.is_some() && n.curve.is_none() {
                        let rect = [rx + 16., 534., rw - 32., 28.];
                        if self.node_selectable(n.id) && !self.scene_write_pending() && !self.editor.has_staged() {
                            u.checkbox(rect, "Только тень", Action::SceneShadowOnly(n.id), n.shadow_only);
                        } else {
                            u.disabled_button(rect, "Только тень");
                        }
                        u.fitted_label([rx + 16., 574., rw - 32., 18.], "Скрыть меш, сохранить его тени", 11., palette.muted);
                    }
                } else {
                    u.label(
                        rx + 16.,
                        178.,
                        "Выберите объект или сустав",
                        12.,
                        palette.muted,
                    );
                }
            }
            if self.surface.tab == 0 { self.draw_path_panel(&mut u, rx, rw); }
            self.draw_property_sections(&mut u, content_start);
            // Clip both drawing and hit regions; scrolling never exposes hidden controls.
            u.set_cursor(self.mouse);
            clip_scrolled(
                &mut u,
                content_start,
                self.properties_content_rect(),
                scroll + properties::CONTENT_SHIFT,
            );
            if self.inspector_max_scroll() > 0. {
                let track = self.properties_scrollbar_rect()[3];
                let thumb = (track * track / (track + self.inspector_max_scroll())).max(28.);
                u.vertical_scrollbar(
                    self.properties_scrollbar_rect(),
                    scroll / self.inspector_max_scroll(),
                    thumb / track,
                    Action::ScrollPanel(0),
                );
            }
        }
        self.draw_creation_toolbar(&mut u);
        if self.animation_panel_visible() {
            self.draw_animation_layers(&mut u);
            self.draw_animation_timeline(&mut u);
        }
        if self.editor.has_staged() {
            let sy = v[1] + v[3] - 40.;
            u.surface([v[0] + 12., sy, v[2] - 24., 32.], palette.panel);
            u.label(v[0] + 22., sy + 9., "Поза изменена", 12., palette.warn);
            u.quiet_button(
                [v[0] + v[2] - 250., sy, 110., 32.],
                "Записать ключ",
                Action::SetKey,
                true,
            );
            u.quiet_button(
                [v[0] + v[2] - 130., sy, 118., 32.],
                "Отбросить",
                Action::Discard,
                false,
            );
        }
        u.rect([0., h - 28., w, 28.], palette.bg);
        let footer = if self.job.is_some() {
            self.status.clone()
        } else {
            match u.hit(self.mouse) {
                Some(Action::KeyPose) => format!(
                    "Ключ всей позы · персонаж: {} · Shift+K",
                    self.active_rig_root
                        .and_then(|id| self.editor.project.node(id))
                        .map(|n| n.name.as_str())
                        .unwrap_or("не настроен")
                ),
                Some(Action::Select(id)) => self
                    .editor
                    .project
                    .node(id)
                    .map(|n| n.name.clone())
                    .unwrap_or_default(),
                Some(Action::SceneSelectLight(_, light)) => {
                    format!("{} · {}", light.name, scene_tree::light_label(light.kind))
                }
                Some(Action::SceneToggleLight(_, light)) => format!(
                    "{} · {}",
                    if light.enabled {
                        "Выключить"
                    } else {
                        "Включить"
                    },
                    light.name
                ),
                Some(ref action) => action.hint().unwrap_or(&self.status).to_string(),
                None => self.status.clone(),
            }
        };
        u.label(
            16.,
            h - 21.,
            shortened(&footer, ((w - 160.) / 6.8) as usize),
            11.,
            palette.muted,
        );
        if let Some(j) = &self.job {
            u.button(
                [w - 130., h - 28., 122., 26.],
                &format!("Отмена: {}", j.label),
                Action::Cancel,
                false,
            );
        }
        if self.job.is_none() {
            u.outline_button([w - 92., h - 30., 80., 24.], "Отчёт", Action::Details, true);
        }
        self.draw_scene_tools(&mut u);
        self.draw_area_handles(&mut u);
        self.draw_component_selection(&mut u);
        self.draw_primitive_panel(&mut u);
        self.draw_modeling_live(&mut u);
        self.draw_advanced_overlay(&mut u);
        self.draw_uv_editor(&mut u);
        self.draw_material_drag(&mut u);
        self.draw_scene_drag(&mut u);
        self.draw_performance(&mut u);
        if let Some(menu) = self.menu {
            crate::chrome::draw_menu(
                &mut u,
                [w, h],
                [v[0], v[1], self.navigation_right() + 12., v[3]],
                menu,
                &self.editor.project,
                self.current_profile(),
                self.editor.selected,
                self.view().orthographic.is_some(),
                self.preferences.grid,
                self.preferences.skeleton,
                [
                    self.preferences.adaptive_preview,
                    self.preferences.performance_overlay,
                    self.preferences.show_backfaces,
                ],
            );
        }
        self.draw_node_editor(&mut u);
        if self.welcome.open { self.draw_welcome(&mut u); }
        if let Some(d) = &self.dialog {
            u.hits.clear();
            u.texts.clear();
            u.rect([0., 0., w, h], [0.015, 0.02, 0.03, 0.82]);
            let x = (w - 780.) / 2.;
            let dialog_height = 540.;
            let y = (h - dialog_height) / 2.;
            u.surface([x, y, 780., dialog_height], palette.panel);
            u.label(
                x + 24.,
                y + 22.,
                match d.purpose {
                    FilePurpose::Environment => "HDRI окружение · Radiance HDR / PNG / JPEG",
                    FilePurpose::Import => "Импорт модели · GLB / glTF / OBJ",
                    FilePurpose::Texture => "Карта · изображение или видео",
                    FilePurpose::Rig => "Список костей · TXT",
                    FilePurpose::Open => "Открыть проект",
                    FilePurpose::CacheSave => "Сохранить кэш жидкости · .afcache",
                    FilePurpose::CacheLoad => "Загрузить кэш жидкости · .afcache",
                    FilePurpose::Save => "Сохранить проект",
                    FilePurpose::Export => "Экспорт PNG",
                    FilePurpose::Sequence => "Папка последовательности PNG",
                },
                22.,
                palette.text,
            );
            u.field([x + 24., y + 66., 524., 36.], &d.path, Action::EditPath);
            u.icon_button(
                [x + 556., y + 66., 92., 36.],
                "Вставить",
                Action::PastePath,
                false,
                Icon::Paste,
            );
            u.icon_button(
                [x + 656., y + 66., 100., 36.],
                "Обзор…",
                Action::BrowsePath,
                false,
                Icon::Folder,
            );
            u.button(
                [x + 24., y + 116., 112., 29.],
                "На уровень ↑",
                Action::ParentDir,
                false,
            );
            u.label(
                x + 148.,
                y + 124.,
                if d.purpose == FilePurpose::Import {
                    "Выберите модель GLB, glTF или OBJ"
                } else {
                    "Путь: Ctrl+V или «Обзор…» для выбора файла / папки"
                },
                11.,
                palette.muted,
            );
            let file_list_y = 163.;
            for (i, (p, dir)) in d.entries.iter().enumerate().skip(d.offset).take(9) {
                let y = y + file_list_y + (i - d.offset) as f32 * 31.;
                u.icon_button(
                    [x + 24., y, 732., 28.],
                    &p.file_name().unwrap_or_default().to_string_lossy(),
                    Action::FileEntry(i),
                    p.to_string_lossy() == d.path,
                    if *dir { Icon::Folder } else { Icon::File },
                );
            }
            u.label(
                x + 24.,
                y + dialog_height - 78.,
                if d.purpose == FilePurpose::Import {
                    shortened(&self.status, 94)
                } else if d.purpose == FilePurpose::Texture {
                    "PNG/JPEG/TGA/BMP/DDS · карта будет встроена в проект".into()
                } else {
                    "Выберите файл или введите полный путь".into()
                },
                11.,
                palette.muted,
            );
            u.button(
                [x + 490., y + dialog_height - 50., 120., 32.],
                "Отмена",
                Action::Cancel,
                false,
            );
            u.button(
                [x + 622., y + dialog_height - 50., 134., 32.],
                "Продолжить",
                Action::FileSubmit,
                true,
            );
            if d.purpose == FilePurpose::Import && self.input.is_none() {
                if let Some(rect) = self.import_focus.as_ref().and_then(|action| {
                    u.hits
                        .iter()
                        .find(|hit| &hit.action == action)
                        .map(|hit| hit.rect)
                }) {
                    u.surface_outline(rect, palette.accent);
                }
            }
        }
        if self.character_panel {
            crate::workflows::draw_character_dialog(
                &mut u,
                w,
                h,
                &self.editor.project,
                &self.rig_draft,
                self.detection.as_ref(),
                self.rig_picker,
                &self.bone_query,
                self.bone_scroll,
                &self.status,
            );
        }
        if self.animation_panel {
            crate::workflows::draw_animation_dialog(
                &mut u,
                w,
                h,
                &self.editor.project,
                self.editor.active_clip,
                self.editor.time,
                self.clip_scroll,
                &self.status,
            );
        }
        if self.pending_time.is_some() {
            u.hits.clear();
            u.texts.clear();
            u.rect([0., 0., w, h], [0.02, 0.025, 0.035, 0.82]);
            let x = (w - 650.) / 2.;
            let y = h / 2. - 100.;
            u.surface([x, y, 650., 200.], palette.panel);
            u.label(
                x + 24.,
                y + 22.,
                "Временная поза ещё не записана",
                22.,
                palette.text,
            );
            u.label(
                x + 24.,
                y + 65.,
                "Выберите, что сделать с изменением перед сменой времени.",
                13.,
                palette.muted,
            );
            u.button(
                [x + 24., y + 126., 182., 36.],
                "Поставить ключ",
                Action::SetKey,
                true,
            );
            u.button(
                [x + 224., y + 126., 182., 36.],
                "Отбросить",
                Action::Discard,
                false,
            );
            u.button(
                [x + 424., y + 126., 202., 36.],
                "Остаться здесь",
                Action::Cancel,
                false,
            );
        }
        if self.poses {
            u.hits.clear();
            u.texts.clear();
            u.rect([0., 0., w, h], [0.02, 0.025, 0.035, 0.87]);
            let x = (w - 660.) / 2.;
            let y = (h - 470.) / 2.;
            u.surface([x, y, 660., 470.], palette.panel);
            u.label(x + 24., y + 24., "Библиотека поз", 24., palette.text);
            u.label(
                x + 24.,
                y + 64.,
                "Позы сохраняются в проекте. Колесо — прокрутка списка.",
                12.,
                palette.muted,
            );
            for (i, p) in self
                .editor
                .project
                .poses
                .iter()
                .enumerate()
                .skip(self.pose_scroll)
                .take(8)
            {
                u.button(
                    [
                        x + 24.,
                        y + 105. + (i - self.pose_scroll) as f32 * 34.,
                        612.,
                        29.,
                    ],
                    &p.name,
                    Action::LoadPose(i),
                    false,
                );
            }
            u.label(
                x + 24.,
                y + 384.,
                "Имя новой позы · Enter — сохранить",
                11.,
                palette.muted,
            );
            u.field(
                [x + 24., y + 414., 340., 34.],
                &format!("Поза {}", self.editor.project.poses.len() + 1),
                Action::SavePose,
            );
            u.button(
                [x + 446., y + 414., 190., 34.],
                "Закрыть",
                Action::Cancel,
                false,
            );
        }
        if self.details {
            u.hits.clear();
            u.texts.clear();
            u.rect([0., 0., w, h], [0.02, 0.025, 0.035, 0.87]);
            let x = (w - 800.) / 2.;
            let y = (h - 450.) / 2.;
            u.surface([x, y, 800., 450.], palette.panel);
            u.label(x + 24., y + 24., "Результат операции", 24., palette.text);
            self.draw_report_copy_feedback(&mut u, [x + 24., y + 56., 752., 18.]);
            let lines = wrapped_report(&self.details_text);
            let pages = lines.len().div_ceil(12);
            let page = self.details_page.min(pages.saturating_sub(1));
            for (i, line) in lines.iter().skip(page * 12).take(12).enumerate() {
                u.label(x + 24., y + 84. + i as f32 * 23., line, 13., palette.text);
            }
            u.label(
                x + 24.,
                y + 366.,
                format!("Страница {} / {}", page + 1, pages),
                11.,
                palette.muted,
            );
            if page > 0 {
                u.button(
                    [x + 24., y + 394., 126., 34.],
                    "Назад",
                    Action::DetailsPage(-1),
                    false,
                );
            }
            if page + 1 < pages {
                u.button(
                    [x + 160., y + 394., 126., 34.],
                    "Далее",
                    Action::DetailsPage(1),
                    false,
                );
            }
            self.draw_report_copy_button(&mut u, [x + 306., y + 394., 270., 34.]);
            u.button(
                [x + 596., y + 394., 180., 34.],
                "Закрыть",
                Action::Cancel,
                true,
            );
        }
        if self.rig_details {
            u.hits.clear();
            u.texts.clear();
            u.rect([0., 0., w, h], [0.02, 0.025, 0.035, 0.87]);
            let x = (w - 920.) / 2.;
            let y = (h - 560.) / 2.;
            u.surface([x, y, 920., 560.], palette.panel);
            u.label(
                x + 24.,
                y + 22.,
                "Проверка списка костей",
                23.,
                palette.text,
            );
            self.draw_report_copy_feedback(&mut u, [x + 24., y + 132., 872., 18.]);
            if let Some(manifest) = &self.rig_manifest {
                let report = manifest.compare(&self.editor.project);
                u.label(
                    x + 24.,
                    y + 60.,
                    format!(
                        "{} · {} костей",
                        shortened(&manifest.armature, 50),
                        manifest.bones.len()
                    ),
                    13.,
                    palette.accent,
                );
                u.label(x + 24., y + 86., report.summary(), 12., palette.text);
                u.label(
                    x + 24.,
                    y + 112.,
                    "TXT содержит имена и связи. Модель и управляющий риг загружаются отдельно.",
                    12.,
                    palette.muted,
                );
                let matched: BTreeMap<_, _> = report
                    .matched
                    .iter()
                    .map(|(n, id)| (n.as_str(), *id))
                    .collect();
                let ambiguous: BTreeSet<_> = report.ambiguous.iter().map(String::as_str).collect();
                let offset = self.rig_scroll.min(manifest.bones.len().saturating_sub(11));
                for (i, bone) in manifest.bones.iter().skip(offset).take(11).enumerate() {
                    let ry = y + 152. + i as f32 * 29.;
                    let id = matched.get(bone.name.as_str());
                    let hierarchy_changed = report
                        .hierarchy_mismatches
                        .iter()
                        .any(|line| line.starts_with(&format!("{}:", bone.name)));
                    let state = if hierarchy_changed {
                        "Связь отличается"
                    } else if id.is_some() {
                        "Найдено"
                    } else if ambiguous.contains(bone.name.as_str()) {
                        "Несколько"
                    } else {
                        "Нет в сцене"
                    };
                    let label = format!(
                        "{}  ·  {}  ·  родитель: {}",
                        state,
                        bone.name,
                        bone.parent.as_deref().unwrap_or("корень")
                    );
                    if let Some(id) = id {
                        u.button([x + 24., ry, 872., 26.], &label, Action::Select(*id), false);
                    } else {
                        u.label(
                            x + 34.,
                            ry + 5.,
                            shortened(&label, 106),
                            12.,
                            if ambiguous.contains(bone.name.as_str()) {
                                palette.warn
                            } else {
                                palette.muted
                            },
                        );
                    }
                }
                u.label(
                    x + 24.,
                    y + 480.,
                    "Колесо — список. Нажмите найденную кость, чтобы выделить её в сцене.",
                    11.,
                    palette.muted,
                );
            } else {
                u.label(
                    x + 24.,
                    y + 100.,
                    "Откройте список TXT с заголовками Armature: и Bones:.",
                    13.,
                    palette.muted,
                );
            }
            u.icon_button(
                [x + 24., y + 510., 210., 32.],
                "Другой список TXT",
                Action::ImportRig,
                false,
                Icon::Import,
            );
            self.draw_report_copy_button(&mut u, [x + 250., y + 510., 270., 32.]);
            u.button(
                [x + 710., y + 510., 186., 32.],
                "Закрыть",
                Action::Cancel,
                true,
            );
        }
        if self.help {
            u.hits.clear();
            u.texts.clear();
            u.rect([0., 0., w, h], [0.02, 0.025, 0.035, 0.87]);
            let x = (w - 740.) / 2.;
            let y = (h - 510.) / 2.;
            u.surface([x, y, 740., 510.], palette.panel);
            u.label(
                x + 28.,
                y + 24.,
                "От первого движения до кадра",
                25.,
                palette.text,
            );
            let lines = [
                "1. «Проект» → «Импорт модели»: BLEND, GLB, glTF или OBJ.",
                "2. «Свойства» → «Материал»: цвет, шероховатость и карты текстур.",
                "3. «Объект» → «Свет»: источники, экспозиция, свет окружения.",
                "4. Гизмо: G / R / S; X / Y / Z — ось, L — локальные оси.",
                "5. Ctrl — привязка; Shift — точное движение; Esc — отмена жеста.",
                "6. Свойства скелета — кости; панель анимации — клипы; Shift+K — полная поза.",
                "ПКМ — орбита. СКМ — панорама. Колесо — масштаб. F — выделенное.",
                "Ctrl+S — сохранить. Ctrl+Z / Ctrl+Y — отмена / повтор. K — ключ.",
                "Ctrl+F — поиск. Tab / Shift+Tab — сцена / свойства.",
                "",
                "Пробел — воспроизведение. Esc — отмена. F1 — эта справка.",
                "«Проект» → «Рендер и экспорт» — PNG; там же отчёт импорта.",
            ];
            for (i, line) in lines.iter().enumerate() {
                u.label(x + 28., y + 80. + i as f32 * 27., *line, 13., palette.text);
            }
            u.button(
                [x + 528., y + 452., 184., 34.],
                "Вернуться к сцене",
                Action::CloseHelp,
                true,
            );
        }
        if let Some((p, _)) = &self.overwrite {
            u.hits.clear();
            u.texts.clear();
            u.rect([0., 0., w, h], [0.02, 0.025, 0.035, 0.87]);
            let x = (w - 700.) / 2.;
            let y = h / 2. - 110.;
            u.surface([x, y, 700., 220.], palette.panel);
            u.label(
                x + 24.,
                y + 22.,
                "Перезаписать существующий результат?",
                22.,
                palette.text,
            );
            u.label(
                x + 24.,
                y + 70.,
                p.to_string_lossy().chars().take(85).collect::<String>(),
                12.,
                palette.muted,
            );
            u.label(
                x + 24.,
                y + 100.,
                "Предыдущий файл или кадры в папке будут заменены.",
                12.,
                palette.danger,
            );
            u.button(
                [x + 260., y + 155., 180., 36.],
                "Отмена",
                Action::Cancel,
                false,
            );
            u.button(
                [x + 456., y + 155., 220., 36.],
                "Перезаписать",
                Action::ConfirmOverwrite,
                true,
            );
        }
        self.draw_dropdown(&mut u);
        self.draw_collision_popup(&mut u);
        self.draw_color_picker(&mut u);
        self.draw_swatch_drag(&mut u);
        self.draw_swatch_menu(&mut u);
        self.draw_environment_library(&mut u);
        if let Some(input) = &mut self.input {
            if let Some(rect) = u
                .hits
                .iter()
                .rev()
                .find(|hit| hit.action == input.purpose && hit.rect[3] >= 22.)
                .map(|hit| hit.rect)
            {
                u.edit_field(rect, input, self.surface.input_error.is_some());
                if let Some(window) = &self.window {
                    window.set_ime_cursor_area(
                        LogicalPosition::new(rect[0], rect[1] + rect[3]),
                        LogicalSize::new(rect[2], 22.),
                    );
                }
                // Field feedback stays in the application footer, including within file/workflow panels.
                u.texts.retain(|text| text.y < (h - 27.) * self.scale);
                u.rect([0., h - 28., w, 28.], palette.bg);
                let message = self.surface.input_error.as_deref().unwrap_or(if input.purpose == Action::MaterialSearch {
                    "Поиск по имени, категории и тегам · ↑/↓ — выбрать · Enter — выбрать · перетаскивание — назначить · Esc — закрыть"
                } else if input.purpose == Action::AssetSearch {
                    "Поиск ресурсов · ↑/↓ — выбрать · Enter — открыть / добавить · Tab — категории / карточки · Esc — закрыть"
                } else { "Enter — применить · Esc — отменить · Tab — следующее поле · Ctrl+A/C/V/X/Z — редактирование" });
                u.fitted_label(
                    [16., h - 21., w - 32., 20.],
                    message,
                    11.,
                    if self.surface.input_error.is_some() {
                        palette.danger
                    } else {
                        palette.muted
                    },
                );
            }
        }
        self.tint_default_resets(&mut u);
        u
    }
    fn scroll(&mut self, delta: f32) {
        if self.welcome_active() || self.swatch_menu_open() {return;}
        if self.collision_scroll(delta) { return; }
        if self.dropdown_is_open(ui::DropdownKind::AssetLibrary) {
            self.asset_popup_scroll(delta);
            return;
        }
        self.slider_click = None;
        if self.modeling_live_scroll(delta) {
            return;
        }
        if self.uv_scroll(delta) {
            return;
        }
        if self.authoring.proportional
            && self.modifiers.alt_key()
            && self.drag.is_none()
            && self.input.is_none()
            && !self.has_modal()
            && self.canvas_input_at(self.mouse)
        {
            self.authoring.proportional_radius =
                (self.authoring.proportional_radius * (delta * 0.12).exp()).clamp(0.0001, 100000.);
            self.redraw();
            return;
        }
        if self
            .primitive_panel_rect()
            .is_some_and(|r| ui::contains(r, self.mouse))
        {
            self.scroll_curve_creation_panel(delta);
            return;
        }
        if self.environment_scroll(delta) {
            return;
        }
        if self.nodes_open() && self.input.is_some() && !self.resolve_input() {
            return;
        }
        if self.material_scroll_wheel(delta) {
            return;
        }
        if self.node_scroll(delta) {
            return;
        }
        if self.dropdown_scroll(delta) {
            return;
        }
        if self.drag.is_some() || self.native_picker.is_some() || self.color_picker.is_some() {
            return;
        }
        if self.input.is_some() && !self.resolve_input() {
            return;
        }
        if !delta.is_finite()
            || self.overwrite.is_some()
            || self.help
            || self.details
            || self.pending_time.is_some()
            || self.menu.is_some()
            || self.dropdown.is_some()
            || self.scene_write_pending()
        {
            return;
        }
        if let Some(dialog) = &mut self.dialog {
            dialog.offset = (dialog.offset as isize - (delta * 3.) as isize)
                .clamp(0, dialog.entries.len().saturating_sub(9) as isize)
                as usize;
        } else if self.character_panel {
            let count = crate::workflows::candidate_nodes(
                &self.editor.project,
                &self.rig_draft,
                self.rig_picker,
                &self.bone_query,
            )
            .len();
            self.bone_scroll = (self.bone_scroll as isize - (delta * 3.) as isize).clamp(
                0,
                count.saturating_sub(crate::workflows::NODE_PAGE_SIZE) as isize,
            ) as usize;
        } else if self.animation_panel {
            self.clip_scroll = (self.clip_scroll as isize - (delta * 2.) as isize).clamp(
                0,
                self.editor
                    .project
                    .clips
                    .len()
                    .saturating_sub(crate::workflows::CLIP_PAGE_SIZE) as isize,
            ) as usize;
        } else if self.rig_details {
            let count = self.rig_manifest.as_ref().map_or(0, |m| m.bones.len());
            self.rig_scroll = (self.rig_scroll as isize - (delta * 3.) as isize)
                .clamp(0, count.saturating_sub(11) as isize) as usize;
        } else if self.poses {
            self.pose_scroll = (self.pose_scroll as isize - (delta * 2.) as isize).clamp(
                0,
                self.editor.project.poses.len().saturating_sub(8) as isize,
            ) as usize;
        } else if self
            .animation_layers_rect()
            .is_some_and(|r| ui::contains(r, self.mouse))
        {
            self.animation_layers_scroll(delta);
        } else if self.animation_panel_visible()
            && ui::contains(self.animation_dock_rect(), self.mouse)
        {
            self.animation.focused = true;
            self.timeline_scroll(delta);
        } else if self.preferences.left_panel && ui::contains(self.left_panel_rect(), self.mouse) {
            self.scene_scroll = (self.scene_scroll as isize - (delta * 2.) as isize).clamp(
                0,
                self.outliner_rows()
                    .len()
                    .saturating_sub(self.scene_capacity()) as isize,
            ) as usize;
        } else if self.preferences.right_panel && ui::contains(self.right_panel_rect(), self.mouse)
        {
            if self.modifiers.control_key() || self.modifiers.super_key() {
                if delta != 0. {
                    self.properties_cycle(if delta > 0. { -1 } else { 1 });
                }
            } else if self.mouse[0] >= self.right_panel_rect()[0] + properties::RAIL_WIDTH {
                self.inspector_scroll =
                    (self.inspector_scroll - delta * 36.).clamp(0., self.inspector_max_scroll());
            }
        } else if self.canvas_input_at(self.mouse) {
            if self.camera_scroll(delta) {
                return;
            }
            self.leave_camera_view();
            self.performance.changed();
            self.distance = (self.distance * (-delta * 0.12).exp()).clamp(0.1, 500.);
        }
    }
    fn key_event(&mut self, event: winit::event::KeyEvent) {
        if event.state != ElementState::Pressed || self.native_picker.is_some() {
            return;
        }
        if self.swatch_menu_key(&event.logical_key) || self.swatch_open_key(&event.logical_key) { return; }
        if self.welcome_key(&event.logical_key,event.repeat) {return;}
        if self.environment_key(&event.logical_key) {
            return;
        }
        let ctrl = self.modifiers.control_key() || self.modifiers.super_key();
        let shift = self.modifiers.shift_key();
        if self.collision_key(&event.logical_key) { return; }
        if self.dropdown_key(&event.logical_key, shift, ctrl, self.modifiers.alt_key()) {
            return;
        }
        if self.color_open_key(&event.logical_key)
            || self.color_key(&event.logical_key, ctrl, shift)
        {
            return;
        }
        if !ctrl
            && !self.modifiers.alt_key()
            && self.import_dialog_key(&event.logical_key, shift, event.repeat)
        {
            return;
        }
        if self.material_key(
            &event.logical_key,
            ctrl,
            self.modifiers.alt_key(),
            event.repeat,
        ) {
            return;
        }
        if self.camera_effect_key(&event.logical_key, ctrl, self.modifiers.alt_key()) { return; }
        if self.input.is_some() {
            if event.logical_key == Key::Named(NamedKey::Escape) {
                self.end_input();
                self.redraw();
                return;
            }
            if event.logical_key == Key::Named(NamedKey::Enter) {
                self.confirm_input();
                self.redraw();
                return;
            }
            if event.logical_key == Key::Named(NamedKey::Tab) {
                self.focus_next_field(shift);
                self.redraw();
                return;
            }
            self.surface.input_error = None;
            let input = self.input.as_mut().unwrap();
            if ctrl {
                if let Key::Character(ref ch) = event.logical_key {
                    match ch.to_lowercase().as_str() {
                        "a" => input.select_all(),
                        "z" if shift => {
                            input.redo();
                        }
                        "z" => {
                            input.undo();
                        }
                        "y" => {
                            input.redo();
                        }
                        "c" => {
                            if let Some(c) = &mut self.clipboard {
                                let _ = c.set_text(input.selected());
                            }
                        }
                        "x" => {
                            if let Some(c) = &mut self.clipboard {
                                if c.set_text(input.selected()).is_ok() {
                                    input.insert("");
                                }
                            }
                        }
                        "v" => {
                            if let Some(c) = &mut self.clipboard {
                                if let Ok(text) = c.get_text() {
                                    input.insert(&text);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                match event.logical_key {
                    Key::Named(NamedKey::Backspace) => input.backspace(),
                    Key::Named(NamedKey::Delete) => input.delete(),
                    Key::Named(NamedKey::ArrowLeft) => input.move_cursor(-1, shift),
                    Key::Named(NamedKey::ArrowRight) => input.move_cursor(1, shift),
                    Key::Named(NamedKey::Home) => input.home(shift),
                    Key::Named(NamedKey::End) => input.end(shift),
                    _ => {
                        if input.preedit.is_empty() {
                            if let Some(text) = event.text {
                                input.insert(&text);
                            }
                        }
                    }
                }
            }
            self.redraw();
            return;
        }
        if self.modeling_live.is_some() {
            match event.logical_key {
                Key::Named(NamedKey::Escape) => self.cancel_modeling_live(),
                Key::Named(NamedKey::Enter) => self.accept_modeling_live(),
                _ => {}
            }
            return;
        }
        if self.uv_key(&event.logical_key, ctrl) || self.knife_key(&event.logical_key) {
            return;
        }
        if self.node_key(&event.logical_key, ctrl, shift) {
            return;
        }
        if event.repeat {
            return;
        }
        if event.logical_key == Key::Named(NamedKey::Escape) {
            self.action(Action::Cancel);
            return;
        }
        if let Some(action) = self.report_shortcut(&event.logical_key, ctrl) {
            self.action(action);
            return;
        }
        if self.dialog.is_some()
            || self.overwrite.is_some()
            || self.pending_time.is_some()
            || self.help
            || self.poses
            || self.details
            || self.rig_details
            || self.character_panel
            || self.animation_panel
            || self.menu.is_some()
            || self.dropdown.is_some()
        {
            return;
        }
        if self.authoring_key(&event.logical_key, ctrl, self.modifiers.alt_key()) {
            self.redraw();
            return;
        }
        if self.properties_key(&event.logical_key, ctrl) {
            return;
        }
        if let Some(action) = self.timeline_shortcut(&event.logical_key, ctrl, shift) {
            self.action(action);
            return;
        }
        let a = match event.logical_key {
            Key::Named(NamedKey::ArrowLeft) => Some(Action::PrevFrame),
            Key::Named(NamedKey::ArrowRight) => Some(Action::NextFrame),
            Key::Named(NamedKey::F1) => Some(Action::Help),
            Key::Named(NamedKey::Tab) => Some(if shift {
                Action::ToggleRightPanel
            } else {
                Action::ToggleLeftPanel
            }),
            Key::Named(NamedKey::Space) => Some(Action::Play),
            Key::Named(NamedKey::Delete) => Some(Action::Delete),
            Key::Character(ch) => match ch.to_lowercase().as_str() {
                "f" if ctrl => Some(Action::SearchScene),

                "s" if ctrl => Some(if shift { Action::SaveAs } else { Action::Save }),
                "o" if ctrl => Some(Action::Open),
                "i" if ctrl => Some(Action::Import),
                "z" if ctrl => Some(if shift { Action::Redo } else { Action::Undo }),
                "y" if ctrl => Some(Action::Redo),
                "k" if shift => Some(Action::KeyPose),
                "k" => Some(Action::Key),
                "f" => Some(Action::FrameSelected),
                "g" => Some(Action::Move),
                "r" => Some(Action::Rotate),
                "s" => Some(Action::Scale),
                "l" => Some(Action::ToggleGizmoSpace),
                "x" => Some(Action::SetAxis(0)),
                "y" => Some(Action::SetAxis(1)),
                "z" => Some(Action::SetAxis(2)),
                "1" => Some(Action::Front),
                "3" => Some(Action::Side),
                "7" => Some(Action::Top),
                _ => None,
            },
            _ => None,
        };
        if let Some(a) = a {
            self.action(a);
        }
    }
    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64().min(0.1);
        self.last_tick = now;
        // The closing snapshot is frozen: neither background scene commits nor
        // simulation ticks may change it while input is suspended for saving.
        if self.close_recovery.is_pending() { return; }
        self.poll_jobs();
        self.tick_modifiers();
        self.poll_picker();
        self.tick_animation(dt);
        self.tick_fx(dt);
        self.tick_fluid_preview();
        self.tick_fluid_playback(dt);
        self.tick_fluid_domains();
        self.tick_physics(dt);
        self.tick_destruction_preview(dt);
        self.scroll_scene_drag();
        if self.dirty
            && self.last_autosave.elapsed() > Duration::from_secs(60)
            && self.job.is_none()
            && self.drag.is_none()
            && self.modeling_live.is_none()
            && self.color_picker.is_none()
        {
            let snapshot = self.editor.project.clone();
            let document_path = self.path.clone();
            let recovery_id = self.recovery_id.clone();
            let (tx, rx) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            std::thread::spawn(move || {
                let result = workspace::recovery_path(document_path.as_deref(), &recovery_id)
                    .and_then(|path| {
                        poseforge_io::save_project(&snapshot, &path)
                            .map(|()| path)
                            .map_err(|e| e.to_string())
                    });
                let _ = tx.send(match result {
                    Ok(path) => JobMessage::Autosaved(path),
                    Err(e) => JobMessage::Error(format!("Ошибка автосохранения: {e}")),
                });
            });
            self.job = Some(Job {
                receiver: rx,
                cancel,
                label: "Автосохранение".into(),
            });
            self.last_autosave = now;
        }
    }
    fn finish_renderer_startup(&mut self,event_loop:&ActiveEventLoop)->bool {
        let Some(job)=self.renderer_startup.as_ref() else{return true;};
        let Some(result)=job.poll() else{return false;};
        let elapsed=job.started.elapsed();
        self.renderer_startup=None;
        match result {
            Ok(mut renderer)=>{
                if let Some(window)=&self.window {
                    let size=window.inner_size();
                    renderer.resize(size.width,size.height);
                    window.set_title("ArtForge Studio · 3D Artwork");
                }
                eprintln!("GPU готов: {} · {:.1} с",renderer.adapter_name,elapsed.as_secs_f64());
                self.renderer=Some(renderer);
                // Loading time must not advance the animation or physics clock.
                self.last_tick=Instant::now();
                if let Some(path)=self.initial.take() {
                    self.cancel_modeling_live();self.open_dropped(path);
                }
                self.redraw();true
            }
            Err(error)=>{self.startup_error=Some(error);event_loop.exit();false}
        }
    }
    fn draw(&mut self) {
        if self.renderer_startup.is_some() {return;}
        let frame_start = Instant::now();
        self.tick();
        self.video_preview.update(&self.editor.project, self.editor.time);
        let effect_time=self.fx_view_time();
        let preview_settings = self.performance.quality(
            self.editor.project.render_settings,
            self.preferences.adaptive_preview,
            self.drag.is_some()
                || self.editor.playing
                || self.fx_preview_active()
                || self.studio.physics_running
                || self.destruction.preview.running()
                || self.fluid_domains.any_playing()
                || self.fluid_preview.is_pending()
                || self.fluid_domains.is_pending(),
        );
        self.ui = self.build_ui();
        let mut view = self.view();
        if self.preferences.viewport_shading < 3 { view.depth_of_field.enabled = false; }
        if self.preferences.viewport_shading < 2 { view.effects.enabled = false; }
        let evaluated = self.evaluated_viewport();
        if let Ok(eval) = evaluated {
            let mut sampled = frame_preparation::render_project(
                &self.editor.project,
                &eval,
                !self.studio.physics.snapshots.is_empty()
                    || !self.editor.project.video_textures.is_empty()
                    || self.editor.project.nodes.iter().any(|n| n.fluid.is_some()),
            );
            if let std::borrow::Cow::Owned(project) = &mut sampled {
                self.video_preview.apply(project);
                self.apply_physics_preview(project);
                self.fluid_domains.apply(project, self.authoring.editing);
            }
            if sampled.render_settings != preview_settings {
                sampled.to_mut().render_settings = preview_settings;
            }
            if !self.scene_tools.isolated.is_empty() {
                self.isolation_preview(sampled.to_mut());
            }
            if let Some(index) = self.solo_light {
                Self::apply_solo_light(sampled.to_mut(), index);
            }
            if self.viewport_shading_needs_override(&sampled) {
                self.apply_viewport_shading(sampled.to_mut());
            }
            let selected: Vec<_> =
                if self.editor.mode == EditorMode::Scene && self.authoring.editing.is_none() {
                    self.object_selection().into_iter().collect()
                } else {
                    Vec::new()
                };
            if let Some(renderer) = &mut self.renderer {
                renderer.set_editor_selection(&selected, self.preferences.palette().accent);
                renderer.set_editor_appearance(
                    self.preferences.palette().viewport_linear(),
                    self.preferences.grid && self.camera_view.is_none(),
                );
                renderer.set_editor_scale(self.scale);
                renderer.set_editor_shading(self.preferences.viewport_shading);
                renderer.set_interactive_preview(self.performance.stage==performance::QualityStage::Moving);
                renderer.set_effect_time(effect_time);
                renderer.set_destruction_times(self.destruction.preview.times());
                let fluid = if self.fluid_on || self.fluid.time() > 0. {
                    self.fluid.positions()
                } else {
                    &[]
                };
                if let Err(e) = renderer.render(
                    &sampled,
                    &eval.global,
                    &view,
                    &self.ui.rects,
                    &self.ui.texts,
                    fluid,
                ) {
                    self.status = e;
                }
            }
        }
        self.performance.finish(frame_start);
    }
}
impl ApplicationHandler for App {
    fn device_event(&mut self, _event_loop:&ActiveEventLoop, _device_id:winit::event::DeviceId, event:winit::event::DeviceEvent) {
        if self.slider_cursor_locked && matches!(self.drag,Some(Drag::Slider(_))) {
            if let winit::event::DeviceEvent::MouseMotion {delta}=event {
                let point=[self.mouse[0]+delta.0 as f32/self.scale,self.mouse[1]];
                self.mouse_move(point);
            }
        }
    }
    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        self.redraw();
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("ArtForge Studio · 3D Artwork")
            .with_inner_size(LogicalSize::new(1440., 900.))
            .with_min_inner_size(LogicalSize::new(1100., 760.));
        match event_loop.create_window(attrs) {
            Ok(w) => {
                let w = Arc::new(w);
                self.scale = w.scale_factor() as f32;
                self.width = w.inner_size().width as f32 / self.scale;
                self.height = w.inner_size().height as f32 / self.scale;
                w.set_title("ArtForge Studio · Подготовка графики…");
                let prepared=Renderer::prepare_window(w.clone());
                self.window=Some(w);
                match prepared.and_then(|startup|renderer_startup::Startup::spawn(move ||pollster::block_on(startup.finish()))) {
                    Ok(job)=>{
                        self.renderer_startup=Some(job);
                        eprintln!("Подготовка графики: выбор адаптера и создание шейдеров…");
                    }
                    Err(error)=>{self.startup_error=Some(error);event_loop.exit();}
                }
            }
            Err(e) => {
                self.startup_error=Some(format!("Не удалось создать окно: {e}"));
                event_loop.exit();
            }
        }
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.close_recovery.is_pending() && !matches!(&event,
            WindowEvent::RedrawRequested | WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. })
        {
            return;
        }
        if self.renderer_startup.is_some() && !matches!(&event,
            WindowEvent::CloseRequested|WindowEvent::Resized(_)|WindowEvent::ScaleFactorChanged{..}) {
            if let WindowEvent::DroppedFile(path)=event {self.initial=Some(path);}
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.cancel_modeling_live();
                if self.native_picker.is_some() {
                    return;
                }
                self.cancel_color_picker();
                self.cancel_drag();
                if !self.resolve_input() {
                    self.redraw();
                    return;
                }
                if self
                    .job
                    .as_ref()
                    .is_some_and(|j| j.label == "Сохранение" || j.label == "Автосохранение")
                {
                    self.status = "Дождитесь завершения сохранения, затем закройте окно.".into();
                    self.redraw();
                    return;
                }
                if self.editor.has_staged() {
                    self.pending_time = Some(self.editor.time);
                    self.status = "Перед закрытием решите, что сделать с временной позой.".into();
                    self.redraw();
                    return;
                }
                if self.dirty {
                    let snapshot = self.editor.project.clone();
                    let document_path = self.path.clone();
                    let recovery_id = self.recovery_id.clone();
                    let result = self.close_recovery.start(move || {
                        workspace::recovery_path(document_path.as_deref(), &recovery_id)
                            .and_then(|path| poseforge_io::save_project(&snapshot, &path).map_err(|e| e.to_string()))
                    });
                    if let Err(e) = result {
                        self.status = format!(
                            "Не удалось сохранить восстановление: {e}. Сохраните проект вручную."
                        );
                    } else {
                        self.job = None;
                        self.editor.playing = false;
                        self.status = "Сохранение восстановления перед закрытием…".into();
                    }
                    self.redraw();
                    return;
                }
                if let Some(j) = &self.job {
                    j.cancel.store(true, Ordering::Relaxed);
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if self.dropdown_is_open(ui::DropdownKind::AssetLibrary) {
                    self.asset_popup_pointer_up();
                } else {
                    self.close_material_dropdown();
                    self.dropdown = None;
                    self.dropdown_focus = None;
                }
                self.cancel_drag();
                if size.width > 0 && size.height > 0 {
                    self.width = size.width as f32 / self.scale;
                    self.height = size.height as f32 / self.scale;
                    if let Some(r) = &mut self.renderer {
                        r.resize(size.width, size.height);
                    }
                    self.redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if self.dropdown_is_open(ui::DropdownKind::AssetLibrary) {
                    self.asset_popup_pointer_up();
                } else {
                    self.close_material_dropdown();
                    self.dropdown = None;
                    self.dropdown_focus = None;
                }
                self.cancel_drag();
                self.scale = scale_factor as f32;
                if let Some(w) = &self.window {
                    let size = w.inner_size();
                    self.width = size.width as f32 / self.scale;
                    self.height = size.height as f32 / self.scale;
                    if let Some(r) = &mut self.renderer {
                        r.resize(size.width, size.height);
                    }
                }
                self.redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.text_selecting = false;
                if let Some(input) = &mut self.input {
                    input.end_drag();
                }
                if !matches!(self.drag,Some(Drag::Slider(_))) { self.cancel_drag(); }
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !self.slider_cursor_locked {
                    self.mouse_move([position.x as f32/self.scale,position.y as f32/self.scale]);
                    self.wrap_slider_pointer();
                }
            },
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed {
                    let captured=self.drag.is_some();
                    self.mouse_down(button);
                    if !captured && self.drag.is_some() {
                        self.capture_drag_pointer();
                    }
                } else {
                    self.mouse_up(button);
                }
                self.redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.,
                };
                self.scroll(d);
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => self.key_event(event),
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::Ime(Ime::Commit(text)) => {
                if let Some(i) = &mut self.input {
                    i.insert(&text);
                    i.preedit.clear();
                    self.redraw();
                }
            }
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                if let Some(i) = &mut self.input {
                    i.preedit = text;
                    self.redraw();
                }
            }
            WindowEvent::DroppedFile(path) => {
                self.dismiss_swatch_menu();
                self.close_asset_popup();
                self.close_material_dropdown();
                self.dropdown = None;
                self.dropdown_focus = None;
                self.cancel_color_picker();
                if self.native_picker.is_some() || !self.resolve_input() {
                    return;
                }
                self.cancel_drag();
                self.cancel_modeling_live();
                self.open_dropped(path);
                self.redraw();
            }
            WindowEvent::Focused(false) => {
                self.dismiss_swatch_menu();
                self.path_pick=None;
                self.path_edit=None;
                self.close_collision_selection();
                self.authoring.reference_pick = None;
                self.close_asset_popup();
                self.cancel_modeling_live();
                self.close_material_dropdown();
                self.dropdown = None;
                self.dropdown_focus = None;
                self.cancel_color_picker();
                self.text_selecting = false;
                if let Some(input) = &mut self.input {
                    input.end_drag();
                }
                self.cancel_drag();
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                self.draw();
                if let Some(frames) = &mut self.smoke_frames {
                    *frames = frames.saturating_sub(1);
                    if *frames == 0 {
                        event_loop.exit();
                    } else {
                        self.redraw();
                    }
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.close_recovery.is_pending() {
            match self.close_recovery.poll() {
                Ok(true) => { event_loop.exit(); return; }
                Ok(false) => {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16)));
                    return;
                }
                Err(error) => {
                    self.status = format!("Не удалось сохранить восстановление: {error}. Сохраните проект вручную.");
                    self.last_tick = Instant::now();
                    self.redraw();
                }
            }
        }
        if !self.finish_renderer_startup(event_loop) {
            if self.renderer_startup.is_some() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()+Duration::from_millis(16)));
            }
            return;
        }
        if self.editor.playing
            || self.fx_preview_active()
            || self.video_preview.is_pending()
            || self.renderer.as_ref().is_some_and(|r| r.preview_pending())
            || self.fluid_domains.is_pending()
            || self.fluid_preview.is_pending()
            || self.studio.physics_running
            || self.studio.physics.is_pending()
            || self.destruction.preview.running()
            || self.destruction.preview.is_pending()
            || self.fluid_domains.any_playing()
            || self.job.is_some()
            || self.native_picker.is_some()
            || (self.preferences.adaptive_preview && self.performance.needs_refinement())
        {
            let next = self.last_tick + Duration::from_millis(16);
            if Instant::now() >= next {
                self.redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                // Requesting redraw here would immediately wake the loop and
                // bypass WaitUntil, polling background jobs at unlimited speed.
                event_loop.set_control_flow(ControlFlow::WaitUntil(next));
            }
        } else if self.drag.is_some() || self.modeling_live.is_some() || self.color_picker.is_some()
        {
            event_loop.set_control_flow(ControlFlow::Wait);
        } else if self.dirty {
            // An open edit can postpone autosave. Do not spin on its expired
            // deadline while waiting for the user to finish that edit.
            let next = (self.last_autosave + Duration::from_secs(60))
                .max(self.last_tick + Duration::from_secs(1));
            if Instant::now() >= next {
                self.redraw();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(next));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn texture_load_targets_material_and_undo_restores_resource_and_slot() {
        let mut project = Project::default();
        project.materials.push(poseforge_core::Material {
            id: 10,
            ..Default::default()
        });
        let mut editor = Editor::new(project.clone());
        let texture = poseforge_core::Texture {
            id: 0,
            name: "normal.png".into(),
            width: 1,
            height: 1,
            rgba8: vec![128, 128, 255, 255].into(),
        };
        editor
            .transact("Map", |p| {
                assert!(assign_loaded_texture(p, texture.clone(), 10, 2));
            })
            .unwrap();
        let assigned = editor.project.material(10).unwrap().normal_texture.unwrap();
        assert_eq!(editor.project.textures[0].id, assigned);
        assert!(editor
            .project
            .material(10)
            .unwrap()
            .base_color_texture
            .is_none());
        assert_eq!(editor.undo_len(), 1);
        assert!(editor.undo());
        assert_eq!(editor.project, project);
        assert!(editor.redo());
        assert_eq!(editor.project.textures[0].rgba8, texture.rgba8);
    }

    #[test]
    fn texture_result_for_deleted_material_never_creates_orphan_asset() {
        let mut project = Project::default();
        let before = project.clone();
        let texture = poseforge_core::Texture {
            id: 0,
            name: "map.png".into(),
            width: 1,
            height: 1,
            rgba8: vec![255; 4].into(),
        };
        assert!(!assign_loaded_texture(
            &mut project,
            texture.clone(),
            400,
            0
        ));
        assert!(!assign_loaded_texture(&mut project, texture, 400, 8));
        assert_eq!(project, before);
    }

    #[test]
    fn cancelled_queued_import_does_not_mutate_scene() {
        let mut app = deterministic_app();
        let before = app.editor.project.clone();
        let (tx, rx) = mpsc::channel();
        app.job = Some(Job {
            receiver: rx,
            cancel: Arc::new(AtomicBool::new(true)),
            label: "Импорт".into(),
        });
        tx.send(JobMessage::Imported(
            model_import::prepare_import(
                before.clone(),
                poseforge_io::ImportResult {
                    project: Project::default(),
                    report: Default::default(),
                },
                None,
                0.,
                &AtomicBool::new(false),
                |_, _| {},
            )
            .unwrap()
            .into(),
        ))
        .unwrap();
        app.poll_jobs();
        assert_eq!(app.editor.project, before);
        assert!(app.job.is_none());
    }

    #[test]
    fn texture_job_prevents_history_from_reusing_captured_material_id() {
        let mut app = deterministic_app();
        app.editor
            .transact("Rename", |p| p.title = "Changed".into())
            .unwrap();
        let before = app.editor.project.clone();
        let (_tx, rx) = mpsc::channel();
        app.job = Some(Job {
            receiver: rx,
            cancel: Arc::new(AtomicBool::new(false)),
            label: "Загрузка текстуры".into(),
        });
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        assert!(app.scene_write_pending());
        app.action(Action::Cancel);
        assert!(app.job.as_ref().unwrap().cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn file_browser_filters_models_and_maps_by_operation_case_insensitively() {
        assert!(file_extension_allowed(
            FilePurpose::Import,
            Path::new("character.OBJ")
        ));
        assert!(file_extension_allowed(
            FilePurpose::Texture,
            Path::new("skin.JPEG")
        ));
        for name in ["skin.TGA", "skin.BMP", "skin.DDS"] {
            assert!(file_extension_allowed(
                FilePurpose::Texture,
                Path::new(name)
            ));
        }
        assert!(!file_extension_allowed(
            FilePurpose::Import,
            Path::new("skin.png")
        ));
        assert!(!file_extension_allowed(
            FilePurpose::Texture,
            Path::new("character.blend")
        ));
    }

    #[test]
    fn import_report_pagination_keeps_all_diagnostics_and_bounds_long_paths() {
        let text = (0..30)
            .map(|i| format!("Warning {i}: {}", "д".repeat(120)))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = wrapped_report(&text);
        assert!(lines.len() > 12);
        assert!(lines.iter().all(|line| line.chars().count() <= 90));
        assert!(lines.iter().any(|line| line.contains("Warning 29")));
        assert_eq!(
            lines
                .iter()
                .flat_map(|line| line.chars())
                .filter(|c| *c == 'д')
                .count(),
            3600
        );
    }

    #[test]
    fn saving_an_old_snapshot_never_marks_later_edits_clean() {
        let mut app = App::new(None);
        let saved = app.editor.project.clone();
        app.editor.project.title = "Изменено во время сохранения".into();
        let (tx, rx) = mpsc::channel();
        app.job = Some(Job {
            receiver: rx,
            cancel: Arc::new(AtomicBool::new(false)),
            label: "Сохранение".into(),
        });
        tx.send(JobMessage::Saved(
            PathBuf::from("snapshot.poseforge"),
            saved.clone(),
        ))
        .unwrap();
        app.poll_jobs();
        assert!(app.dirty);
        assert_eq!(app.saved_snapshot, saved);
        assert_eq!(app.editor.project.title, "Изменено во время сохранения");
    }

    #[test]
    fn a_new_document_cannot_inherit_an_inflight_save_destination() {
        let mut app = App::new(None);
        let original = app.editor.project.clone();
        let (_tx, rx) = mpsc::channel();
        app.job = Some(Job {
            receiver: rx,
            cancel: Arc::new(AtomicBool::new(false)),
            label: "Сохранение".into(),
        });
        app.action(Action::New);
        assert_eq!(app.editor.project, original);
        assert!(app.path.is_none());
    }

    #[test]
    fn save_dialog_cannot_overwrite_an_imported_model_with_project_json() {
        let mut app = App::new(None);
        app.dialog = Some(FileDialog::new(
            FilePurpose::Save,
            PathBuf::from("character.glb"),
        ));
        app.action(Action::FileSubmit);
        assert!(app.job.is_none());
        assert!(app.dialog.is_some());
        assert!(app.overwrite.is_none());
    }

    #[test]
    fn modal_interactions_never_reach_the_scene_toolbar() {
        let mut app = App::new(None);
        app.help = true;
        let ui = app.build_ui();
        assert_eq!(ui.hit([910., 30.]), None);
        assert_eq!(ui.hit([1000., 730.]), None);
        assert!(ui.hits.iter().all(|h| h.action == Action::CloseHelp));
    }

    #[test]
    fn saved_camera_is_restored_without_refitting_the_scene() {
        let mut app = App::new(None);
        app.editor.project.camera.eye = Vec3::new(3.1, 4.2, -2.3);
        app.editor.project.camera.target = Vec3::new(0.3, 0.8, -0.2);
        app.restore_camera();
        let view = app.view();
        assert!(view.eye.distance(app.editor.project.camera.eye) < 1e-5);
        assert_eq!(view.target, app.editor.project.camera.target);
    }
    fn deterministic_app() -> App {
        let mut app = App::new(None);
        app.preferences = Preferences::default();
        app
    }

    fn gizmo_test_app(animation: bool) -> App {
        let mut app = deterministic_app();
        let mut project = Project::default();
        project.nodes.push(poseforge_core::Node::new(1, "Object"));
        app.editor = Editor::new(project);
        app.editor.selected = Some(1);
        app.editor.mode = if animation {
            EditorMode::Animation
        } else {
            EditorMode::Scene
        };
        app.saved_snapshot = app.editor.project.clone();
        app.target = Vec3::ZERO;
        app.yaw = 0.;
        app.pitch = 0.;
        app.distance = 6.;
        app.orthographic = true;
        app.tool = Tool::Move;
        app.axis = 0;
        let v = app.viewport();
        app.mouse = [v[0] + v[2] * 0.5 + 40., v[1] + v[3] * 0.5];
        app
    }

    #[test]
    fn bone_body_selection_does_not_start_a_transform_or_create_history() {
        for scale in [1., 2.] {
            let mut app = gizmo_test_app(false);
            let mut project = Project::default();
            let mut shoulder = poseforge_core::Node::new(1, "Плечо");
            shoulder.is_joint = true;
            shoulder.transform.translation = Vec3::new(-1., 0., 0.);
            let mut elbow = poseforge_core::Node::new(2, "Локоть");
            elbow.is_joint = true;
            elbow.parent = Some(1);
            elbow.transform.translation = Vec3::new(2., 0., 0.);
            project.nodes = vec![shoulder, elbow];
            app.editor = Editor::new(project);
            app.saved_snapshot = app.editor.project.clone();
            app.tool = Tool::Rotate;
            app.scale = scale;
            app.preferences.skeleton = true;
            let before = app.editor.project.clone();
            // The body is far from either joint. It must select its base joint.
            app.mouse = app.project_point(Vec3::new(0.2, 0., 0.)).unwrap();
            app.mouse_down(MouseButton::Left);
            assert_eq!(app.editor.selected, Some(1));
            assert!(app.drag.is_none());
            app.mouse_move([app.mouse[0] + 12., app.mouse[1] + 7.]);
            app.mouse_up(MouseButton::Left);
            assert_eq!(app.editor.project, before);
            assert_eq!(app.editor.undo_len(), 0);
            assert!(!app.editor.has_staged());
        }
    }

    #[test]
    fn selecting_an_object_is_separate_from_dragging_a_handle() {
        let mut app = gizmo_test_app(false);
        app.editor.selected = None;
        let before = app.editor.project.clone();
        app.mouse = app.project_point(Vec3::ZERO).unwrap();
        app.mouse_down(MouseButton::Left);
        assert_eq!(app.editor.selected, Some(1));
        assert!(app.drag.is_none());
        app.mouse_move([app.mouse[0] + 30., app.mouse[1]]);
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
    }

    #[test]
    fn view_ring_commits_one_rotation_and_undo_restores_it() {
        let mut app = gizmo_test_app(false);
        app.tool = Tool::Rotate;
        let center = app.project_point(Vec3::ZERO).unwrap();
        app.mouse = [center[0] + 96., center[1]];
        let before = app.editor.project.clone();
        app.mouse_down(MouseButton::Left);
        assert!(
            matches!(&app.drag, Some(Drag::Gizmo { gesture, .. }) if gesture.handle == crate::gizmo::Handle::Center)
        );
        for step in 1..=12 {
            let angle = step as f32 * std::f32::consts::FRAC_PI_2 / 12.;
            app.mouse_move([center[0] + 96. * angle.cos(), center[1] - 96. * angle.sin()]);
        }
        let ui = app.build_ui();
        assert!(ui.texts.iter().any(|text| text.text.contains('°')));
        assert_eq!(app.editor.undo_len(), 0);
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.editor.undo_len(), 1);
        assert_ne!(app.editor.project, before);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
    }

    #[test]
    fn gizmo_drag_is_one_undo_step_and_unrelated_button_release_is_ignored() {
        let mut app = gizmo_test_app(false);
        let before = app.editor.project.clone();
        app.begin_gizmo_drag(crate::gizmo::Handle::Axis(0));
        assert!(matches!(app.drag, Some(Drag::Gizmo { .. })));
        let start = app.mouse;
        for offset in 1..25 {
            app.mouse_move([start[0] + offset as f32, start[1]]);
        }
        assert_eq!(app.editor.undo_len(), 0);
        assert_ne!(app.editor.project, before);
        app.mouse_up(MouseButton::Right);
        assert!(app.drag.is_some());
        app.mouse_up(MouseButton::Left);
        assert!(app.drag.is_none());
        assert_eq!(app.editor.undo_len(), 1);
        let after = app.editor.project.clone();
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
        app.action(Action::Redo);
        assert_eq!(app.editor.project, after);
    }

    #[test]
    fn gizmo_cancel_and_tool_switch_restore_snapshot_without_history() {
        for switch_tool in [false, true] {
            let mut app = gizmo_test_app(false);
            let before = app.editor.project.clone();
            app.begin_gizmo_drag(crate::gizmo::Handle::Axis(0));
            app.mouse_move([app.mouse[0] + 40., app.mouse[1]]);
            let distance = app.distance;
            app.scroll(4.);
            assert_eq!(app.distance, distance);
            app.action(if switch_tool {
                Action::Rotate
            } else {
                Action::Cancel
            });
            assert!(app.drag.is_none() && app.drag_button.is_none());
            assert_eq!(app.editor.project, before);
            assert_eq!(app.editor.undo_len(), 0);
            assert!(!app.dirty);
        }
    }

    #[test]
    fn gizmo_no_op_has_no_history_and_animation_remains_staged_until_keyed() {
        let mut app = gizmo_test_app(true);
        let before = app.editor.project.clone();
        app.begin_gizmo_drag(crate::gizmo::Handle::Axis(0));
        let start = app.mouse;
        app.mouse_move([start[0] + 20., start[1]]);
        app.mouse_move(start);
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.editor.undo_len(), 0);
        assert!(!app.editor.has_staged());
        app.begin_gizmo_drag(crate::gizmo::Handle::Axis(0));
        app.mouse_move([app.mouse[0] + 40., app.mouse[1]]);
        app.mouse_up(MouseButton::Left);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.has_staged());
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert!(!app.editor.has_staged());
        assert_eq!(app.editor.project, before);
    }

    #[test]
    fn no_op_undo_and_cancel_leave_normalized_document_clean() {
        let mut app = deterministic_app();
        app.action(Action::Undo);
        assert!(!app.dirty);
        app.action(Action::Cancel);
        assert!(!app.dirty);
        app.editor
            .transact("Rename", |p| p.title = "Changed".into())
            .unwrap();
        app.update_dirty();
        assert!(app.dirty);
        app.action(Action::Undo);
        assert!(!app.dirty);
    }

    #[test]
    fn modal_blocks_scene_input_but_inline_field_can_finish_before_scroll() {
        let mut app = deterministic_app();
        app.mouse = [600., 400.];
        let distance = app.distance;
        let project = app.editor.project.clone();
        app.help = true;
        app.scroll(4.);
        app.open_dropped(PathBuf::from("missing.GLB"));
        assert_eq!(app.distance, distance);
        assert_eq!(app.editor.project, project);
        assert!(app.job.is_none());
        app.help = false;
        app.input(Action::EditTitle, "Title".into());
        app.scroll(4.);
        assert!(app.input.is_none());
        assert!(app.distance < distance);
        assert_eq!(app.editor.project, project);
    }

    #[test]
    fn middle_drag_pans_even_when_orbit_tool_is_active() {
        let mut app = deterministic_app();
        app.tool = Tool::Orbit;
        app.mouse = [600., 400.];
        app.ui = app.build_ui();
        app.mouse_down(MouseButton::Middle);
        assert!(matches!(app.drag, Some(Drag::Pan { .. })));
    }

    #[test]
    fn search_reveals_panel_and_accepts_cyrillic_without_editing_document() {
        let mut app = deterministic_app();
        app.preferences.left_panel = false;
        let project = app.editor.project.clone();
        app.action(Action::SearchScene);
        assert!(app.preferences.left_panel);
        app.input.as_mut().unwrap().insert("Кисть");
        app.confirm_input();
        assert_eq!(app.scene_query, "Кисть");
        assert!(app.scene_rows().iter().any(|r| r.matches));
        assert_eq!(app.editor.project, project);
        assert!(!app.dirty);
    }

    #[test]
    fn inspector_scroll_clips_hits_and_inline_fields_preserve_panel_controls() {
        let mut app = deterministic_app();
        app.preferences.right_panel = true;
        app.width = 1100.;
        // The object panel is shorter after removing duplicate light controls.
        // Use a compact window to retain real overflow for the clipping check.
        app.height = 620.;
        app.mouse = [1050., 500.];
        app.scroll(-20.);
        assert!(app.inspector_scroll > 0.);
        let ui = app.build_ui();
        let surface = ui.hits.iter().find(|h| h.action == Action::Poses).unwrap();
        assert!(surface.rect[1] + surface.rect[3] <= app.height - 44.);
        assert!(!ui.hits.iter().any(|h| h.action == Action::EditName));
        assert_eq!(
            ui.hits
                .iter()
                .filter(|h| h.action == Action::DropdownOpen(ui::DropdownKind::LightCreate))
                .count(),
            1
        );
        assert!(!ui.texts.iter().any(|t| matches!(
            t.text.as_str(),
            "Освещение" | "Настроить источники" | "Добавить источник"
        )));
        app.inspector_scroll = 0.;
        app.action(Action::EditName);
        let inline = app.build_ui();
        assert!(inline.hits.iter().any(|h| h.action == Action::EditName));
        assert!(inline
            .hits
            .iter()
            .any(|h| matches!(h.action, Action::Slide(..))));
        assert!(inline
            .hits
            .iter()
            .any(|h| h.action == Action::ToggleProjectMenu));
        assert!(!inline.hits.iter().any(|h| h.action == Action::ConfirmInput));
        assert!(!app.has_modal());
    }

    #[test]
    fn both_themes_and_panel_layouts_keep_hit_targets_inside_window() {
        let mut app = deterministic_app();
        for (width, height) in [(1100., 760.), (1440., 900.)] {
            app.width = width;
            app.height = height;
            for theme in [ui::Theme::Dark, ui::Theme::Light] {
                app.preferences.theme = theme;
                for (left, right) in [(true, true), (false, true), (true, false), (false, false)] {
                    app.preferences.left_panel = left;
                    app.preferences.right_panel = right;
                    for mode in [
                        EditorMode::Scene,
                        EditorMode::Pose,
                        EditorMode::Animation,
                        EditorMode::Render,
                    ] {
                        app.editor.mode = mode;
                        for scale in [1., 1.5] {
                            app.scale = scale;
                            let ui = app.build_ui();
                            for hit in ui.hits {
                                let [x, y, w, h] = hit.rect;
                                assert!(
                                    x >= 0.
                                        && y >= 0.
                                        && w > 0.
                                        && h > 0.
                                        && x + w <= width
                                        && y + h <= height,
                                    "{:?}: {:?}",
                                    hit.action,
                                    hit.rect
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rig_manifest_inspection_never_modifies_author_data() {
        let mut app = deterministic_app();
        let project = app.editor.project.clone();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/grace-rig.txt");
        app.load_rig_manifest(&path);
        assert_eq!(app.rig_manifest.as_ref().unwrap().bones.len(), 623);
        assert!(app.rig_details);
        assert_eq!(app.editor.project, project);
        assert!(!app.dirty);
        let ui = app.build_ui();
        assert!(!ui.hits.iter().any(|h| h.action == Action::Save));
    }

    #[test]
    fn character_mapping_is_saved_and_undoable_without_touching_geometry() {
        let mut app = deterministic_app();
        let meshes = app.editor.project.meshes.clone();
        app.action(Action::CharacterPanel);
        assert_eq!(app.rig_draft.assignments.len(), 17);
        app.action(Action::SaveRigProfile);
        assert_eq!(app.editor.project.rig_profiles.len(), 1);
        assert!(!app.character_panel);
        assert_eq!(app.editor.project.meshes, meshes);
        app.action(Action::Undo);
        assert!(app.editor.project.rig_profiles.is_empty());
        app.action(Action::Redo);
        assert_eq!(app.editor.project.rig_profiles.len(), 1);
    }

    #[test]
    fn viewing_a_role_preserves_the_unsaved_mapping_draft() {
        let mut app = deterministic_app();
        app.prepare_character();
        app.rig_draft.assignments.remove(&BoneRole::Head);
        let draft = app.rig_draft.clone();
        app.select_role(0, false);
        assert!(!app.character_panel);
        app.prepare_character();
        assert_eq!(app.rig_draft, draft);
        assert!(app.editor.project.rig_profiles.is_empty());
    }

    #[test]
    fn manually_chosen_container_stays_active_when_a_bone_is_selected() {
        let mut app = deterministic_app();
        app.prepare_character();
        app.action(Action::SaveRigProfile);
        app.prepare_character();
        let skeleton = app.rig_draft.root.unwrap();
        let id = app
            .editor
            .project
            .nodes
            .iter()
            .map(|node| node.id)
            .max()
            .unwrap()
            + 1;
        let mut container = poseforge_core::Node::new(id, "Character container");
        container.parent = app.editor.project.node(skeleton).unwrap().parent;
        app.editor.project.node_mut(skeleton).unwrap().parent = Some(id);
        app.editor.project.nodes.push(container);
        app.rig_draft.root = Some(id);
        let hand = app.rig_draft.assignments[&BoneRole::LeftHand];
        let index = BoneRole::ALL
            .iter()
            .position(|role| *role == BoneRole::LeftHand)
            .unwrap();
        app.select_role(index, false);
        app.prepare_character();
        assert_eq!(app.rig_draft.root, Some(id));
        assert_eq!(app.editor.project.rig_profiles.len(), 1);
        app.action(Action::SaveRigProfile);
        app.action(Action::Select(hand));
        assert_eq!(app.active_rig_root, Some(id));
        app.prepare_character();
        assert_eq!(app.rig_draft.root, Some(id));
        assert_eq!(app.rig_draft.assignments.len(), 17);
    }

    #[test]
    fn staged_resolution_closes_before_underlying_animation_dialog() {
        let mut app = deterministic_app();
        app.action(Action::AnimationPanel);
        let id = app.editor.selected.unwrap();
        let mut transform = app.editing_transform(id).unwrap();
        transform.translation.x += 0.2;
        app.editor.set_transform(id, transform).unwrap();
        app.action(Action::NextFrame);
        assert!(app.pending_time.is_some());
        assert!(app.animation_panel);
        assert!(app.build_ui().hits.iter().all(|hit| matches!(
            hit.action,
            Action::SetKey | Action::Discard | Action::Cancel
        )));
        app.action(Action::Cancel);
        assert!(app.pending_time.is_none());
        assert!(app.animation_panel);
        assert!(app.editor.has_staged());
    }

    #[test]
    fn role_selection_pauses_animation_and_exits_render_mode() {
        let mut app = deterministic_app();
        app.prepare_character();
        app.action(Action::SaveRigProfile);
        app.editor.mode = EditorMode::Render;
        app.editor.render_animated = true;
        app.editor.playing = true;
        let index = BoneRole::ALL
            .iter()
            .position(|r| *r == BoneRole::LeftHand)
            .unwrap();
        app.select_role(index, true);
        assert!(!app.editor.playing);
        assert_eq!(app.editor.mode, EditorMode::Animation);
        assert!(app.ik);
        assert_eq!(
            app.editor.selected,
            app.current_profile()
                .unwrap()
                .assignments
                .get(&BoneRole::LeftHand)
                .copied()
        );
    }

    #[test]
    fn full_pose_keys_follow_the_selected_character_only() {
        let mut app = deterministic_app();
        app.prepare_character();
        app.action(Action::SaveRigProfile);
        let first_profile = app.current_profile().unwrap().clone();
        let first_nodes: BTreeSet<_> = app.character_scope().unwrap().into_iter().collect();
        let second = app
            .editor
            .project
            .duplicate_subtree(first_profile.root.unwrap())
            .unwrap();
        app.action(Action::Select(second));
        assert_eq!(app.active_rig_root, Some(second));
        let before: Vec<_> = app.editor.project.clips[0]
            .channels
            .iter()
            .filter(|c| first_nodes.contains(&c.node))
            .cloned()
            .collect();
        app.action(Action::KeyPose);
        let after: Vec<_> = app.editor.project.clips[0]
            .channels
            .iter()
            .filter(|c| first_nodes.contains(&c.node))
            .cloned()
            .collect();
        assert_eq!(before, after);
        let new_nodes = app.character_scope().unwrap();
        assert!(!new_nodes.is_empty());
        assert!(new_nodes.iter().all(|id| !first_nodes.contains(id)));
        assert!(new_nodes.iter().all(|id| app.editor.project.clips[0]
            .channels
            .iter()
            .any(|c| c.node == *id && c.keys.iter().any(|k| k.time == app.editor.time))));
    }

    #[test]
    fn imported_character_opens_its_own_mapping_in_an_existing_scene() {
        let mut app = deterministic_app();
        app.prepare_character();
        app.action(Action::SaveRigProfile);
        let original = app.active_rig_root.unwrap();
        let incoming = app.editor.project.clone();
        let roots = poseforge_io::merge_project(&mut app.editor.project, incoming).unwrap();
        app.focus_imported_character(&roots);
        assert!(app.character_panel);
        assert_ne!(app.active_rig_root, Some(original));
        assert_eq!(app.rig_draft.root, app.active_rig_root);
        assert_eq!(app.rig_draft.assignments.len(), 17);
        assert!(app
            .rig_draft
            .root
            .is_some_and(|id| roots.iter().any(|root| app.is_descendant(id, *root))));
    }

    #[test]
    fn body_menu_is_absent_from_tool_rail_in_both_themes() {
        let mut app = deterministic_app();
        app.prepare_character();
        app.action(Action::SaveRigProfile);
        app.width = 1100.;
        app.height = 760.;
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            app.preferences.theme = theme;
            app.menu = None;
            let canvas = app.build_ui();
            assert!(!canvas
                .hits
                .iter()
                .any(|h| h.action == Action::ToggleBodyMenu));
            assert!(!canvas
                .hits
                .iter()
                .any(|h| matches!(h.action, Action::SelectRole(_) | Action::IkRole(_))));
            app.action(Action::ToggleBodyMenu);
            let ui = app.build_ui();
            assert_eq!(
                ui.hits
                    .iter()
                    .filter(|h| matches!(h.action, Action::SelectRole(_)))
                    .count(),
                6
            );
            assert_eq!(
                ui.hits
                    .iter()
                    .filter(|h| matches!(h.action, Action::IkRole(_)))
                    .count(),
                4
            );
            for hit in ui.hits {
                let [x, y, w, h] = hit.rect;
                assert!(
                    x >= 0. && y >= 0. && x + w <= app.width && y + h <= app.height,
                    "{:?}",
                    hit.action
                );
            }
        }
    }

    #[test]
    fn default_workspace_gives_the_model_space_and_reveals_time_only_for_animation() {
        let mut app = deterministic_app();
        app.width = 1100.;
        app.height = 760.;
        let v = app.viewport();
        assert!(app.preferences.left_panel && !app.preferences.right_panel);
        assert!(v[2] * v[3] > app.width * app.height * 0.8);
        let pose = app.build_ui();
        assert!(!pose.hits.iter().any(|h| matches!(
            h.action,
            Action::Timeline | Action::EditFrame | Action::AutoKey
        )));
        for action in [
            Action::Move,
            Action::Rotate,
            Action::Scale,
            Action::ToggleGizmoSpace,
        ] {
            let handle = pose
                .hits
                .iter()
                .find(|hit| hit.action == action)
                .expect("transform control remains reachable");
            assert!(intersect_rect(handle.rect, app.rail_rect()).is_some());
        }
        let rail = app.rail_rect();
        assert!(rail[2] <= app.left_width() && rail[1] + rail[3] <= v[1] + v[3]);
        let document = app.editor.project.clone();
        app.action(Action::Mode(2));
        assert_eq!(app.viewport(), v); // Scene remains visible around the floating timeline.
        assert!(app
            .build_ui()
            .hits
            .iter()
            .any(|h| h.action == Action::Timeline));
        app.action(Action::Mode(1));
        assert_eq!(app.viewport(), v);
        assert_eq!(app.editor.project, document);
    }

    #[test]
    fn floating_tools_block_orbit_pan_and_zoom_without_affecting_canvas_input() {
        let mut app = deterministic_app();
        app.ui = app.build_ui();
        let r = app.rail_rect();
        app.mouse = [r[0] + 2., r[1] + 2.];
        let distance = app.distance;
        for button in [MouseButton::Left, MouseButton::Middle, MouseButton::Right] {
            app.mouse_down(button);
            assert!(app.drag.is_none());
        }
        app.scroll(2.);
        assert_eq!(app.distance, distance);
        app.mouse = [600., 400.];
        app.mouse_down(MouseButton::Right);
        assert!(matches!(app.drag, Some(Drag::Orbit { .. })));
    }

    #[test]
    fn menu_dismissal_consumes_pointer_input_and_keeps_document_unchanged() {
        let mut app = deterministic_app();
        let document = app.editor.project.clone();
        let distance = app.distance;
        app.action(Action::ToggleProjectMenu);
        app.ui = app.build_ui();
        app.mouse = [700., 500.];
        app.scroll(3.);
        app.mouse_down(MouseButton::Middle);
        app.mouse_down(MouseButton::Right);
        app.open_dropped(PathBuf::from("missing.blend"));
        assert!(app.menu.is_some() && app.drag.is_none() && app.job.is_none());
        app.mouse_down(MouseButton::Left);
        assert!(app.menu.is_none() && app.drag.is_none());
        assert_eq!(app.distance, distance);
        app.action(Action::ToggleViewMenu);
        app.action(Action::Cancel);
        assert!(app.menu.is_none());
        assert_eq!(app.editor.project, document);
    }
}

#[cfg(test)]
#[path = "studio_tests.rs"]
mod studio_tests;

#[cfg(test)]
#[path = "lookdev13_tests.rs"]
mod lookdev13_tests;

#[cfg(test)]
#[path = "workspace14_tests.rs"]
mod workspace14_tests;

#[cfg(test)]
#[path = "workspace17_tests.rs"]
mod workspace17_tests;

#[cfg(test)]
#[path = "workspace24_tests.rs"]
mod workspace24_tests;
#[cfg(test)]
#[path = "workspace26_tests.rs"]
mod workspace26_tests;

#[cfg(test)]
#[path = "workspace27_tests.rs"]
mod workspace27_tests;

#[cfg(test)]
#[path = "workspace39_tests.rs"]
mod workspace39_tests;

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod helpers_tests;
