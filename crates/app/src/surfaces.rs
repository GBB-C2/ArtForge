//! Contextual material and light controls. All document edits use editor history.
use super::{App, Tool};
use crate::ui::{Action, DropdownKind, SliderRange, Ui};
use poseforge_core::{AlphaMode, Light, LightKind, Material, Project, MAX_ENABLED_LIGHTS};

pub(super) type MaterialSlots = (
    Option<u64>,
    u64,
    std::sync::Arc<std::collections::BTreeSet<u64>>,
);
#[derive(Default)]
pub(super) struct SurfaceState {
    pub tab: u8,
    pub scroll_offsets: [f32; 15],
    pub(super) mesh: Option<u64>,
    pub(super) selection: Option<u64>,
    pub(super) material_query: String,
    pub(super) material_library: bool,
    pub(super) library_material: Option<u64>,
    pub(super) object_material: Option<(u64, u64)>,
    pub(super) material_slots: std::cell::RefCell<std::collections::BTreeMap<u64, MaterialSlots>>,
    pub(super) material_offset: usize,
    pub(super) material_face_groups: std::cell::RefCell<std::collections::BTreeMap<u64, super::material_users::FaceGroupsCache>>,
    pub(super) expanded_materials: std::collections::BTreeSet<u64>,
    pub(super) material_click: Option<(std::time::Instant, u64, [f32; 2])>,
    pub(super) light: usize,
    pub(super) light_selected: bool,
    pub maps: bool,
    pub media_slot: Option<usize>,
    pub curve_point: u8,
    pub input_error: Option<String>,
}

pub(super) fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
pub(super) fn linear_to_srgb(value: f32) -> f32 {
    let value = value.clamp(0., 1.);
    if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1. / 2.4) - 0.055
    }
}
pub(super) fn color_hex(color: [f32; 3]) -> String {
    let c = color.map(|v| (linear_to_srgb(v) * 255.).round() as u8);
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}
pub(super) fn parse_color(value: &str) -> Result<[f32; 3], String> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(
            "Введите цвет sRGB в формате #RRGGBB. Непрозрачность изменяется отдельно.".into(),
        );
    }
    let mut rgb = [0.; 3];
    for (i, channel) in rgb.iter_mut().enumerate() {
        let value = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| "Некорректный HEX-цвет".to_string())?;
        *channel = srgb_to_linear(value as f32 / 255.);
    }
    Ok(rgb)
}
pub(super) fn emissive_tint(emissive: [f32; 3]) -> [f32; 3] {
    let strength = emissive.into_iter().fold(0., f32::max);
    if strength > 0. {
        emissive.map(|c| c / strength)
    } else {
        [0.; 3]
    }
}
pub(super) fn set_emissive_tint(material: &mut Material, color: [f32; 3]) {
    let old = material.emissive.into_iter().fold(0., f32::max);
    let strength = if old > 0. { old } else { 1. };
    material.emissive = emissive_tint(color).map(|c| c * strength);
}
pub(super) fn number(value: &str, min: f32, max: f32) -> Result<f32, String> {
    let value = value
        .trim()
        .replace(',', ".")
        .parse::<f32>()
        .map_err(|_| "Введите число.".to_string())?;
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(format!("Допустимый диапазон: {min}…{max}."));
    }
    Ok(value)
}
pub(super) fn texture_slot(material: &Material, slot: usize) -> Option<u64> {
    match slot {
        0 => material.base_color_texture,
        1 => material.metallic_roughness_texture,
        2 => material.normal_texture,
        3 => material.occlusion_texture,
        4 => material.emissive_texture,
        _ => None,
    }
}
pub(super) fn set_texture_slot(material: &mut Material, slot: usize, id: Option<u64>) {
    match slot {
        0 => material.base_color_texture = id,
        1 => material.metallic_roughness_texture = id,
        2 => material.normal_texture = id,
        3 => material.occlusion_texture = id,
        4 => material.emissive_texture = id,
        _ => {}
    }
}
pub(super) fn material_number(material: &Material, field: usize) -> f32 {
    match field {
        37..=49 => material.artistic.number(field - 37),
        0 => material.metallic,
        1 => material.roughness,
        2 => material.base_color[3],
        3 => material.normal_scale,
        4 => material.occlusion_strength,
        5 => material.alpha_cutoff,
        6 => material.emissive.into_iter().fold(0., f32::max),
        7 => material.ior,
        8 => material.coat_weight,
        9 => material.coat_roughness,
        10 => material.anisotropy,
        11 => material.anisotropy_rotation,
        12 => material.sheen_weight,
        13 => material.sheen_roughness,
        14..=15 => material.uv_scale[field - 14],
        16..=17 => material.uv_offset[field - 16],
        18 => material.uv_rotation,
        19 => material.npr.bands as f32,
        20 => material.npr.threshold,
        21 => material.npr.softness,
        22 => material.npr.specular_size,
        23 => material.npr.specular_strength,
        24 => material.npr.rim_strength,
        25 => material.npr.rim_width,
        26 => material.npr.ink_width,
        27 => material.npr.palette_strength,
        28 => material.npr.palette_position,
        29 => material.npr.hatch_strength,
        30 => material.npr.hatch_density,
        31 => material.npr.hatch_angle,
        33 => material.npr.light_wrap,
        34 => material.npr.light_color,
        35 => material.npr.halftone_strength,
        36 => material.npr.halftone_density,
        32 => material.npr.hatch_cross,
        _ => 0.,
    }
}
fn set_material_number(material: &mut Material, field: usize, value: f32) {
    match field {
        37..=49 => { material.artistic.set_number(field - 37, value); },
        0 => material.metallic = value,
        1 => material.roughness = value,
        2 => {
            material.base_color[3] = value;
            if value < 1. && material.alpha_mode == AlphaMode::Opaque {
                material.alpha_mode = AlphaMode::Blend;
            }
        }
        3 => material.normal_scale = value,
        4 => material.occlusion_strength = value,
        5 => material.alpha_cutoff = value,
        6 => {
            let old = material.emissive.into_iter().fold(0., f32::max);
            material.emissive = if old > 0. {
                material.emissive.map(|c| (c / old) * value)
            } else {
                [value; 3]
            };
        }
        7 => material.ior = value,
        8 => material.coat_weight = value,
        9 => material.coat_roughness = value,
        10 => material.anisotropy = value,
        11 => material.anisotropy_rotation = value,
        12 => material.sheen_weight = value,
        13 => material.sheen_roughness = value,
        14..=15 => material.uv_scale[field - 14] = value,
        16..=17 => material.uv_offset[field - 16] = value,
        18 => material.uv_rotation = value,
        19 => material.npr.bands = value.round() as u32,
        20 => material.npr.threshold = value,
        21 => material.npr.softness = value,
        22 => material.npr.specular_size = value,
        23 => material.npr.specular_strength = value,
        24 => material.npr.rim_strength = value,
        25 => material.npr.rim_width = value,
        26 => material.npr.ink_width = value,
        27 => material.npr.palette_strength = value,
        28 => material.npr.palette_position = value,
        29 => material.npr.hatch_strength = value,
        30 => material.npr.hatch_density = value,
        31 => material.npr.hatch_angle = value,
        33 => material.npr.light_wrap = value,
        34 => material.npr.light_color = value,
        35 => material.npr.halftone_strength = value,
        36 => material.npr.halftone_density = value,
        32 => material.npr.hatch_cross = value,
        _ => {}
    }
}
pub(super) fn light_number(light: &Light, field: usize) -> f32 {
    match field {
        16 => light.volume.strength,
        0 => light.intensity,
        1..=3 => light.position[field - 1],
        4..=6 => light.direction[field - 4],
        7 => light.range,
        8 => light.inner_cone_angle.to_degrees(),
        9 => light.outer_cone_angle.to_degrees(),
        10..=11 => light.area_size[field - 10],
        12 => light.temperature_kelvin,
        13 => light.area_rotation,
        14 => light.direction.x.atan2(-light.direction.z).to_degrees(),
        15 => light
            .direction
            .normalize_or_zero()
            .y
            .clamp(-1., 1.)
            .asin()
            .to_degrees(),
        _ => 0.,
    }
}
fn set_light_number(light: &mut Light, field: usize, value: f32) {
    if matches!(field, 4..=6 | 14..=15) {
        light.target = None;
    }
    match field {
        16 => light.volume.strength = value,
        0 => light.intensity = value,
        1..=3 => light.position[field - 1] = value,
        4..=6 => light.direction[field - 4] = value,
        7 => light.range = value,
        8 => light.inner_cone_angle = value.to_radians(),
        9 => light.outer_cone_angle = value.to_radians(),
        10..=11 => light.area_size[field - 10] = value,
        12 => light.temperature_kelvin = value,
        13 => light.area_rotation = value,
        14..=15 => {
            let yaw = if field == 14 {
                value
            } else {
                light_number(light, 14)
            }
            .to_radians();
            let pitch = if field == 15 {
                value
            } else {
                light_number(light, 15)
            }
            .to_radians();
            light.direction = glam::Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                -yaw.cos() * pitch.cos(),
            )
            .normalize();
        }
        _ => {}
    }
}
fn kind_label(kind: LightKind) -> &'static str {
    match kind {
        LightKind::Directional => "Солнце",
        LightKind::Point => "Точка",
        LightKind::Spot => "Прожектор",
        LightKind::Area => "Площадной",
    }
}

