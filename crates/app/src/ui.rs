use crate::icons::Icon;
use poseforge_render::{UiRect, UiText};
#[path = "ui_corner_cache.rs"]
mod corner_cache;

/// Shared outer radius for controls, panels and popovers, in logical pixels.
pub const SURFACE_RADIUS: f32 = 4.;

/// sRGB colors, shared by primitives, panels, overlays and viewport decorations.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: [f32; 4],
    pub panel: [f32; 4],
    pub elevated: [f32; 4],
    pub hover: [f32; 4],
    pub accent: [f32; 4],
    pub selection: [f32; 4],
    pub text: [f32; 4],
    pub muted: [f32; 4],
    pub border: [f32; 4],
    pub warn: [f32; 4],
    pub danger: [f32; 4],
    pub disabled: [f32; 4],
    pub viewport: [f32; 4],
    pub slider: [f32; 4],
}

impl Palette {
    /// Renderer clear colors are linear; UI palette values are sRGB.
    pub fn viewport_linear(self) -> [f32; 4] {
        let linear = |value: f32| {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        [
            linear(self.viewport[0]),
            linear(self.viewport[1]),
            linear(self.viewport[2]),
            self.viewport[3],
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}
impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
    pub fn from_name(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            _ => Self::Dark,
        }
    }
    pub const fn palette(self) -> Palette {
        match self {
            Self::Dark => Palette {
                slider: [0.96, 0.77, 0.25, 1.],
                bg: [0.110, 0.114, 0.122, 1.0],
                panel: [0.153, 0.157, 0.169, 1.0],
                elevated: [0.196, 0.204, 0.219, 1.0],
                hover: [0.251, 0.263, 0.282, 1.0],
                accent: [0.569, 0.706, 0.965, 1.0],
                selection: [0.149, 0.212, 0.310, 1.0],
                text: [0.949, 0.953, 0.961, 1.0],
                muted: [0.678, 0.698, 0.737, 1.0],
                border: [0.227, 0.239, 0.267, 1.0],
                warn: [0.914, 0.757, 0.506, 1.0],
                danger: [0.933, 0.549, 0.549, 1.0],
                disabled: [0.451, 0.467, 0.502, 1.0],
                viewport: [0.173, 0.176, 0.192, 1.0],
            },
            Self::Light => Palette {
                slider: [0.67, 0.47, 0.015, 1.],
                bg: [0.929, 0.933, 0.941, 1.0],
                panel: [0.980, 0.980, 0.980, 1.0],
                elevated: [0.910, 0.918, 0.929, 1.0],
                hover: [0.863, 0.878, 0.902, 1.0],
                accent: [0.192, 0.373, 0.769, 1.0],
                selection: [0.878, 0.914, 0.980, 1.0],
                text: [0.149, 0.157, 0.176, 1.0],
                muted: [0.376, 0.404, 0.443, 1.0],
                border: [0.804, 0.820, 0.847, 1.0],
                warn: [0.557, 0.341, 0.047, 1.0],
                danger: [0.686, 0.180, 0.227, 1.0],
                disabled: [0.510, 0.537, 0.573, 1.0],
                viewport: [0.824, 0.831, 0.843, 1.0],
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Blue,
    Violet,
    Coral,
    Gold,
    Green,
    Cyan,
}
impl Accent {
    pub const ALL: [Self; 6] = [
        Self::Blue,
        Self::Violet,
        Self::Coral,
        Self::Gold,
        Self::Green,
        Self::Cyan,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Violet => "violet",
            Self::Coral => "coral",
            Self::Gold => "gold",
            Self::Green => "green",
            Self::Cyan => "cyan",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Blue => "Голубой",
            Self::Violet => "Фиолетовый",
            Self::Coral => "Коралловый",
            Self::Gold => "Золотой",
            Self::Green => "Зелёный",
            Self::Cyan => "Бирюзовый",
        }
    }
    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|a| a.as_str().eq_ignore_ascii_case(name.trim().trim_start_matches('#')))
            .unwrap_or_default()
    }
    pub fn palette(self, theme: Theme) -> Palette {
        let mut p = theme.palette();
        if self == Self::Blue {
            return p;
        }
        let (dark, light) = match self {
            Self::Violet => ([0.77, 0.64, 0.98, 1.], [0.48, 0.27, 0.76, 1.]),
            Self::Coral => ([0.98, 0.64, 0.57, 1.], [0.72, 0.29, 0.22, 1.]),
            Self::Gold => ([0.96, 0.77, 0.25, 1.], [0.60, 0.42, 0.02, 1.]),
            Self::Green => ([0.53, 0.84, 0.61, 1.], [0.17, 0.48, 0.28, 1.]),
            Self::Cyan => ([0.39, 0.82, 0.85, 1.], [0.06, 0.46, 0.51, 1.]),
            Self::Blue => unreachable!(),
        };
        p.accent = if theme == Theme::Dark { dark } else { light };
        p.selection = std::array::from_fn(|i| {
            if i == 3 {
                1.
            } else {
                p.panel[i] * 0.82 + p.accent[i] * 0.18
            }
        });
        p
    }
}

/// Slider values are quantized relative to the lower bound. Exact text entry
/// remains independent of the convenient range represented by the track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}
impl SliderRange {
    pub fn new(min: f64, max: f64, step: f64) -> Self {
        let min = if min.is_finite() { min } else { 0. };
        let max = if max.is_finite() { max } else { min };
        Self {
            min: min.min(max),
            max: max.max(min),
            step: if step.is_finite() && step > 0. {
                step
            } else {
                0.
            },
        }
    }

    // Test adapter for the absolute track mapping; live drags snap relative values.
    #[cfg(test)]
    pub fn value_at(self, x: f32, rect: [f32; 4]) -> f64 {
        let range = Self::new(self.min, self.max, self.step);
        if !x.is_finite() || !rect[0].is_finite() || !rect[2].is_finite() || rect[2] <= 0. {
            return range.min;
        }
        let ratio = ((x as f64 - rect[0] as f64) / rect[2] as f64).clamp(0., 1.);
        if ratio <= 0. || range.min == range.max {
            return range.min;
        }
        if ratio >= 1. {
            return range.max;
        }
        // Weighted endpoints also handle finite bounds whose difference would
        // overflow, such as -f64::MAX .. f64::MAX.
        let value = range.min * (1. - ratio) + range.max * ratio;
        range.clamp_and_snap(value)
    }

    /// Snap in f64 without a round trip through screen coordinates. In particular,
    /// fine adjustments must retain subpixel precision before field validation.
    pub fn clamp_and_snap(self, value: f64) -> f64 {
        let range = Self::new(self.min, self.max, self.step);
        if value.is_nan() || value <= range.min || range.min == range.max {
            return range.min;
        }
        if value >= range.max {
            return range.max;
        }
        let steps = (value - range.min) / range.step;
        let quantized = if range.step > 0. && steps.is_finite() {
            range.min + steps.round() * range.step
        } else {
            value
        };
        if quantized.is_finite() {
            quantized.clamp(range.min, range.max)
        } else {
            value.clamp(range.min, range.max)
        }
    }

