//! Component tool settings, selection expansion and exact transform entry.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S};

impl App {
    pub(super) fn selected_face_region(&self) -> BTreeSet<usize> {
        if self.authoring.selection_mode==S::Face {return self.authoring.faces.clone();}
        let Some(mesh)=self.authoring.editing.and_then(|id|self.editor.editable_mesh(id).ok()) else {return BTreeSet::new();};
        let cache=self.mesh_cache(mesh);let g=&cache.graph;
        let selected:BTreeSet<_>=self.edit_vertices().iter().filter_map(|&v|g.canonical.get(v).copied()).collect();
        (0..mesh.face_count()).filter(|&f| {
            let face=mesh.face_vertices(f).unwrap();
            face.iter().all(|&v|selected.contains(&g.canonical[v as usize]))
                && (self.authoring.selection_mode!=S::Edge || (0..face.len()).all(|i| {
                    let a=g.canonical[face[i] as usize];let b=g.canonical[face[(i+1)%face.len()] as usize];
                    self.authoring.edges.contains(&(a.min(b),a.max(b)))
                }))
        }).collect()
    }
    pub(super) fn mesh_cache(&self, mesh: &poseforge_core::Mesh) -> Arc<mesh_pick::MeshCache> {
        let mut slot = self.authoring.cache.borrow_mut();
        if slot.as_ref().is_none_or(|c| !c.matches(mesh)) {
            *slot = Some(Arc::new(mesh_pick::MeshCache::new(mesh)));
        }
        slot.as_ref().unwrap().clone()
    }
    pub(super) fn modeling_action(&mut self, command: A) -> bool {
        match command {
            A::ModifierPreview => self.authoring.preview_modifiers = !self.authoring.preview_modifiers,
            A::Xray => self.authoring.xray = !self.authoring.xray,
            A::PivotOrigin => self.authoring.pivot_origin = !self.authoring.pivot_origin,
            A::Constrain(axis) => {
                self.authoring.constraint =
                    if axis < 3 && self.authoring.constraint != Some(axis as usize) {
                        Some(axis as usize)
                    } else {
                        None
                    }
            }
            A::Invert | A::Grow | A::Shrink | A::Connected => self.modeling_select(command),
            A::RotateDegrees(_) => self.input(Action::Author(command), "0".into()),
            A::ScaleFactor(_) => self.input(Action::Author(command), "1".into()),
            A::InsetAmount => self.input(
                Action::Author(command),
                ui::number_caption((self.authoring.inset * 100.) as f64),
            ),
            A::BevelHeight => self.input(
                Action::Author(command),
                ui::number_caption(self.authoring.bevel_height as f64),
            ),
            A::LoopPosition => self.input(
                Action::Author(command),
                ui::number_caption((self.authoring.loop_position * 100.) as f64),
            ),
            _ => return false,
        }
        true
    }
    pub(super) fn modeling_confirm(&mut self, action: &Action, text: &str) -> Result<bool, String> {
        let Action::Author(command) = *action else {
            return Ok(false);
        };
        match command {
            A::ProportionalRadius => {
                self.authoring.proportional_radius = surfaces::number(text, 0.0001, 100000.)?
            }
            A::InsetAmount => self.authoring.inset = surfaces::number(text, 0., 95.)? / 100.,
            A::BevelHeight => self.authoring.bevel_height = surfaces::number(text, -1000., 1000.)?,
            A::LoopPosition => {
                self.authoring.loop_position = surfaces::number(text, 1., 99.)? / 100.
            }
            A::RotateDegrees(axis) | A::ScaleFactor(axis) => {
                let pivot = if self.authoring.pivot_origin {
                    Vec3::ZERO
                } else {
                    self.edit_center().ok_or("Выберите компоненты")?
                };
                let matrix = if matches!(command, A::RotateDegrees(_)) {
                    let angle = surfaces::number(text, -3600., 3600.)?.to_radians();
                    Mat4::from_axis_angle([Vec3::X, Vec3::Y, Vec3::Z][axis.min(2) as usize], angle)
                } else {
                    let factor = surfaces::number(text, 0.001, 1000.)?;
                    let mut scale = Vec3::splat(factor);
                    if axis < 3 {
                        scale = Vec3::ONE;
                        scale[axis as usize] = factor;
                    }
                    Mat4::from_scale(scale)
                };
                self.editor
                    .edit_mesh(
                        self.authoring.editing.ok_or("Режим сетки закрыт")?,
                        self.component_transform(
                            Mat4::from_translation(pivot) * matrix * Mat4::from_translation(-pivot),
                        )?,
                    )
                    .map_err(|e| e.to_string())?;
                self.invalidate_geometry();
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    fn modeling_select(&mut self, command: A) {
        let Some(mesh) = self
            .authoring
            .editing
            .and_then(|id| self.editor.editable_mesh(id).ok())
        else {
            return;
        };
        let cache = self.mesh_cache(mesh);
        let g = &cache.graph;
        if self.authoring.selection_mode == S::Edge {
            // Visit endpoints once. A high-valence vertex must not allocate a
            // quadratic list of every pair of incident edges.
            let selected = &self.authoring.edges;
            self.authoring.edges = match command {
                A::Invert => g.edges.difference(selected).copied().collect(),
                A::Grow => {
                    let endpoints: BTreeSet<_> =
                        selected.iter().flat_map(|&(a, b)| [a, b]).collect();
                    g.edges
                        .iter()
                        .copied()
                        .filter(|(a, b)| endpoints.contains(a) || endpoints.contains(b))
                        .collect()
                }
                A::Shrink => {
                    let boundary: BTreeSet<_> = g
                        .edges
                        .difference(selected)
                        .flat_map(|&(a, b)| [a, b])
                        .collect();
                    selected
                        .iter()
                        .copied()
                        .filter(|(a, b)| !boundary.contains(a) && !boundary.contains(b))
                        .collect()
                }
                A::Connected => {
                    let mut visited: BTreeSet<_> =
                        selected.iter().flat_map(|&(a, b)| [a, b]).collect();
                    let mut queue: Vec<_> = visited.iter().copied().collect();
                    while let Some(v) = queue.pop() {
                        for &neighbor in &g.vertex_neighbors[v] {
                            if visited.insert(neighbor) {
                                queue.push(neighbor);
                            }
                        }
                    }
                    g.edges
                        .iter()
                        .copied()
                        .filter(|(a, _)| visited.contains(a))
                        .collect()
                }
                _ => return,
            };
            return;
        }
        // Work on canonical components; UV/normal copies are selected together.
        let (all, selected, neighbors): (BTreeSet<usize>, BTreeSet<usize>, Vec<BTreeSet<usize>>) =
            match self.authoring.selection_mode {
                S::Face => (
                    (0..mesh.face_count()).collect(),
                    self.authoring.faces.clone(),
                    g.face_neighbors.clone(),
                ),
                S::Vertex => (
                    g.canonical.iter().copied().collect(),
                    self.authoring
                        .vertices
                        .iter()
                        .map(|&i| g.canonical[i])
                        .collect(),
                    g.vertex_neighbors.clone(),
                ),
                S::Edge => unreachable!(),
            };
        let mut next = selected.clone();
        match command {
            A::Invert => next = all.difference(&selected).copied().collect(),
            A::Grow => {
                for &i in &selected {
                    next.extend(neighbors[i].iter().copied());
                }
            }
            A::Shrink => next.retain(|&i| neighbors[i].iter().all(|n| selected.contains(n))),
            A::Connected => {
                let mut queue: Vec<_> = selected.iter().copied().collect();
                while let Some(i) = queue.pop() {
                    for &n in &neighbors[i] {
                        if next.insert(n) {
                            queue.push(n);
                        }
                    }
                }
            }
            _ => {}
        }
        match self.authoring.selection_mode {
            S::Face => self.authoring.faces = next,
            S::Vertex => {
                self.authoring.vertices = g
                    .canonical
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| next.contains(c).then_some(i))
                    .collect()
            }
            S::Edge => {
                self.authoring.edges = g
                    .edges
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &e)| next.contains(&i).then_some(e))
                    .collect()
            }
        }
    }
    pub(super) fn draw_modeling_transform_properties(
        &self, u: &mut Ui, x: f32, mut y: f32, w: f32, tool: ui::ModelingTool,
    ) {
        let p = u.palette();
        let axis_width = (w - 12.) / 4.;
        for (i, label) in ["X", "Y", "Z", "Все"].into_iter().enumerate() {
            u.quiet_button(
                [x + i as f32 * (axis_width + 4.), y, axis_width, 28.], label,
                Action::Author(A::Constrain(i as u8)),
                self.authoring.constraint == if i < 3 { Some(i) } else { None },
            );
        }
        y += 34.;
        if tool != ui::ModelingTool::Move {
            u.checkbox([x, y, w, 26.], "Центр: начало объекта", Action::Author(A::PivotOrigin), self.authoring.pivot_origin);
            y += 32.;
        }
        if tool == ui::ModelingTool::Move {
            let center = self.edit_center();
            for (axis, label) in ["Центр X", "Центр Y", "Центр Z"].into_iter().enumerate() {
                u.fitted_label([x, y + 5., w * 0.48, 20.], label, 11., p.muted);
                let r = [x + w * 0.5, y, w * 0.5, 28.];
                if let Some(center) = center {
                    u.field(r, &ui::number_caption(center[axis] as f64), Action::Author(A::Coordinate(axis as u8)));
                } else { u.disabled_button(r, "—"); }
                y += 34.;
            }
        } else {
            let axis = self.authoring.constraint.unwrap_or(self.axis) as u8;
            let (label, value, command) = if tool == ui::ModelingTool::Rotate {
                (format!("Повернуть {}, °", ["X", "Y", "Z"][axis as usize]), "0", A::RotateDegrees(axis))
            } else {
                ("Масштаб ×".into(), "1", A::ScaleFactor(self.authoring.constraint.map_or(3, |i| i as u8)))
            };
            u.fitted_label([x, y + 5., w * 0.55, 20.], &label, 11., p.muted);
            u.field([x + w * 0.58, y, w * 0.42, 28.], value, Action::Author(command));
            y += 34.;
        }
        u.checkbox([x, y, w, 26.], "Мягкое выделение · O", Action::Author(A::Proportional), self.authoring.proportional);
        y += 32.;
        if self.authoring.proportional {
            u.fitted_label([x, y + 5., w * 0.55, 20.], "Радиус, м", 11., p.muted);
            u.field([x + w * 0.58, y, w * 0.42, 28.], &ui::number_caption(self.authoring.proportional_radius as f64), Action::Author(A::ProportionalRadius));
            y += 34.;
        }
        u.checkbox([x, y, w, 26.], "Сквозное выделение · X-Ray", Action::Author(A::Xray), self.authoring.xray);
        u.fitted_label([x, y + 34., w, 18.], "Потяните выделение · Esc — отменить", 10., p.muted);
    }
}