impl App {
    pub(super) fn surface_mesh_ids(&self) -> Vec<u64> {
        let project = &self.editor.project;
        let Some(root) = self
            .editor
            .selected
            .filter(|id| project.node(*id).is_some_and(|n| n.camera.is_none()))
        else {
            return vec![];
        };
        if self.active_light_index().is_some() {
            return vec![];
        }
        let mut ids = Vec::new();
        // The selected node comes first; children remain available for grouped models.
        if let Some(mesh) = project.node(root).and_then(|n| n.mesh) {
            return vec![mesh];
        }
        for node in &project.nodes {
            if self.is_descendant(node.id, root) {
                if let Some(mesh) = node.mesh.filter(|id| !ids.contains(id)) {
                    ids.push(mesh);
                }
            }
        }
        ids
    }
    pub(super) fn surface_mesh_id(&self) -> Option<u64> {
        let ids = self.surface_mesh_ids();
        (self.surface.selection == self.editor.selected)
            .then_some(self.surface.mesh)
            .flatten()
            .filter(|id| ids.contains(id))
            .or_else(|| ids.first().copied())
    }
    pub(super) fn assigned_material_ids(
        &self,
        mesh: &poseforge_core::Mesh,
    ) -> std::sync::Arc<std::collections::BTreeSet<u64>> {
        let revision = mesh
            .topology
            .as_ref()
            .map_or(0, |t| t.face_materials.revision());
        let mut cache = self.surface.material_slots.borrow_mut();
        if let Some((base, old_revision, ids)) = cache.get(&mesh.id) {
            if *base == mesh.material && *old_revision == revision {
                return ids.clone();
            }
        }
        if cache.len() >= 4096 || cache.len() > self.editor.project.meshes.len() + 16 {
            cache.clear();
        }
        let ids = std::sync::Arc::new(mesh.assigned_materials());
        cache.insert(mesh.id, (mesh.material, revision, ids.clone()));
        ids
    }
    pub(super) fn selected_material_id(&self) -> Option<u64> {
        if self.surface.material_library {
            return self
                .surface
                .library_material
                .filter(|&id| self.editor.project.material(id).is_some())
                .or_else(|| self.editor.project.materials.first().map(|m| m.id));
        }

        if self.active_light_index().is_some() || self.material_locked() {
            return None;
        }
        self.surface_mesh_id()
            .and_then(|id| self.editor.project.mesh(id))
            .and_then(|mesh| {
                self.surface
                    .object_material
                    .filter(|(owner, id)| {
                        *owner == mesh.id && self.material_list_ids().contains(id)
                    })
                    .map(|(_, id)| id)
                    .or_else(|| self.material_list_ids().first().copied())
            })
    }
    pub(super) fn surface_light_index(&self) -> Option<usize> {
        (!self.editor.project.lights.is_empty())
            .then(|| self.surface.light.min(self.editor.project.lights.len() - 1))
            .filter(|i| !self.editor.project.light_locked(*i))
    }
    fn edit_surface(&mut self, label: &str, edit: impl FnOnce(&mut Project)) -> bool {
        let result = self.editor.transact(label, edit);
        // Scalar uniforms and material binding keys are synchronized every frame.
        // Keep immutable GPU textures cached while adjusting appearance.
        let committed = self.result(result).is_some();
        if committed {
            self.status = label.into();
        }
        committed
    }
    pub(super) fn surface_action(&mut self, action: Action) {
        if self.material_action(&action) {
            return;
        }
        if action == Action::SurfaceToggleLightShadow && (self.has_modal() || self.job.is_some()) {
            return;
        }
        if matches!(action, Action::SurfaceRenderNumber(_)) {
            self.scene_settings_action(action);
            return;
        }
        self.surface.input_error = None;
        match action {
            Action::SurfaceTab(tab) => {
                if tab == 5 {
                    self.close_uv_workspace(true);
                    self.show_animation_layers();
                    return;
                }
                self.set_properties_tab(tab);
                if tab == 1 && self.authoring.editing.is_some() {
                    self.authoring_action(crate::ui::AuthorAction::SelectMode(
                        crate::ui::MeshSelection::Face,
                    ));
                }
                self.preferences.right_panel = true;
                if tab == 2 {
                    if let Some(index) = self.surface_light_index() {
                        self.select_light(index);
                    }
                }
                self.persist_preferences();
            }
            Action::SurfaceNextMesh => {
                let meshes = self.surface_mesh_ids();
                let next = self
                    .surface_mesh_id()
                    .and_then(|id| meshes.iter().position(|m| *m == id))
                    .map(|index| (index + 1) % meshes.len());
                self.surface.mesh = next.map(|index| meshes[index]);
                self.surface.selection = self.editor.selected;
            }
            Action::SurfaceMaps => {
                self.surface.maps = !self.surface.maps;
                self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
            }
            Action::SurfaceMaterialColor(emissive) => {
                if let Some(material) = self
                    .selected_material_id()
                    .and_then(|id| self.editor.project.material(id))
                {
                    let rgb = if emissive {
                        emissive_tint(material.emissive)
                    } else {
                        [
                            material.base_color[0],
                            material.base_color[1],
                            material.base_color[2],
                        ]
                    };
                    self.input(action, color_hex(rgb));
                }
            }
            Action::SurfaceMaterialNumber(field) => {
                if let Some(material) = self
                    .selected_material_id()
                    .and_then(|id| self.editor.project.material(id))
                {
                    self.input(action, format!("{:.4}", material_number(material, field)));
                }
            }
            Action::SurfaceNprColor(field) => {
                if let Some(color) = self
                    .selected_material_id()
                    .and_then(|id| self.editor.project.material(id))
                    .and_then(|m| super::npr_controls::npr_color(m, field))
                {
                    self.input(action, color_hex(color));
                }
            }
            Action::SurfaceAttenuationColor => {
                if let Some(m) = self
                    .selected_material_id()
                    .and_then(|id| self.editor.project.material(id))
                {
                    let rgb = m.attenuation_color.unwrap_or([
                        m.base_color[0],
                        m.base_color[1],
                        m.base_color[2],
                    ]);
                    self.input(action, color_hex(rgb));
                }
            }
            Action::SurfaceEmissionVolume => {
                if let Some(id) = self.selected_material_id() {
                    self.edit_surface("Объёмное свечение материала", |project| {
                        let Some(material) = project.materials.iter_mut().find(|m| m.id == id) else { return; };
                        material.emission_volume = !material.emission_volume;
                        if material.emission_volume && project.render_settings.atmosphere.density == 0. {
                            project.render_settings.atmosphere.density = 0.035;
                        }
                    });
                }
            }
            Action::SurfaceDoubleSided => {
                if let Some(id) = self.selected_material_id() {
                    self.edit_surface("Настроить материал", |project| {
                        let Some(material) = project.materials.iter_mut().find(|m| m.id == id)
                        else {
                            return;
                        };
                        material.double_sided = !material.double_sided;
                    });
                }
            }
            Action::SurfaceNextTexture(slot) | Action::SurfaceClearTexture(slot) => {
                if let Some(id) = self.selected_material_id() {
                    self.edit_surface("Карта материала", |project| {
                        let current = project.material(id).and_then(|m| texture_slot(m, slot));
                        let next = if matches!(action, Action::SurfaceClearTexture(_)) {
                            None
                        } else {
                            let index = current
                                .and_then(|id| project.textures.iter().position(|t| t.id == id));
                            project
                                .textures
                                .get(index.map_or(0, |i| i + 1))
                                .map(|t| t.id)
                        };
                        if let Some(material) = project.materials.iter_mut().find(|m| m.id == id) {
                            set_texture_slot(material, slot, next);
                        }
                    });
                }
            }
            Action::SurfaceAddLight(kind) => {
                if self
                    .editor
                    .project
                    .lights
                    .iter()
                    .filter(|l| l.enabled)
                    .count()
                    >= MAX_ENABLED_LIGHTS
                {
                    self.status =
                        "Одновременно доступно восемь источников. Отключите или удалите один."
                            .into();
                } else {
                    let kind = match kind {
                        1 => LightKind::Point,
                        2 => LightKind::Spot,
                        3 => LightKind::Area,
                        _ => LightKind::Directional,
                    };
                    let view = self.view();
                    let (position, direction) = super::light_controls::placement_in_view(&view);
                    let index = self.editor.project.lights.len();
                    let added = self.edit_surface("Добавить свет", |project| {
                        project.lights.push(Light {
                            name: format!("{} {}", kind_label(kind), index + 1),
                            kind,
                            position,
                            direction,
                            intensity: if kind == LightKind::Directional {
                                3.
                            } else {
                                60.
                            },
                            ..Light::default()
                        })
                    });
                    if added {
                        self.select_light(index);
                        self.reveal_scene_light(index);
                    }
                }
            }
            Action::SurfaceDeleteLight => {
                self.solo_light = None;
                self.light_aim = None;
                if let Some(index) = self.surface_light_index() {
                    let deleted = self.edit_surface("Удалить свет", |project| {
                        project.remove_light(index);
                    });
                    if deleted {
                        self.surface.light = index.saturating_sub(1);
                        self.surface.light_selected = false;
                    }
                }
            }
            Action::SurfaceToggleLight
            | Action::SurfaceToggleLightShadow
            | Action::SurfaceLightTemperature
            | Action::SurfaceLightVolume
            | Action::SurfacePointLightAtView
            | Action::SurfaceLightKind
            | Action::SurfaceAimLight => {
                if let Some(index) = self.surface_light_index() {
                    let view = self.view();
                    let (position, direction) = super::light_controls::placement_in_view(&view);
                    let Some(resolved) = self.evaluated_light(index) else { return; };
                    let Some(inverse) = self.light_curve_frame(index).map(|m| m.inverse()) else { return; };
                    let updated = self.edit_surface("Настроить свет", |project| {
                        let light = &mut project.lights[index];
                        match action {
                            Action::SurfaceToggleLight => light.enabled = !light.enabled,
                            Action::SurfaceLightVolume => {
                                light.volume.enabled = !light.volume.enabled;
                                if light.volume.enabled && project.render_settings.atmosphere.density == 0. {
                                    project.render_settings.atmosphere.density = 0.035;
                                }
                            }
                            Action::SurfaceLightTemperature => {
                                light.use_temperature = !light.use_temperature
                            }
                            Action::SurfacePointLightAtView => {
                                if let Some(direction) =
                                    (view.target - resolved.position).try_normalize()
                                {
                                    light.direction = inverse.transform_vector3(direction).normalize_or_zero();
                                    light.target = None;
                                }
                            }
                            Action::SurfaceToggleLightShadow => {
                                light.casts_shadow = !light.casts_shadow
                            }
                            Action::SurfaceLightKind => {
                                light.kind = match light.kind {
                                    LightKind::Directional => LightKind::Point,
                                    LightKind::Point => LightKind::Spot,
                                    LightKind::Spot => LightKind::Area,
                                    LightKind::Area => LightKind::Directional,
                                }
                            }
                            Action::SurfaceAimLight => {
                                light.position = inverse.transform_point3(position);
                                light.direction = inverse.transform_vector3(direction).normalize_or_zero();
                                light.target = None;
                            }
                            _ => {}
                        }
                    });
                    if updated && action == Action::SurfaceAimLight {
                        self.select_light(index);
                    } else if updated
                        && action == Action::SurfaceLightKind
                        && self.active_light_index() == Some(index)
                        && self.editor.project.lights[index].kind == LightKind::Point
                        && self.tool == Tool::Rotate
                    {
                        self.tool = Tool::Move;
                        self.status =
                            "Точечный свет излучает во все стороны. G — перемещение.".into();
                    }
                }
            }
            Action::SurfaceLightName => {
                if let Some(i) = self.surface_light_index() {
                    self.input(action, self.editor.project.lights[i].name.clone());
                }
            }
            Action::SurfaceLightColor | Action::SurfaceLightVolumeColor => {
                if let Some(index) = self.surface_light_index() {
                    let color = if action == Action::SurfaceLightVolumeColor { self.editor.project.lights[index].volume.tint }
                        else { self.editor.project.lights[index].color };
                    self.input(action, color_hex(color));
                }
            }
            Action::SurfaceLightNumber(field) => {
                if let Some(index) = self.surface_light_index() {
                    self.input(
                        action,
                        format!(
                            "{:.4}",
                            light_number(&self.evaluated_light(index).unwrap_or_else(|| self.editor.project.lights[index].clone()), field)
                        ),
                    );
                }
            }
            Action::SurfaceRenderNumber(field) => {
                let settings = self.editor.project.render_settings;
                self.input(
                    action,
                    format!(
                        "{:.3}",
                        if field == 0 {
                            settings.exposure
                        } else {
                            settings.ambient_intensity
                        }
                    ),
                );
            }
            _ => {}
        }
    }
    pub(super) fn surface_confirm(&mut self, action: Action, value: &str) -> Result<bool, String> {
        if matches!(action, Action::SurfaceRenderNumber(_)) {
            return self.scene_settings_confirm(action, value);
        }
        let committed = match action {
            Action::SurfaceMaterialName => {
                let id = self
                    .selected_material_id()
                    .ok_or("Материал недоступен или объект заблокирован.")?;
                let result = self.editor.rename_material(id, value);
                self.result(result).is_some()
            }
            Action::SurfaceMaterialColor(emissive) => {
                let color = parse_color(value)?;
                let id = self
                    .selected_material_id()
                    .ok_or("Материал недоступен или объект заблокирован.")?;
                let material = self.editor.project.material(id).ok_or("Материал удалён.")?;
                let current = if emissive {
                    emissive_tint(material.emissive)
                } else {
                    [
                        material.base_color[0],
                        material.base_color[1],
                        material.base_color[2],
                    ]
                };
                // Re-entering the displayed HEX must not quantize the original
                // float value or normalize an unchanged HDR emissive tint.
                if color_hex(current) == color_hex(color) {
                    return Ok(true);
                }
                self.edit_surface("Цвет материала", |project| {
                    if let Some(m) = project.materials.iter_mut().find(|m| m.id == id) {
                        if emissive {
                            set_emissive_tint(m, color);
                        } else {
                            m.base_color[..3].copy_from_slice(&color);
                        }
                    }
                })
            }
            Action::SurfaceMaterialNumber(field) => {
                let (min, max) = match field {
                    37..=49 => poseforge_core::ArtisticSettings::bounds(field - 37).unwrap(),
                    3 => (-16., 16.),
                    6 => (0., 10000.),
                    7 => (1., 3.),
                    9 => (0.03, 1.),
                    10 => (0., 0.95),
                    11 => (-180., 180.),
                    12 => (0., 1.),
                    13 => (0.07, 1.),
                    14..=15 => (0.001, 1000.),
                    16..=17 => (-1000., 1000.),
                    18 => (-180., 180.),
                    19 => (2., 8.),
                    20 => (0.05, 0.95),
                    21 => (0., 0.25),
                    22 => (0.01, 0.5),
                    23..=24 => (0., 2.),
                    25 => (0.01, 1.),
                    26 => (0., 0.5),
                    27 | 29 | 32 => (0., 1.),
                    28 => (0.05, 0.95),
                    30 | 36 => (1., 200.),
                    33..=35 => (0.,1.),
                    31 => (-180., 180.),
                    0..=2 | 4..=5 | 8 => (0., 1.),
                    _ => return Err("Неизвестный параметр материала.".into()),
                };
                let value = number(value, min, max)?;
                let id = self
                    .selected_material_id()
                    .ok_or("Материал недоступен или объект заблокирован.")?;
                if field == 49 && value.fract() != 0. {
                    return Err("Вариант фактуры должен быть целым: 0–65535.".into());
                }
                if field == 19 && value.fract() != 0. {
                    return Err("Число ступеней должно быть целым: 2–8.".into());
                }
                self.edit_surface("Параметр материала", |project| {
                    if let Some(m) = project.materials.iter_mut().find(|m| m.id == id) {
                        set_material_number(m, field, value);
                    }
                })
            }
            Action::SurfaceNprColor(field) => {
                let color = parse_color(value)?;
                let id = self
                    .selected_material_id()
                    .ok_or("Материал недоступен или объект заблокирован.")?;
                let current = self
                    .editor
                    .project
                    .material(id)
                    .and_then(|m| super::npr_controls::npr_color(m, field))
                    .ok_or("Неизвестный цвет NPR.")?;
                if color_hex(current) == color_hex(color) {
                    return Ok(true);
                }
                self.edit_surface("Цвет NPR", |project| {
                    if let Some(m) = project.materials.iter_mut().find(|m| m.id == id) {
                        super::npr_controls::set_npr_color(m, field, color);
                    }
                })
            }
            Action::SurfaceAttenuationColor => {
                let color = parse_color(value)?;
                let id = self
                    .selected_material_id()
                    .ok_or("Материал недоступен или объект заблокирован.")?;
                self.edit_surface("Цвет поглощения", |project| {
                    if let Some(m) = project.materials.iter_mut().find(|m| m.id == id) {
                        m.attenuation_color = Some(color);
                    }
                })
            }
            Action::SurfaceLightName => {
                let name = value.trim();
                if name.is_empty()
                    || name.chars().count() > 96
                    || name.chars().any(char::is_control)
                {
                    return Err("Название: 1–96 символов, без управляющих знаков.".into());
                }
                let index = self.surface_light_index().ok_or("Источник света удалён.")?;
                self.edit_surface("Название света", |p| {
                    p.lights[index].name = name.into()
                })
            }
            Action::SurfaceLightColor | Action::SurfaceLightVolumeColor => {
                let color = parse_color(value)?;
                let index = self.surface_light_index().ok_or("Источник света удалён.")?;
                if color_hex(if action == Action::SurfaceLightVolumeColor { self.editor.project.lights[index].volume.tint } else { self.editor.project.lights[index].color }) == color_hex(color) {
                    return Ok(true);
                }
                self.edit_surface("Цвет света", |p| {
                    if action == Action::SurfaceLightVolumeColor { p.lights[index].volume.tint = color; }
                    else { p.lights[index].color = color; }
                })
            }
            Action::SurfaceLightNumber(field) => {
                let (min, max) = match field {
                    1..=6 => (-100000., 100000.),
                    8..=9 => (0., 90.),
                    10..=11 => (0.001, 10000.),
                    12 => (1000., 12000.),
                    13..=14 => (-180., 180.),
                    15 => (-90., 90.),
                    16 => (0., 10.),
                    _ => (0., 1000000.),
                };
                let value = number(value, min, max)?;
                let index = self.surface_light_index().ok_or("Источник света удалён.")?;
                if field == 8
                    && value.to_radians() >= self.editor.project.lights[index].outer_cone_angle
                {
                    return Err("Внутренний угол должен быть меньше внешнего.".into());
                }
                if field == 9
                    && value.to_radians() <= self.editor.project.lights[index].inner_cone_angle
                {
                    return Err(
                        "Внешний угол должен быть больше внутреннего и не превышать 90°.".into(),
                    );
                }
                let resolved = self
                    .evaluated_light(index)
                    .ok_or("Источник света удалён.")?;
                let inverse = self.light_curve_frame(index).ok_or("Кривая света недоступна")?.inverse();
                self.edit_surface("Параметр света", |p| {
                    if matches!(field, 1..=6 | 14..=15) {
                        let mut edited = resolved.clone();
                        set_light_number(&mut edited, field, value);
                        if matches!(field, 1..=3) { p.lights[index].position = inverse.transform_point3(edited.position); }
                        else {
                            p.lights[index].direction = inverse.transform_vector3(edited.direction).normalize_or_zero();
                            p.lights[index].target = None;
                        }
                    } else { set_light_number(&mut p.lights[index], field, value); }
                })
            }
            Action::SurfaceRenderNumber(field) => {
                let value = number(value, if field == 0 { -16. } else { 0. }, 16.)?;
                self.edit_surface("Экспозиция и окружение", |p| {
                    if field == 0 {
                        p.render_settings.exposure = value;
                    } else {
                        p.render_settings.ambient_intensity = value;
                    }
                })
            }
            _ => return Ok(false),
        };
        if !committed {
            return Err(self.status.clone());
        }
        Ok(true)
    }

