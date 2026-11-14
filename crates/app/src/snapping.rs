//! Frozen drag targets: moving the source never moves its own snap destination.
use super::*;
use crate::ui::{AuthorAction as A, DropdownKind};
use glam::Vec2;
use std::collections::BTreeMap;

pub(super) const LABELS: [&str; 7] = [
    "Без привязки",
    "Вершины",
    "Середины рёбер",
    "Рёбра",
    "Поверхность",
    "Сетка",
    "Автоматически",
];
const CELL: f32 = 24.;
#[derive(Default)]
pub(super) struct State {
    pub mode: u8,
    pub step: f32,
    snapshot: Option<Snapshot>,
    feedback: Option<(Vec3, &'static str)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> App {
        let mut a = App::new(None);
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        a.preferences = Preferences::default();
        a.width = 1440.;
        a.height = 1050.;
        let mut one = poseforge_core::Node::new(1, "Moving");
        one.mesh = Some(10);
        let mut two = one.clone();
        two.id = 2;
        two.transform.translation = Vec3::X * 4.;
        a.editor.project.nodes = vec![one, two];
        a.editor.project.meshes =
            vec![
                poseforge_core::PrimitiveSettings::new(poseforge_core::PrimitiveKind::Cube)
                    .mesh(10, None)
                    .unwrap(),
            ];
        a.editor.selected = Some(1);
        a.tool = Tool::Move;
        a.authoring.xray = true;
        a.frame_all();
        a
    }
    #[test]
    fn vertex_midpoint_edge_and_surface_targets_exclude_moving_instances() {
        let mut a = scene();
        let mesh = &a.editor.project.meshes[0];
        let g = poseforge_core::MeshConnectivity::new(mesh);
        let &(i, j) = g.edges.first().unwrap();
        let p = Vec3::from_array(mesh.vertices[i].position) + Vec3::X * 4.;
        let q = Vec3::from_array(mesh.vertices[j].position) + Vec3::X * 4.;
        for (mode, target) in [(1, p), (2, p.lerp(q, 0.5)), (3, p.lerp(q, 0.31))] {
            a.authoring.snapping.mode = mode;
            a.begin_snapping(false);
            let result = a.snap_translation(target);
            assert!(result.distance(target) < 0.0001);
            assert!(a.authoring.snapping.feedback.is_some());
        }
        let state = a.authoring.snapping.snapshot.as_ref().unwrap();
        assert!(state.points.iter().all(|p| p.world.x >= 3.));
        a.authoring.snapping.mode = 4;
        a.begin_snapping(false);
        let result = a.snap_translation(Vec3::X * 4.);
        assert!(a.authoring.snapping.feedback.is_some());
        assert!((result - Vec3::X * 4.).abs().max_element() >= 0.99);
        a.authoring.snapping.clear();
        assert!(a.authoring.snapping.feedback.is_none());
        assert!(a.authoring.snapping.snapshot.is_none());
    }
    #[test]
    fn snapping_gizmo_converts_world_destination_to_scaled_parent_coordinates() {
        let a = scene();
        let parent = Mat4::from_scale_rotation_translation(
            Vec3::new(2., 3., 4.),
            glam::Quat::from_rotation_y(0.4),
            Vec3::new(1., 2., 3.),
        );
        let original = Transform::from_translation(Vec3::new(0.2, 0.3, 0.4));
        let world = parent * original.matrix();
        let gizmo = crate::gizmo::Gizmo::new(
            crate::gizmo::Mode::Move,
            world,
            false,
            a.view(),
            a.viewport(),
        )
        .unwrap();
        let screen = authoring_controls::project(
            a.view().view_projection(),
            a.viewport(),
            world.w_axis.truncate(),
        )
        .unwrap();
        let drag = crate::gizmo::GizmoDrag::begin(
            gizmo,
            crate::gizmo::Handle::Center,
            original,
            parent,
            screen,
        )
        .unwrap();
        let target = Vec3::new(3., 4., 2.);
        let (transform, actual) = drag.retarget_translation(target);
        assert!(actual.distance(target) < 1e-5);
        assert!(
            parent
                .transform_point3(transform.translation)
                .distance(target)
                < 1e-5
        );
    }
}
impl State {
    pub fn clear(&mut self) {
        self.snapshot = None;
        self.feedback = None;
    }
}
struct Surface {
    inverse: Mat4,
    cache: mesh_pick::MeshCache,
}
struct Point {
    world: Vec3,
    screen: Vec2,
    kind: u8,
}
struct Edge {
    a: Vec3,
    b: Vec3,
    sa: Vec2,
    sb: Vec2,
    wa: f32,
    wb: f32,
}
struct Snapshot {
    projection: Mat4,
    viewport: [f32; 4],
    points: Vec<Point>,
    edges: Vec<Edge>,
    surfaces: Vec<Surface>,
    point_cells: BTreeMap<(i32, i32), Vec<usize>>,
    edge_cells: BTreeMap<(i32, i32), Vec<usize>>,
    wide_edges: Vec<usize>,
}
fn cell(p: Vec2) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}
impl Snapshot {
    fn visible(&self, p: Vec3) -> bool {
        let Some(screen) = authoring_controls::project(self.projection, self.viewport, p) else {
            return false;
        };
        let Some((o, d)) =
            authoring_controls::ray(self.projection.inverse(), self.viewport, screen)
        else {
            return false;
        };
        let distance = (p - o).dot(d);
        !self.surfaces.iter().any(|s| {
            s.cache
                .hit(
                    s.inverse.transform_point3(o),
                    s.inverse.transform_vector3(d),
                )
                .is_some_and(|(_, hit)| hit + distance.abs().max(1.) * 0.0002 < distance)
        })
    }
    fn target(&self, raw: Vec3, mode: u8, xray: bool) -> Option<(Vec3, &'static str)> {
        let screen = Vec2::from_array(authoring_controls::project(
            self.projection,
            self.viewport,
            raw,
        )?);
        let c = cell(screen);
        let mut best: Option<(f32, Vec3, &'static str)> = None;
        let mut consider = |p: Vec3, distance: f32, label: &'static str| {
            if distance <= 12.
                && best.is_none_or(|(d, _, _)| distance < d)
                && (xray || self.visible(p))
            {
                best = Some((distance, p, label));
            }
        };
        if matches!(mode, 1 | 2 | 6) {
            for x in c.0 - 1..=c.0 + 1 {
                for y in c.1 - 1..=c.1 + 1 {
                    for &index in self.point_cells.get(&(x, y)).into_iter().flatten() {
                        let p = &self.points[index];
                        if mode == 6 || mode == p.kind {
                            consider(
                                p.world,
                                p.screen.distance(screen),
                                if p.kind == 1 {
                                    "Вершина"
                                } else {
                                    "Середина ребра"
                                },
                            );
                        }
                    }
                }
            }
        }
        // Prefer explicit points in automatic mode, then edges, then a surface.
        if best.is_none() && matches!(mode, 3 | 6) {
            let mut indices: BTreeSet<_> = self.wide_edges.iter().copied().collect();
            for x in c.0 - 1..=c.0 + 1 {
                for y in c.1 - 1..=c.1 + 1 {
                    indices.extend(self.edge_cells.get(&(x, y)).into_iter().flatten().copied());
                }
            }
            for i in indices {
                let e = &self.edges[i];
                let line = e.sb - e.sa;
                let t =
                    ((screen - e.sa).dot(line) / line.length_squared().max(1e-12)).clamp(0., 1.);
                let u = t * e.wa / (e.wb * (1. - t) + e.wa * t);
                let p = e.a.lerp(e.b, u);
                let distance = (e.sa + line * t).distance(screen);
                if distance <= 12.
                    && best.is_none_or(|(d, _, _)| distance < d)
                    && (xray || self.visible(p))
                {
                    best = Some((distance, p, "Ребро"));
                }
            }
        }
        if let Some((_, p, label)) = best {
            return Some((p, label));
        }
        if matches!(mode, 4 | 6) {
            let (o, d) = authoring_controls::ray(
                self.projection.inverse(),
                self.viewport,
                screen.to_array(),
            )?;
            return self
                .surfaces
                .iter()
                .filter_map(|s| {
                    s.cache
                        .hit(
                            s.inverse.transform_point3(o),
                            s.inverse.transform_vector3(d),
                        )
                        .map(|(_, t)| t)
                })
                .min_by(f32::total_cmp)
                .map(|t| (o + d * t, "Поверхность"));
        }
        None
    }
}
impl App {
    pub(super) fn begin_snapping(&mut self, components: bool) {
        self.authoring.snapping.clear();
        if self.tool != Tool::Move || self.authoring.snapping.mode == 0 {
            return;
        }
        let Ok(eval) = self.evaluated_interactive() else {
            return;
        };
        let projection = self.view().view_projection();
        let viewport = self.viewport();
        let mut snapshot = Snapshot {
            projection,
            viewport,
            points: vec![],
            edges: vec![],
            surfaces: vec![],
            point_cells: BTreeMap::new(),
            edge_cells: BTreeMap::new(),
            wide_edges: vec![],
        };
        let selected = self.object_selection();
        let vertices = self.edit_vertices();
        for node in &self.editor.project.nodes {
            if !self.editor.project.node_visible(node.id, false) {
                continue;
            }
            if !components {
                let mut cursor = Some(node.id);
                let mut excluded = false;
                while let Some(id) = cursor {
                    if selected.contains(&id) {
                        excluded = true;
                        break;
                    }
                    cursor = self.editor.project.node(id).and_then(|n| n.parent);
                }
                if excluded {
                    continue;
                }
            }
            let own = components && self.authoring.editing == Some(node.id);
            let source = node.mesh.and_then(|id| self.editor.project.mesh(id));
            let Some(mesh) = (if own {
                source
            } else {
                eval.modifier_meshes
                    .get(&node.id)
                    .or_else(|| eval.text_meshes.get(&node.id))
                    .or(source)
            }) else {
                continue;
            };
            let Some(&model) = eval.global.get(&node.id) else {
                continue;
            };
            let inverse = model.inverse();
            if !inverse.is_finite() {
                continue;
            }
            let graph = poseforge_core::MeshConnectivity::new(mesh);
            let excluded: BTreeSet<_> = if own {
                vertices
                    .iter()
                    .filter_map(|&i| graph.canonical.get(i).copied())
                    .collect()
            } else {
                BTreeSet::new()
            };
            let mut add_point = |world: Vec3, kind: u8| {
                if let Some(screen) = authoring_controls::project(projection, viewport, world) {
                    let screen = Vec2::from_array(screen);
                    snapshot
                        .point_cells
                        .entry(cell(screen))
                        .or_default()
                        .push(snapshot.points.len());
                    snapshot.points.push(Point {
                        world,
                        screen,
                        kind,
                    });
                }
            };
            for (i, v) in mesh.vertices.iter().enumerate() {
                if graph.canonical[i] == i && !excluded.contains(&i) {
                    add_point(model.transform_point3(Vec3::from_array(v.position)), 1);
                }
            }
            for &(a, b) in &graph.edges {
                if excluded.contains(&a) || excluded.contains(&b) {
                    continue;
                }
                let a = model.transform_point3(Vec3::from_array(mesh.vertices[a].position));
                let b = model.transform_point3(Vec3::from_array(mesh.vertices[b].position));
                add_point(a.lerp(b, 0.5), 2);
                if let (Some(sa), Some(sb)) = (
                    authoring_controls::project(projection, viewport, a),
                    authoring_controls::project(projection, viewport, b),
                ) {
                    let sa = Vec2::from_array(sa);
                    let sb = Vec2::from_array(sb);
                    let lo = cell(sa.min(sb));
                    let hi = cell(sa.max(sb));
                    let id = snapshot.edges.len();
                    if (hi.0 - lo.0 + 1) as i64 * (hi.1 - lo.1 + 1) as i64 > 256 {
                        snapshot.wide_edges.push(id);
                    } else {
                        for x in lo.0..=hi.0 {
                            for y in lo.1..=hi.1 {
                                snapshot.edge_cells.entry((x, y)).or_default().push(id);
                            }
                        }
                    }
                    snapshot.edges.push(Edge {
                        a,
                        b,
                        sa,
                        sb,
                        wa: (projection * a.extend(1.)).w,
                        wb: (projection * b.extend(1.)).w,
                    });
                }
            }
            let mut surface = mesh.clone();
            if own {
                surface.indices = mesh
                    .indices
                    .chunks_exact(3)
                    .filter(|t| {
                        t.iter()
                            .all(|&v| !excluded.contains(&graph.canonical[v as usize]))
                    })
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>()
                    .into();
                surface.topology = None;
            }
            snapshot.surfaces.push(Surface {
                inverse,
                cache: mesh_pick::MeshCache::new(&surface),
            });
        }
        self.authoring.snapping.snapshot = Some(snapshot);
    }
    pub(super) fn snap_translation(&mut self, raw: Vec3) -> Vec3 {
        let state = &mut self.authoring.snapping;
        state.feedback = None;
        let target = if state.mode == 5 {
            let step = if state.step > 0. { state.step } else { 0.1 };
            Some(((raw / step).round() * step, "Сетка"))
        } else {
            state
                .snapshot
                .as_ref()
                .and_then(|s| s.target(raw, state.mode, self.authoring.xray))
        };
        if let Some((p, label)) = target {
            state.feedback = Some((p, label));
            p
        } else {
            raw
        }
    }
    pub(super) fn snap_action(&mut self, a: A) -> bool {
        match a {
            A::SnapMode(mode) => {
                self.authoring.snapping.mode = mode.min(6);
                self.authoring.snapping.clear();
            }
            A::SnapStep => self.input(
                Action::Author(a),
                ui::number_caption(self.snap_step() as f64),
            ),
            _ => return false,
        }
        true
    }
    fn snap_step(&self) -> f32 {
        if self.authoring.snapping.step > 0. {
            self.authoring.snapping.step
        } else {
            0.1
        }
    }
    pub(super) fn draw_snapping(&self, u: &mut ui::Ui) {
        if self.editor.mode != EditorMode::Scene
            || self.tool != Tool::Move
            || self.authoring.knife.is_some()
            || self.primitive_panel.is_some()
            || self.modeling_live.is_some()
            || self.uv.open
        {
            return;
        }
        let area = self.modeling_area();
        if area[2] < 160. || area[3] < 180. {
            return;
        }
        let p = u.palette();
        let width = 268_f32.min(area[2]);
        let x = area[0];
        let y = area[1] + area[3] - 44.;
        u.outlined_surface([x, y, width, 38.], p.panel, p.border);
        u.button(
            [x + 4., y + 4., width - 94., 30.],
            LABELS[self.authoring.snapping.mode as usize],
            Action::DropdownOpen(DropdownKind::SnapMode),
            self.authoring.snapping.mode != 0,
        );
        u.field(
            [x + width - 86., y + 4., 82., 30.],
            &format!("Шаг {}", ui::number_caption(self.snap_step() as f64)),
            Action::Author(A::SnapStep),
        );
        if let Some((world, label)) = self.authoring.snapping.feedback {
            if let Some(q) =
                authoring_controls::project(self.view().view_projection(), self.viewport(), world)
            {
                u.outlined_surface([q[0] - 6., q[1] - 6., 12., 12.], p.panel, p.accent);
                u.mesh_segment([q[0] - 10., q[1]], [q[0] + 10., q[1]], 1.5, p.accent);
                u.mesh_segment([q[0], q[1] - 10.], [q[0], q[1] + 10.], 1.5, p.accent);
                let tx = (q[0] + 16.).min(area[0] + area[2] - 148.);
                let ty = (q[1] - 30.).max(area[1]);
                u.surface([tx, ty, 144., 24.], p.panel);
                u.label(tx + 6., ty + 4., label, 12., p.text);
            }
        }
    }
}
