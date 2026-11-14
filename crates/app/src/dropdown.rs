//! Anchored selection popovers. Candidates are transient; only explicit commit edits the document.
use super::{App, Tool};
use super::curve_binding_controls::BindingTarget;
use crate::icons::Icon;
use crate::ui::{Action, CurveAction, DropdownKind, Hit, Ui};
use crate::ui::RigReference;
use super::rig_references;
use poseforge_core::{AlphaMode, Light, LightKind, Material, MaterialPreset, ToneMapping};
use winit::{
    event::MouseButton,
    keyboard::{Key, NamedKey},
};

#[derive(Clone)]
enum Target {
    CameraEffects { node: u64, effects: poseforge_core::CameraEffects },
    Scene,
    Bone(Vec<poseforge_core::Node>),
    Array {node:poseforge_core::Node, modifier:u64, curves:Vec<u64>},
    Modifiers {node:poseforge_core::Node, modifier:Option<u64>, choices:Vec<u64>},
    Helper(poseforge_core::Node),
    Text {node:u64,settings:poseforge_core::TextSettings},
    RigReference { reference: RigReference, choices: Vec<u64>, selection: Option<u64>, skeleton: Option<u64>, mesh: Option<u64> },
    Path { target: BindingTarget, curves: Vec<u64>, light: Option<Light> },
    MaterialList { selection: Option<u64>, mesh: Option<u64> },
    Library,
    Mesh {
        selection: Option<u64>,
        mesh: u64,
        material: Option<u64>,
    },
    Light(usize, Light),
    Material {
        library: bool,
        selection: Option<u64>,
        mesh: u64,
        material: Box<Material>,
    },
}

#[derive(Clone)]
pub(super) struct Dropdown {
    kind: DropdownKind,
    anchor: [f32; 4],
    candidate: usize,
    offset: usize,
    dragging: bool,
    target: Target,
    query: String,
    entries: Vec<MaterialLibraryEntry>,
    category: Option<String>,
    category_offset: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MaterialLibraryEntry {
    Preset(usize),
    Saved(u64),
}
impl MaterialLibraryEntry {
    fn action(self) -> Action {
        match self {
            Self::Preset(index) => Action::AssetPreset(index),
            Self::Saved(id) => Action::AssetMaterial(id),
        }
    }
}

fn modifier_targets(project:&poseforge_core::Project,owner:u64)->Vec<u64> {
    project.nodes.iter().filter(|n|n.id!=owner && project.modifier_supported(n.id) && !project.node_locked(n.id)).map(|n|n.id).collect()
}
fn preset_category(preset: MaterialPreset) -> &'static str { preset.category() }

fn labels(kind: DropdownKind, project: &poseforge_core::Project) -> Vec<String> {
    if let Some(reference) = RigReference::from_dropdown(kind) {
        let ids = rig_references::choices(project, reference);
        if ids.is_empty() { return vec![match reference { RigReference::Skeleton => "Нет доступных скелетов", RigReference::Mesh => "Нет доступных мешей" }.into()]; }
        let nodes: std::collections::BTreeMap<_, _> = project.nodes.iter().map(|n| (n.id, n)).collect();
        let mut counts = std::collections::BTreeMap::new();
        for node in ids.iter().filter_map(|id| nodes.get(id)) {
            *counts.entry(node.name.as_str()).or_insert(0usize) += 1;
        }
        return ids.iter().filter_map(|id| nodes.get(id)).map(|node| {
            if counts.get(node.name.as_str()).copied().unwrap_or(0) > 1 {
                format!("{} · №{}", node.name, node.id)
            } else { node.name.clone() }
        }).collect();
    }
    if kind == DropdownKind::CurvePath { return project.nodes.iter().map(|n| format!("{} · {}", n.name, if n.is_joint { "Кость" } else if n.empty { "Пустышка" } else if n.camera.is_some() { "Камера" } else if n.curve.is_some() { "Кривая" } else if n.mesh.is_some() { "Меш" } else { "Объект" })).collect(); }
    if kind == DropdownKind::MaterialLibrary {
        return project.materials.iter().map(|m| m.name.clone()).collect();
    }
    if kind == DropdownKind::MaterialCategory {
        return std::iter::once("Все".to_string())
            .chain(
                project
                    .materials
                    .iter()
                    .filter(|m| !m.category.is_empty())
                    .map(|m| m.category.clone())
                    .collect::<std::collections::BTreeSet<_>>(),
            )
            .collect();
    }
    if kind == DropdownKind::SavedViews {
        return if project.saved_views.is_empty() {
            vec!["Нет сохранённых ракурсов".into()]
        } else {
            project.saved_views.iter().map(|v| v.name.clone()).collect()
        };
    }
    if kind == DropdownKind::Collections {
        return std::iter::once("Без папки".into())
            .chain(project.collections.iter().map(|c| c.name.clone()))
            .collect();
    }
    if let DropdownKind::ArrayCurve(..)=kind {
        return std::iter::once("Не выбрана".into()).chain(project.nodes.iter().filter(|n|n.curve.is_some()).map(|n|format!("{} · №{}",n.name,n.id))).collect();
    }
    if let DropdownKind::ModifierTarget(_,owner)=kind {
        return modifier_targets(project,owner).iter().filter_map(|id|project.node(*id))
            .map(|n|format!("{} · №{}",n.name,n.id)).collect();
    }
    let items = match kind {
        DropdownKind::CameraEffectAdd => poseforge_core::CameraEffects::LABELS.to_vec(),
        DropdownKind::ModifierTarget(..) | DropdownKind::ArrayCurve(..) => unreachable!(),
        DropdownKind::ArrayMode(..)=>vec!["Линейный массив","По окружности","Вдоль кривой"],
        DropdownKind::ModifierAdd => poseforge_core::ModifierKind::ALL.into_iter().map(|k|k.label()).collect(),
        DropdownKind::ReflectionSamples => vec!["1 луч", "4 луча", "8 лучей", "16 лучей"],
        DropdownKind::AreaShadowSamples => vec!["4 выборки", "16 выборок", "64 выборки"],
        DropdownKind::Background => vec!["Тема интерфейса", "Однотонный"],
        DropdownKind::MeshCreate => poseforge_core::PrimitiveKind::MESH_MENU.into_iter().map(|(_,k)|poseforge_core::PrimitiveSettings::new(k).label()).collect(),
        DropdownKind::EmptyCreate|DropdownKind::EmptyShape => poseforge_core::EmptyShape::ALL.into_iter().map(|s|s.label()).collect(),
        DropdownKind::ForceCreate|DropdownKind::ForceKind => poseforge_core::ForceFieldKind::ALL.into_iter().map(|s|s.label()).collect(),
        DropdownKind::FxCreate|DropdownKind::FxKind => poseforge_core::FxKind::ALL.into_iter().map(|s|s.label()).collect(),
        DropdownKind::FxPreset(k) => poseforge_core::FxKind::ALL.get(k as usize).map(|k|poseforge_core::FxSettings::preset_labels(*k)).unwrap_or_default(),
        DropdownKind::FxEmission => poseforge_core::FxEmission::ALL.into_iter().map(|q|q.label()).collect(),
        DropdownKind::FxQuality => poseforge_core::FxQuality::ALL.into_iter().map(|q|q.label()).collect(),
        DropdownKind::TextFont => poseforge_core::TextFont::ALL.into_iter().map(|font|font.label()).collect(),
        DropdownKind::CurveCreate => poseforge_core::CurvePreset::BASIC.into_iter().map(|p| p.label()).chain(std::iter::once("Нарисовать кривую")).collect(),
        DropdownKind::SkeletonCreate => vec!["Пустая кость", "Цепь · 3 кости", "Человек · 67 костей"],
        DropdownKind::SnapMode=>super::snapping::LABELS.to_vec(),
        DropdownKind::KnifeAngle=>vec!["Свободный угол","Шаг 15°","Шаг 30°","Шаг 45°","Шаг 90°"],
        DropdownKind::RepairKind=>poseforge_core::RepairKind::ALL.into_iter().map(|k|k.label()).collect(),
        DropdownKind::Alignment => vec!["В плоскость", "В цепочку", "Равномерный шаг"],
        DropdownKind::BonePreset => poseforge_core::BoneDisplayPreset::ALL.into_iter().map(|p| p.label()).collect(),
        DropdownKind::BoneShape => poseforge_core::BoneShape::ALL.into_iter().map(|s| s.label()).collect(),
        DropdownKind::JointShape => poseforge_core::JointShape::ALL.into_iter().map(|s| s.label()).collect(),
        DropdownKind::PhysicsType => vec!["Без физики", "Коллайдер · объём меша", "Ткань · XPBD", "Жидкость · контейнер", "Разрушение · осколки"],
        DropdownKind::FluidSource => {
            vec!["Прямоугольный объём", "Сферический объём", "Столб жидкости"]
        }
        DropdownKind::RenderQuality => vec!["Быстро", "Баланс", "Максимум"],
        DropdownKind::Antialiasing => vec!["Без сглаживания · 1×", "MSAA · 4×"],
        DropdownKind::ToneMapping => vec!["ACES", "Reinhard", "Линейное"],
        DropdownKind::ShadowResolution => vec!["512 px", "1024 px", "2048 px"],
        DropdownKind::LightingPreset => vec!["Студия", "Мягкий свет", "Контраст", "Софтбокс"],
        DropdownKind::LightKind | DropdownKind::LightCreate => {
            vec!["Солнце", "Точка", "Прожектор", "Площадной"]
        }
        DropdownKind::MaterialCreate => {
            MaterialPreset::ALL.into_iter().map(|p| p.label()).collect()
        }
        DropdownKind::MaterialActions => vec![
            "Создать копию",
            "Переименовать",
            "Открыть узлы",
            "Убрать неиспользуемые",
            "Избранное · вкл/выкл",
            "Категория…",
            "Теги…",
            "Добавить / убрать из библиотеки",
            "Поднять слой",
            "Опустить слой",
            "Создать слой по шаблону…",
            "Назначить выбранным граням",
        ],
        DropdownKind::MaterialAlpha => vec!["Непрозрачный", "Отсечение", "Смешивание"],
        DropdownKind::Accent => crate::ui::Accent::ALL
            .into_iter()
            .map(|a| a.label())
            .collect(),
        DropdownKind::SceneFilter => vec!["Меши", "Кости", "Освещение", "Камеры"],
        DropdownKind::SceneActions => vec![
            "Выделить все объекты",
            "Снять выделение",
            "Изоляция · вкл/выкл",
            "Новая папка…",
            "Переместить в папку…",
            "Переименовать папку…",
            "Папка · видимость в рендере",
            "Удалить папку",
            "К выбранному",
            "Дубликат",
        ],
        DropdownKind::AssetLibrary => vec![],
        DropdownKind::MaterialLibrary | DropdownKind::MaterialCategory | DropdownKind::SavedViews | DropdownKind::Collections | DropdownKind::CurvePath | DropdownKind::RigSkeleton | DropdownKind::RigMesh => {
            unreachable!()
        }
    };
    items.into_iter().map(str::to_owned).collect()
}

fn popup_rect(anchor: [f32; 4], size: [f32; 2], count: usize) -> [f32; 4] {
    let width = anchor[2].max(220.).min((size[0] - 16.).max(40.));
    let height = (count.min(6) as f32 * 32. + 12.).min((size[1] - 16.).max(20.));
    let x = anchor[0].clamp(8., (size[0] - width - 8.).max(8.));
    let below = anchor[1] + anchor[3] + 4.;
    let y = if below + height <= size[1] - 8. {
        below
    } else {
        anchor[1] - height - 4.
    }
    .clamp(8., (size[1] - height - 8.).max(8.));
    [x, y, width, height]
}

fn item_icon(kind: DropdownKind, index: usize) -> Option<Icon> {
    match kind {
        DropdownKind::CameraEffectAdd => Some(Icon::Wand),
        DropdownKind::CurvePath => Some(Icon::Object),
        DropdownKind::RigSkeleton => Some(Icon::Bone),
        DropdownKind::RigMesh => Some(Icon::Object),
        DropdownKind::MeshCreate => poseforge_core::PrimitiveKind::MESH_MENU.get(index).map(|(_,k)|super::primitive_controls::primitive_icon(*k)),
        DropdownKind::EmptyCreate|DropdownKind::EmptyShape => [Icon::Empty,Icon::Move,Icon::CurveCircle,Icon::Object,Icon::Sphere,Icon::Cone].get(index).copied(),
        DropdownKind::ForceCreate|DropdownKind::ForceKind => poseforge_core::ForceFieldKind::ALL.get(index).map(|k|super::empty_controls::field_icon(*k)),
        DropdownKind::FxCreate|DropdownKind::FxKind => poseforge_core::FxKind::ALL.get(index).map(|k|super::fx_controls::fx_icon(*k)),
        DropdownKind::CurveCreate => Some(if matches!(index, 1 | 4) { Icon::CurveCircle } else { Icon::Curve }),
        DropdownKind::SkeletonCreate => Some(Icon::Bone),
        DropdownKind::PhysicsType => [Icon::Object, Icon::Physics, Icon::Cloth, Icon::Fluid, Icon::Fracture].get(index).copied(),
        DropdownKind::LightCreate | DropdownKind::LightKind => {
            [Icon::Sun, Icon::Light, Icon::Spot, Icon::Area]
                .get(index)
                .copied()
        }
        _ => None,
    }
}

impl App {
    pub(super) fn dropdown_is_open(&self, kind: DropdownKind) -> bool {
        self.dropdown
            .as_ref()
            .is_some_and(|dropdown| dropdown.kind == kind)
    }

