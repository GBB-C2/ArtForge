//! Normal tools and component alignment share document history and selection.
use super::*;
use crate::ui::{NormalAction as N, ShapeAction as S};
use poseforge_core::{MeshEdit, NormalEdit, ShapeEdit};
pub(super) struct MeshToolsState {
    angle: f32,
    weighted: bool,
    show: bool,
    display: u8,
    xray: bool,
    length: f32,
    rotation: f32,
    strength: f32,
    iterations: u32,
    pin: bool,
    shape_axis: usize,
    shape_strength: f32,
    inflate: f32,
    twist: f32,
}
impl Default for MeshToolsState {
    fn default() -> Self {
        Self {
            angle: 60.,
            weighted: true,
            show: false,
            display: 0,
            xray: false,
            length: 0.12,
            rotation: 15.,
            strength: 0.4,
            iterations: 4,
            pin: true,
            shape_axis: 1,
            shape_strength: 0.5,
            inflate: 0.1,
            twist: 45.,
        }
    }
}
impl MeshToolsState {
    pub(super) fn default_value(field: u8) -> Option<f64> {
        let s = Self::default();
        [s.angle, s.length, s.rotation, s.strength, s.iterations as f32, s.shape_strength, s.inflate, s.twist]
            .get(field as usize).map(|v| *v as f64)
    }
}
impl App {
    pub(super) fn normals_available(&self) -> bool {
        self.active_light_index().is_none()
            && self.editor.selected.is_some_and(|id| {
                self.editor
                    .project
                    .node(id)
                    .is_some_and(|n| n.mesh.is_some())
            })
    }
    fn normal_scope(&self, id: u64) -> BTreeSet<usize> {
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            return BTreeSet::new();
        };
        if self.authoring.editing == Some(id) {
            if self.authoring.selection_mode == ui::MeshSelection::Face
                && !self.authoring.faces.is_empty()
            {
                return self.authoring.faces.clone();
            }
            let vertices = self.edit_vertices();
            if !vertices.is_empty() {
                let keys: BTreeSet<_> = vertices
                    .iter()
                    .map(|i| poseforge_core::position_key(mesh.vertices[*i].position))
                    .collect();
                return (0..mesh.face_count())
                    .filter(|f| {
                        mesh.face_vertices(*f).unwrap().iter().any(|i| {
                            keys.contains(&poseforge_core::position_key(
                                mesh.vertices[*i as usize].position,
                            ))
                        })
                    })
                    .collect();
            }
        }
        (0..mesh.face_count()).collect()
    }
    fn mesh_tool_value(&self, field: u8) -> f32 {
        match field {
            0 => self.mesh_tools.angle,
            1 => self.mesh_tools.length,
            2 => self.mesh_tools.rotation,
            3 => self.mesh_tools.strength,
            5 => self.mesh_tools.shape_strength,
            6 => self.mesh_tools.inflate,
            7 => self.mesh_tools.twist,
            _ => self.mesh_tools.iterations as f32,
        }
    }
    pub(super) fn mesh_tools_confirm(&mut self, a: &Action, text: &str) -> Result<bool, String> {
        let Action::Normal(N::Field(field)) = *a else {
            return Ok(false);
        };
        match field {
            0 => self.mesh_tools.angle = surfaces::number(text, 0., 180.)?,
            1 => self.mesh_tools.length = surfaces::number(text, 0.001, 100.)?,
            2 => self.mesh_tools.rotation = surfaces::number(text, -180., 180.)?,
            3 => self.mesh_tools.strength = surfaces::number(text, 0., 1.)?,
            5 => self.mesh_tools.shape_strength = surfaces::number(text,0.,1.)?,
            6 => self.mesh_tools.inflate = surfaces::number(text,-10000.,10000.)?,
            7 => self.mesh_tools.twist = surfaces::number(text,-3600.,3600.)?,
            4 => {
                let v = surfaces::number(text, 1., 32.)?;
                if v.fract() != 0. {
                    return Err("Число проходов должно быть целым".into());
                }
                self.mesh_tools.iterations = v as u32;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub(super) fn normals_action(&mut self, a: N) {
        if self.job.is_some() || self.scene_write_pending() || self.editor.playing {
            return;
        }
        match a {
            N::Angle(angle) => {
                self.mesh_tools.angle = angle.min(180) as f32;
                return;
            }
            N::Display(mode) => {
                self.mesh_tools.display = mode.min(1);
                self.mesh_tools.show = true;
                return;
            }
            N::XRay => {
                self.mesh_tools.xray = !self.mesh_tools.xray;
                return;
            }
            N::AutoLength => {
                if let Some(id) = self.editor.selected {
                    if let (Ok(mesh), Ok(eval)) =
                        (self.editor.editable_mesh(id), self.evaluated_interactive())
                    {
                        if let Some(model) = eval.global.get(&id) {
                            let mut min = Vec3::splat(f32::INFINITY);
                            let mut max = Vec3::splat(f32::NEG_INFINITY);
                            for v in &mesh.vertices {
                                let p = model.transform_point3(Vec3::from_array(v.position));
                                min = min.min(p);
                                max = max.max(p);
                            }
                            let length = (max - min).length() * 0.04;
                            if length.is_finite() {
                                self.mesh_tools.length = length.clamp(0.001, 100.);
                            }
                        }
                    }
                }
                return;
            }
            N::Field(i) => {
                self.input(
                    Action::Normal(a),
                    ui::number_caption(self.mesh_tool_value(i) as f64),
                );
                return;
            }
            N::Show => {
                self.mesh_tools.show = !self.mesh_tools.show;
                return;
            }
            N::Weighted => {
                self.mesh_tools.weighted = !self.mesh_tools.weighted;
                return;
            }
            _ => {}
        }
        self.cancel_drag();
        self.cancel_modeling_live();
        self.authoring_prune();
        let Some(id) = self.editor.selected.filter(|id| self.node_selectable(*id)) else {
            return;
        };
        let faces = self.normal_scope(id);
        let edit = match a {
            N::Recalculate => NormalEdit::Recalculate {
                angle: self.mesh_tools.angle,
                weighted: self.mesh_tools.weighted,
            },
            N::Flat => NormalEdit::Recalculate {
                angle: 0.,
                weighted: false,
            },
            N::Smooth => NormalEdit::Recalculate {
                angle: 180.,
                weighted: self.mesh_tools.weighted,
            },
            N::Flip => NormalEdit::FlipFaces,
            N::Direction(i) => NormalEdit::Direction(
                [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z][usize::from(i).min(5)],
            ),
            N::Rotate(i) => NormalEdit::Rotate(Quat::from_axis_angle(
                [Vec3::X, Vec3::Y, Vec3::Z][usize::from(i).min(2)],
                self.mesh_tools.rotation.to_radians(),
            )),
            _ => return,
        };
        self.run_mesh_tool(
            id,
            MeshEdit::Normals {
                faces: faces.clone(),
                edit,
            },
            Some(faces),
        );
    }

    pub(super) fn shape_action(&mut self, a: S) {
        if self.job.is_some() || self.scene_write_pending() || self.editor.playing {
            return;
        }
        if let S::Axis(axis) = a { self.mesh_tools.shape_axis = (axis as usize).min(2); return; }
        if a == S::Pin {
            self.mesh_tools.pin = !self.mesh_tools.pin;
            return;
        }
        self.cancel_drag();
        self.cancel_modeling_live();
        self.authoring_prune();
        let Some(id) = self
            .authoring
            .editing
            .filter(|id| self.node_selectable(*id))
        else {
            return;
        };
        let edit = match a {
            S::Sphere => ShapeEdit::Sphere { strength:self.mesh_tools.shape_strength },
            S::Circle => ShapeEdit::Circle { axis:self.mesh_tools.shape_axis,strength:self.mesh_tools.shape_strength },
            S::Inflate => ShapeEdit::Inflate { distance:self.mesh_tools.inflate },
            S::Twist => ShapeEdit::Twist { axis:self.mesh_tools.shape_axis,radians:self.mesh_tools.twist.to_radians() },
            S::Flatten(i) => ShapeEdit::Flatten(i as usize),
            S::Straighten => ShapeEdit::Straighten,
            S::Space => ShapeEdit::Space,
            S::Relax => ShapeEdit::Relax {
                strength: self.mesh_tools.strength,
                iterations: self.mesh_tools.iterations,
                pin_boundary: self.mesh_tools.pin,
            },
            _ => return,
        };
        self.run_mesh_tool(
            id,
            MeshEdit::Shape {
                vertices: self.edit_vertices(),
                edges: if self.authoring.selection_mode == ui::MeshSelection::Edge {
                    self.authoring.edges.clone()
                } else {
                    BTreeSet::new()
                },
                edit,
            },
            None,
        );
    }
    fn run_mesh_tool(&mut self, id: u64, edit: MeshEdit, faces: Option<BTreeSet<usize>>) {
        if self.editor.has_staged() {
            self.status = "Сначала поставьте ключ или отмените временную позу".into();
            return;
        }
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            self.status = "Нужен редактируемый статический меш".into();
            return;
        };
        if mesh.vertices.len() > 12000 || mesh.indices.len() > 48000 {
            let before = self.editor.project.clone();
            let (tx, rx) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let worker = cancel.clone();
            std::thread::spawn(move || {
                let result = prepare_mesh_tool(&before, id, edit, &worker);
                let message = match result {
                    Ok(after) => {
                        JobMessage::MeshToolReady(Box::new(before), Box::new(after), id, faces)
                    }
                    Err(e) => JobMessage::Error(e),
                };
                let _ = tx.send(message);
            });
            self.job = Some(Job {
                receiver: rx,
                cancel,
                label: "Обработка сетки".into(),
            });
            self.status = "Обработка сетки в фоне · Esc — отменить".into();
        } else {
            let result = self.editor.edit_mesh(id, edit);
            if self.result(result).is_some() {
                self.mesh_tool_finished(id, faces);
            }
        }
    }
    pub(super) fn mesh_tool_finished(&mut self, id: u64, faces: Option<BTreeSet<usize>>) {
        if let Some(faces) = faces {
            if self.authoring.editing == Some(id) {
                self.authoring.selection_mode = ui::MeshSelection::Face;
                self.authoring.faces = faces;
                self.authoring.vertices.clear();
                self.authoring.edges.clear();
            }
        }
        self.authoring.diagnostics = None;
        self.invalidate_geometry();
        self.update_dirty();
        self.status = "Обработка сетки завершена · Undo отменяет операцию".into();
    }

    pub(super) fn normals_panel_bottom(&self) -> f32 {
        if self
            .editor
            .selected
            .is_some_and(|id| self.editor.editable_mesh(id).is_ok())
        {
            942.
        } else {
            234.
        }
    }
    pub(super) fn draw_normals_panel(&self, u: &mut Ui, rx: f32, rw: f32) {
        let x = rx + 16.;
        let w = rw - 32.;
        let p = u.palette();
        let Some(id) = self.editor.selected else {
            u.fitted_label([x, 174., w, 24.], "Выберите меш", 12., p.muted);
            return;
        };
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            u.fitted_label(
                [x, 174., w, 24.],
                "Нужен редактируемый статический меш",
                11.,
                p.muted,
            );
            return;
        };
        let scope = self.normal_scope(id);
        let enabled = self.node_selectable(id)
            && self.job.is_none()
            && !self.editor.playing
            && !self.editor.has_staged();
        let command = |u: &mut Ui, r, label: &str, action, primary| {
            if enabled {
                u.button(r, label, Action::Normal(action), primary);
            } else {
                u.disabled_button(r, label);
            }
        };
        let half = (w - 8.) * 0.5;
        let third = (w - 12.) / 3.;
        u.fitted_label(
            [x, 174., w, 20.],
            &if scope.len() == mesh.face_count() {
                format!("Весь меш · {} граней", scope.len())
            } else {
                format!("Выделено: {} из {} граней", scope.len(), mesh.face_count())
            },
            12.,
            p.text,
        );
        u.fitted_label(
            [x, 200., w, 20.],
            "Без выделения — весь объект",
            11.,
            p.muted,
        );
        u.number_field(
            [x, 230., w, 44.],
            "Угол сглаживания, °",
            self.mesh_tools.angle as f64,
            Action::Normal(N::Field(0)),
            SliderRange::new(0., 180., 1.),
        );
        for (i, angle) in [30, 60, 90].into_iter().enumerate() {
            u.quiet_button(
                [x + i as f32 * (third + 6.), 286., third, 28.],
                &format!("{angle}°"),
                Action::Normal(N::Angle(angle)),
                self.mesh_tools.angle == angle as f32,
            );
        }
        u.checkbox(
            [x, 326., w, 28.],
            "Учитывать площадь граней",
            Action::Normal(N::Weighted),
            self.mesh_tools.weighted,
        );
        command(u, [x, 364., w, 32.], "Пересчитать", N::Recalculate, true);
        command(u, [x, 408., half, 30.], "Жёсткие", N::Flat, false);
        command(
            u,
            [x + half + 8., 408., half, 30.],
            "Гладкие",
            N::Smooth,
            false,
        );

        u.checkbox(
            [x, 460., w, 28.],
            "Направления в просмотре",
            Action::Normal(N::Show),
            self.mesh_tools.show,
        );
        for (i, label) in ["Вершины", "Грани"].iter().enumerate() {
            u.quiet_button(
                [x + i as f32 * (half + 8.), 500., half, 30.],
                label,
                Action::Normal(N::Display(i as u8)),
                self.mesh_tools.display == i as u8,
            );
        }
        u.number_field(
            [x, 540., w, 44.],
            "Длина линий, м",
            self.mesh_tools.length as f64,
            Action::Normal(N::Field(1)),
            SliderRange::new(0.001, self.mesh_tools.length.max(1.) as f64, 0.01),
        );
        u.quiet_button(
            [x, 594., w, 28.],
            "Подобрать длину",
            Action::Normal(N::AutoLength),
            false,
        );
        u.checkbox(
            [x, 634., w, 28.],
            "Сквозь поверхность",
            Action::Normal(N::XRay),
            self.mesh_tools.xray,
        );

        command(u, [x, 686., w, 30.], "Развернуть грани", N::Flip, false);
        u.fitted_label([x, 728., w, 20.], "Направить · локальные оси", 11., p.muted);
        for (i, (label, direction)) in [
            ("+X", 0),
            ("+Y", 2),
            ("+Z", 4),
            ("−X", 1),
            ("−Y", 3),
            ("−Z", 5),
        ]
        .iter()
        .enumerate()
        {
            let r = [
                x + (i % 3) as f32 * (third + 6.),
                758. + (i / 3) as f32 * 34.,
                third,
                28.,
            ];
            command(u, r, label, N::Direction(*direction), false);
        }
        u.number_field(
            [x, 836., w, 44.],
            "Угол поворота, °",
            self.mesh_tools.rotation as f64,
            Action::Normal(N::Field(2)),
            SliderRange::new(-180., 180., 1.),
        );
        for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
            let r = [x + i as f32 * (third + 6.), 894., third, 30.];
            if enabled {
                u.quiet_icon_button(
                    r,
                    label,
                    Action::Normal(N::Rotate(i as u8)),
                    false,
                    Icon::Rotate,
                );
            } else {
                u.disabled_button(r, label);
            }
        }
    }
    pub(super) fn draw_deform_properties(&self,u:&mut Ui,x:f32,mut y:f32,w:f32,tool:ui::ModelingTool) {
        use ui::ModelingTool as M;
        if matches!(tool,M::Circle|M::Twist) {
            for (axis,label) in ["X","Y","Z"].into_iter().enumerate() {
                u.quiet_button([x+axis as f32*(w+6.)/3.,y,(w-12.)/3.,28.],label,Action::Shape(S::Axis(axis as u8)),self.mesh_tools.shape_axis==axis);
            } y+=36.;
        }
        let (label,field,range,action) = match tool {
            M::Sphere => ("Сила сферизации",5,SliderRange::new(0.,1.,0.01),S::Sphere),
            M::Circle => ("Сила окружности",5,SliderRange::new(0.,1.,0.01),S::Circle),
            M::Inflate => ("По нормалям, м",6,SliderRange::new(-10.,10.,0.01),S::Inflate),
            M::Twist => ("Угол скручивания, °",7,SliderRange::new(-360.,360.,1.),S::Twist),
            _=>return,
        };
        u.number_field([x,y,w,44.],label,self.mesh_tool_value(field) as f64,Action::Normal(N::Field(field)),range); y+=52.;
        let minimum = if matches!(tool,M::Sphere|M::Circle) {3}else{2};
        if self.edit_vertices().len()>=minimum {
            u.button([x,y,w,30.],"Применить",Action::Shape(action),false);
        } else { u.disabled_button([x,y,w,30.],"Выберите компоненты"); }
        u.fitted_label([x,y+38.,w,18.],"Локальные координаты · Ctrl+Z — отменить",10.,u.palette().muted);
    }
    pub(super) fn draw_relax_properties(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let enabled = self.edit_vertices().len() >= 2;
        u.number_field(
            [x, y, w, 44.],
            "Сила релакса",
            self.mesh_tools.strength as f64,
            Action::Normal(N::Field(3)),
            SliderRange::new(0., 1., 0.01),
        );
        y += 50.;
        u.number_field(
            [x, y, w, 44.],
            "Проходы",
            self.mesh_tools.iterations as f64,
            Action::Normal(N::Field(4)),
            SliderRange::new(1., 32., 1.),
        );
        y += 50.;
        u.checkbox(
            [x, y, w, 28.],
            "Закрепить границы",
            Action::Shape(S::Pin),
            self.mesh_tools.pin,
        );
        y += 34.;
        if enabled {
            u.quiet_button(
                [x, y, w, 30.],
                "Релакс сетки",
                Action::Shape(S::Relax),
                false,
            );
        } else {
            u.disabled_button([x, y, w, 30.], "Релакс сетки");
        }
    }
    pub(super) fn draw_normal_directions(&self, u: &mut Ui) {
        if self.surface.tab != 10 || !self.mesh_tools.show || self.has_modal() {
            return;
        }
        let Some(id) = self.editor.selected else {
            return;
        };
        let Ok(mesh) = self.editor.editable_mesh(id) else {
            return;
        };
        let Ok(eval) = self.evaluated_interactive() else {
            return;
        };
        let model = eval.global[&id];
        let normal = model.inverse().transpose();
        let scope = self.normal_scope(id);
        let mut view = self.view();
        // View projects to physical pixels; Ui applies DPI once when drawing.
        view.viewport = view.viewport.map(|v| v / self.scale);
        let vp = self.viewport();
        let projection = view.view_projection();
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        let mut draw = |a: Vec3, direction: Vec3| {
            let direction = direction.normalize_or_zero();
            if !a.is_finite() || !direction.is_finite() || direction.length_squared() < 0.5 {
                return;
            }
            let b = a + direction * self.mesh_tools.length;
            if let (Some(pa), Some(pb)) = (view.project(a), view.project(b)) {
                let za = projection * a.extend(1.);
                let zb = projection * b.extend(1.);
                let depth = (!self.mesh_tools.xray).then_some([za.z / za.w, zb.z / zb.w]);
                let color = u.palette().accent;
                u.depth_mesh_segment(pa, pb, 1.2, color, depth);
                let delta = glam::Vec2::from_array(pb) - glam::Vec2::from_array(pa);
                if delta.length_squared() > 36. {
                    let d = delta.normalize();
                    let tip = glam::Vec2::from_array(pb);
                    let side = glam::Vec2::new(-d.y, d.x) * 2.5;
                    for end in [tip - d * 5. + side, tip - d * 5. - side] {
                        u.depth_mesh_segment(
                            pb,
                            end.to_array(),
                            1.2,
                            color,
                            depth.map(|z| [z[1]; 2]),
                        );
                    }
                }
            }
        };
        if self.mesh_tools.display == 0 {
            let vertices: BTreeSet<_> = scope
                .iter()
                .flat_map(|f| mesh.face_vertices(*f).unwrap().iter().copied())
                .collect();
            for &index in vertices
                .iter()
                .step_by(vertices.len().div_ceil(2048).max(1))
            {
                let v = &mesh.vertices[index as usize];
                draw(
                    model.transform_point3(Vec3::from_array(v.position)),
                    normal.transform_vector3(Vec3::from_array(v.normal)),
                );
            }
        } else {
            for &face in scope.iter().step_by(scope.len().div_ceil(2048).max(1)) {
                let indices = mesh.face_vertices(face).unwrap();
                let position = |i: u32| {
                    model.transform_point3(Vec3::from_array(mesh.vertices[i as usize].position))
                };
                let origin = position(indices[0]);
                let mut center = Vec3::ZERO;
                let mut direction = Vec3::ZERO;
                for (j, &i) in indices.iter().enumerate() {
                    let a = position(i);
                    let b = position(indices[(j + 1) % indices.len()]);
                    center += a - origin;
                    direction += (a - origin).cross(b - origin);
                }
                draw(origin + center / indices.len() as f32, direction);
            }
        }
        clip_scrolled(u, start, vp, 0.);
    }
}

