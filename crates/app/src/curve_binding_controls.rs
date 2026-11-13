//! One curve constraint workflow for meshes, scene cameras and lights.
use super::*;
use crate::ui::PathAction as P;
use poseforge_core::CurveBinding;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BindingTarget { Node(u64), Light(usize) }

impl App {
    pub(super) fn pick_path_node(&mut self,id:u64) {
        let Some(target)=self.path_pick else{return;};
        if self.path_target()!=Some(target) || !self.path_editable() {self.path_pick=None;self.redraw();return;}
        if !self.node_selectable(id) || self.editor.project.node(id).is_none_or(|n|n.curve.is_none()) {
            self.status="Нужна доступная кривая · пипетка остаётся активной · Esc — отмена".into();self.redraw();return;
        }
        self.path_pick=None;self.path_action(P::Bind(id));self.redraw();
    }
    pub(super) fn path_pick_pointer(&mut self,button:MouseButton)->bool {
        let Some(target)=self.path_pick else{return false;};
        if self.path_target()!=Some(target) || !self.path_editable() {self.path_pick=None;self.redraw();return true;}
        if button==MouseButton::Right {self.path_pick=None;self.status="Выбор кривой отменён".into();self.redraw();return true;}
        if button!=MouseButton::Left {return false;}
        if let Some(action)=self.ui.hit(self.mouse) {
            if let Action::Select(id)=action {self.pick_path_node(id);}else{self.path_pick=None;}
        } else if self.canvas_input_at(self.mouse) {
            if let Some(id)=self.pick_curve_at(self.mouse) {self.pick_path_node(id);}else{self.status="Здесь нет кривой · пипетка остаётся активной · Esc — отмена".into();}
        } else {self.path_pick=None;}
        self.redraw();true
    }