    pub(super) fn dropdown_selected(&self, kind: DropdownKind) -> Option<usize> {
        let settings = self.editor.project.render_settings;
        match kind {
            DropdownKind::SnapMode=>Some(self.authoring.snapping.mode as usize),
            DropdownKind::KnifeAngle=>Some(self.authoring.knife_angle as usize),
            DropdownKind::RepairKind=>Some(self.authoring.repair_kind as usize),
            DropdownKind::Alignment => Some(self.mesh_tools.alignment_mode as usize),
            DropdownKind::ArrayMode(node,id)=>self.editor.project.node(node).and_then(|n|n.modifiers.iter().find(|m|m.id==id)).map(|m|match m.settings {poseforge_core::ModifierSettings::RadialArray {..}=>1,poseforge_core::ModifierSettings::CurveArray {..}=>2,_=>0}),
            DropdownKind::ArrayCurve(node,id)=>self.editor.project.node(node).and_then(|n|n.modifiers.iter().find(|m|m.id==id)).and_then(|m|if let poseforge_core::ModifierSettings::CurveArray {curve,..}=m.settings {Some(curve.and_then(|c|self.editor.project.nodes.iter().filter(|n|n.curve.is_some()).position(|n|n.id==c)).map_or(0,|i|i+1))}else{None}),
            DropdownKind::ModifierAdd | DropdownKind::ModifierTarget(..) => None,
            DropdownKind::BonePreset => self.bone_display_preset_index(),
            DropdownKind::BoneShape => self.bone_display_shape_index(false),
            DropdownKind::JointShape => self.bone_display_shape_index(true),
            DropdownKind::EmptyShape=>self.selected_helper().and_then(|n|poseforge_core::EmptyShape::ALL.iter().position(|s|*s==n.empty_shape)),
            DropdownKind::FxKind=>self.selected_fx().map(|(_,f)|f.kind.index() as usize),
            DropdownKind::FxQuality=>self.selected_fx().and_then(|(_,f)|poseforge_core::FxQuality::ALL.iter().position(|q|*q==f.quality)),
            DropdownKind::FxPreset(_)=>None,
            DropdownKind::FxEmission=>self.selected_fx().map(|(_,f)|f.source_mode().index() as usize),
            DropdownKind::ForceKind=>self.selected_helper().and_then(|n|n.force_field).and_then(|f|poseforge_core::ForceFieldKind::ALL.iter().position(|k|*k==f.kind)),
            DropdownKind::TextFont => self.selected_text().and_then(|(_,s)|poseforge_core::TextFont::ALL.iter().position(|&font|font==s.font)),
            DropdownKind::RigSkeleton | DropdownKind::RigMesh => {
                let reference = RigReference::from_dropdown(kind).unwrap();
                self.rig_reference_value(reference).and_then(|id| rig_references::choices(&self.editor.project, reference).iter().position(|v| *v == id))
            }
            DropdownKind::CurvePath => self.path_binding().and_then(|b| self.path_curve_choices().iter().position(|id| *id == b.curve)),
            DropdownKind::Accent => crate::ui::Accent::ALL
                .iter()
                .position(|a| *a == self.preferences.accent),
            DropdownKind::MaterialLibrary => self.selected_material_id().and_then(|id| self.editor.project.materials.iter().position(|m| m.id == id)),
            DropdownKind::SceneActions | DropdownKind::SceneFilter | DropdownKind::AssetLibrary => None,
            DropdownKind::Collections => Some(
                self.scene_tools
                    .collection
                    .and_then(|id| {
                        self.editor
                            .project
                            .collections
                            .iter()
                            .position(|c| c.id == id)
                    })
                    .map_or(0, |i| i + 1),
            ),
            DropdownKind::SavedViews => self.last_saved_view,
            DropdownKind::CameraEffectAdd => None,
            DropdownKind::MaterialCategory => labels(kind, &self.editor.project)
                .iter()
                .position(|s| *s == self.preferences.material_category)
                .or(Some(0)),
            DropdownKind::ReflectionSamples => [1, 4, 8, 16]
                .iter()
                .position(|n| *n == settings.reflection_samples),
            DropdownKind::AreaShadowSamples => [4, 16, 64]
                .iter()
                .position(|n| *n == settings.area_shadow_samples),
            DropdownKind::EmptyCreate|DropdownKind::ForceCreate|DropdownKind::FxCreate|DropdownKind::MeshCreate
            | DropdownKind::CurveCreate
            | DropdownKind::SkeletonCreate
            | DropdownKind::FluidSource
            | DropdownKind::MaterialCreate
            | DropdownKind::MaterialActions => None,
            DropdownKind::PhysicsType => {
                if self.destruction_source().and_then(|id| self.editor.project.node(id)).is_some_and(|n| n.destruction.is_some()) { return Some(4); }
                if self.selected_fluid().is_some() { return Some(3); }
                match self
                    .editor
                    .selected
                    .and_then(|id| self.editor.project.node(id))
                    .and_then(|n| n.physics)
                    .map(|p| p.kind)
                {
                    None => Some(0),
                    Some(poseforge_core::PhysicsKind::Collider) => Some(1),
                    Some(poseforge_core::PhysicsKind::Cloth) => Some(2),
                    Some(poseforge_core::PhysicsKind::Muscle) => None,
                }
            }
            DropdownKind::Background => match settings.background_mode {
                poseforge_core::BackgroundMode::Theme => Some(0),
                poseforge_core::BackgroundMode::Solid => Some(1),
                _ => None,
            },
            DropdownKind::RenderQuality => {
                (0..3).find(|&q| super::scene_controls::quality_selected(settings, q as u8))
            }
            DropdownKind::Antialiasing => Some(usize::from(settings.antialiasing == 4)),
            DropdownKind::ToneMapping => Some(match settings.tone_mapping {
                ToneMapping::Aces => 0,
                ToneMapping::Reinhard => 1,
                ToneMapping::Linear => 2,
            }),
            DropdownKind::ShadowResolution => [512, 1024, 2048]
                .iter()
                .position(|&v| settings.shadow_resolution == v),
            DropdownKind::LightingPreset | DropdownKind::LightCreate => None,
            DropdownKind::LightKind => {
                self.surface_light_index()
                    .map(|i| match self.editor.project.lights[i].kind {
                        LightKind::Directional => 0,
                        LightKind::Point => 1,
                        LightKind::Spot => 2,
                        LightKind::Area => 3,
                    })
            }
            DropdownKind::MaterialAlpha => self
                .selected_material_id()
                .and_then(|id| self.editor.project.material(id))
                .map(|m| match m.alpha_mode {
                    AlphaMode::Opaque => 0,
                    AlphaMode::Mask => 1,
                    AlphaMode::Blend => 2,
                }),
        }
    }

