//! An anchored, cancellable color transaction shared by all scene color fields.
use super::*;
use crate::color_picker::{ColorControl, ColorDraft, ColorLayout, ColorPicker};
use poseforge_core::AlphaMode;

#[derive(Clone, Copy, Debug)]
enum ColorTarget {
    Mesh,
    Bone(u64, bool),
    Material(u64, bool),
    Absorption(u64),
    Npr(u64, usize),
    Light(usize),
    Volume(usize),
    Environment(usize),
}

pub(super) struct ColorSession {
    pub(super) picker: ColorPicker,
    target: ColorTarget,
    source: Action,
    return_to_field: bool,
    selection: Option<u64>,
    anchor: [f32; 4],
    title: &'static str,
    original_rgb: [f32; 3],
    original_alpha: f32,
    original_alpha_mode: AlphaMode,
    original_absorption: Option<[f32; 3]>,
    original_bone_displays: Vec<(u64, Option<poseforge_core::BoneDisplay>)>,
    pub(super) apply_to_all: bool,
    was_modified: bool,
    opened: Instant,
}

#[derive(Clone)]
pub(super) struct ColorDrag {
    control: ColorControl,
    layout: ColorLayout,
    start: ColorDraft,
    start_point: [f32; 2],
    grabbed_marker: bool,
    track: Option<([f32; 4], SliderRange)>,
    pointer_anchor: f32,
    value_anchor: f32,
    last_pointer: f32,
    last_value: f32,
    fine: bool,
}

fn color_source(action: &Action) -> bool {
    matches!(
        action,
        Action::MeshColor | Action::SurfaceMaterialColor(_)
            | Action::SurfaceAttenuationColor
            | Action::SurfaceNprColor(_)
            | Action::SurfaceLightColor
            | Action::SurfaceLightVolumeColor
            | Action::RenderColor(0..=5)
            | Action::BoneDisplay(ui::BoneDisplayAction::BodyColor | ui::BoneDisplayAction::JointColor)
    )
}

impl App {
    pub(super) fn reset_color(&mut self, source: Action) {
        if self.scene_write_pending() || self.job.is_some() || self.editor.has_staged() { return; }
        if matches!(source, Action::SurfaceMaterialColor(_) | Action::SurfaceNprColor(_) | Action::SurfaceAttenuationColor)
            && !self.material_authoring_available() { return; }
        if matches!(source, Action::SurfaceLightColor | Action::SurfaceLightVolumeColor)
            && self.surface_light_index().is_none_or(|i| self.editor.project.light_locked(i)) { return; }
        if let Action::BoneDisplay(kind) = source {
            let action = if kind == ui::BoneDisplayAction::JointColor { ui::BoneDisplayAction::ResetJointColor }
                else { ui::BoneDisplayAction::ResetBodyColor };
            self.bone_display_action(action);
            return;
        }
        self.open_color_picker(source);
        let Some(target) = self.color_picker.as_ref().map(|s| s.target) else { return; };
        let material = poseforge_core::Material::default();
        let light = poseforge_core::Light::default();
        let render = poseforge_core::RenderSettings::default();
        match target {
            ColorTarget::Mesh => self.preferences.mesh_color = Preferences::default().mesh_color,
            ColorTarget::Material(id, emissive) => {
                let m = self.editor.project.materials.iter_mut().find(|m| m.id == id).unwrap();
                if emissive { m.emissive = material.emissive; }
                else { m.base_color[..3].copy_from_slice(&material.base_color[..3]); }
            }
            ColorTarget::Absorption(id) => self.editor.project.materials.iter_mut().find(|m| m.id == id).unwrap().attenuation_color = None,
            ColorTarget::Npr(id, field) => {
                let color = npr_controls::npr_color(&material, field).unwrap();
                npr_controls::set_npr_color(self.editor.project.materials.iter_mut().find(|m| m.id == id).unwrap(),field,color);
            }
            ColorTarget::Light(index) => self.editor.project.lights[index].color = light.color,
            ColorTarget::Volume(index) => self.editor.project.lights[index].volume.tint = light.volume.tint,
            ColorTarget::Environment(field) => match field {
                0 => self.editor.project.render_settings.sky_color = render.sky_color,
                1 => self.editor.project.render_settings.ground_color = render.ground_color,
                2 => self.editor.project.render_settings.background_color = render.background_color,
                4 => self.editor.project.render_settings.atmosphere.color = render.atmosphere.color,
                5 => self.editor.project.render_settings.npr_contours.color = render.npr_contours.color,
                _ => {}
            },
            ColorTarget::Bone(..) => { self.cancel_color_picker(); return; }
        }
        self.commit_color_picker(); self.ui = self.build_ui(); self.redraw();
    }
    pub(super) fn color_layout(&self) -> Option<ColorLayout> {
        let session = self.color_picker.as_ref()?;
        Some(ColorLayout::anchored(
            session.anchor,
            [self.width, (self.height - 32.).max(1.)],
            session.picker.draft.alpha_enabled(),
        ))
    }

