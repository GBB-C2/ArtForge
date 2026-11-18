//! Cable controls share numeric input, capture, reset and the construction transaction.
use super::*;
use crate::ui::ConstructionAction as C;
use poseforge_core::{CableKind as K, CableSettings};
fn shape_fields(kind: K) -> Vec<u8> {
    let mut fields = match kind {
        K::Ribbon => vec![20, 21],
        K::Twisted => vec![6, 15, 16, 17],
        K::Coiled => vec![6, 16, 17],
        K::Corrugated => vec![6, 18, 19],
        K::Bundle => vec![6, 15, 17],
        _ => vec![6],
    };
    fields.extend([7, 13, 8, 14, 22, 24]);
    fields
}
pub(super) fn field_default(i: u8) -> Option<f64> {
    let c = CableSettings::default();
    Some(match i {
        0..=2 => c.start[i as usize] as f64,
        3..=5 => c.end[(i - 3) as usize] as f64,
        6 => c.radius as f64,
        7 => c.segments as f64,
        8 => c.sag as f64 * 100.,
        9 => 9.81,
        10..=12 => 0.,
        13 => c.sides as f64,
        14 => 0.,
        15 => c.strands as f64,
        16 => c.turns as f64,
        17 => c.spread as f64,
        18 => c.ribs as f64,
        19 => c.groove as f64 * 100.,
        20 => c.width as f64,
        21 => c.thickness as f64,
        22 => 100.,
        23 => 0.,
        24 => 1.,
        25 => (100. - 30.) / 0.99,
        26 => 0.01,
        27 => 3.,
        28 => 35.,
        29 => 0.,
        30 => 16.,
        _ => return None,
    })
}
pub(super) fn set_field(s: &mut construction_tools::Session, i: u8, v: f32) {
    match i {
        13 => s.cable.sides = v as u32,
        14 => s.cable.rotation = v,
        15 => s.cable.strands = v as u32,
        16 => s.cable.turns = v,
        17 => s.cable.spread = v,
        18 => s.cable.ribs = v as u32,
        19 => s.cable.groove = v / 100.,
        20 => s.cable.width = v,
        21 => s.cable.thickness = v,
        22 => s.cable.taper = v / 100.,
        23 => s.cable.collision_margin = v,
        24 => s.cable.uv_repeat = v,
        25 => s.physics.bending = 100. - 0.99 * v,
        26 => s.physics.compliance = v / 10000.,
        27 => s.physics.damping = v / 100.,
        28 => s.physics.friction = v / 100.,
        29 => s.physics.wind = v,
        30 => s.physics.iterations = v as u32,
        _ => {}
    }
}
impl App {
    pub(super) fn cable_extended_field(&self, i: u8) -> Option<(f32, f32, f32, &'static str)> {
        let s = self.construction.session.as_ref()?;
        if s.kind != construction_tools::Kind::Cable {
            return None;
        }
        let c = s.cable;
        Some(match i {
            13 => (c.sides as f32, 3., 16., "Сторон сечения"),
            14 => (c.rotation, -360., 360., "Поворот сечения, °"),
            15 => (c.strands as f32, 2., 6., "Количество жил"),
            16 => (c.turns, -32., 32., "Витков · знак = направление"),
            17 => (
                c.spread,
                0.002,
                1.,
                if c.kind == K::Coiled {
                    "Радиус спирали, м"
                } else {
                    "Разнос жил, м"
                },
            ),
            18 => (c.ribs as f32, 1., 64., "Колец гофры"),
            19 => (c.groove * 100., 0., 90., "Глубина гофры, %"),
            20 => (c.width, 0.002, 2., "Ширина, м"),
            21 => (c.thickness, 0.001, 0.5, "Толщина, м"),
            22 => (c.taper * 100., 5., 200., "Размер на конце, %"),
            23 => (c.collision_margin, 0., 0.1, "Зазор столкновений, м"),
            24 => (c.uv_repeat, 0.1, 100., "Повторы текстуры"),
            25 => (
                (100. - s.physics.bending) / 0.99,
                0.,
                100.,
                "Жёсткость изгиба, %",
            ),
            26 => (s.physics.compliance * 10000., 0., 100., "Растяжимость, %"),
            27 => (s.physics.damping * 100., 0., 100., "Затухание, %"),
            28 => (s.physics.friction * 100., 0., 100., "Трение, %"),
            29 => (s.physics.wind, -30., 30., "Ветер"),
            30 => (s.physics.iterations as f32, 1., 24., "Итерации физики"),
            _ => return None,
        })
    }
    pub(super) fn construction_tabs_height(&self) -> f32 {
        if self
            .construction
            .session
            .as_ref()
            .is_some_and(|s| s.kind == construction_tools::Kind::Cable)
        {
            38.
        } else {
            0.
        }
    }
    pub(super) fn construction_content_height(&self) -> f32 {
        let Some(s) = &self.construction.session else {
            return 0.;
        };
        if s.kind == construction_tools::Kind::Boolean {
            return self.boolean_content_height();
        }
        match self.construction.cable_tab {
            0 => 36. + shape_fields(s.cable.kind).len() as f32 * 44. + 62.,
            1 => 254.,
            _ => 36. + 8. * 44. + 66.,
        }
    }
    pub(super) fn cable_number(&self, u: &mut Ui, x: f32, y: f32, w: f32, i: u8) {
        if let Some((v, min, max, label)) = self.construction_field(i) {
            let step = match i {
                7 | 13 | 15 | 18 | 30 => 1.,
                14 | 16 | 24 | 25 | 27 | 28 | 29 => 0.1,
                26 => 0.01,
                _ => 0.001,
            };
            u.number_field(
                [x, y, w, 38.],
                label,
                if i == 25 {
                    (v * 10.).round() as f64 / 10.
                } else {
                    v as f64
                },
                Action::Construction(C::Field(i)),
                SliderRange::new(min as f64, max as f64, step),
            );
        }
    }
    pub(super) fn draw_cable_type(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        let Some(s) = &self.construction.session else {
            return;
        };
        self.draw_dropdown_button(
            u,
            [x, y, w, 30.],
            ui::DropdownKind::CableType,
            s.cable.kind.label(),
        );
    }
    pub(super) fn draw_cable_controls(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let Some(s) = &self.construction.session else {
            return;
        };
        let p = u.palette();
        match self.construction.cable_tab {
            0 => {
                self.draw_cable_type(u,x,y,w);
                y += 36.;
                for i in shape_fields(s.cable.kind) {
                    self.cable_number(u, x, y, w, i);
                    y += 44.;
                }
                u.checkbox(
                    [x, y, w, 26.],
                    "Закрыть концы",
                    Action::Construction(C::CableCaps),
                    s.cable.caps,
                );
                y += 32.;
                u.fitted_label(
                    [x, y, w, 22.],
                    &format!("Сегментов с учётом деталей: {}", s.cable.sample_segments()),
                    10.,
                    p.muted,
                );
            }
            1 => {
                for end in 0..2 {
                    u.quiet_button(
                        [x, y, w, 28.],
                        if end == 0 {
                            "Указать начало в сцене"
                        } else {
                            "Указать конец в сцене"
                        },
                        Action::Construction(C::Pick(end)),
                        s.pick == Some(end),
                    );
                    y += 32.;
                    let cell = (w - 8.) / 3.;
                    for axis in 0..3 {
                        let i = end * 3 + axis;
                        let (v, _, _, _) = self.construction_field(i).unwrap();
                        u.number_field(
                            [x + axis as f32 * (cell + 4.), y, cell, 38.],
                            ["X", "Y", "Z"][axis as usize],
                            v as f64,
                            Action::Construction(C::Field(i)),
                            SliderRange::new(-10., 10., 0.01),
                        );
                    }
                    y += 44.;
                    u.checkbox(
                        [x, y, w, 26.],
                        if end == 0 {
                            "Закрепить начало"
                        } else {
                            "Закрепить конец"
                        },
                        Action::Construction(C::Pin(end)),
                        if end == 0 {
                            s.cable.pin_start
                        } else {
                            s.cable.pin_end
                        },
                    );
                    y += 32.;
                }
                u.quiet_button(
                    [x, y, w, 28.],
                    "Поменять концы местами",
                    Action::Construction(C::SwapEnds),
                    false,
                );
            }
            _ => {
                let cell = (w - 8.) / 3.;
                for (i, label) in ["Мягкий", "Жёсткий", "Упругий"].into_iter().enumerate()
                {
                    let active = match i {
                        0 => s.physics.compliance == 0.000001 && s.physics.bending == 60.,
                        1 => s.physics.compliance == 0. && s.physics.bending == 1.,
                        _ => s.physics.compliance == 0.0002 && s.physics.bending == 40.,
                    };
                    u.quiet_button(
                        [x + i as f32 * (cell + 4.), y, cell, 28.],
                        label,
                        Action::Construction(C::CablePreset(i as u8)),
                        active,
                    );
                }
                y += 36.;
                for i in [9, 25, 26, 27, 28, 29, 30, 23] {
                    self.cable_number(u, x, y, w, i);
                    y += 44.;
                }
                u.checkbox(
                    [x, y, w, 26.],
                    "Столкновения с объектами",
                    Action::Construction(C::Collisions),
                    s.collisions,
                );
                y += 32.;
                u.quiet_button(
                    [x, y, w, 30.],
                    if self.studio.physics_running {
                        "Остановить физику"
                    } else {
                        "Предпросмотр физики"
                    },
                    Action::Construction(C::Play),
                    self.studio.physics_running,
                );
            }
        }
    }
}