    pub(super) fn draw_dropdown_button(
        &self,
        ui: &mut Ui,
        rect: [f32; 4],
        kind: DropdownKind,
        fallback: &str,
    ) {
        // Closed bindings do not need to allocate a label for every bone in the scene.
        let items = if kind == DropdownKind::CurvePath { Vec::new() } else { labels(kind, &self.editor.project) };
        let label = if kind == DropdownKind::CurvePath { fallback } else {
            self.dropdown_selected(kind).and_then(|i| items.get(i)).map(String::as_str).unwrap_or(fallback)
        };
        let palette = ui.palette();
        let focused = self.dropdown_focus == Some(kind) || self.dropdown_is_open(kind);
        ui.button(rect, "", Action::DropdownOpen(kind), focused);
        if kind == DropdownKind::Accent {
            ui.rounded_rect(
                [rect[0] + 10., rect[1] + (rect[3] - 16.) * 0.5, 16., 16.],
                palette.accent,
                8.,
            );
        }
        let inset = if kind == DropdownKind::Accent {
            36.
        } else {
            10.
        };
        ui.fitted_label(
            [
                rect[0] + inset,
                rect[1] + (rect[3] - 16.) * 0.5,
                rect[2] - inset - 26.,
                16.,
            ],
            label,
            12.,
            if focused {
                palette.accent
            } else {
                palette.text
            },
        );
        let cx = rect[0] + rect[2] - 15.;
        let cy = rect[1] + rect[3] * 0.5;
        // A small vector chevron remains crisp without depending on a font glyph.
        ui.smooth_segment([cx - 4., cy - 2.], [cx, cy + 2.], 1.7, palette.muted);
        ui.smooth_segment([cx, cy + 2.], [cx + 4., cy - 2.], 1.7, palette.muted);
    }

    fn dropdown_valid(&self, dropdown: &Dropdown) -> bool {
        if self.job.is_some() || self.native_picker.is_some() {
            return false;
        }
        if dropdown.kind == DropdownKind::SkeletonCreate
            && (self.editor.has_staged() || self.editor.playing || self.scene_write_pending())
        {
            return false;
        }
        match &dropdown.target {
            Target::CameraEffects {node,effects} => self.surface.tab==0 && self.camera_effect_editable()
                && self.selected_camera_effects()==Some((*node,*effects)),
            Target::Scene => true,
            Target::Array {node,modifier,curves}=>self.surface.tab==8 && self.editor.selected==Some(node.id) && self.modifier_editable(node.id)
                && self.editor.project.node(node.id)==Some(node) && self.selected_modifier().is_some_and(|(owner,m)|owner==node.id && m.id==*modifier)
                && *curves==self.editor.project.nodes.iter().filter(|n|n.curve.is_some()).map(|n|n.id).collect::<Vec<_>>(),
            Target::Modifiers {node,modifier,choices} => self.surface.tab==8 && self.editor.selected==Some(node.id)
                && self.modifier_editable(node.id) && self.editor.project.node(node.id)==Some(node)
                && (modifier.is_none() || self.selected_modifier().is_some_and(|(id,m)|id==node.id && Some(m.id)==*modifier)
                    && modifier_targets(&self.editor.project,node.id)==*choices),
            Target::Bone(nodes) => self.surface.tab == 0 && self.bone_display_editable()
                && self.bone_display_ids() == nodes.iter().map(|n|n.id).collect()
                && nodes.iter().all(|n| self.editor.project.node(n.id) == Some(n)),
            Target::Helper(node)=>self.selected_helper()==Some(node) && self.helper_editable(node.id),
            Target::Text {node,settings} => self.surface.tab==0 && self.selected_text()==Some((*node,settings)) && self.text_editable(*node),
            Target::RigReference { reference, choices, selection, skeleton, mesh } => self.rig_reference_editable()
                && self.editor.selected == *selection && self.authoring.skeleton == *skeleton && self.rig_target() == *mesh
                && rig_references::choices(&self.editor.project, *reference) == *choices,
            Target::Path { target, curves, light } => self.path_target() == Some(*target) && self.path_editable()
                && self.path_curve_choices() == *curves && match target {
                    BindingTarget::Light(i) => self.editor.project.lights.get(*i) == light.as_ref(),
                    BindingTarget::Node(_) => true,
                },
            Target::MaterialList { selection, mesh } => self.surface.tab == 1 && self.preferences.right_panel
                && self.editor.selected == *selection && self.surface_mesh_id() == *mesh && !self.material_locked(),
            Target::Library => self.surface.tab == 1 && self.surface.material_library,
            Target::Mesh {
                selection,
                mesh,
                material,
            } => {
                self.surface.tab == 1
                    && !self.material_locked()
                    && self.editor.selected == *selection
                    && self.surface_mesh_id() == Some(*mesh)
                    && self
                        .editor
                        .project
                        .mesh(*mesh)
                        .is_some_and(|m| m.material == *material)
            }
            Target::Light(index, light) => {
                self.surface.tab == 0
                    && self.active_light_index().is_some()
                    && self.surface_light_index() == Some(*index)
                    && self.editor.project.lights.get(*index) == Some(light)
            }
            Target::Material {
                library,
                selection,
                mesh,
                material,
            } => {
                self.surface.tab == 1
                    && !self.material_locked()
                    && self.surface.material_library == *library
                    && (*library
                        || (self.editor.selected == *selection
                            && self.surface_mesh_id() == Some(*mesh)))
                    && self.selected_material_id() == Some(material.id)
                    && self.editor.project.material(material.id) == Some(material.as_ref())
            }
        }
    }

    pub(super) fn open_dropdown(&mut self, kind: DropdownKind) {
        if self.dropdown.as_ref().is_some_and(|d| d.kind == kind) {
            self.close_asset_popup();
            self.close_material_dropdown();
            self.dropdown = None;
            return;
        }
        self.close_asset_popup();
        self.close_material_dropdown();
        self.dropdown = None;
        if self.has_modal() || self.job.is_some() || self.native_picker.is_some() {
            return;
        }
        if kind == DropdownKind::AssetLibrary && self.editor.has_staged() { return; }
        if kind == DropdownKind::SkeletonCreate
            && (self.editor.has_staged() || self.editor.playing || self.scene_write_pending())
        {
            return;
        }
        let Some(anchor) = self
            .ui
            .hits
            .iter()
            .filter(|hit| hit.action == Action::DropdownOpen(kind))
            .max_by_key(|hit| crate::ui::contains(hit.rect, self.mouse))
            .map(|hit| hit.rect)
            .or_else(|| {
                (matches!(kind, DropdownKind::MaterialLibrary | DropdownKind::MaterialCreate) && self.surface.tab == 1).then(|| {
                    let r = self.right_panel_rect(); [r[0] + r[2] - 42., self.properties_content_rect()[1] + 28., 26., 28.]
                })
            })
            .or_else(|| {
                (kind == DropdownKind::Collections).then(|| {
                    let r = self.left_panel_rect();
                    [r[0] + r[2] - 38., r[1] + 4., 28., 28.]
                })
            })
        else {
            return;
        };
        let target = match kind {
            DropdownKind::CameraEffectAdd => {
                if self.surface.tab!=0 || !self.camera_effect_editable() {return;}
                let Some((node,effects))=self.selected_camera_effects() else {return;};
                Target::CameraEffects {node,effects}
            },
            DropdownKind::ArrayMode(owner,id)|DropdownKind::ArrayCurve(owner,id)=> {
                let Some(node)=self.editor.project.node(owner).filter(|n|self.modifier_editable(n.id)) else {return;};
                if self.surface.tab!=8 || self.selected_modifier().is_none_or(|(n,m)|n!=owner||m.id!=id) {return;}
                Target::Array {node:node.clone(),modifier:id,curves:self.editor.project.nodes.iter().filter(|n|n.curve.is_some()).map(|n|n.id).collect()}
            }
            DropdownKind::ModifierAdd | DropdownKind::ModifierTarget(..) => {
                let Some(node)=self.editor.selected.and_then(|id|self.editor.project.node(id)).filter(|n|self.modifier_editable(n.id)) else {return;};
                if self.surface.tab!=8 {return;}
                let modifier=if let DropdownKind::ModifierTarget(_,owner)=kind {
                    if owner!=node.id {return;}
                    let Some((_,m))=self.selected_modifier() else {return;};Some(m.id)
                }else{None};
                let choices=if modifier.is_some() {modifier_targets(&self.editor.project,node.id)}else{vec![]};
                if modifier.is_some() && choices.is_empty() {return;}
                Target::Modifiers {node:node.clone(),modifier,choices}
            },
            DropdownKind::BoneShape | DropdownKind::JointShape | DropdownKind::BonePreset => {
                if !self.bone_display_editable() || self.surface.tab != 0 { return; }
                Target::Bone(self.bone_display_ids().into_iter().filter_map(|id|self.editor.project.node(id).cloned()).collect())
            }
            DropdownKind::EmptyShape|DropdownKind::ForceKind|DropdownKind::FxKind|DropdownKind::FxPreset(_)|DropdownKind::FxQuality|DropdownKind::FxEmission=>{let Some(n)=self.selected_helper().filter(|n|self.helper_editable(n.id)) else{return;};Target::Helper(n.clone())},
            DropdownKind::TextFont => {
                let Some((node,settings))=self.selected_text() else{return;};
                if !self.text_editable(node) {return;}
                Target::Text {node,settings:settings.clone()}
            }
            DropdownKind::RigSkeleton | DropdownKind::RigMesh => {
                if !self.rig_reference_editable() { return; }
                let reference = RigReference::from_dropdown(kind).unwrap();
                Target::RigReference { reference, choices: rig_references::choices(&self.editor.project, reference),
                    selection: self.editor.selected, skeleton: self.authoring.skeleton, mesh: self.rig_target() }
            }
            DropdownKind::CurvePath => {
                if !self.path_editable() { return; }
                let Some(target) = self.path_target() else { return; };
                let curves = self.path_curve_choices();
                if curves.is_empty() { return; }
                let light = if let BindingTarget::Light(i) = target { self.editor.project.lights.get(i).cloned() } else { None };
                Target::Path { target, curves, light }
            },
            DropdownKind::MaterialLibrary => {
                if self.material_locked() { return; }
                Target::MaterialList { selection: self.editor.selected, mesh: self.surface_mesh_id() }
            }
            DropdownKind::MaterialCreate if self.surface.material_library => Target::Library,
            DropdownKind::MaterialCreate => {
                if self.material_locked() {
                    return;
                }
                let Some(mesh) = self
                    .surface_mesh_id()
                    .and_then(|id| self.editor.project.mesh(id))
                else {
                    return;
                };
                Target::Mesh {
                    selection: self.editor.selected,
                    mesh: mesh.id,
                    material: mesh.material,
                }
            }
            DropdownKind::LightKind => {
                let Some(index) = self.surface_light_index() else {
                    return;
                };
                Target::Light(index, self.editor.project.lights[index].clone())
            }
            DropdownKind::MaterialAlpha
            | DropdownKind::MaterialActions => {
                let mesh = self.surface_mesh_id().unwrap_or(0);
                let Some(material) = self
                    .selected_material_id()
                    .and_then(|id| self.editor.project.material(id))
                    .cloned()
                else {
                    return;
                };
                Target::Material {
                    library: self.surface.material_library,
                    selection: self.editor.selected,
                    mesh,
                    material: Box::new(material),
                }
            }
            _ => Target::Scene,
        };
        self.menu = None;
        self.dropdown_focus = Some(kind);
        self.dropdown = Some(Dropdown {
            kind,
            anchor,
            candidate: self.dropdown_selected(kind).unwrap_or(0),
            offset: self.dropdown_selected(kind).unwrap_or(0).saturating_sub(4),
            dragging: false,
            target,
            query: String::new(), entries: Vec::new(), category: None, category_offset: 0,
        });
        if kind == DropdownKind::AssetLibrary { self.begin_asset_popup(anchor); }
        if kind == DropdownKind::MaterialLibrary {
            self.preferences.collapsed_sections.remove(&100);
            self.inspector_scroll = 0.;
            self.surface.material_query.clear();
            self.input(Action::MaterialSearch, String::new());
            self.refresh_material_dropdown();
        }
        self.status =
            "↑/↓ — выбрать · Enter — применить · Esc — отменить · F4 — открыть список".into();
    }

