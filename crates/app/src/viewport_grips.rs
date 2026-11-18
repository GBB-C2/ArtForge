//! Tool handles in logical screen pixels. Guides are visual only. The glyphs own pointer hits.
use super::*;
use crate::{gizmo, ui::AuthorAction as A};
use glam::Vec2;
pub(super) type Segment = ([f32; 2], [f32; 2]);
const PICK: f32 = 8.;
fn padded(r: [f32; 4], amount: f32) -> [f32; 4] {
    [
        r[0] - amount,
        r[1] - amount,
        r[2] + amount * 2.,
        r[3] + amount * 2.,
    ]
}

fn clip(strokes: &[Segment], area: [f32; 4], obstacles: &[[f32; 4]]) -> Vec<Segment> {
    let mut result = Vec::new();
    for &(a, b) in strokes {
        let (a, b) = (Vec2::from_array(a), Vec2::from_array(b));
        let Some(range) = gizmo::clip_range(a, b, padded(area, -PICK - 1.)) else {
            continue;
        };
        let mut ranges = vec![range];
        for &r in obstacles {
            if let Some((cut_lo, cut_hi)) = gizmo::clip_range(a, b, padded(r, PICK + 2.)) {
                ranges = ranges
                    .into_iter()
                    .flat_map(|(lo, hi)| {
                        [(lo, hi.min(cut_lo)), (lo.max(cut_hi), hi)]
                            .into_iter()
                            .filter(|(lo, hi)| hi - lo > 1e-5)
                    })
                    .collect();
            }
        }
        result.extend(
            ranges
                .into_iter()
                .filter(|(lo, hi)| hi - lo > 1e-5)
                .map(|(lo, hi)| (a.lerp(b, lo).to_array(), a.lerp(b, hi).to_array())),
        );
    }
    result
}

#[derive(Clone, Copy)]
pub(super) enum Glyph {
    Bar,
    Caliper,
    Lens,
    Corner(Vec2, Vec2),
    Plate,
    Stack,
    Eye,
    Arrow,
}

pub(super) fn segment(a: Vec2, b: Vec2) -> Segment {
    (a.to_array(), b.to_array())
}

impl Glyph {
    fn strokes(self, c: Vec2, axis: Vec2) -> Vec<Segment> {
        let n = axis.perp();
        let p = |x: f32, y: f32| c + x * axis + y * n;
        let mut out = Vec::new();
        let mut path = |pts: &[Vec2]| out.extend(pts.windows(2).map(|s| segment(s[0], s[1])));
        match self {
            Self::Bar => {
                path(&[
                    p(-3., -14.),
                    p(3., -14.),
                    p(3., 14.),
                    p(-3., 14.),
                    p(-3., -14.),
                ]);
                for d in [-14., 14.] {
                    path(&[p(-5., d), p(5., d)]);
                }
            }
            Self::Caliper => {
                for s in [-1., 1.] {
                    path(&[p(s * 2., -8.), p(s * 7., -8.), p(s * 7., 8.), p(s * 2., 8.)]);
                }
            }
            Self::Lens => {
                let arc: Vec<_> = (0..=24)
                    .map(|i| {
                        let t = std::f32::consts::TAU * i as f32 / 24.;
                        p(12. * t.cos(), 6. * t.sin() * (1. - 0.45 * t.cos().abs()))
                    })
                    .collect();
                path(&arc);
                path(&[p(-2., 0.), p(2., 0.)]);
            }
            Self::Corner(a, b) => {
                path(&[c + a * 17., c, c + b * 17.]);
                let inner = c + (a + b).normalize_or_zero() * 5.;
                path(&[inner + a * 7., inner, inner + b * 7.]);
            }
            Self::Plate => {
                path(&[
                    p(-6., -11.),
                    p(7., -8.),
                    p(7., 11.),
                    p(-6., 8.),
                    p(-6., -11.),
                ]);
                path(&[p(0., -4.), p(0., 4.)]);
            }
            Self::Stack => {
                for y in [-5., 0., 5.] {
                    let q = c + Vec2::Y * y;
                    path(&[
                        q + Vec2::new(-10., -1.),
                        q + Vec2::new(0., -5.),
                        q + Vec2::new(10., -1.),
                        q + Vec2::new(0., 3.),
                        q + Vec2::new(-10., -1.),
                    ]);
                }
            }
            Self::Eye => {
                let ring: Vec<_> = (0..=24)
                    .map(|i| {
                        let a = i as f32 * std::f32::consts::TAU / 24.;
                        c + Vec2::new(a.cos(), a.sin()) * 7.5
                    })
                    .collect();
                path(&ring);
                path(&[p(-1.5, 0.), p(1.5, 0.)]);
            }
            Self::Arrow => {
                for s in [-1., 1.] {
                    path(&[p(s * 6., -4.), p(s * 13., 0.), p(s * 6., 4.)]);
                }
                path(&[p(-11., 0.), p(11., 0.)]);
            }
        }
        out
    }
}