    pub(super) fn draw_surface_panel(&self, u: &mut Ui, rx: f32, rw: f32) {
        if self.surface.tab == 1 {
            self.draw_material_panel(u, rx, rw);
            return;
        }
        let p = u.palette();
        let x = rx + 16.;
        let width = rw - 32.;
        u.label(x, 174., "СВЕТ", 12., p.muted);
        if let Some(index) = self.surface_light_index() {
            let Some(resolved) = self.evaluated_light(index) else {
                return;
            };
            let light = &resolved;
            u.field([x, 200., width, 30.], &light.name, Action::SurfaceLightName);
            u.checkbox(
                [x, 242., width, 30.],
                "Включён",
                Action::SurfaceToggleLight,
                light.enabled,
            );
            u.checkbox(
                [x, 284., width, 30.],
                "Отбрасывает тени",
                Action::SurfaceToggleLightShadow,
                light.casts_shadow,
            );
            self.draw_dropdown_button(
                u,
                [x, 324., width, 30.],
                DropdownKind::LightKind,
                "Тип источника",
            );
            u.label(x, 372., "Оттенок", 11., p.muted);
            u.color_field(
                [x + 64., 364., width - 64., 30.],
                &color_hex(light.color),
                Action::SurfaceLightColor,
                light.color.map(linear_to_srgb),
            );
            u.checkbox(
                [x, 404., width, 30.],
                "Цветовая температура",
                Action::SurfaceLightTemperature,
                light.use_temperature,
            );
            if light.use_temperature {
                numeric(
                    u,
                    [x, 444., width, 30.],
                    "Температура, K",
                    light.temperature_kelvin,
                    Action::SurfaceLightNumber(12),
                    SliderRange::new(1000., 12000., 50.),
                );
            } else {
                u.disabled_button(
                    [x, 444., width, 30.],
                    &format!("Температура: {:.0} K", light.temperature_kelvin),
                );
            }
            numeric(
                u,
                [x, 490., width, 30.],
                match light.kind {
                    LightKind::Directional => "Освещённость, лк",
                    LightKind::Area => "Яркость площадного",
                    _ => "Сила света, кд",
                },
                light.intensity,
                Action::SurfaceLightNumber(0),
                SliderRange::new(0., 10000., 0.1),
            );
            for (row, label) in ["Положение, м", "Направление XYZ"].into_iter().enumerate()
            {
                let y = 536. + row as f32 * 76.;
                u.label(x, y, label, 11., p.muted);
                for axis in 0..3 {
                    let field = 1 + row * 3 + axis;
                    let w = (width - 12.) / 3.;
                    let value = light_number(light, field);
                    let range = if row == 0 {
                        SliderRange::new(
                            (-10_f64).min(value as f64),
                            10_f64.max(value as f64),
                            0.01,
                        )
                    } else {
                        SliderRange::new(-1., 1., 0.01)
                    };
                    numeric(
                        u,
                        [x + axis as f32 * (w + 6.), y + 23., w, 44.],
                        ["X", "Y", "Z"][axis],
                        value,
                        Action::SurfaceLightNumber(field),
                        range,
                    );
                }
            }
            for (row, field, label, range) in [
                (0, 14, "Направление, °", SliderRange::new(-180., 180., 1.)),
                (1, 15, "Наклон, °", SliderRange::new(-89.9, 89.9, 1.)),
            ] {
                let rect = [x, 690. + row as f32 * 40., width, 30.];
                if light.kind == LightKind::Point {
                    u.disabled_button(rect, "Точка светит во все стороны");
                } else {
                    numeric(
                        u,
                        rect,
                        label,
                        light_number(light, field),
                        Action::SurfaceLightNumber(field),
                        range,
                    );
                }
            }
            numeric(
                u,
                [x, 774., width, 30.],
                "Дальность · 0 = ∞",
                light.range,
                Action::SurfaceLightNumber(7),
                SliderRange::new(0., 100., 0.1),
            );
            if light.kind == LightKind::Area {
                for (field, label) in [(10, "Ширина, м"), (11, "Высота, м"), (13, "Поворот, °")]
                {
                    let row = if field == 13 { 2 } else { field - 10 };
                    numeric(
                        u,
                        [x, 814. + row as f32 * 40., width, 30.],
                        label,
                        light_number(light, field),
                        Action::SurfaceLightNumber(field),
                        if field == 13 {
                            SliderRange::new(-180., 180., 1.)
                        } else {
                            SliderRange::new(0.01, 10., 0.01)
                        },
                    );
                }
            } else if light.kind == LightKind::Spot {
                let (inner, outer) = cone_slider_ranges(light);
                for (row, field, label, range) in [
                    (0, 8, "Внутренний угол, °", inner),
                    (1, 9, "Внешний угол, °", outer),
                ] {
                    numeric(
                        u,
                        [x, 814. + row as f32 * 40., width, 30.],
                        label,
                        light_number(light, field),
                        Action::SurfaceLightNumber(field),
                        range,
                    );
                }
            }
            use crate::ui::LightTool as L;
            u.checkbox(
                [x, 1026., width, 28.],
                "Только этот свет · просмотр",
                Action::LightTool(L::Solo),
                self.solo_light == self.surface_light_index(),
            );
        } else {
            u.label(x, 214., "Источников пока нет", 13., p.text);
        }
        if let Some(index) = self.surface_light_index() {
            let light = &self.editor.project.lights[index];
            u.checkbox([x, 1258., width, 30.], "Объёмный свет", Action::SurfaceLightVolume, light.volume.enabled);
            numeric(u, [x, 1302., width, 30.], "Сила объёма", light.volume.strength,
                Action::SurfaceLightNumber(16), SliderRange::new(0., 10., 0.01));
            u.color_field([x, 1344., width, 28.], "Оттенок объёма", Action::SurfaceLightVolumeColor,
                light.volume.tint.map(linear_to_srgb));
        }
        u.label(x, 1396., "ОБЩАЯ СРЕДА ДЛЯ ВСЕХ ИСТОЧНИКОВ", 10., p.muted);
        for (i, field) in [10, 11, 12, 13].into_iter().enumerate() {
            self.studio_number(u, x, 1428. + i as f32 * 50., width, field);
        }
        u.color_field([x, 1640., width, 28.], "Цвет среды", Action::RenderColor(4),
            self.editor.project.render_settings.atmosphere.color.map(linear_to_srgb));
        u.rect([x, 1058., width, 1.], p.elevated);
        u.label(x, 1066., "ОКРУЖЕНИЕ", 10., p.muted);
        numeric(
            u,
            [x, 1094., width, 30.],
            "Экспозиция, EV",
            self.editor.project.render_settings.exposure,
            Action::SurfaceRenderNumber(0),
            SliderRange::new(-10., 10., 0.1),
        );
        numeric(
            u,
            [x, 1136., width, 30.],
            "Свет окружения",
            self.editor.project.render_settings.ambient_intensity,
            Action::SurfaceRenderNumber(1),
            SliderRange::new(0., 4., 0.01),
        );
        u.button(
            [x, 1182., width, 30.],
            "Рендер, тени и схемы света",
            Action::SurfaceTab(3),
            false,
        );
    }
}

