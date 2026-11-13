//! Liquid and cloth are object properties below Physics, alongside material editing.
use super::*;
use crate::ui::FluidObjectAction as F;
use poseforge_core::{FluidDomainSettings, MaterialPreset, PhysicsKind};

pub(super) const PANEL_BOTTOM: f32 = 980.;
pub(super) fn value(settings: FluidDomainSettings, field: u8) -> Option<f64> {
    let s = settings.simulation;
    Some(match field {
        0 => s.gravity, 1 => s.viscosity, 2 => s.vorticity, 3 => s.boundary_friction,
        4 => s.iterations as f32, 5 => s.max_substeps as f32, 6 => s.time_scale,
        7 => s.surface_cell_size, 8 => s.surface_iso_density, 9 => settings.fill * 100.,
        _ => return None,
    } as f64)
}

fn prepare_attachment(before: &Project, id: u64, settings: FluidDomainSettings, cancel: &AtomicBool)
    -> Result<(Project, fluid_domains::PreparedDomain), String>
{
    if cancel.load(Ordering::Relaxed) { return Err("Подготовка жидкости отменена".into()); }
    let mut candidate = destruction_controls::without_destruction(before, id, true)?;
    let source = candidate.node(id).and_then(|n| n.mesh).and_then(|m| candidate.mesh(m))
        .ok_or("Меш контейнера удалён")?.clone();
    let mut material = MaterialPreset::Water.create();
    material.id = candidate.next_id();
    material.name = format!("Жидкость · {}", candidate.node(id).unwrap().name);
    let mut mesh = source;
    mesh.id = material.id.checked_add(1).ok_or("Закончились идентификаторы ресурсов")?;
    mesh.material = Some(material.id);
    mesh.clear_face_materials();
    let node = candidate.node_mut(id).unwrap();
    node.mesh = Some(mesh.id); node.physics = None; node.fluid = Some(settings);
    node.collision_targets.fluid = Some(Vec::new());
    candidate.materials.push(material); candidate.meshes.push(mesh);
    candidate.validate().map_err(|error| error.to_string())?;
    let ready = fluid_domains::prepare_domain(&candidate, id, cancel)?;
    Ok((candidate, ready))
}

