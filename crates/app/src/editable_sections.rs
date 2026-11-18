//! Per-plane editing inside the knife transaction; every preview starts at the source mesh.
use super::*;
use crate::ui::AuthorAction as A;
use poseforge_core::PlaneCut;

#[derive(Clone)]
pub(in crate::app) struct Panel {
    origin: Vec3,
    center: Vec3,
    normal: Vec3,
    radius: f32,
    seed: usize,
    values: [f32; 4], // offset, two angles, gap
    cap: bool,
}
impl Panel {
    pub(super) fn cut(&self) -> PlaneCut {
        let (u, v) = self.axes();
        let normal = (Quat::from_axis_angle(u, self.values[1].to_radians())
            * Quat::from_axis_angle(v, self.values[2].to_radians())
            * self.normal)
            .normalize();
        PlaneCut {
            normal: normal.to_array(),
            offset: normal.dot(self.origin) + self.values[0],
            width: self.values[3],
            close_gap: self.cap,
            seed_face: self.seed,
        }
    }
    fn axes(&self) -> (Vec3, Vec3) {
        let u = self
            .normal
            .cross(if self.normal.y.abs() < 0.8 {
                Vec3::Y
            } else {
                Vec3::X
            })
            .normalize();
        (u, self.normal.cross(u))
    }
    fn frame(&self) -> (Vec3, Vec3, Vec3) {
        let c = self.cut();
        let n = Vec3::from_array(c.normal);
        let center = self.center + n * (c.offset - n.dot(self.center));
        let u = n
            .cross(if n.y.abs() < 0.8 { Vec3::Y } else { Vec3::X })
            .normalize();
        (center, u, n.cross(u))
    }
}
impl App {
    pub(super) fn section_panels(&self, hit: EdgePoint) -> Result<Vec<Panel>, String> {
        let k = self.authoring.knife.as_ref().ok_or("Разрез не активен")?;
        let mesh = &k.original;
        let graph = poseforge_core::MeshConnectivity::new(mesh);
        let seed = *graph
            .edge_faces
            .get(&hit.edge)
            .and_then(|f| f.first())
            .ok_or("Выберите поверхность")?;
        let a = Vec3::from_array(mesh.vertices[hit.edge.0].position);
        let b = Vec3::from_array(mesh.vertices[hit.edge.1].position);
        let normal = (b - a).try_normalize().ok_or("Нулевое ребро")?;
        let component = graph.connected_faces(seed);
        let (lo, hi) = component
            .iter()
            .flat_map(|&f| mesh.face_vertices(f).unwrap())
            .map(|&v| Vec3::from_array(mesh.vertices[v as usize].position))
            .fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(lo, hi), p| (lo.min(p), hi.max(p)),
            );
        let radius = (hi - lo).length() * 0.6;
        let count = self.authoring.workflow.values[9] as usize;
        let t = hit.t.clamp(0.01, 0.99);
        let span = 2. * t.min(1. - t);
        Ok((0..count)
            .map(|i| Panel {
                origin: a.lerp(
                    b,
                    if count == 1 {
                        t
                    } else {
                        t - span * 0.5 + span * (i + 1) as f32 / (count + 1) as f32
                    },
                ),
                center: (lo + hi) * 0.5,
                normal,
                radius,
                seed,
                values: [0., 0., 0., self.authoring.workflow.values[1]],
                cap: self.authoring.workflow.close_gap,
            })
            .collect())
    }
    pub(super) fn create_sections(&mut self, hit: EdgePoint) -> Result<(), String> {
        let panels = self.section_panels(hit)?;
        let count = panels.len();
        let k = self.authoring.knife.as_mut().unwrap();
        let start = k.sections.len();
        if start + count > 128 {
            return Err("Допускается до 128 разрезов за одну операцию".into());
        }
        let previous = k.section_selected;
        k.sections.extend(panels);
        k.section_selected = Some(start);
        if let Err(e) = self.preview_knife_geometry() {
            let k = self.authoring.knife.as_mut().unwrap();
            k.sections.truncate(start);
            k.section_selected = previous;
            return Err(e);
        }
        self.authoring.knife.as_mut().unwrap().section_batch = Some((hit, start, count));
        self.status = format!(
            "Разрез · плоскостей: {} · переключитесь в «Редактировать» для настройки",
            start + count
        );
        Ok(())
    }
    pub(super) fn section_action(&mut self, a: A) -> bool {
        match a {
            A::SectionMode(edit) => {
                if let Some(k) = self.authoring.knife.as_mut().filter(|k| k.loop_mode) {
                    k.section_edit = edit;
                    k.press = None;
                    k.section_drag = None;
                    if edit {
                        k.section_batch = None;
                    }
                }
            }
            A::SectionSelect(i) => {
                if let Some(k) = self.authoring.knife.as_mut().filter(|k| k.loop_mode) {
                    if i < k.sections.len() {
                        k.section_selected = Some(i);
                        k.section_drag = None;
                        k.press = None;
                    }
                }
            }
            A::SectionDelete => {
                let Some(k) = self.authoring.knife.as_mut() else {
                    return true;
                };
                let Some(i) = k.section_selected else {
                    return true;
                };
                k.press = None;
                k.section_drag = None;
                let old = k.sections.remove(i);
                k.section_selected = i.checked_sub(1).or((!k.sections.is_empty()).then_some(0));
                k.section_batch = None;
                if let Err(e) = self.preview_knife_geometry() {
                    let k = self.authoring.knife.as_mut().unwrap();
                    k.sections.insert(i, old);
                    k.section_selected = Some(i);
                    self.status = e;
                }
            }
            _ => return false,
        }
        true
    }
    pub(in crate::app) fn section_value(&self, i: u8) -> Option<f32> {
        let k = self.authoring.knife.as_ref().filter(|k| k.loop_mode)?;
        let p = k.sections.get(k.section_selected?)?;
        match i {
            1 => Some(p.values[3]),
            10..=12 => Some(p.values[(i - 10) as usize]),
            _ => None,
        }
    }
    pub(in crate::app) fn section_confirm(&mut self, i: u8, text: &str) -> Result<bool, String> {
        let Some(k) = self.authoring.knife.as_ref().filter(|k| k.loop_mode) else {
            return Ok(false);
        };
        if i == 9 && !k.section_edit {
            let count = surfaces::number(text, 1., 16.)?;
            if count.fract() != 0. {
                return Err("Количество линий должно быть целым".into());
            }
            let previous = self.authoring.workflow.values[9];
            self.authoring.workflow.values[9] = count;
            if let Some((hit, start, len)) = k.section_batch {
                let panels = match self.section_panels(hit) {
                    Ok(p) => p,
                    Err(e) => {
                        self.authoring.workflow.values[9] = previous;
                        return Err(e);
                    }
                };
                let k = self.authoring.knife.as_mut().unwrap();
                let old = k.sections.clone();
                let selected = k.section_selected;
                k.sections.splice(start..start + len, panels);
                k.section_selected = Some(start);
                if let Err(e) = self.preview_knife_geometry() {
                    let k = self.authoring.knife.as_mut().unwrap();
                    k.sections = old;
                    k.section_selected = selected;
                    self.authoring.workflow.values[9] = previous;
                    return Err(e);
                }
                self.authoring.knife.as_mut().unwrap().section_batch =
                    Some((hit, start, count as usize));
            }
            return Ok(true);
        }
        let field = match i {
            1 => 3,
            10..=12 => (i - 10) as usize,
            _ => return Ok(false),
        };
        let Some(selected) = k.section_selected else {
            return Ok(false);
        };
        let (min, max) = match field {
            3 => (0., 1000.),
            0 => (-10000., 10000.),
            _ => (-3600., 3600.),
        };
        let value = surfaces::number(text, min, max)?;
        let old = self.authoring.knife.as_ref().unwrap().sections[selected].values[field];
        self.authoring.knife.as_mut().unwrap().sections[selected].values[field] = value;
        if let Err(e) = self.preview_knife_geometry() {
            self.authoring.knife.as_mut().unwrap().sections[selected].values[field] = old;
            return Err(e);
        }
        if i == 1 {
            self.authoring.workflow.values[1] = value;
        }
        Ok(true)
    }
    pub(super) fn section_toggle_cap(&mut self) {
        let k = self.authoring.knife.as_mut().unwrap();
        if let Some(i) = k.section_selected {
            let old = k.sections[i].cap;
            k.sections[i].cap = !old;
            if let Err(e) = self.preview_knife_geometry() {
                self.authoring.knife.as_mut().unwrap().sections[i].cap = old;
                self.status = e;
            } else {
                self.authoring.workflow.close_gap = !old;
            }
        } else {
            self.authoring.workflow.close_gap = !self.authoring.workflow.close_gap;
        }
    }
    pub(super) fn select_section_at_cursor(&mut self) {
        let Some(k) = &self.authoring.knife else {
            return;
        };
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        let Some((o, d)) = authoring_controls::ray(
            self.view().view_projection().inverse(),
            self.viewport(),
            self.mouse,
        ) else {
            return;
        };
        let inv = globals[&k.node].inverse();
        let o = inv.transform_point3(o);
        let d = inv.transform_vector3(d);
        let picked = k
            .sections
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let (c, u, v) = p.frame();
                let n = u.cross(v);
                let den = n.dot(d);
                if den.abs() < 1e-8 {
                    return None;
                }
                let t = n.dot(c - o) / den;
                if t < 0. {
                    return None;
                }
                let delta = o + d * t - c;
                (delta.dot(u).abs() <= p.radius && delta.dot(v).abs() <= p.radius).then_some((i, t))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if let Some(i) = picked {
            let k = self.authoring.knife.as_mut().unwrap();
            k.section_selected = Some(i);
            k.section_drag = Some((i, self.mouse, k.sections[i].values[0]));
            k.press = Some(self.mouse);
        }
    }
    pub(in crate::app) fn move_section_plane(&mut self) {
        let Some(k) = self.authoring.knife.as_ref().filter(|k| k.loop_mode) else {
            return;
        };
        let Some((i, start, offset)) = k.section_drag.filter(|_| k.press.is_some()) else {
            return;
        };
        let panel = &k.sections[i];
        let (c, _, _) = panel.frame();
        let n = Vec3::from_array(panel.cut().normal);
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        let model = globals[&k.node];
        let screen = |p| {
            authoring_controls::project(
                self.view().view_projection(),
                self.viewport(),
                model.transform_point3(p),
            )
            .map(glam::Vec2::from_array)
        };
        let (Some(a), Some(b)) = (screen(c), screen(c + n)) else {
            return;
        };
        let axis = b - a;
        let delta = glam::Vec2::from_array(self.mouse) - glam::Vec2::from_array(start);
        if delta.length() < 3. {
            return;
        }
        let shift = if axis.length_squared() > 9. {
            delta.dot(axis) / axis.length_squared()
        } else {
            -delta.y * panel.radius / 180.
        };
        let precision = if self.modifiers.shift_key() { 0.1 } else { 1. };
        if let Err(e) = self.section_confirm(10, &(offset + shift * precision).to_string()) {
            self.status = e;
        }
    }
    pub(in crate::app) fn draw_section_modes(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        let Some(k) = self.authoring.knife.as_ref() else {
            return;
        };
        let p = u.palette();
        let half = (w - 6.) * 0.5;
        for (i, (name, edit)) in [("Создать", false), ("Редактировать", true)]
            .into_iter()
            .enumerate()
        {
            let r = [x + i as f32 * (half + 6.), y, half, 28.];
            u.quiet_button(
                r,
                "",
                Action::Author(A::SectionMode(edit)),
                k.section_edit == edit,
            );
            u.fitted_label(
                [r[0] + 7., r[1] + 6., r[2] - 14., 18.],
                name,
                12.,
                if k.section_edit == edit {
                    p.accent
                } else {
                    p.text
                },
            );
        }
    }
    pub(in crate::app) fn draw_section_properties(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let Some(k) = self.authoring.knife.as_ref() else {
            return;
        };
        let p = u.palette();
        self.draw_section_modes(u,x,y,w);
        y += 36.;
        u.fitted_label(
            [x, y, w, 18.],
            if k.section_edit {
                "Выберите панель · потяните для смещения"
            } else {
                "Щелчок по поверхности — новый разрез"
            },
            11.,
            p.muted,
        );
        y += 24.;
        if k.section_edit {
            let i = k.section_selected.unwrap_or(0);
            let len = k.sections.len();
            u.fitted_label(
                [x + 32., y, w - 100., 26.],
                &format!("Разрез: {} / {}", if len == 0 { 0 } else { i + 1 }, len),
                12.,
                p.text,
            );
            if len > 0 {
                u.quiet_button(
                    [x, y, 26., 26.],
                    "‹",
                    Action::Author(A::SectionSelect((i + len - 1) % len)),
                    false,
                );
                u.quiet_button(
                    [x + w - 60., y, 26., 26.],
                    "›",
                    Action::Author(A::SectionSelect((i + 1) % len)),
                    false,
                );
                u.quiet_button(
                    [x + w - 28., y, 28., 26.],
                    "×",
                    Action::Author(A::SectionDelete),
                    false,
                );
            }
            y += 34.;
            if len == 0 {
                u.fitted_label(
                    [x, y, w, 24.],
                    "Создайте разрез в режиме «Создать»",
                    11.,
                    p.muted,
                );
                y += 32.;
            } else {
                for field in 10..=12 {
                    self.draw_workflow_field(u, x, y, w, field, 36.);
                    y += 42.;
                }
            }
        } else {
            self.draw_workflow_field(u, x, y, w, 9, 36.);
            y += 42.;
            u.fitted_label(
                [x, y, w, 18.],
                &format!("Всего плоскостей: {} · Ctrl + колесо", k.sections.len()),
                11.,
                p.muted,
            );
            y += 24.;
        }
        if !k.section_edit || !k.sections.is_empty() {
            self.draw_workflow_field(u, x, y, w, 1, 36.);
            y += 42.;
            let cap = k
                .section_selected
                .and_then(|i| k.sections.get(i))
                .map_or(self.authoring.workflow.close_gap, |p| p.cap);
            u.checkbox(
                [x, y, w, 26.],
                "Закрыть торцы зазора",
                Action::Author(A::KnifeCloseGap),
                cap,
            );
            y += 34.;
        }
        self.draw_modeling_confirmation(u, x, y, w);
    }
    pub(super) fn draw_section_planes(&self, u: &mut Ui) {
        let Some(k) = self
            .authoring
            .knife
            .as_ref()
            .filter(|k| k.loop_mode && k.section_edit)
        else {
            return;
        };
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        let model = globals[&k.node];
        let palette = u.palette();
        let project = |p| {
            authoring_controls::project(
                self.view().view_projection(),
                self.viewport(),
                model.transform_point3(p),
            )
        };
        // Draw the selected plane last, with a distinct outline and numbered handle.
        let mut order: Vec<_> = (0..k.sections.len()).collect();
        order.sort_by_key(|i| k.section_selected == Some(*i));
        for i in order {
            let panel = &k.sections[i];
            let (c, a, b) = panel.frame();
            let r = panel.radius;
            let screen: Option<Vec<_>> = [
                c - a * r - b * r,
                c + a * r - b * r,
                c + a * r + b * r,
                c - a * r + b * r,
            ]
            .into_iter()
            .map(project)
            .collect();
            let Some(points) = screen else {
                continue;
            };
            let active = k.section_selected == Some(i);
            let mut fill = if active { palette.accent } else { palette.text };
            fill[3] = if active { 0.12 } else { 0.045 };
            for tri in [[0, 1, 2], [0, 2, 3]] {
                u.mesh_triangle(tri.map(|j| [points[j][0], points[j][1], 0.]), fill, false);
            }
            let mut line = if active {
                palette.accent
            } else {
                palette.muted
            };
            line[3] = if active { 0.9 } else { 0.45 };
            for j in 0..4 {
                u.mesh_segment(
                    points[j],
                    points[(j + 1) % 4],
                    if active { 1.8 } else { 1. },
                    line,
                );
            }
            let area = self.modeling_area();
            let label = [
                (points
                    .iter()
                    .map(|p| p[0])
                    .fold(f32::NEG_INFINITY, f32::max)
                    - 92.)
                    .clamp(area[0], (area[0] + area[2] - 92.).max(area[0])),
                points
                    .iter()
                    .map(|p| p[1])
                    .fold(f32::INFINITY, f32::min)
                    .clamp(area[1], (area[1] + area[3] - 24.).max(area[1])),
            ];
            u.quiet_button(
                [label[0], label[1], 92., 24.],
                &format!("Разрез: {}", i + 1),
                Action::Author(A::SectionSelect(i)),
                active,
            );
        }
    }
}
