//! Scene-camera tools reuse ordinary node selection, transforms and history.
use super::{App, EditorMode, EulerRot, Mat4, Tool, Vec3};
use crate::{
    icons::Icon,
    ui::{Action, SliderRange, Ui},
};
use glam::{Vec2, Vec4};
use poseforge_core::{camera_transform, Camera, CameraLens, Node, Project, Transform};
use poseforge_render::View;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const CAMERA_PANEL_BOTTOM: f32 = 1580.;

fn frame_rect(viewport: [f32; 4], aspect: f32) -> [f32; 4] {
    let width = viewport[2].min(viewport[3] * aspect);
    let height = viewport[3].min(viewport[2] / aspect);
    [
        viewport[0] + (viewport[2] - width) * 0.5,
        viewport[1] + (viewport[3] - height) * 0.5,
        width,
        height,
    ]
}

/// Shared by the GUI export loop and tests; each frame gets evaluated globals.
pub(super) fn export_camera_view(
    project: &Project,
    globals: &BTreeMap<u64, Mat4>,
    fallback: &View,
) -> View {
    let Some(camera) = project
        .active_camera
        .and_then(|id| project.camera_from_node(id, globals))
    else {
        return fallback.clone();
    };
    view_from_camera(&camera, fallback.viewport)
}

pub(super) fn view_from_camera(camera: &Camera, viewport: [f32; 4]) -> View {
    View {
        effects: camera.effects,
        eye: camera.eye,
        target: camera.target,
        up: camera.up,
        fov_y: camera.fov_y_radians,
        viewport,
        orthographic: camera.orthographic.then_some(camera.ortho_size),
        depth_of_field: camera.depth_of_field,
    }
}

fn visible_camera(project: &Project, id: u64) -> bool {
    if !project.node_visible(id, false) {
        return false;
    }
    let mut current = Some(id);
    let mut seen = BTreeSet::new();
    while let Some(id) = current {
        if !seen.insert(id) {
            return false;
        }
        let Some(node) = project.node(id) else {
            return false;
        };
        if !node.visible {
            return false;
        }
        current = node.parent;
    }
    true
}

/// Homogeneous clipping keeps frustum wires inside the actual viewport, even
/// when a camera crosses the near plane or lies partly off screen.
pub(super) fn screen_segment(
    view: &View,
    viewport: [f32; 4],
    a: Vec3,
    b: Vec3,
) -> Option<([f32; 2], [f32; 2])> {
    let matrix = view.view_projection();
    let mut a = matrix * a.extend(1.);
    let mut b = matrix * b.extend(1.);
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    for plane in [
        Vec4::new(1., 0., 0., 1.),
        Vec4::new(-1., 0., 0., 1.),
        Vec4::new(0., 1., 0., 1.),
        Vec4::new(0., -1., 0., 1.),
        Vec4::new(0., 0., 1., 0.),
        Vec4::new(0., 0., -1., 1.),
    ] {
        let da = plane.dot(a);
        let db = plane.dot(b);
        if da < 0. && db < 0. {
            return None;
        }
        if (da < 0.) != (db < 0.) {
            let point = a + (b - a) * (da / (da - db));
            if da < 0. {
                a = point;
            } else {
                b = point;
            }
        }
    }
    if a.w <= 1e-6 || b.w <= 1e-6 {
        return None;
    }
    let project = |p: Vec4| {
        let p = p.truncate() / p.w;
        [
            (viewport[0] + (p.x + 1.) * viewport[2] * 0.5)
                .clamp(viewport[0], viewport[0] + viewport[2]),
            (viewport[1] + (1. - p.y) * viewport[3] * 0.5)
                .clamp(viewport[1], viewport[1] + viewport[3]),
        ]
    };
    Some((project(a), project(b)))
}

