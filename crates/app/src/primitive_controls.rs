//! Nonmodal creation parameters, stored in the selected mesh or curve and edited with
//! the same numeric fields, slider capture and document transactions as properties.
use super::*;
use crate::ui::PrimitiveAction as P;
use poseforge_core::{Material, Node, PhysicsSettings, PrimitiveKind as K, PrimitiveSettings};

pub(super) fn primitive_icon(kind:K)->Icon {match kind {
    K::Cube=>Icon::Object,K::Plane=>Icon::Plane,K::Sphere=>Icon::Sphere,K::Cloth=>Icon::Cloth,K::Fox=>Icon::Fox,
    K::Bezier=>Icon::Curve,K::CurveCircle=>Icon::CurveCircle,K::Cylinder=>Icon::Cylinder,K::Cone=>Icon::Cone,K::Torus=>Icon::Torus,K::Capsule=>Icon::Capsule,K::Pyramid=>Icon::Pyramid,
}}
impl App {
    fn creation_panel_target(&self) -> Option<u64> {
        if self.editor.mode != EditorMode::Scene { return None; }
        let id = self.selected_curve().map(|(id, _)| id)
            .or_else(|| self.selected_primitive().map(|(id, _)| id))?;
        self.node_selectable(id).then_some(id)
    }
    pub(super) fn selected_primitive(&self) -> Option<(u64, PrimitiveSettings)> {
        let id = self.editor.selected?;
        if self.editor.mode != EditorMode::Scene || !self.node_selectable(id) {
            return None;
        }
        let mesh = self.editor.editable_mesh(id).ok()?;
        Some((id, mesh.topology.as_ref()?.primitive?))
    }
    pub(super) fn create_primitive(&mut self, kind: u8) {
        if kind == 5 || kind == 6 { self.curve_action(ui::CurveAction::Create(kind - 5)); return; }
        let Some(kind)=K::from_code(kind) else{return;};
        if self.job.is_some() || self.scene_write_pending() || self.native_picker.is_some() {return;}
        if self.editor.has_staged() {
            self.status = "Сначала поставьте ключ или отбросьте временную позу.".into();
            return;
        }
        let settings = PrimitiveSettings::new(kind);
        let id = self.editor.project.next_id();
        let Ok(mesh) = settings.mesh(id + 2, Some(id + 1)) else {
            return;
        };
        let mut material = Material {
            id: id + 1,
            name: "Материал меша".into(),
            ..Material::default()
        };
        if matches!(kind, K::Cloth | K::Plane) {
            material.double_sided = true;
        }
        if kind == K::Cloth {
            material.base_color = [0.08, 0.25, 0.6, 1.];
        }
        let mut node = Node::new(id, settings.label());
        node.mesh = Some(mesh.id);
        node.transform.translation = Vec3::ZERO;
        if kind == K::Cloth {
            node.physics = Some(PhysicsSettings::default());
            node.collision_targets.cloth=Some(vec![]);
        }
        let result = self.editor.transact("Создать примитив", |p| {
            p.materials.push(material);
            p.meshes.push(mesh);
            p.nodes.push(node);
        });
        if self.result(result).is_none() {
            return;
        }
        self.action(Action::Select(id));
        self.editor.mode = EditorMode::Scene;
        self.editor.playing = false;
        self.tool = Tool::Move;
        self.authoring = Default::default();
        self.set_properties_tab(0);
        self.preferences.right_panel = true;
        self.primitive_panel = Some(id);
        self.primitive_collapsed = false;
        self.surface.curve_point = 0;
        self.studio.physics_running = false;
        self.studio.physics.reset();
        self.invalidate();
        self.status = "Примитив создан · параметры слева в просмотре · 1/2/3 — компоненты после входа в сетку".into();
    }
    pub(super) fn primitive_panel_rect(&self) -> Option<[f32; 4]> {
        let panel = self.primitive_panel?;
        let id = self.creation_panel_target()?;
        if panel != id || self.authoring.editing.is_some() || self.has_modal() {
            return None;
        }
        let height = if self.primitive_collapsed {
            38.
        } else if self.selected_curve().is_some() {
            self.curve_creation_height().min((self.viewport()[3] - 24.).max(96.))
        } else {
            let (_, settings) = self.selected_primitive()?;
            let fields = match settings.kind {
                K::Cube | K::Bezier => 6, K::Sphere => 2, K::Pyramid=>3, _ => 4,
            };
            180. + fields as f32 * 38. + if settings.kind == K::Bezier { 34. } else { 0. }
        };
        let viewport = self.viewport();
        let right = if self.preferences.right_panel {
            self.right_panel_rect()[0] - 12.
        } else {
            self.width - 12.
        };
        let sidebar = self.left_panel_rect();
        let width = 268_f32.min((right - 24.).max(180.));
        let x = (sidebar[0] + sidebar[2] + 12.).min(right - width).max(12.);
        let [x, y] = self.preferences.creation_panel_position.unwrap_or([
            x, (viewport[1] + viewport[3] - height - 12.).max(76.),
        ]);
        Some([x.clamp(12., (self.width - width - 12.).max(12.)),
            y.clamp(viewport[1] + 12., (viewport[1] + viewport[3] - height - 12.).max(viewport[1] + 12.)),
            width, height])
    }
    pub(super) fn begin_creation_panel_drag(&mut self, action: &Action) -> bool {
        if *action != Action::Primitive(P::Collapse) { return false; }
        if !self.resolve_input() { return true; }
        let Some(rect) = self.primitive_panel_rect() else { return true; };
        self.drag = Some(Drag::CreationPanel {
            start: self.mouse, origin: [rect[0], rect[1]],
            original: self.preferences.creation_panel_position, moved: false,
        });
        self.drag_button = Some(MouseButton::Left);
        true
    }
    pub(super) fn update_creation_panel_drag(&mut self, start: [f32;2], origin: [f32;2], original: Option<[f32;2]>, moved: bool) {
        let moved = moved || (self.mouse[0]-start[0]).hypot(self.mouse[1]-start[1]) > 3.;
        if moved {
            self.preferences.creation_panel_position = Some([
                origin[0] + self.mouse[0]-start[0], origin[1] + self.mouse[1]-start[1],
            ]);
            if let Some(rect) = self.primitive_panel_rect() {
                self.preferences.creation_panel_position = Some([rect[0],rect[1]]);
            }
        }
        self.drag = Some(Drag::CreationPanel { start, origin, original, moved });
    }
    pub(super) fn prune_primitive_panel(&mut self) {
        if self.primitive_panel.is_some_and(|id| {
            self.creation_panel_target().is_none_or(|selected| id != selected)
        }) {
            self.primitive_panel = None;
        }
    }
    fn primitive_field(&self, field: u8) -> Option<(f32, f32, f32, &'static str)> {
        let (_, settings) = self.selected_primitive()?;
        if settings.is_curve() {
            return match field {
                0 if settings.kind == K::CurveCircle => Some((settings.size[0], 0.001, 10000., "Радиус окружности, м")),
                3 | 4 => Some((settings.segments[(field - 3) as usize] as f32, 3., settings.max_segments() as f32,
                    if field == 3 { "Сегменты кривой" } else { "Стороны сечения" })),
                6 => Some((settings.radius, 0.001, 10000., "Толщина · радиус, м")),
                10..=12 if settings.kind == K::Bezier => Some((settings.curve_points[self.surface.curve_point.min(3) as usize][(field - 10) as usize],
                    -10000., 10000., ["Точка X, м", "Точка Y, м", "Точка Z, м"][(field - 10) as usize])),
                _ => None,
            };
        }
        if settings.kind.extended() {
            return match (settings.kind,field) {
                (K::Pyramid,0..=2)=>Some((settings.size[field as usize],0.001,10000.,["Ширина, м","Высота, м","Глубина, м"][field as usize])),
                (K::Torus,0)=>Some((settings.size[0],0.001,settings.radius*0.99,"Радиус трубки, м")),
                (K::Cylinder|K::Cone|K::Capsule,1)=>Some((settings.size[1],if settings.kind==K::Capsule {settings.radius*2.}else{0.001},10000.,"Высота, м")),
                (_,6)=>Some((settings.radius,if settings.kind==K::Torus {settings.size[0]*1.01}else{0.001},if settings.kind==K::Capsule {settings.size[1]*0.5}else{10000.},if settings.kind==K::Torus {"Радиус кольца, м"}else{"Радиус, м"})),
                (_,3)=>Some((settings.segments[0] as f32,3.,64.,"Стороны окружности")),
                (_,4)=>Some((settings.segments[1] as f32,if matches!(settings.kind,K::Capsule|K::Torus) {3.}else{1.},64.,if settings.kind==K::Capsule {"Сегменты скругления"}else if settings.kind==K::Torus {"Стороны трубки"}else{"Сегменты высоты"})),
                _=>None,
            };
        }
        match field {
            0..=2
                if matches!(settings.kind, K::Cube | K::Fox)
                    || (field < 2 && settings.kind != K::Sphere) =>
            {
                Some((
                    settings.size[field as usize],
                    0.001,
                    10000.,
                    ["Размер X, м", "Размер Y, м", "Размер Z, м"][field as usize],
                ))
            }
            3..=5
                if settings.kind == K::Cube
                    || field == 3
                    || (field == 4 && !matches!(settings.kind, K::Sphere | K::Fox)) =>
            {
                Some((
                    settings.segments[(field - 3) as usize] as f32,
                    1.,
                    settings.max_segments() as f32,
                    if settings.kind == K::Fox {
                        "Детализация"
                    } else if settings.kind == K::Sphere {
                        "Сетка на сторону"
                    } else {
                        ["Подразделения X", "Подразделения Y", "Подразделения Z"]
                            [(field - 3) as usize]
                    },
                ))
            }
            6 if settings.kind == K::Sphere => Some((settings.radius, 0.001, 10000., "Радиус, м")),
            _ => None,
        }
    }
    fn set_primitive(&mut self, settings: PrimitiveSettings) -> Result<(), String> {
        let (node, old_settings) = self
            .selected_primitive()
            .ok_or("Примитив больше недоступен")?;
        if self.authoring.editing.is_some() {
            return Err("Завершите редактирование компонентов".into());
        }
        if settings == old_settings {
            return Ok(());
        }
        let old = self.editor.editable_mesh(node).map_err(|e| e.to_string())?;
        let mut mesh = settings
            .mesh(old.id, old.material)
            .map_err(|e| e.to_string())?;
        mesh.name = old.name.clone();
        if self
            .editor
            .project
            .nodes
            .iter()
            .filter(|n| n.mesh == Some(old.id))
            .count()
            > 1
        {
            mesh.id = self.editor.project.next_id();
        }
        self.editor
            .transact("Параметры примитива", |p| {
                p.node_mut(node).unwrap().mesh = Some(mesh.id);
                if let Some(slot) = p.meshes.iter_mut().find(|m| m.id == mesh.id) {
                    *slot = mesh;
                } else {
                    p.meshes.push(mesh);
                }
            })
            .map_err(|e| e.to_string())?;
        self.studio.physics_running = false;
        self.studio.physics.reset();
        self.invalidate_geometry();
        self.update_dirty();
        Ok(())
    }
    pub(super) fn primitive_confirm(
        &mut self,
        action: &Action,
        text: &str,
    ) -> Result<bool, String> {
        let Action::Primitive(P::Field(field)) = *action else {
            return Ok(false);
        };
        let (_, min, max, _) = self
            .primitive_field(field)
            .ok_or("Параметр больше недоступен")?;
        let value = surfaces::number(text, min, max)?;
        if (3..=5).contains(&field) && value.fract() != 0. {
            return Err("Подразделения задаются целым числом".into());
        }
        let (_, mut settings) = self
            .selected_primitive()
            .ok_or("Примитив больше недоступен")?;
        match field {
            0..=2 => settings.size[field as usize] = value,
            3..=5 => settings.segments[(field - 3) as usize] = value as u32,
            6 => settings.radius = value,
            10..=12 if settings.kind == K::Bezier => settings.curve_points[self.surface.curve_point.min(3) as usize][(field - 10) as usize] = value,
            _ => return Ok(false),
        }
        self.set_primitive(settings)?;
        Ok(true)
    }
    fn creation_normal(&self) -> Vec3 {
        if self.selected_curve().is_some() {return Vec3::Z;}
        match self.selected_primitive().map(|(_,s)|s.kind) {Some(K::Cloth|K::Bezier|K::CurveCircle)=>Vec3::Z,_=>Vec3::Y}
    }
    pub(super) fn draw_creation_orientation(&self,u: &mut Ui,x:f32,y:f32,w:f32) {
        let Some(id)=self.creation_panel_target() else{return;};
        let Some(node)=self.editor.project.node(id) else{return;};
        u.fitted_label([x,y+7.,76.,18.],"Ось формы",11.,u.palette().muted);
        let cell=(w-84.-8.)/3.;let normal=node.transform.rotation*self.creation_normal();
        for (i,axis) in [Vec3::X,Vec3::Y,Vec3::Z].into_iter().enumerate() {
            let r=[x+84.+i as f32*(cell+4.),y,cell,30.];
            if self.job.is_some() || self.scene_write_pending() || self.editor.has_staged() {u.disabled_button(r,["X","Y","Z"][i]);}
            else {u.button(r,["X","Y","Z"][i],Action::Primitive(P::Orientation(i as u8)),normal.dot(axis)>0.9999);}
        }
    }
    pub(super) fn primitive_action(&mut self, action: P) {
        if let P::Orientation(axis)=action {
            let Some(id)=self.creation_panel_target() else{return;};
            if axis>2 || self.job.is_some() || self.scene_write_pending() || self.editor.has_staged() || self.editor.playing {return;}
            let rotation=Quat::from_rotation_arc(self.creation_normal(),[Vec3::X,Vec3::Y,Vec3::Z][axis as usize]);
            let r=self.editor.transact("Ориентация создаваемого объекта",|p|p.node_mut(id).unwrap().transform.rotation=rotation);
            if self.result(r).is_some() {self.studio.physics.reset();self.invalidate();}
            return;
        }

        if matches!(action, P::Close) {
            self.primitive_panel = None;
            return;
        }
        if action == P::Backdrop {
            return;
        }
        if action == P::Collapse && self.creation_panel_target().is_some() {
            self.primitive_collapsed = !self.primitive_collapsed;
            return;
        }
        if action == P::Open && self.selected_curve().is_some() {
            if let Some(id) = self.creation_panel_target() {
                self.primitive_panel = Some(id);
                self.primitive_collapsed = false;
                self.curves.popup_scroll = 0.;
            }
            return;
        }
        let Some((id, settings)) = self.selected_primitive() else {
            return;
        };
        match action {
            P::CurvePoint(point) => self.surface.curve_point = point.min(3),
            P::Open => {
                self.primitive_panel = Some(id);
                self.primitive_collapsed = false;
            }
            P::Collapse => self.primitive_collapsed = !self.primitive_collapsed,
            P::Field(field) => {
                if let Some((value, _, _, _)) = self.primitive_field(field) {
                    self.input(Action::Primitive(action), ui::number_caption(value as f64));
                }
            }
            P::Smooth | P::Reset => {
                let mut next = if action == P::Reset {
                    PrimitiveSettings::new(settings.kind)
                } else {
                    settings
                };
                if action == P::Smooth {
                    next.smooth = !next.smooth;
                }
                if let Err(e) = self.set_primitive(next) {
                    self.status = e;
                }
            }
            P::Backdrop | P::Close | P::Orientation(_) => {}
        }
    }
    pub(super) fn draw_primitive_panel(&self, u: &mut Ui) {
        let Some([x, y, w, h]) = self.primitive_panel_rect() else {
            return;
        };
        if self.selected_curve().is_some() {
            self.draw_curve_creation_panel(u, [x, y, w, h]);
            return;
        }
        let Some((_, settings)) = self.selected_primitive() else {
            return;
        };
        let p = u.palette();
        u.floating_surface([x, y, w, h], 0.90, true);
        u.hits.push(ui::Hit {
            rect: [x, y, w, h],
            action: Action::Primitive(P::Backdrop),
        });
        u.quiet_icon_button(
            [x + 8., y + 4., w - 48., 30.],
            &format!("{} · параметры", settings.label()),
            Action::Primitive(P::Collapse),
            false,
            primitive_icon(settings.kind),
        );
        u.quiet_icon_button(
            [x + w - 36., y + 4., 28., 28.],
            "",
            Action::Primitive(P::Close),
            false,
            Icon::Close,
        );
        if self.primitive_collapsed {
            return;
        }
        let fields: &[u8] = match settings.kind {
            K::Cube => &[0, 1, 2, 3, 4, 5],
            K::Bezier => &[6, 3, 4, 10, 11, 12],
            K::CurveCircle => &[0, 6, 3, 4],
            K::Sphere => &[6, 3],
            K::Fox => &[0, 1, 2, 3],
            K::Cylinder|K::Cone|K::Capsule=>&[6,1,3,4],
            K::Torus=>&[6,0,3,4],
            K::Pyramid=>&[0,1,2],
            _ => &[0, 1, 3, 4],
        };
        let mut row = y + 44.;
        self.draw_creation_orientation(u,x+14.,row,w-28.);row+=38.;
        if settings.kind == K::Bezier {
            let cell = (w - 34.) / 4.;
            for point in 0..4u8 {
                u.quiet_button([x + 14. + point as f32 * (cell + 2.), row, cell, 28.],
                    &["P0", "P1", "P2", "P3"][point as usize], Action::Primitive(P::CurvePoint(point)), self.surface.curve_point == point);
            }
            row += 34.;
        }
        for &field in fields {
            let Some((value, min, max, label)) = self.primitive_field(field) else {
                continue;
            };
            let label = if matches!(settings.kind, K::Plane | K::Cloth) {
                match field {
                    0 => "Ширина, м",
                    1 => "Длина, м",
                    3 => "Сетка по ширине",
                    _ => "Сетка по длине",
                }
            } else {
                label
            };
            let range = if (3..=5).contains(&field) {
                SliderRange::new(min as f64, max as f64, 1.)
            } else if field >= 10 {
                SliderRange::new((-5_f64).min(value as f64), 5_f64.max(value as f64), 0.01)
            } else {
                SliderRange::new(min as f64, 10_f64.max(value as f64), 0.01)
            };
            u.number_field(
                [x + 14., row, w - 28., 32.],
                label,
                value as f64,
                Action::Primitive(P::Field(field)),
                range,
            );
            row += 38.;
        }
        u.checkbox(
            [x + 14., row, w - 28., 28.],
            "Сглаженные нормали",
            Action::Primitive(P::Smooth),
            settings.smooth,
        );
        row += 31.;
        let count = self
            .editor
            .selected
            .and_then(|id| self.editor.editable_mesh(id).ok())
            .map_or(0, |m| m.face_count());
        u.fitted_label(
            [x + 14., row, w - 28., 16.],
            &format!("{count} полигонов{}", if settings.is_curve() { " · параметрическая кривая" } else { "" }),
            11.,
            p.muted,
        );
        row += 25.;
        let half = (w - 34.) * 0.5;
        u.quiet_button(
            [x + 14., row, half, 28.],
            "Сбросить",
            Action::Primitive(P::Reset),
            false,
        );
        u.quiet_button(
            [x + 20. + half, row, half, 28.],
            "Готово",
            Action::Primitive(P::Close),
            true,
        );
    }
}