    pub(super) fn choose_dropdown(&mut self, index: usize) {
        if self.dropdown_is_open(DropdownKind::AssetLibrary) { self.choose_asset(index); return; }
        if self.dropdown_is_open(DropdownKind::MaterialLibrary) { self.choose_material_dropdown(index); return; }
        let Some(dropdown) = self.dropdown.take() else {
            return;
        };
        if !self.dropdown_valid(&dropdown)
            || index >= labels(dropdown.kind, &self.editor.project).len()
        {
            return;
        }
        self.dropdown_focus = Some(dropdown.kind);
        if dropdown.kind == DropdownKind::SceneFilter {
            self.scene_tools.hidden_types ^= 1 << index;
            self.joints_only = false;
            self.scene_scroll = 0;
            self.dropdown = Some(dropdown);
            return;
        }
        if dropdown.kind == DropdownKind::Accent {
            self.preferences.accent = crate::ui::Accent::ALL[index];
            self.persist_preferences();
            return;
        }
        let action = match dropdown.kind {
            DropdownKind::CameraEffectAdd => (index<7).then_some(Action::CameraTool(crate::ui::CameraTool::AddEffect(index+4))),
            DropdownKind::ArrayMode(node,id)=>(index<3).then_some(Action::Modifier(crate::ui::ModifierAction::ArrayMode(node,id,index as u8))),
            DropdownKind::ArrayCurve(node,id)=>match &dropdown.target {Target::Array {curves,..}=> {
                if index==0 {Some(Action::Modifier(crate::ui::ModifierAction::Curve(node,id,None)))}else{curves.get(index-1).map(|target|Action::Modifier(crate::ui::ModifierAction::Curve(node,id,Some(*target))))}
            },_=>None},
            DropdownKind::ModifierAdd => poseforge_core::ModifierKind::ALL.get(index).map(|k|Action::Modifier(crate::ui::ModifierAction::Add(*k))),
            DropdownKind::ModifierTarget(copy,_) => match &dropdown.target {
                Target::Modifiers {node,modifier:Some(id),choices} => choices.get(index).map(|target|Action::Modifier(crate::ui::ModifierAction::Transfer(node.id,*id,*target,copy))),
                _=>None,
            },
            DropdownKind::BonePreset => poseforge_core::BoneDisplayPreset::ALL.get(index).map(|p| Action::BoneDisplay(crate::ui::BoneDisplayAction::Preset(*p))),
            DropdownKind::SnapMode=>Some(Action::Author(crate::ui::AuthorAction::SnapMode(index as u8))),
            DropdownKind::KnifeAngle=>Some(Action::Author(crate::ui::AuthorAction::KnifeAngle(index as u8))),
            DropdownKind::RepairKind=>Some(Action::Author(crate::ui::AuthorAction::RepairMode(index as u8))),
            DropdownKind::Alignment => Some(Action::Shape(crate::ui::ShapeAction::AlignmentMode(index as u8))),
            DropdownKind::BoneShape => poseforge_core::BoneShape::ALL.get(index).map(|s| Action::BoneDisplay(crate::ui::BoneDisplayAction::BodyShape(*s))),
            DropdownKind::JointShape => poseforge_core::JointShape::ALL.get(index).map(|s| Action::BoneDisplay(crate::ui::BoneDisplayAction::JointShape(*s))),
            DropdownKind::RigSkeleton | DropdownKind::RigMesh => match &dropdown.target {
                Target::RigReference { reference, choices, .. } => choices.get(index).map(|id| Action::RigReferenceSet(*reference, *id)),
                _ => None,
            },
            DropdownKind::CurvePath => self.path_curve_choices().get(index).copied().map(|id| Action::Path(crate::ui::PathAction::Bind(id))),
            DropdownKind::MaterialLibrary | DropdownKind::AssetLibrary => unreachable!(),
            DropdownKind::Accent => unreachable!(),
            DropdownKind::SceneFilter => unreachable!(),
            DropdownKind::Collections => Some(Action::SceneTool(
                crate::ui::SceneTool::AssignCollection(if index == 0 {
                    None
                } else {
                    self.editor.project.collections.get(index - 1).map(|c| c.id)
                }),
            )),
            DropdownKind::SceneActions => {
                use crate::ui::SceneTool as S;
                match index {
                    0 => Some(Action::SceneTool(S::SelectAll)),
                    1 => Some(Action::SceneTool(S::Clear)),
                    2 => Some(Action::SceneTool(S::Isolate)),
                    3 => Some(Action::SceneTool(S::NewCollection)),
                    4 => Some(Action::DropdownOpen(DropdownKind::Collections)),
                    5 => self
                        .scene_tools
                        .collection
                        .map(|id| Action::SceneTool(S::CollectionName(id))),
                    6 => self
                        .scene_tools
                        .collection
                        .map(|id| Action::SceneTool(S::CollectionRender(id))),
                    7 => self
                        .scene_tools
                        .collection
                        .map(|id| Action::SceneTool(S::CollectionDelete(id))),
                    8 => Some(Action::FrameSelected),
                    9 => Some(Action::Duplicate),
                    _ => None,
                }
            }
            DropdownKind::SavedViews => self
                .editor
                .project
                .saved_views
                .get(index)
                .map(|_| Action::CameraTool(crate::ui::CameraTool::RecallView(index))),
            DropdownKind::ReflectionSamples => Some(Action::World(
                crate::ui::WorldAction::ReflectionSamples([1, 4, 8, 16][index]),
            )),
            DropdownKind::AreaShadowSamples => Some(Action::World(
                crate::ui::WorldAction::ShadowSamples([4, 16, 64][index]),
            )),
            DropdownKind::Background => Some(Action::Studio(crate::ui::StudioAction::Background(
                index as u8,
            ))),
            DropdownKind::MeshCreate => {
                poseforge_core::PrimitiveKind::MESH_MENU.get(index).map(|&(kind,_)|Action::Studio(crate::ui::StudioAction::Mesh(kind)))
            }
            DropdownKind::EmptyCreate=>Some(Action::Helper(crate::ui::HelperAction::Empty(index as u8))),
            DropdownKind::EmptyShape=>Some(Action::Helper(crate::ui::HelperAction::Shape(index as u8))),
            DropdownKind::ForceCreate=>Some(Action::Helper(crate::ui::HelperAction::CreateField(index as u8))),
            DropdownKind::ForceKind=>Some(Action::Helper(crate::ui::HelperAction::Kind(index as u8))),
            DropdownKind::FxCreate=>Some(Action::Fx(crate::ui::FxAction::Create(index as u8))),
            DropdownKind::FxKind=>Some(Action::Fx(crate::ui::FxAction::Kind(index as u8))),
            DropdownKind::FxPreset(_)=>Some(Action::Fx(crate::ui::FxAction::Preset(index as u8))),
            DropdownKind::FxEmission=>Some(Action::Fx(crate::ui::FxAction::Emission(index as u8))),
            DropdownKind::FxQuality=>Some(Action::Fx(crate::ui::FxAction::Quality(index as u8))),
            DropdownKind::TextFont => Some(Action::Text(crate::ui::TextAction::Font(index as u8))),
            DropdownKind::CurveCreate => Some(Action::Curve(if index == poseforge_core::CurvePreset::BASIC.len() {
                CurveAction::Draw
            } else {
                CurveAction::Create(index as u8)
            })),
            DropdownKind::SkeletonCreate => Some(Action::Author(if index == 0 {
                crate::ui::AuthorAction::CreateBone
            } else {
                crate::ui::AuthorAction::CreateSkeleton(index == 2)
            })),
            DropdownKind::PhysicsType => Some(if index == 4 { Action::Destruction(crate::ui::DestructionAction::Attach) }
                else { Action::Studio(crate::ui::StudioAction::Physics(index as u8)) }),
            DropdownKind::FluidSource => Some(Action::Studio(crate::ui::StudioAction::FluidFill(
                index as u8,
            ))),
            DropdownKind::RenderQuality => Some(Action::RenderQuality(index as u8)),
            DropdownKind::Antialiasing => {
                Some(Action::RenderAntialiasing(if index == 0 { 1 } else { 4 }))
            }
            DropdownKind::ToneMapping => Some(Action::RenderToneMapping(index as u8)),
            DropdownKind::ShadowResolution => {
                Some(Action::RenderShadowResolution([512, 1024, 2048][index]))
            }
            DropdownKind::LightingPreset => Some(Action::LightingPreset(index as u8)),
            DropdownKind::LightCreate => Some(Action::SurfaceAddLight(index)),
            DropdownKind::MaterialCreate => Some(Action::SurfaceCreateMaterialPreset(index)),
            DropdownKind::MaterialCategory => {
                self.preferences.material_category = if index == 0 {
                    String::new()
                } else {
                    labels(dropdown.kind, &self.editor.project)[index].clone()
                };
                self.surface.material_offset = 0;
                self.persist_preferences();
                None
            }
            DropdownKind::MaterialActions => Some(match index {
                0 => Action::SurfaceUniqueMaterial,
                1 => Action::SurfaceMaterialName,
                2 => Action::Node(crate::ui::NodeAction::Open),
                3 => Action::SurfacePruneMaterials,
                4 => Action::MaterialFavorite,
                5 => Action::MaterialCategory,
                6 => Action::MaterialTags,
                7 => Action::MaterialLibraryToggle,
                8 => Action::MaterialLayerMove(1),
                9 => Action::MaterialLayerMove(-1),
                10 => Action::DropdownOpen(DropdownKind::MaterialCreate),
                _ => Action::MaterialApplyFaces,
            }),
            DropdownKind::MaterialAlpha => {
                let Target::Material { material, .. } = dropdown.target else {
                    return;
                };
                let mode = [AlphaMode::Opaque, AlphaMode::Mask, AlphaMode::Blend][index];
                let result = self.editor.transact(
                    "Режим прозрачности материала",
                    |p| {
                        if let Some(m) = p.materials.iter_mut().find(|m| m.id == material.id) {
                            m.alpha_mode = mode;
                        }
                    },
                );
                self.result(result);
                None
            }
            DropdownKind::LightKind => {
                let Target::Light(light_index, _) = dropdown.target else {
                    return;
                };
                let kind = [
                    LightKind::Directional,
                    LightKind::Point,
                    LightKind::Spot,
                    LightKind::Area,
                ][index];
                let result = self.editor.transact("Тип источника света", |p| {
                    p.lights[light_index].kind = kind
                });
                if self.result(result).is_some()
                    && kind == LightKind::Point
                    && self.active_light_index() == Some(light_index)
                {
                    self.tool = Tool::Move;
                }
                None
            }
        };
        if let Some(action) = action {
            self.action(action);
        }
        if dropdown.kind != DropdownKind::CameraEffectAdd {
            self.dropdown_focus = Some(dropdown.kind);
        }
        self.redraw();
    }