fn prepare_mesh_tool(
    project: &Project,
    id: u64,
    edit: MeshEdit,
    cancel: &AtomicBool,
) -> Result<Project, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Операция отменена".into());
    }
    let mut editor = Editor::new(project.clone());
    editor.edit_mesh(id, edit).map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Операция отменена".into());
    }
    Ok(editor.project)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut a = App::new(None);
        a.preferences = Preferences::default();
        a.width = 1280.;
        a.height = 900.;
        a.editor = Editor::new(Project::default());
        a.editor.mode = EditorMode::Scene;
        a.create_primitive(0);
        a.primitive_panel = None;
        a
    }
    #[test]
    fn normal_tab_keeps_component_controls_and_scopes_edits_to_faces() {
        let mut a = app();
        let node = a.editor.selected.unwrap();
        a.action(Action::SurfaceTab(10));
        a.action(Action::Author(ui::AuthorAction::SelectMode(
            ui::MeshSelection::Face,
        )));
        assert_eq!(a.surface.tab, 10);
        a.authoring.faces = BTreeSet::from([0]);
        let before = a.editor.project.clone();
        a.action(Action::Normal(N::Direction(2)));
        assert_eq!(a.authoring.faces, BTreeSet::from([0]));
        let old = before.meshes.first().unwrap();
        let m = a.editor.editable_mesh(node).unwrap();
        for f in 1..6 {
            for (&i, &j) in old
                .face_vertices(f)
                .unwrap()
                .iter()
                .zip(m.face_vertices(f).unwrap())
            {
                assert_eq!(old.vertices[i as usize], m.vertices[j as usize]);
            }
        }
        a.action(Action::Undo);
        assert_eq!(a.editor.project, before);
        a.action(Action::CreateCamera);
        assert!(!a
            .build_ui()
            .hits
            .iter()
            .any(|h| h.action == Action::SurfaceTab(10)));
    }
    #[test]
    fn normal_parameter_slider_cancel_restores_settings_without_document_history() {
        for theme in [ui::Theme::Dark, ui::Theme::Light] {
            for scale in [1., 2.] {
                let mut a = app();
                a.preferences.theme = theme;
                a.scale = scale;
                a.action(Action::SurfaceTab(10));
                let before = a.editor.project.clone();
                let history = a.editor.undo_len();
                let original = a.mesh_tools.angle;
                a.ui = a.build_ui();
                let h=a.ui.hits.iter().find(|h|matches!(&h.action,Action::Slide(field,_,_) if **field==Action::Normal(N::Field(0)))).unwrap();
                a.mouse = [h.rect[0] + h.rect[2] * 0.8, h.rect[1] + h.rect[3] * 0.5];
                a.mouse_down(MouseButton::Left);
                a.mouse_move([a.mouse[0] - 30., a.mouse[1]]);
                assert_ne!(a.mesh_tools.angle, original);
                a.cancel_drag();
                assert_eq!(a.mesh_tools.angle, original);
                assert_eq!(a.editor.project, before);
                assert_eq!(a.editor.undo_len(), history);
                assert!(a
                    .mesh_tools_confirm(&Action::Normal(N::Field(4)), "1.5")
                    .is_err());
            }
        }
    }
    #[test]
    fn dense_mesh_jobs_are_cancelable_stale_safe_and_commit_once() {
        let mut a = app();
        let node = a.editor.selected.unwrap();
        let id = a.editor.project.node(node).unwrap().mesh.unwrap();
        let mut settings =
            poseforge_core::PrimitiveSettings::new(poseforge_core::PrimitiveKind::Cube);
        settings.segments = [48; 3];
        let replacement = settings.mesh(id, None).unwrap();
        a.editor.project.meshes[0] = replacement;
        let before = a.editor.project.clone();
        let history = a.editor.undo_len();
        for cancel in [true, false] {
            a.normals_action(N::Smooth);
            assert!(a.job.is_some());
            assert!(a.scene_write_pending());
            assert_eq!(a.editor.project, before);
            if cancel {
                a.action(Action::Cancel);
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            while a.job.is_some() && Instant::now() < deadline {
                a.poll_jobs();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(a.job.is_none());
            if cancel {
                assert_eq!(a.editor.project, before);
                assert_eq!(a.editor.undo_len(), history);
            } else {
                assert_ne!(a.editor.project, before);
                assert_eq!(a.editor.undo_len(), history + 1);
                a.action(Action::Undo);
                assert_eq!(a.editor.project, before);
            }
        }
        let faces = (0..6).collect();
        assert!(prepare_mesh_tool(
            &before,
            node,
            MeshEdit::Normals {
                faces,
                edit: NormalEdit::FlipFaces
            },
            &AtomicBool::new(true)
        )
        .is_err());
        let mut stale = before.clone();
        stale.title = "stale".into();
        let (tx, rx) = mpsc::channel();
        a.job = Some(Job {
            receiver: rx,
            cancel: Arc::new(AtomicBool::new(false)),
            label: "Обработка сетки".into(),
        });
        tx.send(JobMessage::MeshToolReady(
            Box::new(stale),
            Box::default(),
            node,
            None,
        ))
        .unwrap();
        a.poll_jobs();
        assert_eq!(a.editor.project, before);
    }
    #[test]
    fn normal_display_presets_depth_and_auto_length_leave_document_untouched() {
        let mut a = app();
        let id = a.editor.selected.unwrap();
        a.action(Action::SurfaceTab(10));
        a.action(Action::Author(ui::AuthorAction::SelectMode(
            ui::MeshSelection::Face,
        )));
        a.authoring.faces = BTreeSet::from([0]);
        a.action(Action::Normal(N::AutoLength));
        let length = a.mesh_tools.length;
        a.editor.project.node_mut(id).unwrap().transform.scale = Vec3::splat(3.);
        a.invalidate_geometry();
        a.frame_all();
        let before = a.editor.project.clone();
        let history = a.editor.undo_len();
        a.action(Action::Normal(N::Angle(30)));
        assert_eq!(a.mesh_tools.angle, 30.);
        a.action(Action::Normal(N::AutoLength));
        assert!((a.mesh_tools.length - length * 3.).abs() < 1e-5);
        a.action(Action::Normal(N::Display(1)));
        let mut faces = Ui::with_theme(1., ui::Theme::Dark, [0.; 2]);
        a.draw_normal_directions(&mut faces);
        assert!(!faces.rects.is_empty());
        assert!(
            faces.rects.len() <= 3,
            "only the selected face and its arrowhead"
        );
        assert!(faces.rects.iter().all(|r| r.depth.is_some()));
        a.action(Action::Normal(N::Display(0)));
        let mut vertices = Ui::with_theme(1., ui::Theme::Dark, [0.; 2]);
        a.draw_normal_directions(&mut vertices);
        assert!(vertices.rects.len() > faces.rects.len());
        a.action(Action::Normal(N::XRay));
        let mut xray = Ui::with_theme(1., ui::Theme::Dark, [0.; 2]);
        a.draw_normal_directions(&mut xray);
        assert!(!xray.rects.is_empty());
        assert!(xray.rects.iter().all(|r| r.depth.is_none()));
        assert_eq!(a.editor.project, before);
        assert_eq!(a.editor.undo_len(), history);
    }
    #[test]
    fn normal_arrows_match_the_same_model_points_at_both_display_scales() {
        let mut a = app();
        a.frame_all();
        a.action(Action::SurfaceTab(10));
        for mode in [0, 1] {
            a.action(Action::Normal(N::Display(mode)));
            a.scale = 1.;
            let mut one = Ui::with_theme(1., ui::Theme::Dark, [0.; 2]);
            a.draw_normal_directions(&mut one);
            a.scale = 2.;
            let mut two = Ui::with_theme(2., ui::Theme::Light, [0.; 2]);
            a.draw_normal_directions(&mut two);
            assert!(!one.rects.is_empty());
            assert_eq!(one.rects.len(), two.rects.len());
            for (a, b) in one.rects.iter().zip(&two.rects) {
                for (x, y) in a.segment.unwrap().iter().zip(b.segment.unwrap()) {
                    assert!((x - y / 2.).abs() < 1e-3);
                }
                assert_eq!(a.depth, b.depth);
            }
        }
    }
}