pub(super) struct Grip {
    pub center: Vec2,
    pub axis: Vec2,
    pub glyph: Glyph,
    pub action: A,
    pub label: &'static str,
    pub value: String,
    pub guides: Vec<Segment>,
}
impl Grip {
    pub(super) fn new(
        center: Vec2,
        axis: Vec2,
        glyph: Glyph,
        action: A,
        label: &'static str,
        value: String,
    ) -> Self {
        Self {
            center,
            axis: axis.try_normalize().unwrap_or(Vec2::X),
            glyph,
            action,
            label,
            value,
            guides: Vec::new(),
        }
    }
    pub(super) fn tether(&mut self, from: Vec2) {
        self.guides.push(segment(from, self.center));
    }
}
pub(super) struct GripArea {
    area: [f32; 4],
    obstacles: Vec<[f32; 4]>,
    centers: Vec<Vec2>,
    labels: Vec<[f32; 4]>,
}
impl GripArea {
    pub(super) fn new(app: &App) -> Self {
        let mut nav = app.view_widget_rect();
        nav[3] += if app.camera_view.is_some() { 84. } else { 52. };
        let mut obstacles = vec![nav, app.shading_toolbar_rect(), app.rail_rect()];
        if let Some(panel) = app.tool_panel_rect() {
            obstacles.push(panel);
        }
        Self {
            area: app.modeling_area(),
            obstacles,
            centers: Vec::new(),
            labels: Vec::new(),
        }
    }
    /// Move a small grip away from nearby controls, keeping a leader to its actual target.
    pub(super) fn place(&mut self, grip: &mut Grip, source: Vec2) -> bool {
        let bounds = padded(self.area, -23.);
        if bounds[2] <= 0. || bounds[3] <= 0. || !source.is_finite() || !grip.center.is_finite() {
            return false;
        }
        if !ui::contains(padded(self.area, 48.), source.to_array())
            && !ui::contains(self.area, grip.center.to_array())
        {
            return false;
        }
        let tip = grip.center;
        let anchor = Vec2::new(
            tip.x.clamp(bounds[0], bounds[0] + bounds[2]),
            tip.y.clamp(bounds[1], bounds[1] + bounds[3]),
        );
        let valid = |p: Vec2| {
            ui::contains(bounds, p.to_array())
                && !self
                    .obstacles
                    .iter()
                    .any(|r| ui::contains(padded(*r, 24.), p.to_array()))
                && self.centers.iter().all(|c| p.distance(*c) >= 46.)
        };
        let mut chosen = None;
        'find: for radius in [0., 48., 82.] {
            for d in [
                grip.axis.perp(),
                -grip.axis.perp(),
                grip.axis,
                -grip.axis,
                Vec2::Y,
                Vec2::NEG_Y,
                Vec2::X,
                Vec2::NEG_X,
            ] {
                let p = anchor + d * radius;
                if valid(p) {
                    chosen = Some(p);
                    break 'find;
                }
            }
        }
        // If the floating inspector covers the entire neighbourhood, use its nearest free side.
        let chosen = chosen.or_else(|| {
            self.obstacles
                .iter()
                .flat_map(|r| {
                    let o = padded(*r, 26.);
                    [
                        Vec2::new(o[0], anchor.y),
                        Vec2::new(o[0] + o[2], anchor.y),
                        Vec2::new(anchor.x, o[1]),
                        Vec2::new(anchor.x, o[1] + o[3]),
                    ]
                })
                .filter(|&p| valid(p))
                .min_by(|a, b| {
                    a.distance_squared(anchor)
                        .total_cmp(&b.distance_squared(anchor))
                })
        });
        let Some(p) = chosen else {
            return false;
        };
        if p.distance(tip) > 1. {
            grip.guides.push(segment(tip, p));
        }
        grip.center = p;
        self.centers.push(p);
        true
    }
    pub(super) fn paint(&mut self, u: &mut Ui, grip: &Grip, mouse: [f32; 2], active: bool) {
        let palette = u.palette();
        let strokes = clip(
            &grip.glyph.strokes(grip.center, grip.axis),
            self.area,
            &self.obstacles,
        );
        if strokes.is_empty() {
            return;
        }
        let action = Action::Author(grip.action);
        let guides = clip(&grip.guides, self.area, &self.obstacles);
        let mut trace = palette.accent;
        trace[3] = 0.64;
        u.smooth_segments(&guides, 1.35, trace);
        u.path_hit(action.clone(), strokes.clone(), PICK);
        let highlight = active || u.hit(mouse) == Some(action);
        u.smooth_segments(
            &strokes,
            if highlight { 6. } else { 4.8 },
            [0.035, 0.045, 0.065, 0.92],
        );
        u.smooth_segments(&strokes, if highlight { 3.1 } else { 2.3 }, palette.accent);
        if highlight && !matches!(grip.glyph, Glyph::Eye) {
            let mut tips = Vec::with_capacity(6);
            let n = grip.axis.perp();
            for sign in [-1., 1.] {
                let tip = grip.center + grip.axis * (24. * sign);
                let back = tip - grip.axis * (4. * sign);
                tips.extend([
                    segment(grip.center + grip.axis * (16. * sign), tip),
                    segment(back + 2.5 * n, tip),
                    segment(back - 2.5 * n, tip),
                ]);
            }
            u.smooth_segments(
                &clip(&tips, self.area, &self.obstacles),
                1.4,
                palette.accent,
            );
        }
        let label = u.language.text(grip.label);
        let text = if highlight {
            let value = if u.language == crate::localization::Language::English {
                grip.value.replace(" м", " m")
            } else {
                grip.value.clone()
            };
            format!("{label} · {value}")
        } else {
            label.into_owned()
        };
        let width = (ui::caption_width(&text, 11.).ceil() + 14.).max(42.);
        for [sx, dy] in [
            [1., -24.],
            [-1., -24.],
            [1., 12.],
            [-1., 12.],
            [1., -44.],
            [-1., -44.],
        ] {
            let r = [
                grip.center.x + if sx > 0. { 21. } else { -width - 21. },
                grip.center.y + dy,
                width,
                20.,
            ];
            if !ui::contains(self.area, [r[0], r[1]])
                || !ui::contains(self.area, [r[0] + r[2], r[1] + r[3]])
                || self
                    .obstacles
                    .iter()
                    .chain(&self.labels)
                    .any(|o| intersect_rect(r, *o).is_some())
                || self
                    .centers
                    .iter()
                    .any(|p| ui::contains(padded(r, 14.), p.to_array()))
            {
                continue;
            }
            let mut fill = palette.panel;
            fill[3] = 0.93;
            u.outlined_surface(
                r,
                fill,
                if highlight {
                    palette.accent
                } else {
                    palette.border
                },
            );
            u.raw_fitted_label(
                [r[0] + 6., r[1] + 3., r[2] - 12., r[3] - 5.],
                &text,
                11.,
                palette.text,
            );
            self.labels.push(r);
            break;
        }
    }
}

impl App {
    pub(super) fn grip_projection(&self, p: Vec3) -> Option<Vec2> {
        self.project_point(p)
            .map(Vec2::from_array)
            .filter(|p| p.is_finite())
    }
    /// Screen direction and local units per logical pixel, including the object's scaling.
    pub(super) fn grip_axis(
        &self,
        world: Vec3,
        displacement: Vec3,
        sample: f32,
        fallback: (Vec2, f32),
    ) -> (Vec2, f32) {
        let project = |v| self.grip_projection(v);
        if let (Some(a), Some(b)) = (
            project(world - displacement * sample),
            project(world + displacement * sample),
        ) {
            let d = b - a;
            let len = d.length();
            if len.is_finite() && len >= 6. {
                return (d / len, 2. * sample / len);
            }
        }
        fallback
    }
}
