//! Direct panel editing, captured slider gestures and native path selection.
use super::*;
use crate::file_picker::{self, PickerFilter, PickerKind, PickerRequest};
use crate::ui::SliderRange;

pub(super) struct SliderClick {
    action: Action,
    point: [f32; 2],
    time: Instant,
}

#[derive(Clone)]
pub(super) struct SliderDrag {
    pub(super) action: Action,
    pub(super) range: SliderRange,
    rect: [f32; 4],
    pointer_anchor: f32,
    value_anchor: f64,
    last_pointer: f32,
    fine: bool,
    start_value: f64,
    last_value: f64,
    start_time: f64,
    transform: Option<Transform>,
}

#[derive(Clone, Copy, PartialEq)]
enum PickerTarget {
    Path(FilePurpose),
    Blender,
}
pub(super) struct NativePicker {
    target: PickerTarget,
    receiver: Receiver<Result<Option<PathBuf>, String>>,
}

fn clean_path(value: &str) -> String {
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(value);
    value.to_owned()
}

fn finite_number(value: &str) -> Result<f64, String> {
    value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| "Введите конечное число; дробная часть — через точку или запятую.".into())
}

impl App {
    pub(super) fn capture_drag_pointer(&mut self) {
        let Some(window)=&self.window else {return;};
        self.slider_cursor_locked=matches!(self.drag,Some(Drag::Slider(_)))
            && window.set_cursor_grab(winit::window::CursorGrabMode::Locked).is_ok();
        if self.slider_cursor_locked { window.set_cursor_visible(false); }
        else {let _=window.set_cursor_grab(winit::window::CursorGrabMode::Confined);}
    }
    pub(super) fn release_drag_pointer(&mut self) {
        self.slider_cursor_locked=false;
        if let Some(window)=&self.window {
            let _=window.set_cursor_grab(winit::window::CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
    }
    pub(super) fn rebase_slider_pointer(&mut self, x:f32) {
        if let Some(Drag::Slider(drag))=&mut self.drag {
            drag.pointer_anchor+=x-drag.last_pointer;
            drag.last_pointer=x;
            self.mouse[0]=x;
        }
    }
    pub(super) fn wrap_slider_pointer(&mut self) {
        if self.slider_cursor_locked || !matches!(self.drag,Some(Drag::Slider(_))) || self.width<16. {return;}
        let x=if self.mouse[0]<=2. {self.width-4.} else if self.mouse[0]>=self.width-2. {4.} else {return;};
        if self.window.as_ref().is_some_and(|w|w.set_cursor_position(winit::dpi::LogicalPosition::new(x,self.mouse[1])).is_ok()) {
            self.rebase_slider_pointer(x);
        }
    }
    /// A click only arms the bar. Two stationary clicks open its exact value;
    /// a changed value, another action or cancelled capture ends the sequence.
    pub(super) fn slider_double_click(&mut self, action: &Action) -> bool {
        let now = Instant::now();
        let double = self.slider_click.as_ref().is_some_and(|click| {
            click.action == *action
                && now.duration_since(click.time) < Duration::from_millis(350)
                && (self.mouse[0] - click.point[0]).hypot(self.mouse[1] - click.point[1]) < 5.
        });
        self.slider_click = if double {
            None
        } else {
            Some(SliderClick {
                action: action.clone(),
                point: self.mouse,
                time: now,
            })
        };
        double
    }

    pub(super) fn move_slider_click(&mut self, point: [f32; 2]) {
        if self.slider_click.as_ref().is_some_and(|click| {
            (point[0] - click.point[0]).hypot(point[1] - click.point[1]) >= 5.
        }) {
            self.slider_click = None;
        }
    }

    /// Ignore the small pointer jitter of a click. Shift deliberately bypasses
    /// this threshold so fine adjustments still retain subpixel precision.
    pub(super) fn defer_slider_drag(&mut self, point: [f32; 2]) -> bool {
        if self.drag.is_none() {
            return false;
        }
        let Some(click) = &self.slider_click else {
            return false;
        };
        if !self.modifiers.shift_key()
            && (point[0] - click.point[0]).hypot(point[1] - click.point[1]) < 3.
        {
            return true;
        }
        self.slider_click = None;
        false
    }

    /// Keep the draft in its owner. A failed commit never changes focus or selection.
    pub(super) fn confirm_input(&mut self) -> bool {
        let Some(input) = self.input.take() else {
            return true;
        };
        if !input.preedit.is_empty() {
            self.input = Some(input);
            return false;
        }
        let creates = matches!(
            input.purpose,
            Action::CreateClip
                | Action::DuplicateClip
                | Action::SavePose
                | Action::Author(crate::ui::AuthorAction::ExtendJoint)
        );
        if !creates && !input.changed() {
            self.end_input();
            return true;
        }
        let value=if input.purpose==Action::Text(ui::TextAction::Content) {input.value.as_str()} else {input.value.trim()};
        match self.apply_field_value(input.purpose.clone(), value) {
            Ok(()) => {
                self.end_input();
                self.prune_animation_selection();
                self.sync_fluid_settings();
                true
            }
            Err(error) => {
                self.status = error.clone();
                self.surface.input_error = Some(error);
                self.input = Some(input);
                false
            }
        }
    }

    /// Moving focus past an untouched creation field must not create an item.
    pub(super) fn resolve_input(&mut self) -> bool {
        if self.input.as_ref().is_some_and(|i| {
            i.preedit.is_empty()
                && !i.changed()
                && matches!(
                    i.purpose,
                    Action::CreateClip
                        | Action::DuplicateClip
                        | Action::SavePose
                        | Action::Author(crate::ui::AuthorAction::ExtendJoint)
                )
        }) {
            self.end_input();
            true
        } else {
            self.confirm_input()
        }
    }

    fn apply_transform_number(
        &mut self,
        index: usize,
        value: f64,
        base: Option<Transform>,
    ) -> Result<(), String> {
        if index >= 9 || !value.is_finite() || !(value as f32).is_finite() {
            return Err("Введите конечное число в диапазоне трансформации.".into());
        }
        let id = self.editor.selected.ok_or("Сначала выберите объект.")?;
        if !self.node_selectable(id) {
            return Err("Объект скрыт или заблокирован.".into());
        }
        let mut t = base
            .or_else(|| self.editing_transform(id))
            .ok_or("Трансформация недоступна.")?;
        let v = value as f32;
        if index < 3 {
            t.translation[index] = v;
        } else if index < 6 {
            let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
            let mut e = Vec3::new(x, y, z);
            e[index - 3] = v.to_radians();
            t.rotation = Quat::from_euler(EulerRot::XYZ, e.x, e.y, e.z);
        } else {
            if v < 0.001 {
                return Err("Масштаб должен быть не меньше 0.001.".into());
            }
            t.scale[index - 6] = v;
        }
        self.editor
            .set_transform(id, t)
            .map_err(|e| e.to_string())?;
        self.update_dirty();
        Ok(())
    }

    pub(super) fn apply_field_value(&mut self, action: Action, value: &str) -> Result<(), String> {
        if self.modifier_confirm(&action,value)? {return Ok(());}
        if action == Action::Collision(crate::ui::CollisionAction::Search) {
            self.collision_search_confirm(value); return Ok(());
        }
        if action == Action::AssetSearch {
            if value.chars().any(char::is_control) || value.chars().count() > 128 { return Err("Поиск: до 128 символов".into()); }
            self.assets.query = value.into(); return Ok(());
        }
        if self.path_confirm(&action, value)? { return Ok(()); }
        if self.helper_confirm(&action,value)? {return Ok(());}
        if self.fx_confirm(&action,value)? {return Ok(());}
        if self.text_confirm(&action, value)? {return Ok(());}
        if self.curve_confirm(&action, value)? { return Ok(()); }
        if self.destruction_confirm(&action, value)? { return Ok(()); }
        if self.fluid_object_confirm(&action, value)? {
            return Ok(());
        }
        if self.mesh_tools_confirm(&action, value)? {
            return Ok(());
        }
        if self.primitive_confirm(&action, value)? {
            return Ok(());
        }
        if self.node_confirm(&action, value)? {
            return Ok(());
        }
        if self.uv_confirm(&action, value)? {
            return Ok(());
        }
        if self.authoring_confirm(&action, value)? {
            return Ok(());
        }
        if self.studio_confirm(&action, value)? {
            return Ok(());
        }
        if self.collection_confirm(&action, value)? {
            return Ok(());
        }
        if self.saved_view_confirm(&action, value)? {
            return Ok(());
        }
        if self.camera_confirm(&action, value)? {
            return Ok(());
        }
        if self.color_confirm(&action, value)? {
            return Ok(());
        }
        if self.animation_confirm(&action, value)? {
            return Ok(());
        }
        if self.scene_settings_confirm(action.clone(), value)? {
            return Ok(());
        }
        if self.media_confirm(&action, value)? || self.morph_confirm(&action, value)? { return Ok(()); }
        if self.material_metadata_confirm(&action, value)? {
            return Ok(());
        }
        if self.surface_confirm(action.clone(), value)? {
            return Ok(());
        }
        match action {
            Action::MaterialSearch => {
                self.surface.material_query = value.chars().take(128).collect();
                self.surface.material_offset = 0;
            }
            Action::SearchScene => {
                self.scene_query = value.into();
                self.scene_scroll = 0;
            }
            Action::SearchBones => {
                self.bone_query = value.into();
                self.bone_scroll = 0;
            }
            Action::EditBlenderPath => {
                let value = clean_path(value);
                self.preferences.blender_path = if value.is_empty() {
                    None
                } else {
                    Some(
                        poseforge_io::locate_blender(Some(Path::new(&value)))
                            .map_err(|e| e.to_string())?,
                    )
                };
                self.persist_preferences();
                self.status = "Путь к Blender сохранён.".into();
            }
            Action::EditPath => {
                let value = clean_path(value);
                if value.is_empty() {
                    return Err("Укажите путь к файлу или папке.".into());
                }
                let dialog = self.dialog.as_mut().ok_or("Панель выбора файла закрыта.")?;
                dialog.path = value;
                dialog.refresh();
            }
            Action::CreateClip => {
                self.editor.create_clip(value).map_err(|e| e.to_string())?;
                self.editor.mode = EditorMode::Animation;
                self.status = format!("Создан клип: {value}");
            }
            Action::RenameClip => {
                self.editor
                    .rename_active_clip(value)
                    .map_err(|e| e.to_string())?;
            }
            Action::DuplicateClip => {
                self.editor
                    .duplicate_active_clip(value)
                    .map_err(|e| e.to_string())?;
                self.editor.mode = EditorMode::Animation;
                self.status = format!("Создана независимая копия: {value}");
            }
            Action::SavePose => {
                if value.is_empty() {
                    return Err("Введите название позы.".into());
                }
                self.editor.save_pose(value).map_err(|e| e.to_string())?;
                self.status = format!("Сохранена поза: {value}");
            }
            Action::EditTitle => {
                if value.is_empty() {
                    return Err("Название проекта не должно быть пустым.".into());
                }
                self.editor
                    .transact("Название проекта", |p| {
                        p.title = value.into()
                    })
                    .map_err(|e| e.to_string())?;
            }
            Action::EditName => {
                if value.is_empty() {
                    return Err("Имя объекта не должно быть пустым.".into());
                }
                let id = self.editor.selected.ok_or("Сначала выберите объект.")?;
                self.editor
                    .transact("Имя объекта", |p| {
                        if let Some(n) = p.node_mut(id) {
                            n.name = value.into();
                        }
                    })
                    .map_err(|e| e.to_string())?;
            }
            Action::EditTransform(index) => {
                self.apply_transform_number(index, finite_number(value)?, None)?
            }
            Action::EditFps => {
                let v = value
                    .parse::<u32>()
                    .ok()
                    .filter(|v| (1..=240).contains(v))
                    .ok_or("Частота кадров — целое число от 1 до 240.")?;
                self.editor
                    .transact("Частота кадров", |p| p.fps = v)
                    .map_err(|e| e.to_string())?;
            }
            Action::EditDuration => {
                let v = finite_number(value)?;
                if v < 0. {
                    return Err("Длительность не может быть отрицательной.".into());
                }
                self.editor
                    .transact("Длительность", |p| p.duration = v)
                    .map_err(|e| e.to_string())?;
                self.editor.time = self.editor.time.min(v);
            }
            Action::EditFrame => {
                let frame = value
                    .parse::<u32>()
                    .map_err(|_| "Номер кадра — целое неотрицательное число.")?;
                self.set_time(frame as f64 / self.editor.project.fps as f64);
            }
            Action::EditWidth | Action::EditHeight => {
                let v = value
                    .parse::<u32>()
                    .ok()
                    .filter(|v| (64..=4096).contains(v))
                    .ok_or("Размер изображения — целое число от 64 до 4096 пикселей.")?;
                self.editor
                    .transact("Размер PNG", |project| {
                        if action == Action::EditWidth {
                            project.render_settings.output_width = v;
                        } else {
                            project.render_settings.output_height = v;
                        }
                    })
                    .map_err(|error| error.to_string())?;
            }
            _ => return Err("Это поле больше недоступно.".into()),
        }
        self.update_dirty();
        Ok(())
    }

    pub(super) fn focus_next_field(&mut self, reverse: bool) {
        let current = self.input.as_ref().map(|input| input.purpose.clone());
        let mut fields = Vec::new();
        for hit in &self.ui.hits {
            if self.modeling_live.is_some()
                && !matches!(
                    hit.action,
                    Action::Author(crate::ui::AuthorAction::LiveField(_))
                )
            {
                continue;
            }
            if hit.action.is_field() && hit.rect[3] >= 22. && !fields.contains(&hit.action) {
                fields.push(hit.action.clone());
            }
        }
        if fields.is_empty() || !self.resolve_input() {
            return;
        }
        let index = current.and_then(|a| fields.iter().position(|v| *v == a));
        let next = match (index, reverse) {
            (Some(i), false) => (i + 1) % fields.len(),
            (Some(i), true) => (i + fields.len() - 1) % fields.len(),
            (None, false) => 0,
            (None, true) => fields.len() - 1,
        };
        self.action(fields[next].clone());
    }

    pub(super) fn active_field_rect(&self) -> Option<[f32; 4]> {
        let action = &self.input.as_ref()?.purpose;
        self.ui
            .hits
            .iter()
            .rev()
            .find(|h| &h.action == action && h.rect[3] >= 22.)
            .map(|h| h.rect)
    }

    pub(super) fn begin_slider(&mut self, action: Action, _range: SliderRange, _value: f64) {
        if self.native_picker.is_some() || self.scene_write_pending() {
            return;
        }
        if action == Action::EditFrame && self.editor.has_staged() {
            self.status = "Запишите или отбросьте изменённую позу перед сменой кадра.".into();
            return;
        }
        self.ui = self.build_ui();
        let Some((rect, range, value)) =
            self.ui.hits.iter().rev().find_map(|hit| match &hit.action {
                Action::Slide(field, live_range, live_value)
                    if **field == action && ui::contains(hit.rect, self.mouse) =>
                {
                    Some((hit.rect, *live_range, *live_value))
                }
                _ => None,
            })
        else {
            return;
        };
        self.editor.playing = false;
        if matches!(action, Action::FluidNumber(_)) {
            self.fluid_preview.cancel();
        }
        self.surface.input_error = None;
        let drag = SliderDrag {
            action,
            range,
            rect,
            pointer_anchor: self.mouse[0],
            value_anchor: value,
            last_pointer: self.mouse[0],
            fine: self.modifiers.shift_key(),
            start_value: value,
            last_value: value,
            start_time: self.editor.time,
            transform: self
                .editor
                .selected
                .and_then(|id| self.editing_transform(id)),
        };
        self.editor.begin_edit("Изменить параметр ползунком");
        self.drag = Some(Drag::Slider(drag));
        self.drag_button = Some(MouseButton::Left);
        // Relative dragging works from anywhere on the bar, including the value.
        // Pressing never jumps or rounds it before a possible double click.
    }

    pub(super) fn update_slider(&mut self, mut drag: SliderDrag, point: [f32; 2]) {
        if !point.iter().all(|v| v.is_finite()) {
            return;
        }
        let fine = self.modifiers.shift_key();
        if fine != drag.fine {
            drag.pointer_anchor = drag.last_pointer;
            drag.value_anchor = drag.last_value;
            drag.fine = fine;
        }
        // A vertical pointer move or modifier-only event must not quantize an
        // exact value merely because the thumb has capture.
        if point[0] == drag.last_pointer {
            self.drag = Some(Drag::Slider(drag));
            return;
        }
        let integer = matches!(
            drag.action,
            Action::EditFps
                | Action::EditWidth
                | Action::EditHeight
                | Action::EditFrame
                | Action::FluidNumber(4 | 5)
                | Action::Destruction(crate::ui::DestructionAction::Field(0 | 1 | 7))
                | Action::FluidObject(crate::ui::FluidObjectAction::Field(4 | 5))
                | Action::SurfaceMaterialNumber(19)
                | Action::Studio(crate::ui::StudioAction::Field(49 | 50))
                | Action::Primitive(crate::ui::PrimitiveAction::Field(3..=5))
                | Action::Author(crate::ui::AuthorAction::PaintRadius)
                | Action::Normal(crate::ui::NormalAction::Field(4))
        );
        let step = drag.range.step * if fine && !integer { 0.1 } else { 1. };
        let snapping = SliderRange::new(drag.range.min, drag.range.max, step);
        let span = drag.range.max - drag.range.min;
        let raw = drag.value_anchor
            + (point[0] - drag.pointer_anchor) as f64 / drag.rect[2].max(1.) as f64
                * span
                * if fine { 0.1 } else { 1. };
        let value = snapping.clamp_and_snap(raw);
        drag.last_pointer = point[0];
        if raw <= drag.range.min || raw >= drag.range.max {
            // Capture may continue far beyond the rail. Rebase at a limit so
            // reversing direction responds immediately instead of dead travel.
            drag.pointer_anchor = point[0];
            drag.value_anchor = value;
        }
        if value == drag.last_value {
            self.drag = Some(Drag::Slider(drag));
            return;
        }
        self.slider_click = None;
        let at_start = (value - drag.start_value).abs() <= step.max(1e-9) * 0.49;
        let result = if at_start {
            self.editor.cancel_edit();
            self.editor.time = drag.start_time;
            match drag.action {
                Action::Author(crate::ui::AuthorAction::PaintRadius) => {
                    self.weights.radius = drag.start_value as f32
                }
                Action::Author(crate::ui::AuthorAction::PaintStrength) => {
                    self.weights.strength = drag.start_value as f32
                }
                Action::Studio(crate::ui::StudioAction::Field(50)) => {
                    self.studio.bake_frames = drag.start_value as usize
                }
                Action::Normal(crate::ui::NormalAction::Field(_)) => {
                    let _ = self.mesh_tools_confirm(&drag.action, &drag.start_value.to_string());
                }
                _ => {}
            }

            self.editor.begin_edit("Изменить параметр ползунком");
            Ok(())
        } else if let Action::EditTransform(index) = drag.action {
            self.apply_transform_number(index, value, drag.transform)
        } else {
            let value = if integer {
                format!("{value:.0}")
            } else {
                format!("{value:.8}")
            };
            self.apply_field_value(drag.action.clone(), &value)
        };
        match result {
            Ok(()) => {
                drag.last_value = if at_start { drag.start_value } else { value };
                self.status = format!(
                    "{} · {} · Esc — отменить",
                    ui::number_caption(drag.last_value),
                    if fine {
                        "Точно ×0,1"
                    } else {
                        "Shift — точнее"
                    }
                );
            }
            Err(error) => self.status = error,
        }
        self.drag = Some(Drag::Slider(drag));
        self.update_dirty();
    }

    pub(super) fn restore_slider_preferences(&mut self, drag: &SliderDrag) {
        self.editor.time = drag.start_time;
        match drag.action {
            Action::Author(crate::ui::AuthorAction::PaintRadius) => {
                self.weights.radius = drag.start_value as f32
            }
            Action::Author(crate::ui::AuthorAction::PaintStrength) => {
                self.weights.strength = drag.start_value as f32
            }
            Action::Studio(crate::ui::StudioAction::Field(50)) => {
                self.studio.bake_frames = drag.start_value as usize
            }
            Action::Normal(crate::ui::NormalAction::Field(_)) => {
                let _ = self.mesh_tools_confirm(&drag.action, &drag.start_value.to_string());
            }
            _ => {}
        }
    }

    fn replace_path_draft(&mut self, purpose: Action, value: &str) {
        if !self.input.as_ref().is_some_and(|i| i.purpose == purpose) {
            let previous = if purpose == Action::EditBlenderPath {
                self.preferences
                    .blender_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                self.dialog
                    .as_ref()
                    .map(|d| d.path.clone())
                    .unwrap_or_default()
            };
            self.input(purpose, previous);
        }
        self.surface.input_error = None;
        if let Some(input) = &mut self.input {
            input.select_all();
            input.insert(value);
        }
    }

    pub(super) fn paste_path(&mut self, blender: bool) {
        let text = self
            .clipboard
            .as_mut()
            .ok_or("Буфер обмена недоступен.")
            .and_then(|clipboard| {
                clipboard
                    .get_text()
                    .map_err(|_| "В буфере обмена нет текста.")
            });
        match text {
            Ok(text) => {
                let purpose = if blender {
                    Action::EditBlenderPath
                } else {
                    Action::EditPath
                };
                self.replace_path_draft(purpose, &clean_path(&text));
            }
            Err(error) => self.status = error.into(),
        }
    }

    pub(super) fn browse_path(&mut self, blender: bool) {
        if self.native_picker.is_some() {
            return;
        }
        let (target, kind, title, extensions, initial) = if blender {
            (
                PickerTarget::Blender,
                PickerKind::OpenFile,
                "Выберите Blender",
                vec!["exe"],
                self.preferences.blender_path.clone(),
            )
        } else {
            let Some(d) = &self.dialog else {
                return;
            };
            let (kind, title, extensions) = match d.purpose {
                FilePurpose::CacheSave => (
                    PickerKind::SaveFile,
                    "Сохранить кэш жидкости",
                    vec!["afcache"],
                ),
                FilePurpose::CacheLoad => (
                    PickerKind::OpenFile,
                    "Загрузить кэш жидкости",
                    vec!["afcache"],
                ),
                FilePurpose::Environment => (
                    PickerKind::OpenFile,
                    "Панорама HDRI",
                    vec!["hdr", "png", "jpg", "jpeg"],
                ),
                FilePurpose::Import => (
                    PickerKind::OpenFile,
                    "Импорт модели",
                    vec!["glb", "gltf", "obj"],
                ),
                FilePurpose::Texture => {
                    (PickerKind::OpenFile, "Изображение или видео", vec!["png", "jpg", "jpeg", "tga", "bmp", "dds", "mp4", "m4v", "mov", "webm", "mkv", "avi", "ogv", "gif"])
                }
                FilePurpose::Rig => (PickerKind::OpenFile, "Список костей", vec!["txt"]),
                FilePurpose::Open => (
                    PickerKind::OpenFile,
                    "Открыть проект",
                    vec!["artforge", "poseforge", "json"],
                ),
                FilePurpose::Save => (
                    PickerKind::SaveFile,
                    "Сохранить проект",
                    vec!["artforge", "poseforge"],
                ),
                FilePurpose::Export => (PickerKind::SaveFile, "Экспорт PNG", vec!["png"]),
                FilePurpose::Sequence => {
                    (PickerKind::Folder, "Папка последовательности PNG", vec![])
                }
            };
            (
                PickerTarget::Path(d.purpose),
                kind,
                title,
                extensions,
                Some(PathBuf::from(&d.path)),
            )
        };
        let field = if blender {
            Action::EditBlenderPath
        } else {
            Action::EditPath
        };
        let initial = self
            .input
            .as_ref()
            .filter(|i| i.purpose == field)
            .map(|i| PathBuf::from(clean_path(&i.value)))
            .or(initial);
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let owner = self
            .window
            .as_ref()
            .and_then(|window| window.window_handle().ok())
            .and_then(|h| match h.as_raw() {
                RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
                _ => None,
            });
        let filters = if extensions.is_empty() {
            Vec::new()
        } else {
            vec![PickerFilter {
                name: title.into(),
                extensions: extensions.into_iter().map(String::from).collect(),
            }]
        };
        let request = PickerRequest {
            kind,
            title: title.into(),
            initial_path: initial,
            filters,
            owner,
        };
        let (sender, receiver) = mpsc::channel();
        self.native_picker = Some(NativePicker { target, receiver });
        self.editor.playing = false;
        self.text_selecting = false;
        self.status = "Выберите путь в системном окне; после выбора нажмите «Продолжить».".into();
        let owner_window = self.window.clone();
        std::thread::spawn(move || {
            let _owner_window = owner_window;
            let _ = sender.send(file_picker::pick(request));
        });
    }

    pub(super) fn poll_picker(&mut self) {
        let Some(picker) = &self.native_picker else {
            return;
        };
        let result = match picker.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Окно выбора пути закрылось без результата.".into())
            }
        };
        let target = picker.target;
        self.native_picker = None;
        match result {
            Ok(Some(path)) => {
                if matches!(target, PickerTarget::Path(p) if !self.dialog.as_ref().is_some_and(|d| d.purpose == p))
                {
                    return;
                }
                let field = if target == PickerTarget::Blender {
                    Action::EditBlenderPath
                } else {
                    Action::EditPath
                };
                self.replace_path_draft(field, &path.to_string_lossy());
                self.status = "Путь выбран. Enter — применить, Esc — вернуть прежний путь.".into();
            }
            Ok(None) => self.status = "Выбор пути отменён.".into(),
            Err(error) => self.status = error,
        }
        self.redraw();
    }
}

#[cfg(test)]
mod slider_click_tests {
    use super::*;

    #[test]
    fn double_click_requires_the_same_field_position_and_time_window() {
        let mut app = App::new(None);
        let field = Action::EditTransform(0);
        app.mouse = [100., 100.];
        assert!(!app.slider_double_click(&field));
        assert!(app.slider_double_click(&field));
        assert!(app.slider_click.is_none());
        assert!(!app.slider_double_click(&field));
        assert!(!app.slider_double_click(&Action::EditTransform(1)));
        app.mouse[0] += 10.;
        assert!(!app.slider_double_click(&Action::EditTransform(1)));
        app.slider_click = Some(SliderClick {
            action: field.clone(),
            point: app.mouse,
            time: Instant::now() - Duration::from_secs(1),
        });
        assert!(!app.slider_double_click(&field));
    }
}
