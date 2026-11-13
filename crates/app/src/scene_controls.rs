//! Project-owned render, environment and fluid settings, using direct panel edits.
use super::surfaces::{color_hex, linear_to_srgb, number, parse_color};
use super::App;
use crate::icons::Icon;
use crate::ui::{Action, DropdownKind, SliderRange, Ui};
use poseforge_core::{FluidSettings, Light, LightKind, Project, RenderSettings, ToneMapping, Vec3};

pub(super) const RENDER_PANEL_BOTTOM: f32 = 2270.;
#[cfg(test)]
pub(super) const FLUID_PANEL_BOTTOM: f32 = 1630.;

pub(super) fn render_number(settings: RenderSettings, field: usize) -> Option<f32> {
    match field {
        0 => Some(settings.exposure),
        1 => Some(settings.ambient_intensity),
        2 => Some(settings.shadow_bias),
        3 => Some(settings.shadow_strength),
        4 => Some(settings.shadow_filter_radius),
        5 => Some(settings.ambient_occlusion),
        6 => Some(settings.ambient_occlusion_radius),
        7 => Some(settings.hdri_strength),
        8 => Some(settings.hdri_rotation),
        9 => Some(settings.npr_contours.width),
        10 => Some(settings.npr_contours.depth_threshold * 100.),
        11 => Some(settings.npr_contours.normal_angle),
        _ => None,
    }
}

pub(super) fn fluid_number(settings: FluidSettings, field: usize) -> Option<f32> {
    match field {
        0 => Some(settings.gravity),
        1 => Some(settings.viscosity),
        2 => Some(settings.vorticity),
        3 => Some(settings.boundary_friction),
        4 => Some(settings.iterations as f32),
        5 => Some(settings.max_substeps as f32),
        6 => Some(settings.time_scale),
        7 => Some(settings.surface_cell_size),
        8 => Some(settings.surface_iso_density),
        _ => None,
    }
}

fn apply_quality(settings: &mut RenderSettings, quality: u8) -> bool {
    let (samples, shadows, resolution) = match quality {
        0 => (1, false, 512),
        1 => (4, true, 1024),
        2 => (4, true, 2048),
        _ => return false,
    };
    settings.ambient_occlusion = if quality == 0 { 0. } else { 0.7 };
    settings.antialiasing = samples;
    settings.shadows = shadows;
    settings.shadow_resolution = resolution;
    true
}

pub(super) fn quality_selected(settings: RenderSettings, quality: u8) -> bool {
    let mut preset = settings;
    apply_quality(&mut preset, quality)
        && preset.antialiasing == settings.antialiasing
        && preset.ambient_occlusion == settings.ambient_occlusion
        && preset.shadows == settings.shadows
        && preset.shadow_resolution == settings.shadow_resolution
}

pub(super) fn apply_fluid_preset(settings: &mut FluidSettings, preset: u8) -> bool {
    let (viscosity, vorticity, friction, iterations) = match preset {
        0 => (0.04, 0.1, 0.05, 4),
        1 => (0.16, 0.02, 0.18, 6),
        _ => return false,
    };
    settings.gravity = 9.81;
    settings.viscosity = viscosity;
    settings.vorticity = vorticity;
    settings.boundary_friction = friction;
    settings.iterations = iterations;
    settings.max_substeps = 4;
    true
}

#[cfg(test)]
fn fluid_preset_selected(settings: FluidSettings, preset: u8) -> bool {
    let mut selected = settings;
    apply_fluid_preset(&mut selected, preset) && selected == settings
}

fn directional(name: &str, direction: [f32; 3], color: [f32; 3], intensity: f32) -> Light {
    Light {
        name: name.into(),
        kind: LightKind::Directional,
        direction: Vec3::from_array(direction).normalize(),
        color,
        intensity,
        ..Light::default()
    }
}

fn lighting_preset(preset: u8) -> Option<(Vec<Light>, [f32; 3], [f32; 3])> {
    // Directions describe light travel. Every enabled source may cast shadows.
    match preset {
        0 => Some((
            vec![
                directional("Студия · основной", [-0.5, -1., -0.7], [1., 0.88, 0.74], 3.),
                directional(
                    "Студия · заполнение",
                    [0.8, -0.4, -0.3],
                    [0.58, 0.72, 1.],
                    0.8,
                ),
                directional("Студия · контровой", [0.2, -0.5, 1.], [0.78, 0.88, 1.], 1.8),
            ],
            [0.28, 0.32, 0.42],
            [0.09, 0.08, 0.07],
        )),
        1 => Some((
            vec![
                directional(
                    "Мягкий свет · основной",
                    [-0.4, -1., -0.6],
                    [1., 0.96, 0.9],
                    1.8,
                ),
                directional(
                    "Мягкий свет · заполнение",
                    [0.6, -0.4, -0.8],
                    [0.8, 0.88, 1.],
                    1.2,
                ),
                directional("Мягкий свет · контровой", [0., -0.5, 1.], [1.; 3], 0.8),
            ],
            [0.45, 0.48, 0.55],
            [0.18, 0.17, 0.16],
        )),
        2 => Some((
            vec![
                directional(
                    "Контраст · основной",
                    [-0.9, -0.65, -0.5],
                    [1., 0.78, 0.56],
                    4.2,
                ),
                directional(
                    "Контраст · заполнение",
                    [0.8, -0.3, -0.5],
                    [0.42, 0.58, 1.],
                    0.25,
                ),
                directional(
                    "Контраст · контровой",
                    [0.5, -0.25, 1.],
                    [0.58, 0.76, 1.],
                    2.8,
                ),
            ],
            [0.14, 0.17, 0.24],
            [0.035, 0.03, 0.025],
        )),
        3 => Some((
            vec![
                Light {
                    name: "Софтбокс · основной".into(),
                    kind: LightKind::Area,
                    position: Vec3::new(-3., 4., 3.),
                    direction: Vec3::new(3., -3., -3.).normalize(),
                    area_size: [3., 3.],
                    intensity: 5.,
                    color: [1., 0.93, 0.85],
                    ..Light::default()
                },
                Light {
                    name: "Софтбокс · заполнение".into(),
                    kind: LightKind::Area,
                    position: Vec3::new(3., 2.5, 1.),
                    direction: Vec3::new(-3., -1.5, -1.).normalize(),
                    area_size: [2., 2.],
                    intensity: 2.,
                    color: [0.72, 0.83, 1.],
                    ..Light::default()
                },
                directional("Софтбокс · контровой", [0.1, -0.4, 1.], [1.; 3], 1.2),
            ],
            [0.28, 0.32, 0.42],
            [0.09, 0.08, 0.07],
        )),
        _ => None,
    }
}