impl App {
    pub(super) fn finish_fluid_attachment(&mut self, before: Project, after: Project, id: u64, ready: fluid_domains::PreparedDomain) {
        self.job = None;
        if self.editor.project != before {
            self.status = "Сцена изменилась; повторите назначение жидкости.".into(); return;
        }
        let result = self.editor.transact("Назначить жидкость примитиву", |p| *p = after);
        if self.result(result).is_some() {
            self.fluid_domains.install(id, ready);
            self.scene_tools.material_expanded.insert(id);
            self.set_properties_tab(4); self.authoring.editing = None;
            self.studio.physics_running = false; self.studio.physics.reset();
            self.tick_fluid_domains(); self.invalidate();
            self.status = "Жидкость назначена. Поверхность готова; ▶ запускает симуляцию.".into();
        }
    }
    #[cfg(test)]
    pub(super) fn wait_for_background_jobs(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while self.job.is_some() && Instant::now() < deadline {
            self.poll_jobs(); std::thread::sleep(Duration::from_millis(2));
        }
        assert!(self.job.is_none(), "background operation did not finish");
    }
    pub(super) fn selected_fluid(&self) -> Option<(u64, FluidDomainSettings)> {
        if self.active_light_index().is_some() { return None; }
        let id = self.editor.selected?;
        Some((id, self.editor.project.node(id)?.fluid?))
    }
    pub(super) fn fluid_object_action(&mut self, action: F) {
        if self.job.is_some() || self.native_picker.is_some() { return; }
        if self.active_light_index().is_some() {
            self.status = "Выберите объёмный примитив в сцене.".into();
            return;
        }
        let Some(id) = self.editor.selected.filter(|id| self.node_selectable(*id) || (!self.editor.project.node_locked(*id) && self.editor.project.node(*id).is_some_and(|n| n.destruction.is_some()))) else {
            self.status = "Выберите незаблокированный объёмный примитив.".into();
            return;
        };
        if action == F::Attach {
            if self.editor.project.node(id).is_some_and(|n| n.fluid.is_some()) {
                self.set_properties_tab(4);
                return;
            }
            let Some(source) = self.editor.project.node(id).and_then(|n| n.mesh)
                .and_then(|m| self.editor.project.mesh(m)).cloned() else { return };
            let (min, max) = super::fluid_domains::mesh_bounds(&source);
            if !min.is_finite() || !max.is_finite() || (max - min).min_element() < 0.08 {
                self.status = "Жидкости нужен объёмный примитив. Для плоскости выберите «Ткань».".into();
                return;
            }
            let settings = FluidDomainSettings {
                simulation: self.editor.project.fluid_settings,
                ..Default::default()
            };
            let before = self.editor.project.clone();
            let cancel = Arc::new(AtomicBool::new(false)); let c = cancel.clone();
            let (tx, receiver) = mpsc::channel();
            match std::thread::Builder::new().name("attach-fluid".into()).spawn(move || {
                let result = prepare_attachment(&before, id, settings, &c);
                let message = match result {
                    Ok((after, ready)) => JobMessage::FluidAttached(Box::new(before), Box::new(after), id, Box::new(ready)),
                    Err(error) => JobMessage::Error(error),
                };
                let _ = tx.send(message);
            }) {
                Ok(_) => {
                    self.job = Some(Job { receiver, cancel, label: "Подготовка жидкости".into() });
                    self.status = "Подготовка формы жидкости… Esc — отменить.".into();
                    self.redraw();
                }
                Err(error) => self.status = format!("Не удалось запустить подготовку жидкости: {error}"),
            }
            return;
        }
        let Some((_, settings)) = self.selected_fluid() else { return };
        match action {
            F::Field(field) => {
                if let Some(v) = value(settings, field) {
                    self.input(Action::FluidObject(action), if matches!(field, 4 | 5) { format!("{v:.0}") } else { v.to_string() });
                }
            }
            F::Material => {
                self.surface.material_library = false;
                self.surface.object_material = None;
                self.action(Action::SurfaceTab(1));
            }
            F::Play => {
                if !settings.enabled { self.status = "Сначала включите участие жидкости в симуляции.".into(); return; }
                self.editor.playing = false;
                self.fluid_domains.toggle_playback(id);
                self.last_tick = Instant::now();
                self.status = if self.fluid_domains.playing(id) {
                    "Предпросмотр жидкости без ключей · время анимации не меняется."
                } else { "Предпросмотр жидкости на паузе." }.into();
            }
            F::Reset => {
                self.editor.playing = false;
                self.fluid_domains.reset_playback(id);
                self.tick_fluid_domains();
            }
            F::Remove | F::Enabled | F::MeshCollision | F::Preset(_) => {
                let result = self.editor.transact("Изменить жидкость объекта", |p| {
                    let node = p.node_mut(id).unwrap();
                    match action {
                        F::Remove => node.fluid = None,
                        F::Enabled => node.fluid.as_mut().unwrap().enabled = !settings.enabled,
                        F::MeshCollision => node.fluid.as_mut().unwrap().mesh_collision = !settings.mesh_collision,
                        F::Preset(preset) => {
                            super::scene_controls::apply_fluid_preset(&mut node.fluid.as_mut().unwrap().simulation, preset);
                        }
                        _ => {}
                    }
                });
                if self.result(result).is_some() {
                    self.tick_fluid_domains();
                    self.invalidate();
                }
            }
            F::Attach => {}
        }
    }
    pub(super) fn fluid_object_confirm(&mut self, action: &Action, text: &str) -> Result<bool, String> {
        let Action::FluidObject(F::Field(field)) = *action else { return Ok(false) };
        let (id, mut settings) = self.selected_fluid().ok_or("Жидкость больше не выбрана")?;
        if !self.node_selectable(id) { return Err("Контейнер заблокирован".into()); }
        let number = text.trim().replace(',', ".").parse::<f32>().ok()
            .filter(|v| v.is_finite()).ok_or("Введите конечное число")?;
        if matches!(field, 4 | 5) && number.fract() != 0. { return Err("Введите целое число".into()); }
        let s = &mut settings.simulation;
        match field {
            0 => s.gravity = number, 1 => s.viscosity = number, 2 => s.vorticity = number,
            3 => s.boundary_friction = number,
            4 if (1. ..=16.).contains(&number) => s.iterations = number as u32,
            5 if (1. ..=8.).contains(&number) => s.max_substeps = number as u32,
            4 | 5 => return Err("Итерации: 1–16; подшаги: 1–8".into()),
            6 => s.time_scale = number, 7 => s.surface_cell_size = number,
            8 => s.surface_iso_density = number, 9 => settings.fill = number / 100.,
            _ => return Err("Неизвестный параметр жидкости".into()),
        }
        settings.validate().map_err(|e| e.to_string())?;
        self.editor.transact("Параметр жидкости объекта", |p| p.node_mut(id).unwrap().fluid = Some(settings))
            .map_err(|e| e.to_string())?;
        self.update_dirty();
        Ok(true)
    }
    pub(super) fn draw_fluid_object(&self, u: &mut Ui, rx: f32, rw: f32) {
        let x = rx + 16.;
        let width = rw - 32.;
        let p = u.palette();
        let Some((id, settings)) = self.selected_fluid() else {
            u.fitted_label([x, 174., width, 20.], "Жидкость назначается примитиву", 12., p.text);
            u.fitted_label([x, 200., width, 20.], "Замкнутая форма меша задаёт жидкость.", 11., p.muted);
            u.icon_button([x, 234., width, 32.], "Добавить жидкость", Action::FluidObject(F::Attach), false, Icon::Fluid);
            u.fitted_label([x, 282., width, 20.], "Сначала выберите куб или другой объём.", 11., p.muted);
            return;
        };
        let (count, time, error) = self.fluid_domains.info(id);
        let shift = self.collision_layout_shift(poseforge_core::CollisionKind::Fluid);
        u.fitted_label([x, 174., width, 20.], "Жидкость по форме примитива", 12., p.text);
        u.fitted_label([x, 200., width, 20.], &format!("{count} частиц · {time:.2} с · живая поверхность"), 10., p.muted);
        let material = self.editor.project.node(id).and_then(|n| n.mesh)
            .and_then(|m| self.editor.project.mesh(m)).and_then(|m| m.material)
            .and_then(|id| self.editor.project.material(id)).map(|m| m.name.as_str()).unwrap_or("Материал");
        u.icon_button([x, 230., width, 32.], material, Action::FluidObject(F::Material), false, Icon::Material);
        let half = (width - 8.) * 0.5;
        let playing = self.fluid_domains.playing(id);
        u.icon_button([x, 272., half, 30.], if playing { "Пауза" } else { "Запустить" },
            Action::FluidObject(F::Play), playing, if playing { Icon::Pause } else { Icon::Play });
        u.icon_button([x + half + 8., 272., half, 30.], "В начало", Action::FluidObject(F::Reset), false, Icon::Start);
        u.checkbox([x, 312., width, 30.], "Участвует в симуляции", Action::FluidObject(F::Enabled), settings.enabled);
        u.checkbox([x, 348., width, 30.], "Столкновения с мешами", Action::FluidObject(F::MeshCollision), settings.mesh_collision);
        self.draw_collision_list(u, poseforge_core::CollisionKind::Fluid, x, 380., width);
        u.button([x, 410. + shift, half, 30.], "Вода", Action::FluidObject(F::Preset(0)), false);
        u.button([x + half + 8., 410. + shift, half, 30.], "Густая", Action::FluidObject(F::Preset(1)), false);
        for (row, (field, label, min, max, step)) in [
            (9, "Заполнение, %", 10., 100., 1.),
            (0, "Гравитация, м/с²", 0., 30., 0.1),
            (1, "Вязкость", 0., 0.2, 0.005),
            (2, "Вихри", 0., 5., 0.01),
            (3, "Трение стенок", 0., 1., 0.01),
            (4, "Итерации", 1., 16., 1.),
            (5, "Подшаги, макс.", 1., 8., 1.),
            (6, "Скорость времени", 0.1, 2., 0.05),
            (7, "Ячейка поверхности, м", 0.025, 0.12, 0.001),
            (8, "Порог поверхности", 0.1, 1.2, 0.01),
        ].into_iter().enumerate() {
            u.number_field([x, 456. + shift + row as f32 * 42., width, 30.], label,
                value(settings, field).unwrap(), Action::FluidObject(F::Field(field)), SliderRange::new(min, max, step));
        }
        u.button([x, 886. + shift, width, 30.], "Убрать жидкость с объекта", Action::FluidObject(F::Remove), false);
        let surface_status = if self.fluid_domains.initializing(id) { "Подготовка поверхности…" }
            else if self.fluid_domains.surface_pending(id, settings) { "Обновление поверхности…" }
            else { "Поверхность обновляется автоматически." };
        u.fitted_label([x, 932. + shift, width, 20.], error.unwrap_or(surface_status), 11., if error.is_some() { p.warn } else { p.muted });
    }
    pub(super) fn draw_physics_workspace(&self, u: &mut Ui, rx: f32, rw: f32) {
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        let cursor_y = self.property_cursor_y(self.mouse[1] + self.inspector_scroll.min(self.inspector_max_scroll()) + properties::CONTENT_SHIFT);
        u.set_cursor([self.mouse[0], cursor_y - 44.]);
        if self.surface.tab == 13 {
            self.draw_destruction(u, rx, rw);
        } else if self.surface.tab == 4 || (self.surface.tab == 6 && self.selected_fluid().is_some()) {
            self.draw_fluid_object(u, rx, rw);
        } else if self.surface.tab == 11 && self.editor.selected.and_then(|id| self.editor.project.node(id))
            .is_none_or(|n| n.physics.is_none_or(|s| s.kind != PhysicsKind::Cloth))
        {
            let p = u.palette();
            u.fitted_label([rx + 16., 174., rw - 32., 20.], "Ткань — физика выбранного меша", 12., p.text);
            u.icon_button([rx + 16., 208., rw - 32., 32.], "Назначить ткань", Action::Studio(ui::StudioAction::Physics(2)), false, Icon::Cloth);
            u.fitted_label([rx + 16., 256., rw - 32., 20.], "Подойдёт плоскость с подразделениями.", 11., p.muted);
        } else {
            self.draw_physics_panel(u, rx, rw);
        }
        u.translate_section(start, 44.);
        u.set_cursor([self.mouse[0], cursor_y]);
        let width = (rw - 48.) / 3.;
        u.quiet_icon_button([rx + 16., 174., width, 30.], "Жидкость", Action::SurfaceTab(4), self.surface.tab == 4 || (self.surface.tab == 6 && self.selected_fluid().is_some()), Icon::Fluid);
        let cloth = self.surface.tab == 11 || (self.surface.tab == 6 && self.editor.selected
            .and_then(|id| self.editor.project.node(id)).and_then(|n| n.physics)
            .is_some_and(|s| s.kind == PhysicsKind::Cloth));
        u.quiet_icon_button([rx + 24. + width, 174., width, 30.], "Ткань", Action::SurfaceTab(11), cloth, Icon::Cloth);
        u.quiet_icon_button([rx + 32. + width * 2., 174., width, 30.], "Разрушение", Action::SurfaceTab(13), self.surface.tab == 13, Icon::Fracture);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluid_collision_toggle_is_one_undo_and_keeps_the_container_mesh() {
        let mut app = App::new(None);
        let mut project = Project::default();
        let mut node = poseforge_core::Node::new(1, "Liquid"); node.mesh = Some(2);
        node.fluid = Some(FluidDomainSettings::default()); project.nodes.push(node);
        project.meshes.push(poseforge_core::box_mesh(2, "Container", Vec3::splat(0.3), None));
        app.editor = Editor::new(project); app.reset_workflow_selection(); app.editor.selected = Some(1);
        let before = app.editor.project.clone(); let history = app.editor.undo_len();
        app.fluid_object_action(F::MeshCollision);
        assert!(!app.editor.project.node(1).unwrap().fluid.unwrap().mesh_collision);
        assert_eq!(app.editor.project.meshes, before.meshes);
        assert_eq!(app.editor.undo_len(), history + 1);
        app.editor.undo(); assert_eq!(app.editor.project, before);
        app.fluid_domains.clear();
    }

    #[test]
    fn cancelled_attachment_keeps_source_and_undo_history_unchanged() {
        let mut app = App::new(None);
        app.editor = Editor::new(Project::default());
        app.create_primitive(0);
        let before = app.editor.project.clone();
        let history = app.editor.undo_len();
        app.fluid_object_action(F::Attach);
        assert_eq!(app.editor.project, before, "worker must not mutate the document");
        app.job.as_ref().unwrap().cancel.store(true, Ordering::Relaxed);
        app.wait_for_background_jobs();
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo_len(), history);
        assert!(app.fluid_domains.info(app.editor.selected.unwrap()).0 == 0);
    }