fn frustum_segments(camera: &Camera, aspect: f32, depth: f32) -> Vec<(Vec3, Vec3)> {
    let forward = (camera.target - camera.eye).normalize();
    let right = forward.cross(camera.up).normalize();
    let up = right.cross(forward).normalize();
    let half_height = if camera.orthographic {
        camera.ortho_size.min(depth * 3.)
    } else {
        depth * (camera.fov_y_radians * 0.5).tan()
    };
    let half_width = half_height * aspect;
    let center = camera.eye + forward * depth;
    let corners = [
        center - right * half_width - up * half_height,
        center + right * half_width - up * half_height,
        center + right * half_width + up * half_height,
        center - right * half_width + up * half_height,
    ];
    let mut lines = Vec::with_capacity(12);
    for i in 0..4 {
        lines.push((corners[i], corners[(i + 1) % 4]));
        let origin = if camera.orthographic {
            corners[i] - forward * depth
        } else {
            camera.eye
        };
        lines.push((origin, corners[i]));
    }
    // The upper triangular tab makes camera roll and its local up explicit.
    lines.push((corners[2], center + up * half_height * 1.35));
    lines.push((center + up * half_height * 1.35, corners[3]));
    lines
}

impl App {
    pub(super) fn selected_camera_id(&self) -> Option<u64> {
        self.editor.selected.filter(|id| {
            self.editor
                .project
                .node(*id)
                .is_some_and(|n| n.camera.is_some())
        })
    }
    pub(super) fn camera_object_view(&self) -> Option<View> {
        let id = self.camera_view?;
        let globals = self.evaluated_interactive().ok()?.global;
        let camera = self.editor.project.camera_from_node(id, &globals)?;
        let viewport = self.viewport().map(|v| v * self.scale);
        let aspect = self.editor.project.render_settings.output_width as f32
            / self.editor.project.render_settings.output_height.max(1) as f32;
        let frame = frame_rect(viewport, aspect);
        let expansion = viewport[3] / frame[3].max(1.);
        let mut view = view_from_camera(&camera, viewport);
        view.fov_y = (2. * ((camera.fov_y_radians * 0.5).tan() * expansion).atan())
            .min(std::f32::consts::PI - 1e-5);
        view.orthographic = view.orthographic.map(|half_height| half_height * expansion);
        Some(view)
    }
    /// Free navigation detaches the view without moving the camera object.
    pub(super) fn leave_camera_view(&mut self) {
        let Some(view) = self.camera_object_view().or_else(|| {
            self.recalled_view
                .as_ref()
                .map(|c| view_from_camera(c, self.viewport().map(|v| v * self.scale)))
        }) else {
            self.camera_view = None;
            return;
        };
        self.camera_view = None;
        self.recalled_view = None;
        self.last_saved_view = None;
        self.free_view_fov = Some(view.fov_y);
        self.target = view.target;
        let delta = view.eye - view.target;
        self.distance = delta.length().max(0.1);
        let direction = delta / self.distance;
        self.yaw = direction.x.atan2(direction.z);
        self.pitch = direction.y.clamp(-1., 1.).asin().clamp(-1.56, 1.56);
        self.orthographic = view.orthographic.is_some();
        if let Some(half_height) = view.orthographic {
            self.distance = half_height / 0.42;
        }
    }
    pub(super) fn camera_action(&mut self, action: Action) {
        if action == Action::CameraExitView {
            self.leave_camera_view();
            self.status = "Свободный вид. Камера сцены не перемещена.".into();
            return;
        }
        if action == Action::CameraClearActive {
            let result =
                self.editor.transact("Камера вывода: текущий вид", |p| {
                    p.active_camera = None
                });
            if self.result(result).is_some() {
                self.leave_camera_view();
            }
            return;
        }
        let create_for_view = action == Action::CameraView
            && !self.editor.project.nodes.iter().any(|n| n.camera.is_some());
        if action == Action::CreateCamera || create_for_view {
            if self.editor.has_staged() {
                self.status = "Сначала поставьте ключ или отбросьте временную позу.".into();
                return;
            }
            self.leave_camera_view();
            let view = self.view();
            let (position, direction) = if create_for_view {
                (view.eye, (view.target - view.eye).normalize_or_zero())
            } else {
                super::light_controls::placement_in_view(&view)
            };
            let camera = Camera {
                eye: position,
                target: position + direction,
                up: if create_for_view { view.up } else { Vec3::Y },
                fov_y_radians: view.fov_y.clamp(1_f32.to_radians(), 179_f32.to_radians()),
                orthographic: view.orthographic.is_some(),
                ortho_size: view.orthographic.unwrap_or(3.),
                ..Camera::default()
            };
            let id = self.editor.project.next_id();
            if id == u64::MAX {
                self.status = "Исчерпано пространство ID объектов.".into();
                return;
            }
            let count = self
                .editor
                .project
                .nodes
                .iter()
                .filter(|n| n.camera.is_some())
                .count();
            let mut node = Node::new(id, format!("Камера {}", count + 1));
            node.transform = camera_transform(&camera);
            node.camera = Some(CameraLens::from_camera(&camera));
            let result = self.editor.transact("Добавить камеру", |p| {
                p.nodes.push(node);
                if p.active_camera.is_none() {
                    p.active_camera = Some(id);
                }
            });
            if self.result(result).is_some() {
                self.clear_light_selection();
                self.editor.selected = Some(id);
                self.editor.mode = EditorMode::Scene;
                self.editor.playing = false;
                self.tool = Tool::Move;
                self.set_properties_tab(0);
                self.inspector_scroll = 0.;
                self.preferences.left_panel = true;
                self.preferences.right_panel = true;
                self.scene_query.clear();
                self.joints_only = false;
                self.scene_scroll = self.scene_node_offset(id);
                self.status="Камера добавлена. G — переместить, R — повернуть; «Вид из камеры» — посмотреть кадр.".into();
                if create_for_view {
                    self.camera_view = Some(id);
                    self.tool = Tool::Orbit;
                    self.status = "Камера создана из текущего вида. Вы смотрите через неё.".into();
                }
            }
            return;
        }
        let Some(id) = self.selected_camera_id().or_else(|| {
            if action == Action::CameraView {
                self.camera_view
                    .or(self.editor.project.active_camera)
                    .or_else(|| {
                        self.editor
                            .project
                            .nodes
                            .iter()
                            .find(|n| n.camera.is_some())
                            .map(|n| n.id)
                    })
            } else {
                None
            }
        }) else {
            self.status = "Выберите камеру в сцене.".into();
            return;
        };
        match action {
            Action::CameraView => {
                if self.camera_view == Some(id) {
                    self.leave_camera_view();
                    self.status = "Свободный вид. Камера сцены не перемещена.".into();
                    return;
                }
                self.editor.playing = false;
                self.recalled_view = None;
                self.camera_view = Some(id);
                self.tool = Tool::Orbit;
                self.status = "Вид из камеры. Замок связывает навигацию с её положением.".into();
            }
            Action::CameraSetActive => {
                let result = self.editor.transact("Камера для рендера", |p| {
                    p.active_camera = Some(id)
                });
                self.result(result);
            }
            Action::CameraProjection => {
                if !self.node_selectable(id) {
                    self.status = "Камера скрыта или заблокирована.".into();
                    return;
                }
                let result = self.editor.transact("Проекция камеры", |p| {
                    if let Some(lens) = p.node_mut(id).and_then(|n| n.camera.as_mut()) {
                        lens.orthographic = !lens.orthographic;
                    }
                });
                self.result(result);
            }
            Action::CameraNumber(field) => {
                if let Some(lens) = self.editor.project.node(id).and_then(|n| n.camera) {
                    let value = match field {
                        0 => lens.fov_y_radians.to_degrees(),
                        1 => lens.ortho_size,
                        2 => lens.depth_of_field.focus_distance,
                        3 => lens.depth_of_field.f_stop,
                        _ => { let Some((value, _, _, _)) = lens.effects.field(field) else { return; }; value },
                    };
                    self.input(action, format!("{value:.5}"));
                }
            }
            Action::CameraAlignView => {
                if !self.node_selectable(id) {
                    self.status = "Камера скрыта или заблокирована.".into();
                    return;
                }
                let view = self.view();
                let Ok(eval) = self.evaluated_interactive() else {
                    return;
                };
                let camera = self
                    .camera_view
                    .and_then(|camera_id| {
                        self.editor
                            .project
                            .camera_from_node(camera_id, &eval.global)
                    })
                    .unwrap_or_else(|| Camera {
                        eye: view.eye,
                        target: view.target,
                        up: view.up,
                        fov_y_radians: view.fov_y.clamp(1_f32.to_radians(), 179_f32.to_radians()),
                        orthographic: view.orthographic.is_some(),
                        ortho_size: view.orthographic.unwrap_or(3.),
                        ..Camera::default()
                    });
                let Ok(parent) = self.editor.project.transform_frame(id, &eval.global, self.editor.time) else { return; };
                if crate::gizmo::uniform_parent_rotation(parent).is_none() {
                    self.status =
                        "Выравнивание камеры требует родителя с равномерным масштабом.".into();
                    return;
                }
                let transform =
                    Transform::from_matrix(parent.inverse() * camera_transform(&camera).matrix());
                self.editor.begin_edit("Камера из текущего вида");
                let result = self.editor.set_transform(id, transform);
                if self.result(result).is_some() {
                    let result = self
                        .editor
                        .transact("Объектив из текущего вида", |p| {
                            p.node_mut(id).unwrap().camera = Some(CameraLens::from_camera(&camera))
                        });
                    if self.result(result).is_some() {
                        let result = self.editor.commit_edit();
                        self.result(result);
                    } else {
                        self.editor.cancel_edit();
                    }
                } else {
                    self.editor.cancel_edit();
                }
            }
            _ => {}
        }
        self.update_dirty();
    }