    pub(super) fn draw_dropdown(&mut self, ui: &mut Ui) {
        if self.dropdown_is_open(DropdownKind::AssetLibrary) { self.draw_asset_popup(ui); return; }
        if self.dropdown_is_open(DropdownKind::MaterialLibrary) { self.draw_material_dropdown(ui); return; }
        let Some(dropdown) = self.dropdown.clone() else {
            return;
        };
        if !self.dropdown_valid(&dropdown)
            || (!matches!(
                dropdown.kind,
                DropdownKind::SavedViews | DropdownKind::Collections | DropdownKind::MaterialCreate
            ) && !ui
                .hits
                .iter()
                .any(|hit| hit.action == Action::DropdownOpen(dropdown.kind)))
        {
            self.dropdown = None;
            return;
        }
        let items = labels(dropdown.kind, &self.editor.project);
        let panel = popup_rect(dropdown.anchor, [self.width, self.height], items.len());
        let [x, y, width, height] = panel;
        let p = ui.palette();
        let physical = panel.map(|v| v * ui.scale);
        ui.texts.retain(|text| {
            let w = text.text.chars().count() as f32 * text.size;
            text.x >= physical[0] + physical[2]
                || text.x + w <= physical[0]
                || text.y >= physical[1] + physical[3]
                || text.y + text.size * 1.5 <= physical[1]
        });
        ui.hits.clear();
        ui.hits.push(Hit {
            rect: [0., 0., self.width, self.height],
            action: Action::DropdownDismiss,
        });
        ui.surface([x + 2., y + 3., width, height], [0., 0., 0., 0.16]);
        ui.outlined_surface(panel, p.panel, p.border);
        let selected = self.dropdown_selected(dropdown.kind);
        let count = items.len();
        let page = ((height - 12.) / 32.).floor().max(1.) as usize;
        let offset = dropdown.offset.min(count.saturating_sub(page));
        for (index, label) in items.into_iter().enumerate().skip(offset).take(page) {
            let rect = [
                x + 6.,
                y + 6. + (index - offset) as f32 * 32.,
                width - if count > page { 26. } else { 12. },
                30.,
            ];
            if matches!(&dropdown.target, Target::RigReference { choices, .. } if choices.is_empty()) {
                ui.fitted_label([rect[0]+8.,rect[1]+7.,rect[2]-16.,16.],&label,12.,p.muted);
                continue;
            }
            ui.menu_row(
                rect,
                Action::DropdownChoose(index),
                dropdown.candidate == index,
            );
            let checked = if dropdown.kind == DropdownKind::SceneFilter {
                ui.surface_outline([rect[0] + 5., rect[1] + 6., 18., 18.], p.border);
                self.scene_tools.hidden_types & (1 << index) == 0
            } else if dropdown.kind==DropdownKind::CameraEffectAdd {
                self.selected_camera_effects().is_some_and(|(_,effects)|effects.contains(index+4))
            } else {
                selected == Some(index)
            };
            if checked {
                let check_x = if item_icon(dropdown.kind, index).is_some()
                    || dropdown.kind == DropdownKind::Accent
                {
                    rect[0] + rect[2] - 22.
                } else {
                    rect[0] + 6.
                };
                Icon::Check.draw(ui, [check_x, rect[1] + 7., 16., 16.], p.accent);
            }
            if dropdown.kind == DropdownKind::Accent {
                ui.rounded_rect(
                    [rect[0] + 7., rect[1] + 7., 16., 16.],
                    crate::ui::Accent::ALL[index].palette(ui.theme).accent,
                    8.,
                );
            }
            if let Some(icon) = item_icon(dropdown.kind, index) {
                icon.draw(
                    ui,
                    [rect[0] + 6., rect[1] + 6., 18., 18.],
                    if dropdown.candidate == index {
                        p.accent
                    } else {
                        p.muted
                    },
                );
            }
            ui.fitted_label(
                [rect[0] + 28., rect[1] + 7., rect[2] - 36., 16.],
                &label,
                12.,
                p.text,
            );
        }
        if count > page {
            ui.vertical_scrollbar(
                [x + width - 17., y + 6., 12., height - 12.],
                offset as f32 / (count - page) as f32,
                page as f32 / count as f32,
                Action::ScrollPanel(2),
            );
        }
    }

    pub(super) fn dropdown_scroll(&mut self, delta: f32) -> bool {
        if self.dropdown_is_open(DropdownKind::AssetLibrary) { return self.asset_popup_scroll(delta); }
        if self.dropdown_is_open(DropdownKind::MaterialLibrary) { return self.material_dropdown_scroll(delta); }
        let Some(d) = &mut self.dropdown else {
            return false;
        };
        let count = labels(d.kind, &self.editor.project).len();
        let rect = popup_rect(d.anchor, [self.width, self.height], count);
        let page = ((rect[3] - 12.) / 32.).floor().max(1.) as usize;
        if delta.is_finite() && crate::ui::contains(rect, self.mouse) {
            d.offset = (d.offset as isize - (delta * 2.).round() as isize)
                .clamp(0, count.saturating_sub(page) as isize) as usize;
            self.redraw();
        }
        true
    }
    pub(super) fn dropdown_pointer_up(&mut self) {
        self.asset_popup_pointer_up();
        if let Some(d) = &mut self.dropdown {
            d.dragging = false;
        }
    }
    pub(super) fn dropdown_pointer_down(&mut self, button: MouseButton) -> bool {
        if self.dropdown_is_open(DropdownKind::AssetLibrary) { return self.asset_popup_pointer_down(button); }
        if self.dropdown_is_open(DropdownKind::MaterialLibrary) { return self.material_dropdown_pointer_down(button); }
        if self.dropdown.is_none() {
            return false;
        }
        if button == MouseButton::Left {
            match self.ui.hit(self.mouse) {
                Some(Action::ScrollPanel(2)) => {
                    if let Some(d) = &mut self.dropdown {
                        d.dragging = true;
                    }
                    self.dropdown_pointer_move();
                }
                Some(Action::DropdownChoose(index)) => self.choose_dropdown(index),
                _ => self.dropdown = None,
            }
        } else {
            self.dropdown = None;
        }
        self.redraw();
        true
    }

