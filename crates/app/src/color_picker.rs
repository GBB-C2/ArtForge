//! A transactional sRGB/HSV popover. App owns focus, preview and document history.
//! Geometry and pointer values are logical; cached quads use physical pixels.
use crate::icons::Icon;
use crate::ui::{contains, Action, Hit, SliderRange, Ui};
use poseforge_render::UiRect;
use std::f32::consts::{PI, SQRT_2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorControl {
    Apply,
    Cancel,
    Hex,
    Hue,
    Saturation,
    Value,
    Alpha,
    Wheel,
    Sv,
    RestoreOriginal,
}

impl ColorControl {
    pub fn is_field(self) -> bool {
        matches!(
            self,
            Self::Hex | Self::Hue | Self::Saturation | Self::Value | Self::Alpha
        )
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Apply => "Применить цвет одним действием · Enter",
            Self::Cancel => "Вернуть исходный цвет и закрыть панель · Esc",
            Self::Hex => {
                "sRGB #RRGGBB; для материала также #RRGGBBAA · шесть цифр сохраняют непрозрачность"
            }
            Self::Hue => "H — оттенок, 0–360° · 360° совпадает с 0°",
            Self::Saturation => "S — насыщенность, 0–100% · серый сохраняет выбранный оттенок",
            Self::Value => "V — яркость HSV, 0–100% · чёрный сохраняет оттенок и насыщенность",
            Self::Alpha => "Непрозрачность, 0–100% · RGB сохраняется и при 0%",
            Self::Wheel => {
                "Потяните кольцо оттенка · точное значение — поле H · Esc отменяет весь выбор"
            }
            Self::Sv => "По горизонтали — насыщенность, вверх — яркость · точные значения — S и V",
            Self::RestoreOriginal => "Вернуть исходный цвет, непрозрачность и положение маркеров",
        }
    }
}

/// HSV of bounded, encoded sRGB, not linear RGB. Hue increases clockwise on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        0.
    }
}

fn hue(value: f32) -> f32 {
    if value.is_finite() {
        let wrapped = value.rem_euclid(360.);
        // f32 rounding can return the divisor for a tiny negative dividend.
        if wrapped >= 360. || wrapped == 0. {
            0.
        } else {
            wrapped
        }
    } else {
        0.
    }
}

/// Only the editing representation is bounded. The draft separately retains
/// original out-of-range document values until a real RGB edit is made.
pub fn rgb_to_hsv(rgb: [f32; 3], remembered_hue: f32) -> Hsv {
    let [r, g, b] = rgb.map(unit);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    let h = if chroma <= 1e-7 {
        hue(remembered_hue)
    } else if max == r {
        hue(60. * ((g - b) / chroma))
    } else if max == g {
        hue(60. * ((b - r) / chroma + 2.))
    } else {
        hue(60. * ((r - g) / chroma + 4.))
    };
    Hsv {
        hue: h,
        saturation: if max > 0. { chroma / max } else { 0. },
        value: max,
    }
}

pub fn hsv_to_rgb(hsv: Hsv) -> [f32; 3] {
    let h = hue(hsv.hue) / 60.;
    let s = unit(hsv.saturation);
    let v = unit(hsv.value);
    let chroma = v * s;
    let x = chroma * (1. - (h.rem_euclid(2.) - 1.).abs());
    let m = v - chroma;
    let rgb = match h as u8 {
        0 => [chroma, x, 0.],
        1 => [x, chroma, 0.],
        2 => [0., chroma, x],
        3 => [0., x, chroma],
        4 => [x, 0., chroma],
        _ => [chroma, 0., x],
    };
    rgb.map(|c| (c + m).clamp(0., 1.))
}

#[derive(Clone, Debug)]
pub struct ColorDraft {
    rgb: [f32; 3],
    alpha: f32,
    hsv: Hsv,
    original: [f32; 4],
    original_hsv: Hsv,
    alpha_enabled: bool,
}

impl ColorDraft {
    /// Finite HDR RGB is retained exactly. Nonfinite source values have no
    /// meaningful display color and are sanitized locally, never in the document.
    pub fn new(srgb: [f32; 4], alpha_enabled: bool) -> Self {
        let rgb = [srgb[0], srgb[1], srgb[2]].map(|c| if c.is_finite() { c } else { 0. });
        let alpha = if srgb[3].is_finite() { srgb[3] } else { 1. };
        let hsv = rgb_to_hsv(rgb, 0.);
        Self {
            rgb,
            alpha,
            hsv,
            original: [rgb[0], rgb[1], rgb[2], alpha],
            original_hsv: hsv,
            alpha_enabled,
        }
    }

    pub fn rgba(&self) -> [f32; 4] {
        [self.rgb[0], self.rgb[1], self.rgb[2], self.alpha]
    }

    pub fn original_rgba(&self) -> [f32; 4] {
        self.original
    }
    pub fn hsv(&self) -> Hsv {
        self.hsv
    }
    pub fn alpha_enabled(&self) -> bool {
        self.alpha_enabled
    }
    pub fn rgb_changed(&self) -> bool {
        self.rgb != [self.original[0], self.original[1], self.original[2]]
    }
    pub fn alpha_changed(&self) -> bool {
        self.alpha != self.original[3]
    }