#[cfg(test)]
fn choice_row<const N: usize>(u: &mut Ui, rect: [f32; 4], choices: [(&str, Action, bool); N]) {
    let p = u.palette();
    let width = (rect[2] - (N.saturating_sub(1) as f32) * 6.) / N as f32;
    for (index, (label, action, selected)) in choices.into_iter().enumerate() {
        let x = rect[0] + index as f32 * (width + 6.);
        let r = [x, rect[1], width, rect[3]];
        u.button(r, "", action, selected);
        u.fitted_label(
            [x + 6., rect[1] + (rect[3] - 15.) * 0.5, width - 12., 16.],
            label,
            11.,
            if selected { p.accent } else { p.text },
        );
    }
}

impl App {
    fn scene_settings_edit(&mut self, label: &str, operation: impl FnOnce(&mut Project)) -> bool {
        let result = self.editor.transact(label, operation);
        if self.result(result).is_some() {
            self.status = label.into();
            true
        } else {
            false
        }
    }

    pub(super) fn scene_settings_action(&mut self, action: Action) {
        self.surface.input_error = None;
        match action {
            Action::SurfaceRenderNumber(field) => {
                if let Some(value) = render_number(self.editor.project.render_settings, field) {
                    self.input(action, format!("{value:.6}"));
                }
            }
            Action::RenderColor(field) => {
                let settings = self.editor.project.render_settings;
                let color = match field {
                    0 => settings.sky_color,
                    1 => settings.ground_color,
                    2 => settings.background_color,
                    4 => settings.atmosphere.color,
                    5 => settings.npr_contours.color,
                    _ => return,
                };
                self.input(action, color_hex(color));
            }
            Action::RenderAntialiasing(samples) => {
                self.scene_settings_edit("Сглаживание", |p| {
                    p.render_settings.antialiasing = samples
                });
            }
            Action::RenderToneMapping(mode) => {
                let tone_mapping = match mode {
                    0 => ToneMapping::Aces,
                    1 => ToneMapping::Reinhard,
                    2 => ToneMapping::Linear,
                    _ => return,
                };
                self.scene_settings_edit("Тональное отображение", |p| {
                    p.render_settings.tone_mapping = tone_mapping
                });
            }
            Action::RenderToggleShadows => {
                self.scene_settings_edit("Тени", |p| {
                    p.render_settings.shadows = !p.render_settings.shadows
                });
            }
            Action::RenderShadowResolution(resolution) => {
                self.scene_settings_edit("Размер карты теней", |p| {
                    p.render_settings.shadow_resolution = resolution
                });
            }
            Action::RenderQuality(quality) if quality < 3 => {
                self.scene_settings_edit("Качество рендера", |p| {
                    apply_quality(&mut p.render_settings, quality);
                });
            }
            Action::LightingPreset(preset) => {
                if let Some((lights, sky, ground)) = lighting_preset(preset) {
                    self.scene_settings_edit("Схема освещения", |p| {
                        p.clear_light_organization();
                        p.lights = lights;
                        p.render_settings.sky_color = sky;
                        p.render_settings.ground_color = ground;
                    });
                }
            }
            Action::FluidNumber(field) => {
                if let Some(value) = fluid_number(self.editor.project.fluid_settings, field) {
                    self.input(
                        action,
                        if matches!(field, 4 | 5) {
                            format!("{value:.0}")
                        } else {
                            format!("{value:.6}")
                        },
                    );
                }
            }
            Action::FluidPreset(preset) if preset < 2 => {
                self.scene_settings_edit("Профиль жидкости", |p| {
                    apply_fluid_preset(&mut p.fluid_settings, preset);
                });
            }
            Action::FluidResetSettings => {
                self.scene_settings_edit(
                    "Сбросить параметры жидкости",
                    |p| p.fluid_settings = FluidSettings::default(),
                );
            }
            _ => {}
        }
    }