    pub(super) fn dropdown_pointer_move(&mut self) {
        if self.dropdown_is_open(DropdownKind::AssetLibrary) { self.asset_popup_pointer_move(); return; }
        if self.dropdown_is_open(DropdownKind::MaterialLibrary) { self.material_dropdown_pointer_move(); return; }
        if let Some(d) = &mut self.dropdown {
            if d.dragging {
                let count = labels(d.kind, &self.editor.project).len();
                let r = popup_rect(d.anchor, [self.width, self.height], count);
                let page = ((r[3] - 12.) / 32.).floor().max(1.) as usize;
                let h = (r[3] - 12.) * page as f32 / count as f32;
                let ratio = ((self.mouse[1] - r[1] - 6. - h * 0.5) / (r[3] - 12. - h).max(1.))
                    .clamp(0., 1.);
                d.offset = (ratio * count.saturating_sub(page) as f32).round() as usize;
                return;
            }
        }
        if self.dropdown.is_some() {
            if let Some(Action::DropdownChoose(index)) = self.ui.hit(self.mouse) {
                if let Some(dropdown) = self.dropdown.as_mut() {
                    dropdown.candidate = index;
                }
            }
        }
    }

    pub(super) fn dropdown_key(&mut self, key: &Key, shift: bool, ctrl: bool, alt: bool) -> bool {
        if self.dropdown_is_open(DropdownKind::AssetLibrary) { return self.asset_popup_key(key, shift); }
        if self.dropdown_is_open(DropdownKind::MaterialLibrary) { return self.material_dropdown_key(key); }
        if let Some(dropdown) = self.dropdown.as_mut() {
            let count = labels(dropdown.kind, &self.editor.project).len();
            match key {
                Key::Named(NamedKey::ArrowDown) => {
                    dropdown.candidate = (dropdown.candidate + 1) % count
                }
                Key::Named(NamedKey::ArrowUp) => {
                    dropdown.candidate = (dropdown.candidate + count - 1) % count
                }
                Key::Named(NamedKey::Home | NamedKey::PageUp) => dropdown.candidate = 0,
                Key::Named(NamedKey::End | NamedKey::PageDown) => dropdown.candidate = count - 1,
                Key::Named(NamedKey::Enter | NamedKey::Space) => {
                    let candidate = dropdown.candidate;
                    self.choose_dropdown(candidate);
                }
                Key::Named(NamedKey::Escape | NamedKey::Tab | NamedKey::F4) => self.dropdown = None,
                _ => {}
            }
            if let Some(d) = &mut self.dropdown {
                let rect = popup_rect(
                    d.anchor,
                    [self.width, self.height],
                    labels(d.kind, &self.editor.project).len(),
                );
                let page = ((rect[3] - 12.) / 32.).floor().max(1.) as usize;
                if d.candidate < d.offset {
                    d.offset = d.candidate;
                } else if d.candidate >= d.offset + page {
                    d.offset = d.candidate + 1 - page;
                }
            }
            self.redraw();
            return true;
        }
        if self.has_modal() || self.input.is_some() || self.drag.is_some() || self.job.is_some() {
            return false;
        }
        let visible: Vec<_> = self
            .ui
            .hits
            .iter()
            .filter_map(|hit| match hit.action {
                Action::DropdownOpen(kind) => Some(kind),
                _ => None,
            })
            .collect();
        if visible.is_empty() {
            self.dropdown_focus = None;
            return false;
        }
        if key == &Key::Named(NamedKey::Tab) && self.dropdown_focus.is_some() {
            let current = visible
                .iter()
                .position(|kind| Some(*kind) == self.dropdown_focus)
                .unwrap_or(0);
            self.dropdown_focus = Some(
                visible[(current + if shift { visible.len() - 1 } else { 1 }) % visible.len()],
            );
            self.redraw();
            return true;
        }
        let focused = self.dropdown_focus.filter(|kind| visible.contains(kind));
        let open = key == &Key::Named(NamedKey::F4)
            || (!ctrl
                && focused.is_some()
                && ((alt && key == &Key::Named(NamedKey::ArrowDown))
                    || matches!(key, Key::Named(NamedKey::Enter | NamedKey::Space))));
        if open {
            let hovered = match self.ui.hit(self.mouse) {
                Some(Action::DropdownOpen(kind)) => Some(kind),
                _ => None,
            };
            let inspector = self.ui.hits.iter().find_map(|h| match h.action {
                Action::DropdownOpen(kind)
                    if self.preferences.right_panel && h.rect[0] >= self.right_panel_rect()[0] =>
                {
                    Some(kind)
                }
                _ => None,
            });
            self.open_dropdown(focused.or(hovered).or(inspector).unwrap_or(visible[0]));
            self.redraw();
        }
        open
    }
}

impl App {
    pub(super) fn material_dropdown_active(&self) -> bool {
        self.dropdown.as_ref().is_some_and(|d| d.kind == DropdownKind::MaterialLibrary && self.dropdown_valid(d))
    }
    pub(super) fn close_material_dropdown(&mut self) {
        if !self.dropdown_is_open(DropdownKind::MaterialLibrary) { return; }
        self.dropdown = None;
        if self.input.as_ref().is_some_and(|i| i.purpose == Action::MaterialSearch) { self.end_input(); }
        self.dropdown_focus = Some(DropdownKind::MaterialLibrary);
    }
    pub(super) fn material_categories(&self) -> Vec<Option<String>> {
        let categories: std::collections::BTreeSet<_> = self.editor.project.materials.iter()
            .filter(|m| m.in_library).map(|m| m.category.clone())
            .chain(MaterialPreset::ALL.into_iter().map(|preset| preset_category(preset).to_string()))
            .collect();
        std::iter::once(None).chain(categories.into_iter().map(Some)).collect()
    }
    fn refresh_material_dropdown(&mut self) {
        let categories = self.material_categories();
        if let Some(d) = &mut self.dropdown {
            if !categories.contains(&d.category) { d.category = None; }
        }
        let category = self.dropdown.as_ref().and_then(|d| d.category.as_deref());
        let query = self.input.as_ref().filter(|i| i.purpose == Action::MaterialSearch)
            .map_or(self.surface.material_query.as_str(), |i| i.value.as_str()).trim().to_lowercase();
        let mut entries: Vec<_> = MaterialPreset::ALL.into_iter().enumerate().filter(|(_, preset)| {
            !self.preferences.material_favorites
                && category.is_none_or(|c| c == preset_category(*preset))
                && (query.is_empty() || format!("{} {} готовый готовые шаблон шаблоны", preset.label(), preset_category(*preset)).to_lowercase().contains(&query))
        }).map(|(index, _)| MaterialLibraryEntry::Preset(index)).collect();
        entries.extend(self.editor.project.materials.iter().filter(|m| {
            m.in_library && category.is_none_or(|c| c == m.category)
                && (!self.preferences.material_favorites || m.favorite)
                && (query.is_empty() || format!("{} {} {}", m.name, m.category, m.tags.join(" ")).to_lowercase().contains(&query))
        }).map(|m| MaterialLibraryEntry::Saved(m.id)));
        let changed = self.dropdown.as_ref().is_some_and(|d| d.entries != entries || d.query != query);
        if let Some(d) = &mut self.dropdown {
            if changed {
                let previous = d.entries.get(d.candidate).copied();
                d.candidate = previous.and_then(|entry| entries.iter().position(|item| *item == entry)).unwrap_or(0);
                d.entries = entries; d.query = query;
            }
        }
        if changed {
            if let Some(d) = self.dropdown.as_ref() {
                let offset = d.candidate.saturating_sub(self.material_dropdown_page(d) - 1);
                self.dropdown.as_mut().unwrap().offset = offset;
            }
        }
    }
    fn material_dropdown_rect(&self, d: &Dropdown) -> [f32; 4] {
        let width = 480_f32.min((self.width - 16.).max(80.));
        let height = (84. + d.entries.len().clamp(4, 6) as f32 * 40.).min((self.height - 16.).max(124.));
        let x = (d.anchor[0] + d.anchor[2] - width).clamp(8., (self.width - width - 8.).max(8.));
        let below = d.anchor[1] + d.anchor[3] + 4.;
        let y = if below + height <= self.height - 8. { below } else { d.anchor[1] - height - 4. }
            .clamp(8., (self.height - height - 8.).max(8.));
        [x, y, width, height]
    }
    fn material_dropdown_page(&self, d: &Dropdown) -> usize {
        ((self.material_dropdown_rect(d)[3] - 84.) / 40.).floor().max(1.) as usize
    }
    fn material_dropdown_offset(&self, d: &Dropdown) -> usize {
        d.offset.min(d.entries.len().saturating_sub(self.material_dropdown_page(d)))
    }
    pub(super) fn material_popup_visible_ids(&self) -> Vec<u64> {
        self.dropdown.as_ref().filter(|d| d.kind == DropdownKind::MaterialLibrary)
            .map(|d| d.entries.iter().skip(self.material_dropdown_offset(d)).take(self.material_dropdown_page(d))
                .filter_map(|entry| if let MaterialLibraryEntry::Saved(id) = entry { Some(*id) } else { None }).collect()).unwrap_or_default()
    }
    pub(super) fn material_popup_visible_presets(&self) -> Vec<usize> {
        self.dropdown.as_ref().filter(|d| d.kind == DropdownKind::MaterialLibrary)
            .map(|d| d.entries.iter().skip(self.material_dropdown_offset(d)).take(self.material_dropdown_page(d))
                .filter_map(|entry| if let MaterialLibraryEntry::Preset(index) = entry { Some(*index) } else { None }).collect()).unwrap_or_default()
    }
    fn choose_material_dropdown(&mut self, index: usize) {
        let Some(d) = self.dropdown.clone() else { return; };
        let entry = d.entries.get(index).copied();
        let valid = self.dropdown_valid(&d);
        self.close_material_dropdown();
        if !valid { return; }
        let Some(entry) = entry else { return; };
        if let MaterialLibraryEntry::Saved(id) = entry {
            if self.editor.project.material(id).is_none() { return; }
        }
        self.add_from_material_library(entry.action());
    }
    fn draw_material_dropdown(&mut self, ui: &mut Ui) {
        self.refresh_material_dropdown();
        let Some(mut d) = self.dropdown.clone() else { return; };
        let anchor = ui.hits.iter().rev().find(|h| h.action == Action::DropdownOpen(DropdownKind::MaterialLibrary)).map(|h| h.rect);
        if !self.dropdown_valid(&d) || anchor.is_none() { self.close_material_dropdown(); return; }
        d.anchor = anchor.unwrap();
        self.dropdown.as_mut().unwrap().anchor = d.anchor;
        let [x, y, width, height] = self.material_dropdown_rect(&d);
        let panel = [x, y, width, height]; let p = ui.palette();
        let physical = panel.map(|v| v * ui.scale);
        ui.texts.retain(|text| {
            let w = text.text.chars().count() as f32 * text.size;
            text.x >= physical[0] + physical[2] || text.x + w <= physical[0]
                || text.y >= physical[1] + physical[3] || text.y + text.size * 1.5 <= physical[1]
        });
        ui.hits.clear();
        ui.hits.push(Hit { rect: [0., 0., self.width, self.height], action: Action::DropdownDismiss });
        ui.surface([x + 2., y + 3., width, height], [0., 0., 0., 0.16]);
        ui.outlined_surface(panel, p.panel, p.border);
        Icon::Search.draw(ui, [x + 10., y + 14., 18., 18.], p.muted);
        let query = self.input.as_ref().filter(|i| i.purpose == Action::MaterialSearch)
            .map_or(self.surface.material_query.as_str(), |i| i.value.as_str());
        ui.field([x + 34., y + 8., width - 42., 30.], if query.is_empty() { "Поиск материалов" } else { query }, Action::MaterialSearch);
        ui.checkbox([x + 8., y + 44., width - 80., 24.], "Избранные", Action::MaterialFavorites, self.preferences.material_favorites);
        ui.fitted_label([x + width - 66., y + 49., 56., 16.], &d.entries.len().to_string(), 11., p.muted);
        let page = self.material_dropdown_page(&d); let offset = self.material_dropdown_offset(&d);
        let selected = self.selected_material_id();
        let sidebar = (width * 0.3).min(144.);
        ui.rect([x + sidebar, y + 76., 1., height - 84.], p.border);
        let categories = self.material_categories();
        let category_page = ((height - 84.) / 30.).floor().max(1.) as usize;
        let category_offset = d.category_offset.min(categories.len().saturating_sub(category_page));
        for (index, category) in categories.iter().enumerate().skip(category_offset).take(category_page) {
            let r = [x + 6., y + 76. + (index - category_offset) as f32 * 30., sidebar - 12., 28.];
            ui.menu_row(r, Action::MaterialLibraryCategory(index), d.category == *category);
            let label = category.as_deref().map_or("Все", |name| if name.is_empty() { "Без категории" } else { name });
            ui.fitted_label([r[0] + 6., r[1] + 6., r[2] - 12., 18.], label, 11., p.text);
        }
        for (index, entry) in d.entries.iter().enumerate().skip(offset).take(page) {
            let preset;
            let m = match *entry {
                MaterialLibraryEntry::Saved(id) => {
                    let Some(material) = self.editor.project.material(id) else { continue; };
                    material
                }
                MaterialLibraryEntry::Preset(index) => {
                    let Some(source) = MaterialPreset::ALL.get(index) else { continue; };
                    preset = source.create();
                    &preset
                }
            };
            let rect = [x + sidebar + 6., y + 76. + (index - offset) as f32 * 40., width - sidebar - if d.entries.len() > page { 26. } else { 12. }, 38.];
            ui.menu_row(rect, Action::DropdownChoose(index), d.candidate == index);
            self.material_previews.draw(ui, [rect[0] + 4., rect[1] + 3., 32., 32.], m, &self.editor.project.textures);
            if let MaterialLibraryEntry::Saved(id) = *entry {
                ui.fitted_label([rect[0] + 44., rect[1] + 11., rect[2] - 72., 16.], &m.name, 12., p.text);
                if selected == Some(id) { Icon::Check.draw(ui, [rect[0] + 26., rect[1] + 22., 12., 12.], p.accent); }
                ui.scene_close_button([rect[0] + rect[2] - 25., rect[1] + 7., 24., 24.], Action::MaterialLibraryRemove(id));
            } else {
                ui.fitted_label([rect[0] + 44., rect[1] + 3., rect[2] - 50., 17.], &m.name, 12., p.text);
                ui.fitted_label([rect[0] + 44., rect[1] + 21., rect[2] - 50., 14.], "Готовый материал", 10., p.muted);
            }
        }
        if d.entries.is_empty() { ui.fitted_label([x + sidebar + 12., y + 86., width - sidebar - 24., 40.], "Материалы не найдены", 12., p.muted); }
        if d.entries.len() > page {
            ui.vertical_scrollbar([x + width - 17., y + 76., 12., page as f32 * 40.], offset as f32 / (d.entries.len() - page) as f32,
                page as f32 / d.entries.len() as f32, Action::ScrollPanel(2));
        }
    }
    fn material_dropdown_scroll(&mut self, delta: f32) -> bool {
        let Some(d) = self.dropdown.as_ref() else { return false; };
        let rect = self.material_dropdown_rect(d); let page = self.material_dropdown_page(d);
        if delta.is_finite() && crate::ui::contains(rect, self.mouse) {
            if self.mouse[0] < rect[0] + (rect[2] * 0.3).min(144.) && self.mouse[1] > rect[1] + 76. {
                let count = self.material_categories().len();
                let page = ((rect[3] - 84.) / 30.).floor().max(1.) as usize;
                let d = self.dropdown.as_mut().unwrap();
                d.category_offset = (d.category_offset as isize - delta.round() as isize).clamp(0, count.saturating_sub(page) as isize) as usize;
                self.redraw(); return true;
            }
            let d = self.dropdown.as_mut().unwrap();
            d.offset = (d.offset as isize - (delta * 2.).round() as isize).clamp(0, d.entries.len().saturating_sub(page) as isize) as usize;
        }
        self.redraw(); true
    }
    fn material_dropdown_pointer_down(&mut self, button: MouseButton) -> bool {
        if button != MouseButton::Left { self.close_material_dropdown(); self.redraw(); return true; }
        match self.ui.hit(self.mouse) {
            Some(Action::DropdownChoose(index)) => {
                let entry = self.dropdown.as_ref().and_then(|d|d.entries.get(index)).copied();
                self.close_material_dropdown();
                if let Some(entry) = entry {
                    self.begin_material_drag(&entry.action());
                    if let Some(super::Drag::Material(drag)) = &mut self.drag { drag.add_layer = true; }
                }
            }
            Some(Action::MaterialLibraryCategory(index)) => {
                if let Some(category) = self.material_categories().get(index).cloned() {
                    let d = self.dropdown.as_mut().unwrap(); d.category = category; d.offset = 0; d.candidate = 0;
                    self.refresh_material_dropdown();
                }
            }
            Some(Action::MaterialLibraryRemove(id)) => {
                let result = self.editor.set_material_library(id, false); self.result(result);
                self.refresh_material_dropdown();
            }
            Some(Action::MaterialSearch) => {
                if self.input.is_none() { self.input(Action::MaterialSearch, self.surface.material_query.clone()); }
                if let Some(rect) = self.active_field_rect() {
                    if let Some(input) = &mut self.input { input.click(rect, self.mouse, self.modifiers.shift_key()); self.text_selecting = true; }
                }
            }
            Some(Action::MaterialSearchClear) => {
                self.surface.material_query.clear(); self.input(Action::MaterialSearch, String::new()); self.refresh_material_dropdown();
            }
            Some(Action::MaterialFavorites) => {
                self.preferences.material_favorites = !self.preferences.material_favorites; self.persist_preferences(); self.refresh_material_dropdown();
            }
            Some(Action::ScrollPanel(2)) => { self.dropdown.as_mut().unwrap().dragging = true; self.material_dropdown_pointer_move(); }
            _ => {
                if self.dropdown.as_ref().is_none_or(|d| !crate::ui::contains(self.material_dropdown_rect(d), self.mouse)) { self.close_material_dropdown(); }
            }
        }
        self.redraw(); true
    }
    fn material_dropdown_pointer_move(&mut self) {
        let Some(d) = &self.dropdown else { return; };
        if d.dragging {
            let rect = self.material_dropdown_rect(d); let page = self.material_dropdown_page(d);
            let count = d.entries.len(); let track = page as f32 * 40.;
            let thumb = (track * page as f32 / count.max(1) as f32).max(28.).min(track);
            let ratio = ((self.mouse[1] - rect[1] - 76. - thumb * 0.5) / (track - thumb).max(1.)).clamp(0., 1.);
            self.dropdown.as_mut().unwrap().offset = (ratio * count.saturating_sub(page) as f32).round() as usize;
        } else if let Some(Action::DropdownChoose(index)) = self.ui.hit(self.mouse) {
            self.dropdown.as_mut().unwrap().candidate = index;
        }
    }
    fn material_dropdown_key(&mut self, key: &Key) -> bool {
        if self.input.as_ref().is_some_and(|i| !i.preedit.is_empty()) { return false; }
        self.refresh_material_dropdown();
        match key {
            Key::Named(NamedKey::Escape | NamedKey::Tab | NamedKey::F4) => self.close_material_dropdown(),
            Key::Named(NamedKey::Enter) => {
                if let Some(d) = &self.dropdown { self.choose_material_dropdown(d.candidate); }
            }
            Key::Named(NamedKey::Home | NamedKey::End) if self.input.as_ref().is_none_or(|i| i.value.is_empty()) => {
                let d = self.dropdown.as_ref().unwrap(); let page = self.material_dropdown_page(d);
                let d = self.dropdown.as_mut().unwrap();
                d.candidate = if *key == Key::Named(NamedKey::End) { d.entries.len().saturating_sub(1) } else { 0 };
                d.offset = d.candidate.saturating_sub(page - 1);
            }
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowUp | NamedKey::PageDown | NamedKey::PageUp) => {
                let d = self.dropdown.as_ref().unwrap(); let page = self.material_dropdown_page(d);
                let d = self.dropdown.as_mut().unwrap();
                if !d.entries.is_empty() {
                    let step = if matches!(key, Key::Named(NamedKey::PageDown | NamedKey::PageUp)) { page } else { 1 };
                    d.candidate = if matches!(key, Key::Named(NamedKey::ArrowDown | NamedKey::PageDown)) {
                        (d.candidate + step).min(d.entries.len() - 1)
                    } else { d.candidate.saturating_sub(step) };
                    if d.candidate < d.offset { d.offset = d.candidate; }
                    if d.candidate >= d.offset + page { d.offset = d.candidate + 1 - page; }
                }
            }
            _ => return false, // The ordinary text editor owns typing, clipboard, IME and caret keys.
        }
        self.redraw(); true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui;
    use crate::workspace::Preferences;
    use poseforge_core::{box_mesh, Editor, Node, Project};

