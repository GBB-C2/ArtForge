//! Scene rows, precise parameters and one cancellable geometry worker.
use super::*;
use crate::ui::{DropdownKind, ModifierAction as M};
use poseforge_core::{EvaluatedScene, MeshModifier, ModifierKey, ModifierSettings as S};

type Completed = Vec<(u64, ModifierKey, Result<poseforge_core::Mesh, String>)>;
struct Worker {
    keys: Vec<(u64, ModifierKey)>,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Completed>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
#[derive(Default)]
pub(super) struct State {
    pub selected: Option<(u64, u64)>,
    ready: std::collections::BTreeMap<u64, (ModifierKey, Result<poseforge_core::Mesh, String>)>,
    worker: Option<Worker>,
}

impl App {
    pub(super) fn frame_modifier_selection(&mut self, selection: &BTreeSet<u64>) -> bool {
        if self.authoring.editing.is_some()
            || !selection.iter().any(|id| {
                self.editor
                    .project
                    .node(*id)
                    .is_some_and(|n| n.modifiers.iter().any(|m| m.enabled))
            })
        {
            return false;
        }
        let Ok(eval) = self.evaluated_interactive() else {
            return false;
        };
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for id in selection {
            let Some(model) = eval.global.get(id) else {
                continue;
            };
            let mesh = eval.modifier_meshes.get(id).or_else(|| {
                self.editor
                    .project
                    .node(*id)
                    .and_then(|n| n.mesh)
                    .and_then(|m| self.editor.project.mesh(m))
            });
            if let Some(mesh) = mesh {
                for v in &mesh.vertices {
                    let p = model.transform_point3(Vec3::from_array(v.position));
                    min = min.min(p);
                    max = max.max(p);
                }
            } else {
                min = min.min(model.w_axis.truncate());
                max = max.max(model.w_axis.truncate());
            }
        }
        if !min.is_finite() {
            return false;
        }
        self.leave_camera_view();
        let view = self.view();
        let aspect = (view.viewport[2] / view.viewport[3].max(1.)).max(0.01);
        let radius = ((max - min).length() * 0.5).max(0.05) * 1.15;
        self.target = (min + max) * 0.5;
        self.distance = if self.orthographic {
            radius / aspect.min(1.) / 0.42
        } else {
            radius
                / ((view.fov_y * 0.5).tan() * aspect.min(1.))
                    .atan()
                    .sin()
                    .max(0.001)
        };
        true
    }
    pub(super) fn evaluated_interactive(
        &self,
    ) -> Result<EvaluatedScene, poseforge_core::CoreError> {
        let mut eval = self.editor.evaluated_base()?;
        for node in self
            .editor
            .project
            .nodes
            .iter()
            .filter(|n| n.modifiers.iter().any(|m| m.enabled))
        {
            if self.authoring.editing == Some(node.id) && !self.authoring.preview_modifiers {
                continue;
            }
            let Ok(input) = self.editor.project.prepare_modifiers(node.id, &eval.global) else {
                continue;
            };
            let ready = self
                .procedural
                .ready
                .get(&node.id)
                .filter(|(key, _)| *key == input.key)
                .map(|(_, m)| m.clone())
                .or_else(|| input.cached());
            if let Some(Ok(mesh)) = ready {
                eval.modifier_meshes.insert(node.id, mesh);
            }
        }
        Ok(eval)
    }
    pub(super) fn tick_modifiers(&mut self) {
        let Ok(eval) = self.editor.evaluated_base() else {
            return;
        };
        let inputs: Vec<_> = self
            .editor
            .project
            .nodes
            .iter()
            .filter(|n| {
                n.modifiers.iter().any(|m| m.enabled)
                    && (self.authoring.editing != Some(n.id) || self.authoring.preview_modifiers)
            })
            .filter_map(|n| {
                self.editor
                    .project
                    .prepare_modifiers(n.id, &eval.global)
                    .ok()
                    .map(|input| (n.id, input))
            })
            .collect();
        self.procedural
            .ready
            .retain(|id, (key, _)| inputs.iter().any(|(n, input)| n == id && &input.key == key));
        let keys: Vec<_> = inputs
            .iter()
            .map(|(n, input)| (*n, input.key.clone()))
            .collect();
        if let Some(worker) = &self.procedural.worker {
            if worker.keys != keys {
                worker.cancel.store(true, Ordering::Relaxed);
            }
            match worker.receiver.try_recv() {
                Ok(results) => {
                    for (id, key, result) in results {
                        if keys.iter().any(|(n, k)| *n == id && *k == key) {
                            if let Err(error) = &result {
                                self.status = error.clone();
                            }
                            self.procedural.ready.insert(id, (key, result));
                        }
                    }
                    self.procedural.worker = None;
                    self.redraw();
                }
                Err(mpsc::TryRecvError::Disconnected) => self.procedural.worker = None,
                Err(mpsc::TryRecvError::Empty) => return,
            }
        }
        let missing: Vec<_> = inputs
            .into_iter()
            .filter(|(id, _)| !self.procedural.ready.contains_key(id))
            .collect();
        if missing.is_empty() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        let window = self.window.clone();
        std::thread::spawn(move || {
            let mut results = Vec::new();
            for (id, input) in missing {
                if token.load(Ordering::Relaxed) {
                    break;
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    input.evaluate(&token)
                }))
                .map_err(|_| "Не удалось рассчитать модификаторы".to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
                if token.load(Ordering::Relaxed) {
                    break;
                }
                results.push((id, input.key, result));
            }
            let _ = sender.send(results);
            if let Some(window) = window {
                window.request_redraw();
            }
        });
        self.procedural.worker = Some(Worker {
            keys,
            cancel,
            receiver,
        });
    }
    pub(super) fn modifier_editable(&self, node: u64) -> bool {
        self.editor.project.modifier_supported(node)
            && !self.editor.project.node_locked(node)
            && self.job.is_none()
            && self.native_picker.is_none()
            && !self.scene_write_pending()
            && !self.editor.has_staged()
            && !self.editor.playing
            && self.authoring.editing.is_none()
    }
    pub(super) fn selected_modifier(&self) -> Option<(u64, &MeshModifier)> {
        let (node, id) = self.procedural.selected?;
        if self.editor.selected != Some(node) {
            return None;
        }
        self.editor
            .project
            .node(node)?
            .modifiers
            .iter()
            .find(|m| m.id == id)
            .map(|m| (node, m))
    }
    pub(super) fn modifier_targets(&self) -> Vec<u64> {
        let Some((source, _)) = self.selected_modifier() else {
            return vec![];
        };
        self.editor
            .project
            .nodes
            .iter()
            .filter(|n| n.id != source && self.modifier_editable(n.id))
            .map(|n| n.id)
            .collect()
    }
    fn select_modifier(&mut self, node: u64, id: u64) {
        if !self
            .editor
            .project
            .node(node)
            .is_some_and(|n| n.modifiers.iter().any(|m| m.id == id))
        {
            return;
        }
        self.action(Action::Select(node));
        self.editor.selected = Some(node);
        self.scene_tools.selected = BTreeSet::from([node]);
        self.procedural.selected = Some((node, id));
        self.scene_tools.material_expanded.insert(node);
        self.collapsed.remove(&node);
        self.set_properties_tab(8);
        self.preferences.right_panel = true;
        self.inspector_scroll = 0.;
    }
    pub(super) fn modifier_action(&mut self, action: M) {
        if let M::Select(node, id) = action {
            self.select_modifier(node, id);
            return;
        }
        let owner = match action {
            M::Add(_) => self.editor.selected,
            M::Toggle(n, _)
            | M::Remove(n, _)
            | M::Field(n, _, _)
            | M::Axis(n, _, _)
            | M::Smooth(n, _)
            | M::Reset(n, _)
            | M::Step(n, _, _)
            | M::Duplicate(n, _)
            | M::Transfer(n, _, _, _)
            | M::ArrayMode(n, _, _)
            | M::Curve(n, _, _)
            | M::Follow(n, _)
            | M::Apply(n) => Some(n),
            M::Select(..) => None,
        };
        let Some(node) = owner.filter(|n| self.modifier_editable(*n)) else {
            return;
        };
        match action {
            M::Add(kind) => {
                let result = self.editor.add_modifier(node, kind);
                if let Some(id) = self.result(result) {
                    self.select_modifier(node, id);
                }
            }
            M::Remove(_, id) => {
                let result = self.editor.remove_modifier(node, id);
                if self.result(result).is_some() {
                    self.procedural.selected = self
                        .editor
                        .project
                        .node(node)
                        .and_then(|n| n.modifiers.first())
                        .map(|m| (node, m.id));
                }
            }
            M::Step(_, id, step) => {
                let stack = &self.editor.project.node(node).unwrap().modifiers;
                let Some(index) = stack.iter().position(|m| m.id == id) else {
                    return;
                };
                let next =
                    (index as isize + step as isize).clamp(0, stack.len() as isize - 1) as usize;
                if next == index {
                    return;
                }
                let before = stack
                    .get(if next > index { next + 1 } else { next })
                    .map(|m| m.id);
                let result = self.editor.transfer_modifier(node, id, node, before, false);
                self.result(result);
            }
            M::Duplicate(_, id) | M::Transfer(_, id, _, _) => {
                let (target, copy) = match action {
                    M::Transfer(_, _, target, copy) => (target, copy),
                    _ => (node, true),
                };
                if !self.modifier_editable(target) {
                    return;
                }
                let result = self.editor.transfer_modifier(node, id, target, None, copy);
                if let Some(id) = self.result(result) {
                    self.select_modifier(target, id);
                }
            }
            M::Apply(_) => {
                let Ok(eval) = self.editor.evaluated_base() else {
                    return;
                };
                let Ok(input) = self.editor.project.prepare_modifiers(node, &eval.global) else {
                    return;
                };
                let key = input.key.clone();
                let ready = self
                    .procedural
                    .ready
                    .get(&node)
                    .filter(|(k, _)| *k == key)
                    .map(|(_, m)| m.clone())
                    .or_else(|| input.cached());
                let Some(Ok(mesh)) = ready else {
                    self.status = "Дождитесь успешного расчёта стека перед применением".into();
                    return;
                };
                let result = self.editor.apply_evaluated_modifiers(node, &key, mesh);
                if self.result(result).is_some() {
                    self.procedural.selected = None;
                }
            }
            _ => {
                let id = match action {
                    M::Toggle(_, id)
                    | M::Field(_, id, _)
                    | M::Axis(_, id, _)
                    | M::Smooth(_, id)
                    | M::ArrayMode(_, id, _)
                    | M::Curve(_, id, _)
                    | M::Follow(_, id)
                    | M::Reset(_, id) => id,
                    _ => return,
                };
                let Some(mut modifier) = self
                    .editor
                    .project
                    .node(node)
                    .and_then(|n| n.modifiers.iter().find(|m| m.id == id))
                    .cloned()
                else {
                    return;
                };
                match action {
                    M::Field(_, _, field) => {
                        if self
                            .selected_modifier()
                            .is_some_and(|(n, m)| n == node && m.id == id)
                        {
                            self.input(Action::Modifier(action), field_value(&modifier, field));
                        }
                        return;
                    }
                    M::Toggle(..) => modifier.enabled = !modifier.enabled,
                    M::Axis(_, _, axis) => {
                        if let S::Mirror { axis: old, .. } = &mut modifier.settings {
                            *old = axis;
                        } else {
                            return;
                        }
                    }
                    M::Smooth(..) => {
                        if let S::Subdivision { smooth, .. } = &mut modifier.settings {
                            *smooth = !*smooth;
                        } else {
                            return;
                        }
                    }
                    M::ArrayMode(_, _, mode) => {
                        let count = match modifier.settings {
                            S::Array { count, .. }
                            | S::RadialArray { count, .. }
                            | S::CurveArray { count, .. } => count,
                            _ => return,
                        };
                        modifier.settings = match mode {
                            0 => S::Array {
                                count,
                                offset: [2., 0., 0.],
                            },
                            1 => S::RadialArray {
                                count,
                                axis: 1,
                                radius: 2.,
                                angle: std::f32::consts::TAU,
                                rotate: true,
                            },
                            2 => S::CurveArray {
                                count,
                                curve: None,
                                spacing: 2.,
                                start: 0.,
                                roll: 0.,
                                follow: true,
                            },
                            _ => return,
                        };
                    }
                    M::Curve(_, _, target) => {
                        if let S::CurveArray { curve, .. } = &mut modifier.settings {
                            *curve = target;
                        } else {
                            return;
                        }
                    }
                    M::Follow(..) => match &mut modifier.settings {
                        S::RadialArray { rotate, .. } => *rotate = !*rotate,
                        S::CurveArray { follow, .. } => *follow = !*follow,
                        _ => return,
                    },
                    M::Reset(..) => {
                        modifier.settings = match modifier.settings {
                            S::RadialArray { .. } => S::RadialArray {
                                count: 2,
                                axis: 1,
                                radius: 2.,
                                angle: std::f32::consts::TAU,
                                rotate: true,
                            },
                            S::CurveArray { curve, .. } => S::CurveArray {
                                count: 2,
                                curve,
                                spacing: 2.,
                                start: 0.,
                                roll: 0.,
                                follow: true,
                            },
                            _ => modifier.settings.kind().settings(),
                        }
                    }
                    _ => return,
                }
                let result = self.editor.update_modifier(node, modifier);
                self.result(result);
            }
        }
        self.redraw();
    }
    pub(super) fn modifier_confirm(
        &mut self,
        action: &Action,
        value: &str,
    ) -> Result<bool, String> {
        let Action::Modifier(M::Field(node, id, field)) = *action else {
            return Ok(false);
        };
        if !self.modifier_editable(node) {
            return Err("Объект недоступен для изменения".into());
        }
        let Some((_, m)) = self
            .selected_modifier()
            .filter(|(n, m)| *n == node && m.id == id)
        else {
            return Err("Выбор модификатора изменился".into());
        };
        let mut m = m.clone();
        if field == 0 {
            m.name = value.trim().to_owned();
            if m.name.chars().any(char::is_control) {
                return Err("Имя не должно содержать управляющие символы".into());
            }
        } else {
            let number = |min, max| surfaces::number(value, min, max);
            let integer = |max| -> Result<f32, String> {
                let n = number(1., max)?;
                if n.fract() != 0. {
                    Err("Введите целое число".into())
                } else {
                    Ok(n)
                }
            };
            match &mut m.settings {
                S::Mirror { plane, .. } if field == 1 => *plane = number(-10000., 10000.)?,
                S::Array { count, .. }
                | S::RadialArray { count, .. }
                | S::CurveArray { count, .. }
                    if field == 1 =>
                {
                    *count = integer(128.)? as u16
                }
                S::Array { offset, .. } if (2..=4).contains(&field) => {
                    offset[(field - 2) as usize] = number(-10000., 10000.)?
                }
                S::RadialArray { radius, .. } if field == 2 => *radius = number(0., 10000.)?,
                S::RadialArray { angle, .. } if field == 3 => {
                    *angle = number(-360., 360.)?.to_radians()
                }
                S::CurveArray { spacing, .. } if field == 2 => *spacing = number(0.000001, 10000.)?,
                S::CurveArray { start, .. } if field == 3 => *start = number(-10000., 10000.)?,
                S::CurveArray { roll, .. } if field == 4 => {
                    *roll = number(-360., 360.)?.to_radians()
                }
                S::Bevel { width, .. } if field == 1 => *width = number(0., 1000.)?,
                S::Bevel { segments, .. } if field == 2 => *segments = integer(32.)? as u32,
                S::Bevel { profile, .. } if field == 3 => *profile = number(-100., 100.)? / 100.,
                S::Bevel { angle, .. } if field == 4 => *angle = number(0., 180.)?.to_radians(),
                S::Subdivision { levels, .. } if field == 1 => *levels = integer(4.)? as u8,
                S::Solidify { thickness, .. } if field == 1 => *thickness = number(0., 10000.)?,
                S::Solidify { offset, .. } if field == 2 => *offset = number(-1., 1.)?,
                _ => return Err("Параметр модификатора изменился".into()),
            }
        }
        self.editor
            .update_modifier(node, m)
            .map_err(|e| e.to_string())?;
        self.update_dirty();
        Ok(true)
    }
    pub(super) fn modifier_panel_bottom(&self) -> f32 {
        980. + self
            .editor
            .selected
            .and_then(|n| self.editor.project.node(n))
            .map_or(0, |n| n.modifiers.len()) as f32
            * 28.
    }
    pub(super) fn draw_modifier_panel(&self, u: &mut Ui, x: f32, w: f32) {
        let p = u.palette();
        let x = x + 12.;
        let w = w - 34.;
        let mut y = 204.;
        let Some(node) = self
            .editor
            .selected
            .and_then(|id| self.editor.project.node(id))
            .filter(|n| n.mesh.is_some())
        else {
            u.fitted_label([x, y, w, 22.], "Выберите меш в сцене", 12., p.muted);
            return;
        };
        let enabled = self.modifier_editable(node.id);
        if self.authoring.editing.is_some() {
            u.quiet_button(
                [x, y, w, 30.],
                "Завершить правку сетки",
                Action::Author(ui::AuthorAction::ToggleEdit),
                false,
            );
            y += 38.;
            u.checkbox(
                [x, y, w, 28.],
                "Результат модификаторов",
                Action::Author(ui::AuthorAction::ModifierPreview),
                self.authoring.preview_modifiers,
            );
            y += 36.;
        }
        let button = |u: &mut Ui, r, label, action, active| {
            if enabled {
                u.quiet_button(r, label, action, active);
            } else {
                u.disabled_button(r, label);
            }
        };
        button(
            u,
            [x, y, w, 30.],
            "+ Добавить модификатор",
            Action::DropdownOpen(DropdownKind::ModifierAdd),
            false,
        );
        y += 40.;
        if !self.editor.project.modifier_supported(node.id) {
            u.fitted_label(
                [x, y, w, 20.],
                "Нужен обычный меш без skin и симуляции",
                11.,
                p.muted,
            );
            return;
        }
        u.fitted_label(
            [x, y, w, 18.],
            "Порядок сверху вниз меняет результат",
            11.,
            p.muted,
        );
        y += 24.;
        u.fitted_label(
            [x, y, w, 18.],
            if self.authoring.editing.is_some() {
                "Режим сетки: редактируется исходный меш"
            } else {
                "Перенос в сцене · Ctrl — копия на другой меш"
            },
            10.,
            p.muted,
        );
        y += 30.;
        if let Some((_, modifier)) = self.selected_modifier() {
            let id = modifier.id;
            if enabled {
                u.field(
                    [x, y, w, 28.],
                    &modifier.name,
                    Action::Modifier(M::Field(node.id, id, 0)),
                );
            } else {
                u.disabled_button([x, y, w, 28.], &modifier.name);
            }
            y += 36.;
            button(
                u,
                [x, y, w * 0.5 - 3., 28.],
                if modifier.enabled {
                    "Включён"
                } else {
                    "Выключен"
                },
                Action::Modifier(M::Toggle(node.id, id)),
                modifier.enabled,
            );
            button(
                u,
                [x + w * 0.5 + 3., y, w * 0.5 - 3., 28.],
                "Сброс параметров",
                Action::Modifier(M::Reset(node.id, id)),
                false,
            );
            y += 38.;
            if modifier.settings.kind() == poseforge_core::ModifierKind::Array {
                let mode = match modifier.settings {
                    S::RadialArray { .. } => 1,
                    S::CurveArray { .. } => 2,
                    _ => 0,
                };
                if enabled {
                    self.draw_dropdown_button(
                        u,
                        [x, y, w, 30.],
                        DropdownKind::ArrayMode(node.id, id),
                        ["Линейный массив", "По окружности", "Вдоль кривой"][mode],
                    );
                } else {
                    u.disabled_button(
                        [x, y, w, 30.],
                        ["Линейный массив", "По окружности", "Вдоль кривой"][mode],
                    );
                }
                y += 38.;
            }
            let fields: Vec<(&str, u8)> = match modifier.settings {
                S::Mirror { axis, .. } => {
                    for a in 0..3 {
                        button(
                            u,
                            [x + a as f32 * (w + 4.) / 3., y, (w - 8.) / 3., 26.],
                            ["X", "Y", "Z"][a],
                            Action::Modifier(M::Axis(node.id, id, a as u8)),
                            axis == a as u8,
                        );
                    }
                    y += 34.;
                    vec![("Плоскость зеркала", 1)]
                }
                S::Array { .. } => vec![
                    ("Копий, всего", 1),
                    ("Шаг X", 2),
                    ("Шаг Y", 3),
                    ("Шаг Z", 4),
                ],
                S::RadialArray { axis, rotate, .. } => {
                    for a in 0..3 {
                        button(
                            u,
                            [x + a as f32 * (w + 4.) / 3., y, (w - 8.) / 3., 26.],
                            ["X", "Y", "Z"][a],
                            Action::Modifier(M::Axis(node.id, id, a as u8)),
                            axis == a as u8,
                        );
                    }
                    y += 34.;
                    button(
                        u,
                        [x, y, w, 28.],
                        "Поворачивать копии",
                        Action::Modifier(M::Follow(node.id, id)),
                        rotate,
                    );
                    y += 36.;
                    vec![("Копий, всего", 1), ("Радиус, м", 2), ("Дуга, °", 3)]
                }
                S::CurveArray { curve, follow, .. } => {
                    let label = curve
                        .and_then(|id| self.editor.project.node(id))
                        .map_or("Выбрать кривую…", |n| n.name.as_str());
                    if enabled {
                        self.draw_dropdown_button(
                            u,
                            [x, y, w, 30.],
                            DropdownKind::ArrayCurve(node.id, id),
                            label,
                        );
                    } else {
                        u.disabled_button([x, y, w, 30.], label);
                    }
                    y += 38.;
                    button(
                        u,
                        [x, y, w, 28.],
                        "Ось X вдоль пути",
                        Action::Modifier(M::Follow(node.id, id)),
                        follow,
                    );
                    y += 36.;
                    vec![
                        ("Копий, всего", 1),
                        ("Шаг, м", 2),
                        ("Начало, м", 3),
                        ("Поворот / копию, °", 4),
                    ]
                }
                S::Bevel { .. } => vec![
                    ("Ширина, м", 1),
                    ("Сегменты", 2),
                    ("Профиль, %", 3),
                    ("Порог угла, °", 4),
                ],
                S::Subdivision { smooth, .. } => {
                    button(
                        u,
                        [x, y, w, 28.],
                        if smooth {
                            "Catmull–Clark · сглаживание"
                        } else {
                            "Простое подразделение"
                        },
                        Action::Modifier(M::Smooth(node.id, id)),
                        smooth,
                    );
                    y += 36.;
                    vec![("Уровни 1–4", 1)]
                }
                S::Solidify { .. } => vec![("Толщина", 1), ("Смещение −1…1", 2)],
            };
            for (label, field) in fields {
                u.fitted_label([x, y + 5., w * 0.51, 18.], label, 11., p.text);
                let r = [x + w * 0.54, y, w * 0.46, 28.];
                let value = field_value(modifier, field);
                if enabled {
                    u.field(r, &value, Action::Modifier(M::Field(node.id, id, field)));
                } else {
                    u.disabled_button(r, &value);
                }
                y += 36.;
            }
            y += 8.;
            for (i, (label, action)) in [
                ("Выше", M::Step(node.id, id, -1)),
                ("Ниже", M::Step(node.id, id, 1)),
                ("Копия", M::Duplicate(node.id, id)),
                ("Удалить", M::Remove(node.id, id)),
            ]
            .into_iter()
            .enumerate()
            {
                button(
                    u,
                    [x + i as f32 * (w + 4.) / 4., y, (w - 12.) / 4., 28.],
                    label,
                    Action::Modifier(action),
                    false,
                );
            }
            y += 36.;
            for (i, copy) in [false, true].into_iter().enumerate() {
                let r = [x + i as f32 * (w + 6.) / 2., y, (w - 6.) / 2., 28.];
                let label = if copy {
                    "Копировать в…"
                } else {
                    "Перенести в…"
                };
                if self.modifier_targets().is_empty() {
                    u.disabled_button(r, label);
                } else {
                    button(
                        u,
                        r,
                        label,
                        Action::DropdownOpen(DropdownKind::ModifierTarget(copy, node.id)),
                        false,
                    );
                }
            }
            y += 38.;
        } else {
            u.fitted_label(
                [x, y, w, 20.],
                "Выберите модификатор в списке",
                11.,
                p.muted,
            );
            y += 30.;
        }
        let status = if let Some((_, Err(error))) = self.procedural.ready.get(&node.id) {
            error.as_str()
        } else if self.procedural.worker.is_some() {
            "Расчёт геометрии…"
        } else {
            "Исходная геометрия сохранена"
        };
        u.fitted_label([x, y, w, 18.], status, 10., p.muted);
        y += 28.;
        u.fitted_label([x, y, w, 18.], "Стек объекта", 12., p.text);
        y += 24.;
        for (i, m) in node.modifiers.iter().enumerate() {
            let r = [x, y, w - 50., 26.];
            u.quiet_button(
                r,
                &format!("{}. {}", i + 1, m.name),
                Action::Modifier(M::Select(node.id, m.id)),
                self.procedural.selected == Some((node.id, m.id)),
            );
            if enabled {
                u.eye_button(
                    [x + w - 48., y, 24., 26.],
                    Action::Modifier(M::Toggle(node.id, m.id)),
                    m.enabled,
                );
                u.scene_close_button(
                    [x + w - 24., y, 24., 26.],
                    Action::Modifier(M::Remove(node.id, m.id)),
                );
            }
            y += 28.;
        }
        if !node.modifiers.is_empty() {
            y += 12.;
            button(
                u,
                [x, y, w, 28.],
                "Применить весь стек к мешу",
                Action::Modifier(M::Apply(node.id)),
                false,
            );
        }
    }
}
fn field_value(m: &MeshModifier, field: u8) -> String {
    if field == 0 {
        return m.name.clone();
    }
    let value = match m.settings {
        S::Mirror { plane, .. } => plane,
        S::Array { count, .. } | S::RadialArray { count, .. } | S::CurveArray { count, .. }
            if field == 1 =>
        {
            return count.to_string()
        }
        S::Array { offset, .. } => *offset.get(field.saturating_sub(2) as usize).unwrap_or(&0.),
        S::RadialArray { radius, angle, .. } => {
            if field == 2 {
                radius
            } else {
                angle.to_degrees()
            }
        }
        S::CurveArray {
            spacing,
            start,
            roll,
            ..
        } => match field {
            2 => spacing,
            3 => start,
            _ => roll.to_degrees(),
        },
        S::Bevel {
            width,
            segments,
            profile,
            angle,
        } => match field {
            1 => width,
            2 => return segments.to_string(),
            3 => profile * 100.,
            _ => angle.to_degrees(),
        },
        S::Subdivision { levels, .. } => return levels.to_string(),
        S::Solidify { thickness, offset } => {
            if field == 1 {
                thickness
            } else {
                offset
            }
        }
    };
    format!("{value:.4}")
}