    pub(super) fn scene_settings_confirm(
        &mut self,
        action: Action,
        value: &str,
    ) -> Result<bool, String> {
        let committed = match action {
            Action::SurfaceRenderNumber(field) => {
                let (min, max) = match field {
                    0 => (-16., 16.),
                    1 => (0., 16.),
                    2 => (0., 0.02),
                    3 => (0., 1.),
                    4 => (0., 4.),
                    5 => (0., 2.),
                    6 => (0.01, 10.),
                    7 => (0., 32.),
                    8 => (-180., 180.),
                    9 => (0., 8.),
                    10 => (0.01, 100.),
                    11 => (1., 179.),
                    _ => return Err("Неизвестный параметр рендера.".into()),
                };
                let value = number(value, min, max)?;
                self.scene_settings_edit("Параметр рендера", |p| match field {
                    0 => p.render_settings.exposure = value,
                    1 => p.render_settings.ambient_intensity = value,
                    2 => p.render_settings.shadow_bias = value,
                    3 => p.render_settings.shadow_strength = value,
                    4 => p.render_settings.shadow_filter_radius = value,
                    5 => p.render_settings.ambient_occlusion = value,
                    6 => p.render_settings.ambient_occlusion_radius = value,
                    7 => p.render_settings.hdri_strength = value,
                    8 => p.render_settings.hdri_rotation = value,
                    9 => p.render_settings.npr_contours.width = value,
                    10 => p.render_settings.npr_contours.depth_threshold = value / 100.,
                    _ => p.render_settings.npr_contours.normal_angle = value,
                })
            }
            Action::RenderColor(field) => {
                if !matches!(field, 0..=2 | 4..=5) {
                    return Err("Неизвестный цвет окружения.".into());
                }
                let color = parse_color(value)?;
                let current = match field {
                    0 => self.editor.project.render_settings.sky_color,
                    1 => self.editor.project.render_settings.ground_color,
                    2 => self.editor.project.render_settings.background_color,
                    4 => self.editor.project.render_settings.atmosphere.color,
                    _ => self.editor.project.render_settings.npr_contours.color,
                };
                // Retain authored floating-point/HDR precision when the shown
                // swatch is accepted without changing its displayed HEX value.
                if color_hex(current) == color_hex(color) {
                    return Ok(true);
                }
                self.scene_settings_edit("Цвет окружения и фона", |p| match field
                {
                    0 => p.render_settings.sky_color = color,
                    1 => p.render_settings.ground_color = color,
                    2 => p.render_settings.background_color = color,
                    4 => p.render_settings.atmosphere.color = color,
                    _ => p.render_settings.npr_contours.color = color,
                })
            }
            Action::FluidNumber(field) => {
                if field > 8 {
                    return Err("Неизвестный параметр жидкости.".into());
                }
                if matches!(field, 4 | 5) {
                    let integer = value
                        .trim()
                        .parse::<u32>()
                        .map_err(|_| "Введите целое число.".to_string())?;
                    self.scene_settings_edit("Параметр жидкости", |p| {
                        if field == 4 {
                            p.fluid_settings.iterations = integer;
                        } else {
                            p.fluid_settings.max_substeps = integer;
                        }
                    })
                } else {
                    let (min, max) = match field {
                        0 => (0., 30.),
                        1 => (0., 0.2),
                        2 => (0., 5.),
                        3 => (0., 1.),
                        6 => (0.1, 2.),
                        7 => (0.025, 0.12),
                        _ => (0.1, 1.2),
                    };
                    let value = number(value, min, max)?;
                    self.scene_settings_edit("Параметр жидкости", |p| match field {
                        0 => p.fluid_settings.gravity = value,
                        1 => p.fluid_settings.viscosity = value,
                        2 => p.fluid_settings.vorticity = value,
                        3 => p.fluid_settings.boundary_friction = value,
                        6 => p.fluid_settings.time_scale = value,
                        7 => p.fluid_settings.surface_cell_size = value,
                        _ => p.fluid_settings.surface_iso_density = value,
                    })
                }
            }
            _ => return Ok(false),
        };
        if committed {
            Ok(true)
        } else {
            Err(self.status.clone())
        }
    }