    pub fn hex(&self) -> String {
        let [r, g, b] = self.rgb.map(byte);
        if self.alpha_enabled {
            format!("#{r:02X}{g:02X}{b:02X}{:02X}", byte(self.alpha))
        } else {
            format!("#{r:02X}{g:02X}{b:02X}")
        }
    }

    /// Numeric units match the visible fields and slider ranges: degrees and %.
    pub fn value(&self, control: ColorControl) -> Option<f32> {
        match control {
            ColorControl::Hue => Some(self.hsv.hue),
            ColorControl::Saturation => Some(self.hsv.saturation * 100.),
            ColorControl::Value => Some(self.hsv.value * 100.),
            ColorControl::Alpha if self.alpha_enabled => Some(unit(self.alpha) * 100.),
            _ => None,
        }
    }

    pub fn text(&self, control: ColorControl) -> String {
        if control == ColorControl::Hex {
            self.hex()
        } else if let Some(value) = self.value(control) {
            let text = format!("{value:.4}");
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            String::new()
        }
    }

    pub fn set_text(&mut self, control: ColorControl, text: &str) -> Result<bool, String> {
        if control == ColorControl::Hex {
            return self.set_hex(text);
        }
        if self.value(control).is_none() {
            return Err("Это поле не редактирует цвет.".into());
        }
        let text = text
            .trim()
            .trim_end_matches(['°', '%'])
            .trim()
            .replace(',', ".");
        let value = text
            .parse::<f32>()
            .map_err(|_| "Введите число.".to_string())?;
        if !value.is_finite() {
            return Err("Введите конечное число.".into());
        }
        // Accepting a rounded display caption must not change the float source.
        if self
            .text(control)
            .parse::<f32>()
            .is_ok_and(|shown| shown == value)
        {
            return Ok(false);
        }
        self.set_value(control, value)
    }

    pub fn set_value(&mut self, control: ColorControl, value: f32) -> Result<bool, String> {
        let max = if control == ColorControl::Hue {
            360.
        } else {
            100.
        };
        if !value.is_finite() || !(0. ..=max).contains(&value) {
            return Err(format!("Допустимый диапазон: 0…{max}."));
        }
        let mut next = self.hsv;
        match control {
            ColorControl::Hue => next.hue = hue(value),
            ColorControl::Saturation => next.saturation = value / 100.,
            ColorControl::Value => next.value = value / 100.,
            ColorControl::Alpha if self.alpha_enabled => {
                let next = value / 100.;
                let changed = self.alpha != next;
                self.alpha = next;
                return Ok(changed);
            }
            _ => return Err("Это поле не редактирует цвет.".into()),
        }
        Ok(self.set_hsv(next))
    }

    fn set_hsv(&mut self, next: Hsv) -> bool {
        if next == self.hsv {
            return false;
        }
        self.hsv = next;
        self.rgb = hsv_to_rgb(next);
        true
    }

    fn set_hex(&mut self, text: &str) -> Result<bool, String> {
        let text = text.trim();
        let text = text.strip_prefix('#').unwrap_or(text);
        if !matches!(text.len(), 6 | 8)
            || (text.len() == 8 && !self.alpha_enabled)
            || !text.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(if self.alpha_enabled {
                "Введите #RRGGBB или #RRGGBBAA (альфа в конце).".into()
            } else {
                "Введите цвет sRGB #RRGGBB.".into()
            });
        }
        let channel = |index: usize| u8::from_str_radix(&text[index..index + 2], 16).unwrap();
        let rgb = [channel(0), channel(2), channel(4)];
        let mut changed = false;
        // Compare displayed bytes first: Enter on the displayed HEX is a no-op,
        // including HDR or float colors that are not exactly on an 8-bit grid.
        if rgb != self.rgb.map(byte) {
            self.rgb = rgb.map(|c| c as f32 / 255.);
            let mut next = rgb_to_hsv(self.rgb, self.hsv.hue);
            if next.value == 0. {
                next.saturation = self.hsv.saturation;
            }
            self.hsv = next;
            changed = true;
        }
        if text.len() == 8 {
            let alpha = channel(6);
            if alpha != byte(self.alpha) {
                self.alpha = alpha as f32 / 255.;
                changed = true;
            }
        }
        Ok(changed)
    }

    pub fn restore_original(&mut self) -> bool {
        let changed = self.rgb_changed() || self.alpha_changed() || self.hsv != self.original_hsv;
        self.rgb = [self.original[0], self.original[1], self.original[2]];
        self.alpha = self.original[3];
        self.hsv = self.original_hsv;
        changed
    }
}

fn byte(value: f32) -> u8 {
    (unit(value) * 255.).round() as u8
}

#[derive(Clone, Copy, Debug)]
pub struct ColorLayout {
    pub panel: [f32; 4],
    pub wheel: [f32; 4],
    pub sv: [f32; 4],
    alpha_enabled: bool,
}