    pub(super) fn object_curve_translation(&self, id: u64, scene: &poseforge_core::EvaluatedScene)
        -> Result<Option<Arc<poseforge_core::CurveTranslation>>, poseforge_core::CoreError>
    {
        let node = self.editor.project.node(id).ok_or(poseforge_core::CoreError::MissingNode(id))?;
        let Some(binding) = node.curve_binding else { return Ok(None); };
        if self.path_offset_editing_for(BindingTarget::Node(id)) { return Ok(None); }
        let local = scene.local.get(&id).ok_or(poseforge_core::CoreError::MissingNode(id))?;
        self.editor.project.curve_translation(binding, &scene.global, self.editor.time, local.translation, node.camera.is_some())
            .map(|path| Some(Arc::new(path)))
    }
    pub(super) fn set_object_curve_position(&mut self, id: u64, path: &poseforge_core::CurveTranslation,
        position: f32) -> Result<(), poseforge_core::CoreError>
    {
        let source = path.binding();
        if !self.node_selectable(id) || self.editor.project.node(source.curve).is_none_or(|n| n.curve.is_none()) {
            return Err(poseforge_core::CoreError::Invalid("Объект или кривая недоступны".into()));
        }
        let node = self.editor.project.node_mut(id).ok_or(poseforge_core::CoreError::MissingNode(id))?;
        let binding = node.curve_binding.as_mut().ok_or_else(|| poseforge_core::CoreError::Invalid("Привязка объекта изменилась".into()))?;
        if (CurveBinding { position: source.position, ..*binding }) != source {
            return Err(poseforge_core::CoreError::Invalid("Привязка объекта изменилась".into()));
        }
        let next = CurveBinding { position, ..*binding }; next.validate()?; *binding = next; Ok(())
    }
    pub(super) fn translate_object_on_curve(&mut self, id: u64, path: &poseforge_core::CurveTranslation,
        target: Vec3) -> Result<(), poseforge_core::CoreError>
    {
        self.set_object_curve_position(id, path, path.position(target)?)
    }
    pub(super) fn path_target(&self) -> Option<BindingTarget> {
        if self.surface.tab != 0 { return None; }
        if let Some(index) = self.active_light_index() { return Some(BindingTarget::Light(index)); }
        let id = self.editor.selected?;
        let node = self.editor.project.node(id)?;
        if node.fx.is_some_and(|f|f.source.is_some()) {return None;}
        (node.curve.is_none() && !node.is_joint && (node.mesh.is_some() || node.camera.is_some() || node.empty) && !self.rig_context())
            .then_some(BindingTarget::Node(id))
    }
    pub(super) fn path_editable(&self) -> bool {
        self.job.is_none() && self.native_picker.is_none() && !self.scene_write_pending() && !self.editor.has_staged() && match self.path_target() {
            Some(BindingTarget::Node(id)) => self.node_selectable(id),
            Some(BindingTarget::Light(index)) => !self.editor.project.light_locked(index),
            None => false,
        }
    }
    pub(super) fn path_binding(&self) -> Option<CurveBinding> {
        match self.path_target()? {
            BindingTarget::Node(id) => self.editor.project.node(id)?.curve_binding,
            BindingTarget::Light(index) => self.editor.project.lights.get(index)?.curve_binding,
        }
    }
    pub(super) fn path_curve_choices(&self) -> Vec<u64> {
        self.editor.project.nodes.iter().filter(|n| n.curve.is_some()).map(|n| n.id).collect()
    }
    pub(super) fn path_panel_start(&self) -> Option<f32> {
        match self.path_target()? {
            BindingTarget::Light(_) => Some(1690.),
            BindingTarget::Node(id) if self.editor.project.node(id)?.camera.is_some() => Some(self.camera_panel_bottom()),
            BindingTarget::Node(id) if self.editor.project.node(id)?.text.is_some() => Some(text_controls::PANEL_BOTTOM),
            BindingTarget::Node(id) if self.editor.project.node(id)?.empty => Some(self.helper_panel_bottom()),
            BindingTarget::Node(_) => Some(602.),
        }
    }
    pub(super) fn path_panel_bottom(&self) -> Option<f32> {
        Some(self.path_panel_start()? + if self.path_binding().is_some() { 316. } else { 86. })
    }
    fn set_path_binding(&mut self, target: BindingTarget, binding: CurveBinding) -> Result<(), String> {
        binding.validate().map_err(|e| e.to_string())?;
        self.editor.transact("Привязка к кривой", |p| match target {
            BindingTarget::Node(id) => {
                let node = p.node_mut(id).unwrap();
                if node.curve_binding.is_none() {
                    node.transform.translation = Vec3::ZERO; node.transform.rotation = glam::Quat::IDENTITY;
                    node.rest_transform = Some(node.transform);
                }
                node.curve_binding = Some(binding);
            }
            BindingTarget::Light(index) => {
                let light = &mut p.lights[index];
                if light.curve_binding.is_none() { light.position = Vec3::ZERO; light.direction = Vec3::NEG_Z; }
                light.curve_binding = Some(binding);
            }
        }).map_err(|e| e.to_string())?;
        self.update_dirty(); Ok(())
    }
    pub(super) fn path_action(&mut self, action: P) {
        let Some(target) = self.path_target() else { return; };
        if !self.path_editable() { return; }
        if action == P::Edit { self.toggle_path_edit(); return; }
        if action == P::ResetOffset { self.reset_path_offset(); return; }
        if matches!(action, P::Pick | P::Bind(_) | P::Clear | P::SelectCurve) { self.path_edit = None; }
        if action==P::Pick {
            self.cancel_drag();self.close_collision_selection();self.authoring.reference_pick=None;self.focus_pick=None;self.light_aim=None;self.dropdown=None;
            self.editor.playing=false;self.path_pick=Some(target);self.status="Выберите кривую в сцене или списке · Esc / правая кнопка — отмена".into();self.redraw();return;
        }
        if action == P::SelectCurve {
            if let Some(binding) = self.path_binding() { self.action(Action::Select(binding.curve)); }
            return;
        }
        self.editor.playing = false;
        if let P::Field(field) = action {
            if let Some(binding) = self.path_binding() {
                let value = if field == 0 { binding.position } else { binding.speed };
                self.input(Action::Path(action), ui::number_caption(value as f64 * 100.));
            }
            return;
        }
        let result = if action == P::Clear {
            if self.path_binding().is_none() { return; }
            let scene = match self.evaluated_interactive() { Ok(scene) => scene, Err(e) => { self.status = e.to_string(); return; } };
            match target {
                BindingTarget::Node(id) => {
                    let mut prepared = self.editor.project.clone();
                    match prepared.detach_curve_node(id, &scene.global, scene.local[&id]) {
                        Ok(()) => self.editor.transact("Отвязать от кривой", |p| *p = prepared).map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    }
                }
                BindingTarget::Light(index) => self.editor.transact("Отвязать свет от кривой", |p| {
                    p.lights[index] = scene.lights[index].clone();
                }).map_err(|e| e.to_string()),
            }
        } else {
            let mut binding = self.path_binding().unwrap_or_default();
            match action {
                P::Bind(curve) => {
                    if self.editor.project.node(curve).is_none_or(|n| n.curve.is_none()) { return; }
                    binding.curve = curve;
                }
                P::Follow if self.path_binding().is_some() => binding.follow = !binding.follow,
                P::Repeat if self.path_binding().is_some() => binding.repeat = !binding.repeat,
                _ => return,
            }
            self.set_path_binding(target, binding)
        };
        if self.result(result).is_some() { self.status = "Привязка обновлена · перемещение вдоль пути · Edit — свободное смещение · Undo — отмена".into(); }
    }
    pub(super) fn path_confirm(&mut self, action: &Action, value: &str) -> Result<bool, String> {
        let Action::Path(P::Field(field)) = action else { return Ok(false); };
        let target = self.path_target().ok_or("Выберите меш, камеру или свет")?;
        if !self.path_editable() { return Err("Объект недоступен для изменения".into()); }
        let mut binding = self.path_binding().ok_or("Сначала выберите кривую")?;
        let number: f64 = value.trim().replace(',', ".").parse().map_err(|_| "Введите число")?;
        if !number.is_finite() || (*field == 0 && !(0. ..=100.).contains(&number))
            || (*field == 1 && number.abs() > 10000.) || *field > 1
        { return Err("Положение: 0–100 %. Скорость: от −10000 до 10000 %/с".into()); }
        if *field == 0 { binding.position = (number / 100.) as f32; } else { binding.speed = (number / 100.) as f32; }
        self.set_path_binding(target, binding)?;
        Ok(true)
    }
    pub(super) fn draw_path_panel(&self, u: &mut Ui, rx: f32, rw: f32) {
        let Some(start) = self.path_panel_start() else { return; };
        let x = rx + 16.; let width = rw - 32.; let y = start + 8.; let p = u.palette();
        let binding = self.path_binding();
        let label = binding.and_then(|b| self.editor.project.node(b.curve)).map_or("Выбрать кривую…", |n| n.name.as_str());
        if self.path_editable() && !self.path_curve_choices().is_empty() {
            self.draw_dropdown_button(u, [x,y,width-38.,30.], ui::DropdownKind::CurvePath, label);
            u.quiet_icon_button([x+width-32.,y,32.,30.],"",Action::Path(P::Pick),self.path_pick.is_some(),Icon::Eyedropper);
        } else {
            u.disabled_button([x,y,width-38.,30.],if self.path_curve_choices().is_empty() {"Нет кривых в сцене"} else {label});
            u.disabled_icon_button([x+width-32.,y,32.,30.],"",Icon::Eyedropper);
        }
        let Some(binding) = binding else {
            u.fitted_label([x,y+42.,width,20.], "Создайте путь через меню «Кривые»", 11., p.muted); return;
        };
        let start_hits = u.hits.len();
        u.number_field([x,y+42.,width,32.], "Положение, %", binding.position as f64 * 100.,
            Action::Path(P::Field(0)), SliderRange::new(0.,100.,0.1));
        u.number_field([x,y+82.,width,32.], "Скорость, %/с", binding.speed as f64 * 100.,
            Action::Path(P::Field(1)), SliderRange::new(-100.,100.,1.));
        if self.selected_text().is_some_and(|(_,s)|s.on_curve) {u.disabled_button([x,y+124.,width,28.],"Направление букв задаёт кривая");}
        else {u.checkbox([x,y+124.,width,28.], "Поворот по кривой", Action::Path(P::Follow), binding.follow);}
        u.checkbox([x,y+158.,width,28.], "Повторять движение", Action::Path(P::Repeat), binding.repeat);
        let half = (width - 6.) * 0.5;
        u.outline_button([x,y+196.,width-36.,30.], "Edit", Action::Path(P::Edit), self.editing_path_curve().is_some());
        if self.path_offset().is_some_and(|offset| offset != Vec3::ZERO) {
            u.quiet_icon_button([x+width-30.,y+196.,30.,30.], "", Action::Path(P::ResetOffset), false, Icon::Reset);
        } else { Icon::Reset.draw(u,[x+width-24.,y+202.,18.,18.],p.disabled); }
        u.quiet_icon_button([x,y+234.,half,30.], "Кривая", Action::Path(P::SelectCurve), false, Icon::Curve);
        u.quiet_button([x+half+6.,y+234.,half,30.], "Отвязать", Action::Path(P::Clear), false);
        u.fitted_label([x,y+276.,width,20.], if self.editing_path_curve().is_some() { "Edit · свободное смещение · Esc — выход" }
            else { "G — вдоль пути · Edit — свободно" }, 10., p.muted);
        if !self.path_editable() { u.hits.truncate(start_hits); }
    }
}