    fn app() -> App {
        let mut project = Project::default();
        project.materials.push(Material {
            id: 1,
            ..Material::default()
        });
        project
            .meshes
            .push(box_mesh(2, "Меш", glam::Vec3::ONE, Some(1)));
        let mut node = Node::new(3, "Объект");
        node.mesh = Some(2);
        project.nodes.push(node);
        let mut app = App::new(None);
        app.preferences = Preferences::default();
        app.preferences.right_panel = true;
        app.preferences.left_panel = true;
        app.width = 1000.;
        app.height = 760.;
        app.editor = Editor::new(project);
        app.editor.selected = Some(3);
        app.saved_snapshot = app.editor.project.clone();
        app
    }

    fn open(app: &mut App, kind: DropdownKind) {
        app.surface.tab = match kind {
            DropdownKind::MaterialLibrary | DropdownKind::MaterialAlpha => 1,
            DropdownKind::LightKind => 0,
            _ => 3,
        };
        let mut rect = None;
        for offset in (0..1300).step_by(80) {
            app.inspector_scroll = offset as f32;
            app.ui = app.build_ui();
            rect = app
                .ui
                .hits
                .iter()
                .find(|hit| hit.action == Action::DropdownOpen(kind))
                .map(|hit| hit.rect);
            if rect.is_some() {
                break;
            }
        }
        let rect = rect.expect("dropdown trigger visible in scrolled inspector");
        app.mouse_move([rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5]);
        app.mouse_down(MouseButton::Left);
        app.mouse_up(MouseButton::Left);
        assert!(app.dropdown.is_some());
        app.ui = app.build_ui();
    }