impl ColorLayout {
    pub fn anchored(anchor: [f32; 4], viewport: [f32; 2], alpha_enabled: bool) -> Self {
        if viewport.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return Self {
                panel: [0.; 4],
                wheel: [0.; 4],
                sv: [0.; 4],
                alpha_enabled,
            };
        }
        let margin = 8_f32.min(viewport[0] * 0.05).min(viewport[1] * 0.05);
        let width = 300_f32.min((viewport[0] - 2. * margin).max(0.));
        let height = (if alpha_enabled { 496_f32 } else { 460_f32 })
            .min((viewport[1] - 2. * margin).max(0.));
        let anchor = if valid_rect(anchor) {
            anchor
        } else {
            [margin, margin, 0., 0.]
        };
        let right = viewport[0] - margin - width;
        let bottom = viewport[1] - margin - height;
        let x = if anchor[0] + width <= viewport[0] - margin {
            anchor[0]
        } else {
            anchor[0] + anchor[2] - width
        }
        .clamp(margin, right.max(margin));
        let below = anchor[1] + anchor[3] + 6.;
        let above = anchor[1] - height - 6.;
        let y = if below <= bottom {
            below
        } else if above >= margin {
            above
        } else {
            anchor[1]
        }
        .clamp(margin, bottom.max(margin));
        let diameter = (height - 284. - if alpha_enabled { 36. } else { 0. })
            .min(width - 28.)
            .max(0.);
        let wheel = [x + (width - diameter) * 0.5, y + 50., diameter, diameter];
        let inner = (diameter * 0.5 - ring_width(diameter)).max(0.);
        let square = ((inner - 8.).max(0.) * SQRT_2).min(diameter);
        let sv = [
            wheel[0] + (diameter - square) * 0.5,
            wheel[1] + (diameter - square) * 0.5,
            square,
            square,
        ];
        Self {
            panel: [x, y, width, height],
            wheel,
            sv,
            alpha_enabled,
        }
    }

    fn usable(self) -> bool {
        valid_rect(self.panel)
            && valid_rect(self.wheel)
            && valid_rect(self.sv)
            && self.panel[2] >= 220.
            && self.wheel[2] >= 72.
            && self.wheel[2] <= 256.
            && self.sv[2] >= 24.
    }

    /// Only the actual annulus and SV square begin drags; their gap is inert.
    pub fn drag_at(self, point: [f32; 2]) -> Option<ColorControl> {
        if !self.usable() || point.iter().any(|v| !v.is_finite()) {
            return None;
        }
        if contains(self.sv, point) {
            return Some(ColorControl::Sv);
        }
        let center = self.center();
        let distance = (point[0] - center[0]).hypot(point[1] - center[1]);
        let radius = self.wheel[2] * 0.5;
        (distance >= radius - ring_width(self.wheel[2]) && distance <= radius)
            .then_some(ColorControl::Wheel)
    }

    fn center(self) -> [f32; 2] {
        [
            self.wheel[0] + self.wheel[2] * 0.5,
            self.wheel[1] + self.wheel[3] * 0.5,
        ]
    }

    pub fn hue_at(self, point: [f32; 2], previous: f32) -> f32 {
        if !self.usable() || point.iter().any(|v| !v.is_finite()) {
            return hue(previous);
        }
        let center = self.center();
        let x = point[0] as f64 - center[0] as f64;
        let y = point[1] as f64 - center[1] as f64;
        if x.hypot(y) < 1e-5 {
            return hue(previous);
        }
        hue(y.atan2(x).to_degrees() as f32)
    }

    pub fn hue_position(self, h: f32) -> [f32; 2] {
        let center = self.center();
        let angle = hue(h).to_radians();
        let radius = (self.wheel[2] - ring_width(self.wheel[2])) * 0.5;
        [
            center[0] + angle.cos() * radius,
            center[1] + angle.sin() * radius,
        ]
    }

    pub fn sv_at(self, point: [f32; 2], previous: Hsv) -> Hsv {
        if !self.usable() || point.iter().any(|v| !v.is_finite()) {
            return previous;
        }
        Hsv {
            hue: previous.hue,
            saturation: ((point[0] - self.sv[0]) / self.sv[2]).clamp(0., 1.),
            value: (1. - (point[1] - self.sv[1]) / self.sv[3]).clamp(0., 1.),
        }
    }

    pub fn sv_position(self, hsv: Hsv) -> [f32; 2] {
        [
            self.sv[0] + unit(hsv.saturation) * self.sv[2],
            self.sv[1] + (1. - unit(hsv.value)) * self.sv[3],
        ]
    }
}

fn valid_rect(rect: [f32; 4]) -> bool {
    rect.iter().all(|v| v.is_finite()) && rect[2] > 0. && rect[3] > 0.
}

fn ring_width(diameter: f32) -> f32 {
    (diameter * 0.1).clamp(10., 18.)
}

#[derive(Default)]
struct GradientCache {
    wheel_key: Option<(u32, u32)>,
    sv_key: Option<(u32, u32, u32)>,
    wheel: Vec<UiRect>,
    sv: Vec<UiRect>,
}

pub struct ColorPicker {
    pub draft: ColorDraft,
    pub notice: Option<String>,
    cache: GradientCache,
}

impl ColorPicker {
    pub fn new(srgb: [f32; 4], alpha_enabled: bool) -> Self {
        Self {
            draft: ColorDraft::new(srgb, alpha_enabled),
            notice: None,
            cache: GradientCache::default(),
        }
    }

    /// App keeps the initiating control through capture/release. Crossing the
    /// square while dragging the ring never changes the meaning of the gesture.
    pub fn drag(&mut self, control: ColorControl, layout: ColorLayout, point: [f32; 2]) -> bool {
        if !layout.usable() || point.iter().any(|v| !v.is_finite()) {
            return false;
        }
        let hsv = self.draft.hsv();
        match control {
            ColorControl::Wheel => self.draft.set_hsv(Hsv {
                hue: layout.hue_at(point, hsv.hue),
                ..hsv
            }),
            ColorControl::Sv => self.draft.set_hsv(layout.sv_at(point, hsv)),
            _ => false,
        }
    }

