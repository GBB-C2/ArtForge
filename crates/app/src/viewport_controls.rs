//! Overlay layout, axis navigation and camera-bound view gestures.
use super::*;
impl App {
    pub(super) fn viewport_shading_needs_override(&self, project: &Project) -> bool {
        self.preferences.viewport_shading < 3
            || project.materials.iter().any(|m| m.double_sided != self.preferences.show_backfaces)
            || project.meshes.iter().any(|m| m.material.is_none())
    }
    pub(super) fn apply_viewport_shading(&self, project: &mut poseforge_core::Project) {
        self.apply_viewport_backfaces(project);
        let mode = self.preferences.viewport_shading;
        if mode >= 3 { return; }
        let s = &mut project.render_settings;
        s.atmosphere.density = 0.; s.exposure = 0.; s.shadows = false;
        s.ray_shadows = false; s.ray_reflections = false; s.hdri_enabled = false; s.hdri_background = false;
        s.npr_contours.width = 0.; s.ambient_occlusion = 0.;
        s.sky_color = [0.55, 0.58, 0.65]; s.ground_color = [0.12, 0.13, 0.16]; s.ambient_intensity = 0.8;
        s.background_mode = poseforge_core::BackgroundMode::Theme; s.transparent_background = false;
        project.lights = vec![poseforge_core::Light {
            name: "Студийный свет".into(), color: [1.; 3], intensity: 3., casts_shadow: false,
            ..Default::default()
        }];
        // The replacement light must not inherit collection visibility from scene light 0.
        for collection in &mut project.collections { collection.light_members.clear(); }
        project.clean_outliner();
        if mode <= 1 {
            for node in &mut project.nodes { node.material_layers.clear(); }
            for material in &mut project.materials {
                let id = material.id; let name = material.name.clone(); let visible = material.visible;
                *material = poseforge_core::Material { id, name, visible, base_color: [0.42, 0.45, 0.5, 1.],
                    roughness: 0.8, double_sided: self.preferences.show_backfaces, ..Default::default() };
            }
        }
    }

    pub(super) fn apply_viewport_backfaces(&self, project: &mut Project) {
        // This is applied only to disposable viewport/picking snapshots.
        let show = self.preferences.show_backfaces;
        for material in &mut project.materials { material.double_sided = show; }
        if project.meshes.iter().any(|mesh| mesh.material.is_none()) {
            let id = project.next_id();
            project.materials.push(poseforge_core::Material { id, double_sided: show, ..Default::default() });
            for mesh in &mut project.meshes { if mesh.material.is_none() { mesh.material = Some(id); } }
        }
    }

    fn rail_tools(&self) -> Vec<(Action, Icon, bool, bool)> {
        let mut tools = vec![(Action::Orbit, Icon::Orbit, self.tool == Tool::Orbit, true)];
        if self.authoring.editing.is_some() {
            tools.extend([
                (Action::Move, Icon::Move, self.tool == Tool::Move, true),
                (Action::Author(ui::AuthorAction::ToggleEdit), Icon::Object, false, true),
            ]);
        } else if self.editor.mode != EditorMode::Render || self.active_light_index().is_some() {
            tools.extend([
                (Action::Move, Icon::Move, self.tool == Tool::Move, true),
                (Action::Rotate, Icon::Rotate, self.tool == Tool::Rotate, true),
                (Action::Scale, Icon::Scale, self.tool == Tool::Scale, self.active_light_index().is_none()),
            ]);
            if self.tool != Tool::Orbit {
                tools.push((Action::ToggleGizmoSpace, Icon::AxisSpace, self.gizmo_local || self.tool == Tool::Scale, self.tool != Tool::Scale));
                if let Some(node) = self.editor.selected.and_then(|id| self.editor.project.node(id)).filter(|n| n.is_joint) {
                    tools.push((Action::Ik, Icon::RigBind, self.ik, self.ik || self.pose_chain(node.id).is_ok()));
                }
            }
        }
        tools
    }
    pub(super) fn tools_expanded(&self) -> bool { self.preferences.tools_expanded }
    pub(super) fn tools_height(&self) -> f32 {
        12. + self.rail_tools().len() as f32 * 34.
    }
    pub(super) fn draw_tool_rail(&mut self, u: &mut Ui) {
        if !self.scene_panel_visible() { return; }
        let rail = self.rail_rect();
        u.outlined_surface(rail, u.palette().panel, u.palette().border);
        let columns = ((rail[2] - 12.) / 34.).floor().max(1.) as usize;
        for (i, (action, icon, active, enabled)) in self.rail_tools().into_iter().enumerate() {
            let rect = [rail[0] + 6. + (i % columns) as f32 * 34., rail[1] + 6. + (i / columns) as f32 * 34., 32., 32.];
            if enabled { u.quiet_icon_button(rect, "", action, active, icon); }
            else { u.disabled_icon_button(rect, "", icon); }
        }
    }

}

