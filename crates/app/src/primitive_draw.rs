//! Shift creation: drag a footprint, then set height with a second click.
use super::*;
use poseforge_core::{Material, Node, PrimitiveKind as K, PrimitiveSettings};
#[derive(Clone)]
pub(super) struct Draw {
    selected: Option<u64>,
    selection: BTreeSet<u64>,
    settings: PrimitiveSettings,
    id: u64,
    start: [f32; 2],
    anchor: Vec3,
    u: Vec3,
    v: Vec3,
    normal: Vec3,
    width: f32,
    depth: f32,
    center: Vec3,
    height: f32,
    pixels: f32,
    moved: bool,
}
impl App {
    pub(super) fn construction_point(&self, origin: Vec3, normal: Vec3) -> Option<Vec3> {
        let (eye, ray) = crate::gizmo::pointer_ray(&self.view(), self.viewport(), self.mouse)?;
        let d = ray.dot(normal);
        if d.abs() < 1e-6 {
            return None;
        }
        let t = (origin - eye).dot(normal) / d;
        let p = eye + ray * t;
        (t >= 0. && p.is_finite() && p.abs().max_element() < 10000.).then_some(p)
    }
    pub(super) fn surface_anchor(&self) -> Option<(Vec3, Vec3)> {
        let eval = self.evaluated_interactive().ok()?;
        let hit = crate::picking::pick_surface(
            &self.picking_project(),
            &eval,
            self.view().view_projection(),
            self.viewport(),
            self.mouse,
        )?;
        let n = self.editor.project.node(hit.node)?;
        let mesh = eval
            .modifier_meshes
            .get(&hit.node)
            .or_else(|| self.editor.project.mesh(n.mesh?))?;
        let face = mesh.face_vertices(hit.face)?;
        let model = *eval.global.get(&hit.node)?;
        let p = |i: usize| {
            model.transform_point3(Vec3::from_array(mesh.vertices[face[i] as usize].position))
        };
        let normal = (1..face.len() - 1)
            .map(|i| (p(i) - p(0)).cross(p(i + 1) - p(0)))
            .sum::<Vec3>()
            .try_normalize()?;
        Some((hit.position, normal))
    }
    pub(super) fn primitive_draw_down(&mut self) -> bool {
        if let Some(draw) = self.primitive_height.take() {
            self.finish_primitive_draw(draw);
            return true;
        }
        if !self.modifiers.shift_key()
            || self.authoring.editing.is_some()
            || self.editor.mode != EditorMode::Scene
            || self.editor.playing
            || self.job.is_some()
            || self.editor.has_staged()
        {
            return false;
        }
        let kind = self.primitive_draw_kind.or_else(|| {
            self.selected_primitive()
                .filter(|_| self.primitive_panel.is_some())
                .map(|(_, s)| s.kind)
        });
        let Some(kind) = kind else {
            return false;
        };
        if matches!(kind, K::Bezier | K::CurveCircle) {
            return false;
        }
        let Some((eye, ray)) = crate::gizmo::pointer_ray(&self.view(), self.viewport(), self.mouse)
        else {
            return true;
        };
        let (anchor, normal) = self.surface_anchor().unwrap_or_else(|| {
            let normal = if ray.y.abs() > 0.05 { Vec3::Y } else { -ray };
            (
                self.construction_point(self.target, normal)
                    .unwrap_or(eye + ray * self.distance),
                normal,
            )
        });
        let base = if normal.dot(Vec3::X).abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Z
        };
        let u = (base - normal * base.dot(normal)).normalize();
        let v = u.cross(normal).normalize();
        self.cancel_modeling_session();
        self.cancel_modeling_live();
        self.cancel_drag();
        self.editor.begin_edit("Нарисовать примитив");
        let id = self.editor.project.next_id();
        let settings = PrimitiveSettings::new(kind);
        let draw = Draw {
            selected: self.editor.selected,
            selection: self.scene_tools.selected.clone(),
            settings,
            id,
            start: self.mouse,
            anchor,
            u,
            v,
            normal,
            width: 0.01,
            depth: 0.01,
            center: anchor,
            height: 0.01,
            pixels: (self.distance * 0.002).max(0.0001),
            moved: false,
        };
        self.drag = Some(Drag::PrimitiveDraw(draw));
        self.drag_button = Some(MouseButton::Left);
        self.status =
            "Тяните основание · отпустите ЛКМ, затем задайте высоту · Esc — отменить".into();
        true
    }
    fn primitive_draw_preview(&mut self, d: &Draw) {
        let mut settings = d.settings;
        settings.size = [d.width, d.height, d.depth];
        if matches!(settings.kind, K::Plane | K::Cloth) {
            settings.size = [d.width, d.depth, 1.];
        }
        if matches!(
            settings.kind,
            K::Cylinder | K::Cone | K::Capsule | K::Sphere
        ) {
            settings.radius = d.width.max(d.depth) * 0.5;
        }
        if settings.kind == K::Capsule {
            settings.size[1] = settings.size[1].max(settings.radius * 2.);
        }
        if settings.kind == K::Torus {
            settings.radius = d.width.max(d.depth) * 0.5;
            settings.size[0] = (settings.radius * 0.2).max(0.001);
        }
        let Ok(mesh) = settings.mesh(d.id + 2, Some(d.id + 1)) else {
            return;
        };
        let mut n = Node::new(d.id, settings.label());
        n.mesh = Some(mesh.id);
        // Cloth lives in XY; the plane and volumes use local Y as their normal/height.
        let rotation = if settings.kind == K::Cloth {
            glam::Mat3::from_cols(d.u, -d.v, d.normal)
        } else {
            glam::Mat3::from_cols(d.u, d.normal, d.v)
        };
        n.transform.rotation = Quat::from_mat3(&rotation);
        n.transform.translation = d.center
            + if matches!(settings.kind, K::Plane | K::Cloth) {
                Vec3::ZERO
            } else {
                d.normal * (d.height * 0.5)
            };
        if settings.kind == K::Cloth {
            n.physics = Some(Default::default());
            n.collision_targets.cloth = Some(vec![]);
        }
        let material = Material {
            id: d.id + 1,
            name: "Материал меша".into(),
            double_sided: matches!(settings.kind, K::Plane | K::Cloth),
            ..Default::default()
        };
        let result = self.editor.transact("Нарисовать примитив", |p| {
            if let Some(slot) = p.nodes.iter_mut().find(|n| n.id == d.id) {
                *slot = n;
            } else {
                p.nodes.push(n);
                p.materials.push(material);
            }
            if let Some(slot) = p.meshes.iter_mut().find(|m| m.id == mesh.id) {
                *slot = mesh;
            } else {
                p.meshes.push(mesh);
            }
        });
        if self.result(result).is_some() {
            self.editor.selected = Some(d.id);
            self.invalidate_geometry();
        }
    }
    pub(super) fn update_primitive_draw(&mut self, mut d: Draw) {
        if let Some(point) = self.construction_point(d.anchor, d.normal) {
            let delta = point - d.anchor;
            let x = delta.dot(d.u);
            let z = delta.dot(d.v);
            d.width = x.abs().clamp(0.001, 1000.);
            d.depth = z.abs().clamp(0.001, 1000.);
            d.height = d.width.max(d.depth);
            d.center = d.anchor + d.u * (x * 0.5) + d.v * (z * 0.5);
            d.moved |= (self.mouse[0] - d.start[0]).hypot(self.mouse[1] - d.start[1]) > 3.;
            self.primitive_draw_preview(&d);
        }
        self.drag = Some(Drag::PrimitiveDraw(d));
    }
    pub(super) fn primitive_base_up(&mut self, mut d: Draw) {
        if !d.moved {
            self.cancel_primitive_draw(&d);
            return;
        }
        if matches!(d.settings.kind, K::Plane | K::Cloth | K::Sphere | K::Torus) {
            self.finish_primitive_draw(d);
        } else {
            d.start = self.mouse;
            self.primitive_height = Some(d);
            self.status =
                "Двигайте мышь вверх/вниз — высота · ЛКМ/Enter — создать · Esc — отменить".into();
        }
    }
    pub(super) fn primitive_height_move(&mut self) {
        let Some(mut d) = self.primitive_height.take() else {
            return;
        };
        d.height = ((d.start[1] - self.mouse[1]) * d.pixels)
            .abs()
            .clamp(0.001, 1000.);
        self.primitive_draw_preview(&d);
        self.primitive_height = Some(d);
    }
    pub(super) fn finish_primitive_draw(&mut self, d: Draw) {
        let r = self.editor.commit_edit();
        if self.result(r).is_some() {
            self.editor.selected = Some(d.id);
            self.scene_tools.selected = BTreeSet::from([d.id]);
            self.primitive_panel = Some(d.id);
            self.primitive_collapsed = false;
            self.primitive_draw_kind = None;
            self.tool = Tool::Move;
            self.status = "Примитив создан · Shift + ЛКМ — нарисовать следующий".into();
        }
        self.invalidate_geometry();
        self.update_dirty();
    }
    pub(super) fn cancel_primitive_draw(&mut self, d: &Draw) {
        self.editor.cancel_edit();
        self.editor.selected = d.selected;
        self.scene_tools.selected = d.selection.clone();
        self.invalidate_geometry();
    }
    pub(super) fn route_primitive_draw(&mut self, a: &Action) -> bool {
        if self.primitive_height.is_none() {
            if *a == Action::Cancel && self.primitive_draw_kind.take().is_some() {
                return true;
            }
            return false;
        }
        if matches!(a, Action::Cancel | Action::Undo | Action::Redo) {
            let d = self.primitive_height.take().unwrap();
            self.cancel_primitive_draw(&d);
            return true;
        }
        if *a == Action::ConfirmInput {
            let d = self.primitive_height.take().unwrap();
            self.finish_primitive_draw(d);
            return true;
        }
        if Self::preview_navigation_action(a) {
            return false;
        }
        let d = self.primitive_height.take().unwrap();
        self.cancel_primitive_draw(&d);
        false
    }
}