    pub fn ratio(self, value: f64) -> f32 {
        let range = Self::new(self.min, self.max, self.step);
        if value.is_nan() || value <= range.min || range.min == range.max {
            return 0.;
        }
        if value >= range.max {
            return 1.;
        }
        let span = range.max - range.min;
        let ratio = if span.is_finite() {
            (value - range.min) / span
        } else {
            (value * 0.5 - range.min * 0.5) / (range.max * 0.5 - range.min * 0.5)
        };
        ratio.clamp(0., 1.) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropdownKind {
    CameraEffectAdd,
    ModifierAdd,
    ModifierTarget(bool,u64),
    Accent,
    ReflectionSamples,
    AreaShadowSamples,
    Background,
    MeshCreate,
    EmptyCreate, EmptyShape, ForceCreate, ForceKind, FxCreate, FxKind, FxPreset(u8), FxQuality, FxEmission,
    TextFont,
    CurveCreate,
    CurvePath,
    SkeletonCreate,
    BoneShape,
    JointShape,
    RigSkeleton,
    RigMesh,
    PhysicsType,
    FluidSource,
    RenderQuality,
    Antialiasing,
    ToneMapping,
    ShadowResolution,
    LightingPreset,
    LightKind,
    LightCreate,
    MaterialLibrary,
    AssetLibrary,
    MaterialCreate,
    MaterialAlpha,
    MaterialActions,
    MaterialCategory,
    SavedViews,
    SceneActions,
    SceneFilter,
    Collections,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigReference { Skeleton, Mesh }
impl RigReference {
    pub fn dropdown(self) -> DropdownKind {
        match self { Self::Skeleton => DropdownKind::RigSkeleton, Self::Mesh => DropdownKind::RigMesh }
    }
    pub fn from_dropdown(kind: DropdownKind) -> Option<Self> {
        match kind { DropdownKind::RigSkeleton => Some(Self::Skeleton), DropdownKind::RigMesh => Some(Self::Mesh), _ => None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneTool {
    SelectAll,
    Clear,
    Isolate,
    NewCollection,
    CollectionName(u64),
    CollectionSelect(u64),
    AssignCollection(Option<u64>),
    CollectionCollapse(u64),
    CollectionVisible(u64),
    CollectionRender(u64),
    CollectionLock(u64),
    CollectionDelete(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightTool {
    #[cfg(test)]
    AimLast,
    PickTarget,
    PickTrackedTarget,
    StopTracking,
    Solo,
    SoloOff,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraTool {
    AddEffect(usize),
    SelectEffect(usize),
    ToggleEffect(usize),
    RemoveEffect(usize),
    MoveEffect(usize, i8),
    Effects,
    Dof,
    Thirds,
    PickFocus,
    SaveView,
    RecallView(usize),
    DeleteView,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StudioAction {
    AbsorptionFromBase,
    GeometricThickness,
    Field(u8),
    Mesh(u8),
    Physics(u8),
    PhysicsPlay,
    PhysicsReset,
    PhysicsBake,
    PhysicsSequence,
    PhysicsSelfCollision,
    PhysicsPin,
    LayerAdd,
    LayerSelect(usize),
    LayerEnabled(usize),
    LayerSolo(usize),
    LayerRemove(usize),
    LayerMove(bool),
    LayerBlend,
    LayerClip,
    Background(u8),
    CameraLock,
    FluidFill(u8),
    FluidFromMesh,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldAction {
    Library,
    CloseLibrary,
    LibraryPage(i32),
    ChooseEnvironment(usize),
    ForgetEnvironment(usize),
    Import,
    Enabled,
    Background,
    Reflections,
    Shadows,
    ReflectionSamples(u32),
    ShadowSamples(u32),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeAction {
    LoadImage,
    NextImage,
    ImageData,
    InspectorScroll,
    Open,
    Close,
    AddMenu,
    Add(usize),
    Select(u32),
    Move(u32),
    Output(u32),
    Input(u32, usize),
    Disconnect(u32, usize),
    Delete,
    Toggle,
    ResetView,
    Value(usize),
    Position(usize),
    Pan,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeshSelection {
    #[default]
    Vertex,
    Edge,
    Face,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveAction {
    Orientation(u8),
    CurvePoint(u8),
    Field(u8),
    Smooth,
    Reset,
    Collapse,
    Close,
    Open,
    Backdrop,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AuthorAction {
    Knife,
    KnifeAccept,
    KnifeCancel,
    Slide,
    Proportional,
    ProportionalRadius,
    SelectTool(u8),
    EdgeLoop,
    EdgeRing,
    Merge,
    Weld,
    WeldDistance,
    Bridge,
    BridgeSegments,
    Dissolve,
    Inspect,
    SelectDefects,
    EdgeBevel,
    LiveField(u8),
    LiveDrag(u8),
    LiveAccept,
    LiveCancel,
    LiveBackdrop,
    LiveToggle,
    Xray,
    PivotOrigin,
    Constrain(u8),
    Invert,
    Grow,
    Shrink,
    Connected,
    RotateDegrees(u8),
    ScaleFactor(u8),
    InsetAmount,
    BevelHeight,
    Bevel,
    Inset,
    LoopPosition,
    LoopCut,
    Paint,
    PaintJoint(i8),
    PaintRadius,
    PaintStrength,
    ToggleEdit,
    SelectMode(MeshSelection),
    SelectAll,
    Clear,
    Coordinate(u8),
    Extrude,
    DeleteFaces,
    Subdivide,
    Smooth,
    Flat,
    CreateSkeleton(bool),
    CreateBone,
    ExtendJoint,
    Bind,
    Unbind,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UvAction {
    Open,
    Close,
    Backdrop,
    Mode(u8),
    Tool(u8),
    Seams(bool),
    Unwrap,
    Pack,
    Stretch,
    Frame,
    All,
    Number(u8),
    PanelNumber(u8),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FluidObjectAction {
    Attach,
    Remove,
    Enabled,
    MeshCollision,
    Play,
    Reset,
    Material,
    Preset(u8),
    Field(u8),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DestructionAction {
    Attach,
    Remove,
    MeshCollision,
    ShardCollision,
    Material, Field(u8), Reseed, Floor, ShowFloor, Dust, Create, Bake, Restore, Play, Reset }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveAction {
    Create(u8), Draw, EditPoints, Point(i8), Field(u8), AddGeometry, RemoveGeometry,
    Profile(u8), Caps, Closed, Interpolation(u8), AddPoint, DeletePoint, Convert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperAction {Empty(u8),Shape(u8),Size,CreateField(u8),Kind(u8),Number(u8),Enabled,Target(u8),Play,Reset}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxAction {Create(u8),Kind(u8),Enabled,Field(u8),Color,Play,Reset,Illuminate,Defaults,Source(Option<u64>),Fields,MeshCollision,Cloud,Burst,Loop,
    Preset(u8),Quality(u8),Gas,Hollow,Evolution,Smooth,Point(u8),Channel(u8),KeyColor(u8),ClearCache,Upgrade,Emission(u8),SelfShadow,TemperatureColor,SmokeColor}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAction {Create,Content,Field(u8),Convert,Font(u8),OnCurve}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathAction { Pick, Bind(u64), Clear, Follow, Repeat, Field(u8), SelectCurve, Edit, ResetOffset }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaAction { Slot(usize), UvNumber(usize,u8), ResetUv(usize), VideoNumber(usize,u8), VideoLoop(usize), VideoEnabled(usize), NormalFlip, NprPreset(u8), PruneVideos }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorphAction { Add, Select(usize), Weight(usize), Rename(usize), Delete(usize), Copy, Sources, Source(u64), Replace, Key, Page(i8), ApplyCopy, Original }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierAction {
    Add(poseforge_core::ModifierKind), Select(u64,u64), Toggle(u64,u64), Remove(u64,u64),
    Field(u64,u64,u8), Axis(u64,u64,u8), Smooth(u64,u64), Reset(u64,u64),
    Step(u64,u64,i8), Duplicate(u64,u64), Transfer(u64,u64,u64,bool), Apply(u64),
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Modifier(ModifierAction),
    Helper(HelperAction),
    Fx(FxAction),
    CreateEmpty,
    Text(TextAction),
    Collision(CollisionAction),
    Media(MediaAction),
    Morph(MorphAction),
    Curve(CurveAction),
    Path(PathAction),
    AssetCategory(u8),
    AssetSearch,
    AssetSearchClear,
    AssetMaterial(u64),
    AssetPreset(usize),
    AssetScene(u8),
    ModelingTool(ModelingTool),
    FluidObject(FluidObjectAction),
    Destruction(DestructionAction),
    ResetNumber(Box<Action>),
    RigStage(u8),
    BoneDisplay(BoneDisplayAction),
    RigReferencePick(RigReference),
    RigReferenceSet(RigReference, u64),
    Normal(NormalAction),
    Shape(ShapeAction),
    Rig(RigAction),
    InterfaceTheme(Theme),
    Uv(UvAction),
    Primitive(PrimitiveAction),
    MaterialFavorite,
    MaterialFavorites,
    MaterialCategory,
    MaterialTags,
    MaterialGrid,
    MaterialBrowse(bool),
    MaterialLibraryToggle,
    MaterialApplyFaces,
    MaterialUnassignFaces,
    MaterialUserDetach(u64, u64),
    MaterialLibraryRemove(u64),
    MaterialLibraryCategory(usize),
    MaterialLayerVisible(u64, u64),
    MaterialLayerMove(i8),
    MaterialSelect(u64),
    MaterialUsers(u64),
    MaterialUserSelect(u64, u64),
    MaterialSize,
    AdaptivePreview,
    PerformanceOverlay,
    ToggleTools,
    ToggleSection(u16),
    ViewAxis(u8),
    ViewOrbit,
    ObjectScroll,
    MaterialSearch,
    MaterialSearchClear,
    MaterialScroll,
    MaterialAssign(u64, u64, Option<u64>),
    World(WorldAction),
    Node(NodeAction),
    Author(AuthorAction),
    Studio(StudioAction),
    ScrollPanel(u8),
    DropdownOpen(DropdownKind),
    DropdownChoose(usize),
    DropdownDismiss,
    ToggleProjectMenu,
    ShowWelcome,
    CloseWelcome,
    Language(crate::localization::Language),
    OpenRecent(usize),
    ToggleViewMenu,
    ViewShading(u8),
    ViewPan,
    ViewZoom,
    ToggleBodyMenu,
    DismissMenu,
    New,
    Open,
    Save,
    SaveAs,
    Import,
    Undo,
    Redo,
    Mode(u8),
    Select(u64),
    SceneSelectLight(usize, Box<poseforge_core::Light>),
    SceneToggleLight(usize, Box<poseforge_core::Light>),
    SceneToggleNode(u64),
    SceneShadowOnly(u64),
    SceneDelete(poseforge_core::SceneItem),
    SceneDeleteLight(usize, Box<poseforge_core::Light>),
    SceneExpand(u64),
    SceneMaterial(u64, u64),
    SceneCloth(u64),
    SceneClothVisible(u64),
    SceneClothRemove(u64),
    SceneDestruction(u64),
    SceneDestructionVisible(u64),
    SceneDestructionRemove(u64),
    SceneFluid(u64),
    SceneFluidVisible(u64),
    SceneFluidRemove(u64),
    SceneMaterialVisible(u64),
    SceneMaterialDetach(u64, u64),
    Play,
    Start,
    PreviousKey,
    NextKey,
    Key,
    AutoKey,
    Reset,
    Duplicate,
    Delete,
    FrameSelected,
    Front,
    Side,
    Top,
    Perspective,
    Orbit,
    Move,
    Rotate,
    Scale,
    ToggleGizmoSpace,
    EditTitle,
    EditName,
    EditTransform(usize),
    EditFps,
    EditDuration,
    Slide(Box<Action>, SliderRange, f64),
    SetAxis(usize),
    SetClip(usize),
    ToggleVisible,
    Timeline,
    Animation(crate::animation_timeline::TimelineControl),
    DeleteKey,
    CopyPose,
    PastePose,
    Poses,
    SavePose,
    LoadPose(usize),
    Details,
    ImportReport,
    CopyReport,
    DetailsPage(i32),
    Ik,
    SetKey,
    Discard,
    Cancel,
    Export,
    ExportSequence,
    EditWidth,
    EditHeight,
    Transparent,
    Fluid,
    FluidBake,
    FluidBakeCancel,
    FluidCacheClear,
    FluidCacheSave,
    FluidCacheLoad,
    ResetFluid,
    FluidSurface,
    FileEntry(usize),
    ParentDir,
    FileSubmit,
    EditPath,
    BrowsePath,
    PastePath,
    ConfirmOverwrite,
    SetCamera,
    CreateCamera,
    CameraView,
    CameraAlignView,
    CameraExitView,
    CameraSetActive,
    CameraClearActive,
    CameraProjection,
    CameraNumber(usize),
    CameraTool(CameraTool),
    CameraEffect(u64, CameraTool),
    LightTool(LightTool),
    SceneTool(SceneTool),
    PanelResize(bool),
    SurfaceTab(u8),
    SurfaceNextMesh,
    SurfaceMaps,
    SurfaceCreateMaterial,
    SurfaceCreateMaterialPreset(usize),
    SurfaceResetMapping,
    SurfaceUniqueMaterial,
    SurfaceUnassignMaterial,
    SurfacePruneMaterials,
    SurfaceMaterialName,
    SurfaceMaterialList,
    SurfaceAssignMaterial(u64),
    SurfaceMaterialColor(bool),
    SurfaceAttenuationColor,
    SurfaceNprToggle,
    SurfaceArtisticToggle,
    SurfaceArtisticNextStyle,
    SurfaceArtisticReset,
    SurfaceNprColor(usize),
    OpenColorPicker(Box<Action>),
    ResetColor(Box<Action>),
    ColorPicker(crate::color_picker::ColorControl),
    SurfaceMaterialNumber(usize),
    SurfaceDoubleSided,
    SurfaceEmissionVolume,
    SurfacePreset(usize),
    SurfaceNextTexture(usize),
    SurfaceClearTexture(usize),
    SurfaceImportTexture(usize),
    SurfaceAddLight(usize),
    SurfaceDeleteLight,
    SurfaceToggleLight,
    SurfaceToggleLightShadow,
    SurfaceLightTemperature,
    #[allow(dead_code)] // Retained as an internal command for existing navigation regressions.
    SurfacePointLightAtView,
    SurfaceLightKind,
    SurfaceAimLight,
    SurfaceLightColor,
    SurfaceLightVolumeColor,
    SurfaceLightVolume,
    SurfaceLightName,
    SurfaceLightNumber(usize),
    SurfaceRenderNumber(usize),
    RenderAntialiasing(u32),
    RenderToneMapping(u8),
    RenderToggleShadows,
    RenderShadowResolution(u32),
    RenderQuality(u8),
    RenderColor(usize),
    LightingPreset(u8),
    FluidNumber(usize),
    FluidPreset(u8),
    FluidResetSettings,
    FluidFrame,
    ConfirmInput,
    CloseHelp,
    Help,
    ToggleLeftPanel,
    ToggleAnimationPanel,
    ToggleAnimationLayers,
    AnimationLayersScroll,
    ToggleRightPanel,
    SearchScene,
    ClearSearch,
    ImportRig,
    ToggleGrid,
    ToggleBackfaces,
    ToggleSkeleton,
    CharacterPanel,
    DetectSkeleton,
    SaveRigProfile,
    PickBone(usize),
    ClearBone(usize),
    SelectRole(usize),
    IkRole(usize),
    PickRigRoot,
    SetRigRoot(u64),
    AssignBone(u64),
    SearchBones,
    ClearBoneSearch,
    BackToRig,
    AnimationPanel,
    CreateClip,
    RenameClip,
    DuplicateClip,
    KeyPose,
    PrevFrame,
    NextFrame,
    EditFrame,
    ShiftKeys(i32),
    EditBlenderPath,
    BrowseBlenderPath,
    PasteBlenderPath,
    DetectBlender,
    ToggleBlendImportAnimations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelingTool {
    Select(u8), Move, Rotate, Scale,
    Extrude, Inset, Bevel, LoopCut, Knife, Slide,
    Merge, Weld, Bridge, Dissolve, DeleteFaces, Subdivide,
    Smooth, Flat, Flatten, Straighten, Space, Relax, Inspect,
    Sphere, Circle, Inflate, Twist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneDisplayAction {
    BodyColor,
    JointColor,
    ResetBodyColor,
    ResetJointColor,
    BodyShape(poseforge_core::BoneShape),
    JointShape(poseforge_core::JointShape),
    BodyFilled,
    JointFilled,
    BodyFollowView,
    JointFollowView,
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RigAction {
    Handles,
    Controller(usize),
    Endpoint(usize),
    Pole,
    Kinematics(bool),
    KeyPart,
    ResetPart,
    CopyPart,
    PastePart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionAction { Open, SourceOpen, SourcePick, Search, ClearSearch, Close, Pick, Add(u64), Toggle(u64,u64), Remove(u64,u64) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalAction {
    Field(u8),
    Angle(u8),
    Display(u8),
    AutoLength,
    XRay,
    Show,
    Weighted,
    Recalculate,
    Flat,
    Smooth,
    Flip,
    Direction(u8),
    Rotate(u8),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeAction {
    Sphere,
    Circle,
    Inflate,
    Twist,
    Axis(u8),
    Flatten(u8),
    Straighten,
    Space,
    Relax,
    Pin,
}
impl Action {
    pub fn search_clear(&self) -> Option<Self> {
        match self {
            Self::Collision(CollisionAction::Search) => Some(Self::Collision(CollisionAction::ClearSearch)),
            Self::SearchScene => Some(Self::ClearSearch),
            Self::MaterialSearch => Some(Self::MaterialSearchClear),
            Self::AssetSearch => Some(Self::AssetSearchClear),
            Self::SearchBones => Some(Self::ClearBoneSearch),
            _ => None,
        }
    }
    fn text_reserve(&self) -> f32 {
        if self.search_clear().is_some() {
            28.
        } else {
            0.
        }
    }

    pub fn is_field(&self) -> bool {
        if matches!(self, Self::BoneDisplay(BoneDisplayAction::BodyColor | BoneDisplayAction::JointColor)) { return true; }
        if matches!(self,Self::Modifier(ModifierAction::Field(..))) {return true;}
        if matches!(self,Self::Fx(FxAction::Field(_)|FxAction::Color|FxAction::KeyColor(_)|FxAction::SmokeColor)) {return true;}
        if matches!(self,Self::Helper(HelperAction::Size|HelperAction::Number(_))) {return true;}
        if matches!(self, Self::Text(TextAction::Content|TextAction::Field(_))) {return true;}
        if matches!(self, Self::Collision(CollisionAction::Search)) { return true; }
        if matches!(self, Self::Media(MediaAction::UvNumber(..) | MediaAction::VideoNumber(..)) | Self::Morph(MorphAction::Weight(_) | MorphAction::Rename(_))) { return true; }
        if matches!(self, Self::AssetSearch) { return true; }
        if matches!(self, Self::Curve(CurveAction::Field(_))) { return true; }
        if matches!(self, Self::Path(PathAction::Field(_))) { return true; }
        if matches!(self, Self::Destruction(DestructionAction::Field(_))) { return true; }
        if matches!(self, Self::FluidObject(FluidObjectAction::Field(_))) {
            return true;
        }
        if matches!(self, Self::Normal(NormalAction::Field(_))) {
            return true;
        }
        if matches!(self, Self::Uv(UvAction::Number(_) | UvAction::PanelNumber(_))) {
            return true;
        }
        if matches!(self, Self::Primitive(PrimitiveAction::Field(_))) {
            return true;
        }
        if matches!(
            self,
            Self::Node(NodeAction::Value(_) | NodeAction::Position(_))
        ) {
            return true;
        }
        if let Self::ColorPicker(control) = self {
            return control.is_field();
        }
        if let Self::Animation(control) = self {
            return control.is_field();
        }
        matches!(
            self,
            Self::Author(
                AuthorAction::ProportionalRadius
                    | AuthorAction::LiveField(_)
                    | AuthorAction::WeldDistance
                    | AuthorAction::BridgeSegments
                    | AuthorAction::Coordinate(_)
                    | AuthorAction::RotateDegrees(_)
                    | AuthorAction::ScaleFactor(_)
                    | AuthorAction::InsetAmount
                    | AuthorAction::BevelHeight
                    | AuthorAction::LoopPosition
                    | AuthorAction::ExtendJoint
                    | AuthorAction::PaintRadius
                    | AuthorAction::PaintStrength
            ) | Self::Studio(StudioAction::Field(_))
                | Self::EditTitle
                | Self::EditName
                | Self::EditTransform(_)
                | Self::CameraNumber(_)
                | Self::CameraTool(CameraTool::SaveView)
                | Self::SceneTool(SceneTool::CollectionName(_))
                | Self::EditFps
                | Self::EditDuration
                | Self::EditWidth
                | Self::EditHeight
                | Self::EditFrame
                | Self::EditPath
                | Self::EditBlenderPath
                | Self::MaterialSearch
                | Self::SearchScene
                | Self::SearchBones
                | Self::MaterialCategory
                | Self::MaterialTags
                | Self::SurfaceMaterialName
                | Self::SurfaceMaterialColor(_)
                | Self::SurfaceAttenuationColor
                | Self::SurfaceMaterialNumber(_)
                | Self::SurfaceLightColor
                | Self::SurfaceLightVolumeColor
                | Self::SurfaceLightName
                | Self::SurfaceLightNumber(_)
                | Self::SurfaceRenderNumber(_)
                | Self::RenderColor(_)
                | Self::FluidNumber(_)
                | Self::CreateClip
                | Self::RenameClip
                | Self::DuplicateClip
                | Self::SavePose
        )
    }

    /// Contextual status-bar help. Keep shortcuts aligned with App::keyboard.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::ModelingTool(tool) => Some(tool.title()),
            Self::Author(AuthorAction::SelectAll) => Some("Выбрать все компоненты · A"),
            Self::Author(AuthorAction::Clear) => Some("Снять выделение компонентов"),
            Self::Author(AuthorAction::Grow) => Some("Расширить выделение на соседние компоненты · +"),
            Self::Author(AuthorAction::Shrink) => Some("Убрать граничные компоненты выделения · −"),
            Self::Author(AuthorAction::Connected) => Some("Выделить все связанные компоненты · L"),
            Self::Author(AuthorAction::Invert) => Some("Инвертировать выделение в текущем режиме"),
            Self::Author(AuthorAction::EdgeLoop) => Some("Выбрать петлю рёбер"),
            Self::Author(AuthorAction::EdgeRing) => Some("Выбрать кольцо рёбер"),

            Self::Uv(UvAction::Open) => Some("UV: развёртка слева, модель и свойства справа · U"),
            Self::Uv(UvAction::Close) => Some("Вернуться из UV к сцене"),
            Self::Uv(UvAction::Mode(0)) => Some("Вершины UV · 1"),
            Self::Uv(UvAction::Mode(1)) => Some("Рёбра UV · 2"),
            Self::Uv(UvAction::Mode(_)) => Some("Острова UV · 3"),
            Self::Uv(UvAction::Tool(0)) => Some("Сдвиг UV · G"),
            Self::Uv(UvAction::Tool(1)) => Some("Поворот UV · R"),
            Self::Uv(UvAction::Tool(_)) => Some("Масштаб UV · S"),
            Self::Uv(UvAction::Frame) => Some("Вписать UV в левую область · F"),
            Self::Uv(UvAction::Stretch) => Some("Показать растяжение UV"),
            Self::Rig(RigAction::Endpoint(_)) => Some("Переместить кисть или стопу с IK · Esc — отменить"),
            Self::Rig(RigAction::Controller(_)) => Some("Выбрать часть тела · R — вращение · G — перемещение"),
            Self::Rig(RigAction::Pole) => Some("Направление сгиба локтя или колена; кисть или стопа остаётся на месте"),
            Self::Rig(RigAction::Kinematics(_)) => Some("Выбрать IK или FK без изменения текущей позы"),
            Self::Rig(RigAction::KeyPart) => Some("Поставить ключи выбранной части тела: цепи конечности или одной кости"),
            Self::Rig(RigAction::ResetPart) => Some("Вернуть выбранную часть тела в исходную позу · один шаг Undo"),
            Self::Rig(RigAction::CopyPart) => Some("Скопировать позу выбранной части тела"),
            Self::Rig(RigAction::PastePart) => Some("Вставить позу в ту же часть скелета, сохраняя остальные кости"),
            Self::Rig(RigAction::Handles) => Some("Показать контуры корпуса, головы и круглые IK-контролы кистей, стоп и сгиба"),
            Self::Author(AuthorAction::Xray)=>Some("Сквозной выбор вершин и рёбер выбранного меша · Alt+Z"),
            Self::Author(AuthorAction::PivotOrigin)=>Some("Центр действия: начало объекта или центр уникальных выбранных вершин"),
            Self::Author(AuthorAction::Bevel)=>Some("Фаска выбранных вершин, рёбер или граней · B · параметры зависят от режима выделения"),
            Self::Author(AuthorAction::Inset)=>Some("Вставить меньшую грань с четырёхугольным ободком · I"),
            Self::Author(AuthorAction::LoopCut)=>Some("Разрез непрерывной полосы четырёхугольников через выбранное ребро · C"),
            Self::DropdownOpen(DropdownKind::MeshCreate) => Some("Добавить меш · базовые формы, цилиндр, конус, тор и капсула · ↑/↓ и Enter"),
            Self::DropdownOpen(DropdownKind::CurveCreate) => Some("Создать кривую или нарисовать мышью · параметры появятся в просмотре"),
            Self::DropdownOpen(DropdownKind::SkeletonCreate) => Some("Создать объект скелета · пустая кость, цепь или гуманоид · ↑/↓ и Enter"),
            Self::Author(AuthorAction::CreateBone) => Some("Добавить отдельную кость без меша и привязки в центре координат"),
            Self::CameraTool(CameraTool::Effects) => Some("Эффекты выбранной камеры · вид из камеры, режим Материал или Рендер"),
            Self::DropdownOpen(DropdownKind::CameraEffectAdd) => Some("Добавить эффект камеры · ↑/↓ и Enter · Esc — отмена"),
            Self::CameraTool(CameraTool::AddEffect(_)) => Some("Добавить эффект в список выбранной камеры"),
            Self::CameraTool(CameraTool::SelectEffect(_)) | Self::CameraEffect(_,CameraTool::SelectEffect(_)) => Some("Эффект камеры · перетащите для порядка · Alt+↑/↓ — переставить · Enter — значение · пробел — глаз"),
            Self::CameraTool(CameraTool::MoveEffect(..)) | Self::CameraEffect(_,CameraTool::MoveEffect(..)) => Some("Переставить эффект в списке · Alt+↑/↓ · Ctrl+Z — отменить"),
            Self::CameraEffect(_,CameraTool::ToggleEffect(_)) => Some("Включить или выключить эффект этой камеры с сохранением параметров"),
            Self::CameraEffect(_,CameraTool::RemoveEffect(_)) => Some("Удалить эффект этой камеры · Ctrl+Z — отменить"),
            Self::CameraTool(CameraTool::ToggleEffect(_)) => Some("Включить или выключить эффект с сохранением его параметров"),
            Self::CameraTool(CameraTool::RemoveEffect(_)) => Some("Удалить эффект камеры · Ctrl+Z — отменить"),
            Self::Modifier(ModifierAction::Select(..)) => Some("Параметры модификатора · перетащите для порядка или переноса · Ctrl — копия"),
            Self::Modifier(ModifierAction::Toggle(..)) => Some("Включить или выключить модификатор без изменения исходной сетки"),
            Self::Modifier(ModifierAction::Remove(..)) => Some("Удалить модификатор из стека · Ctrl+Z — отменить"),
            Self::Modifier(ModifierAction::Apply(_)) => Some("Записать результат в меш и очистить стек · Ctrl+Z возвращает исходную сетку и весь стек"),
            Self::Modifier(ModifierAction::Reset(..)) => Some("Вернуть начальные параметры этого модификатора"),
            Self::DropdownOpen(DropdownKind::ModifierAdd) => Some("Добавить модификатор в конец стека · ↑/↓ и Enter"),
            Self::DropdownOpen(DropdownKind::ModifierTarget(..)) => Some("Выбрать меш для переноса или копирования модификатора"),
            Self::DropdownOpen(DropdownKind::LightCreate) => Some("Создать источник света · ↑/↓ и Enter"),
            Self::DropdownOpen(DropdownKind::MaterialActions) => Some("Действия с материалом"),
            Self::DropdownOpen(DropdownKind::MaterialCreate) => Some("Создать и назначить материал"),
            Self::DropdownOpen(DropdownKind::SceneFilter) => Some("Фильтр объектов: меши, кости, освещение, камеры"),
            Self::SceneTool(SceneTool::NewCollection) => Some("Создать папку с выделенными объектами"),
            Self::DropdownOpen(DropdownKind::Accent) => Some("Цвет акцента интерфейса · ↑/↓ и Enter"),
            Self::DropdownOpen(DropdownKind::MaterialLibrary) => Some("Библиотека материалов · выбор и перетаскивание · Alt+↓"),
            Self::DropdownOpen(DropdownKind::AssetLibrary) => Some("Библиотека · категории, поиск и миниатюры"),
            Self::DropdownOpen(DropdownKind::TextFont) => Some("Шрифт текста · встроенные начертания с кириллицей"),
            Self::OpenColorPicker(_) => Some("Выбрать цвет · двойной щелчок — ввод HEX · Alt+↓ — палитра"),
            Self::ResetColor(_) => Some("Сбросить только цвет · Ctrl+Z — отменить"),
            Self::ColorPicker(control) => Some(control.hint()),
            Self::CopyReport => Some("Скопировать весь открытый отчёт в буфер обмена · Ctrl+C"),
            Self::Animation(control) => Some(control.hint()),
            Self::ResetNumber(_) => Some("Сбросить значение параметра"),
            Self::Slide(..) => Some("Потяните для изменения · Двойной щелчок — ввод числа · Shift — точно · Esc — отменить"),
            Self::CreateCamera => Some("Создать камеру в сцене"),
            Self::Studio(StudioAction::CameraLock) => Some("Связать навигацию с камерой: вращение и перемещение меняют её положение"),
            Self::Studio(StudioAction::LayerEnabled(_)) => Some("Включить / выключить слой"),
            Self::Studio(StudioAction::LayerSolo(_)) => Some("Соло слоя"),
            Self::Studio(StudioAction::LayerRemove(_)) => Some("Удалить слой"),
            Self::CameraView => Some("Вид через камеру; если камер нет, создать из текущего вида"),
            Self::CameraExitView => Some("Вернуться к свободному обзору"),
            Self::CameraSetActive => Some("Использовать выбранную камеру для рендера"),
            Self::CameraClearActive => Some("Использовать сохранённый ракурс проекта для рендера"),
            Self::BrowsePath => Some("Выбрать файл или папку в системном окне"),
            Self::PastePath => Some("Вставить путь из буфера обмена в поле"),
            Self::BrowseBlenderPath => Some("Найти исполняемый файл Blender в системном окне"),
            Self::PasteBlenderPath => Some("Вставить путь к Blender из буфера обмена"),
            Self::SurfaceTab(0) => Some("Объект · трансформация, камера или свет выбранного объекта · Ctrl+PageUp/PageDown — вкладки"),
            Self::SurfaceTab(5) => Some("Слои анимации сцены · Ctrl+PageUp/PageDown — вкладки"),
            Self::SurfaceTab(3) => Some("Рендер · качество, тени, цвет изображения и параметры PNG · Ctrl+PageUp/PageDown — вкладки"),
            Self::SurfaceTab(4) => Some("Физика · жидкость выбранного примитива"),
            Self::SurfaceTab(13) => Some("Физика · разрушение: осколки, разлёт и запись падения"),
            Self::FluidObject(FluidObjectAction::Play) => Some("Запустить или приостановить жидкость без ключей и движения шкалы анимации"),
            Self::FluidObject(FluidObjectAction::Reset) => Some("Вернуть жидкость в исходное состояние · анимация и ключи сохраняются"),
            Self::Studio(StudioAction::PhysicsPlay) => Some("Запустить или приостановить ткань без записи ключей"),
            Self::Studio(StudioAction::PhysicsReset) => Some("Сбросить предпросмотр ткани · исходный меш и анимация сохраняются"),
            Self::Destruction(DestructionAction::Play) => Some("Запустить или приостановить падение осколков без записи ключей"),
            Self::Destruction(DestructionAction::Reset) => Some("Вернуть осколки в исходное состояние · записанные ключи сохраняются"),
            Self::Destruction(DestructionAction::Create) => Some("Разбить выпуклый меш на закрытые объёмные осколки · исходник сохраняется"),
            Self::Destruction(DestructionAction::Bake) => Some("Записать падение и отскок от пола и включённых мешей-коллайдеров"),
            Self::Destruction(DestructionAction::MeshCollision) => Some("Использовать меши из списка столкновений в предпросмотре и при записи анимации"),
            Self::Destruction(DestructionAction::ShardCollision) => Some("Столкновения между осколками с включённым флажком · предпросмотр и запись анимации"),
            Self::FluidObject(FluidObjectAction::MeshCollision) => Some("Использовать меши из списка столкновений · поверхность жидкости обновляется автоматически"),
            Self::Text(TextAction::Create) => Some("Добавить объёмный текст · кириллица, латиница, цифры"),
            Self::Text(TextAction::Convert) => Some("Преобразовать надпись в редактируемые полигоны · Undo вернёт текст"),
            Self::Text(TextAction::OnCurve) => Some("Изогнуть текст вдоль привязанной кривой · путь выбирается ниже"),
            Self::Helper(HelperAction::CreateField(_)) | Self::DropdownOpen(DropdownKind::ForceCreate) => Some("Создать поле · сила, ветер, вихрь или магнит · действует на симуляцию"),
            Self::Helper(HelperAction::Number(_)|HelperAction::Size) => Some("Перетащить для изменения · двойной щелчок — точное значение · Esc — отмена"),
            Self::Fx(FxAction::Create(_)) | Self::DropdownOpen(DropdownKind::FxCreate) => Some("Добавить объёмный FX · огонь, дым, туман или пар"),
            Self::Fx(FxAction::Play) => Some("Общий запуск или пауза FX · без записи ключей"),
            Self::Fx(FxAction::Reset) => Some("Общие часы FX к началу выбранного эффекта · настройки и ключи сохраняются"),
            Self::Fx(FxAction::Color) => Some("Цвет FX в формате #RRGGBB"),
            Self::Fx(FxAction::Gas) => Some("Перенос плотности и расчёт давления в объёме · работа в фоне · готовые кадры кэшируются"),
            Self::Fx(FxAction::Hollow) => Some("Учитывать только оболочку меша · включите для комнаты или полого контейнера"),
            Self::Fx(FxAction::ClearCache) => Some("Отменить расчёт и очистить кэш всех газовых FX · настройки сохраняются"),
            Self::Fx(FxAction::Evolution) => Some("Размер, плотность, свечение и оттенок по времени FX · четыре точки от 0 до 100%"),
            Self::Fx(FxAction::Smooth) => Some("Плавная интерполяция без выхода за значения соседних точек"),
            Self::Fx(FxAction::Point(_)) => Some("Выбрать точку времени · значения редактируются ниже"),
            Self::Fx(FxAction::Channel(_)) => Some("Показать график выбранного параметра"),
            Self::Fx(FxAction::KeyColor(_)) => Some("Оттенок точки #RRGGBB · умножается на основной цвет FX"),
            Self::Fx(FxAction::Upgrade) => Some("Перевести выбранный эффект на расчёт топлива, горения и дыма · Undo возвращает прежний FX"),
            Self::Fx(FxAction::Field(64)) => Some("Подача в секунду; для импульса — полное количество, для начального объёма — заполнение"),
            Self::Fx(FxAction::Field(65)) => Some("Температура источника в кельвинах · ниже порога зажигания топливо не горит"),
            Self::Fx(FxAction::Field(67)) => Some("Температура, при которой начинается расход топлива"),
            Self::Fx(FxAction::Field(74)) => Some("Длительность подачи взрыва · после неё остаются горение и движение дыма"),
            Self::Fx(FxAction::SelfShadow) => Some("Поглощение света внутри этого объёма от самого сильного источника · упрощается при движении вида"),
            Self::Fx(FxAction::TemperatureColor) => Some("Цвет пламени по температуре · выбранный цвет служит оттенком"),
            Self::Fx(FxAction::Field(66..=77)) => Some("Параметр нового FX · двойной щелчок вводит число · Esc отменяет изменение"),
            Self::Fx(FxAction::Field(17)) => Some("Размер расчётной области относительно источника · выходящий газ рассеивается"),
            Self::Fx(FxAction::Field(18)) => Some("Скорость уменьшения плотности газа · 0 сохраняет дым внутри закрытого объёма"),
            Self::Fx(FxAction::Field(19)) => Some("Подъём горячего газа · силовые поля действуют дополнительно"),
            Self::Fx(FxAction::Field(32..=47)) => Some("Параметр выбранной точки · перетащить или ввести число двойным щелчком · Esc отменяет"),
            Self::Fx(FxAction::Illuminate) => Some("Огонь освещает меши точечным светом без теней · используются свободные места из 8 источников"),
            Self::Fx(FxAction::Defaults) => Some("Начальные параметры текущего FX · трансформация, время и включение сохраняются · Undo отменяет сброс"),
            Self::Fx(FxAction::Source(None)) => Some("Отвязать источник FX с сохранением положения · Undo отменяет действие"),
            Self::Fx(FxAction::Fields) => Some("Сила, ветер, вихрь и магнит изменяют поток FX"),
            Self::Fx(FxAction::Cloud) => Some("Округлое облако вместо непрерывного столба · работает с огнём, дымом, туманом и паром"),
            Self::Fx(FxAction::Burst) => Some("Расширение и затухание за заданное время · четыре прежних типа FX"),
            Self::Fx(FxAction::Loop) => Some("Повторять цикл FX · скорость меняет темп и длительность"),
            Self::Fx(FxAction::Field(15)) => Some("Длительность при скорости 1 · текущая скорость FX меняет темп"),
            Self::Fx(FxAction::Field(16)) => Some("Момент запуска на общих часах FX, в секундах"),
            Self::Fx(FxAction::MeshCollision) => Some("Столкновения потока с поверхностями выбранных мешей"),
            Self::Collision(CollisionAction::SourceOpen) => Some("Источник FX · поиск меша или кривой"),
            Self::Collision(CollisionAction::SourcePick) => Some("Выбрать источник FX в просмотре или сцене · Esc — отмена"),
            Self::Fx(FxAction::Field(6|7)) => Some("Доля ширины или высоты внутри области FX · общий размер задаётся масштабом объекта"),
            Self::Fx(FxAction::Field(10|11)) => Some("Снос верхней части эффекта по локальной X или Z · это форма, не сила ветра"),
            Self::Fx(_) | Self::DropdownOpen(DropdownKind::FxKind) => Some("Параметры объёмного эффекта · изменения отменяются через Undo"),
            Self::Helper(HelperAction::Play) => Some("Запустить или приостановить выбранные у поля типы физики · без записи ключей"),
            Self::Helper(HelperAction::Reset) => Some("В начало · сбросить предпросмотр выбранных у поля типов физики, сохранив анимацию и настройки"),
            Self::Helper(_) => Some("Параметры пустышки или силового поля · изменения отменяются через Undo"),
            Self::DropdownOpen(DropdownKind::EmptyCreate|DropdownKind::EmptyShape) => Some("Оси, стрелки, окружность, куб, сфера или конус · без геометрии в рендере"),
            Self::DropdownOpen(DropdownKind::ForceKind) => Some("Тип действия силового поля"),
            Self::CreateEmpty => Some("Добавить пустышку в центре координат · трансформация и родитель для объектов"),
            Self::Primitive(PrimitiveAction::Orientation(_)) => Some("Ориентация при создании · направить основную ось по X, Y или Z · Undo"),
            Self::Collision(CollisionAction::Open) => Some("Добавить меш в столкновения · поиск по имени · ↑/↓ и Enter"),
            Self::Collision(CollisionAction::Pick) => Some("Пипетка столкновений · выберите меш в просмотре или сцене · Esc — отмена"),
            Self::Collision(CollisionAction::Toggle(..)) => Some("Включить или выключить столкновения с этим мешем"),
            Self::Collision(CollisionAction::Remove(..)) => Some("Убрать меш из списка столкновений · объект остаётся в сцене"),
            Self::Destruction(DestructionAction::Restore) => Some("Убрать созданные осколки и вернуть исходный меш · действие можно отменить"),
            Self::Destruction(DestructionAction::Reseed) => Some("При пересоздании менять вариант разрезов; выключите для повторяемого результата"),
            Self::Destruction(DestructionAction::Floor) => Some("Столкновения осколков с полом при предпросмотре и записи анимации"),
            Self::Destruction(DestructionAction::ShowFloor) => Some("Показать прозрачную плоскость пола только в редакторе"),
            Self::Destruction(DestructionAction::Dust) => Some("Пылевое облако в предпросмотре и рендере · размер, плотность и время ниже"),
            Self::SurfaceTab(14) => Some("Библиотека · примитивы, материалы, кривые, свет, сцены, камеры и скелеты"),
            Self::Curve(CurveAction::Point(-1)) => Some("Предыдущая точка кривой"),
            Self::Curve(CurveAction::Point(_)) => Some("Следующая точка кривой"),
            Self::Curve(CurveAction::Draw) => Some("Нарисовать кривую мышью в плоскости текущего вида"),
            Self::Path(PathAction::Field(0)) => Some("Положение вдоль длины кривой · 0–100 % · двойной щелчок — точный ввод"),
            Self::Path(PathAction::Field(_)) => Some("Скорость по времени сцены · 0 — без движения · отрицательное значение — назад"),
            Self::Path(PathAction::Pick) => Some("Выбрать объект, скелет или кость в сцене или списке · Esc / правая кнопка — отмена"),
            Self::Path(PathAction::Edit) => Some("Edit · свободное перемещение гизмо без разрыва привязки · повторное нажатие — вдоль кривой"),
            Self::Path(PathAction::ResetOffset) => Some("Сбросить смещение и вернуть объект на кривую · положение на пути, поворот и масштаб сохраняются"),
            Self::Path(PathAction::SelectCurve) => Some("Выделить объект привязки"),
            Self::Path(PathAction::Clear) => Some("Убрать привязку, сохранив текущее положение объекта"),
            Self::Path(PathAction::Follow) => Some("Наследовать ориентацию цели; для кривой — направление по касательной"),
            Self::Path(PathAction::Repeat) => Some("Повторять проход · на открытой кривой возврат в начало будет скачком"),
            Self::MaterialUsers(_) => Some("Раскрыть объекты, которым назначен материал"),
            Self::MaterialUserSelect(..) => Some("Выделить полигоны меша, назначенные этому материалу"),
            Self::MaterialUserDetach(..) => Some("Удалить назначение этой группы полигонов; материал остаётся"),
            Self::MaterialApplyFaces => Some("Назначить выбранный материал выделенным полигонам меша"),
            Self::MaterialUnassignFaces => Some("Снять выбранный материал только с выделенных полигонов"),
            Self::SurfaceTab(12) => Some("UV · выделение, трансформация, швы и развёртка"),
            Self::SurfaceTab(11) => Some("Физика · симуляция ткани"),
            Self::RenderAntialiasing(_) => Some("Сглаживание краёв: 4× требует больше ресурсов; доступный режим зависит от видеокарты"),
            Self::RenderToggleShadows => Some("Тени от включённых источников: солнце, точка и прожектор; отдельный флажок — в свойствах света"),
            Self::RenderShadowResolution(_) => Some("Целевое разрешение; при большом числе источников движок уменьшает его в пределах бюджета памяти теней"),
            Self::SurfaceRenderNumber(4) => Some("Радиус фильтра глубины в текселях; это сглаживание тени, а не физический размер источника"),
            Self::RenderToneMapping(_) => Some("Преобразование яркости и бликов в итоговое изображение"),
            Self::RenderQuality(_) => Some("Изменить качество сглаживания и теней одним действием"),
            Self::LightingPreset(_) => Some("Заменить схему света; Undo вернёт предыдущие источники"),
            Self::RenderColor(_) => Some("Цвет sRGB #RRGGBB; сохраняется в проекте"),
            Self::Fluid => Some("Подключить жидкость к таймлайну · ▶ / пробел — запуск и пауза"),
            Self::ResetFluid => Some("Остановить таймлайн и вернуть жидкость к нулевому кадру"),
            Self::FluidNumber(_) => Some("Числовой параметр жидкости; изменение физики начинает расчёт заново"),
            Self::FluidPreset(_) => Some("Применить параметры жидкости; геометрия сцены сохраняется"),
            Self::FluidFrame => Some("Показать область расчёта жидкости целиком"),
            Self::FluidResetSettings => Some("Вернуть стандартные параметры жидкости"),
            Self::SurfaceTab(6) => Some("Физика выбранного меша: ткань, столкновения, трение и изгиб"),
            Self::SurfaceTab(7) => Some("Геометрия: редактирование вершин и граней · F2"),
            Self::RigStage(0) => Some("Скелет и IK: создание костей, положение и проверка цепи"),
            Self::BoneDisplay(BoneDisplayAction::BodyColor) => Some("Цвет выбранной кости · предпросмотр, HEX, Enter — применить, Esc — отменить"),
            Self::BoneDisplay(BoneDisplayAction::JointColor) => Some("Отдельный цвет сустава выбранной кости"),
            Self::BoneDisplay(BoneDisplayAction::ResetBodyColor) => Some("Сбросить только цвет кости · форму и цвет сустава сохранить · Undo"),
            Self::BoneDisplay(BoneDisplayAction::ResetJointColor) => Some("Сбросить только цвет сустава · форму и цвет кости сохранить · Undo"),
            Self::DropdownOpen(DropdownKind::BoneShape) => Some("Форма кости · ↑/↓ — выбор, Enter — применить, Esc — закрыть"),
            Self::DropdownOpen(DropdownKind::JointShape) => Some("Форма сустава · ↑/↓ — выбор, Enter — применить, Esc — закрыть"),
            Self::BoneDisplay(BoneDisplayAction::BodyFollowView) => Some("Разворачивать форму вокруг оси кости к камере; выключено — ориентация кости"),
            Self::BoneDisplay(BoneDisplayAction::JointFollowView) => Some("Обращать сустав к камере; выключено — плоскость XY выбранной кости"),
            Self::BoneDisplay(BoneDisplayAction::BodyFilled | BoneDisplayAction::JointFilled) => Some("Переключить заливку и контур; оформление не меняет привязку"),
            Self::BoneDisplay(BoneDisplayAction::BodyShape(_) | BoneDisplayAction::JointShape(_)) => Some("Изменить форму выбранной кости или её сустава"),
            Self::BoneDisplay(BoneDisplayAction::Reset) => Some("Вернуть серую гранёную кость и круглый сустав · можно отменить через Undo"),
            Self::RigStage(1) => Some("Риг: привязка скелета к геометрии и редактирование весов"),
            Self::RigStage(2) => Some("Выставление в позу: части тела, FK/IK и ключи"),
            Self::RigReferencePick(RigReference::Skeleton) => Some("Пипетка скелета · выберите кость, скелет в сцене или его привязанный меш · Esc — отмена"),
            Self::RigReferencePick(RigReference::Mesh) => Some("Пипетка меша · выберите меш в просмотре или в сцене · Esc — отмена"),
            Self::SurfaceTab(10) => Some("Нормали: угол сглаживания, взвешенный пересчёт, направление и разворот граней"),
            Self::Shape(ShapeAction::Sphere) => Some("Приблизить выбранные компоненты к сфере"),
            Self::Shape(ShapeAction::Circle) => Some("Выровнять выделение по окружности в локальной плоскости"),
            Self::Shape(ShapeAction::Inflate) => Some("Сдвинуть выделение по геометрическим нормалям"),
            Self::Shape(ShapeAction::Twist) => Some("Скрутить выделение вдоль локальной оси"),
            Self::Shape(ShapeAction::Axis(_)) => Some("Локальная ось инструмента"),
            Self::Shape(ShapeAction::Straighten) => Some("Выпрямить открытую цепочку; концы остаются на месте"),
            Self::Shape(ShapeAction::Space) => Some("Распределить вершины равномерно вдоль выбранной цепочки"),
            Self::Shape(ShapeAction::Flatten(_)) => Some("Выровнять компоненты по средней координате локальной оси"),
            Self::Normal(NormalAction::Flip) => Some("Развернуть выбранные грани вместе с нормалями; Undo отменяет"),
            Self::Normal(NormalAction::Flat) => Some("Жёсткие нормали выбранной области; без выделения — весь меш"),
            Self::Normal(NormalAction::Smooth) => Some("Сгладить нормали выбранной области; без выделения — весь меш"),
            Self::Normal(NormalAction::Angle(_)) => Some("Выбрать угол сглаживания; затем нажмите «Пересчитать»"),
            Self::Normal(NormalAction::Display(0)) => Some("Показать нормали вершин: направления сглаживания поверхности"),
            Self::Normal(NormalAction::Display(_)) => Some("Показать геометрические нормали граней: направление лицевой стороны"),
            Self::Normal(NormalAction::AutoLength) => Some("Подобрать длину линий по размеру объекта в сцене"),
            Self::Normal(NormalAction::XRay) => Some("Показывать направления нормалей через поверхности"),
            Self::Author(AuthorAction::LiveDrag(_)) => Some("Тяните стрелку вдоль оси или кольцо от центра; точное значение — в панели операции"),
            Self::Author(AuthorAction::LiveToggle) => Some("Развернуть или свернуть параметры операции"),
            Self::Author(AuthorAction::LiveAccept) => Some("Применить предпросмотр · Enter"),
            Self::Author(AuthorAction::LiveCancel) => Some("Отменить предпросмотр · Esc"),
            Self::SurfaceTab(9) => Some("Настройки программы: тема, акцент, просмотр и импорт"),
            Self::InterfaceTheme(_) => Some("Изменить тему интерфейса"),
            Self::SurfaceTab(8) => Some("Модификаторы · порядок, параметры и перенос между мешами"),
            Self::Mode(0) => Some("Объект · завершить редактирование сетки и выбирать объекты целиком"),
            Self::Primitive(PrimitiveAction::CurvePoint(_)) => Some("Выбрать контрольную точку Безье для редактирования X/Y/Z"),
            Self::Primitive(PrimitiveAction::Open) => Some("Открыть полупрозрачную панель параметров меша или кривой"),
            Self::Primitive(PrimitiveAction::Collapse) => Some("Свернуть или развернуть параметры фигуры"),
            Self::Primitive(PrimitiveAction::Close) => Some("Закрыть панель, сохранив фигуру · Esc"),
            Self::Primitive(PrimitiveAction::Reset) => Some("Вернуть исходные размеры и подразделения · можно отменить"),
            Self::Author(AuthorAction::SelectMode(MeshSelection::Vertex)) => Some("Вершина · 1 в режиме сетки · Shift: добавить или снять выбор · потяните выделение"),
            Self::Author(AuthorAction::SelectMode(MeshSelection::Edge)) => Some("Ребро · 2 в режиме сетки · Shift: добавить или снять выбор · потяните выделение"),
            Self::Author(AuthorAction::SelectMode(MeshSelection::Face)) => Some("Грань · 3 в режиме сетки · Shift: добавить или снять выбор · E: экструдировать"),
            Self::Author(_) => Some("Редактирование геометрии и скелета с Undo/Redo"),
            Self::SurfaceTab(1) => Some("Материал выбранного объекта · цвет, карты, стекло и ткань · Ctrl+PageUp/PageDown — вкладки"),
            Self::SurfaceTab(2) => Some("Добавление источников света и настройка рендера в реальном времени"),
            Self::SurfaceNextMesh => Some("Выбрать следующий меш сцены для редактирования материала"),
            Self::SurfaceCreateMaterial => Some("Создать и назначить новый материал выбранному объекту или ветви · Alt+N"),
            Self::SurfaceMaterialName => Some("Имя общего материала; Enter — сохранить, Esc — отменить"),
            Self::SurfaceMaterialList => Some("Назначить материал проекта · Alt+↓ · стрелки и Enter · Esc без изменений"),
            Self::SurfaceAssignMaterial(_) => Some("Назначить существующий материал; остальные экземпляры вне выбранной ветви сохранят прежнюю привязку"),
            Self::SurfaceMaterialNumber(7) => Some("IOR управляет отражением диэлектрика; прохождение света и преломление пока не моделируются"),
            Self::SurfaceMaterialNumber(8) => Some("Вес верхнего отражающего слоя лака"),
            Self::SurfaceMaterialNumber(9) => Some("Шероховатость верхнего слоя лака"),
            Self::SurfaceToggleLightShadow => Some("Включить или выключить тень этого источника; сам свет остаётся включённым · Alt+S"),
            Self::SurfaceUniqueMaterial => Some("Отделить материал выбранного объекта/ветви; для меша вне выделения — всех его экземпляров · Alt+D"),
            Self::SurfaceMaterialColor(true) => Some("Оттенок свечения сохраняет силу HDR; чёрный выключает свечение"),
            Self::SurfaceAttenuationColor => Some("Цвет пропущенного света на длине поглощения; белый не окрашивает стекло"),
            Self::Studio(StudioAction::AbsorptionFromBase) => Some("Использовать цвет материала и его карту для поглощения, как в прежних проектах"),
            Self::SurfaceMaterialNumber(10) => Some("Вытянутость блика: направление следует UV-развёртке поверхности"),
            Self::SurfaceMaterialNumber(11) => Some("Поворот вытянутого блика относительно UV-развёртки, в градусах"),
            Self::SurfaceMaterialNumber(12) => Some("Мягкий ворсистый блеск; действует на непрозрачную неметаллическую часть материала"),
            Self::SurfaceMaterialNumber(13) => Some("Шероховатость ворсистого слоя: малое значение — узкий краевой блик, большое — мягкий и широкий"),
            Self::SurfaceMaterialColor(_) | Self::SurfaceLightColor => Some("Введите sRGB HEX #RRGGBB; значения сохраняются в линейном пространстве"),
            Self::SurfaceNextTexture(_) => Some("Выбрать следующую текстуру проекта; после последней — без карты"),
            Self::SurfaceImportTexture(_) => Some("Загрузить PNG или JPEG в этот слот материала"),
            Self::SurfaceClearTexture(_) => Some("Отключить карту; сам ресурс останется в проекте"),
            Self::SurfaceEmissionVolume => Some("Рассеивать свечение материала в общей среде · цвет задаётся свечением"),
            Self::SurfaceLightVolume => Some("Объём этого источника · собственные сила и оттенок; среда общая для всех источников"),
            Self::SurfaceLightVolumeColor => Some("Оттенок объёма умножается на цвет выбранного источника"),
            Self::SurfaceLightTemperature => Some("Умножить оттенок на цвет теплового источника; температура сохраняется при выключении"),
            Self::SurfaceLightNumber(14) => Some("Направление света по горизонтали, −180…180° · 0°: −Z, 90°: +X"),
            Self::SurfaceLightNumber(15) => Some("Наклон света, −90…90° · положительное значение направляет луч вверх"),
            Self::SurfacePointLightAtView => Some("Направить луч на центр свободного вида, сохранив положение источника"),
            Self::SurfaceResetMapping => Some("Вернуть масштаб UV 1×1, сдвиг 0 и поворот 0° одной операцией Undo"),
            Self::SurfaceAimLight => Some("Расположить свет перед камерой и направить на центр вида"),
            Self::SurfacePreset(_) => Some("Основа поверхности, включая анизотропию и ворс; цвет, поглощение, карты и свечение сохраняются · Alt+1…5"),
            Self::ToggleProjectMenu => Some("Открыть, импортировать, сохранить или экспортировать проект"),
            Self::ToggleTools => Some("Свернуть или развернуть инструменты"),
            Self::ViewAxis(0 | 1) => Some("Ось X · повторное нажатие — противоположный вид"),
            Self::ViewAxis(2 | 3) => Some("Ось Y · повторное нажатие — противоположный вид"),
            Self::ViewAxis(_) => Some("Ось Z · повторное нажатие — противоположный вид"),
            Self::ViewOrbit => Some("Потяните для вращения вида"),
            Self::ObjectScroll | Self::MaterialScroll => Some("Прокрутить список"),
            Self::MaterialAssign(..) => Some("Выбрать материал; перетащите для назначения · Esc — отменить"),
            Self::MaterialFavorite => Some("Добавить или убрать текущий материал из избранного"),
            Self::MaterialFavorites => Some("Показывать только избранные материалы"),
            Self::MaterialGrid => Some("Переключить список и плитки материалов"),
            Self::MaterialSize => Some("Размер сфер: 36, 52 или 72 пикселя"),
            Self::MaterialCategory => Some("Категория материала · до 64 символов"),
            Self::MaterialTags => Some("Теги через запятую · используются при поиске"),
            Self::SurfaceUnassignMaterial => Some("Снять материал с выбранного меша · можно отменить"),
            Self::SurfacePruneMaterials => Some("Убрать неиспользуемые материалы из библиотеки · можно отменить"),
            Self::MaterialSearch => Some("Поиск по имени, категории и тегам · ↑/↓ — выбор · Enter — назначить · Esc — закрыть"),
            Self::AssetSearch => Some("Поиск в выбранной категории библиотеки"),
            Self::AssetSearchClear => Some("Очистить поиск ресурсов"),
            Self::MaterialSearchClear => Some("Сбросить поиск материалов"),
            Self::ViewShading(mode) => Some(match mode { 0 => "Каркас · рёбра сетки", 1 => "Сплошной · нейтральная поверхность",
                2 => "Материал · материалы под студийным светом", _ => "Рендер · свет и эффекты сцены" }),
            Self::ToggleViewMenu => Some("Ракурс камеры, сетка и отображение скелета"),
            Self::ToggleBodyMenu => Some("Выбрать часть тела и способ позирования"),
            Self::CharacterPanel => Some("Распознать части тела и настроить соответствия костей"),
            Self::AnimationPanel => Some("Клипы, кадры и запись полной позы"),
            Self::KeyPose => Some("Записать все суставы активного персонажа одним действием · Shift+K"),
            Self::PrevFrame => Some("Предыдущий кадр · стрелка влево"),
            Self::NextFrame => Some("Следующий кадр · стрелка вправо"),
            Self::EditFrame => Some("Перейти к точному номеру кадра"),
            Self::EditBlenderPath => Some("Путь к установленному Blender; пустая строка включает автопоиск"),
            Self::DetectBlender => Some("Проверить путь к Blender для импорта .blend"),
            Self::ToggleBlendImportAnimations => Some("Готовые клипы из .blend; модель, текстуры и скелет загружаются всегда · Tab / Пробел"),
            Self::New => Some("Новая пустая сцена"),
            Self::Open => Some("Открыть проект · Ctrl+O"),
            Self::Save => Some("Сохранить проект · Ctrl+S"),
            Self::SaveAs => Some("Сохранить проект в новый файл · Ctrl+Shift+S"),
            Self::Import => Some("Добавить модель GLB/glTF или OBJ · Ctrl+I"),
            Self::ImportReport => Some("Ресурсы и предупреждения последнего импорта"),
            Self::Undo => Some("Отменить последнее изменение · Ctrl+Z"),
            Self::Redo => Some("Повторить изменение · Ctrl+Shift+Z / Ctrl+Y"),
            Self::ToggleAnimationPanel => Some("Показать или скрыть панель анимации"),
            Self::ToggleAnimationLayers => Some("Показать или скрыть слои анимации"),
            Self::AnimationLayersScroll => Some("Прокрутка слоёв анимации"),
            Self::ToggleLeftPanel => Some("Показать или скрыть дерево сцены · Tab"),
            Self::ToggleRightPanel => Some("Показать или скрыть свойства · Shift+Tab"),
            Self::SearchScene => Some("Найти объект или кость по имени · Ctrl+F"),
            Self::ClearSearch => Some("Очистить поиск по сцене"),
            Self::LightTool(LightTool::PickTarget) => Some("Выбрать объект и направить свет один раз"),
            Self::LightTool(LightTool::PickTrackedTarget) => Some("Выбрать объект, за которым свет будет следить при движении и анимации"),
            Self::LightTool(LightTool::StopTracking) => Some("Сохранить текущее направление и отключить слежение"),
            Self::ImportRig => Some("Загрузить TXT со списком костей для проверки рига"),
            Self::ToggleGrid => Some("Показать или скрыть сетку рабочего пространства"),
            Self::ToggleBackfaces => Some("Отображать задние грани в просмотре · материалы и экспорт сохраняют свои настройки"),
            Self::ToggleSkeleton => Some("Показать или скрыть кости поверх модели · выбор по всей длине"),
            Self::Move => Some("Перемещение · потяните стрелку или центральный квадрат"),
            Self::Rotate => Some("Вращение · цветные кольца задают ось; внешнее — поворот по виду"),
            Self::Scale => Some("Масштаб · потяните квадрат оси или центр для всех осей"),
            Self::Ik => Some("Включить или выключить IK выбранной цепочки"),
            Self::ToggleGizmoSpace => Some("Мировые / локальные оси · L · Масштаб всегда в локальных осях"),
            Self::Orbit => Some("Обзор · ПКМ: орбита · СКМ: панорама · Колесо: масштаб"),
            Self::Key | Self::SetKey => Some("Записать текущую позу в ключ анимации · K"),
            Self::AutoKey => Some("Автоматически записывать ключи при изменении позы"),
            Self::FrameSelected => Some("Приблизить камеру к выбранному объекту · F"),
            Self::Front => Some("Вид спереди · 1"),
            Self::Side => Some("Вид сбоку · 3"),
            Self::Top => Some("Вид сверху · 7"),
            Self::ViewPan => Some("Сдвиг вида · потяните руку · Shift — точнее · Esc — отмена"),
            Self::ViewZoom => Some("Масштаб вида · потяните лупу вверх/вниз · Shift — точнее · Esc — отмена"),
            Self::Perspective => Some("Переключить перспективную и ортографическую камеру"),
            Self::Play => Some("Воспроизведение / пауза · Пробел"),
            Self::Start => Some("Перейти к началу анимации"),
            Self::PreviousKey => Some("Перейти к предыдущему ключу"),
            Self::NextKey => Some("Перейти к следующему ключу"),
            Self::Reset => Some("Вернуть выбранный сустав в исходную позу"),
            Self::Delete => Some("Удалить выбранный объект · Delete"),
            Self::SceneDelete(poseforge_core::SceneItem::Folder(_)) => Some("Удалить папку; содержимое останется в сцене"),
            Self::SceneDelete(_) | Self::SceneDeleteLight(..) => Some("Удалить из сцены · Ctrl+Z — отменить"),
            Self::SceneExpand(_) => Some("Развернуть или свернуть материалы и дочерние объекты"),
            Self::SceneCloth(_) => Some("Ткань примитива · открыть настройки"),
            Self::SceneClothVisible(_) => Some("Показать или скрыть деформацию ткани · исходный меш сохраняется"),
            Self::SceneClothRemove(_) => Some("Снять ткань с примитива · Ctrl+Z — вернуть"),
            Self::SceneDestruction(_) => Some("Разрушение примитива · открыть настройки"),
            Self::SceneDestructionVisible(_) => Some("Показать осколки или исходную форму · настройки сохраняются"),
            Self::SceneDestructionRemove(_) => Some("Снять разрушение и вернуть исходный примитив · Ctrl+Z — вернуть"),
            Self::Destruction(DestructionAction::Attach) => Some("Добавить разрушение примитиву и создать осколки"),
            Self::Destruction(DestructionAction::Remove) => Some("Удалить компонент разрушения · исходник и материал сохраняются"),
            Self::Destruction(DestructionAction::Material) => Some("Материал исходного примитива и осколков"),
            Self::SceneFluid(_) => Some("Жидкость примитива · открыть настройки"),
            Self::SceneFluidVisible(_) => Some("Показать / скрыть жидкость, сохранив её настройки"),
            Self::SceneFluidRemove(_) => Some("Снять жидкость с примитива · Ctrl+Z — вернуть"),
            Self::MaterialLibraryRemove(_) => Some("Убрать из библиотеки; назначения на объектах сохраняются"),
            Self::MaterialLibraryToggle => Some("Добавить материал в библиотеку или убрать из неё"),
            Self::MaterialLayerVisible(..) => Some("Показать / скрыть этот слой материала"),
            Self::MaterialLayerMove(_) => Some("Изменить порядок наложения слоёв"),
            Self::MaterialSelect(_) => Some("Выбрать слой · двойной щелчок — переименовать · перетаскивание — назначить"),
            Self::SceneMaterialVisible(_) => Some("Включить материал или показать дефолтную поверхность · Ctrl+Z — отменить"),
            Self::SceneMaterialDetach(..) => Some("Снять этот материал с объекта; ресурс материала сохранится в проекте · Ctrl+Z — отменить"),
            Self::SceneMaterial(..) => Some("Материал · потяните для перестановки или назначения другому объекту"),
            Self::SceneToggleNode(_) => Some("Показать или скрыть объект"),
            Self::SceneShadowOnly(_) => Some("Скрыть поверхность меша, сохранив его тени · объект остаётся доступен в панели сцены"),
            Self::ToggleVisible => Some("Показать или скрыть выбранный объект"),
            Self::SetAxis(0) => Some("Выделить ось X на гизмо · X · Для изменения потяните её ручку"),
            Self::SetAxis(1) => Some("Выделить ось Y на гизмо · Y · Для изменения потяните её ручку"),
            Self::SetAxis(2) => Some("Выделить ось Z на гизмо · Z · Для изменения потяните её ручку"),
            Self::Help => Some("Открыть руководство и горячие клавиши · F1"),
            Self::Export => Some("Сохранить текущий кадр в PNG"),
            Self::ExportSequence => Some("Сохранить последовательность кадров в PNG"),
            Self::DropdownOpen(_) => Some("Открыть список · ↑/↓ — выбрать · Enter — применить · Esc — отменить · F4"),
            _ if self.is_field() => Some("Введите значение здесь · Enter — применить · Esc — отменить"),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Hit {
    pub rect: [f32; 4],
    pub action: Action,
}
struct PathHit {
    action: Action,
    segments: Vec<([f32; 2], [f32; 2])>,
    radius: f32,
}
#[derive(Default)]
pub struct Ui {
    pub language: crate::localization::Language,
    pub rects: Vec<UiRect>,
    pub texts: Vec<UiText>,
    pub hits: Vec<Hit>,
    pub scale: f32,
    pub theme: Theme,
    pub accent: Accent,
    cursor: Option<[f32; 2]>,
    slider_range: Option<(Action, SliderRange)>,
    pub number_values: Vec<(Action, f64)>,
    path_hits: Vec<PathHit>,
}
impl Ui {
    pub fn palette(&self) -> Palette {
        self.accent.palette(self.theme)
    }
    fn slider_fill(&self) -> [f32; 4] {
        let p = self.palette();
        let amount = if self.theme == Theme::Dark { 0.40 } else { 0.32 };
        std::array::from_fn(|i| p.elevated[i] * (1. - amount) + p.accent[i] * amount)
    }
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }
    pub fn with_theme(scale: f32, theme: Theme, cursor: [f32; 2]) -> Self {
        Self {
            scale,
            theme,
            cursor: Some(cursor),
            ..Default::default()
        }
    }
    /// Cursor coordinates use the same logical space as controls being emitted.
    pub fn set_cursor(&mut self, cursor: [f32; 2]) {
        self.cursor = Some(cursor);
    }
    /// Freeze the visible mapping while a drag changes the value from which a
    /// field would otherwise derive its convenient slider range.
    pub fn set_slider_range(&mut self, action: Action, range: SliderRange) {
        self.slider_range = Some((action, range));
    }
    pub fn image(&mut self,rect:[f32;4],uv:[f32;4]) {
        self.rect(rect,[1.;4]);
        if let Some(paint)=self.rects.last_mut() {paint.image_uv=Some(uv);}
    }
    pub fn rect(&mut self, r: [f32; 4], c: [f32; 4]) {
        self.rects.push(UiRect {
            image_uv: None,
            triangle: None,
            segment: None,
            depth: None,
            rect: r.map(|v| v * self.scale),
            color: c,
        });
    }
    /// Small rounded surfaces reuse the existing quad renderer. Corner coverage
    /// is sampled in physical-pixel-sized tiles; rectangles never overlap within
    /// a fill, so translucent shadows retain a uniform opacity.
    pub fn rounded_rect(&mut self, r: [f32; 4], color: [f32; 4], radius: f32) {
        if !r.iter().all(|v| v.is_finite())
            || !self.scale.is_finite()
            || self.scale <= 0.
            || r[2] <= 0.
            || r[3] <= 0.
            || !radius.is_finite()
        {
            return;
        }
        let radius = radius.max(0.).min(r[2] / 2.).min(r[3] / 2.);
        if radius == 0. {
            self.rect(r, color);
            return;
        }
        let [x, y, w, h] = r;
        if w > radius * 2. {
            self.rect([x + radius, y, w - radius * 2., h], color);
        }
        if h > radius * 2. {
            self.rect([x, y + radius, radius, h - radius * 2.], color);
            self.rect([x + w - radius, y + radius, radius, h - radius * 2.], color);
        }
        self.rounded_corners(r, color, radius, None);
    }
    fn rounded_corners(&mut self, r: [f32; 4], color: [f32; 4], radius: f32, stroke: Option<f32>) {
        let [x, y, w, h] = r;
        let mask = corner_cache::mask(radius, self.scale, stroke);
        let tile = mask.tile;
        for sample in &mask.tiles {
            let [dx, dy] = sample.offset;
            let mut c = color;
            c[3] *= sample.coverage;
            for tx in [x + dx, x + w - dx - tile] {
                for ty in [y + dy, y + h - dy - tile] {
                    self.rect([tx, ty, tile, tile], c);
                }
            }
        }
    }
    pub fn surface(&mut self, r: [f32; 4], color: [f32; 4]) {
        self.rounded_rect(r, color, SURFACE_RADIUS);
    }
    pub fn outlined_surface(&mut self, r: [f32; 4], fill: [f32; 4], outline: [f32; 4]) {
        self.surface(r, outline);
        self.rounded_rect(
            [r[0] + 1., r[1] + 1., r[2] - 2., r[3] - 2.],
            fill,
            SURFACE_RADIUS - 1.,
        );
    }
    /// The outline must not become an opaque backing under a translucent fill.
    pub fn floating_surface(&mut self, r: [f32; 4], alpha: f32, accented: bool) {
        let p = self.palette();
        let mut fill = p.panel;
        if accented {
            for (channel, accent) in fill[..3].iter_mut().zip(p.accent) {
                *channel = *channel * 0.94 + accent * 0.06;
            }
        }
        fill[3] = alpha;
        self.surface(r, fill);
        let mut border = if accented { p.accent } else { p.border };
        border[3] = if accented { 0.6 } else { alpha };
        self.surface_outline(r, border);
    }
    /// Overlay a complete focus ring without painting over icons or the fill.
    pub fn surface_outline(&mut self, r: [f32; 4], color: [f32; 4]) {
        if !r.iter().all(|value| value.is_finite())
            || r[2] <= 0.
            || r[3] <= 0.
            || !self.scale.is_finite()
            || self.scale <= 0.
        {
            return;
        }
        let [x, y, width, height] = r;
        let radius = SURFACE_RADIUS.min(width * 0.5).min(height * 0.5);
        let stroke = 1_f32.min(radius);
        if width > radius * 2. {
            self.rect([x + radius, y, width - radius * 2., stroke], color);
            self.rect(
                [x + radius, y + height - stroke, width - radius * 2., stroke],
                color,
            );
        }
        if height > radius * 2. {
            self.rect([x, y + radius, stroke, height - radius * 2.], color);
            self.rect(
                [x + width - stroke, y + radius, stroke, height - radius * 2.],
                color,
            );
        }
        self.rounded_corners(r, color, radius, Some(stroke));
    }
    pub fn label(&mut self, x: f32, y: f32, text: impl Into<String>, size: f32, color: [f32; 4]) {
        let text=text.into();
        self.raw_label(x,y,self.language.text(&text).into_owned(),size,color);
    }
    pub fn raw_label(&mut self, x:f32, y:f32, text:impl Into<String>, size:f32, color:[f32;4]) {
        self.texts.push(UiText {
            weight: 400,
            x: x * self.scale,
            y: y * self.scale,
            text: text.into(),
            size: size * self.scale,
            color,
        });
    }
    pub fn section_label(&mut self, r: [f32; 4], text: &str, size: f32) {
        let p = self.palette();
        self.fitted_label(r, text, size, p.accent);
        if let Some(label) = self.texts.last_mut() {
            label.weight = 600;
        }
    }
    pub fn fitted_label(&mut self, r: [f32; 4], text: &str, size: f32, color: [f32; 4]) {
        self.raw_label(r[0], r[1], fit_button_label(&self.language.text(text), r[2], size), size, color);
    }
    pub fn raw_fitted_label(&mut self,r:[f32;4],text:&str,size:f32,color:[f32;4]) {
        self.raw_label(r[0],r[1],fit_button_label(text,r[2],size),size,color);
    }
    /// Direct text entry uses the same hit list as all other controls, so panel
    /// clipping and modal layers also apply to a focused field.
    pub fn field(&mut self, r: [f32; 4], value: &str, action: Action) {
        let padding=action.field_padding();
        if !field_rect_valid(r) {
            return;
        }
        let p = self.palette();
        let hovered = self.cursor.is_some_and(|cursor| contains(r, cursor));
        self.surface_outline(r, if hovered { p.accent } else { p.border });
        if let Action::Author(AuthorAction::Coordinate(axis) | AuthorAction::RotateDegrees(axis) | AuthorAction::ScaleFactor(axis)) = action {
            if axis < 3 {
                self.rounded_rect([r[0] + 3., r[1] + 3., 3., r[3] - 6.], crate::gizmo::axis_color(self.theme, axis as usize), 1.);
            }
        }
        self.raw_label(
            r[0] + padding,
            r[1] + (r[3] - FIELD_LINE_HEIGHT) * 0.5,
            fit_button_label(
                value,
                r[2] - padding - FIELD_PADDING - action.text_reserve(),
                FIELD_FONT_SIZE,
            ),
            FIELD_FONT_SIZE,
            p.text,
        );
        let clear = action.search_clear();
        self.hits.push(Hit { rect: r, action });
        if let Some(clear) = clear {
            self.search_clear_button(r, clear);
        }
    }
    pub fn disabled_number_field(&mut self,r:[f32;4],label:&str,value:f64,action:Action,range:SliderRange) {
        let (rects,texts,hits,values)=(self.rects.len(),self.texts.len(),self.hits.len(),self.number_values.len());
        self.number_field(r,label,value,action,range);
        let disabled=self.palette().disabled;
        for text in &mut self.texts[texts..] {text.color=disabled;}
        for rect in &mut self.rects[rects..] {rect.color[3]*=0.4;}
        self.hits.truncate(hits);self.number_values.truncate(values);
    }

    fn search_clear_button(&mut self, r: [f32; 4], action: Action) {
        self.quiet_button(
            [r[0] + r[2] - 28., r[1] + 2., 26., r[3] - 4.],
            "×",
            action,
            false,
        );
    }

    /// Keep exact text entry and the anchored color panel separately reachable.
    pub fn color_field(&mut self, r: [f32; 4], value: &str, action: Action, srgb: [f32; 3]) {
        if !field_rect_valid(r) {
            return;
        }
        let p = self.palette();
        let height = r[3].min(26.); let top = r[1] + (r[3] - height) * 0.5;
        let field = [r[0], top, (r[2]-28.).max(16.), height];
        let rgb = srgb.map(|v| if v.is_finite() { v.clamp(0., 1.) } else { 0. });
        self.field(field, "", action.clone());
        self.rounded_rect(field, [rgb[0],rgb[1],rgb[2],1.], 4.);
        if self.cursor.is_some_and(|cursor| contains(field, cursor)) { self.surface_outline(field, p.accent); }
        let linear = rgb.map(|v| if v <= 0.04045 {v/12.92} else {((v+0.055)/1.055).powf(2.4)});
        let luminance = linear[0]*0.2126 + linear[1]*0.7152 + linear[2]*0.0722;
        let ink = if luminance > 0.179 { [0.,0.,0.,1.] } else { [1.,1.,1.,1.] };
        self.fitted_label([field[0]+8.,top+(height-16.)*0.5,field[2]-16.,16.], value, 11., ink);
        self.hits.push(Hit { rect: field, action: Action::OpenColorPicker(Box::new(action.clone())) });
        let reset = match action {
            Action::BoneDisplay(BoneDisplayAction::BodyColor) => Action::BoneDisplay(BoneDisplayAction::ResetBodyColor),
            Action::BoneDisplay(BoneDisplayAction::JointColor) => Action::BoneDisplay(BoneDisplayAction::ResetJointColor),
            _ => Action::ResetColor(Box::new(action)),
        };
        self.hits.push(Hit { rect: [r[0]+r[2]-24.,top,24.,height], action: reset });
        Icon::Reset.draw(self,[r[0]+r[2]-20.,top+(height-16.)*0.5,16.,16.],p.muted);
    }

    /// One filled bar contains its label and right-aligned value. The underlying
    /// field hit retains keyboard focus and clipping; the top hit owns dragging
    /// and double-click entry across the entire visible control.
    pub fn number_field(
        &mut self,
        r: [f32; 4],
        label: &str,
        value: f64,
        action: Action,
        range: SliderRange,
    ) {
        if !field_rect_valid(r) {
            return;
        }
        let range = self
            .slider_range
            .as_ref()
            .filter(|(field, _)| field == &action)
            .map(|(_, range)| *range)
            .unwrap_or(range);
        let p = self.palette();
        let active = self
            .slider_range
            .as_ref()
            .is_some_and(|(field, _)| field == &action);
        let height = r[3].min(if matches!(action, Action::Author(AuthorAction::LiveField(_))) { 30. } else { 26. });
        let top = r[1] + (r[3] - height) * 0.5;
        let axis = label.split(|c: char| !c.is_alphanumeric()).find_map(|word| {
            match word { "X" => Some(0), "Y" => Some(1), "Z" => Some(2), _ => None }
        });
        let compact = r[2] < 120.;
        let reset_width = if compact { 20. } else { 24. };
        let gap = if compact { 2. } else { 4. };
        let field = [r[0], top, (r[2] - reset_width - gap).max(16.), height];
        let hovered = self.cursor.is_some_and(|cursor| contains(field, cursor));
        self.rounded_rect(
            field,
            if active || hovered { p.accent } else { p.border },
            3.,
        );
        self.rounded_rect(
            [field[0] + 1., field[1] + 1., field[2] - 2., field[3] - 2.],
            p.panel,
            2.,
        );
        let track = [field[0] + 2., field[1] + 2., field[2] - 4., field[3] - 4.];
        self.rounded_rect(track, p.elevated, 1.);
        let x = track[0] + track[2] * range.ratio(value);
        let signed = range.min < 0. && range.max > 0.;
        let origin = if signed {
            track[0] + track[2] * range.ratio(0.)
        } else {
            track[0]
        };
        let fill = self.slider_fill();
        let fill_rect = [x.min(origin), track[1], (x - origin).abs(), track[3]];
        let transparency = matches!(action, Action::SurfaceMaterialNumber(2) | Action::Studio(StudioAction::Field(0))
            | Action::ColorPicker(crate::color_picker::ColorControl::Alpha));
        if transparency {
            let cell = 5.;
            for row in 0..(track[3] / cell).ceil() as usize {
                for column in 0..(fill_rect[2] / cell).ceil() as usize {
                    let color = if (row + column) % 2 == 0 { p.panel } else { p.muted };
                    self.rect([fill_rect[0] + column as f32 * cell, track[1] + row as f32 * cell,
                        (fill_rect[2] - column as f32 * cell).min(cell),
                        (track[3] - row as f32 * cell).min(cell)], color);
                }
            }
            self.rounded_rect(fill_rect, [fill[0], fill[1], fill[2], 0.66], 1.);
        } else { self.rounded_rect(fill_rect, fill, 1.); }
        if signed {
            self.rect([origin - 0.5, track[1] + track[3] - 4., 1., 4.], p.muted);
        }
        if let Some(axis) = axis {
            self.rounded_rect([field[0] + 3., top + 3., 3., height - 6.], crate::gizmo::axis_color(self.theme, axis), 1.);
        }
        let available = (field[2] - FIELD_PADDING * 2.).max(0.);
        let (label, suffix) = if let Some(label) = label.strip_suffix(", %") {
            (label, "%")
        } else if let Some(label) = label.strip_suffix(", °") {
            (label, "°")
        } else if let Some(label) = label.strip_suffix(", м") {
            (label, " м")
        } else {
            (label, "")
        };
        let suffix_width: f32 = suffix
            .chars()
            .map(|c| character_advance(c, FIELD_FONT_SIZE))
            .sum();
        let value_text = fit_button_label(
            &format!(
                "{}{}",
                number_caption_for_width(value, available - suffix_width),
                suffix
            ),
            available,
            FIELD_FONT_SIZE,
        );
        let value_width: f32 = value_text
            .chars()
            .map(|c| character_advance(c, FIELD_FONT_SIZE))
            .sum();
        let text_y = field[1] + (field[3] - FIELD_LINE_HEIGHT) * 0.5;
        let label_width = available - value_width - 12.;
        if label_width >= 14. {
            self.fitted_label(
                [field[0] + FIELD_PADDING, text_y, label_width, FIELD_LINE_HEIGHT],
                label,
                FIELD_FONT_SIZE,
                p.text,
            );
        }
        self.label(
            field[0] + field[2] - FIELD_PADDING - value_width,
            text_y,
            value_text,
            FIELD_FONT_SIZE,
            p.text,
        );
        self.hits.push(Hit {
            rect: field,
            action: action.clone(),
        });
        let reset = [r[0] + r[2] - reset_width, top, reset_width, height];
        self.number_values.push((action.clone(), value));
        let reset_hover = self.cursor.is_some_and(|cursor| contains(reset, cursor));
        // The glyph alone changes color on hover; no background or border.
        Icon::Reset.draw(self, [reset[0] + (reset_width - 16.) * 0.5, reset[1] + (height - 16.) * 0.5, 16., 16.],
            if reset_hover { p.accent } else { p.muted });
        self.hits.push(Hit { rect: reset, action: Action::ResetNumber(Box::new(action.clone())) });
        self.hits.push(Hit {
            rect: field,
            action: Action::Slide(Box::new(action), range, value),
        });
    }

    /// Paint the current draft over its final, possibly clipped hit rectangle.
    /// Text is emitted only in whole glyph estimates that fit the inner bounds;
    /// unlike rectangles, the renderer cannot crop individual text runs.
    pub fn edit_field(&mut self, r: [f32; 4], edit: &mut TextEdit, invalid: bool) {
        if !field_rect_valid(r)
            || r[3] < 20.
            || r[2] < FIELD_PADDING * 2. + 2.
            || !self.scale.is_finite()
            || self.scale <= 0.
        {
            return;
        }
        let physical = r.map(|v| v * self.scale);
        // Keep the field's axis mark inside the focus outline during exact entry.
        let axis = (0..3).find(|&axis| self.rects.iter().any(|paint| {
            paint.color == crate::gizmo::axis_color(self.theme, axis)
                && paint.rect[0] >= physical[0] + 2. * self.scale
                && paint.rect[0] + paint.rect[2] <= physical[0] + 6.1 * self.scale
                && paint.rect[1] >= physical[1]
                && paint.rect[1] + paint.rect[3] <= physical[1] + physical[3]
                && paint.rect[3] > physical[3] * 0.5
        }));
        self.texts.retain(|text| {
            let width: f32 = text
                .text
                .chars()
                .map(|c| character_advance(c, text.size))
                .sum();
            text.x >= physical[0] + physical[2]
                || text.x + width <= physical[0]
                || text.y >= physical[1] + physical[3]
                || text.y + text.size * (FIELD_LINE_HEIGHT / FIELD_FONT_SIZE) <= physical[1]
        });
        let p = self.palette();
        let focus = if invalid { p.danger } else { p.accent };
        if self.hits.iter().any(|hit| {
            matches!(&hit.action, Action::Slide(action, _, _) if **action == edit.purpose)
        }) {
            self.rounded_rect(r, p.elevated, SURFACE_RADIUS);
        }
        self.surface_outline(r, focus);
        if let Some(axis) = axis {
            self.rounded_rect([r[0] + 3., r[1] + 3., 3., r[3] - 6.], crate::gizmo::axis_color(self.theme, axis), 1.);
        }
        let layout = edit.layout(r);
        let left = layout.x_at(layout.selection.start);
        let right = layout.x_at(layout.selection.end);
        if right > left {
            self.rect(
                [left, layout.origin[1], right - left, FIELD_LINE_HEIGHT],
                p.selection,
            );
        }
        let text: String = layout.chars[layout.start..layout.end].iter().collect();
        if !text.is_empty() {
            self.raw_label(
                layout.origin[0],
                layout.origin[1],
                text,
                FIELD_FONT_SIZE,
                p.text,
            );
        }
        let preedit_left = layout.x_at(layout.preedit.start);
        let preedit_right = layout.x_at(layout.preedit.end);
        if preedit_right > preedit_left {
            self.rect(
                [
                    preedit_left,
                    layout.origin[1],
                    preedit_right - preedit_left,
                    FIELD_LINE_HEIGHT,
                ],
                p.hover,
            );
        }
        self.rect(
            [
                layout.x_at(layout.caret),
                layout.origin[1],
                1.,
                FIELD_LINE_HEIGHT,
            ],
            focus,
        );
        if let Some(clear) = edit.purpose.search_clear() {
            self.search_clear_button(r, clear);
        }
    }

    /// Move a contiguous layout section before the inspector applies clipping.
    pub fn translate_section(&mut self, start: (usize, usize, usize), y: f32) {
        for rect in &mut self.rects[start.0..] {
            rect.rect[1] += y * self.scale;
            if let Some(points) = &mut rect.triangle {
                for point in points {
                    point[1] += y * self.scale;
                }
            }
            if let Some(line) = &mut rect.segment {
                line[1] += y * self.scale;
                line[3] += y * self.scale;
            }
        }
        for text in &mut self.texts[start.1..] {
            text.y += y * self.scale;
        }
        for hit in &mut self.hits[start.2..] {
            hit.rect[1] += y;
        }
    }
    pub fn button(&mut self, r: [f32; 4], text: &str, action: Action, active: bool) {
        self.button_visual(r, text, active, true);
        self.hits.push(Hit { rect: r, action });
    }
    /// Selected tools use a complete rounded outline and a persistent fill.
    pub fn menu_row(&mut self, r: [f32; 4], action: Action, highlighted: bool) {
        let p = self.palette();
        if highlighted || self.cursor.is_some_and(|c| contains(r, c)) {
            self.surface(r, p.hover);
        }
        self.hits.push(Hit { rect: r, action });
    }
    pub fn vertical_scrollbar(&mut self, r: [f32; 4], ratio: f32, fraction: f32, action: Action) {
        let p = self.palette();
        let h = (r[3] * fraction.clamp(0., 1.)).max(28.).min(r[3]);
        let y = r[1] + (r[3] - h) * ratio.clamp(0., 1.);
        self.surface([r[0] + 3., r[1], 6., r[3]], p.elevated);
        self.surface(
            [r[0] + 2., y, 8., h],
            if self.cursor.is_some_and(|c| contains(r, c)) {
                p.accent
            } else {
                p.muted
            },
        );
        self.hits.push(Hit { rect: r, action });
    }
    pub fn outline_button(&mut self, r: [f32; 4], text: &str, action: Action, enabled: bool) {
        let p = self.palette();
        self.surface_outline(r, p.border);
        if enabled {
            self.quiet_button(r, text, action, false);
        } else {
            self.fitted_label(
                [r[0] + 7., r[1] + (r[3] - 17.) * 0.5, r[2] - 14., 17.],
                text,
                13.,
                p.muted,
            );
        }
    }
    pub fn quiet_button(&mut self, r: [f32; 4], text: &str, action: Action, active: bool) {
        let p = self.palette();
        if active {
            self.outlined_surface(r, p.selection, p.accent);
        } else if self.cursor.is_some_and(|cursor| contains(r, cursor)) {
            self.surface(r, p.hover);
        }
        let padding = if r[2] <= 36. { 4. } else { 8. };
        let caption = fit_button_label(&self.language.text(text), r[2] - padding * 2., 13.);
        let width: f32 = caption.chars().map(|c| character_advance(c, 13.)).sum();
        self.label(
            r[0] + if r[2] <= 36. {
                (r[2] - width) * 0.5
            } else {
                padding
            },
            r[1] + (r[3] - 17.) / 2.,
            caption,
            13.,
            if active { p.accent } else { p.text },
        );
        self.hits.push(Hit { rect: r, action });
    }
    /// Tree disclosure is an unboxed vector chevron, with a generous hit target.
    /// Search-forced expansion remains visible but does not offer a dead click.
    pub fn disclosure_button(&mut self, r: [f32; 4], action: Option<Action>, expanded: bool) {
        let p = self.palette();
        let hovered = action.is_some() && self.cursor.is_some_and(|c| contains(r, c));
        let color = if hovered { p.accent } else { p.muted };
        let icon = if expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        };
        icon.draw(
            self,
            [
                r[0] + (r[2] - 16.) * 0.5,
                r[1] + (r[3] - 16.) * 0.5,
                16.,
                16.,
            ],
            color,
        );
        if let Some(action) = action {
            self.hits.push(Hit { rect: r, action });
        }
    }
    /// Disabled controls do not participate in pointer hit testing.
    pub fn disabled_button(&mut self, r: [f32; 4], text: &str) {
        self.button_visual(r, text, false, false);
    }

    /// Persistent Boolean state has a visible check mark, independent of hue.
    /// The caption and square share one action and one full-row hit target.
    pub fn checkbox(&mut self, r: [f32; 4], text: &str, action: Action, checked: bool) {
        if !self.decorated_control_valid(r) {
            return;
        }
        self.checkbox_visual(r, text, checked, true);
        self.hits.push(Hit { rect: r, action });
    }

    pub fn disabled_checkbox(&mut self, r: [f32; 4], text: &str, checked: bool) {
        if self.decorated_control_valid(r) {
            self.checkbox_visual(r, text, checked, false);
        }
    }

    fn checkbox_visual(&mut self, r: [f32; 4], text: &str, checked: bool, enabled: bool) {
        let p = self.palette();
        let hovered = enabled && self.cursor.is_some_and(|cursor| contains(r, cursor));
        if hovered {
            self.surface(r, p.hover);
        }
        let compact = r[2] < 160.;
        let padding = if compact { 4. } else { 6. };
        let right_padding = if compact { 3. } else { 6. };
        let size = (if compact { 14_f32 } else { 16_f32 }).min(r[3] - 4.);
        let gap = if compact { 4. } else { 6. };
        let square = [r[0] + padding, r[1] + (r[3] - size) * 0.5, size, size];
        let outline = if !enabled {
            p.border
        } else if hovered || checked {
            p.accent
        } else {
            p.muted
        };
        let fill = if checked {
            if enabled {
                p.accent
            } else {
                p.border
            }
        } else {
            p.elevated
        };
        self.outlined_surface(square, fill, outline);
        if checked {
            let mark = if !enabled {
                p.muted
            } else if self.theme == Theme::Dark {
                p.bg
            } else {
                p.panel
            };
            Icon::Check.draw(
                self,
                [square[0] + 1., square[1] + 1., size - 2., size - 2.],
                mark,
            );
        }
        let width = (r[2] - padding - right_padding - size - gap).max(0.);
        if width <= 0. {
            return;
        }
        let full_width: f32 = text.chars().map(|c| character_advance(c, 12.)).sum();
        let smaller_width: f32 = text.chars().map(|c| character_advance(c, 11.)).sum();
        let font_size = if full_width > width && smaller_width <= width {
            11.
        } else {
            12.
        };
        self.fitted_label(
            [
                square[0] + size + gap,
                r[1] + (r[3] - font_size - 4.) * 0.5,
                width,
                font_size + 4.,
            ],
            text,
            font_size,
            if enabled { p.text } else { p.disabled },
        );
    }

    /// A command icon supplements its caption; compact transport controls can
    /// use an empty caption while keeping their Action's existing tooltip.
    pub fn icon_button(
        &mut self,
        r: [f32; 4],
        text: &str,
        action: Action,
        active: bool,
        icon: Icon,
    ) {
        if !self.decorated_control_valid(r) {
            return;
        }
        self.button_surface(r, active, true);
        self.icon_caption(r, text, icon, active, true);
        self.hits.push(Hit { rect: r, action });
    }

    pub fn quiet_icon_button(
        &mut self,
        r: [f32; 4],
        text: &str,
        action: Action,
        active: bool,
        icon: Icon,
    ) {
        if !self.decorated_control_valid(r) {
            return;
        }
        let p = self.palette();
        if active {
            self.outlined_surface(r, p.selection, p.accent);
        } else if self.cursor.is_some_and(|cursor| contains(r, cursor)) {
            self.surface(r, p.hover);
        }
        self.icon_caption(r, text, icon, active, true);
        self.hits.push(Hit { rect: r, action });
    }

    /// Larger vector glyphs for the modeling palette, with shared theme/input states.
    pub fn modeling_icon_button(&mut self, rect: [f32;4], action: Action, icon: Icon, active: bool, enabled: bool) {
        if !self.decorated_control_valid(rect) { return; }
        let p = self.palette();
        if enabled && active { self.outlined_surface(rect,p.selection,p.accent); }
        else if enabled && self.cursor.is_some_and(|cursor| contains(rect,cursor)) { self.surface(rect,p.hover); }
        let size = 28_f32.min(rect[2]-8.).min(rect[3]-8.);
        icon.draw(self,[rect[0]+(rect[2]-size)*0.5,rect[1]+(rect[3]-size)*0.5,size,size],
            if !enabled {p.disabled}else if active {p.accent}else{p.text});
        if enabled { self.hits.push(Hit {rect,action}); }
    }

    /// Visibility controls remain unframed even when enabled.
    pub fn scene_close_button(&mut self, rect: [f32; 4], action: Action) {
        let p = self.palette();
        if self.cursor.is_some_and(|cursor| contains(rect, cursor)) {
            self.surface(rect, p.hover);
        }
        crate::icons::Icon::Close.draw(
            self,
            [
                rect[0] + (rect[2] - 14.) * 0.5,
                rect[1] + (rect[3] - 14.) * 0.5,
                14.,
                14.,
            ],
            p.muted,
        );
        self.hits.push(Hit { rect, action });
    }
    pub fn eye_button(&mut self, r: [f32; 4], action: Action, visible: bool) {
        if !self.decorated_control_valid(r) {
            return;
        }
        let p = self.palette();
        if self.cursor.is_some_and(|cursor| contains(r, cursor)) {
            self.surface(r, p.hover);
        }
        let icon = [
            r[0] + (r[2] - 16.) * 0.5,
            r[1] + (r[3] - 16.) * 0.5,
            16.,
            16.,
        ];
        Icon::Eye.draw(self, icon, if visible { p.text } else { p.muted });
        if !visible {
            self.smooth_segment(
                [icon[0] + 2., icon[1] + 14.],
                [icon[0] + 14., icon[1] + 2.],
                1.5,
                p.muted,
            );
        }
        self.hits.push(Hit { rect: r, action });
    }

    pub fn disabled_icon_button(&mut self, r: [f32; 4], text: &str, icon: Icon) {
        if self.decorated_control_valid(r) {
            self.button_surface(r, false, false);
            self.icon_caption(r, text, icon, false, false);
        }
    }

    fn decorated_control_valid(&self, r: [f32; 4]) -> bool {
        field_rect_valid(r)
            && r[2] >= 18.
            && r[3] >= 18.
            && self.scale.is_finite()
            && self.scale > 0.
    }

    fn icon_caption(&mut self, r: [f32; 4], text: &str, icon: Icon, active: bool, enabled: bool) {
        let p = self.palette();
        let color = if !enabled {
            p.disabled
        } else if active {
            p.accent
        } else {
            p.text
        };
        let compact = r[2] < 110.;
        let size = if compact { 14_f32 } else { 16_f32 }.min(r[3] - 4.);
        if text.is_empty() {
            let size = 16_f32.min(r[2] - 4.).min(r[3] - 4.);
            icon.draw(
                self,
                [
                    r[0] + (r[2] - size) * 0.5,
                    r[1] + (r[3] - size) * 0.5,
                    size,
                    size,
                ],
                color,
            );
            return;
        }
        let padding = if compact { 4. } else { 8. };
        let gap = if compact { 4. } else { 6. };
        let right_padding = if compact { 3. } else { 8. };
        icon.draw(
            self,
            [r[0] + padding, r[1] + (r[3] - size) * 0.5, size, size],
            color,
        );
        let width = (r[2] - padding - size - gap - right_padding).max(0.);
        if width <= 0. {
            return;
        }
        let font_size = if compact { 12. } else { 13. };
        let full_width: f32 = text.chars().map(|c| character_advance(c, font_size)).sum();
        let smaller_width: f32 = text.chars().map(|c| character_advance(c, 11.)).sum();
        // Preserve short Russian captions on dense transport/command rows.
        // Long labels still use a visible ellipsis instead of shrinking further.
        let font_size = if full_width > width && smaller_width <= width {
            11.
        } else {
            font_size
        };
        self.fitted_label(
            [
                r[0] + padding + size + gap,
                r[1] + (r[3] - font_size - 4.) * 0.5,
                width,
                font_size + 4.,
            ],
            text,
            font_size,
            color,
        );
    }

    fn button_surface(&mut self, r: [f32; 4], active: bool, enabled: bool) {
        let p = self.palette();
        let hovered = enabled && self.cursor.is_some_and(|cursor| contains(r, cursor));
        let fill = if active {
            p.selection
        } else if hovered {
            p.hover
        } else {
            p.elevated
        };
        // A one-pixel inset gives every control a clear boundary at any DPI.
        self.outlined_surface(r, fill, if active { p.accent } else { p.border });
    }
    fn button_visual(&mut self, r: [f32; 4], text: &str, active: bool, enabled: bool) {
        self.button_surface(r, active, enabled);
        let p = self.palette();
        let compact = r[2] <= 40.;
        let padding = if compact { 4. } else { 10. };
        let caption = fit_button_label(&self.language.text(text), (r[2] - padding * 2.).max(0.), 13.);
        let text_x = r[0]
            + if compact {
                ((r[2]
                    - caption
                        .chars()
                        .map(|c| character_advance(c, 13.))
                        .sum::<f32>())
                    / 2.)
                    .max(padding)
            } else {
                padding
            };
        self.label(
            text_x,
            r[1] + (r[3] - 17.) / 2.,
            caption,
            13.,
            if !enabled {
                p.disabled
            } else if active {
                p.accent
            } else {
                p.text
            },
        );
    }
    /// Antialiased filled geometry for viewport overlays. Coverage is computed
    /// in physical pixels, then emitted through the existing ordered quad path.
    pub fn triangle(&mut self, points: [[f32; 2]; 3], color: [f32; 4]) {
        self.convex_fill(&points, color);
    }

    pub fn convex_polygon(&mut self, points: &[[f32; 2]], color: [f32; 4]) {
        if let [a, b, c] = points {
            self.triangle([*a, *b, *c], color);
        } else {
            self.convex_fill(points, color);
        }
    }

    /// Butt caps let adjacent translucent ring segments meet without repeatedly
    /// compositing rounded caps into bright dots at every tessellation vertex.
    pub fn smooth_segment(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) {
        let delta = [b[0] - a[0], b[1] - a[1]];
        let length = delta[0].hypot(delta[1]);
        if !length.is_finite() || length < 1e-6 || !width.is_finite() || width <= 0. {
            return;
        }
        let normal = [
            -delta[1] / length * width * 0.5,
            delta[0] / length * width * 0.5,
        ];
        self.convex_fill(
            &[
                [a[0] + normal[0], a[1] + normal[1]],
                [b[0] + normal[0], b[1] + normal[1]],
                [b[0] - normal[0], b[1] - normal[1]],
                [a[0] - normal[0], a[1] - normal[1]],
            ],
            color,
        );
    }
    /// Dense edit meshes use one antialiased GPU instance per edge instead of
    /// rasterizing each line into hundreds of CPU scanline rectangles.
    pub fn mesh_segment(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) {
        if !a.iter().chain(&b).all(|v| v.is_finite())
            || !width.is_finite()
            || width <= 0.
            || (b[0] - a[0]).hypot(b[1] - a[1]) < 1e-5
        {
            return;
        }
        let a = a.map(|v| v * self.scale);
        let b = b.map(|v| v * self.scale);
        let stroke = width * self.scale;
        let pad = stroke * 0.5 + 1.;
        self.rects.push(UiRect {
            image_uv: None,
            triangle: None,
            rect: [
                a[0].min(b[0]) - pad,
                a[1].min(b[1]) - pad,
                (b[0] - a[0]).abs() + pad * 2.,
                (b[1] - a[1]).abs() + pad * 2.,
            ],
            color,
            segment: Some([a[0], a[1], b[0], b[1], stroke]),
            depth: None,
        });
    }

    pub fn mesh_triangle(&mut self, points: [[f32; 3]; 3], color: [f32; 4], depth_test: bool) {
        if points.iter().flatten().any(|v| !v.is_finite()) {
            return;
        }
        let points = points.map(|[x, y, z]| [x * self.scale, y * self.scale, z]);
        let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        if max_x - min_x < 0.00001 || max_y - min_y < 0.00001 {
            return;
        }
        self.rects.push(UiRect {
            image_uv: None,
            triangle: Some(points),
            rect: [min_x, min_y, max_x - min_x, max_y - min_y],
            color,
            segment: None,
            depth: depth_test.then_some([points[0][2], points[1][2]]),
        });
    }

    pub fn depth_mesh_segment(
        &mut self,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: [f32; 4],
        depth: Option<[f32; 2]>,
    ) {
        let start = self.rects.len();
        self.mesh_segment(a, b, width, color);
        if let Some(rect) = self.rects.get_mut(start) {
            rect.depth = depth;
        }
    }

    /// Composite a whole stroked path once. Adjacent antialiased segments share
    /// coverage instead of darkening or brightening their translucent joins.
    pub fn smooth_segments(
        &mut self,
        segments: &[([f32; 2], [f32; 2])],
        width: f32,
        color: [f32; 4],
    ) {
        if segments.len() == 1 {
            self.smooth_segment(segments[0].0, segments[0].1, width, color);
            return;
        }
        if color.iter().any(|v| !v.is_finite()) || color[3] <= 0. {
            return;
        }
        let mut coverage = Ui::new(self.scale);
        for &(a, b) in segments {
            coverage.smooth_segment(a, b, width, [1.; 4]);
        }
        if coverage.rects.is_empty() {
            return;
        }
        let bounds = coverage.rects.iter().fold(
            [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ],
            |b, r| {
                [
                    b[0].min(r.rect[0]),
                    b[1].min(r.rect[1]),
                    b[2].max(r.rect[0] + r.rect[2]),
                    b[3].max(r.rect[1] + r.rect[3]),
                ]
            },
        );
        let columns = (bounds[2] - bounds[0]) as usize;
        let rows = (bounds[3] - bounds[1]) as usize;
        let Some(area) = columns.checked_mul(rows).filter(|n| *n <= 4_194_304) else {
            // Keep abnormal, extremely large input bounded. Normal viewport
            // gizmos occupy a small, screen-sized mask even at high DPI.
            for r in coverage.rects {
                let mut tint = color;
                tint[3] *= r.color[3];
                self.rects.push(UiRect {
                    image_uv: None,
                    triangle: None,
                    segment: None,
                    depth: None,
                    rect: r.rect,
                    color: tint,
                });
            }
            return;
        };
        let mut mask = vec![0f32; area];
        for r in coverage.rects {
            let x = (r.rect[0] - bounds[0]) as usize;
            let y = (r.rect[1] - bounds[1]) as usize;
            let span = r.rect[2] as usize;
            for pixel in &mut mask[y * columns + x..y * columns + x + span] {
                *pixel = (*pixel + r.color[3]).min(1.);
            }
        }
        for y in 0..rows {
            let row = &mask[y * columns..(y + 1) * columns];
            let mut x = 0;
            while x < columns {
                let alpha = row[x];
                if alpha <= 1e-6 {
                    x += 1;
                    continue;
                }
                let start = x;
                x += 1;
                while x < columns && (row[x] - alpha).abs() < 1e-6 {
                    x += 1;
                }
                let mut tint = color;
                tint[3] *= alpha;
                self.rects.push(UiRect {
                    image_uv: None,
                    triangle: None,
                    segment: None,
                    depth: None,
                    rect: [
                        bounds[0] + start as f32,
                        bounds[1] + y as f32,
                        (x - start) as f32,
                        1.,
                    ],
                    color: tint,
                });
            }
        }
    }

    fn convex_fill(&mut self, points: &[[f32; 2]], color: [f32; 4]) {
        if !(3..=32).contains(&points.len())
            || !self.scale.is_finite()
            || self.scale <= 0.
            || color.iter().any(|v| !v.is_finite())
            || color[3] <= 0.
        {
            return;
        }
        let mut physical = [[0f32; 2]; 32];
        for (out, point) in physical.iter_mut().zip(points) {
            *out = [point[0] * self.scale, point[1] * self.scale];
        }
        let points = &physical[..points.len()];
        if points
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 16_777_216.)
        {
            return;
        }
        let top = points
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            .floor() as i32;
        let bottom = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil() as i32;
        // Callers clip their overlays to the viewport. This also bounds work for
        // malformed projections before they can reach the renderer.
        if bottom <= top || bottom - top > 16_384 {
            return;
        }
        // Precompute edge slopes once. Small clipped polygons need no heap
        // allocation or division inside the physical-pixel scanline loop.
        let mut edges = [[0f32; 4]; 32];
        let mut edge_count = 0;
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            let (low, high) = if a[1] <= b[1] { (a, b) } else { (b, a) };
            if high[1] - low[1] < 1e-7 {
                continue;
            }
            edges[edge_count] = [
                low[0],
                low[1],
                high[1],
                (high[0] - low[0]) / (high[1] - low[1]),
            ];
            edge_count += 1;
        }
        for y in top..bottom {
            let mut spans = [(0f32, 0f32); 4];
            let mut boundaries = [0i32; 16];
            let mut boundary_count = 0;
            for (sample, span) in spans.iter_mut().enumerate() {
                let scan_y = y as f32 + (sample as f32 + 0.5) / 4.;
                let mut left = f32::INFINITY;
                let mut right = f32::NEG_INFINITY;
                for &[x, low, high, slope] in &edges[..edge_count] {
                    if low <= scan_y && high > scan_y {
                        let x = x + (scan_y - low) * slope;
                        left = left.min(x);
                        right = right.max(x);
                    }
                }
                if left < right {
                    *span = (left, right);
                    boundaries[boundary_count..boundary_count + 4].copy_from_slice(&[
                        left.floor() as i32,
                        left.ceil() as i32,
                        right.floor() as i32,
                        right.ceil() as i32,
                    ]);
                    boundary_count += 4;
                }
            }
            let boundaries = &mut boundaries[..boundary_count];
            boundaries.sort_unstable();
            for interval in boundaries.windows(2) {
                if interval[0] == interval[1] {
                    continue;
                }
                let x = interval[0] as f32;
                let coverage: f32 = spans
                    .iter()
                    .map(|(left, right)| (right.min(x + 1.) - left.max(x)).clamp(0., 1.))
                    .sum::<f32>()
                    / 4.;
                if coverage <= 0. {
                    continue;
                }
                let mut covered = color;
                covered[3] *= coverage;
                self.rects.push(UiRect {
                    image_uv: None,
                    triangle: None,
                    segment: None,
                    depth: None,
                    rect: [x, y as f32, (interval[1] - interval[0]) as f32, 1.],
                    color: covered,
                });
            }
        }
    }
    pub fn round_icon_button(
        &mut self,
        r: [f32; 4],
        action: Option<Action>,
        active: bool,
        icon: Icon,
    ) {
        let p = self.palette();
        let hover = action.is_some() && self.cursor.is_some_and(|point| circle_contains(r, point));
        let radius = r[2].min(r[3]) * 0.5;
        self.rounded_rect(
            r,
            if active {
                p.accent
            } else if hover {
                p.hover
            } else {
                p.panel
            },
            radius,
        );
        if active {
            let inner = [r[0] + 1.5, r[1] + 1.5, r[2] - 3., r[3] - 3.];
            self.rounded_rect(inner, p.selection, radius - 1.5);
        }
        icon.draw(
            self,
            [r[0] + 9., r[1] + 9., r[2] - 18., r[3] - 18.],
            if active {
                p.accent
            } else if action.is_some() {
                p.text
            } else {
                p.muted
            },
        );
        if let Some(action) = action {
            self.hits.push(Hit { rect: r, action });
        }
    }
    /// A stroke is selectable near its visible line, never across its empty
    /// bounding box. Coordinates and tolerance are logical pixels.
    pub fn path_hit(&mut self, action: Action, segments: Vec<([f32; 2], [f32; 2])>, radius: f32) {
        if segments.is_empty() {
            return;
        }
        let mut lo = [f32::INFINITY; 2];
        let mut hi = [f32::NEG_INFINITY; 2];
        for &(a, b) in &segments {
            for i in 0..2 {
                lo[i] = lo[i].min(a[i]).min(b[i]);
                hi[i] = hi[i].max(a[i]).max(b[i]);
            }
        }
        self.hits.push(Hit {
            rect: [
                lo[0] - radius,
                lo[1] - radius,
                hi[0] - lo[0] + radius * 2.,
                hi[1] - lo[1] + radius * 2.,
            ],
            action: action.clone(),
        });
        self.path_hits.push(PathHit {
            action,
            segments,
            radius,
        });
    }
    #[cfg(test)]
    pub fn control_at(&self, p: [f32; 2]) -> Option<Action> {
        self.hit(p).map(|action| match action {
            Action::Slide(field, _, _) => *field,
            action => action,
        })
    }
    #[cfg(test)]
    pub fn action_point(&self, action: &Action) -> Option<[f32; 2]> {
        if let Some(path) = self.path_hits.iter().rev().find(|h| &h.action == action) {
            return path
                .segments
                .iter()
                .map(|(a, b)| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5])
                .find(|&p| {
                    self.hit(p).as_ref() == Some(action) || self.control_at(p).as_ref() == Some(action)
                });
        }
        self.hits
            .iter()
            .rev()
            .filter(|h| &h.action == action)
            .map(|h| [h.rect[0] + h.rect[2] * 0.5, h.rect[1] + h.rect[3] * 0.5])
            .find(|&p| {
                self.hit(p).as_ref() == Some(action) || self.control_at(p).as_ref() == Some(action)
            })
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<Action> {
        self.hits
            .iter()
            .rev()
            .find(|h| {
                if let Some(path) = self
                    .path_hits
                    .iter()
                    .rev()
                    .find(|path| path.action == h.action)
                {
                    return contains(h.rect, p)
                        && path.segments.iter().any(|&(a, b)| {
                            let (a, b, p) = (
                                glam::Vec2::from_array(a),
                                glam::Vec2::from_array(b),
                                glam::Vec2::from_array(p),
                            );
                            let d = b - a;
                            let t = ((p - a).dot(d) / d.length_squared().max(0.0001)).clamp(0., 1.);
                            p.distance(a + d * t) <= path.radius
                        });
                }
                if (matches!(h.action, Action::ViewAxis(_) | Action::ViewOrbit | Action::Rig(RigAction::Endpoint(_) | RigAction::Pole))
                    || (matches!(
                        h.action,
                        Action::CameraView | Action::Perspective | Action::ViewPan | Action::ViewZoom | Action::Studio(StudioAction::CameraLock)
                    ) && (h.rect[2] - h.rect[3]).abs() < 0.1))
                {
                    circle_contains(h.rect, p)
                } else {
                    contains(h.rect, p)
                }
            })
            .map(|h| h.action.clone())
    }
}
pub fn circle_contains(r: [f32; 4], p: [f32; 2]) -> bool {
    let x = (p[0] - r[0] - r[2] * 0.5) / (r[2] * 0.5).max(0.001);
    let y = (p[1] - r[1] - r[3] * 0.5) / (r[3] * 0.5).max(0.001);
    x * x + y * y <= 1.
}
/// Conservative character-width estimate for short control captions. The real
/// text shaper remains authoritative; this only prevents captions overflowing.
fn character_advance(c: char, font_size: f32) -> f32 {
    let em = match c {
        ' ' | '.' | ',' | ':' | ';' | '!' | 'i' | 'l' | 'I' | '|' => 0.35,
        'W' | 'M' | 'w' | 'm' | 'Ш' | 'Щ' | 'Ж' | 'Ю' | 'Ы' | 'Ф' | 'ш' | 'щ' | 'ж' | 'ю' | 'ы'
        | 'ф' => 0.95,
        c if c as u32 >= 0x2e80 => 1.0,
        _ => 0.65,
    };
    em * font_size
}
pub(crate) fn fit_button_label(text: &str, max_width: f32, font_size: f32) -> String {
    let advance = |c: char| character_advance(c, font_size);
    if text.chars().map(advance).sum::<f32>() <= max_width {
        return text.to_owned();
    }
    if max_width < font_size {
        return String::new();
    }
    let mut result = String::new();
    let mut width = font_size; // Reserve one ellipsis glyph.
    for c in text.chars() {
        width += advance(c);
        if width > max_width {
            break;
        }
        result.push(c);
    }
    result.push('…');
    result
}

pub fn contains(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] <= r[0] + r[2] && p[1] >= r[1] && p[1] <= r[1] + r[3]
}

const FIELD_FONT_SIZE: f32 = 13.;
const FIELD_LINE_HEIGHT: f32 = 17.;
const FIELD_PADDING: f32 = 4.;
const EDIT_HISTORY_LIMIT: usize = 128;

fn field_rect_valid(r: [f32; 4]) -> bool {
    r.iter().all(|v| v.is_finite()) && r[2] >= 8. && r[3] >= 18.
}

pub(crate) fn number_caption(value: f64) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if value != 0. && (value.abs() < 0.0001 || value.abs() >= 1_000_000.) {
        return format!("{value:.3e}");
    }
    let value = format!("{value:.4}");
    let value = value.trim_end_matches('0').trim_end_matches('.');
    if value == "-0" {
        "0".into()
    } else {
        value.into()
    }
}

/// Keep a useful magnitude visible in compact axis controls. The editing draft
/// retains its original precision; reducing a caption never changes model data.
fn number_caption_for_width(value: f64, width: f32) -> String {
    let fits = |text: &str| {
        text.chars()
            .map(|c| character_advance(c, FIELD_FONT_SIZE))
            .sum::<f32>()
            <= width
    };
    let full = number_caption(value);
    if fits(&full) || !value.is_finite() {
        return full;
    }
    for decimals in (0..=3).rev() {
        let candidate = format!("{value:.decimals$}");
        if candidate
            .parse::<f64>()
            .is_ok_and(|v| v != 0. || value == 0.)
            && fits(&candidate)
        {
            return candidate;
        }
    }
    for decimals in (0..=2).rev() {
        let candidate = format!("{value:.decimals$e}");
        if fits(&candidate) {
            return candidate;
        }
    }
    full
}

#[derive(Clone, Debug)]
struct EditSnapshot {
    value: String,
    cursor: usize,
    anchor: Option<usize>,
}

struct EditLayout {
    chars: Vec<char>,
    advances: Vec<f32>,
    start: usize,
    end: usize,
    origin: [f32; 2],
    width: f32,
    caret: usize,
    selection: std::ops::Range<usize>,
    preedit: std::ops::Range<usize>,
}
impl EditLayout {
    fn x_at(&self, index: usize) -> f32 {
        self.origin[0]
            + (self.advances[index.min(self.chars.len())] - self.advances[self.start])
                .clamp(0., self.width)
    }
}

/// Cursor and selection offsets are Unicode scalar positions, never UTF-8 bytes.
#[derive(Clone, Debug)]
pub struct TextEdit {
    pub value: String,
    pub cursor: usize,
    pub anchor: Option<usize>,
    pub preedit: String,
    pub purpose: Action,
    original: String,
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    scroll_start: usize,
    drag_anchor: Option<usize>,
}
impl TextEdit {
    pub fn new(value: String, purpose: Action) -> Self {
        let cursor = value.chars().count();
        Self {
            original: value.clone(),
            value,
            cursor,
            anchor: Some(0),
            preedit: String::new(),
            purpose,
            undo: Vec::new(),
            redo: Vec::new(),
            scroll_start: 0,
            drag_anchor: None,
        }
    }
    pub fn changed(&self) -> bool {
        self.value != self.original
    }
    pub fn selection(&self) -> std::ops::Range<usize> {
        let length = self.value.chars().count();
        let cursor = self.cursor.min(length);
        let a = self.anchor.unwrap_or(cursor).min(length);
        a.min(cursor)..a.max(cursor)
    }
    pub fn selected(&self) -> String {
        let r = self.selection();
        self.value.chars().skip(r.start).take(r.len()).collect()
    }
    pub fn insert(&mut self, text: &str) {
        self.replace(self.selection(), text);
    }
    pub fn backspace(&mut self) {
        let mut range = self.selection();
        if range.is_empty() && range.start > 0 {
            range.start -= 1;
        }
        self.replace(range, "");
    }
    pub fn delete(&mut self) {
        let mut range = self.selection();
        if range.is_empty() && range.end < self.value.chars().count() {
            range.end += 1;
        }
        self.replace(range, "");
    }
    pub fn move_cursor(&mut self, delta: isize, select: bool) {
        self.cursor = self.cursor.min(self.value.chars().count());
        if select {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = self
            .cursor
            .saturating_add_signed(delta)
            .min(self.value.chars().count());
        self.drag_anchor = None;
    }
    pub fn home(&mut self, select: bool) {
        self.move_cursor(-(self.cursor as isize), select)
    }
    pub fn end(&mut self, select: bool) {
        self.move_cursor(
            self.value.chars().count() as isize - self.cursor as isize,
            select,
        )
    }
    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.value.chars().count();
    }

    /// Both pointer presses and Shift/drag extensions share the scrolled
    /// rendering metrics. Repeated extensions keep the original anchor.
    pub fn click(&mut self, rect: [f32; 4], point: [f32; 2], select: bool) {
        if !field_rect_valid(rect) || point.iter().any(|v| !v.is_finite()) {
            return;
        }
        self.preedit.clear();
        let layout = self.layout(rect);
        let x = point[0] - layout.origin[0];
        let index = if x < 0. && layout.start > 0 {
            layout.start - 1
        } else if x > layout.width && layout.end < layout.chars.len() {
            (layout.end + 1).min(layout.chars.len())
        } else {
            let mut index = layout.start;
            while index < layout.end {
                let midpoint = (layout.advances[index] + layout.advances[index + 1]) * 0.5
                    - layout.advances[layout.start];
                if x < midpoint {
                    break;
                }
                index += 1;
            }
            index
        };
        if select {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = index;
        self.drag_anchor = Some(self.anchor.unwrap_or(index));
    }

    pub fn drag(&mut self, rect: [f32; 4], point: [f32; 2]) {
        self.anchor
            .get_or_insert(self.drag_anchor.unwrap_or(self.cursor));
        self.click(rect, point, true);
    }

    pub fn end_drag(&mut self) {
        self.drag_anchor = None;
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            value: self.value.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }

    fn restore(&mut self, snapshot: EditSnapshot) {
        self.value = snapshot.value;
        self.cursor = snapshot.cursor;
        self.anchor = snapshot.anchor;
        self.preedit.clear();
        self.scroll_start = 0;
        self.drag_anchor = None;
    }

    fn replace(&mut self, range: std::ops::Range<usize>, text: &str) {
        let text: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
        let mut chars: Vec<char> = self.value.chars().collect();
        chars.splice(range.clone(), text.iter().copied());
        let value: String = chars.into_iter().collect();
        if value != self.value {
            if self.undo.len() >= EDIT_HISTORY_LIMIT {
                self.undo.remove(0);
            }
            self.undo.push(self.snapshot());
            self.redo.clear();
        }
        self.value = value;
        self.cursor = range.start + text.len();
        self.anchor = None;
        self.preedit.clear();
        self.drag_anchor = None;
    }

    fn layout(&mut self, rect: [f32; 4]) -> EditLayout {
        let mut chars: Vec<char> = self
            .value
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        self.cursor = self.cursor.min(chars.len());
        self.anchor = self.anchor.map(|anchor| anchor.min(chars.len()));
        let selection = self.selection();
        let preedit: Vec<char> = self.preedit.chars().filter(|c| !c.is_control()).collect();
        let preedit_count = preedit.len();
        chars.splice(self.cursor..self.cursor, preedit);
        let mut advances = Vec::with_capacity(chars.len() + 1);
        advances.push(0.);
        for &c in &chars {
            advances.push(
                advances.last().copied().unwrap_or(0.) + character_advance(c, FIELD_FONT_SIZE),
            );
        }
        let width = (rect[2] - self.purpose.field_padding() - FIELD_PADDING - 1. - self.purpose.text_reserve()).max(0.);
        let caret = self.cursor + preedit_count;
        let mut start = self.scroll_start.min(caret);
        while start < caret && advances[caret] - advances[start] > width {
            start += 1;
        }
        // Reveal more context after shortening the draft without jumping the
        // viewport while the user moves within a still-visible portion.
        while start > 0 && advances[chars.len()] - advances[start - 1] <= width {
            start -= 1;
        }
        let mut end = start;
        while end < chars.len() && advances[end + 1] - advances[start] <= width {
            end += 1;
        }
        self.scroll_start = start;
        let selection_start = selection.start
            + if selection.start >= self.cursor {
                preedit_count
            } else {
                0
            };
        let selection_end = selection.end
            + if selection.end > self.cursor {
                preedit_count
            } else {
                0
            };
        EditLayout {
            chars,
            advances,
            start,
            end,
            origin: [
                rect[0] + self.purpose.field_padding(),
                rect[1] + (rect[3] - FIELD_LINE_HEIGHT) * 0.5,
            ],
            width,
            caret,
            selection: if selection.is_empty() {
                caret..caret
            } else {
                selection_start..selection_end
            },
            preedit: self.cursor..caret,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_mapping_quantizes_then_clamps_and_preserves_endpoints() {
        let rect = [10., 20., 200., 12.];
        let range = SliderRange::new(-2., 3., 0.25);
        assert_eq!(range.value_at(-50., rect), -2.);
        assert_eq!(range.value_at(300., rect), 3.);
        assert_eq!(range.value_at(113., rect), 0.5);
        assert_eq!(range.ratio(-100.), 0.);
        assert_eq!(range.ratio(100.), 1.);
        assert_eq!(range.ratio(0.5), 0.5);
        let narrow = SliderRange::new(0.002, 0.003, 0.01);
        assert_eq!(narrow.value_at(rect[0], rect), 0.002);
        assert_eq!(narrow.value_at(rect[0] + rect[2], rect), 0.003);
        for x in 0..240 {
            let value = narrow.value_at(x as f32, rect);
            assert!((narrow.min..=narrow.max).contains(&value));
        }
        let final_partial_step = SliderRange::new(0., 0.93, 0.1);
        assert_eq!(final_partial_step.value_at(210., rect), 0.93);
        assert!(final_partial_step.value_at(209., rect) <= 0.93);
    }

    #[test]
    fn slider_mapping_handles_invalid_and_extreme_finite_inputs() {
        let rect = [0., 0., 100., 12.];
        for range in [
            SliderRange::new(3., -2., f64::NAN),
            SliderRange::new(f64::NAN, f64::INFINITY, 0.),
            SliderRange {
                min: 3.,
                max: 2.,
                step: -1.,
            },
            SliderRange::new(-f64::MAX, f64::MAX, 0.1),
        ] {
            for x in [f32::NAN, f32::INFINITY, -100., 0., 50., 100., 300.] {
                assert!(range.value_at(x, rect).is_finite());
            }
            assert!(range.value_at(10., [0.; 4]).is_finite());
            for value in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY, -1., 0., 1.] {
                assert!((0. ..=1.).contains(&range.ratio(value)));
            }
        }
        let huge = SliderRange::new(-f64::MAX, f64::MAX, 0.);
        assert_eq!(huge.value_at(50., rect), 0.);
        assert_eq!(huge.ratio(0.), 0.5);
    }

    #[test]
    fn numeric_bars_share_entry_and_drag_bounds_at_both_dpi_scales() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 2.] {
                for (width, height) in [(240., 30.), (160., 30.), (120., 44.), (80., 44.)] {
                    let r = [10., 20., width, height];
                    let range = SliderRange::new(-10., 10., 0.1);
                    let mut ui = Ui::with_theme(scale, theme, [0.; 2]);
                    ui.number_field(r, "X", 1.25, Action::EditTransform(0), range);
                    assert_eq!(ui.hits.len(), 3);
                    let exact = &ui.hits[0];
                    let track = &ui.hits[2];
                    assert_eq!(
                        ui.hit([
                            exact.rect[0] + exact.rect[2] * 0.5,
                            exact.rect[1] + exact.rect[3] * 0.5
                        ]),
                        Some(Action::Slide(
                            Box::new(Action::EditTransform(0)),
                            range,
                            1.25
                        ))
                    );
                    assert_eq!(exact.rect, track.rect);
                    assert_eq!(
                        ui.hit([
                            track.rect[0] + track.rect[2] * 0.5,
                            track.rect[1] + track.rect[3] * 0.5
                        ]),
                        Some(Action::Slide(
                            Box::new(Action::EditTransform(0)),
                            range,
                            1.25
                        ))
                    );
                    assert_eq!(range.value_at(track.rect[0], track.rect), -10.);
                    assert_eq!(
                        range.value_at(track.rect[0] + track.rect[2], track.rect),
                        10.
                    );
                    assert!(ui.rects.iter().all(|q| q.rect[0] >= r[0] * scale
                        && q.rect[1] >= r[1] * scale
                        && q.rect[0] + q.rect[2] <= (r[0] + r[2]) * scale + 0.001
                        && q.rect[1] + q.rect[3] <= (r[1] + r[3]) * scale + 0.001));
                    assert!(ui.texts.iter().any(|text| text.text == "1.25"));
                }
            }
        }
    }

    #[test]
    fn percentage_bar_places_the_complete_value_at_the_right_edge() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 2.] {
                let mut ui = Ui::with_theme(scale, theme, [0.; 2]);
                let r = [10., 20., 200., 26.];
                ui.number_field(
                    r,
                    "Непрозрачность, %",
                    100.,
                    Action::ColorPicker(crate::color_picker::ColorControl::Alpha),
                    SliderRange::new(0., 100., 0.1),
                );
                let value = ui.texts.iter().find(|t| t.text == "100%").unwrap();
                let width: f32 = value.text.chars().map(|c| character_advance(c, value.size)).sum();
                let field = ui.hits[0].rect;
                assert!((value.x + width - (field[0] + field[2] - FIELD_PADDING) * scale).abs() < 0.001);
                assert!(value.x >= r[0] * scale);
                let mut edit = TextEdit::new("100".into(), Action::ColorPicker(crate::color_picker::ColorControl::Alpha));
                ui.edit_field(field, &mut edit, false);
                assert!(!ui.texts.iter().any(|t| t.text == "100%"));
                assert!(ui.texts.iter().any(|t| t.text == "100"));
            }
        }
    }

    #[test]
    fn active_slider_keeps_its_frozen_mapping_and_compact_values_fit() {
        let mut ui = Ui::new(1.);
        let active = Action::EditTransform(0);
        let frozen = SliderRange::new(0., 10., 0.1);
        let derived = SliderRange::new(0., 20., 0.1);
        ui.set_slider_range(active.clone(), frozen);
        ui.number_field([10., 20., 240., 30.], "X", 5., active.clone(), derived);
        let track = ui.hits.last().unwrap().rect;
        assert_eq!(
            ui.hits.last().unwrap().action,
            Action::Slide(Box::new(active), frozen, 5.)
        );
        let filled = [track[0] + track[2] * 0.25, track[1] + track[3] * 0.5];
        let fill = ui.slider_fill();
        assert!(ui.rects.iter().any(|q| q.color == fill && contains(q.rect, filled)));
        assert_eq!(track[3], 26.);
        ui.number_field(
            [10., 60., 80., 44.],
            "Y",
            0.001,
            Action::EditTransform(1),
            derived,
        );
        assert_eq!(
            ui.hits.last().unwrap().action,
            Action::Slide(Box::new(Action::EditTransform(1)), derived, 0.001)
        );
        assert!(ui.texts.iter().any(|text| text.text == "0.001"));
    }

    #[test]
    fn slider_snapping_retains_precision_without_projecting_to_pixels() {
        let range = SliderRange::new(1_000_000_000., 1_000_000_100., 0.00001);
        let snapped = range.clamp_and_snap(1_000_000_000.123456);
        assert!((snapped - 1_000_000_000.123_46).abs() < 0.000001);
        assert_eq!(range.clamp_and_snap(f64::NEG_INFINITY), range.min);
        assert_eq!(range.clamp_and_snap(f64::INFINITY), range.max);
        assert_eq!(range.clamp_and_snap(f64::NAN), range.min);
    }

    #[test]
    fn signed_bar_fill_starts_at_zero_and_stays_in_control() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 2.] {
                let r = [20., 20., 240., 30.];
                let mut ui = Ui::with_theme(scale, theme, [0.; 2]);
                ui.number_field(
                    r,
                    "EV",
                    -5.,
                    Action::SurfaceRenderNumber(0),
                    SliderRange::new(-10., 10., 0.1),
                );
                let track = ui.hits.last().unwrap().rect;
                let filled = [
                    (track[0] + track[2] * 0.375) * scale,
                    (track[1] + track[3] * 0.5) * scale,
                ];
                let empty = [
                    (track[0] + track[2] * 0.75) * scale,
                    (track[1] + track[3] * 0.5) * scale,
                ];
                let fill = ui.slider_fill();
                assert!(ui.rects.iter().any(|q| q.color == fill && contains(q.rect, filled)));
                assert!(!ui.rects.iter().any(|q| q.color == fill && contains(q.rect, empty)));
            }
        }
    }

    #[test]
    fn axis_strips_match_gizmo_colors_and_reset_icons_have_no_button_surface() {
        for theme in [Theme::Dark, Theme::Light] {
            for accent in Accent::ALL {
                for (axis, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    let mut ui = Ui::with_theme(2., theme, [0.; 2]);
                    ui.accent = accent;
                    ui.number_field([10., 20., 160., 30.], label, 0.5,
                        Action::EditTransform(axis), SliderRange::new(0., 1., 0.01));
                    let field = ui.hits[0].rect;
                    let reset = ui.hits[1].rect;
                    assert!(reset[0] > field[0] + field[2]);
                    assert_eq!(ui.hit([reset[0] + reset[2] * 0.5, reset[1] + reset[3] * 0.5]),
                        Some(Action::ResetNumber(Box::new(Action::EditTransform(axis)))));
                    assert!(ui.rects.iter().any(|r| r.color == crate::gizmo::axis_color(theme, axis)
                        && r.rect[2] > 0. && r.rect[2] <= 6. && r.rect[0] > field[0] * 2.
                        && r.rect[0] + r.rect[2] < (field[0] + field[2]) * 2.));
                    assert!(ui.rects.iter().any(|r| r.color == ui.slider_fill()));
                    assert!(!ui.rects.iter().any(|r| r.rect == reset.map(|v| v * 2.)));
                    let mut edit = TextEdit::new("0.5".into(), Action::EditTransform(axis));
                    ui.edit_field(field, &mut edit, false);
                    assert!(ui.rects.iter().any(|r| r.color == crate::gizmo::axis_color(theme, axis)));
                }
            }
        }
    }

    #[test]
    fn inline_draft_replaces_stale_text_and_keeps_selection_ime_and_caret_inside_field() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 2.] {
                let r = [20., 30., 124., 30.];
                let mut ui = Ui::with_theme(scale, theme, [0.; 2]);
                ui.field(r, "Старое значение", Action::EditPath);
                ui.label(20., 5., "Путь к файлу", 13., theme.palette().muted);
                let mut edit =
                    TextEdit::new("C:/Проект/Персонажи/骨🦊.glb".into(), Action::EditPath);
                edit.preedit = "新骨".into();
                let original = edit.value.clone();
                ui.edit_field(r, &mut edit, true);
                assert_eq!(edit.value, original);
                assert!(!edit.changed());
                assert!(edit.scroll_start > 0);
                assert_eq!(ui.hits.len(), 1);
                assert!(ui.texts.iter().any(|text| text.text == "Путь к файлу"));
                let texts: Vec<_> = ui
                    .texts
                    .iter()
                    .filter(|text| text.y >= r[1] * scale)
                    .collect();
                assert_eq!(texts.len(), 1);
                assert!(texts[0].text.ends_with("新骨"));
                assert!(!texts[0].text.contains("Старое"));
                let width: f32 = texts[0]
                    .text
                    .chars()
                    .map(|c| character_advance(c, texts[0].size))
                    .sum();
                assert!(texts[0].x + width <= (r[0] + r[2] - FIELD_PADDING) * scale);
                assert!(ui.rects.iter().all(|q| q.rect[0] >= r[0] * scale
                    && q.rect[1] >= r[1] * scale
                    && q.rect[0] + q.rect[2] <= (r[0] + r[2]) * scale + 0.001
                    && q.rect[1] + q.rect[3] <= (r[1] + r[3]) * scale + 0.001));
                assert_eq!(ui.rects.last().unwrap().color, theme.palette().danger);
            }
        }
        let mut ui = Ui::new(2.);
        let mut edit = TextEdit::new("Не показывать обрезанным".into(), Action::EditName);
        ui.edit_field([10., 10., 120., 19.], &mut edit, false);
        assert!(ui.rects.is_empty() && ui.texts.is_empty() && ui.hits.is_empty());
    }

    #[test]
    fn pointer_selection_uses_unicode_positions_and_preserves_the_drag_anchor() {
        let r = [10., 20., 300., 30.];
        let value = "Кость 🦊骨";
        let x_at = |index: usize| {
            r[0] + FIELD_PADDING
                + value
                    .chars()
                    .take(index)
                    .map(|c| character_advance(c, FIELD_FONT_SIZE))
                    .sum::<f32>()
        };
        let mut edit = TextEdit::new(value.into(), Action::EditName);
        edit.click(r, [x_at(2), 35.], false);
        assert_eq!(edit.cursor, 2);
        assert!(edit.selection().is_empty());
        edit.drag(r, [x_at(7), 35.]);
        assert_eq!(edit.selection(), 2..7);
        assert_eq!(edit.selected(), "сть 🦊");
        edit.drag(r, [x_at(1), 35.]);
        assert_eq!(edit.selection(), 1..2);
        edit.end_drag();
        edit.click(r, [x_at(6), 35.], false);
        edit.click(r, [x_at(8), 35.], true);
        assert_eq!(edit.selected(), "🦊骨");
        assert!(!edit.changed());
        assert!(!edit.undo());
    }

    #[test]
    fn pointer_selection_follows_the_visible_scrolled_draft() {
        let r = [10., 20., 95., 30.];
        let mut edit = TextEdit::new("C:/Персонажи/Папка/Кость.glb".into(), Action::EditPath);
        let mut ui = Ui::new(1.);
        ui.edit_field(r, &mut edit, false);
        let visible_start = edit.scroll_start;
        assert!(visible_start > 0);
        edit.click(r, [r[0] + FIELD_PADDING, 35.], false);
        assert_eq!(edit.cursor, visible_start);
        edit.drag(r, [r[0] + r[2] + 40., 35.]);
        assert_eq!(edit.anchor, Some(visible_start));
        assert!(edit.cursor > visible_start);
        let selection = edit.selected();
        assert!(!selection.is_empty());
        assert!(edit.value.ends_with(&selection));
    }

    #[test]
    fn draft_undo_redo_preserves_unicode_selection_and_original_change_state() {
        let mut edit = TextEdit::new("Кость 🦊".into(), Action::EditName);
        edit.insert("骨\n\t🦊\0");
        assert_eq!(edit.value, "骨🦊");
        assert!(edit.changed());
        assert!(edit.undo());
        assert_eq!(edit.value, "Кость 🦊");
        assert_eq!(edit.selected(), "Кость 🦊");
        assert!(!edit.changed());
        assert!(edit.redo());
        assert_eq!(edit.value, "骨🦊");
        edit.preedit = "未確定".into();
        edit.backspace();
        assert_eq!(edit.value, "骨");
        assert!(edit.preedit.is_empty());
        assert!(edit.undo());
        assert_eq!(edit.value, "骨🦊");
        assert_eq!(edit.cursor, 2);
        edit.insert("手");
        assert!(!edit.redo());
        assert_eq!(edit.value, "骨🦊手");
    }

    #[test]
    fn compact_axis_values_show_a_magnitude_instead_of_identical_ellipsis() {
        for value in [-0.408248, -0.816497, -0.557086] {
            let text = number_caption_for_width(value, 37.);
            assert!(text.parse::<f64>().is_ok());
            assert!(!text.contains('…'));
            assert!(
                text.chars()
                    .map(|c| character_advance(c, FIELD_FONT_SIZE))
                    .sum::<f32>()
                    <= 37.
            );
        }
    }

    #[test]
    fn overlay_coverage_preserves_area_and_logical_size_at_both_dpi_scales() {
        for scale in [1., 2.] {
            let mut ui = Ui::new(scale);
            ui.triangle([[10., 10.], [50., 10.], [10., 40.]], [1.; 4]);
            let covered_area: f32 = ui
                .rects
                .iter()
                .map(|r| r.rect[2] * r.rect[3] * r.color[3])
                .sum();
            assert!((covered_area / (scale * scale) - 600.).abs() < 0.5);
            assert!(ui.rects.iter().any(|r| r.color[3] > 0. && r.color[3] < 1.));
            assert!(ui.rects.iter().all(|r| r.rect[0] >= 10. * scale
                && r.rect[0] + r.rect[2] <= 50. * scale
                && r.rect[1] >= 10. * scale
                && r.rect[1] + r.rect[3] <= 40. * scale));
            ui.rects.clear();
            ui.smooth_segment([20., 20.], [80., 20.], 3., [1.; 4]);
            let covered_area: f32 = ui
                .rects
                .iter()
                .map(|r| r.rect[2] * r.rect[3] * r.color[3])
                .sum();
            let expected = 60. * 3.;
            assert!((covered_area / (scale * scale) - expected).abs() < 1.);
            assert!(ui
                .rects
                .iter()
                .all(|r| r.rect.iter().all(|v| v.is_finite()) && r.rect[2] > 0. && r.rect[3] > 0.));
        }
    }

    #[test]
    fn overlay_primitives_ignore_invalid_or_unbounded_geometry() {
        let mut ui = Ui::new(1.);
        ui.triangle([[0., 0.], [f32::NAN, 1.], [1., 2.]], [1.; 4]);
        ui.triangle([[0., 0.], [1., 20_000.], [2., 0.]], [1.; 4]);
        ui.smooth_segment([0., 0.], [10., 10.], f32::INFINITY, [1.; 4]);
        ui.smooth_segment([0., 0.], [10., 10.], 0., [1.; 4]);
        assert!(ui.rects.is_empty());
    }

    #[test]
    fn viewport_conversion_uses_srgb_transfer_and_preserves_alpha() {
        let mut palette = Theme::Dark.palette();
        palette.viewport = [0., 0.04045, 0.5, 0.75];
        let linear = palette.viewport_linear();
        assert_eq!(linear[0], 0.);
        assert!((linear[1] - 0.0031308).abs() < 0.000001);
        assert!((linear[2] - 0.21404114).abs() < 0.000001);
        assert_eq!(linear[3], 0.75);
    }
    #[test]
    fn disabled_controls_are_not_clickable() {
        let mut ui = Ui::with_theme(2., Theme::Light, [20., 20.]);
        ui.disabled_button([10., 10., 100., 30.], "Отмена");
        assert_eq!(ui.hit([20., 20.]), None);
        // Rounded controls consist of multiple quads. Check their actual bounds,
        // not the incidental position of the first central strip.
        let bounds = ui.rects.iter().fold(
            [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ],
            |b, r| {
                [
                    b[0].min(r.rect[0]),
                    b[1].min(r.rect[1]),
                    b[2].max(r.rect[0] + r.rect[2]),
                    b[3].max(r.rect[1] + r.rect[3]),
                ]
            },
        );
        assert_eq!(bounds, [20., 20., 220., 80.]);
        assert_eq!(ui.texts[0].color, Theme::Light.palette().disabled);
        ui.button([10., 50., 100., 30.], "Открыть", Action::Open, false);
        assert_eq!(ui.hit([20., 60.]), Some(Action::Open));
    }

    #[test]
    fn checkbox_square_and_caption_share_one_logical_action_and_checked_shape() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 1.5, 2.] {
                let rect = [10., 20., 132., 28.];
                let mut off = Ui::with_theme(scale, theme, [0., 0.]);
                off.checkbox(rect, "Соседние позы", Action::ToggleSkeleton, false);
                let mut on = Ui::with_theme(scale, theme, [0., 0.]);
                on.checkbox(rect, "Соседние позы", Action::ToggleSkeleton, true);
                for ui in [&off, &on] {
                    assert_eq!(ui.hits.len(), 1);
                    assert_eq!(ui.hits[0].rect, rect);
                    assert_eq!(ui.hit([19., 34.]), Some(Action::ToggleSkeleton));
                    assert_eq!(ui.hit([110., 34.]), Some(Action::ToggleSkeleton));
                    assert_eq!(ui.hit([143., 34.]), None);
                    assert_eq!(ui.texts[0].text, "Соседние позы");
                    for primitive in &ui.rects {
                        let [x, y, w, h] = primitive.rect;
                        assert!(w > 0. && h > 0.);
                        assert!(x >= rect[0] * scale && y >= rect[1] * scale);
                        assert!(x + w <= (rect[0] + rect[2]) * scale + 0.001);
                        assert!(y + h <= (rect[1] + rect[3]) * scale + 0.001);
                    }
                }
                // The check has its own geometry; state remains distinguishable
                // when a display or theme cannot convey the accent hue.
                assert!(on.rects.len() > off.rects.len());
            }
        }
    }

    #[test]
    fn disabled_adorned_controls_have_no_hits_and_follow_theme_tokens() {
        for theme in [Theme::Dark, Theme::Light] {
            let mut ui = Ui::with_theme(2., theme, [20., 20.]);
            ui.disabled_checkbox([10., 10., 160., 28.], "Тени", true);
            ui.disabled_icon_button([10., 50., 100., 28.], "Удалить", Icon::Delete);
            assert_eq!(ui.hit([20., 20.]), None);
            assert_eq!(ui.hit([20., 60.]), None);
            assert!(ui.hits.is_empty());
            assert!(ui
                .texts
                .iter()
                .all(|text| text.color == theme.palette().disabled));
            assert!(ui
                .rects
                .iter()
                .all(|rect| rect.rect.iter().all(|value| value.is_finite())));
        }
    }

    #[test]
    fn compact_adorned_captions_preserve_transport_and_key_editing_labels() {
        for scale in [1., 1.5, 2.] {
            for (width, label, icon) in [
                (80., "Отмена", Icon::Undo),
                (80., "Повтор", Icon::Redo),
                (72., "Играть", Icon::Play),
                (68., "Копия", Icon::Copy),
                (88., "Вставить", Icon::Paste),
                (88., "Дубликат", Icon::Duplicate),
                (82., "Удалить", Icon::Delete),
            ] {
                let mut ui = Ui::new(scale);
                ui.icon_button([5., 10., width, 28.], label, Action::Play, false, icon);
                assert_eq!(ui.texts.last().unwrap().text, label, "{width}px {label}");
                let text = ui.texts.last().unwrap();
                let right = text.x
                    + text
                        .text
                        .chars()
                        .map(|c| character_advance(c, text.size))
                        .sum::<f32>();
                assert!(right <= (5. + width) * scale + 0.001);
            }
            for (width, label) in [
                (58., "Цикл"),
                (92., "Автоключ"),
                (100., "Диапазон"),
                (132., "Соседние позы"),
            ] {
                let mut ui = Ui::new(scale);
                ui.checkbox([5., 10., width, 28.], label, Action::AutoKey, true);
                assert_eq!(ui.texts.last().unwrap().text, label, "{width}px {label}");
            }
        }
    }

    #[test]
    fn icon_only_transport_retains_action_hint_and_shared_field_hit_priority() {
        let mut ui = Ui::with_theme(1., Theme::Dark, [20., 20.]);
        ui.icon_button(
            [10., 10., 28., 28.],
            "",
            Action::PreviousKey,
            false,
            Icon::Previous,
        );
        assert!(ui.texts.is_empty());
        let action = ui.hit([20., 20.]).unwrap();
        assert_eq!(action, Action::PreviousKey);
        assert!(action.hint().is_some());
        // A later field/modal hit owns the same pointer region as it did before
        // decorated controls existed; icon rendering adds no separate targets.
        ui.field([10., 10., 100., 28.], "24", Action::EditFrame);
        assert_eq!(ui.hit([20., 20.]), Some(Action::EditFrame));
        let count = ui.hits.len();
        ui.checkbox([0., 0., f32::NAN, 28.], "Ошибка", Action::AutoKey, true);
        ui.icon_button([0., 0., 0., 28.], "Ошибка", Action::Play, false, Icon::Play);
        assert_eq!(ui.hits.len(), count);
    }
    #[test]
    fn theme_switch_updates_hover_and_selection_colors() {
        for theme in [Theme::Dark, Theme::Light] {
            let p = theme.palette();
            let mut ui = Ui::with_theme(1., theme, [20., 20.]);
            ui.button([10., 10., 100., 30.], "Обзор", Action::Orbit, false);
            let fill_at = |ui: &Ui, point| {
                ui.rects
                    .iter()
                    .rev()
                    .find(|r| contains(r.rect, point))
                    .unwrap()
                    .color
            };
            assert_eq!(fill_at(&ui, [40., 25.]), p.hover);
            ui.button([10., 50., 100., 30.], "Поза", Action::Mode(1), true);
            assert_eq!(fill_at(&ui, [40., 65.]), p.selection);
            assert_eq!(ui.texts[1].color, p.accent);
            assert_eq!(Theme::from_name(theme.as_str()), theme);
        }
    }
    #[test]
    fn both_themes_have_readable_primary_and_secondary_text() {
        fn luminance(c: [f32; 4]) -> f32 {
            let channel = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
        }
        fn contrast(a: [f32; 4], b: [f32; 4]) -> f32 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for theme in [Theme::Dark, Theme::Light] {
            let p = theme.palette();
            assert!(contrast(p.text, p.elevated) >= 4.5);
            assert!(contrast(p.muted, p.panel) >= 4.5);
            assert!(contrast(p.accent, p.selection) >= 4.5);
            for accent in Accent::ALL {
                let mut ui = Ui::with_theme(1., theme, [0.; 2]);
                ui.accent = accent;
                assert!(contrast(ui.palette().text, ui.slider_fill()) >= 4.5);
            }
        }
    }
    #[test]
    fn compact_button_captions_remain_visible() {
        let mut ui = Ui::new(1.);
        for (width, text) in [(22., ">"), (22., "−"), (23., "×"), (28., "×"), (30., "|<")] {
            ui.button([10., 10., width, 28.], text, Action::ClearSearch, false);
            let caption = ui.texts.last().unwrap();
            assert_eq!(caption.text, text);
            assert!(caption.x >= 14. && caption.x < 10. + width);
            ui.quiet_button([10., 10., width, 28.], text, Action::ClearSearch, false);
            assert_eq!(ui.texts.last().unwrap().text, text);
        }
    }
    #[test]
    fn narrow_captions_truncate_at_unicode_boundaries() {
        assert_eq!(fit_button_label("Кисть", 200., 13.), "Кисть");
        let shortened = fit_button_label("Выбрать персонажа", 70., 13.);
        assert!(shortened.ends_with('…'));
        assert!(shortened.chars().count() < "Выбрать персонажа".chars().count());
        assert_eq!(fit_button_label("骨格", 26., 13.), "骨格");
        assert!(fit_button_label("Сцена", 8., 13.).is_empty());
    }
    #[test]
    fn cyrillic_selection_is_safe() {
        let mut e = TextEdit::new("Персонаж 🦊".into(), Action::EditTitle);
        e.insert("Кисть");
        e.move_cursor(-1, false);
        e.backspace();
        assert_eq!(e.value, "Кись");
        e.select_all();
        assert_eq!(e.selected(), "Кись");
        e.insert("骨");
        assert_eq!(e.value, "骨");
    }
}

impl Action {
    fn field_padding(&self)->f32 {
        if matches!(self, Self::Author(AuthorAction::Coordinate(_) | AuthorAction::RotateDegrees(_) | AuthorAction::ScaleFactor(_))) {8.} else {FIELD_PADDING}
    }
}
#[cfg(test)]
mod text_entry_spacing_tests {
    use super::*;
    #[test]
    fn entry_has_no_inserted_prefix_and_idle_active_origins_agree() {
        for scale in [1.,2.] {for theme in [Theme::Dark,Theme::Light] {
            let purpose=Action::Text(TextAction::Content);let mut edit=TextEdit::new("Текст".into(),purpose.clone());edit.insert("Привет");assert_eq!(edit.value,"Привет");
            edit.home(false);edit.insert("А");assert_eq!(edit.value,"АПривет");edit.undo();assert_eq!(edit.value,"Привет");edit.end(false);edit.insert(" мир");assert_eq!(edit.value,"Привет мир");
            let mut u=Ui::new(scale);u.theme=theme;let r=[20.,30.,240.,32.];u.field(r,&edit.value,purpose);let idle=u.texts.last().unwrap().x;u.edit_field(r,&mut edit,false);
            assert_eq!(u.texts.last().unwrap().text,"Привет мир");assert_eq!(u.texts.last().unwrap().x,idle);assert_eq!(idle,(r[0]+4.)*scale);
        }}
    }
}