#[cfg(test)]
mod orientation_tests {
    use super::*;
    #[test]
    fn creation_axes_are_absolute_reversible_and_preserved_after_parameter_edits() {
        for curve in [false,true] {
            let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.preferences=Preferences::default();
            if curve {a.create_curve(1);}else {a.create_primitive(1);}
            let id=a.editor.selected.unwrap();let before=a.editor.project.clone();let count=a.editor.undo_len();let normal=a.creation_normal();
            a.primitive_action(P::Orientation(0));assert!((a.editor.project.node(id).unwrap().transform.rotation*normal).distance(Vec3::X)<1e-5);
            assert_eq!(a.editor.undo_len(),count+1);let rotation=a.editor.project.node(id).unwrap().transform.rotation;
            if curve {a.curve_confirm(&Action::Curve(ui::CurveAction::Field(3)),"24").unwrap();}
            else {a.primitive_confirm(&Action::Primitive(P::Field(0)),"3").unwrap();}
            assert_eq!(a.editor.project.node(id).unwrap().transform.rotation,rotation);
            a.editor.undo();a.editor.undo();assert_eq!(a.editor.project,before);
            a.primitive_action(P::Orientation(2));assert!((a.editor.project.node(id).unwrap().transform.rotation*normal).distance(Vec3::Z)<1e-5);
        }
    }
    #[test]
    fn axis_controls_are_in_mesh_and_curve_creation_overlays() {
        for theme in [ui::Theme::Light,ui::Theme::Dark] {for scale in [1.,2.] {for curve in [false,true] {
            let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.preferences=Preferences::default();a.preferences.theme=theme;a.scale=scale;a.width=1440.;a.height=900.;
            if curve {a.create_curve(0);}else{a.create_primitive(0);}
            let u=a.build_ui();for axis in 0..3 {assert!(u.action_point(&Action::Primitive(P::Orientation(axis))).is_some());}
        }}}
    }
}