    pub(super) fn open_color_picker(&mut self, source: Action) {
        if !color_source(&source) || self.native_picker.is_some() || self.scene_write_pending() {
            return;
        }
        if self.has_modal() || !self.resolve_input() {
            return;
        }
        let (target, original_rgb, alpha, alpha_mode, title) = match source {
            Action::MeshColor => (ColorTarget::Mesh,self.preferences.mesh_color,1.,AlphaMode::Opaque,"Цвет сетки"),
            Action::BoneDisplay(kind @ (ui::BoneDisplayAction::BodyColor | ui::BoneDisplayAction::JointColor)) => {
                if !self.bone_display_editable() { return; }
                let node = self.bone_display_node().unwrap();
                let display = node.bone_display.unwrap_or_default();
                let joint = kind == ui::BoneDisplayAction::JointColor;
                (ColorTarget::Bone(node.id,joint), if joint { display.joint_color } else { display.color },
                    1.,AlphaMode::Opaque,if joint { "Цвет сустава" } else { "Цвет кости" })
            }
            Action::SurfaceNprColor(field) => {
                let Some(id) = self.selected_material_id() else {
                    return;
                };
                let Some(rgb) = self
                    .editor
                    .project
                    .material(id)
                    .and_then(|m| npr_controls::npr_color(m, field))
                else {
                    return;
                };
                (
                    ColorTarget::Npr(id, field),
                    rgb,
                    1.,
                    AlphaMode::Opaque,
                    [
                        "NPR · цвет тени",
                        "NPR · цвет края",
                        "NPR · цвет контура",
                        "NPR · полутон",
                        "NPR · свет",
                    ][field],
                )
            }
            Action::SurfaceAttenuationColor => {
                let Some(id) = self.selected_material_id() else {
                    return;
                };
                let Some(m) = self.editor.project.material(id) else {
                    return;
                };
                (
                    ColorTarget::Absorption(id),
                    m.attenuation_color.unwrap_or([
                        m.base_color[0],
                        m.base_color[1],
                        m.base_color[2],
                    ]),
                    1.,
                    AlphaMode::Opaque,
                    "Цвет поглощения стекла",
                )
            }
            Action::SurfaceMaterialColor(emissive) => {
                let Some(id) = self.selected_material_id() else {
                    return;
                };
                let Some(material) = self.editor.project.material(id) else {
                    return;
                };
                (
                    ColorTarget::Material(id, emissive),
                    if emissive {
                        material.emissive
                    } else {
                        [
                            material.base_color[0],
                            material.base_color[1],
                            material.base_color[2],
                        ]
                    },
                    if emissive { 1. } else { material.base_color[3] },
                    material.alpha_mode,
                    if emissive {
                        "Цвет свечения"
                    } else {
                        "Цвет материала"
                    },
                )
            }
            Action::SurfaceLightVolumeColor => {
                let Some(index) = self.surface_light_index() else { return; };
                (ColorTarget::Volume(index), self.editor.project.lights[index].volume.tint,
                    1., AlphaMode::Opaque, "Оттенок объёмного света")
            }
            Action::SurfaceLightColor => {
                let Some(index) = self.surface_light_index() else {
                    return;
                };
                (
                    ColorTarget::Light(index),
                    self.editor.project.lights[index].color,
                    1.,
                    AlphaMode::Opaque,
                    "Цвет источника света",
                )
            }
            Action::RenderColor(field) => {
                let settings = self.editor.project.render_settings;
                let (rgb, title) = match field {
                    0 => (settings.sky_color, "Верх окружения"),
                    1 => (settings.ground_color, "Низ окружения"),
                    2 => (settings.background_color, "Цвет фона"),
                    4 => (settings.atmosphere.color, "Цвет атмосферы"),
                    5 => (settings.npr_contours.color, "Цвет экранного контура"),
                    _ => return,
                };
                (
                    ColorTarget::Environment(field),
                    rgb,
                    1.,
                    AlphaMode::Opaque,
                    title,
                )
            }
            _ => return,
        };
        if original_rgb.iter().any(|v| !v.is_finite()) || !alpha.is_finite() {
            self.status = "Невозможно редактировать нечисловой цвет.".into();
            return;
        }
        let anchor = self
            .ui
            .hits
            .iter()
            .rev()
            .find(|hit| hit.action == Action::OpenColorPicker(Box::new(source.clone())))
            .or_else(|| self.ui.hits.iter().rev().find(|hit| hit.action == source))
            .map(|hit| hit.rect)
            .unwrap_or([
                self.width - self.right_width() + 12.,
                220.,
                self.right_width() - 36.,
                30.,
            ]);
        let display = if matches!(target, ColorTarget::Bone(..) | ColorTarget::Mesh) {
            original_rgb
        } else if matches!(target, ColorTarget::Material(_, true)) {
            surfaces::emissive_tint(original_rgb)
                .map(surfaces::linear_to_srgb)
        } else {
            original_rgb.map(surfaces::linear_to_srgb)
        };
        let mut picker = ColorPicker::new(
            [display[0], display[1], display[2], alpha],
            matches!(target, ColorTarget::Material(_, false)),
        );
        if matches!(target, ColorTarget::Material(_, true)) {
            picker.notice = Some("Оттенок · сила свечения отдельно".into());
        } else if original_rgb.iter().any(|v| !(0. ..=1.).contains(v)) {
            picker.notice = Some("HDR сохранён · новый цвет — sRGB 0–1".into());
        }
        self.editor.playing = false;
        let original_absorption = match target {
            ColorTarget::Absorption(id) => self
                .editor
                .project
                .material(id)
                .and_then(|m| m.attenuation_color),
            _ => None,
        };
        let original_bone_displays = if matches!(target, ColorTarget::Bone(..)) {
            self.bone_display_ids().into_iter().map(|id| (id, self.editor.project.node(id).unwrap().bone_display)).collect()
        } else { Vec::new() };
        if !matches!(target, ColorTarget::Mesh) { self.editor.begin_edit(title); }
        self.color_picker = Some(ColorSession {
            picker,
            target,
            source,
            return_to_field: false,
            selection: self.editor.selected,
            anchor,
            title,
            original_rgb,
            original_alpha: alpha,
            original_alpha_mode: alpha_mode,
            original_absorption,
            original_bone_displays,
            apply_to_all: false,
            was_modified: self.editor.modified,
            opened: Instant::now(),
        });
        self.menu = None;
        self.surface.input_error = None;
        self.status =
            "Цвет: предпросмотр · Enter или вне панели — применить · Esc — отменить".into();
        // Input may arrive before the next repaint. Install popup hit ownership
        // now so Tab and clicks cannot reuse a covered inspector/canvas control.
        self.ui = self.build_ui();
    }

