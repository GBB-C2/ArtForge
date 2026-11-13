//! Artwork, material, layer and physics authoring share the existing undo/input path.
use super::*;
use crate::ui::{DropdownKind, SliderRange, StudioAction as S, Ui};
use poseforge_core::{AnimationLayer, BackgroundMode, Mesh, PhysicsKind, PhysicsSettings, Vertex};

#[derive(Default)]
pub(super) struct StudioState {
    pub physics_running: bool,
    pub physics: super::physics_preview::PhysicsPreview,
    pub layer: usize,
    pub bake_frames: usize,
}

impl StudioState {
    pub fn bake_frames(&self) -> usize {
        if self.bake_frames == 0 {
            48
        } else {
            self.bake_frames
        }
    }
}

// Retain the UV sphere used by existing material thumbnails.
pub(super) fn sphere(id: u64, material: u64) -> Mesh {
    let mut m = Mesh {
        id,
        name: "Сфера".into(),
        vertices: vec![].into(),
        indices: vec![].into(),
        material: Some(material),
        topology: None,
        morph_targets: vec![],
    };
    for y in 0..=24 {
        for x in 0..=32 {
            let u = x as f32 / 32.;
            let v = y as f32 / 24.;
            let theta = v * std::f32::consts::PI;
            let phi = u * std::f32::consts::TAU;
            let n = Vec3::new(
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            );
            m.vertices.push(Vertex {
                position: (n * 0.5).to_array(),
                normal: n.to_array(),
                uv: [u, v],
                ..Vertex::default()
            });
        }
    }
    for y in 0..24 {
        for x in 0..32 {
            let a = y * 33 + x;
            let b = a + 33;
            m.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    m
}

impl App {
    fn studio_value(&self, field: u8) -> Option<(f32, f32, f32, &'static str)> {
        let settings = self.editor.project.render_settings;
        match field {
            0..=2 => {
                let m = self.editor.project.material(self.selected_material_id()?)?;
                Some(match field {
                    0 => (m.transmission, 0., 1., "Пропускание"),
                    1 => (m.thickness, 0.001, 100., "Толщина, м"),
                    _ => (m.attenuation_distance, 0.001, 1000., "Длина поглощения, м"),
                })
            }
            10 => Some((settings.atmosphere.density, 0., 1., "Плотность тумана")),
            11 => Some((settings.atmosphere.height, 0.1, 1000., "Высота тумана, м")),
            12 => Some((settings.atmosphere.distance, 1., 500., "Глубина объёма, м")),
            13 => Some((settings.atmosphere.anisotropy, -0.9, 0.9, "Направленность")),
            20..=22 => Some((
                self.editor.project.fluid_settings.source_center[(field - 20) as usize],
                -100.,
                100.,
                ["Центр X", "Центр Y", "Центр Z"][(field - 20) as usize],
            )),
            23..=25 => Some((
                self.editor.project.fluid_settings.source_size[(field - 23) as usize],
                0.08,
                10.,
                ["Размер X", "Размер Y", "Размер Z"][(field - 23) as usize],
            )),
            40..=49 => {
                let p = self.editor.project.node(self.editor.selected?)?.physics?;
                Some(match field {
                    40 => (p.compliance, 0., 0.01, "Податливость"),
                    41 => (p.damping, 0., 1., "Затухание"),
                    42 => (p.gravity, 0., 30., "Гравитация"),
                    43 => (p.wind, -30., 30., "Ветер"),
                    44 => (p.thickness, 0.001, 0.5, "Толщина контакта"),
                    45 => (p.activation, 0., 1., "Активация мышцы"),
                    46 => (p.contraction, 0., 0.6, "Сокращение мышцы"),
                    47 => (p.bending, 1., 100., "Изгиб · податливость"),
                    48 => (p.friction, 0., 1., "Трение контакта"),
                    _ => (p.iterations as f32, 1., 24., "Итерации расчёта"),
                })
            }
            50 => Some((
                self.studio.bake_frames() as f32,
                2.,
                240.,
                "Запись · число кадров",
            )),
            60 => Some((
                self.editor
                    .project
                    .animation_layers
                    .get(self.studio.layer)?
                    .weight,
                0.,
                1.,
                "Вес слоя",
            )),
            _ => None,
        }
    }
    pub(super) fn studio_confirm(&mut self, action: &Action, text: &str) -> Result<bool, String> {
        let Action::Studio(S::Field(field)) = *action else {
            return Ok(false);
        };
        let (_, min, max, _) = self
            .studio_value(field)
            .ok_or("Параметр больше недоступен")?;
        let value = super::surfaces::number(text, min, max)?;
        if field == 50 {
            if value.fract() != 0. {
                return Err("Число кадров должно быть целым".into());
            }
            self.studio.bake_frames = value as usize;
            return Ok(true);
        }
        let material = self.selected_material_id();
        let node = self.editor.selected;
        let layer = self.studio.layer;
        if (40..=49).contains(&field) && !node.is_some_and(|id| self.node_selectable(id)) {
            return Err("Объект заблокирован".into());
        }
        self.editor
            .transact("Настроить artwork", |p| match field {
                0..=2 => {
                    if let Some(m) = p.materials.iter_mut().find(|m| Some(m.id) == material) {
                        match field {
                            0 => m.transmission = value,
                            1 => m.thickness = value,
                            _ => m.attenuation_distance = value,
                        }
                    }
                }
                10 => p.render_settings.atmosphere.density = value,
                11 => p.render_settings.atmosphere.height = value,
                12 => p.render_settings.atmosphere.distance = value,
                13 => p.render_settings.atmosphere.anisotropy = value,
                20..=22 => p.fluid_settings.source_center[(field - 20) as usize] = value,
                23..=25 => p.fluid_settings.source_size[(field - 23) as usize] = value,
                40..=49 => {
                    if let Some(s) = node
                        .and_then(|id| p.node_mut(id))
                        .and_then(|n| n.physics.as_mut())
                    {
                        match field {
                            40 => s.compliance = value,
                            41 => s.damping = value,
                            42 => s.gravity = value,
                            43 => s.wind = value,
                            44 => s.thickness = value,
                            45 => s.activation = value,
                            46 => s.contraction = value,
                            47 => s.bending = value,
                            48 => s.friction = value,
                            _ => s.iterations = value.round() as u32,
                        }
                    }
                }
                60 => {
                    if let Some(l) = p.animation_layers.get_mut(layer) {
                        l.weight = value;
                    }
                }
                _ => {}
            })
            .map_err(|e| e.to_string())?;
        self.update_dirty();
        Ok(true)
    }
    pub(super) fn studio_action(&mut self, command: S) {
        if self.scene_write_pending() || self.job.is_some() {
            return;
        }
        if command == S::Physics(3) {
            self.fluid_object_action(ui::FluidObjectAction::Attach);
            return;
        }
        // Removed creation modes cannot be invoked by stale menu hits either.
        if matches!(command, S::Mesh(11..) | S::Physics(3..) | S::Background(2..)) {
            return;
        }
        if let S::Mesh(kind) = command {
            self.create_primitive(kind);
            return;
        }
        match command {
            S::Field(f) => {
                if let Some((v, _, _, _)) = self.studio_value(f) {
                    self.input(Action::Studio(command), format!("{v:.6}"));
                }
                return;
            }
            S::CameraLock => {
                self.camera_navigation_lock = !self.camera_navigation_lock;
                self.status = if self.camera_navigation_lock {
                    "Навигация от лица камеры: ПКМ — осмотр, СКМ — сдвиг, колесо — движение."
                } else {
                    "Камера отсоединена от навигации: движение возвращает свободный вид."
                }
                .into();
                return;
            }
            S::PhysicsPlay => {
                self.studio.physics_running = !self.studio.physics_running;
                if self.studio.physics_running { self.editor.playing = false; self.last_tick = Instant::now(); }
                self.status = if self.studio.physics_running {
                    "Предпросмотр ткани без ключей · запись формы и анимации выполняется отдельно."
                } else { "Предпросмотр ткани на паузе." }.into();
                return;
            }
            S::PhysicsReset => {
                self.studio.physics_running = false;
                self.studio.physics.reset();
                return;
            }
            S::PhysicsSequence => {
                self.start_physics_bake();
                return;
            }
            S::PhysicsBake => {
                self.bake_physics();
                return;
            }
            S::LayerSelect(i) => {
                if i < self.editor.project.animation_layers.len() {
                    self.studio.layer = i;
                }
                return;
            }
            _ => {}
        }
        let selected = self.editor.selected;
        let layer = self.studio.layer;
        let material_id = self.selected_material_id();
        if matches!(
            command,
            S::Physics(_) | S::PhysicsPin | S::PhysicsSelfCollision
        ) && !selected.is_some_and(|id| self.node_selectable(id) || (!self.editor.project.node_locked(id) && self.editor.project.node(id).is_some_and(|n| n.destruction.is_some())))
        {
            self.status = "Выберите доступный меш.".into();
            return;
        }
        if let S::Physics(kind) = command {
            let Some(node) = selected.and_then(|id| self.editor.project.node(id)) else { return; };
            let Some(mesh) = node.mesh.and_then(|id| self.editor.project.mesh(id)) else { return; };
            if kind == 2 && (node.skin.is_some() || mesh.vertices.len() > 8192 || !mesh.morph_targets.is_empty()) {
                self.status = "Ткани нужен меш до 8192 вершин без skin и morph. Предыдущий компонент сохранён.".into();
                return;
            }
        }
        let replacement = if matches!(command, S::Physics(_)) {
            if let Some(id) = selected {
                match destruction_controls::without_destruction(&self.editor.project, id, true) {
                    Ok(p) => Some(p), Err(error) => { self.status = error; return; }
                }
            } else { None }
        } else { None };
        let result = self.editor.transact("Настроить сцену", |p| {
            if let Some(replacement) = replacement { *p = replacement; }
            match command {
                S::GeometricThickness => {
                    if let Some(m) = p.materials.iter_mut().find(|m| Some(m.id) == material_id) {
                        m.geometric_thickness = !m.geometric_thickness;
                    }
                }
                S::AbsorptionFromBase => {
                    if let Some(m) = p.materials.iter_mut().find(|m| Some(m.id) == material_id) {
                        m.attenuation_color = None;
                    }
                }
                S::Mesh(_) => {}
                S::Physics(kind) => {
                    if let Some(id) = selected {
                        if let Some(mesh_id) = p.node(id).and_then(|n| n.mesh) {
                            let valid = p.node(id).is_some_and(|n| n.skin.is_none())
                                && p.mesh(mesh_id).is_some_and(|m| {
                                    m.vertices.len() <= 8192 && m.morph_targets.is_empty()
                                });
                            if kind == 1 || kind == 0 || valid {
                                if kind > 1
                                    && p.nodes.iter().filter(|n| n.mesh == Some(mesh_id)).count() > 1
                                {
                                    let mut m = p.mesh(mesh_id).unwrap().clone();
                                    m.id = p.next_id();
                                    p.node_mut(id).unwrap().mesh = Some(m.id);
                                    p.meshes.push(m);
                                }
                                p.node_mut(id).unwrap().fluid = None;
                                if kind == 2 {
                                    if p.node(id).unwrap().physics.is_none_or(|s|s.kind!=PhysicsKind::Cloth) {p.node_mut(id).unwrap().collision_targets.cloth=Some(vec![]);}
                                    // A freshly created plane needs a grid to bend.
                                    // Preserve already subdivided or authored meshes.
                                    let source = p.mesh(mesh_id).unwrap();
                                    let grid = source.topology.as_ref().and_then(|t| t.primitive)
                                        .filter(|s| s.kind == poseforge_core::PrimitiveKind::Plane && s.segments[0] == 1 && s.segments[1] == 1)
                                        .map(|mut s| { s.segments = [16, 16, 1]; s });
                                    if let Some(grid) = grid {
                                        if let Ok(mesh) = grid.mesh(p.next_id(), source.material) {
                                            p.node_mut(id).unwrap().mesh = Some(mesh.id);
                                            p.meshes.push(mesh);
                                        }
                                    }
                                }
                                p.node_mut(id).unwrap().physics = match kind {
                                    0 => None,
                                    1 => Some(PhysicsSettings {
                                        kind: PhysicsKind::Collider,
                                        ..Default::default()
                                    }),
                                    _ => Some(PhysicsSettings::default()),
                                };
                            }
                        }
                    }
                }
                S::PhysicsSelfCollision => {
                    if let Some(s) = selected
                        .and_then(|id| p.node_mut(id))
                        .and_then(|n| n.physics.as_mut())
                    {
                        s.self_collision = !s.self_collision;
                    }
                }
                S::PhysicsPin => {
                    if let Some(s) = selected
                        .and_then(|id| p.node_mut(id))
                        .and_then(|n| n.physics.as_mut())
                    {
                        s.pinned = !s.pinned;
                    }
                }
                S::Background(mode) => {
                    p.render_settings.background_mode = match mode {
                        0 => BackgroundMode::Theme,
                        _ => BackgroundMode::Solid,
                    }
                }
                S::FluidFill(kind) => p.fluid_settings.source_kind = kind.min(2),
                S::FluidFromMesh => {
                    if let Some((min, max)) =
                        selected.and_then(|id| super::physics_preview::node_bounds(p, id))
                    {
                        p.fluid_settings.source_center = ((min + max) * 0.5).to_array();
                        p.fluid_settings.source_size = (max - min).max(Vec3::splat(0.08)).to_array();
                    }
                }
                S::LayerAdd => {
                    if p.clips.is_empty() {
                        p.clips
                            .push(poseforge_core::Clip::new("Анимация", p.duration));
                    }
                    p.animation_layers.push(AnimationLayer {
                        name: format!("Слой {}", p.animation_layers.len() + 1),
                        clip: 0,
                        weight: 1.,
                        additive: false,
                        enabled: true,
                        solo: false,
                    });
                }
                S::LayerEnabled(i) => {
                    if let Some(l) = p.animation_layers.get_mut(i) {
                        l.enabled = !l.enabled;
                    }
                }
                S::LayerSolo(i) => {
                    if let Some(l) = p.animation_layers.get_mut(i) {
                        l.solo = !l.solo;
                    }
                }
                S::LayerRemove(i) => {
                    if i < p.animation_layers.len() {
                        p.animation_layers.remove(i);
                    }
                }
                S::LayerMove(up) => {
                    let other = if up {
                        layer.saturating_sub(1)
                    } else {
                        layer + 1
                    };
                    if layer < p.animation_layers.len() && other < p.animation_layers.len() {
                        p.animation_layers.swap(layer, other);
                    }
                }
                S::LayerBlend => {
                    if let Some(l) = p.animation_layers.get_mut(layer) {
                        l.additive = !l.additive;
                    }
                }
                S::LayerClip => {
                    if let Some(l) = p.animation_layers.get_mut(layer) {
                        l.clip = (l.clip + 1) % p.clips.len().max(1);
                    }
                }
                _ => {}
            }
        });
        if self.result(result).is_some() {
            if matches!(command, S::Physics(_)) {
                self.studio.physics_running = false;
                self.studio.physics.reset();
                if let Some(id) = selected { self.fluid_domains.reset(id); }
                self.invalidate_geometry();
            }
            match command {
                S::LayerAdd => {
                    if self.editor.active_clip.is_none() {
                        self.editor.active_clip = Some(0);
                    }
                    self.studio.layer =
                        self.editor.project.animation_layers.len().saturating_sub(1);
                    self.show_animation_layers();
                }
                S::LayerRemove(index) => {
                    self.studio.layer = if index < layer {
                        layer.saturating_sub(1)
                    } else {
                        layer
                    }
                    .min(self.editor.project.animation_layers.len().saturating_sub(1));
                }
                S::LayerMove(up) => {
                    self.studio.layer = if up {
                        layer.saturating_sub(1)
                    } else {
                        (layer + 1)
                            .min(self.editor.project.animation_layers.len().saturating_sub(1))
                    };
                }
                S::Physics(2) => {
                    if let Some(id) = selected { self.scene_tools.material_expanded.insert(id); }
                    self.set_properties_tab(11);
                    self.status = "Ткань назначена примитиву и показана рядом с материалом.".into();
                }
                S::Physics(k) if k > 1 => self.status =
                    "Деформация: меш до 8192 вершин без skin/morph; запустите расчёт в свойствах."
                        .into(),
                S::FluidFill(_) | S::FluidFromMesh => {
                    self.rebuild_fluid_source();
                    self.fluid_on = true;
                    self.editor.mode = EditorMode::Animation;
                }
                _ => {}
            }
        }
    }
    pub(super) fn studio_number(&self, u: &mut Ui, x: f32, y: f32, width: f32, field: u8) {
        if let Some((value, min, max, label)) = self.studio_value(field) {
            u.number_field(
                [x, y, width, 48.],
                label,
                value as f64,
                Action::Studio(S::Field(field)),
                SliderRange::new(
                    min as f64,
                    max as f64,
                    if field == 40 {
                        0.000001
                    } else if field == 49 || field == 50 {
                        1.
                    } else {
                        0.001
                    },
                ),
            );
        }
    }
    pub(super) fn draw_material_studio(&self, u: &mut Ui, x: f32, width: f32, y: f32) {
        if self.npr_enabled() {
            self.draw_npr_details(u, x, width, y);
            return;
        }
        u.label(x, y, "СТЕКЛО И ПРОПУСКАНИЕ", 11., u.palette().muted);
        for i in 0..3 {
            self.studio_number(u, x, y + 24. + i as f32 * 52., width, i);
        }
        if let Some(m) = self
            .selected_material_id()
            .and_then(|id| self.editor.project.material(id))
        {
            let color =
                m.attenuation_color
                    .unwrap_or([m.base_color[0], m.base_color[1], m.base_color[2]]);
            u.color_field(
                [x, y + 190., width, 30.],
                "Поглощение",
                Action::SurfaceAttenuationColor,
                color.map(super::surfaces::linear_to_srgb),
            );
            u.button(
                [x, y + 228., width, 28.],
                "Из цвета материала",
                Action::Studio(S::AbsorptionFromBase),
                m.attenuation_color.is_none(),
            );
            u.checkbox(
                [x, y + 264., width, 28.],
                "Толщина по геометрии",
                Action::Studio(S::GeometricThickness),
                m.geometric_thickness,
            );
            u.label(x, y + 316., "БЛИК И ТКАНЬ", 11., u.palette().muted);
            for (row, (field, label, min, max, step)) in [
                (10, "Анизотропия", 0., 0.95, 0.01),
                (11, "Направление блика, °", -180., 180., 1.),
                (12, "Ворсистый блеск", 0., 1., 0.01),
                (13, "Шероховатость", 0.07, 1., 0.01),
            ]
            .into_iter()
            .enumerate()
            {
                u.number_field(
                    [x, y + 342. + row as f32 * 56., width, 48.],
                    label,
                    super::surfaces::material_number(m, field) as f64,
                    Action::SurfaceMaterialNumber(field),
                    SliderRange::new(min, max, step),
                );
            }
        }
    }
    pub(super) fn draw_render_studio(&self, u: &mut Ui, x: f32, width: f32) {
        u.label(x, 1260., "ФОН", 11., u.palette().muted);
        self.draw_dropdown_button(
            u,
            [x, 1284., width, 30.],
            DropdownKind::Background,
            "Фон проекта",
        );

    }
    #[cfg(test)]
    pub(super) fn draw_fluid_studio(&self, u: &mut Ui, x: f32, width: f32) {
        u.label(x, 1024., "СОЗДАНИЕ ЖИДКОСТИ", 11., u.palette().muted);
        self.draw_dropdown_button(
            u,
            [x, 1052., width, 30.],
            DropdownKind::FluidSource,
            "Создать объём",
        );
        u.button(
            [x, 1092., width, 30.],
            "Объём выбранного меша",
            Action::Studio(S::FluidFromMesh),
            false,
        );
        for field in 20..=25 {
            self.studio_number(u, x, 1136. + (field - 20) as f32 * 56., width, field);
        }
        u.fitted_label(
            [x, 1490., width, 32.],
            "Коллайдеры влияют на жидкость.",
            11.,
            u.palette().muted,
        );
    }
    pub(super) fn cloth_collision_shift(&self) -> f32 {36. + self.collision_layout_shift(poseforge_core::CollisionKind::Cloth)}
    pub(super) fn physics_panel_bottom(&self) -> f32 {
        match self
            .editor
            .selected
            .and_then(|id| self.editor.project.node(id))
            .and_then(|n| n.physics)
            .map(|s| s.kind)
        {
            Some(PhysicsKind::Cloth) => 1000. + self.cloth_collision_shift(),
            Some(PhysicsKind::Muscle) => 1064.,
            Some(PhysicsKind::Collider) => 285.,
            None => 240.,
        }
    }
    pub(super) fn draw_physics_panel(&self, u: &mut Ui, rx: f32, rw: f32) {
        let x = rx + 16.;
        let width = rw - 32.;
        if self.editor.selected.is_none_or(|id| {
            !self.node_selectable(id)
                || self
                    .editor
                    .project
                    .node(id)
                    .is_none_or(|n| n.mesh.is_none())
        }) {
            u.fitted_label(
                [x, 174., width, 40.],
                "Выберите незаблокированный меш",
                12.,
                u.palette().muted,
            );
            return;
        }
        let cloth=self.editor.selected.and_then(|id|self.editor.project.node(id)).and_then(|n|n.physics).is_some_and(|s|s.kind==PhysicsKind::Cloth);
        if cloth {u.fitted_label([x,174.,width,30.],"Ткань · симуляция",12.,u.palette().text);}
        else {self.draw_dropdown_button(u,[x,174.,width,30.],DropdownKind::PhysicsType,"Ранее созданная мышца");}
        if let Some(settings) = self
            .editor
            .selected
            .and_then(|id| self.editor.project.node(id))
            .and_then(|n| n.physics)
        {
            if settings.kind == PhysicsKind::Collider {
                u.fitted_label(
                    [x, 220., width, 36.],
                    "Препятствие для ткани, жидкости и осколков.",
                    11.,
                    u.palette().muted,
                );
                u.fitted_label([x, 258., width, 20.], "Столкновения по поверхности меша.", 10., u.palette().muted);
                return;
            }
            let half = (width - 8.) / 2.;
            u.icon_button(
                [x, 220., half, 30.],
                if self.studio.physics_running {
                    "Пауза"
                } else {
                    "Запустить"
                },
                Action::Studio(S::PhysicsPlay),
                self.studio.physics_running,
                if self.studio.physics_running { Icon::Pause } else { Icon::Play },
            );
            u.button(
                [x + half + 8., 220., half, 30.],
                "Сбросить",
                Action::Studio(S::PhysicsReset),
                false,
            );
            u.button(
                [x, 262., width, 30.],
                "Записать форму в меш",
                Action::Studio(S::PhysicsBake),
                false,
            );
            u.checkbox(
                [x, 302., width, 30.],
                "Закрепить край",
                Action::Studio(S::PhysicsPin),
                settings.pinned,
            );
            let mut fields = vec![40, 41, 42, 43, 44, 47, 48, 49];
            if settings.kind == PhysicsKind::Muscle {
                fields.extend([45, 46]);
            }
            let shift=if cloth {self.cloth_collision_shift()} else {0.};
            if cloth {self.draw_collision_list(u,poseforge_core::CollisionKind::Cloth,x,342.,width);}
            let bottom = 342. + shift + fields.len() as f32 * 52.;
            u.checkbox(
                [x, bottom, width, 30.],
                "Самостолкновения ткани",
                Action::Studio(S::PhysicsSelfCollision),
                settings.self_collision,
            );
            self.studio_number(u, x, bottom + 42., width, 50);
            u.button(
                [x, bottom + 104., width, 30.],
                "Записать анимацию · 24 кадр/с",
                Action::Studio(S::PhysicsSequence),
                false,
            );
            u.fitted_label(
                [x, bottom + 144., width, 30.],
                "С исходной формы · Esc отменяет запись",
                10.,
                u.palette().muted,
            );
            if cloth {u.button([x,bottom+180.,width,30.],"Убрать ткань с объекта",Action::Studio(S::Physics(0)),false);}
            for (row, field) in fields.into_iter().enumerate() {
                self.studio_number(u, x, 342. + shift + row as f32 * 52., width, field);
            }
        }
    }
}
