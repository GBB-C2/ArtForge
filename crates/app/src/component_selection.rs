//! Component marquee/lasso selection and reversible transient selection state.
use super::*;
use crate::ui::MeshSelection as S;
#[derive(Clone)]
pub(super) struct SelectionSnapshot {
    pub vertices: BTreeSet<usize>,
    pub edges: BTreeSet<(usize, usize)>,
    pub faces: BTreeSet<usize>,
    pub mode: S,
}
impl SelectionSnapshot {
    pub fn capture(a: &authoring_controls::AuthoringState) -> Self {
        Self {
            vertices: a.vertices.clone(),
            edges: a.edges.clone(),
            faces: a.faces.clone(),
            mode: a.selection_mode,
        }
    }
    pub fn restore(&self, a: &mut authoring_controls::AuthoringState) {
        a.vertices = self.vertices.clone();
        a.edges = self.edges.clone();
        a.faces = self.faces.clone();
        a.selection_mode = self.mode;
    }
}
#[derive(Clone)]
pub(super) struct ComponentSelection {
    pub original: SelectionSnapshot,
    pub path: Vec<[f32; 2]>,
    pub lasso: bool,
    pub add: bool,
    pub subtract: bool,
}
fn inside(path: &[[f32; 2]], p: [f32; 2], lasso: bool) -> bool {
    if !lasso {
        let a = path[0];
        let b = path[path.len() - 1];
        return p[0] >= a[0].min(b[0])
            && p[0] <= a[0].max(b[0])
            && p[1] >= a[1].min(b[1])
            && p[1] <= a[1].max(b[1]);
    }
    let mut inside = false;
    for i in 0..path.len() {
        let a = path[i];
        let b = path[(i + 1) % path.len()];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}
impl App {
    pub(super) fn begin_component_selection(&mut self) -> bool {
        if self.authoring.editing.is_none()
            || self.authoring.selection_tool == 0
            || self.tool == Tool::Orbit
        {
            return false;
        }
        self.drag = Some(Drag::ComponentSelection(ComponentSelection {
            original: SelectionSnapshot::capture(&self.authoring),
            path: vec![self.mouse, self.mouse],
            lasso: self.authoring.selection_tool == 2,
            add: self.modifiers.shift_key(),
            subtract: self.modifiers.control_key(),
        }));
        self.drag_button = Some(MouseButton::Left);
        self.status = "Выделение: Shift — добавить, Ctrl — вычесть, Esc — отменить".into();
        true
    }
    pub(super) fn update_component_selection(&mut self, mut drag: ComponentSelection, p: [f32; 2]) {
        if drag.lasso {
            let last = drag.path[drag.path.len() - 1];
            if (p[0] - last[0]).hypot(p[1] - last[1]) >= 2. {
                if drag.path.len() >= 256 {
                    drag.path = drag.path.iter().step_by(2).copied().collect();
                }
                drag.path.push(p);
            }
        } else {
            drag.path[1] = p;
        }
        self.drag = Some(Drag::ComponentSelection(drag));
        self.redraw();
    }
    pub(super) fn finish_component_selection(&mut self, drag: ComponentSelection) {
        let Some(id) = self.authoring.editing else {
            return;
        };
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            return;
        };
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        if drag.lasso && drag.path.len() < 3 {
            return;
        }
        let model = globals[&id];
        let inverse = model.inverse();
        let view = self.view();
        let projection = view.view_projection();
        let unproject = projection.inverse();
        let viewport = self.viewport();
        let cache = self.mesh_cache(mesh);
        let project = |p: Vec3| {
            let clip = projection * p.extend(1.);
            if clip.w <= 0. {
                return None;
            }
            let q = clip.truncate() / clip.w;
            if !(0.0..=1.).contains(&q.z) {
                return None;
            }
            Some([
                viewport[0] + (q.x * 0.5 + 0.5) * viewport[2],
                viewport[1] + (0.5 - q.y * 0.5) * viewport[3],
            ])
        };
        let pick = |local: Vec3| {
            let world = model.transform_point3(local);
            let Some(p) = project(world) else {
                return false;
            };
            if !ui::contains(viewport, p) || !inside(&drag.path, p, drag.lasso) {
                return false;
            }
            if self.authoring.xray {
                return true;
            }
            let x = (p[0] - viewport[0]) / viewport[2] * 2. - 1.;
            let y = 1. - (p[1] - viewport[1]) / viewport[3] * 2.;
            let origin = unproject.project_point3(Vec3::new(x, y, 0.));
            let direction = (unproject.project_point3(Vec3::new(x, y, 0.5)) - origin).normalize();
            let distance = (world - origin).dot(direction);
            cache
                .hit(
                    inverse.transform_point3(origin),
                    inverse.transform_vector3(direction),
                )
                .is_none_or(|(_, hit)| hit + distance.abs().max(1.) * 0.0001 >= distance)
        };
        let position = |i: usize| Vec3::from_array(mesh.vertices[i].position);
        let mut vertices = BTreeSet::new();
        let mut edges = BTreeSet::new();
        let mut faces = BTreeSet::new();
        match drag.original.mode {
            S::Vertex => {
                for (i, &c) in cache.graph.canonical.iter().enumerate() {
                    if i == c && pick(position(i)) {
                        vertices.insert(i);
                    }
                }
            }
            S::Edge => {
                for &e in &cache.graph.edges {
                    if pick((position(e.0) + position(e.1)) * 0.5) {
                        edges.insert(e);
                    }
                }
            }
            S::Face => {
                for f in 0..mesh.face_count() {
                    let face = mesh.face_vertices(f).unwrap();
                    let p = face.iter().map(|&v| position(v as usize)).sum::<Vec3>()
                        / face.len() as f32;
                    if pick(p) {
                        faces.insert(f);
                    }
                }
            }
        }
        fn apply<T: Ord + Clone>(
            old: &BTreeSet<T>,
            next: BTreeSet<T>,
            add: bool,
            subtract: bool,
        ) -> BTreeSet<T> {
            if subtract {
                old.difference(&next).cloned().collect()
            } else if add {
                old.union(&next).cloned().collect()
            } else {
                next
            }
        }
        match drag.original.mode {
            S::Vertex => {
                self.authoring.vertices =
                    apply(&drag.original.vertices, vertices, drag.add, drag.subtract)
            }
            S::Edge => {
                self.authoring.edges = apply(&drag.original.edges, edges, drag.add, drag.subtract)
            }
            S::Face => {
                self.authoring.faces = apply(&drag.original.faces, faces, drag.add, drag.subtract)
            }
        }
        self.status = "Выделение обновлено · G/R/S — трансформация".into();
        self.redraw();
    }
    pub(super) fn draw_component_selection(&self, u: &mut Ui) {
        let Some(Drag::ComponentSelection(d)) = &self.drag else {
            return;
        };
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        let color = u.palette().accent;
        let points = if d.lasso {
            d.path.clone()
        } else {
            let a = d.path[0];
            let b = d.path[1];
            vec![a, [b[0], a[1]], b, [a[0], b[1]]]
        };
        for i in 0..points.len() {
            u.mesh_segment(points[i], points[(i + 1) % points.len()], 1.5, color);
        }
        clip_scrolled(u, start, self.viewport(), 0.);
    }
    pub(super) fn mesh_double_click(&mut self) -> bool {
        let now = Instant::now();
        let double = self.authoring.last_click.is_some_and(|(time, p, mode)| {
            now.duration_since(time) < Duration::from_millis(350)
                && mode == self.authoring.selection_mode
                && (p[0] - self.mouse[0]).hypot(p[1] - self.mouse[1]) < 5.
        });
        self.authoring.last_click = if double {
            None
        } else {
            Some((now, self.mouse, self.authoring.selection_mode))
        };
        if double {
            self.modeling_action(ui::AuthorAction::Connected);
            self.status = "Выбраны связанные компоненты".into();
        }
        double
    }
}