    pub(super) fn camera_confirm(&mut self, action: &Action, value: &str) -> Result<bool, String> {
        let Action::CameraNumber(field) = action else {
            return Ok(false);
        };
        let id = self.selected_camera_id().ok_or("Выберите камеру.")?;
        if !self.node_selectable(id) {
            return Err("Камера скрыта или заблокирована.".into());
        }
        let value = value
            .trim()
            .replace(',', ".")
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or("Введите конечное число.")?;
        let mut lens = self.editor.project.node(id).and_then(|n| n.camera).unwrap();
        match field {
            0 if (1. ..=179.).contains(&value) => lens.fov_y_radians = value.to_radians(),
            1 if (0.001..=1_000_000.).contains(&value) => lens.ortho_size = value,
            2 if (0.01..=100000.).contains(&value) => lens.depth_of_field.focus_distance = value,
            3 if (0.7..=64.).contains(&value) => lens.depth_of_field.f_stop = value,
            4..=10 => lens.effects.set(*field, value).map_err(|e| e.to_string())?,
            2 | 3 => return Err("Фокус: 0.01–100000 м; диафрагма: f/0.7–f/64.".into()),
            0 => return Err("Угол обзора — от 1 до 179 градусов.".into()),
            1 => return Err("Полувысота кадра — от 0.001 до 1000000.".into()),
            _ => return Err("Неизвестный параметр камеры.".into()),
        }
        lens.validate().map_err(|e| e.to_string())?;
        self.editor
            .transact("Объектив камеры", |p| {
                p.node_mut(id).unwrap().camera = Some(lens)
            })
            .map_err(|e| e.to_string())?;
        self.update_dirty();
        Ok(true)
    }