pub(super) fn numeric(
    u: &mut Ui,
    rect: [f32; 4],
    label: &str,
    value: f32,
    action: Action,
    range: SliderRange,
) {
    u.number_field(rect, label, value as f64, action, range);
}

fn cone_slider_ranges(light: &Light) -> (SliderRange, SliderRange) {
    let inner = light.inner_cone_angle.to_degrees() as f64;
    let outer = light.outer_cone_angle.to_degrees() as f64;
    // Keep slider endpoints strictly inside the neighboring cone. Exact entry
    // retains the full validated domain, including unusual imported values.
    let inner_max = outer - (outer * 0.5).min(0.1);
    let inner_max = if inner_max.is_nan() {
        89.9
    } else {
        inner_max.clamp(0., 89.9)
    };
    let outer_min = inner + ((90. - inner) * 0.5).min(0.1);
    let outer_max = 89.9_f64.max(outer_min).max(outer.min(90.));
    (
        SliderRange::new(0., inner_max, 0.1),
        SliderRange::new(outer_min, outer_max, 0.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{box_mesh, Editor, Node, Vec3};

    fn app_fixture() -> App {
        let mut project = Project::default();
        project.materials.push(Material {
            id: 1,
            base_color: [0.5, 0.5, 0.5, 0.35],
            ..Material::default()
        });
        project
            .meshes
            .push(box_mesh(2, "Shared box", Vec3::ONE, Some(1)));
        for id in [3, 4] {
            let mut node = Node::new(id, format!("Box {id}"));
            node.mesh = Some(2);
            project.nodes.push(node);
        }
        let mut app = App::new(None);
        app.editor = Editor::new(project);
        app.editor.selected = Some(3);
        app.saved_snapshot = app.editor.project.clone();
        app.preferences = crate::workspace::Preferences::default();
        app.preferences.right_panel = true;
        app
    }

    #[test]
    fn hex_uses_srgb_and_rejects_alpha_or_partial_values() {
        let gray = parse_color("#808080").unwrap();
        assert!((gray[0] - 0.2158605).abs() < 0.00001);
        assert_eq!(color_hex(gray), "#808080");
        assert_eq!(parse_color(" 00ff80 ").unwrap()[1], 1.);
        for value in ["#FFF", "#00FF0080", "#GG0000", "NaN", "é12345"] {
            assert!(parse_color(value).is_err());
        }
    }

    #[test]
    fn numeric_fields_preserve_data_on_invalid_input() {
        for value in ["NaN", "inf", "-1", "1.1", "oops"] {
            assert!(number(value, 0., 1.).is_err());
        }
        assert_eq!(number("0,25", 0., 1.).unwrap(), 0.25);
    }

    #[test]
    fn cone_slider_endpoints_keep_valid_narrow_and_nearly_right_angle_cones() {
        for (inner, outer) in [(0_f32, 45_f32), (10., 10.05), (0., 0.001), (89.95, 90.)] {
            let light = Light {
                inner_cone_angle: inner.to_radians(),
                outer_cone_angle: outer.to_radians(),
                ..Light::default()
            };
            let (inner_range, outer_range) = cone_slider_ranges(&light);
            for x in [0., 25., 50., 75., 100.] {
                let inner = (inner_range.value_at(x, [0., 0., 100., 20.]) as f32).to_radians();
                let outer = (outer_range.value_at(x, [0., 0., 100., 20.]) as f32).to_radians();
                assert!(inner >= 0. && inner < light.outer_cone_angle);
                assert!(outer > light.inner_cone_angle && outer <= std::f32::consts::FRAC_PI_2);
            }
        }
    }

    #[test]
    fn map_slots_are_independent_and_clearable() {
        let mut material = Material::default();
        for slot in 0..5 {
            set_texture_slot(&mut material, slot, Some(slot as u64 + 10));
        }
        for slot in 0..5 {
            assert_eq!(texture_slot(&material, slot), Some(slot as u64 + 10));
        }
        set_texture_slot(&mut material, 2, None);
        assert_eq!(texture_slot(&material, 2), None);
        assert_eq!(texture_slot(&material, 1), Some(11));
    }

    #[test]
    fn emission_strength_preserves_chroma_and_zero_recovers_white() {
        let mut material = Material {
            emissive: [0.2, 0.4, 0.1],
            ..Material::default()
        };
        set_material_number(&mut material, 6, 2.);
        assert_eq!(material.emissive, [1., 2., 0.5]);
        set_material_number(&mut material, 6, 0.);
        set_material_number(&mut material, 6, 3.);
        assert_eq!(material.emissive, [3.; 3]);
    }

    #[test]
    fn emission_tint_preserves_hdr_strength_and_black_disables_emission() {
        let mut material = Material {
            emissive: [100., 200., 50.],
            ..Material::default()
        };
        set_emissive_tint(&mut material, parse_color("#008040").unwrap());
        assert_eq!(material_number(&material, 6), 200.);
        assert_eq!(material.emissive[0], 0.);
        assert!(material.emissive[2] > 0. && material.emissive[2] < 200.);
        assert_eq!(emissive_tint(material.emissive)[1], 1.);
        set_emissive_tint(&mut material, [0.; 3]);
        assert_eq!(material.emissive, [0.; 3]);
        set_emissive_tint(&mut material, parse_color("#FF0000").unwrap());
        assert_eq!(material.emissive, [1., 0., 0.]);
        material.emissive = [f32::MAX / 2., f32::MAX, 0.];
        set_material_number(&mut material, 6, 100.);
        assert_eq!(material.emissive, [50., 100., 0.]);
    }

    #[test]
    fn material_color_preserves_alpha_and_is_one_undo_step() {
        let mut app = app_fixture();
        let before = app.editor.project.clone();
        assert_eq!(
            app.surface_confirm(Action::SurfaceMaterialColor(false), "#808080"),
            Ok(true)
        );
        let material = app.editor.project.material(1).unwrap();
        assert_eq!(material.base_color[3], 0.35);
        assert!((material.base_color[0] - 0.2158605).abs() < 0.00001);
        assert_eq!(app.editor.undo_len(), 1);
        assert!(app.editor.undo());
        assert_eq!(app.editor.project, before);
        assert!(app.editor.redo());
        assert_eq!(app.editor.project.material(1).unwrap().base_color[3], 0.35);
    }

    #[test]
    fn invalid_field_remains_open_without_dirtying_or_history() {
        let mut app = app_fixture();
        let before = app.editor.project.clone();
        app.surface_action(Action::SurfaceMaterialNumber(1));
        app.input.as_mut().unwrap().insert("1.5");
        app.confirm_input();
        assert_eq!(app.input.as_ref().unwrap().value, "1.5");
        assert!(app.surface.input_error.is_some());
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
        assert!(!app.dirty);
        app.input.as_mut().unwrap().value = "0,25".into();
        app.confirm_input();
        assert!(app.input.is_none());
        assert_eq!(app.editor.project.material(1).unwrap().roughness, 0.25);
        assert_eq!(app.editor.undo_len(), 1);
    }

    #[test]
    fn unique_material_adds_only_to_selected_instance_and_undo_restores() {
        let mut app = app_fixture();
        let before = app.editor.project.clone();
        app.surface_action(Action::SurfaceUniqueMaterial);
        let changed = app.editor.project.node(3).unwrap().mesh.unwrap();
        assert_eq!(changed, 2);
        assert_eq!(app.editor.project.node(4).unwrap().mesh, Some(2));
        assert_eq!(app.editor.project.mesh(2).unwrap().material, Some(1));
        assert_ne!(app.editor.project.node(3).unwrap().material_layers[0].material,1);
        assert!(app.editor.project.node(4).unwrap().material_layers.is_empty());
        app.editor.project.validate().unwrap();
        assert_eq!(app.editor.undo_len(), 1);
        assert!(app.editor.undo());
        assert_eq!(app.editor.project, before);
    }

    #[test]
    fn unique_material_detaches_selected_branch_from_other_instances() {
        let mut app = app_fixture();
        app.editor.project.nodes.push(Node::new(5, "Group"));
        app.editor.project.node_mut(3).unwrap().parent = Some(5);
        app.editor.selected = Some(5);
        app.surface_action(Action::SurfaceUniqueMaterial);
        assert_eq!(app.editor.project.node(3).unwrap().mesh, Some(2));
        assert_eq!(app.editor.project.node(3).unwrap().material_layers.len(),1);
        assert_eq!(app.editor.project.node(4).unwrap().mesh, Some(2));
        app.editor.project.validate().unwrap();
    }

    #[test]
    fn opacity_edit_enables_alpha_blend_and_exposure_is_undoable() {
        let mut app = app_fixture();
        app.surface_confirm(Action::SurfaceMaterialNumber(2), "0.4")
            .unwrap();
        assert_eq!(
            app.editor.project.material(1).unwrap().alpha_mode,
            AlphaMode::Blend
        );
        let before = app.editor.project.clone();
        app.surface_confirm(Action::SurfaceRenderNumber(0), "2")
            .unwrap();
        assert_eq!(app.editor.project.render_settings.exposure, 2.);
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        assert!(app
            .surface_confirm(Action::SurfaceRenderNumber(0), "17")
            .is_err());
        assert_eq!(app.editor.project, before);
    }

    #[test]
    fn locked_object_material_is_not_editable() {
        let mut app = app_fixture();
        app.editor.project.node_mut(3).unwrap().locked = true;
        let before = app.editor.project.clone();
        assert!(app.selected_material_id().is_none());
        assert!(app
            .surface_confirm(Action::SurfaceMaterialNumber(1), "0.2")
            .is_err());
        app.surface_action(Action::SurfaceUniqueMaterial);
        assert_eq!(app.editor.project, before);
    }

    #[test]
    fn light_add_limit_and_cone_validation_preserve_scene() {
        let mut app = app_fixture();
        while app.editor.project.lights.len() < MAX_ENABLED_LIGHTS {
            app.surface_action(Action::SurfaceAddLight(2));
        }
        let before = app.editor.project.clone();
        app.surface_action(Action::SurfaceAddLight(1));
        assert_eq!(app.editor.project, before);
        assert!(app
            .surface_confirm(Action::SurfaceLightNumber(8), "80")
            .is_err());
        assert_eq!(app.editor.project, before);
        assert_eq!(
            app.surface_confirm(Action::SurfaceLightNumber(9), "60"),
            Ok(true)
        );
        assert_eq!(
            app.surface_confirm(Action::SurfaceLightNumber(8), "30"),
            Ok(true)
        );
        app.editor.project.validate().unwrap();
        app.surface_action(Action::SurfaceToggleLight);
        app.surface_action(Action::SurfaceAddLight(1));
        assert_eq!(app.editor.project.lights.len(), MAX_ENABLED_LIGHTS + 1);
        assert_eq!(
            app.editor
                .project
                .lights
                .iter()
                .filter(|light| light.enabled)
                .count(),
            MAX_ENABLED_LIGHTS
        );
    }

    #[test]
    fn contextual_controls_stay_in_panel_when_scrolled_at_minimum_size() {
        let mut app = app_fixture();
        app.width = 1100.;
        app.height = 760.;
        for theme in [crate::ui::Theme::Dark, crate::ui::Theme::Light] {
            app.preferences.theme = theme;
            for tab in [1, 2] {
                app.surface.tab = tab;
                for maps in [false, true] {
                    app.surface.maps = maps;
                    for scroll in [0., app.inspector_max_scroll()] {
                        app.inspector_scroll = scroll;
                        let ui = app.build_ui();
                        for hit in ui.hits.iter().filter(|hit| {
                            matches!(
                                hit.action,
                                Action::SurfaceMaterialNumber(_)
                                    | Action::SurfaceImportTexture(_)
                                    | Action::SurfaceLightNumber(_)
                                    | Action::SurfaceRenderNumber(_)
                            )
                        }) {
                            assert!(hit.rect[0] >= app.right_panel_rect()[0]);
                            assert!(hit.rect[1] >= 116.);
                            assert!(hit.rect[1] + hit.rect[3] <= app.height - 44.);
                        }
                    }
                }
            }
        }
    }
}