#[derive(Clone)]
pub(super) struct CameraNavigation {
    id: u64,
    start: [f32; 2],
    camera: poseforge_core::Camera,
    parent: Mat4,
    original: Transform,
    curve: Option<Arc<poseforge_core::CurveTranslation>>,
    pan: bool,
    zoom: bool,
    speed: f32,
}

impl App {
    pub(super) fn navigation_right(&self) -> f32 {
        if self.preferences.right_panel {
            self.right_panel_rect()[0] - 12.
        } else {
            self.width - 12.
        }
    }
    pub(super) fn view_widget_rect(&self) -> [f32; 4] {
        [self.navigation_right() - 116., 68., 112., 112.]
    }
    pub(super) fn shading_toolbar_rect(&self) -> [f32; 4] {
        let r = self.view_menu_rect(); [r[0], r[1], 146., r[3]]
    }
    pub(super) fn view_menu_rect(&self) -> [f32; 4] {
        let r = self.view_widget_rect();
        [r[0] - 158., r[1] + 6., 26., 26.]
    }
    pub(super) fn object_scrollbar_rect(&self) -> [f32; 4] {
        let r = self.left_panel_rect();
        [
            r[0] + r[2] - 16.,
            r[1] + 74.,
            12.,
            self.scene_capacity() as f32 * 27.,
        ]
    }
    pub(super) fn update_object_scroll(&mut self, start: f32, value: usize, y: f32) {
        let count = self.outliner_rows().len();
        let cap = self.scene_capacity();
        let max = count.saturating_sub(cap);
        let r = self.object_scrollbar_rect();
        let thumb = (r[3] * cap as f32 / count.max(1) as f32).max(28.).min(r[3]);
        self.scene_scroll = (value as f32 + (y - start) * max as f32 / (r[3] - thumb).max(1.))
            .round()
            .clamp(0., max as f32) as usize;
    }
    pub(super) fn start_object_scroll(&mut self) {
        let r = self.object_scrollbar_rect();
        let count = self.outliner_rows().len();
        let cap = self.scene_capacity();
        let max = count.saturating_sub(cap);
        let thumb = (r[3] * cap as f32 / count.max(1) as f32).max(28.).min(r[3]);
        let top = r[1] + (r[3] - thumb) * self.scene_scroll as f32 / max.max(1) as f32;
        if self.mouse[1] < top || self.mouse[1] > top + thumb {
            self.scene_scroll = ((self.mouse[1] - r[1] - thumb * 0.5) / (r[3] - thumb).max(1.)
                * max as f32)
                .round()
                .clamp(0., max as f32) as usize;
        }
        self.drag = Some(Drag::ObjectScroll {
            start: self.mouse[1],
            value: self.scene_scroll,
        });
        self.drag_button = Some(MouseButton::Left);
    }
    pub(super) fn draw_view_widget(&self, u: &mut Ui) {
        let r = self.view_widget_rect();
        let center = [r[0] + 56., r[1] + 56.];
        let p = u.palette();
        let view = self.view();
        let backward = (view.eye - view.target).normalize_or_zero();
        let right = view.up.cross(backward).normalize_or_zero();
        let up = backward.cross(right).normalize_or_zero();
        let hovered = ui::circle_contains(r, self.mouse);
        if hovered {
            u.rounded_rect(r, [p.panel[0], p.panel[1], p.panel[2], 0.92], 56.);
        }
        u.hits.push(ui::Hit {
            rect: r,
            action: Action::ViewOrbit,
        });
        let mut endpoints: Vec<_> = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, a)| {
            (
                i,
                [center[0] + a.dot(right) * 36., center[1] - a.dot(up) * 36.],
                a.dot(backward),
            )
        })
        .collect();
        endpoints.sort_by(|a, b| a.2.total_cmp(&b.2));
        let hot = endpoints
            .iter()
            .rev()
            .find(|(_, pt, d)| {
                let radius = if *d >= -0.01 { 14. } else { 11. };
                ui::circle_contains(
                    [pt[0] - radius, pt[1] - radius, radius * 2., radius * 2.],
                    self.mouse,
                )
            })
            .map(|e| e.0);
        // Paint the rear arms first so front-facing handles always win overlaps.
        for &(i, point, depth) in &endpoints {
            let mut color = [
                [0.94, 0.32, 0.36, 1.],
                [0.46, 0.79, 0.26, 1.],
                [0.31, 0.57, 1., 1.],
            ][i / 2];
            color[3] = if depth < 0. { 0.35 } else { 0.9 };
            u.smooth_segment(center, point, if i % 2 == 0 { 2.2 } else { 1.2 }, color);
        }
        for (i, point, depth) in endpoints {
            let color = [
                [0.94, 0.32, 0.36, 1.],
                [0.46, 0.79, 0.26, 1.],
                [0.31, 0.57, 1., 1.],
            ][i / 2];
            let near = depth >= -0.01;
            let radius = if near { 11. } else { 8. };
            let active = depth > 0.999 && view.orthographic.is_some();
            if hot == Some(i) || active {
                let outer = radius + 2.5;
                u.rounded_rect(
                    [point[0] - outer, point[1] - outer, outer * 2., outer * 2.],
                    p.text,
                    outer,
                );
            }
            u.rounded_rect(
                [
                    point[0] - radius,
                    point[1] - radius,
                    radius * 2.,
                    radius * 2.,
                ],
                color,
                radius,
            );
            if !near || i % 2 == 1 {
                let inner = radius - 1.5;
                u.rounded_rect(
                    [point[0] - inner, point[1] - inner, inner * 2., inner * 2.],
                    p.panel,
                    inner,
                );
            }
            // Single-letter centered glyphs remain legible at both DPI scales.
            let size = if near { 11. } else { 9. };
            u.label(
                point[0] - size * 0.325,
                point[1] - size * 0.65,
                ["X", "Y", "Z"][i / 2],
                size,
                if near && i % 2 == 0 { p.bg } else { color },
            );
            let hit = if near { 14. } else { 11. };
            u.hits.push(ui::Hit {
                rect: [point[0] - hit, point[1] - hit, hit * 2., hit * 2.],
                action: Action::ViewAxis(i as u8),
            });
        }
        let menu = self.view_menu_rect();
        for (i, icon) in [Icon::Grid, Icon::Sphere, Icon::Material, Icon::Render].into_iter().enumerate() {
            u.quiet_icon_button([menu[0] + 30. + i as f32 * 30., menu[1], 26., 26.], "",
                Action::ViewShading(i as u8), self.preferences.viewport_shading == i as u8, icon);
        }
        u.icon_button(
            self.view_menu_rect(),
            "",
            Action::ToggleViewMenu,
            self.menu == Some(crate::chrome::Menu::View),
            Icon::Eye,
        );
    }
    fn camera_navigation(&mut self, pan: bool) -> Option<CameraNavigation> {
        let id = self.camera_view?;
        if !self.node_selectable(id) || self.scene_write_pending() {
            self.status = "Камера или её родитель недоступны для изменения.".into();
            return None;
        }
        let evaluated = self.editor.evaluated().ok()?;
        let camera = self
            .editor
            .project
            .camera_from_node(id, &evaluated.global)?;
        let parent = self.editor.project.transform_frame(id, &evaluated.global, self.editor.time).ok()?;
        if crate::gizmo::uniform_parent_rotation(parent).is_none() {
            self.status = "Навигация камеры требует родителя с равномерным масштабом.".into();
            return None;
        }
        Some(CameraNavigation {
            id,
            start: self.mouse,
            camera,
            parent,
            original: *evaluated.local.get(&id)?,
            curve: match self.object_curve_translation(id, &evaluated) {
                Ok(path) => path, Err(e) => { self.status = e.to_string(); return None; }
            },
            pan,
            zoom: false,
            speed: self.distance.clamp(0.1, 100.) * 0.0015,
        })
    }
    fn apply_camera_navigation(
        &mut self,
        drag: &CameraNavigation,
        camera: poseforge_core::Camera,
        translate_only: bool,
    ) {
        if translate_only {
            if let Some(path) = &drag.curve {
                let result = if drag.zoom {
                    let forward = (drag.camera.target - drag.camera.eye).normalize();
                    path.advance((camera.eye - drag.camera.eye).dot(forward))
                        .and_then(|position| self.set_object_curve_position(drag.id, path, position))
                } else { self.translate_object_on_curve(drag.id, path, camera.eye) };
                self.result(result); return;
            }
            let mut local = drag.original;
            local.translation += drag
                .parent
                .inverse()
                .transform_vector3(camera.eye - drag.camera.eye);
            let result = self.editor.set_transform(drag.id, local);
            self.result(result);
            return;
        }
        let mut local = Transform::from_matrix(
            drag.parent.inverse() * poseforge_core::camera_transform(&camera).matrix(),
        );
        local.scale = drag.original.scale;
        let r = self.editor.set_transform(drag.id, local);
        self.result(r);
    }
    /// In lock-to-camera mode, all previews derive from the initial pose.
    pub(super) fn begin_camera_navigation(&mut self, pan: bool, button: MouseButton) -> bool {
        if !self.camera_navigation_lock || self.camera_view.is_none() {
            return false;
        }
        if let Some(drag) = self.camera_navigation(pan) {
            self.editor.playing = false;
            self.editor.begin_edit("Навигация камеры");
            self.drag = Some(Drag::CameraNavigate(drag));
            self.drag_button = Some(button);
            self.status =
                "Камера: ПКМ — осмотр, СКМ — сдвиг, колесо — вперёд/назад; Esc — отмена.".into();
        }
        true
    }
    pub(super) fn update_camera_navigation(&mut self, drag: CameraNavigation, point: [f32; 2]) {
        if self.camera_view != Some(drag.id) || !self.node_selectable(drag.id) {
            self.cancel_drag();
            return;
        }
        let delta = [point[0] - drag.start[0], point[1] - drag.start[1]];
        if delta[0].abs() + delta[1].abs() < 0.0001 {
            self.editor.cancel_edit();
            self.editor.begin_edit("Навигация камеры");
            self.invalidate();
            return;
        }
        let mut camera = drag.camera.clone();
        let forward = (camera.target - camera.eye).normalize();
        let right = forward.cross(camera.up).normalize();
        let up = right.cross(forward);
        let fine = if self.modifiers.shift_key() { 0.15 } else { 1. };
        if drag.zoom {
            let movement = -forward * delta[1] * drag.speed * 8. * fine;
            camera.eye += movement;
            camera.target += movement;
        } else if drag.pan {
            let movement = (-right * delta[0] + up * delta[1]) * drag.speed * fine;
            camera.eye += movement;
            camera.target += movement;
        } else {
            let reference_right = forward.cross(Vec3::Y).try_normalize().unwrap_or(right);
            let level_up = reference_right.cross(forward).normalize();
            let roll = forward.dot(level_up.cross(up)).atan2(level_up.dot(up));
            let yaw = forward.x.atan2(forward.z) - delta[0] * 0.006 * fine;
            let pitch =
                (forward.y.clamp(-1., 1.).asin() - delta[1] * 0.006 * fine).clamp(-1.56, 1.56);
            let direction = Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                yaw.cos() * pitch.cos(),
            );
            let level_up = direction
                .cross(Vec3::Y)
                .normalize()
                .cross(direction)
                .normalize();
            camera.target = camera.eye + direction;
            camera.up = glam::Quat::from_axis_angle(direction, roll) * level_up;
        }
        self.apply_camera_navigation(&drag, camera, drag.pan || drag.zoom);
    }
    pub(super) fn camera_scroll(&mut self, delta: f32) -> bool {
        if !self.camera_navigation_lock || self.camera_view.is_none() {
            return false;
        }
        if let Some(mut drag) = self.camera_navigation(false) {
            drag.zoom = true;
            if delta == 0. || !delta.is_finite() {
                return true;
            }
            let mut camera = drag.camera.clone();
            let movement = (camera.target - camera.eye).normalize()
                * delta.clamp(-10., 10.)
                * drag.speed
                * 80.;
            camera.eye += movement;
            camera.target += movement;
            self.editor.playing = false;
            self.editor.begin_edit("Переместить камеру");
            self.apply_camera_navigation(&drag, camera, true);
            let r = self.editor.commit_edit();
            self.result(r);
        }
        true
    }
    pub(super) fn click_view_axis(&mut self, axis: u8) {
        let directions = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ];
        let Some(&direction) = directions.get(axis as usize) else {
            return;
        };
        let view = self.view();
        let aligned = (view.eye - view.target).normalize_or_zero().dot(direction) > 0.999;
        self.orient_view(if aligned { axis ^ 1 } else { axis });
    }
    pub(super) fn orient_view(&mut self, axis: u8) {
        let Some(&direction) = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ]
        .get(axis as usize) else {
            return;
        };
        if self.camera_navigation_lock && self.camera_view.is_some() {
            if let Some(drag) = self.camera_navigation(false) {
                let mut camera = drag.camera.clone();
                camera.target = camera.eye - direction;
                camera.up = if axis == 2 {
                    Vec3::NEG_Z
                } else if axis == 3 {
                    Vec3::Z
                } else {
                    Vec3::Y
                };
                self.editor.begin_edit("Направление камеры");
                self.apply_camera_navigation(&drag, camera, false);
                let r = self.editor.commit_edit();
                self.result(r);
            }
        } else {
            self.leave_camera_view();
            self.yaw = direction.x.atan2(direction.z);
            self.pitch = direction.y.asin().clamp(-1.56, 1.56);
        }
    }
    pub(super) fn view_navigation_active(&self, mode: u8) -> bool {
        match &self.drag {
            Some(Drag::ViewWidget { navigation, .. }) => *navigation == mode,
            Some(Drag::CameraNavigate(drag)) => if drag.zoom { mode == 2 } else if drag.pan { mode == 1 } else { mode == 0 },
            _ => false,
        }
    }
    pub(super) fn begin_view_widget(&mut self) {
        self.begin_view_navigation(0);
    }
    pub(super) fn begin_view_navigation(&mut self, navigation: u8) {
        if self.begin_camera_navigation(navigation == 1, MouseButton::Left) {
            if let Some(Drag::CameraNavigate(drag)) = &mut self.drag { drag.zoom = navigation == 2; }
            return;
        }
        let camera_view = self.camera_view;
        let recalled_view = self.recalled_view.clone();
        let last_saved_view = self.last_saved_view;
        let saved_view = (
            self.yaw,
            self.pitch,
            self.target,
            self.distance,
            self.orthographic,
            self.free_view_fov,
        );
        self.leave_camera_view();
        self.drag = Some(Drag::ViewWidget {
            navigation,
            target: self.target,
            distance: self.distance,
            start: self.mouse,
            yaw: self.yaw,
            pitch: self.pitch,
            saved_view,
            camera_view,
            recalled_view,
            last_saved_view,
        });
        self.drag_button = Some(MouseButton::Left);
    }
    pub(super) fn cancel_view_navigation(&mut self, drag: &Drag) {
        if let Drag::ViewWidget {
            saved_view,
            camera_view,
            recalled_view,
            last_saved_view,
            ..
        } = drag
        {
            (
                self.yaw,
                self.pitch,
                self.target,
                self.distance,
                self.orthographic,
                self.free_view_fov,
            ) = *saved_view;
            self.camera_view = *camera_view;
            self.recalled_view = recalled_view.clone();
            self.last_saved_view = *last_saved_view;
        }
    }
    pub(super) fn toggle_view_projection(&mut self) {
        if self.camera_navigation_lock {
            if let Some(id) = self.camera_view {
                if !self.node_selectable(id) || self.scene_write_pending() {
                    return;
                }
                let result = self.editor.transact("Проекция камеры", |p| {
                    if let Some(lens) = p.node_mut(id).and_then(|n| n.camera.as_mut()) {
                        lens.orthographic = !lens.orthographic;
                    }
                });
                self.result(result);
                return;
            }
        }
        self.leave_camera_view();
        self.orthographic = !self.orthographic;
    }
}