    pub fn draw(&mut self, ui: &mut Ui, layout: ColorLayout, title: &str, error: Option<&str>) {
        if !ui.scale.is_finite() || !(0.25..=8.).contains(&ui.scale) || !valid_rect(layout.panel) {
            return;
        }
        let p = ui.palette();
        let [x, y, width, height] = layout.panel;
        ui.outlined_surface(layout.panel, p.panel, p.border);
        if !layout.usable() || layout.alpha_enabled != self.draft.alpha_enabled {
            if width >= 100. && height >= 64. {
                ui.fitted_label(
                    [x + 8., y + 8., width - 16., 18.],
                    "Увеличьте окно для выбора цвета",
                    11.,
                    p.text,
                );
                ui.button(
                    [x + 8., y + height - 36., width - 16., 28.],
                    "Отмена",
                    Action::ColorPicker(ColorControl::Cancel),
                    false,
                );
            }
            return;
        }
        ui.fitted_label([x + 14., y + 10., width - 56., 18.], title, 13., p.text);
        ui.label(x + 14., y + 30., "sRGB · HSV", 11., p.muted);
        ui.quiet_icon_button(
            [x + width - 34., y + 7., 26., 26.],
            "",
            Action::ColorPicker(ColorControl::Cancel),
            false,
            Icon::Close,
        );

        self.cache.prepare(layout, ui.scale, self.draft.hsv.hue);
        append_cached(ui, &self.cache.wheel, [layout.wheel[0], layout.wheel[1]]);
        // Draw a neutral border behind the square, independent of its current hue.
        ui.rect(
            [
                layout.sv[0] - 1.,
                layout.sv[1] - 1.,
                layout.sv[2] + 2.,
                layout.sv[3] + 2.,
            ],
            p.border,
        );
        append_cached(ui, &self.cache.sv, [layout.sv[0], layout.sv[1]]);
        marker(
            ui,
            layout.hue_position(self.draft.hsv.hue),
            hsv_to_rgb(Hsv {
                hue: self.draft.hsv.hue,
                saturation: 1.,
                value: 1.,
            }),
        );
        marker(
            ui,
            layout.sv_position(self.draft.hsv),
            self.draft.rgb.map(unit),
        );
        ui.hits.push(Hit {
            rect: layout.wheel,
            action: Action::ColorPicker(ColorControl::Wheel),
        });
        ui.hits.push(Hit {
            rect: layout.sv,
            action: Action::ColorPicker(ColorControl::Sv),
        });
        // At component limits the handle extends outside the square. App checks
        // its circular radius before accepting the bounding-rectangle hit.
        let sv_marker = layout.sv_position(self.draft.hsv);
        ui.hits.push(Hit {
            rect: [sv_marker[0] - 6., sv_marker[1] - 6., 12., 12.],
            action: Action::ColorPicker(ColorControl::Sv),
        });

        let bottom = layout.wheel[1] + layout.wheel[3];
        let inner_width = width - 28.;
        let swatch_width = (inner_width - 8.) * 0.5;
        let old = [x + 14., bottom + 8., swatch_width, 26.];
        let new = [x + 22. + swatch_width, bottom + 8., swatch_width, 26.];
        ui.quiet_button(
            old,
            "Было",
            Action::ColorPicker(ColorControl::RestoreOriginal),
            false,
        );
        ui.surface(new, p.elevated);
        ui.label(new[0] + 8., new[1] + 5., "Стало", 12., p.text);
        for (rect, rgba) in [(old, self.draft.original_rgba()), (new, self.draft.rgba())] {
            draw_swatch(ui, [rect[0] + 54., rect[1] + 3., rect[2] - 60., 20.], rgba);
        }
        ui.label(x + 14., bottom + 48., "HEX", 12., p.muted);
        ui.field(
            [x + 52., bottom + 42., width - 66., 28.],
            &self.draft.hex(),
            Action::ColorPicker(ColorControl::Hex),
        );
        for (index, control, caption, maximum, step) in [
            (0, ColorControl::Hue, "H · Оттенок, °", 360., 0.1),
            (
                1,
                ColorControl::Saturation,
                "S · Насыщенность, %",
                100.,
                0.1,
            ),
            (2, ColorControl::Value, "V · Яркость, %", 100., 0.1),
        ] {
            ui.number_field(
                [x + 14., bottom + 76. + index as f32 * 34., inner_width, 28.],
                caption,
                self.draft.value(control).unwrap() as f64,
                Action::ColorPicker(control),
                SliderRange::new(0., maximum, step),
            );
        }
        let alpha_offset = if self.draft.alpha_enabled {
            ui.number_field(
                [x + 14., bottom + 178., inner_width, 28.],
                "A · Непрозрачность, %",
                self.draft.value(ColorControl::Alpha).unwrap() as f64,
                Action::ColorPicker(ColorControl::Alpha),
                SliderRange::new(0., 100., 0.1),
            );
            36.
        } else {
            0.
        };
        let feedback = error
            .or(self.notice.as_deref())
            .unwrap_or("Enter — применить · Esc — отменить");
        ui.fitted_label(
            [x + 14., bottom + 178. + alpha_offset, inner_width, 14.],
            feedback,
            10.5,
            if error.is_some() { p.danger } else { p.muted },
        );
        let button_width = (inner_width - 8.) * 0.5;
        ui.button(
            [x + 14., bottom + 198. + alpha_offset, button_width, 28.],
            "Отмена",
            Action::ColorPicker(ColorControl::Cancel),
            false,
        );
        ui.button(
            [
                x + 22. + button_width,
                bottom + 198. + alpha_offset,
                button_width,
                28.,
            ],
            "Применить",
            Action::ColorPicker(ColorControl::Apply),
            true,
        );
    }
}