    #[test]
    fn late_attachment_cannot_overwrite_a_changed_document() {
        let mut app = App::new(None);
        app.editor = Editor::new(Project::default());
        app.create_primitive(0);
        app.fluid_object_action(F::Attach);
        app.editor.project.title = "Changed while preparing".into();
        let changed = app.editor.project.clone();
        app.wait_for_background_jobs();
        assert_eq!(app.editor.project, changed);
        assert!(app.status.contains("Сцена изменилась"));
    }

    #[test]
    fn attach_uses_independent_material_and_container_and_is_one_undo() {
        let mut app = App::new(None);
        app.editor = Editor::new(Project::default());
        app.create_primitive(0);
        let id = app.editor.selected.unwrap();
        let before = app.editor.project.clone();
        let history = app.editor.undo_len();
        app.fluid_object_action(F::Attach);
        app.wait_for_background_jobs();
        let node = app.editor.project.node(id).unwrap();
        assert!(node.fluid.is_some() && node.physics.is_none());
        let mesh = app.editor.project.mesh(node.mesh.unwrap()).unwrap();
        assert_ne!(node.mesh, before.node(id).unwrap().mesh);
        assert!(mesh.topology.as_ref().unwrap().primitive.is_some());
        assert_eq!(app.editor.project.material(mesh.material.unwrap()).unwrap().ior, 1.333);
        assert_eq!(app.editor.undo_len(), history + 1);
        assert_eq!(app.surface.tab, 4);
        assert!(app.editor.undo());
        assert_eq!(app.editor.project, before);
        assert!(app.editor.redo());
        let saved = app.editor.project.clone();
        app.tick_fluid_domains();
        assert_eq!(app.editor.project, saved);
    }

    #[test]
    fn plane_uses_cloth_physics_with_a_bending_grid_and_own_tab() {
        let mut app = App::new(None);
        app.editor = Editor::new(Project::default());
        app.create_primitive(1);
        let before = app.editor.project.clone();
        app.fluid_object_action(F::Attach);
        app.wait_for_background_jobs();
        assert_eq!(app.editor.project, before);
        app.action(Action::Studio(ui::StudioAction::Physics(2)));
        let node = app.editor.project.node(app.editor.selected.unwrap()).unwrap();
        assert_eq!(node.physics.unwrap().kind, PhysicsKind::Cloth);
        assert!(node.fluid.is_none());
        assert_eq!(app.surface.tab, 11);
        assert!(app.editor.project.mesh(node.mesh.unwrap()).unwrap().vertices.len() > 4);
        assert!(app.editor.undo());
        assert_eq!(app.editor.project, before);
    }
}
