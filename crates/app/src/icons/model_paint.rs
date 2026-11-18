//! Rasterize the approved paths at the display DPI. The three mask channels
//! carry ink, secondary ink, and action coverage, so a theme change reuses the mask.
use super::model_art::{Command, Layer, Shape, Style};
use super::Icon;
use crate::ui::{Palette, Ui};
use std::{cell::RefCell, collections::VecDeque};
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

const MAX_MASK_SIDE: u16 = 256;
const CACHE_CAPACITY: usize = 64;

struct Span {
    x: u16,
    y: u16,
    width: u16,
    rgba: [u8; 4],
}

struct Mask {
    icon: Icon,
    pixels: u16,
    spans: Vec<Span>,
}

thread_local! {
    static MASKS: RefCell<VecDeque<Mask>> = const { RefCell::new(VecDeque::new()) };
}

fn shape_path(shape: &Shape) -> Option<Path> {
    match shape {
        Shape::Oval(cx, cy, rx, ry) => {
            let bounds = Rect::from_xywh(cx - rx, cy - ry, 2. * rx, 2. * ry)?;
            PathBuilder::from_oval(bounds)
        }
        Shape::Path(commands) => {
            let mut builder = PathBuilder::new();
            for &command in *commands {
                match command {
                    Command::M(x, y) => builder.move_to(x, y),
                    Command::L(x, y) => builder.line_to(x, y),
                    Command::Q(x1, y1, x, y) => builder.quad_to(x1, y1, x, y),
                    Command::C(x1, y1, x2, y2, x, y) => builder.cubic_to(x1, y1, x2, y2, x, y),
                    Command::Z => builder.close(),
                }
            }
            builder.finish()
        }
    }
}

fn rasterize(icon: Icon, layers: &[Layer], pixels: u16) -> Option<Mask> {
    let mut pixmap = Pixmap::new(pixels.into(), pixels.into())?;
    let zoom = f32::from(pixels) / 24.;
    let transform = Transform::from_scale(zoom, zoom);
    let mut paint = Paint::default();
    let ink = [255, 0, 0];
    let minor = [0, 255, 0];
    let accent = [0, 0, 255];
    for Layer(style, shape) in layers {
        let path = shape_path(shape)?;
        // Fill alpha and stroke width in the artwork's logical units.
        let ([r, g, b], fill, width) = match style {
            Style::Edge => (ink, 0, 1.56),
            Style::Minor => (minor, 0, 1.25),
            Style::Action => (accent, 0, 1.9),
            Style::Cable => (accent, 0, 2.5),
            Style::Facet => (accent, 74, 1.7),
            Style::Body => (ink, 18, 0.),
            Style::Wash => (accent, 59, 0.),
            Style::Node => (ink, 255, 0.),
            Style::Hot => (accent, 255, 0.),
        };
        if fill > 0 {
            paint.set_color_rgba8(r, g, b, fill);
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
        if width > 0. {
            paint.set_color_rgba8(r, g, b, 255);
            let stroke = Stroke {
                width,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, transform, None);
        }
    }

    let mut spans = Vec::new();
    for (y, row) in pixmap.data().chunks_exact(usize::from(pixels) * 4).enumerate() {
        let mut x = 0;
        while x < pixels {
            let start = x;
            let i = usize::from(x) * 4;
            let rgba = [row[i], row[i + 1], row[i + 2], row[i + 3]];
            x += 1;
            while x < pixels && row[usize::from(x) * 4..][..4] == rgba[..] {
                x += 1;
            }
            if rgba[3] != 0 {
                spans.push(Span { x: start, y: y as u16, width: x - start, rgba });
            }
        }
    }
    Some(Mask { icon, pixels, spans })
}

pub(super) fn draw(
    ui: &mut Ui,
    icon: Icon,
    layers: &'static [Layer],
    origin: [f32; 2],
    size: f32,
    foreground: [f32; 4],
    palette: Option<Palette>,
) {
    let physical = size * ui.scale;
    if !physical.is_finite() || !origin.iter().all(|v| (v * ui.scale).is_finite()) {
        return;
    }
    let pixels = physical.round().clamp(1., f32::from(MAX_MASK_SIDE)) as u16;
    MASKS.with_borrow_mut(|cache| {
        let mask = if let Some(index) = cache.iter().position(|m| m.icon == icon && m.pixels == pixels) {
            &cache[index]
        } else {
            let Some(new) = rasterize(icon, layers, pixels) else { return };
            if cache.len() == CACHE_CAPACITY {
                let _ = cache.pop_front();
            }
            cache.push_back(new);
            cache.back().unwrap()
        };
        let unit = size / f32::from(pixels);
        for span in &mask.spans {
            let [r, g, b, a] = span.rgba.map(f32::from);
            let mut color = foreground;
            if let Some(p) = &palette {
                let total = (r + g + b).max(1.);
                for (i, c) in color[..3].iter_mut().enumerate() {
                    *c = (r * p.text[i] + g * p.muted[i] + b * p.accent[i]) / total;
                }
            }
            color[3] *= a / 255.;
            ui.rect([
                origin[0] + f32::from(span.x) * unit,
                origin[1] + f32::from(span.y) * unit,
                f32::from(span.width) * unit,
                unit,
            ], color);
        }
    });
}
