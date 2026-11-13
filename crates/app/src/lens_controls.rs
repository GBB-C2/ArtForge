//! Camera focus and saved viewpoints use document history; guides stay out of export.
use super::*;
use crate::ui::CameraTool as C;
use poseforge_core::{Camera, SavedView};

impl App {
    pub(super) fn camera_snapshot(&self) -> Camera {
        let view = self.view();
        Camera {
            eye: view.eye,
            target: view.target,
            up: view.up,
            fov_y_radians: view.fov_y,
            orthographic: view.orthographic.is_some(),
            ortho_size: view.orthographic.unwrap_or(3.),
            depth_of_field: view.depth_of_field,
            effects: view.effects,
            ..Camera::default()
        }
    }
    pub(super) fn lens_action(&mut self, action: C) {
        match action {
            C::SaveView => {
                if self.editor.project.saved_views.len() >= 64 {
                    self.status = "В проекте уже 64 ракурса.".into();
                    return;
                }
                self.input(
                    Action::CameraTool(action),
                    format!("Вид {}", self.editor.project.saved_views.len() + 1),
                );
            }
            C::RecallView(index) => {
                if let Some(view) = self.editor.project.saved_views.get(index) {
                    self.recalled_view = Some(view.camera.clone());
                    self.last_saved_view = Some(index);
                    self.camera_view = None;
                    self.editor.playing = false;
                    self.tool = Tool::Orbit;
                    self.performance.changed();
                    self.status = format!("Ракурс «{}»", view.name);
                }
            }
            C::DeleteView => {
                if let Some(index) = self
                    .last_saved_view
                    .filter(|i| *i < self.editor.project.saved_views.len())
                {
                    let result = self.editor.transact("Удалить ракурс", |p| {
                        p.saved_views.remove(index);
                    });
                    if self.result(result).is_some() {
                        self.last_saved_view = None;
                    }
                }
            }
            C::Dof | C::Thirds | C::PickFocus | C::Effects => {
                let Some(id) = self
                    .selected_camera_id()
                    .or(self.camera_view)
                    .filter(|id| self.node_selectable(*id))
                else {
                    self.status = "Выберите доступную камеру.".into();
                    return;
                };
                if action == C::PickFocus {
                    self.camera_view = Some(id);
                    self.recalled_view = None;
                    self.focus_pick = Some(id);
                    self.tool = Tool::Orbit;
                    self.status = "Щёлкните по поверхности для фокусировки. Esc — отмена.".into();
                    return;
                }
                let result = self.editor.transact("Объектив камеры", |p| {
                    if let Some(lens) = p.node_mut(id).and_then(|n| n.camera.as_mut()) {
                        if action == C::Dof {
                            lens.depth_of_field.enabled = !lens.depth_of_field.enabled;
                        } else if action == C::Effects {
                            lens.effects.enabled = !lens.effects.enabled;
                        } else {
                            lens.thirds = !lens.thirds;
                        }
                    }
                });
                self.result(result);
            }
        }
        self.update_dirty();
    }
    pub(super) fn saved_view_confirm(
        &mut self,
        action: &Action,
        name: &str,
    ) -> Result<bool, String> {
        if *action != Action::CameraTool(C::SaveView) {
            return Ok(false);
        }
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err("Название: от 1 до 64 символов без управляющих знаков.".into());
        }
        if self.editor.project.saved_views.len() >= 64 {
            return Err("Не более 64 ракурсов.".into());
        }
        if self
            .editor
            .project
            .saved_views
            .iter()
            .any(|v| v.name == name)
        {
            return Err("Ракурс с таким названием уже существует.".into());
        }
        let camera = self.camera_snapshot();
        camera.validate().map_err(|e| e.to_string())?;
        self.editor
            .transact("Сохранить ракурс", |p| {
                p.saved_views.push(SavedView {
                    name: name.into(),
                    camera,
                })
            })
            .map_err(|e| e.to_string())?;
        self.last_saved_view = Some(self.editor.project.saved_views.len() - 1);
        self.update_dirty();
        Ok(true)
    }
    pub(super) fn focus_pointer_down(&mut self) -> bool {
        let Some(id) = self.focus_pick else {
            return false;
        };
        if !self.node_selectable(id) {
            self.focus_pick = None;
            return true;
        }
        let Ok(eval) = self.evaluated_interactive() else {
            return true;
        };
        let Some(hit) = crate::picking::pick_surface(
            &self.picking_project(),
            &eval,
            self.view().view_projection(),
            self.viewport(),
            self.mouse,
        ) else {
            self.status = "В этой точке нет поверхности. Выберите объект или нажмите Esc.".into();
            return true;
        };
        let Some(camera) = self.editor.project.camera_from_node(id, &eval.global) else {
            self.focus_pick = None;
            return true;
        };
        let distance = (hit.position - camera.eye).dot((camera.target - camera.eye).normalize());
        if !(0.01..=100000.).contains(&distance) {
            self.status = "Точка вне диапазона фокусировки.".into();
            return true;
        }
        let result = self.editor.transact("Фокус по поверхности", |p| {
            let lens = p.node_mut(id).unwrap().camera.as_mut().unwrap();
            lens.depth_of_field.focus_distance = distance;
            lens.depth_of_field.enabled = true;
        });
        if self.result(result).is_some() {
            self.status = format!("Фокус: {distance:.3} м");
            self.focus_pick = None;
        }
        self.update_dirty();
        true
    }
}