    pub(super) fn camera_pointer_down(&mut self) -> bool {
        if self.has_modal()
            || !self.canvas_input_at(self.mouse)
            || self.drag.is_some()
            || self.camera_object_view().is_some()
        {
            return false;
        }
        if self
            .selected_gizmo()
            .and_then(|g| g.hit(self.mouse))
            .is_some()
        {
            return false;
        }
        let Ok(eval) = self.evaluated_interactive() else {
            return false;
        };
        let view = self.view();
        let vp = self.viewport();
        let picked = self
            .editor
            .project
            .nodes
            .iter()
            .filter(|n| n.camera.is_some() && self.node_selectable(n.id))
            .filter_map(|n| {
                let camera = self.editor.project.camera_from_node(n.id, &eval.global)?;
                let (point, _) = screen_segment(&view, vp, camera.eye, camera.eye)?;
                let distance = Vec2::from_array(point).distance(Vec2::from_array(self.mouse));
                (distance <= 14.).then_some((n.id, distance, camera.eye.distance(view.eye)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.2.total_cmp(&b.2)))
            .map(|p| p.0);
        let Some(id) = picked else {
            return false;
        };
        self.action(Action::Select(id));
        self.set_properties_tab(0);
        self.preferences.right_panel = true;
        self.tool = Tool::Move;
        self.redraw();
        true
    }

    pub(super) fn draw_camera_overlay(&self, u: &mut Ui) {
        // The camera-view button is always available, including in an empty
        // scene. Only frustums require a full scene evaluation.
        let eval = (self.editor.mode != EditorMode::Render
            && self.editor.project.nodes.iter().any(|n| n.camera.is_some()))
            .then(|| self.evaluated_interactive().ok()).flatten();
        let view = self.view();
        let vp = self.viewport();
        let palette = u.palette();
        let aspect = self.editor.project.render_settings.output_width as f32
            / self.editor.project.render_settings.output_height.max(1) as f32;
        for node in self
            .editor
            .project
            .nodes
            .iter()
            .filter(|n| n.camera.is_some() && self.editor.mode != EditorMode::Render)
            .take(128)
        {
            let Some(eval) = &eval else { break; };
            if !self.in_isolation(node.id)
                || !visible_camera(&self.editor.project, node.id)
                || self.camera_view == Some(node.id)
            {
                continue;
            }
            let Some(camera) = self.editor.project.camera_from_node(node.id, &eval.global) else {
                continue;
            };
            let selected = self.editor.selected == Some(node.id);
            let color = if selected {
                palette.accent
            } else {
                palette.muted
            };
            let depth = (view.eye.distance(camera.eye) * 0.13).clamp(0.12, 2.);
            for (a, b) in frustum_segments(&camera, aspect.clamp(0.05, 20.), depth) {
                if let Some((a, b)) = screen_segment(&view, vp, a, b) {
                    u.smooth_segment(a, b, if selected { 1.7 } else { 1.1 }, color);
                }
            }
            if let Some((center, _)) = screen_segment(&view, vp, camera.eye, camera.eye) {
                if center[0] >= vp[0] + 12.
                    && center[0] <= vp[0] + vp[2] - 12.
                    && center[1] >= vp[1] + 12.
                    && center[1] <= vp[1] + vp[3] - 12.
                {
                    let rect = [center[0] - 11., center[1] - 9., 22., 18.];
                    u.surface(
                        rect,
                        if selected {
                            palette.selection
                        } else {
                            palette.panel
                        },
                    );
                    Icon::Camera.draw(u, rect, color);
                }
            }
        }
        if self.camera_object_view().is_some() {
            let frame = frame_rect(vp, aspect);
            // Opaque bars make this region the exact output crop. The expanded
            // projection above preserves the camera lens within the safe frame.
            for rect in [
                [vp[0], vp[1], vp[2], frame[1] - vp[1]],
                [
                    vp[0],
                    frame[1] + frame[3],
                    vp[2],
                    vp[1] + vp[3] - frame[1] - frame[3],
                ],
                [vp[0], frame[1], frame[0] - vp[0], frame[3]],
                [
                    frame[0] + frame[2],
                    frame[1],
                    vp[0] + vp[2] - frame[0] - frame[2],
                    frame[3],
                ],
            ] {
                if rect[2] > 0. && rect[3] > 0. {
                    u.rect(rect, palette.bg);
                }
            }
            if self
                .camera_view
                .and_then(|id| self.editor.project.node(id))
                .and_then(|n| n.camera)
                .is_some_and(|lens| lens.thirds)
            {
                for fraction in [1. / 3., 2. / 3.] {
                    let color = [palette.muted[0], palette.muted[1], palette.muted[2], 0.55];
                    u.smooth_segment(
                        [frame[0] + frame[2] * fraction, frame[1]],
                        [frame[0] + frame[2] * fraction, frame[1] + frame[3]],
                        1.,
                        color,
                    );
                    u.smooth_segment(
                        [frame[0], frame[1] + frame[3] * fraction],
                        [frame[0] + frame[2], frame[1] + frame[3] * fraction],
                        1.,
                        color,
                    );
                }
            }
            let edges = [
                ([frame[0], frame[1]], [frame[0] + frame[2], frame[1]]),
                (
                    [frame[0], frame[1] + frame[3]],
                    [frame[0] + frame[2], frame[1] + frame[3]],
                ),
                ([frame[0], frame[1]], [frame[0], frame[1] + frame[3]]),
                (
                    [frame[0] + frame[2], frame[1]],
                    [frame[0] + frame[2], frame[1] + frame[3]],
                ),
            ];
            for (a, b) in edges {
                u.smooth_segment(a, b, 1., palette.accent);
            }
        }
        if self.camera_view.is_some() {
            u.round_icon_button(
                [self.navigation_right() - 80., 184., 32., 32.],
                Some(Action::Studio(crate::ui::StudioAction::CameraLock)),
                self.camera_navigation_lock,
                Icon::Lock,
            );
        }
        if let Some(id) = self
            .selected_camera_id()
            .or(self.camera_view)
            .or(self.editor.project.active_camera)
        {
            u.round_icon_button(
                [self.navigation_right() - 42., 184., 32., 32.],
                Some(Action::CameraView),
                self.camera_view == Some(id),
                Icon::Camera,
            );
        } else {
            u.round_icon_button(
                [self.navigation_right() - 42., 184., 32., 32.],
                Some(Action::CameraView),
                false,
                Icon::Camera,
            );
        }
        self.draw_view_navigation_buttons(u);
    }

    pub(super) fn draw_view_navigation_buttons(&self, u: &mut Ui) {
        let view = self.view();
        let projection = if view.orthographic.is_some() { Icon::Orthographic } else { Icon::Perspective };
        let viewport = self.viewport();
        let vertical = viewport[1] + viewport[3] >= 332.;
        for (i, (action, icon)) in [(Action::Perspective, projection), (Action::ViewZoom, Icon::Search), (Action::ViewPan, Icon::Hand)].into_iter().enumerate() {
            let rect = if vertical {
                [self.navigation_right() - 42., 220. + i as f32 * 36., 32., 32.]
            } else {
                [self.navigation_right() - 114. + i as f32 * 36., 220., 32., 32.]
            };
            let active = match action {
                Action::Perspective => view.orthographic.is_some(),
                Action::ViewPan => self.view_navigation_active(1),
                _ => self.view_navigation_active(2),
            };
            u.round_icon_button(rect, Some(action), active, icon);
        }
    }

    pub(super) fn draw_camera_panel(&self, u: &mut Ui, rx: f32, rw: f32) {
        let Some(id) = self.selected_camera_id() else {
            return;
        };
        let Some(node) = self.editor.project.node(id) else {
            return;
        };
        let lens = node.camera.unwrap();
        let p = u.palette();
        u.field(
            [rx + 16., 171., rw - 32., 32.],
            &node.name,
            Action::EditName,
        );
        u.label(
            rx + 16.,
            215.,
            if self.editor.project.active_camera == Some(id) {
                "КАМЕРА · используется для рендера"
            } else {
                "КАМЕРА · локальные оси"
            },
            10.,
            p.muted,
        );
        if let Some(t) = self.editing_transform(id) {
            let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
            let values = [
                t.translation.to_array(),
                [x.to_degrees(), y.to_degrees(), z.to_degrees()],
            ];
            for (row, label) in ["Положение, м", "Вращение, градусы"].iter().enumerate()
            {
                let y = 245. + row as f32 * 72.;
                u.label(rx + 16., y, *label, 11., p.muted);
                for axis in 0..3 {
                    let value = values[row][axis] as f64;
                    let range = if row == 0 {
                        let bound = 10_f64.max(value.abs() * 2.);
                        SliderRange::new(-bound, bound, 0.01)
                    } else {
                        SliderRange::new(-180., 180., 0.1)
                    };
                    u.number_field(
                        [
                            rx + 16. + axis as f32 * (rw - 26.) / 3.,
                            y + 22.,
                            (rw - 42.) / 3.,
                            44.,
                        ],
                        ["X", "Y", "Z"][axis],
                        value,
                        Action::EditTransform(row * 3 + axis),
                        range,
                    );
                }
            }
        }
        u.button(
            [rx + 16., 402., rw - 32., 30.],
            if lens.orthographic {
                "Проекция: ортографическая"
            } else {
                "Проекция: перспектива"
            },
            Action::CameraProjection,
            false,
        );
        u.label(
            rx + 16.,
            444.,
            if lens.orthographic {
                "Полувысота кадра, м"
            } else {
                "Вертикальный угол обзора, °"
            },
            11.,
            p.muted,
        );
        u.number_field(
            [rx + 16., 466., rw - 32., 44.],
            "",
            if lens.orthographic {
                lens.ortho_size as f64
            } else {
                lens.fov_y_radians.to_degrees() as f64
            },
            Action::CameraNumber(if lens.orthographic { 1 } else { 0 }),
            if lens.orthographic {
                SliderRange::new(0.01, 30., 0.01)
            } else {
                SliderRange::new(1., 179., 0.1)
            },
        );
        u.icon_button(
            [rx + 16., 526., rw - 32., 32.],
            "Вид из камеры",
            Action::CameraView,
            self.camera_view == Some(id),
            Icon::Eye,
        );
        u.icon_button(
            [rx + 16., 568., rw - 32., 32.],
            "Камера из текущего вида",
            Action::CameraAlignView,
            false,
            Icon::Camera,
        );
        u.button(
            [rx + 16., 610., rw - 32., 32.],
            "Использовать для рендера",
            Action::CameraSetActive,
            self.editor.project.active_camera == Some(id),
        );
        u.button(
            [rx + 16., 652., rw - 32., 30.],
            "Рендер из свободного вида",
            Action::CameraClearActive,
            self.editor.project.active_camera.is_none(),
        );
        u.label(
            rx + 16.,
            700.,
            "G — перемещение · R — поворот",
            11.,
            p.muted,
        );
        u.label(
            rx + 16.,
            722.,
            "Кадр: разрешение в панели «Рендер»",
            10.,
            p.muted,
        );
        u.label(
            rx + 16.,
            748.,
            if self.camera_navigation_lock && self.camera_view.is_some() {
                "Замок: ПКМ — осмотр; колесо — движение"
            } else {
                "ПКМ / колесо: выйти из вида камеры"
            },
            10.,
            p.muted,
        );
        use crate::ui::CameraTool as C;
        let x = rx + 16.;
        let width = rw - 32.;
        u.checkbox(
            [x, 790., width, 28.],
            "Глубина резкости",
            Action::CameraTool(C::Dof),
            lens.depth_of_field.enabled,
        );
        u.number_field(
            [x, 828., width, 44.],
            "Фокус, м",
            lens.depth_of_field.focus_distance as f64,
            Action::CameraNumber(2),
            SliderRange::new(0.01, 100., 0.01),
        );
        u.number_field(
            [x, 880., width, 44.],
            "Диафрагма · f/",
            lens.depth_of_field.f_stop as f64,
            Action::CameraNumber(3),
            SliderRange::new(0.7, 64., 0.1),
        );
        u.button(
            [x, 934., width, 30.],
            "Выбрать фокус в кадре",
            Action::CameraTool(C::PickFocus),
            self.focus_pick == Some(id),
        );
        u.checkbox(
            [x, 974., width, 28.],
            "Сетка третей",
            Action::CameraTool(C::Thirds),
            lens.thirds,
        );
        u.button(
            [x, 1012., width, 30.],
            "Сохранить ракурс…",
            Action::CameraTool(C::SaveView),
            false,
        );
        self.draw_dropdown_button(
            u,
            [x, 1052., width, 30.],
            crate::ui::DropdownKind::SavedViews,
            "Сохранённые ракурсы",
        );
        if self
            .last_saved_view
            .is_some_and(|i| i < self.editor.project.saved_views.len())
        {
            u.button(
                [x, 1092., width, 28.],
                "Удалить выбранный ракурс",
                Action::CameraTool(C::DeleteView),
                false,
            );
        }
        u.checkbox([x, 1154., width, 28.], "Эффекты объектива", Action::CameraTool(C::Effects), lens.effects.enabled);
        for field in 4..=10 {
            let (value, min, max, label) = lens.effects.field(field).unwrap();
            u.number_field([x, 1194. + (field - 4) as f32 * 50., width, 44.], label, value as f64,
                Action::CameraNumber(field), SliderRange::new(min as f64, max as f64, 0.01));
        }
        u.fitted_label([x, 1548., width, 22.], "Вид из камеры · Материал / Рендер", 10., p.muted);
    }
}
