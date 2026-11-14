//! Interactive knife and proportional-edit state share the editor transaction.
use super::*;
use crate::ui::AuthorAction as A;
use poseforge_core::{EdgePoint, MeshEdit};
#[path = "smart_knife.rs"]
mod smart_knife;

pub(super) struct KnifeState {
    node: u64,
    first: Option<Vec3>,
    selection: component_selection::SelectionSnapshot,
    cuts: usize,
    pub(super) press: Option<[f32; 2]>,
}
impl App {
    pub(super) fn modeling_area(&self) -> [f32; 4] {
        let left = self.left_panel_rect();
        let viewport = self.viewport();
        let x = (left[0] + left[2] + 12.).max(viewport[0]);
        let right = if self.preferences.right_panel {
            self.right_panel_rect()[0] - 12.
        } else {
            self.width - 12.
        };
        let top = viewport[1].max(72.);
        let right = right.min(viewport[0] + viewport[2]);
        let bottom = (self.panel_bottom() - 12.).min(viewport[1] + viewport[3]);
        [x, top, (right - x).max(1.), (bottom - top).max(1.)]
    }
    pub(super) fn advanced_action(&mut self, command: A) -> bool {
        match command {
            A::Knife => {
                self.cancel_modeling_live();
                let Some(node) = self.authoring.editing else {
                    return true;
                };
                self.editor.begin_edit("Нож");
                self.authoring.knife = Some(KnifeState {
                    node,
                    first: None,
                    selection: component_selection::SelectionSnapshot::capture(&self.authoring),
                    cuts: 0,
                    press: None,
                });
                self.status =
                    "Нож · потяните линию через грани · Shift / C — угол · Enter — применить · Esc — отменить"
                        .into();
            }
            A::KnifeSnap => self.authoring.knife_snap = !self.authoring.knife_snap,
            A::KnifeAngle(mode) => self.authoring.knife_angle = mode.min(4),
            A::KnifeAccept => self.accept_knife(),
            A::KnifeCancel => self.cancel_knife(),
            A::Proportional => self.authoring.proportional = !self.authoring.proportional,
            A::ProportionalRadius => self.input(
                Action::Author(command),
                ui::number_caption(self.authoring.proportional_radius as f64),
            ),
            _ => return false,
        }
        true
    }
    pub(super) fn cancel_knife(&mut self) {
        if let Some(k) = self.authoring.knife.take() {
            self.release_drag_pointer();
            self.editor.cancel_edit();
            k.selection.restore(&mut self.authoring);
            self.invalidate_geometry();
            self.update_dirty();
            self.status = "Нож · отменено".into();
        }
    }
    fn accept_knife(&mut self) {
        if let Some(k) = self.authoring.knife.take() {
            self.release_drag_pointer();
            let r = self.editor.commit_edit();
            if self.result(r).is_some() {
                self.status = format!("Нож · разрезов: {}", k.cuts);
            }
            self.authoring.vertices.clear();
            self.authoring.edges.clear();
            self.authoring.faces.clear();
            self.invalidate_geometry();
            self.update_dirty();
        }
    }
    pub(super) fn route_knife_action(&mut self, action: &Action) -> bool {
        if self.authoring.knife.is_none() {
            return false;
        }
        match action {
            Action::ToggleSection(id)
                if matches!(
                    *id,
                    super::modeling_palette::TOOLS_SECTION
                        | super::modeling_palette::PROPERTIES_SECTION
                ) =>
            {
                false
            }
            Action::ScrollPanel(0) | Action::ToggleRightPanel => false,
            Action::Author(A::KnifeAccept | A::KnifeCancel | A::KnifeSnap | A::KnifeAngle(_))
            | Action::DropdownOpen(ui::DropdownKind::KnifeAngle)
            | Action::DropdownChoose(_) => false,
            Action::Cancel | Action::Undo | Action::Redo => {
                self.cancel_knife();
                true
            }
            _ => {
                self.cancel_knife();
                false
            }
        }
    }
    pub(super) fn knife_key(&mut self, key: &Key) -> bool {
        if self.authoring.knife.is_none() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Backspace) => {
                let k = self.authoring.knife.as_mut().unwrap();
                k.first = None;
                k.press = None;
            }
            Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                self.authoring.knife_angle = if self.authoring.knife_angle == 0 {
                    3
                } else {
                    0
                }
            }
            Key::Named(NamedKey::Enter) => self.accept_knife(),
            Key::Named(NamedKey::Escape) => self.cancel_knife(),
            Key::Character(c)
                if self.modifiers.control_key()
                    && (c.eq_ignore_ascii_case("z") || c.eq_ignore_ascii_case("y")) =>
            {
                self.cancel_knife()
            }
            _ => {}
        }
        self.redraw();
        true
    }
    fn knife_pick(&self) -> Option<EdgePoint> {
        self.smart_knife_pick(self.knife_cursor())
    }
    pub(super) fn knife_click(&mut self) {
        self.smart_knife_click();
    }
    pub(super) fn component_transform(&self, matrix: Mat4) -> Result<MeshEdit, String> {
        let vertices = self.edit_vertices();
        if !self.authoring.proportional {
            return Ok(MeshEdit::Transform { vertices, matrix });
        }
        let node = self.authoring.editing.ok_or("Выберите меш")?;
        let mesh = self.editor.editable_mesh(node).map_err(|e| e.to_string())?;
        let model = self
            .editor
            .project
            .global_transforms()
            .map_err(|e| e.to_string())?[&node];
        Ok(MeshEdit::WeightedTransform {
            weights: poseforge_core::proportional_weights(
                mesh,
                &vertices,
                self.authoring.proportional_radius,
                model,
            ),
            matrix,
        })
    }
    pub(super) fn draw_advanced_overlay(&self, u: &mut Ui) {
        let p = u.palette();
        let vp = self.viewport();
        let project = |point| authoring_controls::project(self.view().view_projection(), vp, point);
        let start = (u.rects.len(), u.texts.len(), u.hits.len());
        if self.authoring.proportional && self.authoring.knife.is_none() {
            if let (Some(node), Some(center), Ok(globals)) = (
                self.authoring.editing,
                self.edit_center(),
                self.editor.project.global_transforms(),
            ) {
                let center = globals[&node].transform_point3(center);
                let normal = (self.view().eye - self.view().target).normalize_or_zero();
                let right = normal.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
                let up = right.cross(normal).normalize_or_zero();
                for i in 0..64 {
                    let point = |j: usize| {
                        let a = j as f32 * std::f32::consts::TAU / 64.;
                        project(
                            center
                                + (right * a.cos() + up * a.sin())
                                    * self.authoring.proportional_radius,
                        )
                    };
                    if let (Some(a), Some(b)) = (point(i), point(i + 1)) {
                        u.mesh_segment(a, b, 1.3, p.accent);
                    }
                }
            }
        }
        if let Some(k) = &self.authoring.knife {
            if let (Ok(mesh), Ok(globals)) = (
                self.editor.editable_mesh(k.node),
                self.editor.project.global_transforms(),
            ) {
                let screen = |e: EdgePoint| {
                    project(
                        globals[&k.node].transform_point3(
                            Vec3::from_array(mesh.vertices[e.edge.0].position)
                                .lerp(Vec3::from_array(mesh.vertices[e.edge.1].position), e.t),
                        ),
                    )
                };
                let hover = self.knife_pick().and_then(screen);
                if let Some(a) = k.press.or_else(|| {
                    k.first
                        .and_then(|p| project(globals[&k.node].transform_point3(p)))
                }) {
                    if let Some(b) = hover.or(Some(self.knife_cursor())) {
                        u.mesh_segment(a, b, 2., p.accent);
                    }
                    u.rect([a[0] - 4., a[1] - 4., 8., 8.], p.accent);
                }
                if let Some(a) = hover {
                    u.rect([a[0] - 3., a[1] - 3., 6., 6.], p.text);
                }
            }
            let area = self.modeling_area();
            let x = area[0];
            let y = area[1] + 48.;
            let w = 270_f32.min(area[2]);
            u.surface([x, y, w, 100.], p.panel);
            u.fitted_label(
                [x + 10., y + 8., w - 20., 20.],
                &format!("НОЖ · разрезов {}", k.cuts),
                12.,
                p.accent,
            );
            u.fitted_label(
                [x + 10., y + 30., w - 20., 18.],
                "Непрерывно · Backspace — новая линия",
                11.,
                p.muted,
            );
            u.button(
                [x + 10., y + 58., (w - 26.) / 2., 30.],
                "Применить",
                Action::Author(A::KnifeAccept),
                true,
            );
            u.button(
                [x + 16. + (w - 26.) / 2., y + 58., (w - 26.) / 2., 30.],
                "Отмена",
                Action::Author(A::KnifeCancel),
                false,
            );
        }
        clip_scrolled(u, start, self.modeling_area(), 0.);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn knife_pointer_preview_accept_and_cancel_share_one_transaction() {
        let mut a = App::new(None);
        a.preferences = Preferences::default();
        a.width = 1280.;
        a.height = 800.;
        let mut node = poseforge_core::Node::new(1, "Knife");
        node.mesh = Some(2);
        a.editor = Editor::new(Project {
            nodes: vec![node],
            meshes: vec![poseforge_core::PrimitiveSettings::new(
                poseforge_core::PrimitiveKind::Plane,
            )
            .mesh(2, None)
            .unwrap()],
            ..Default::default()
        });
        a.editor.mode = EditorMode::Scene;
        a.editor.selected = Some(1);
        a.authoring.editing = Some(1);
        a.authoring.xray = true;
        let original = a.editor.project.clone();
        let mesh = &original.meshes[0];
        let face = mesh.face_vertices(0).unwrap();
        let screen = |j: usize| {
            let p = (Vec3::from_array(mesh.vertices[face[j] as usize].position)
                + Vec3::from_array(mesh.vertices[face[(j + 1) % 4] as usize].position))
                * 0.5;
            authoring_controls::project(a.view().view_projection(), a.viewport(), p).unwrap()
        };
        let points = [screen(0), screen(2)];
        for accept in [false, true] {
            a.action(Action::Author(A::Knife));
            let ui = a.build_ui();
            let panel = ui
                .hits
                .iter()
                .find(|h| h.action == Action::Author(A::KnifeAccept))
                .unwrap()
                .rect;
            let left = a.left_panel_rect();
            assert!(panel[0] >= left[0] + left[2]);
            assert!(panel[0] + panel[2] <= a.right_panel_rect()[0]);
            for p in points {
                a.mouse = p;
                assert!(a.knife_pick().is_some());
                a.knife_click();
            }
            assert_eq!(a.editor.project.meshes[0].face_count(), 2);
            if accept {
                a.action(Action::Author(A::KnifeAccept));
                a.editor.undo();
            } else {
                a.action(Action::Cancel);
            }
            assert_eq!(a.editor.project, original);
            assert!(a.authoring.knife.is_none());
        }
        a.authoring.selection_mode = ui::MeshSelection::Vertex;
        a.authoring.vertices.insert(0);
        a.action(Action::Author(A::Slide));
        assert!(a.modeling_live.is_some());
        a.authoring_confirm(&Action::Author(A::LiveField(0)), "25")
            .unwrap();
        assert_ne!(a.editor.project, original);
        a.action(Action::Author(A::LiveAccept));
        a.editor.undo();
        assert_eq!(a.editor.project, original);
        a.authoring.proportional = true;
        a.authoring.proportional_radius = 2.;
        a.authoring.vertices.insert(0);
        for scale in [1., 2.] {
            a.scale = scale;
            let mut overlay = Ui::with_theme(scale, ui::Theme::Dark, [0.; 2]);
            a.draw_advanced_overlay(&mut overlay);
            assert!(!overlay.rects.is_empty());
            let area = a.modeling_area();
            for r in &overlay.rects {
                assert!(r.rect[0] >= area[0] * scale - 0.01);
                assert!(r.rect[0] + r.rect[2] <= (area[0] + area[2]) * scale + 0.01);
            }
        }
    }
}