    #[test]
    fn candidates_are_transient_escape_cancels_and_enter_creates_one_undo() {
        let mut app = app();
        let original = app.editor.project.clone();
        open(&mut app, DropdownKind::ToneMapping);
        assert!(app.dropdown_key(&Key::Named(NamedKey::End), false, false, false));
        assert_eq!(app.editor.project, original);
        assert_eq!(app.editor.undo_len(), 0);
        app.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
        assert!(app.dropdown.is_none());
        assert_eq!(app.editor.project, original);
        open(&mut app, DropdownKind::ToneMapping);
        app.dropdown_key(&Key::Named(NamedKey::End), false, false, false);
        app.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false);
        assert_eq!(
            app.editor.project.render_settings.tone_mapping,
            ToneMapping::Linear
        );
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, original);
    }

    #[test]
    fn popup_consumes_pointer_wheel_and_shortcuts_without_editing_underneath() {
        let mut app = app();
        let original = app.editor.project.clone();
        open(&mut app, DropdownKind::RenderQuality);
        let scroll = app.inspector_scroll;
        let distance = app.distance;
        app.mouse = [500., 300.];
        app.scroll(-10.);
        assert_eq!(app.inspector_scroll, scroll);
        assert_eq!(app.distance, distance);
        assert!(app.dropdown_key(&Key::Character("z".into()), false, true, false));
        app.mouse_down(MouseButton::Left);
        assert!(app.dropdown.is_none());
        assert!(app.drag.is_none() && app.input.is_none());
        assert_eq!(app.editor.project, original);
        assert_eq!(app.editor.undo_len(), 0);
    }

    #[test]
    fn material_alpha_menu_commits_explicit_choice_and_rejects_stale_target() {
        let mut app = app();
        open(&mut app, DropdownKind::MaterialAlpha);
        app.choose_dropdown(2);
        assert_eq!(app.editor.project.materials[0].alpha_mode, AlphaMode::Blend);
        assert_eq!(app.editor.undo_len(), 1);
        open(&mut app, DropdownKind::MaterialAlpha);
        app.editor.selected = None;
        let before = app.editor.project.clone();
        app.choose_dropdown(1);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 1);
    }

    #[test]
    fn light_type_and_creation_menus_are_explicit_and_snapshot_guarded() {
        let mut app = app();
        app.action(Action::SurfaceAddLight(1));
        open(&mut app, DropdownKind::LightKind);
        let index = app.surface_light_index().unwrap();
        app.choose_dropdown(3);
        assert_eq!(app.editor.project.lights[index].kind, LightKind::Area);
        open(&mut app, DropdownKind::LightKind);
        app.editor.project.lights[index].name.push_str(" изменён");
        let before = app.editor.project.clone();
        app.choose_dropdown(0);
        assert_eq!(app.editor.project, before);
        app.ui = app.build_ui();
        let trigger = app
            .ui
            .hits
            .iter()
            .rev()
            .find(|h| h.action == Action::DropdownOpen(DropdownKind::LightCreate))
            .unwrap()
            .rect;
        app.mouse = [trigger[0] + trigger[2] * 0.5, trigger[1] + trigger[3] * 0.5];
        app.action(Action::DropdownOpen(DropdownKind::LightCreate));
        assert_eq!(app.dropdown.as_ref().unwrap().anchor, trigger);
        app.action(Action::DropdownDismiss);
        app.preferences.right_panel = false;
        app.ui = app.build_ui();
        app.action(Action::DropdownOpen(DropdownKind::LightCreate));
        app.ui = app.build_ui();
        assert!(app.dropdown.is_some());
        let count = app.editor.project.lights.len();
        app.choose_dropdown(2);
        assert_eq!(app.editor.project.lights.len(), count + 1);
        assert_eq!(
            app.editor.project.lights.last().unwrap().kind,
            LightKind::Spot
        );
    }

    #[test]
    fn keyboard_entry_returned_focus_and_tab_navigation_are_available() {
        let mut app = app();
        app.preferences.left_panel = false;
        app.surface.tab = 3;
        app.dropdown_focus = Some(DropdownKind::RenderQuality);
        app.ui = app.build_ui();
        assert!(app.dropdown_key(&Key::Named(NamedKey::F4), false, false, false));
        assert_eq!(
            app.dropdown.as_ref().unwrap().kind,
            DropdownKind::RenderQuality
        );
        app.dropdown_key(&Key::Named(NamedKey::Escape), false, false, false);
        app.ui = app.build_ui();
        assert!(app.dropdown_key(&Key::Named(NamedKey::Tab), false, false, false));
        assert_eq!(app.dropdown_focus, Some(DropdownKind::Antialiasing));
        assert!(app.dropdown_key(&Key::Named(NamedKey::Enter), false, false, false));
        assert_eq!(
            app.dropdown.as_ref().unwrap().kind,
            DropdownKind::Antialiasing
        );
    }

    #[test]
    fn popups_flip_inside_minimum_window_in_both_themes_and_dpi() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for scale in [1., 2.] {
                let mut app = app();
                app.preferences.theme = theme;
                app.width = 900.;
                app.height = 640.;
                app.scale = scale;
                open(&mut app, DropdownKind::MaterialAlpha);
                for hit in &app.ui.hits {
                    assert!(hit.rect[0] >= 0. && hit.rect[1] >= 0.);
                    assert!(hit.rect[0] + hit.rect[2] <= app.width);
                    assert!(hit.rect[1] + hit.rect[3] <= app.height);
                    assert!(matches!(
                        hit.action,
                        Action::DropdownChoose(_) | Action::DropdownDismiss
                    ));
                }
                assert_eq!(
                    app.ui
                        .hits
                        .iter()
                        .filter(|hit| matches!(hit.action, Action::DropdownChoose(_)))
                        .count(),
                    3
                );
            }
        }
    }

    #[test]
    fn softbox_preset_is_an_undoable_area_lighting_rig() {
        let mut app = app();
        let before = app.editor.project.clone();
        open(&mut app, DropdownKind::LightingPreset);
        app.choose_dropdown(3);
        assert_eq!(
            app.editor
                .project
                .lights
                .iter()
                .filter(|l| l.kind == LightKind::Area)
                .count(),
            2
        );
        app.editor.project.validate().unwrap();
        assert_eq!(app.editor.undo_len(), 1);
        app.action(Action::Undo);
        assert_eq!(app.editor.project, before);
    }
}