impl GradientCache {
    fn prepare(&mut self, layout: ColorLayout, scale: f32, h: f32) {
        let wheel_key = (layout.wheel[2].to_bits(), scale.to_bits());
        if self.wheel_key != Some(wheel_key) {
            self.wheel = wheel_quads(layout.wheel[2], scale);
            self.wheel_key = Some(wheel_key);
        }
        let sv_key = (layout.sv[2].to_bits(), scale.to_bits(), hue(h).to_bits());
        if self.sv_key != Some(sv_key) {
            self.sv = sv_quads(layout.sv[2], scale, h);
            self.sv_key = Some(sv_key);
        }
    }
}

/// Two physical pixels per tile balance continuous gradients and quad count.
/// Tile edges share exact coordinates; annulus coverage is sampled at its rim.
fn wheel_quads(diameter: f32, scale: f32) -> Vec<UiRect> {
    let size = diameter * scale;
    let steps = (size / 2.).ceil().clamp(16., 256.) as usize;
    let tile = size / steps as f32;
    let outer = size * 0.5;
    let inner = outer - ring_width(diameter) * scale;
    let mut quads = Vec::new();
    for row in 0..steps {
        for col in 0..steps {
            let x = col as f32 * tile;
            let y = row as f32 * tile;
            let cx = x + tile * 0.5 - outer;
            let cy = y + tile * 0.5 - outer;
            let distance = cx.hypot(cy);
            let reach = tile * 0.5 * SQRT_2;
            if distance + reach < inner || distance - reach > outer {
                continue;
            }
            let coverage = if distance - reach >= inner && distance + reach <= outer {
                1.
            } else {
                let mut covered = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = x + (sx as f32 + 0.5) * tile * 0.25 - outer;
                        let py = y + (sy as f32 + 0.5) * tile * 0.25 - outer;
                        let r = px.hypot(py);
                        covered += usize::from(r >= inner && r <= outer);
                    }
                }
                covered as f32 / 16.
            };
            if coverage == 0. {
                continue;
            }
            let rgb = hsv_to_rgb(Hsv {
                hue: cy.atan2(cx) * 180. / PI,
                saturation: 1.,
                value: 1.,
            });
            quads.push(UiRect {
                triangle: None,
                segment: None,
                depth: None,
                rect: [x, y, tile, tile],
                color: [rgb[0], rgb[1], rgb[2], coverage],
            });
        }
    }
    quads
}

fn sv_quads(diameter: f32, scale: f32, h: f32) -> Vec<UiRect> {
    let size = diameter * scale;
    let steps = (size / 2.).ceil().clamp(8., 192.) as usize;
    let tile = size / steps as f32;
    let mut quads = Vec::with_capacity(steps * steps);
    for row in 0..steps {
        for col in 0..steps {
            let rgb = hsv_to_rgb(Hsv {
                hue: h,
                saturation: (col as f32 + 0.5) / steps as f32,
                value: 1. - (row as f32 + 0.5) / steps as f32,
            });
            quads.push(UiRect {
                triangle: None,
                segment: None,
                depth: None,
                rect: [col as f32 * tile, row as f32 * tile, tile, tile],
                color: [rgb[0], rgb[1], rgb[2], 1.],
            });
        }
    }
    quads
}

fn append_cached(ui: &mut Ui, quads: &[UiRect], origin: [f32; 2]) {
    let x = origin[0] * ui.scale;
    let y = origin[1] * ui.scale;
    ui.rects.extend(quads.iter().map(|quad| UiRect {
        triangle: None,
        segment: None,
        depth: None,
        rect: [
            quad.rect[0] + x,
            quad.rect[1] + y,
            quad.rect[2],
            quad.rect[3],
        ],
        color: quad.color,
    }));
}

fn marker(ui: &mut Ui, point: [f32; 2], rgb: [f32; 3]) {
    // Both outlines stay recognizable over all wheel colors and either theme.
    for (radius, color) in [
        (6., [0.06, 0.06, 0.06, 1.]),
        (4.5, [1., 1., 1., 1.]),
        (2.75, [rgb[0], rgb[1], rgb[2], 1.]),
    ] {
        ui.rounded_rect(
            [
                point[0] - radius,
                point[1] - radius,
                radius * 2.,
                radius * 2.,
            ],
            color,
            radius,
        );
    }
}