    pub(super) fn draw_render_settings(&self, u: &mut Ui, rx: f32, rw: f32) {
        let p = u.palette();
        let x = rx + 16.;
        let width = rw - 32.;
        let settings = self.editor.project.render_settings;
        u.label(x, 171., "КАЧЕСТВО РЕНДЕРА", 11., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 195., width, 30.],
            DropdownKind::RenderQuality,
            "Пользовательское",
        );
        u.label(x, 239., "Сглаживание", 11., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 259., width, 30.],
            DropdownKind::Antialiasing,
            "Пользовательское",
        );
        if let Some(renderer) = &self.renderer {
            let effective = renderer.effective_antialiasing(settings.antialiasing);
            let fallback = effective != settings.antialiasing;
            let label = if fallback {
                "На этом GPU доступно 1×.".into()
            } else {
                format!("Активно: {effective}×")
            };
            u.fitted_label(
                [x, 303., width, 16.],
                &label,
                10.,
                if fallback { p.warn } else { p.muted },
            );
        } else {
            u.fitted_label(
                [x, 303., width, 16.],
                "MSAA сглаживает границы геометрии.",
                10.,
                p.muted,
            );
        }
        u.label(x, 327., "Тональное отображение", 11., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 349., width, 30.],
            DropdownKind::ToneMapping,
            "Пользовательское",
        );
        u.checkbox(
            [x, 397., width, 32.],
            "Тени",
            Action::RenderToggleShadows,
            settings.shadows,
        );
        u.label(x, 443., "Размер карты теней", 11., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 465., width, 30.],
            DropdownKind::ShadowResolution,
            "Пользовательское",
        );
        u.number_field(
            [x, 511., width, 30.],
            "Смещение тени",
            settings.shadow_bias as f64,
            Action::SurfaceRenderNumber(2),
            SliderRange::new(0., 0.02, 0.0001),
        );
        u.number_field(
            [x, 551., width, 30.],
            "Сила тени",
            settings.shadow_strength as f64,
            Action::SurfaceRenderNumber(3),
            SliderRange::new(0., 1., 0.01),
        );
        u.number_field(
            [x, 591., width, 30.],
            "Фильтр тени",
            settings.shadow_filter_radius as f64,
            Action::SurfaceRenderNumber(4),
            SliderRange::new(0., 4., 0.1),
        );
        let shadow_label = if !settings.shadows {
            "Тени выключены".into()
        } else if let Some(stats) = self
            .renderer
            .as_ref()
            .map(|renderer| renderer.shadow_stats())
            .filter(|stats| stats.layers > 0)
        {
            format!(
                "Тени: {} ист. · {} px{}",
                stats.shadow_lights,
                stats.effective_resolution,
                if stats.effective_resolution < stats.requested_resolution {
                    " · авто"
                } else {
                    ""
                }
            )
        } else {
            "Солнце · точка · прожектор".into()
        };
        u.fitted_label([x, 623., width, 11.], &shadow_label, 9., p.muted);
        let later = (u.rects.len(), u.texts.len(), u.hits.len());
        u.rect([x, 634., width, 1.], p.border);
        u.label(x, 650., "ОСВЕЩЕНИЕ И ОКРУЖЕНИЕ", 11., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 672., width, 30.],
            DropdownKind::LightingPreset,
            "Выбрать схему",
        );
        u.fitted_label(
            [x, 714., width, 16.],
            "Схема заменяет свет и окружение.",
            10.,
            p.muted,
        );
        u.number_field(
            [x, 746., width, 30.],
            "Экспозиция, EV",
            settings.exposure as f64,
            Action::SurfaceRenderNumber(0),
            SliderRange::new(-10., 10., 0.1),
        );
        u.number_field(
            [x, 786., width, 30.],
            "Свет окружения",
            settings.ambient_intensity as f64,
            Action::SurfaceRenderNumber(1),
            SliderRange::new(0., 4., 0.01),
        );
        for (field, label, value) in [
            (0, "Верх", settings.sky_color),
            (1, "Низ", settings.ground_color),
            (2, "Фон", settings.background_color),
        ] {
            let y = 831. + field as f32 * 40.;
            u.label(x, y + 8., label, 11., p.muted);
            u.color_field(
                [x + 84., y, width - 84., 30.],
                &color_hex(value),
                Action::RenderColor(field),
                value.map(linear_to_srgb),
            );
        }
        self.draw_render_studio(u, x, width);
        u.rect([x, 955., width, 1.], p.border);
        u.label(x, 974., "ВЫВОД PNG", 11., p.muted);
        let half = (width - 10.) * 0.5;
        u.number_field(
            [x, 998., half, 44.],
            "Ш",
            settings.output_width as f64,
            Action::EditWidth,
            SliderRange::new(64., 4096., 1.),
        );
        u.number_field(
            [x + half + 10., 998., half, 44.],
            "В",
            settings.output_height as f64,
            Action::EditHeight,
            SliderRange::new(64., 4096., 1.),
        );
        u.checkbox(
            [x, 1054., width, 32.],
            "Прозрачный фон",
            Action::Transparent,
            settings.transparent_background,
        );
        u.icon_button(
            [x, 1098., width, 32.],
            "Сохранить PNG",
            Action::Export,
            true,
            Icon::Export,
        );
        u.icon_button(
            [x, 1140., width, 32.],
            "Последовательность PNG",
            Action::ExportSequence,
            false,
            Icon::Sequence,
        );
        u.icon_button(
            [x, 1184., width, 32.],
            "Сохранить текущую камеру",
            Action::SetCamera,
            false,
            Icon::Camera,
        );
        u.fitted_label(
            [x, 1230., width, 16.],
            "Настройки сохраняются в проекте.",
            10.,
            p.muted,
        );
        u.translate_section(later, 660.);
        self.draw_world_settings(u, x, width);
        for (field, label, y, min, max, step) in [
            (9, "Ширина, px", 2034., 0., 8., 0.1),
            (10, "Глубина, %", 2084., 0.01, 100., 0.1),
            (11, "Угол, °", 2134., 1., 179., 1.),
        ] {
            u.number_field(
                [x, y, width, 42.],
                label,
                render_number(settings, field).unwrap() as f64,
                Action::SurfaceRenderNumber(field),
                SliderRange::new(min, max, step),
            );
        }
        u.color_field(
            [x, 2188., width, 30.],
            "Цвет линии",
            Action::RenderColor(5),
            settings.npr_contours.color.map(linear_to_srgb),
        );
        u.fitted_label(
            [x, 2232., width, 18.],
            "Ширина 0 — контур выключен",
            10.,
            p.muted,
        );
        u.label(x, 652., "КОНТАКТНОЕ ЗАТЕНЕНИЕ · AO", 10., p.muted);
        u.number_field(
            [x, 678., width, 30.],
            "Сила · 0 = выкл.",
            settings.ambient_occlusion as f64,
            Action::SurfaceRenderNumber(5),
            SliderRange::new(0., 2., 0.01),
        );
        u.number_field(
            [x, 718., width, 30.],
            "Радиус, м",
            settings.ambient_occlusion_radius as f64,
            Action::SurfaceRenderNumber(6),
            SliderRange::new(0.01, 2., 0.01),
        );
        u.fitted_label(
            [x, 763., width, 16.],
            "Затемняет только свет окружения.",
            10.,
            p.muted,
        );
    }

    #[cfg(test)]
    pub(super) fn draw_fluid_settings(&self, u: &mut Ui, rx: f32, rw: f32) {
        self.draw_fluid_studio(u, rx + 16., rw - 32.);
        let p = u.palette();
        let x = rx + 16.;
        let width = rw - 32.;
        let settings = self.editor.project.fluid_settings;
        u.label(x, 171., "СИМУЛЯЦИЯ ЖИДКОСТИ", 11., p.muted);
        u.fitted_label(
            [x, 196., width, 18.],
            &format!(
                "{} частиц · {:.2} с",
                self.fluid.positions().len(),
                self.fluid.time()
            ),
            11.,
            p.text,
        );
        u.checkbox(
            [x, 222., width - 38., 32.],
            "Жидкость в таймлайне",
            Action::Fluid,
            self.fluid_on,
        );
        u.icon_button(
            [x + width - 32., 222., 32., 32.],
            "",
            Action::ResetFluid,
            false,
            Icon::Start,
        );
        u.icon_button(
            [x, 266., width * 0.5 - 3., 32.],
            "Область",
            Action::FluidFrame,
            false,
            Icon::Eye,
        );
        u.button(
            [x + width * 0.5 + 3., 266., width * 0.5 - 3., 32.],
            if self.fluid_preview.baking.is_some() {
                "Отмена"
            } else {
                "Запечь"
            },
            if self.fluid_preview.baking.is_some() {
                Action::FluidBakeCancel
            } else {
                Action::FluidBake
            },
            false,
        );
        u.button(
            [x + width - 66., 354., 66., 24.],
            "Сброс",
            Action::FluidCacheClear,
            false,
        );
        u.fitted_label(
            [x, 355., width - 72., 18.],
            &self.fluid_preview.baking.map_or_else(
                || {
                    format!(
                        "Кэш: {} · {:.1} МиБ",
                        self.fluid_preview.cache.frames.len(),
                        self.fluid_preview.cache.bytes as f64 / 1048576.
                    )
                },
                |(done, total)| format!("Запекание: {done}/{total}"),
            ),
            10.,
            p.muted,
        );
        u.button(
            [x, 310., width, 32.],
            "Снимок поверхности",
            Action::FluidSurface,
            false,
        );
        u.button(
            [x, 382., width * 0.5 - 3., 24.],
            "Сохранить",
            Action::FluidCacheSave,
            false,
        );
        u.button(
            [x + width * 0.5 + 3., 382., width * 0.5 - 3., 24.],
            "Загрузить",
            Action::FluidCacheLoad,
            false,
        );
        u.label(x, 418., "ПРОФИЛЬ", 11., p.muted);
        choice_row(
            u,
            [x, 440., width, 30.],
            [
                (
                    "Вода",
                    Action::FluidPreset(0),
                    fluid_preset_selected(settings, 0),
                ),
                (
                    "Густая",
                    Action::FluidPreset(1),
                    fluid_preset_selected(settings, 1),
                ),
            ],
        );
        for (field, label, min, max, step) in [
            (0, "Гравитация, м/с²", 0., 30., 0.1),
            (1, "Вязкость", 0., 0.2, 0.005),
            (2, "Вихри", 0., 5., 0.01),
            (3, "Трение стенок", 0., 1., 0.01),
            (4, "Итерации", 1., 16., 1.),
            (5, "Подшаги, макс.", 1., 8., 1.),
            (6, "Скорость времени", 0.1, 2., 0.05),
            (7, "Ячейка, м", 0.025, 0.12, 0.001),
            (8, "Порог поверхности", 0.1, 1.2, 0.01),
        ] {
            u.number_field(
                [x, 488. + field as f32 * 42., width, 30.],
                label,
                fluid_number(settings, field).unwrap_or_default() as f64,
                Action::FluidNumber(field),
                SliderRange::new(min, max, step),
            );
        }
        u.fitted_label(
            [x, 876., width, 16.],
            "Изменение физики начинает",
            10.,
            p.muted,
        );
        u.fitted_label(
            [x, 892., width, 16.],
            "расчёт заново после применения.",
            10.,
            p.muted,
        );
        u.button(
            [x, 924., width, 32.],
            "Сбросить параметры",
            Action::FluidResetSettings,
            false,
        );
        u.fitted_label(
            [x, 972., width, 16.],
            "Столкновения: область и сфера.",
            10.,
            p.muted,
        );
        let report = &self.fluid_preview.report;
        u.fitted_label(
            [x, 996., width, 16.],
            &format!(
                "Подшагов: {} · очередь {:.2} с",
                report.substeps, report.backlog_seconds
            ),
            10.,
            p.muted,
        );
        if report.substep_limit_reached {
            u.fitted_label(
                [x, 1018., width, 16.],
                "Достигнут лимит подшагов.",
                10.,
                p.warn,
            );
        } else if self.fluid_edit_active() {
            u.fitted_label(
                [x, 1018., width, 16.],
                "Ввод параметра: расчёт на паузе.",
                10.,
                p.muted,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Drag, Preferences};
    use super::*;
    use crate::ui::Theme;
    use poseforge_core::{Editor, EditorMode};
    use winit::event::MouseButton;

    fn settings_app(tab: u8) -> App {
        let mut app = App::new(None);
        app.preferences = Preferences::default();
        app.preferences.left_panel = true;
        app.preferences.right_panel = true;
        app.editor = Editor::new(Project::demo());
        app.editor.mode = EditorMode::Scene;
        if tab == 4 {
            app.create_primitive(0);
            app.fluid_object_action(crate::ui::FluidObjectAction::Attach);
            app.wait_for_background_jobs();
            let selected = app.editor.selected;
            app.editor = Editor::new(app.editor.project.clone());
            app.editor.selected = selected;
            app.height = 1200.;
            app.primitive_panel = None;
        }
        app.saved_snapshot = app.editor.project.clone();
        app.dirty = false;
        app.surface.tab = tab;
        app
    }

    fn replace_draft(app: &mut App, value: &str) {
        let input = app.input.as_mut().expect("The field owns text input");
        input.select_all();
        input.insert(value);
    }

    fn edit_field(app: &mut App, action: Action, value: &str) {
        app.action(action.clone());
        assert_eq!(app.input.as_ref().unwrap().purpose, action);
        replace_draft(app, value);
        app.action(Action::ConfirmInput);
    }

    fn begin_visible_slider(app: &mut App, action: Action) -> [f32; 4] {
        app.ui = app.build_ui();
        let (rect, range, value) = app
            .ui
            .hits
            .iter()
            .rev()
            .find_map(|hit| match &hit.action {
                Action::Slide(field, range, value) if **field == action => {
                    Some((hit.rect, *range, *value))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Expected a visible slider for {action:?}"));
        app.mouse_move([
            rect[0] + rect[2] * range.ratio(value),
            rect[1] + rect[3] * 0.5,
        ]);
        app.mouse_down(MouseButton::Left);
        assert!(matches!(app.drag, Some(Drag::Slider(_))));
        rect
    }

    #[test]
    fn quality_presets_leave_authored_environment_and_output_unchanged() {
        let settings = RenderSettings {
            exposure: 2.,
            ambient_intensity: 0.3,
            tone_mapping: ToneMapping::Linear,
            output_width: 2048,
            output_height: 1024,
            transparent_background: true,
            sky_color: [0.8, 0.4, 0.2],
            ground_color: [0.05; 3],
            background_color: [0.2, 0.3, 0.4],
            shadow_bias: 0.005,
            shadow_strength: 0.4,
            ..RenderSettings::default()
        };
        for quality in 0..3 {
            let mut changed = settings;
            assert!(apply_quality(&mut changed, quality));
            changed.validate().unwrap();
            assert!(quality_selected(changed, quality));
            changed.antialiasing = settings.antialiasing;
            changed.shadows = settings.shadows;
            changed.shadow_resolution = settings.shadow_resolution;
            changed.ambient_occlusion = settings.ambient_occlusion;
            assert_eq!(changed, settings);
        }
    }

    #[test]
    fn fluid_presets_preserve_surface_quality_and_playback_speed() {
        let original = FluidSettings {
            time_scale: 0.5,
            surface_cell_size: 0.03,
            surface_iso_density: 0.7,
            ..FluidSettings::default()
        };
        for preset in 0..2 {
            let mut settings = original;
            assert!(apply_fluid_preset(&mut settings, preset));
            settings.validate().unwrap();
            assert_eq!(settings.time_scale, original.time_scale);
            assert_eq!(settings.surface_cell_size, original.surface_cell_size);
            assert_eq!(settings.surface_iso_density, original.surface_iso_density);
        }
        let mut unsupported = original;
        assert!(!apply_fluid_preset(&mut unsupported, 2));
        assert_eq!(unsupported, original);
    }

    #[test]
    fn light_presets_are_valid_and_have_a_shadow_casting_key() {
        for preset in 0..3 {
            let (lights, sky, ground) = lighting_preset(preset).unwrap();
            assert_eq!(lights.len(), 3);
            assert_eq!(lights[0].kind, LightKind::Directional);
            assert!(lights[0].enabled && lights[0].intensity > 0.);
            Project {
                lights,
                render_settings: RenderSettings {
                    sky_color: sky,
                    ground_color: ground,
                    ..RenderSettings::default()
                },
                ..Project::default()
            }
            .validate()
            .unwrap();
        }
    }

    #[test]
    fn inline_render_numbers_commit_once_and_reject_nonfinite_or_out_of_range_values() {
        let mut app = settings_app(3);
        let before = app.editor.project.clone();
        edit_field(&mut app, Action::SurfaceRenderNumber(2), "0,0035");
        assert!(app.input.is_none());
        assert_eq!(app.editor.project.render_settings.shadow_bias, 0.0035);
        assert_eq!(app.editor.undo_len(), 1);
        assert!(app.dirty);
        assert!(app.editor.undo());
        assert_eq!(app.editor.project, before);
        app.update_dirty();

        for (field, invalid) in [(0, "NaN"), (0, "17"), (1, "-0.1"), (2, "0.021"), (3, "1.1")] {
            edit_field(&mut app, Action::SurfaceRenderNumber(field), invalid);
            assert_eq!(app.input.as_ref().unwrap().value, invalid);
            assert!(app.surface.input_error.is_some());
            assert_eq!(app.editor.project, before);
            assert_eq!(app.editor.undo_len(), 0);
            assert!(!app.dirty);
            app.action(Action::Cancel);
        }
        assert!(app
            .scene_settings_confirm(Action::SurfaceRenderNumber(99), "1")
            .is_err());
        assert!(app
            .scene_settings_confirm(Action::RenderColor(6), "#FFFFFF")
            .is_err());
        assert_eq!(app.editor.project, before);
    }

    #[test]
    fn inline_environment_hex_preserves_precision_until_changed_and_undo_restores_it() {
        let mut app = settings_app(3);
        app.editor.project.render_settings.sky_color = [1.75, 0.300012, 0.122233];
        app.saved_snapshot = app.editor.project.clone();
        let before = app.editor.project.clone();
        app.action(Action::RenderColor(0));
        app.action(Action::ConfirmInput);
        assert!(app.input.is_none());
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), 0);
        assert!(!app.dirty);

        edit_field(&mut app, Action::RenderColor(0), "#68A0D8");
        assert!(app.input.is_none());
        assert_eq!(
            app.editor.project.render_settings.sky_color,
            parse_color("#68A0D8").unwrap()
        );
        assert_eq!(
            app.editor.project.render_settings.ground_color,
            before.render_settings.ground_color
        );
        assert_eq!(app.editor.undo_len(), 1);
        assert!(app.editor.undo());
        assert_eq!(app.editor.project, before);
        app.update_dirty();

        for invalid in ["#GGFFFF", "#12345", "NaN", "#112233FF"] {
            edit_field(&mut app, Action::RenderColor(0), invalid);
            assert_eq!(app.input.as_ref().unwrap().value, invalid);
            assert!(app.surface.input_error.is_some());
            assert_eq!(app.editor.project, before);
            assert_eq!(app.editor.undo_len(), 0);
            app.action(Action::Cancel);
        }
    }

    #[test]
    fn quality_and_lighting_actions_preserve_geometry_and_output_in_one_transaction() {
        for action in [
            Action::RenderQuality(0),
            Action::RenderQuality(1),
            Action::RenderQuality(2),
            Action::LightingPreset(0),
            Action::LightingPreset(1),
            Action::LightingPreset(2),
        ] {
            let mut app = settings_app(3);
            app.editor.project.render_settings = RenderSettings {
                antialiasing: 1,
                shadows: true,
                shadow_resolution: 2048,
                exposure: 1.7,
                ambient_intensity: 0.4,
                output_width: 2048,
                output_height: 1024,
                transparent_background: true,
                tone_mapping: ToneMapping::Reinhard,
                background_color: [0.2, 0.1, 0.05],
                ..RenderSettings::default()
            };
            let before = app.editor.project.clone();
            app.action(action.clone());
            let after = app.editor.project.clone();
            assert_ne!(after, before);
            let mut untouched = after.clone();
            match action {
                Action::RenderQuality(_) => {
                    untouched.render_settings.antialiasing = before.render_settings.antialiasing;
                    untouched.render_settings.shadows = before.render_settings.shadows;
                    untouched.render_settings.ambient_occlusion =
                        before.render_settings.ambient_occlusion;
                    untouched.render_settings.shadow_resolution =
                        before.render_settings.shadow_resolution;
                }
                Action::LightingPreset(_) => {
                    untouched.lights = before.lights.clone();
                    untouched.render_settings.sky_color = before.render_settings.sky_color;
                    untouched.render_settings.ground_color = before.render_settings.ground_color;
                }
                _ => unreachable!(),
            }
            assert_eq!(untouched, before);
            assert_eq!(app.editor.undo_len(), 1);
            assert!(app.editor.undo());
            assert_eq!(app.editor.project, before);
            assert!(app.editor.redo());
            assert_eq!(app.editor.project, after);
        }
    }

    #[test]
    fn fluid_integer_sliders_commit_one_edit_and_cancel_restores_parameters() {
        for (field, maximum) in [(4, 16), (5, 8)] {
            let mut app = settings_app(4);
            let before = app.editor.project.clone();
            let rect = begin_visible_slider(&mut app, Action::FluidObject(crate::ui::FluidObjectAction::Field(field)));
            for fraction in [0.6, 0.8, 1.] {
                app.mouse_move([rect[0] + rect[2] * fraction, rect[1] + rect[3] * 0.5]);
                assert_eq!(app.editor.undo_len(), 0);
            }
            assert_eq!(
                super::super::fluid_object_controls::value(app.selected_fluid().unwrap().1, field),
                Some(maximum as f64)
            );
            app.mouse_up(MouseButton::Left);
            assert!(app.drag.is_none());
            assert_eq!(app.editor.undo_len(), 1);
            assert!(app.editor.undo());
            assert_eq!(app.editor.project, before);

            let rect = begin_visible_slider(&mut app, Action::FluidObject(crate::ui::FluidObjectAction::Field(field)));
            app.mouse_move([rect[0] + rect[2] * 0.85, rect[1] + rect[3] * 0.5]);
            assert_ne!(app.editor.project, before);
            app.action(Action::Cancel);
            assert!(app.drag.is_none());
            assert_eq!(app.editor.project, before);
            assert_eq!(app.editor.undo_len(), 0);
            assert!(!app.dirty);
        }
    }

    #[test]
    fn fluid_fields_reject_fractional_integers_and_unsafe_values_without_losing_focus() {
        let mut app = settings_app(4);
        let before = app.editor.project.clone();
        for (field, invalid) in [
            (4, "4.5"),
            (4, "0"),
            (4, "17"),
            (5, "9"),
            (5, "-1"),
            (0, "NaN"),
            (1, "0.21"),
            (2, "-0.01"),
            (3, "1.01"),
            (6, "0"),
            (7, "0.024"),
            (8, "1.21"),
        ] {
            edit_field(&mut app, Action::FluidObject(crate::ui::FluidObjectAction::Field(field)), invalid);
            assert_eq!(
                app.input.as_ref().unwrap().purpose,
                Action::FluidObject(crate::ui::FluidObjectAction::Field(field))
            );
            assert_eq!(app.input.as_ref().unwrap().value, invalid);
            assert!(app.surface.input_error.is_some());
            assert_eq!(app.editor.project, before);
            assert_eq!(app.editor.undo_len(), 0);
            assert!(!app.dirty);
            app.action(Action::Cancel);
        }
        assert!(app
            .fluid_object_confirm(&Action::FluidObject(crate::ui::FluidObjectAction::Field(10)), "1")
            .is_err());
        edit_field(&mut app, Action::FluidObject(crate::ui::FluidObjectAction::Field(4)), "6");
        assert!(app.input.is_none());
        assert_eq!(app.selected_fluid().unwrap().1.simulation.iterations, 6);
        assert_eq!(app.editor.undo_len(), 1);
        assert!(app.editor.undo());
        assert_eq!(app.editor.project, before);
    }

    #[test]
    fn png_output_fields_and_transparency_are_persistent_undoable_project_edits() {
        let mut app = settings_app(3);
        let before = app.editor.project.clone();
        edit_field(&mut app, Action::EditWidth, "2048");
        edit_field(&mut app, Action::EditHeight, "1024");
        app.action(Action::Transparent);
        let after = app.editor.project.clone();
        assert_eq!(after.render_settings.output_width, 2048);
        assert_eq!(after.render_settings.output_height, 1024);
        assert!(after.render_settings.transparent_background);
        assert_eq!(app.editor.undo_len(), 3);
        let stored: Project = serde_json::from_slice(&serde_json::to_vec(&after).unwrap()).unwrap();
        assert_eq!(stored.render_settings, after.render_settings);
        for _ in 0..3 {
            assert!(app.editor.undo());
        }
        assert_eq!(app.editor.project, before);
        app.update_dirty();
        for (action, invalid) in [
            (Action::EditWidth, "63"),
            (Action::EditHeight, "4097"),
            (Action::EditWidth, "128.5"),
        ] {
            edit_field(&mut app, action, invalid);
            assert!(app.input.is_some());
            assert!(app.surface.input_error.is_some());
            assert_eq!(app.editor.project, before);
            assert_eq!(app.editor.undo_len(), 0);
            app.action(Action::Cancel);
        }
    }

    #[test]
    fn both_panels_fit_standard_inspector_widths_in_each_theme() {
        let app = settings_app(3);
        for width in [258., 288.] {
            for theme in [Theme::Dark, Theme::Light] {
                for tab in [3, 4] {
                    let mut ui = Ui::new(1.);
                    ui.theme = theme;
                    let bottom = if tab == 3 {
                        app.draw_render_settings(&mut ui, 0., width);
                        RENDER_PANEL_BOTTOM
                    } else {
                        app.draw_fluid_settings(&mut ui, 0., width);
                        FLUID_PANEL_BOTTOM
                    };
                    for hit in &ui.hits {
                        assert!(hit.rect.iter().all(|value| value.is_finite()));
                        assert!(
                            hit.rect[0] >= 16. && hit.rect[0] + hit.rect[2] <= width - 16. + 0.001,
                            "Control exceeds the inspector: {:?}",
                            hit.action
                        );
                        assert!(hit.rect[1] >= 171. && hit.rect[1] + hit.rect[3] <= bottom);
                    }
                    for text in &ui.texts {
                        // Compact captions may elide; their right-aligned numeric value must remain readable.
                        if text.text.contains('…') {
                            assert!(ui.hits.iter().any(|h| matches!(h.action, Action::Slide(..))
                                && text.y >= h.rect[1] && text.y < h.rect[1] + h.rect[3]),
                                "Only numeric captions may elide: {}", text.text);
                        }
                        assert!(text.x >= 16. && text.x < width - 16.);
                        assert!(text.y >= 171. && text.y + text.size <= bottom);
                    }
                }
            }
        }
    }
}