    fn color_target_valid(&self) -> bool {
        let Some(session) = &self.color_picker else {
            return false;
        };
        match session.target {
            ColorTarget::Mesh => true,
            ColorTarget::Bone(id,_) => self.editor.selected == Some(id) && self.bone_display_editable()
                && self.bone_display_ids() == session.original_bone_displays.iter().map(|(id,_)| *id).collect(),
            ColorTarget::Material(id, _)
            | ColorTarget::Absorption(id)
            | ColorTarget::Npr(id, _) => {
                self.editor.selected == session.selection && self.selected_material_id() == Some(id)
            }
            ColorTarget::Light(index) | ColorTarget::Volume(index) => self.surface_light_index() == Some(index),
            ColorTarget::Environment(field) => matches!(field, 0..=2 | 4..=5),
        }
    }

    /// Only validated scalar color values are previewed. The editor owns one
    /// snapshot for the entire popup, so moving a marker never clones a mesh.
    pub(super) fn preview_color(&mut self) -> Result<(), String> {
        if !self.color_target_valid() {
            return Err("Объект выбора цвета больше недоступен.".into());
        }
        let session = self.color_picker.as_ref().ok_or("Панель цвета закрыта.")?;
        let rgba = session.picker.draft.rgba();
        if rgba
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Цвет должен находиться в диапазоне sRGB 0–1.".into());
        }
        let rgb_changed = session.picker.draft.rgb_changed();
        let alpha_changed = session.picker.draft.alpha_changed();
        let rgb = if rgb_changed {
            [rgba[0], rgba[1], rgba[2]].map(surfaces::srgb_to_linear)
        } else {
            session.original_rgb
        };
        match session.target {
            ColorTarget::Mesh => { self.preferences.mesh_color = [rgba[0],rgba[1],rgba[2]]; }
            ColorTarget::Bone(_,joint) => {
                for (id, original) in &session.original_bone_displays {
                    let node = self.editor.project.node_mut(*id).ok_or("Кость удалена.")?;
                    let mut display = original.unwrap_or_default();
                    let color = [rgba[0],rgba[1],rgba[2]];
                    let previous = if joint { display.joint_color } else { display.color };
                    node.bone_display = if (rgb_changed || session.apply_to_all) && color != previous {
                        if joint { display.joint_color = color; } else { display.color = color; }
                        Some(display)
                    } else { *original };
                }
            }
            ColorTarget::Npr(id, field) => {
                let material = self
                    .editor
                    .project
                    .materials
                    .iter_mut()
                    .find(|m| m.id == id)
                    .ok_or("Материал удалён.")?;
                npr_controls::set_npr_color(material, field, rgb);
            }
            ColorTarget::Absorption(id) => {
                let material = self
                    .editor
                    .project
                    .materials
                    .iter_mut()
                    .find(|m| m.id == id)
                    .ok_or("Материал удалён.")?;
                material.attenuation_color = if rgb_changed {
                    Some(rgb)
                } else {
                    session.original_absorption
                };
            }
            ColorTarget::Material(id, emissive) => {
                let material = self
                    .editor
                    .project
                    .materials
                    .iter_mut()
                    .find(|m| m.id == id)
                    .ok_or("Материал удалён.")?;
                if emissive {
                    // Always preserve the opening HDR strength, including after
                    // passing through black during this same preview session.
                    material.emissive = session.original_rgb;
                    if rgb_changed {
                        surfaces::set_emissive_tint(material, rgb);
                    }
                } else {
                    let alpha = if alpha_changed {
                        rgba[3]
                    } else {
                        session.original_alpha
                    };
                    material.base_color = [rgb[0], rgb[1], rgb[2], alpha];
                    material.alpha_mode = session.original_alpha_mode;
                    if alpha_changed && alpha < 1. && material.alpha_mode == AlphaMode::Opaque {
                        material.alpha_mode = AlphaMode::Blend;
                    }
                }
            }
            ColorTarget::Volume(index) => { self.editor.project.lights[index].volume.tint = rgb; }
            ColorTarget::Light(index) => {
                self.editor.project.lights[index].color = rgb;
            }
            ColorTarget::Environment(field) => match field {
                0 => self.editor.project.render_settings.sky_color = rgb,
                1 => self.editor.project.render_settings.ground_color = rgb,
                2 => self.editor.project.render_settings.background_color = rgb,
                4 => self.editor.project.render_settings.atmosphere.color = rgb,
                5 => self.editor.project.render_settings.npr_contours.color = rgb,
                _ => unreachable!(),
            },
        }
        self.update_dirty();
        Ok(())
    }

    pub(super) fn color_confirm(&mut self, action: &Action, value: &str) -> Result<bool, String> {
        if let Action::BoneDisplay(kind @ (ui::BoneDisplayAction::BodyColor | ui::BoneDisplayAction::JointColor)) = action {
            if !self.bone_display_editable() { return Err("Кость недоступна для изменения".into()); }
            let color = surfaces::parse_color(value)?.map(surfaces::linear_to_srgb);
            self.edit_bone_displays("Цвет костей", |display| {
                let target = if *kind == ui::BoneDisplayAction::JointColor { &mut display.joint_color } else { &mut display.color };
                if target.iter().zip(color).any(|(a,b)| (*a-b).abs() >= 0.00001) { *target = color; }
            })?;
            return Ok(true);
        }
        let Action::ColorPicker(control) = action else {
            return Ok(false);
        };
        if !control.is_field() {
            return Ok(false);
        }
        let session = self.color_picker.as_mut().ok_or("Панель цвета закрыта.")?;
        let previous = session.picker.draft.clone();
        session.picker.draft.set_text(*control, value)?;
        if let Err(error) = self.preview_color() {
            if let Some(session) = &mut self.color_picker {
                session.picker.draft = previous;
            }
            return Err(error);
        }
        Ok(true)
    }

    pub(super) fn color_action(&mut self, control: ColorControl) {
        if self.color_picker.is_none() {
            return;
        }
        match control {
            ColorControl::Apply => {
                self.finish_color_with_focus(true);
            }
            ColorControl::Cancel => {
                self.finish_color_with_focus(false);
            }
            ColorControl::RestoreOriginal => {
                self.color_picker
                    .as_mut()
                    .unwrap()
                    .picker
                    .draft
                    .restore_original();
                if let Err(error) = self.preview_color() {
                    self.status = error;
                }
                self.surface.input_error = None;
            }
            ColorControl::Wheel | ColorControl::Sv => self.begin_color_drag(control, None),
            _ if control.is_field() => {
                let text = self
                    .color_picker
                    .as_ref()
                    .unwrap()
                    .picker
                    .draft
                    .text(control);
                self.input(Action::ColorPicker(control), text);
            }
            _ => {}
        }
    }

    pub(super) fn begin_color_slider(&mut self, control: ColorControl) {
        self.ui = self.build_ui();
        let slider = self.ui.hits.iter().rev().find_map(|hit| match &hit.action {
            Action::Slide(action, range, value)
                if **action == Action::ColorPicker(control)
                    && ui::contains(hit.rect, self.mouse) =>
            {
                Some((hit.rect, *range, *value as f32))
            }
            _ => None,
        });
        if let Some(slider) = slider {
            self.begin_color_drag(control, Some(slider));
        }
    }

    fn begin_color_drag(
        &mut self,
        control: ColorControl,
        track: Option<([f32; 4], SliderRange, f32)>,
    ) {
        let Some(layout) = self.color_layout() else {
            return;
        };
        let session = self.color_picker.as_ref().unwrap();
        let sv_marker = layout.sv_position(session.picker.draft.hsv());
        let on_sv_marker = control == ColorControl::Sv
            && (self.mouse[0] - sv_marker[0]).hypot(self.mouse[1] - sv_marker[1]) <= 6.;
        if track.is_none() && !on_sv_marker && layout.drag_at(self.mouse) != Some(control) {
            return;
        }
        let (pointer_anchor, value_anchor, on_thumb) = if let Some((_, _, value)) = track {
            // Numeric color bars share relative dragging and double-click entry.
            (self.mouse[0], value, true)
        } else {
            let marker = if control == ColorControl::Wheel {
                layout.hue_position(session.picker.draft.hsv().hue)
            } else {
                layout.sv_position(session.picker.draft.hsv())
            };
            (
                self.mouse[0],
                0.,
                (self.mouse[0] - marker[0]).hypot(self.mouse[1] - marker[1]) <= 6.,
            )
        };
        let drag = ColorDrag {
            control,
            layout,
            start: session.picker.draft.clone(),
            start_point: self.mouse,
            grabbed_marker: on_thumb,
            track: track.map(|(rect, range, _)| (rect, range)),
            pointer_anchor,
            value_anchor,
            last_pointer: self.mouse[0],
            last_value: value_anchor,
            fine: self.modifiers.shift_key(),
        };
        self.surface.input_error = None;
        self.drag_button = Some(MouseButton::Left);
        self.drag = Some(Drag::Color(drag.clone()));
        if !on_thumb {
            self.update_color_drag(drag, self.mouse);
        }
    }

    pub(super) fn update_color_drag(&mut self, mut drag: ColorDrag, point: [f32; 2]) {
        if point.iter().any(|v| !v.is_finite()) {
            return;
        }
        let Some(session) = &mut self.color_picker else {
            return;
        };
        let previous = session.picker.draft.clone();
        let changed = if drag.grabbed_marker
            && (point[0] - drag.start_point[0]).hypot(point[1] - drag.start_point[1]) < 0.01
        {
            session.picker.draft = drag.start.clone();
            true
        } else if let Some((rect, range)) = drag.track {
            let fine = self.modifiers.shift_key();
            if fine != drag.fine {
                drag.pointer_anchor = drag.last_pointer;
                drag.value_anchor = drag.last_value;
                drag.fine = fine;
            }
            let value = (drag.value_anchor as f64
                + (point[0] - drag.pointer_anchor) as f64 / rect[2].max(1.) as f64
                    * (range.max - range.min)
                    * if fine { 0.1 } else { 1. })
            .clamp(range.min, range.max) as f32;
            drag.last_pointer = point[0];
            drag.last_value = value;
            if drag
                .start
                .value(drag.control)
                .is_some_and(|initial| (value - initial).abs() < 1e-5)
            {
                session.picker.draft = drag.start.clone();
                true
            } else {
                session
                    .picker
                    .draft
                    .set_value(drag.control, value)
                    .unwrap_or(false)
            }
        } else if drag.grabbed_marker && drag.control == ColorControl::Sv {
            let marker = drag.layout.sv_position(drag.start.hsv());
            let adjusted = [
                marker[0] + point[0] - drag.start_point[0],
                marker[1] + point[1] - drag.start_point[1],
            ];
            session.picker.drag(drag.control, drag.layout, adjusted)
        } else if drag.grabbed_marker && drag.control == ColorControl::Wheel {
            let initial = drag.start.hsv().hue;
            let angle = drag.layout.hue_at(drag.start_point, initial);
            let previous = angle + session.picker.draft.hsv().hue - initial;
            let delta = drag.layout.hue_at(point, previous) - angle;
            session
                .picker
                .draft
                .set_value(ColorControl::Hue, (initial + delta).rem_euclid(360.))
                .unwrap_or(false)
        } else {
            session.picker.drag(drag.control, drag.layout, point)
        };
        if changed {
            if drag.track.is_some() && point[0] != drag.start_point[0] {
                self.slider_click = None;
            }
            if let Err(error) = self.preview_color() {
                self.color_picker.as_mut().unwrap().picker.draft = previous;
                self.status = error;
            }
        }
        self.drag = Some(Drag::Color(drag));
    }

    pub(super) fn restore_color_drag(&mut self, drag: &ColorDrag) {
        if let Some(session) = &mut self.color_picker {
            session.picker.draft = drag.start.clone();
            if let Err(error) = self.preview_color() {
                self.status = error;
            }
        }
    }

    fn release_color_pointer(&mut self) {
        if matches!(self.drag, Some(Drag::Color(_))) {
            self.drag = None;
            self.drag_button = None;
            if let Some(w) = &self.window {
                let _ = w.set_cursor_grab(winit::window::CursorGrabMode::None);
            }
        }
    }

    pub(super) fn commit_color_picker(&mut self) -> bool {
        if self.color_picker.is_none() || !self.resolve_input() {
            return false;
        }
        if !self.color_target_valid() {
            self.cancel_color_picker();
            self.status = "Выбор цвета отменён: объект изменился.".into();
            return false;
        }
        self.release_color_pointer();
        let session = self.color_picker.take().unwrap();
        let result = if matches!(session.target,ColorTarget::Mesh) {
                self.remember_mesh_color(session.original_rgb);
                self.persist_preferences(); Ok(()) }
            else { self.editor.commit_edit() };
        self.end_input();
        self.update_dirty();
        match result {
            Ok(()) => {
                self.status = if matches!(session.target, ColorTarget::Mesh) {
                    "Цвет сетки применён · ПКМ по цвету — отменить или вернуть".into()
                } else { format!("{} применён · Ctrl+Z — отменить", session.title) };
                true
            }
            Err(error) => {
                self.status = format!("Цвет не применён: {error}");
                false
            }
        }
    }

    pub(super) fn cancel_color_picker(&mut self) {
        let Some(session) = self.color_picker.take() else {
            return;
        };
        self.release_color_pointer();
        self.end_input();
        if matches!(session.target, ColorTarget::Mesh) { self.preferences.mesh_color=session.original_rgb; }
        else { self.editor.cancel_edit(); self.editor.modified = session.was_modified; }
        self.update_dirty();
        self.status = "Выбор цвета отменён.".into();
    }

    fn finish_color_with_focus(&mut self, apply: bool) -> bool {
        let source = self
            .color_picker
            .as_ref()
            .filter(|session| session.return_to_field && self.color_target_valid())
            .map(|session| session.source.clone());
        let closed = if apply {
            self.commit_color_picker()
        } else {
            self.cancel_color_picker();
            true
        };
        if closed {
            if let Some(source) = source {
                self.action(source);
                self.ui = self.build_ui();
            }
        }
        closed
    }

    /// Called before generic field/canvas routing. Dismissal is one consumed click.
    pub(super) fn color_pointer_down(&mut self, button: MouseButton) -> bool {
        let Some(layout) = self.color_layout() else {
            return false;
        };
        let exact = self.color_picker.as_ref().filter(|session| button == MouseButton::Left
            && session.opened.elapsed().as_millis() <= 450 && ui::contains(session.anchor, self.mouse)
            && !session.picker.draft.rgb_changed() && !session.picker.draft.alpha_changed())
            .map(|session| (session.source.clone(), session.picker.draft.rgba()));
        if let Some((source, rgba)) = exact {
            let rgb = [rgba[0],rgba[1],rgba[2]].map(|v| (v*255.).round() as u8);
            self.cancel_color_picker(); self.ui = self.build_ui();
            self.input(source, format!("#{:02X}{:02X}{:02X}",rgb[0],rgb[1],rgb[2]));
            return true;
        }
        if !ui::contains(layout.panel, self.mouse) {
            self.commit_color_picker();
            return true;
        }
        button != MouseButton::Left
    }

    pub(super) fn draw_color_picker(&mut self, u: &mut Ui) {
        if self.color_picker.is_some() && !self.color_target_valid() {
            self.cancel_color_picker();
        }
        if let Some(session) = &mut self.color_picker {
            if let Some(hit) =
                u.hits.iter().rev().find(|hit| {
                    hit.action == Action::OpenColorPicker(Box::new(session.source.clone()))
                })
            {
                session.anchor = hit.rect;
            }
        }
        let Some(layout) = self.color_layout() else {
            return;
        };
        u.hits.clear();
        let [x, y, w, h] = layout.panel.map(|v| v * u.scale);
        // Exclude scene strokes from the popup, including depth-tested mesh
        // overlays. Keep their original geometry and clip only their coverage.
        let previous=std::mem::take(&mut u.rects);
        for rect in previous {
            if (rect.segment.is_some() || rect.triangle.is_some()) && intersect_rect(rect.rect,[x,y,w,h]).is_some() {
                let [rx,ry,rw,rh]=rect.rect;
                for bounds in [[rx,ry,rw,(y-ry).max(0.)],[rx,(y+h).max(ry),rw,(ry+rh-(y+h).max(ry)).max(0.)],
                    [rx,ry.max(y),(x-rx).max(0.),(ry+rh).min(y+h)-ry.max(y)],
                    [(x+w).max(rx),ry.max(y),(rx+rw-(x+w).max(rx)).max(0.),(ry+rh).min(y+h)-ry.max(y)]] {
                    if let Some(clip)=intersect_rect(rect.rect,bounds) {let mut part=rect.clone();part.rect=clip;u.rects.push(part);}
                }
            } else {u.rects.push(rect);}
        }
        u.texts.retain(|text| {
            let width = text.text.chars().count() as f32 * text.size;
            text.x >= x + w
                || text.x + width <= x
                || text.y >= y + h
                || text.y + text.size * 1.5 <= y
        });
        let session = self.color_picker.as_mut().unwrap();
        session.picker.draw(
            u,
            layout,
            session.title,
            self.surface.input_error.as_deref(),
        );
    }

    /// Returns false only when normal text editing should receive this key.
    pub(super) fn color_key(&mut self, key: &Key, _ctrl: bool, shift: bool) -> bool {
        if self.color_picker.is_none() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Escape) => {
                self.finish_color_with_focus(false);
            }
            Key::Named(NamedKey::Enter) => {
                self.finish_color_with_focus(true);
            }
            Key::Named(NamedKey::Tab) => self.focus_next_field(shift),
            _ => return self.input.is_none(),
        }
        self.redraw();
        true
    }

    pub(super) fn color_open_key(&mut self, key: &Key) -> bool {
        if self.color_picker.is_some()
            || !self.modifiers.alt_key()
            || *key != Key::Named(NamedKey::ArrowDown)
        {
            return false;
        }
        let Some(source) = self
            .input
            .as_ref()
            .map(|i| i.purpose.clone())
            .filter(color_source)
        else {
            return false;
        };
        if self.resolve_input() {
            self.action(Action::OpenColorPicker(Box::new(source)));
            if let Some(session) = &mut self.color_picker {
                session.return_to_field = true;
            }
        }
        true
    }

    pub(super) fn action_with_color_picker(&mut self, action: &Action) -> bool {
        if self.color_picker.is_none() {
            return false;
        }
        if matches!(
            action,
            Action::Cancel | Action::ColorPicker(ColorControl::Cancel)
        ) {
            self.finish_color_with_focus(false);
            self.redraw();
            return true;
        }
        if matches!(action, Action::ColorPicker(_) | Action::ConfirmInput)
            || matches!(action, Action::Slide(inner, _, _) if matches!(**inner, Action::ColorPicker(_)))
            || matches!(action, Action::ResetNumber(inner) if matches!(**inner, Action::ColorPicker(_)))
        {
            return false;
        }
        // Selection, document and tool changes cancel before their own command.
        self.cancel_color_picker();
        false
    }
}
