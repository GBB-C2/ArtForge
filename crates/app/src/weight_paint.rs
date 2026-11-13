//! Normalized four-influence painting with a cached, visible-surface overlay.
use super::*;
use crate::ui::AuthorAction as A;
use poseforge_core::{Mesh, Vertex};
use poseforge_simulation::mesh_contact::{MeshCollider, Triangle};
use std::cell::RefCell;
pub(super) struct WeightPaint {
    pub target: Option<u64>,
    pub joint: usize,
    pub radius: f32,
    pub strength: f32,
    cache: RefCell<Option<Cache>>,
}
impl Default for WeightPaint {
    fn default() -> Self {
        Self {
            target: None,
            joint: 0,
            radius: 48.,
            strength: 0.25,
            cache: RefCell::new(None),
        }
    }
}
struct Cache {
    project: Project,
    globals: BTreeMap<u64, Mat4>,
    view: Mat4,
    viewport: [f32; 4],
    world: Vec<Vec3>,
    bvh: MeshCollider,
    target: u64,
    points: Vec<Option<[f32; 2]>>,
    visible: Vec<bool>,
}
impl Cache {
    fn reproject(&mut self, app: &App) {
        let view = app.view();
        self.view = view.view_projection();
        self.viewport = view.viewport;
        let inverse = self.view.inverse();
        let mut scratch = vec![];
        self.points = self.world.iter().map(|p| app.project_point(*p)).collect();
        self.visible = self
            .world
            .iter()
            .zip(&self.points)
            .map(|(point, screen)| {
                let Some(screen) = screen else {
                    return false;
                };
                let x = (screen[0] - view.viewport[0]) / view.viewport[2] * 2. - 1.;
                let y = 1. - (screen[1] - view.viewport[1]) / view.viewport[3] * 2.;
                let start = inverse.project_point3(Vec3::new(x, y, 0.));
                !self.bvh.occluded(start, *point, &mut scratch)
            })
            .collect();
    }
}
#[derive(Clone)]
pub(super) struct WeightStroke {
    node: u64,
    mesh: u64,
    joint: usize,
    joints: usize,
    previous: [f32; 2],
}
fn positions(
    p: &Project,
    eval: &poseforge_core::EvaluatedScene,
    id: u64,
    mesh: &Mesh,
) -> Option<Vec<Vec3>> {
    let n = p.node(id)?;
    let model = *eval.global.get(&id)?;
    let joints: Option<Vec<_>> = n
        .skin
        .as_ref()
        .map(|skin| {
            skin.joints
                .iter()
                .zip(&skin.inverse_bind_matrices)
                .map(|(j, b)| Some(*eval.global.get(j)? * *b))
                .collect::<Option<Vec<_>>>()
        })
        .transpose_option()?;
    let morphs = eval.morph_weights.get(&id).unwrap_or(&n.morph_weights);
    Some(
        mesh.vertices
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let mut point = Vec3::from_array(v.position);
                for (t, &w) in mesh.morph_targets.iter().zip(morphs) {
                    if w != 0. {
                        point += Vec3::from_array(t.positions[i]) * w;
                    }
                }
                if let Some(joints) = &joints {
                    if v.weights.iter().sum::<f32>() > 0.0001 {
                        return (0..4)
                            .filter(|i| v.weights[*i] > 0.)
                            .map(|i| {
                                joints[v.joints[i] as usize].transform_point3(point) * v.weights[i]
                            })
                            .sum();
                    }
                }
                model.transform_point3(point)
            })
            .collect(),
    )
}
trait TransposeOption<T> {
    fn transpose_option(self) -> Option<Option<T>>;
}
impl<T> TransposeOption<T> for Option<Option<T>> {
    fn transpose_option(self) -> Option<Option<T>> {
        match self {
            None => Some(None),
            Some(v) => v.map(Some),
        }
    }
}
fn paint_vertex(v: &mut Vertex, joint: u32, count: usize, amount: f32) {
    let mut values = BTreeMap::<u32, f32>::new();
    for i in 0..4 {
        if v.weights[i] > 0. {
            *values.entry(v.joints[i]).or_default() += v.weights[i];
        }
    }
    let old = values.remove(&joint).unwrap_or(0.).clamp(0., 1.);
    let target = if amount >= 0. {
        old + (1. - old) * amount.min(1.)
    } else {
        old * (1. + amount.max(-1.))
    };
    if values.is_empty() && count > 1 {
        values.insert(if joint == 0 { 1 } else { 0 }, 1.);
    }
    let mut others: Vec<_> = values.into_iter().collect();
    others.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    others.truncate(3);
    let sum = others.iter().map(|p| p.1).sum::<f32>();
    v.joints = [joint; 4];
    v.weights = [0.; 4];
    v.weights[0] = if sum > 0. { target } else { 1. };
    for (i, (id, w)) in others.into_iter().enumerate() {
        v.joints[i + 1] = id;
        v.weights[i + 1] = w / sum * (1. - target);
    }
}
impl App {
    pub(super) fn weight_action(&mut self, action: A) -> bool {
        match action {
            A::Paint => {
                if self.weights.target.take().is_some() {
                    return true;
                }
                let target = self.editor.selected.or(self.authoring.bind_target);
                let Some(id) = target.filter(|id| self.node_selectable(*id)) else {
                    self.status = "Выберите доступный меш со скелетом".into();
                    return true;
                };
                let valid = self.editor.project.node(id).is_some_and(|n| {
                    n.skin.is_some()
                        && n.mesh
                            .and_then(|m| self.editor.project.mesh(m))
                            .is_some_and(|m| m.vertices.len() <= 20000)
                });
                if !valid {
                    self.status = "Кисть: меш со скелетом, до 20000 вершин.".into();
                    return true;
                }
                self.authoring.editing = None;
                self.weights.target = Some(id);
                self.weights.joint = 0;
                self.weights.cache.replace(None);
                self.status =
                    "Кисть весов: ЛКМ — добавить, Ctrl + ЛКМ — убрать; Esc отменяет мазок.".into();
            }
            A::PaintJoint(delta) => {
                if let Some(count) = self
                    .weights
                    .target
                    .and_then(|id| self.editor.project.node(id))
                    .and_then(|n| n.skin.as_ref())
                    .map(|s| s.joints.len())
                {
                    self.weights.joint = (self.weights.joint as i32 + delta as i32)
                        .rem_euclid(count as i32) as usize;
                }
            }
            A::PaintRadius => self.input(Action::Author(action), self.weights.radius.to_string()),
            A::PaintStrength => {
                self.input(Action::Author(action), self.weights.strength.to_string())
            }
            _ => return false,
        }
        true
    }
    fn update_weight_cache(&self) -> bool {
        let Some(id) = self.weights.target.filter(|id| self.node_selectable(*id)) else {
            return false;
        };
        let Ok(eval) = self.evaluated_interactive() else {
            return false;
        };
        let matrix = self.view().view_projection();
        {
            let mut cached = self.weights.cache.borrow_mut();
            if let Some(c) = cached.as_mut().filter(|c| {
                c.target == id && c.globals == eval.global && c.project == self.editor.project
            }) {
                if c.view != matrix || c.viewport != self.view().viewport {
                    c.reproject(self);
                }
                return true;
            }
        }
        let p = &self.editor.project;
        let Some(mesh) = p.node(id).and_then(|n| n.mesh).and_then(|m| p.mesh(m)) else {
            return false;
        };
        let Some(points) = positions(p, &eval, id, mesh) else {
            return false;
        };
        let visible_ids = p.visible_node_ids(false);
        let mut triangles = vec![];
        for n in &p.nodes {
            if !visible_ids.contains(&n.id) {
                continue;
            }
            let Some(m) = n.mesh.and_then(|m| p.mesh(m)) else {
                continue;
            };
            let Some(vertices) = positions(p, &eval, n.id, m) else {
                continue;
            };
            if triangles.len() + m.indices.len() / 3 > 200000 {
                return false;
            }
            triangles.extend(
                m.indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|t| Triangle(t.map(|i| vertices[i as usize]))),
            );
        }
        let Ok(bvh) = MeshCollider::new(triangles) else {
            return false;
        };
        let mut cached = Cache {
            project: p.clone(),
            globals: eval.global,
            view: matrix,
            viewport: self.view().viewport,
            world: points,
            bvh,
            target: id,
            points: vec![],
            visible: vec![],
        };
        cached.reproject(self);
        self.weights.cache.replace(Some(cached));
        true
    }
    pub(super) fn weight_pointer_down(&mut self) -> bool {
        let Some(id) = self.weights.target else {
            return false;
        };
        if !self.update_weight_cache() {
            self.status =
                "Кисть недоступна: проверьте блокировку и размер сцены (до 200000 треугольников)."
                    .into();
            return true;
        }
        let Some(n) = self.editor.project.node(id) else {
            return true;
        };
        let Some(skin) = &n.skin else {
            return true;
        };
        let count = skin.joints.len();
        if count == 0 {
            return true;
        }
        let mesh_id = n.mesh.unwrap();
        let joint = self.weights.joint.min(count - 1);
        let can_paint = self.weights.cache.borrow().as_ref().is_some_and(|c| {
            c.points.iter().zip(&c.visible).any(|(p, v)| {
                *v && p.is_some_and(|p| {
                    (p[0] - self.mouse[0]).hypot(p[1] - self.mouse[1]) < self.weights.radius
                })
            })
        });
        if !can_paint {
            return true;
        }
        self.editor.begin_edit("Нарисовать веса кости");
        let mesh_id = if self
            .editor
            .project
            .nodes
            .iter()
            .filter(|n| n.mesh == Some(mesh_id))
            .count()
            > 1
        {
            let mut mesh = self.editor.project.mesh(mesh_id).unwrap().clone();
            mesh.id = self.editor.project.next_id();
            let id = mesh.id;
            self.editor.project.meshes.push(mesh);
            self.editor
                .project
                .node_mut(self.weights.target.unwrap())
                .unwrap()
                .mesh = Some(id);
            id
        } else {
            mesh_id
        };
        let stroke = WeightStroke {
            node: id,
            mesh: mesh_id,
            joint,
            joints: count,
            previous: self.mouse,
        };
        self.weight_dab(&stroke, self.mouse, 1.);
        self.drag = Some(Drag::Weights(stroke));
        self.drag_button = Some(MouseButton::Left);
        true
    }
    fn weight_dab(&mut self, stroke: &WeightStroke, pointer: [f32; 2], scale: f32) {
        if self.weights.target != Some(stroke.node)
            || !self.node_selectable(stroke.node)
            || !self.update_weight_cache()
        {
            return;
        }
        let amounts = {
            let cache = self.weights.cache.borrow();
            let cache = cache.as_ref().unwrap();
            let radius = self.weights.radius;
            let sign = if self.modifiers.control_key() {
                -1.
            } else {
                1.
            };
            let strength = self.weights.strength * scale * sign;
            let amounts: Vec<_> = cache
                .points
                .iter()
                .zip(&cache.visible)
                .enumerate()
                .filter_map(|(i, (p, visible))| {
                    if !visible {
                        return None;
                    }
                    let p = p.as_ref()?;
                    let d =
                        ((p[0] - pointer[0]).powi(2) + (p[1] - pointer[1]).powi(2)).sqrt() / radius;
                    (d < 1.).then_some((i, strength * (1. - d * d).powi(2)))
                })
                .collect();
            amounts
        };
        if amounts.is_empty() {
            return;
        }
        if let Some(mesh) = self
            .editor
            .project
            .meshes
            .iter_mut()
            .find(|m| m.id == stroke.mesh)
        {
            for (i, amount) in amounts {
                paint_vertex(
                    &mut mesh.vertices[i],
                    stroke.joint as u32,
                    stroke.joints,
                    amount,
                );
            }
        }
        self.invalidate();
    }
    pub(super) fn update_weight_stroke(&mut self, mut stroke: WeightStroke, p: [f32; 2]) {
        let delta = [p[0] - stroke.previous[0], p[1] - stroke.previous[1]];
        let distance = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
        let spacing = (self.weights.radius * 0.2).max(4.);
        let steps = (distance / spacing).ceil().clamp(1., 64.) as usize;
        if distance > 0.1 {
            for step in 1..=steps {
                let t = step as f32 / steps as f32;
                self.weight_dab(
                    &stroke,
                    [
                        stroke.previous[0] + delta[0] * t,
                        stroke.previous[1] + delta[1] * t,
                    ],
                    (distance / steps as f32 / spacing).min(1.),
                );
            }
        }
        stroke.previous = p;
        self.drag = Some(Drag::Weights(stroke));
    }
    pub(super) fn draw_weights(&self, u: &mut Ui) {
        if !self.update_weight_cache() {
            return;
        }
        let cache = self.weights.cache.borrow();
        let c = cache.as_ref().unwrap();
        let Some(mesh) = self
            .editor
            .project
            .node(c.target)
            .and_then(|n| n.mesh)
            .and_then(|id| self.editor.project.mesh(id))
        else {
            return;
        };
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        for (i, p) in c.points.iter().enumerate() {
            if !c.visible[i] {
                continue;
            }
            let Some([x, y]) = p else {
                continue;
            };
            let v = &mesh.vertices[i];
            let w = (0..4)
                .filter(|j| v.joints[*j] as usize == self.weights.joint)
                .map(|j| v.weights[j])
                .sum::<f32>()
                .clamp(0., 1.);
            let color = if w < 0.5 {
                [0., w * 2., 1. - w, 0.9]
            } else {
                [(w - 0.5) * 2., 1. - (w - 0.5) * 2., 0., 0.9]
            };
            u.rounded_rect([x - 2.5, y - 2.5, 5., 5.], color, 2.5);
        }
        let v = self.viewport();
        u.label(
            v[0] + 140.,
            v[1] + 48.,
            "ВЕСА · синий 0 → красный 1 · Ctrl вычитает",
            12.,
            u.palette().text,
        );
        if self.canvas_input_at(self.mouse) {
            let r = self.weights.radius;
            for i in 0..64 {
                let a = i as f32 * std::f32::consts::TAU / 64.;
                let b = (i + 1) as f32 * std::f32::consts::TAU / 64.;
                u.smooth_segment(
                    [self.mouse[0] + r * a.cos(), self.mouse[1] + r * a.sin()],
                    [self.mouse[0] + r * b.cos(), self.mouse[1] + r * b.sin()],
                    1.,
                    [1., 1., 1., 0.7],
                );
            }
        }
        clip_scrolled(u, start, v, 0.);
    }
    pub(super) fn draw_weight_controls(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        u.button(
            [x, y, w, 30.],
            if self.weights.target.is_some() {
                "Завершить рисование весов"
            } else {
                "Рисовать веса костей"
            },
            Action::Author(A::Paint),
            self.weights.target.is_some(),
        );
        let name = self
            .weights
            .target
            .and_then(|id| self.editor.project.node(id))
            .and_then(|n| n.skin.as_ref())
            .and_then(|s| s.joints.get(self.weights.joint))
            .and_then(|id| self.editor.project.node(*id))
            .map_or("Кость не выбрана", |n| n.name.as_str());
        u.button(
            [x, y + 40., 28., 28.],
            "‹",
            Action::Author(A::PaintJoint(-1)),
            false,
        );
        u.fitted_label(
            [x + 34., y + 46., w - 68., 20.],
            name,
            11.,
            u.palette().text,
        );
        u.button(
            [x + w - 28., y + 40., 28., 28.],
            "›",
            Action::Author(A::PaintJoint(1)),
            false,
        );
        u.number_field(
            [x, y + 80., w, 30.],
            "Радиус кисти, px",
            self.weights.radius as f64,
            Action::Author(A::PaintRadius),
            crate::ui::SliderRange::new(4., 256., 1.),
        );
        u.number_field(
            [x, y + 120., w, 30.],
            "Сила кисти",
            self.weights.strength as f64,
            Action::Author(A::PaintStrength),
            crate::ui::SliderRange::new(0.01, 1., 0.01),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weights_replace_weakest_keep_four_influences_and_normalize_after_erasing() {
        let mut v = Vertex {
            joints: [0, 1, 2, 3],
            weights: [0.4, 0.3, 0.2, 0.1],
            ..Default::default()
        };
        paint_vertex(&mut v, 4, 5, 0.5);
        assert!(v.joints.contains(&4));
        assert!(!v.joints.contains(&3));
        assert!((v.weights.iter().sum::<f32>() - 1.).abs() < 1e-6);
        paint_vertex(&mut v, 4, 5, -1.);
        assert_eq!(v.weights[0], 0.);
        assert!((v.weights.iter().sum::<f32>() - 1.).abs() < 1e-6);
        let mut single = Vertex {
            weights: [1., 0., 0., 0.],
            ..Default::default()
        };
        paint_vertex(&mut single, 0, 1, -1.);
        assert_eq!(single.weights, [1., 0., 0., 0.]);
    }
}