fn draw_swatch(ui: &mut Ui, rect: [f32; 4], rgba: [f32; 4]) {
    let p = ui.palette();
    ui.surface(rect, p.border);
    let inner = [rect[0] + 1., rect[1] + 1., rect[2] - 2., rect[3] - 2.];
    // Reuse the exact rounded coverage for the checkerboard as well as the
    // color; square checker tiles must not protrude through the swatch corners.
    let start = ui.rects.len();
    ui.rounded_rect(inner, [1.; 4], crate::ui::SURFACE_RADIUS - 1.);
    let coverage = ui.rects.split_off(start);
    let tile = 5. * ui.scale;
    let origin = [inner[0] * ui.scale, inner[1] * ui.scale];
    for quad in coverage {
        let [x, y, width, height] = quad.rect;
        let first_col = ((x - origin[0]) / tile).floor().max(0.) as usize;
        let first_row = ((y - origin[1]) / tile).floor().max(0.) as usize;
        let end_col = ((x + width - origin[0]) / tile).ceil() as usize;
        let end_row = ((y + height - origin[1]) / tile).ceil() as usize;
        for row in first_row..end_row {
            for col in first_col..end_col {
                let left = x.max(origin[0] + col as f32 * tile);
                let top = y.max(origin[1] + row as f32 * tile);
                let right = (x + width).min(origin[0] + (col + 1) as f32 * tile);
                let bottom = (y + height).min(origin[1] + (row + 1) as f32 * tile);
                if right > left && bottom > top {
                    let value = if (row + col) % 2 == 0 { 0.93 } else { 0.68 };
                    ui.rects.push(UiRect {
                        triangle: None,
                        segment: None,
                        depth: None,
                        rect: [left, top, right - left, bottom - top],
                        color: [value, value, value, quad.color[3]],
                    });
                }
            }
        }
    }
    ui.rounded_rect(inner, rgba.map(unit), crate::ui::SURFACE_RADIUS - 1.);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Theme;

    fn near(a: f32, b: f32) {
        assert!((a - b).abs() < 0.00003, "{a} != {b}");
    }
    fn rgb_near(a: [f32; 3], b: [f32; 3]) {
        for (a, b) in a.into_iter().zip(b) {
            near(a, b);
        }
    }
    fn layout(alpha: bool) -> ColorLayout {
        ColorLayout::anchored([1170., 96., 112., 28.], [1280., 800.], alpha)
    }

    #[test]
    fn srgb_hsv_round_trips_and_primaries_use_the_declared_model() {
        for (rgb, h) in [
            ([1., 0., 0.], 0.),
            ([1., 1., 0.], 60.),
            ([0., 1., 0.], 120.),
            ([0., 1., 1.], 180.),
            ([0., 0., 1.], 240.),
            ([1., 0., 1.], 300.),
        ] {
            let hsv = rgb_to_hsv(rgb, 51.);
            near(hsv.hue, h);
            near(hsv.saturation, 1.);
            near(hsv.value, 1.);
            rgb_near(hsv_to_rgb(hsv), rgb);
        }
        for r in 0..=10 {
            for g in 0..=10 {
                for b in 0..=10 {
                    let rgb = [r as f32 / 10., g as f32 / 10., b as f32 / 10.];
                    rgb_near(hsv_to_rgb(rgb_to_hsv(rgb, 217.)), rgb);
                }
            }
        }
        near(rgb_to_hsv([0.5; 3], 217.).hue, 217.);
        near(rgb_to_hsv([0.; 3], 217.).hue, 217.);
    }

    #[test]
    fn hue_seam_cardinals_and_pointer_inverse_agree() {
        let layout = layout(false);
        for h in [-f32::EPSILON, -0., f32::MAX, f32::NAN, f32::INFINITY] {
            assert!((0. ..360.).contains(&hue(h)));
        }
        for h in [0., 0.01, 45., 90., 180., 270., 359.99, 360., -0.01, 721.] {
            let point = layout.hue_position(h);
            assert_eq!(layout.drag_at(point), Some(ColorControl::Wheel));
            let result = layout.hue_at(point, 12.);
            let difference = (result - hue(h) + 180.).rem_euclid(360.) - 180.;
            assert!(difference.abs() < 0.001, "hue {h} became {result}");
        }
        assert!(layout.hue_position(90.)[1] > layout.center()[1]);
        assert!(layout.hue_position(270.)[1] < layout.center()[1]);
        near(layout.hue_at(layout.center(), 247.), 247.);
        let center = layout.center();
        assert_eq!(
            layout.drag_at([center[0] + layout.wheel[2] * 0.36, center[1]]),
            None
        );
        assert_eq!(layout.drag_at([layout.wheel[0] - 4., center[1]]), None);
        rgb_near(
            hsv_to_rgb(Hsv {
                hue: 0.,
                saturation: 0.73,
                value: 0.66,
            }),
            hsv_to_rgb(Hsv {
                hue: 360.,
                saturation: 0.73,
                value: 0.66,
            }),
        );
    }

    #[test]
    fn sv_mapping_clamps_outside_drag_and_keeps_the_initiating_control() {
        let layout = layout(false);
        for s in [0., 0.1, 0.5, 1.] {
            for v in [0., 0.2, 0.8, 1.] {
                let hsv = Hsv {
                    hue: 173.,
                    saturation: s,
                    value: v,
                };
                let point = layout.sv_position(hsv);
                assert_eq!(layout.drag_at(point), Some(ColorControl::Sv));
                let mapped = layout.sv_at(point, hsv);
                near(mapped.saturation, s);
                near(mapped.value, v);
                near(mapped.hue, 173.);
            }
        }
        let mut picker = ColorPicker::new([0.3, 0.5, 0.8, 1.], false);
        let h = picker.draft.hsv.hue;
        assert!(picker.drag(ColorControl::Sv, layout, [-10000., 10000.]));
        near(picker.draft.hsv.hue, h);
        near(picker.draft.hsv.saturation, 0.);
        near(picker.draft.hsv.value, 0.);
        assert!(picker.drag(ColorControl::Wheel, layout, layout.hue_position(32.)));
        near(picker.draft.hsv.hue, 32.);
        near(picker.draft.hsv.saturation, 0.);
        near(picker.draft.hsv.value, 0.);
    }

    #[test]
    fn gray_black_and_transparency_keep_useful_editing_memory() {
        let mut draft = ColorDraft::new([0.2, 0.4, 0.8, 0.37], true);
        let h = draft.hsv.hue;
        let s = draft.hsv.saturation;
        draft.set_value(ColorControl::Saturation, 0.).unwrap();
        near(draft.hsv.hue, h);
        draft.set_value(ColorControl::Saturation, s * 100.).unwrap();
        rgb_near(draft.rgb, [0.2, 0.4, 0.8]);
        draft.set_value(ColorControl::Value, 0.).unwrap();
        draft.set_value(ColorControl::Hue, 313.).unwrap();
        draft.set_value(ColorControl::Value, 80.).unwrap();
        near(draft.hsv.hue, 313.);
        near(draft.hsv.saturation, s);
        draft.set_text(ColorControl::Hex, "#000000").unwrap();
        near(draft.hsv.hue, 313.);
        near(draft.hsv.saturation, s);
        draft.set_value(ColorControl::Value, 80.).unwrap();
        let rgb = draft.rgb;
        draft.set_value(ColorControl::Alpha, 0.).unwrap();
        assert_eq!(draft.rgb, rgb);
        draft.set_value(ColorControl::Alpha, 100.).unwrap();
        assert_eq!(draft.rgb, rgb);
        draft.restore_original();
        assert_eq!(draft.rgba(), [0.2, 0.4, 0.8, 0.37]);
        near(draft.hsv.hue, h);
        near(draft.hsv.saturation, s);
        assert!(!draft.rgb_changed() && !draft.alpha_changed());
    }

    #[test]
    fn hdr_and_unquantized_rgb_survive_open_hex_accept_and_alpha_only_changes() {
        for rgba in [
            [0.1234567, 0.2345678, 0.8765432, 0.54321],
            [4.25, -0.1, 1.5, 0.456789],
        ] {
            let mut draft = ColorDraft::new(rgba, true);
            assert_eq!(draft.rgba(), rgba);
            assert!(!draft
                .set_text(ColorControl::Hex, &draft.hex().to_lowercase())
                .unwrap());
            assert_eq!(draft.rgba(), rgba);
            draft.set_value(ColorControl::Alpha, 25.).unwrap();
            assert!(!draft.rgb_changed());
            assert!(draft.alpha_changed());
            assert_eq!(draft.rgb, [rgba[0], rgba[1], rgba[2]]);
            draft.set_text(ColorControl::Hex, "#123456").unwrap();
            near(draft.alpha, 0.25);
            assert!(draft.rgb_changed());
            draft.restore_original();
            assert_eq!(draft.rgba(), rgba);
        }
        let mut draft = ColorDraft::new([0., 0., 0., 0.5], true);
        draft.set_text(ColorControl::Hex, "#FF008040").unwrap();
        assert_eq!(draft.rgb, [1., 0., 128. / 255.]);
        near(draft.alpha, 64. / 255.);
        let mut opaque = ColorDraft::new([0.1, 0.2, 0.3, 1.], false);
        assert!(opaque.set_text(ColorControl::Hex, "#FF008040").is_err());
        assert!(opaque.set_value(ColorControl::Alpha, 50.).is_err());
    }

    #[test]
    fn invalid_input_is_atomic_and_nonfinite_sources_never_reach_geometry() {
        let mut draft = ColorDraft::new([0.2, 0.4, 0.8, 0.5], true);
        for text in [
            "",
            "#",
            "#FFF",
            "#12345G",
            "#123456789",
            "#顔色abcdef",
            "NaN",
        ] {
            assert!(draft.set_text(ColorControl::Hex, text).is_err());
            assert_eq!(draft.rgba(), [0.2, 0.4, 0.8, 0.5]);
        }
        for control in [
            ColorControl::Hue,
            ColorControl::Saturation,
            ColorControl::Value,
            ColorControl::Alpha,
        ] {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1., 361.] {
                assert!(draft.set_value(control, value).is_err());
                assert_eq!(draft.rgba(), [0.2, 0.4, 0.8, 0.5]);
            }
            assert!(draft.set_text(control, "NaN").is_err());
        }
        draft.set_text(ColorControl::Hue, "360°").unwrap();
        near(draft.hsv.hue, 0.);
        draft.set_text(ColorControl::Saturation, "12,5%").unwrap();
        near(draft.hsv.saturation, 0.125);
        let rgb = hsv_to_rgb(Hsv {
            hue: f32::INFINITY,
            saturation: f32::NAN,
            value: 0.5,
        });
        assert!(rgb.iter().all(|c| c.is_finite() && (0. ..=1.).contains(c)));
        let mut picker =
            ColorPicker::new([f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN], true);
        assert!(picker.draft.rgba().iter().all(|v| v.is_finite()));
        let before = picker.draft.rgba();
        assert!(!picker.drag(ColorControl::Sv, layout(true), [f32::NAN, 0.]));
        assert_eq!(picker.draft.rgba(), before);
    }

    #[test]
    fn popover_bounds_hits_and_gradient_cache_agree_at_each_theme_and_dpi() {
        for alpha in [false, true] {
            for theme in [Theme::Dark, Theme::Light] {
                for scale in [1., 1.5, 2.] {
                    let layout = layout(alpha);
                    assert!(layout.panel[0] >= 0. && layout.panel[1] >= 0.);
                    assert!(layout.panel[0] + layout.panel[2] <= 1280.);
                    assert!(layout.panel[1] + layout.panel[3] <= 800.);
                    let mut picker = ColorPicker::new([0.2, 0.4, 0.8, 0.37], alpha);
                    picker
                        .draft
                        .set_value(ColorControl::Saturation, 0.)
                        .unwrap();
                    let mut ui = Ui::with_theme(scale, theme, [-1., -1.]);
                    picker.draw(&mut ui, layout, "Цвет материала", None);
                    let sv_marker = layout.sv_position(picker.draft.hsv());
                    let outside_square = [sv_marker[0] - 4., sv_marker[1]];
                    assert!(!contains(layout.sv, outside_square));
                    assert_eq!(layout.drag_at(outside_square), None);
                    assert_eq!(
                        ui.hit(outside_square),
                        Some(Action::ColorPicker(ColorControl::Sv))
                    );
                    assert!(ui.rects.len() < 40000);
                    for quad in &ui.rects {
                        assert!(quad
                            .rect
                            .iter()
                            .chain(quad.color.iter())
                            .all(|v| v.is_finite()));
                        assert!(quad.rect[2] > 0. && quad.rect[3] > 0.);
                        assert!(quad.rect[0] >= layout.panel[0] * scale - 0.01);
                        assert!(quad.rect[1] >= layout.panel[1] * scale - 0.01);
                        assert!(
                            quad.rect[0] + quad.rect[2]
                                <= (layout.panel[0] + layout.panel[2]) * scale + 0.01
                        );
                        assert!(
                            quad.rect[1] + quad.rect[3]
                                <= (layout.panel[1] + layout.panel[3]) * scale + 0.01
                        );
                    }
                    for control in [
                        ColorControl::Apply,
                        ColorControl::Cancel,
                        ColorControl::Hex,
                        ColorControl::Hue,
                        ColorControl::Saturation,
                        ColorControl::Value,
                        ColorControl::RestoreOriginal,
                    ] {
                        assert!(ui
                            .hits
                            .iter()
                            .any(|hit| hit.action == Action::ColorPicker(control)));
                    }
                    assert_eq!(
                        ui.hits
                            .iter()
                            .any(|hit| hit.action == Action::ColorPicker(ColorControl::Alpha)),
                        alpha
                    );
                    assert!(ui.texts.iter().any(|text| text.text == "sRGB · HSV"));
                    let wheel_ptr = picker.cache.wheel.as_ptr();
                    let sv_ptr = picker.cache.sv.as_ptr();
                    picker
                        .draft
                        .set_value(ColorControl::Saturation, 12.)
                        .unwrap();
                    picker.cache.prepare(layout, scale, picker.draft.hsv.hue);
                    assert_eq!(picker.cache.wheel.as_ptr(), wheel_ptr);
                    assert_eq!(picker.cache.sv.as_ptr(), sv_ptr);
                    // Theme, alpha, S/V and panel position cannot invalidate a hue gradient.
                    let mut shifted = layout;
                    shifted.panel[0] -= 10.;
                    shifted.wheel[0] -= 10.;
                    shifted.sv[0] -= 10.;
                    picker.cache.prepare(shifted, scale, picker.draft.hsv.hue);
                    assert_eq!(picker.cache.wheel.as_ptr(), wheel_ptr);
                    assert_eq!(picker.cache.sv.as_ptr(), sv_ptr);
                    picker.draft.set_value(ColorControl::Hue, 12.).unwrap();
                    picker.cache.prepare(layout, scale, picker.draft.hsv.hue);
                    assert_eq!(picker.cache.wheel.as_ptr(), wheel_ptr);
                    assert_eq!(picker.cache.sv_key.unwrap().2, 12_f32.to_bits());
                    let first = picker.cache.sv[0].color;
                    let expected = hsv_to_rgb(Hsv {
                        hue: 12.,
                        saturation: (picker.cache.sv[0].rect[0] + picker.cache.sv[0].rect[2] / 2.)
                            / (layout.sv[2] * scale),
                        value: 1.
                            - (picker.cache.sv[0].rect[1] + picker.cache.sv[0].rect[3] / 2.)
                                / (layout.sv[3] * scale),
                    });
                    rgb_near([first[0], first[1], first[2]], expected);
                }
            }
        }
        for viewport in [[100., 100.], [0., 0.], [f32::NAN, 700.]] {
            let layout = ColorLayout::anchored([f32::NAN; 4], viewport, false);
            let mut picker = ColorPicker::new([0.; 4], false);
            let mut ui = Ui::new(1.);
            picker.draw(&mut ui, layout, "Цвет", None);
            assert!(ui
                .rects
                .iter()
                .flat_map(|quad| quad.rect)
                .all(f32::is_finite));
        }
    }
}
