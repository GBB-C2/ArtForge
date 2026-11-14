//! Numeric resets use the same validation and undo path as exact value entry.
use super::*;
use crate::ui::{AuthorAction, NormalAction, PrimitiveAction, StudioAction};
use poseforge_core::{CameraLens, FluidDomainSettings, FluidSettings, Light, Material, MaterialPreset,
    PhysicsSettings, PrimitiveSettings, RenderSettings};

impl App {
    fn default_material(&self) -> Material {
        let liquid_material = self.editor.selected.and_then(|id| self.editor.project.node(id))
            .filter(|node| node.fluid.is_some()).and_then(|node| node.mesh)
            .and_then(|id| self.editor.project.mesh(id)).and_then(|mesh| mesh.material);
        if liquid_material.is_some() && liquid_material == self.selected_material_id() {
            MaterialPreset::Water.create()
        } else { Material::default() }
    }

    pub(super) fn number_default(&self, action: &Action) -> Option<f64> {
        let render = RenderSettings::default();
        let fluid = FluidSettings::default();
        let physics = PhysicsSettings::default();
        Some(match *action {
            Action::MeshOpacity => 50.,
            Action::BoneDisplay(ui::BoneDisplayAction::Size(_)) => 1.,
            Action::Fx(ui::FxAction::Field(i))=>fx_controls::number(fx_controls::defaults(self.selected_fx()?.1),i)?,
            Action::Helper(ui::HelperAction::Size)=>1.,
            Action::Helper(ui::HelperAction::Number(i))=>[10.,3.,2.,0.5].get(i as usize).copied()?,
            Action::Path(ui::PathAction::Field(_)) => 0.,
            Action::Media(ui::MediaAction::UvNumber(_,field)) => if field < 2 { 1. } else { 0. },
            Action::Media(ui::MediaAction::VideoNumber(_,field)) => if field == 2 { 1. } else { 0. },
            Action::Morph(ui::MorphAction::Weight(_)) => 0.,
            Action::EditTransform(index) if index < 9 => {
                let t = self.editor.selected.and_then(|id| self.editor.project.node(id))
                    .filter(|node| node.is_joint).and_then(|node| node.rest_transform)
                    .unwrap_or_default();
                if index < 3 { t.translation[index] as f64 }
                else if index < 6 {
                    let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                    [x, y, z][index - 3].to_degrees() as f64
                } else { t.scale[index - 6] as f64 }
            }
            Action::Text(ui::TextAction::Field(field)) => [1.,0.08,1.,0.,3.].get(field as usize).copied()?,
            Action::Curve(ui::CurveAction::Field(field)) => match field {
                0..=2 => 0., 3 => 12., 4 => 0.05, 5 => 8., 6 => 0., _ => return None,
            },
            Action::EditWidth => render.output_width as f64,
            Action::EditHeight => render.output_height as f64,
            Action::EditFps => Project::default().fps as f64,
            Action::EditDuration => Project::default().duration,
            Action::EditFrame => 0.,
            Action::SurfaceMaterialNumber(field @ 37..=49) => {
                let material = self.editor.project.material(self.selected_material_id()?)?;
                poseforge_core::ArtisticSettings::for_style(material.artistic.style)
                    .number(field - 37) as f64
            }
            Action::SurfaceMaterialNumber(field) if field <= 36 =>
                surfaces::material_number(&self.default_material(), field) as f64,
            Action::SurfaceLightNumber(field) if field <= 16 => {
                let mut v = surfaces::light_number(&Light::default(), field);
                // Keep the paired spot angles valid when resetting only one.
                if let Some(light) = self.active_light_index().and_then(|i| self.editor.project.lights.get(i)) {
                    if field == 8 { v = v.min((light.outer_cone_angle.to_degrees() - 0.01).max(0.)); }
                    if field == 9 { v = v.max(light.inner_cone_angle.to_degrees() + 0.01).min(90.); }
                }
                v as f64
            }
            Action::SurfaceRenderNumber(field) => scene_controls::render_number(render, field)? as f64,
            Action::FluidNumber(field) => scene_controls::fluid_number(fluid, field)? as f64,
            Action::Destruction(ui::DestructionAction::Field(field)) => destruction_controls::Settings::default().value(field)?,
            Action::FluidObject(ui::FluidObjectAction::Field(field)) =>
                fluid_object_controls::value(FluidDomainSettings::default(), field)?,
            Action::Primitive(PrimitiveAction::Field(field)) => {
                let s = PrimitiveSettings::new(self.selected_primitive()?.1.kind);
                match field {
                    0..=2 => s.size[field as usize] as f64,
                    3..=5 => s.segments[(field - 3) as usize] as f64,
                    6 => s.radius as f64,
                    10..=12 if s.kind == poseforge_core::PrimitiveKind::Bezier => s.curve_points[self.surface.curve_point.min(3) as usize][(field - 10) as usize] as f64,
                    _ => return None,
                }
            }
            Action::Studio(StudioAction::Field(field)) => (match field {
                0 => self.default_material().transmission,
                1 => self.default_material().thickness,
                2 => self.default_material().attenuation_distance,
                10 => render.atmosphere.density,
                11 => render.atmosphere.height,
                12 => render.atmosphere.distance,
                13 => render.atmosphere.anisotropy,
                20..=22 => fluid.source_center[(field - 20) as usize],
                23..=25 => fluid.source_size[(field - 23) as usize],
                40 => physics.compliance, 41 => physics.damping, 42 => physics.gravity,
                43 => physics.wind, 44 => physics.thickness, 45 => physics.activation,
                46 => physics.contraction, 47 => physics.bending, 48 => physics.friction,
                49 => physics.iterations as f32,
                50 => studio_controls::StudioState::default().bake_frames() as f32,
                60 => 1.,
                _ => return None,
            }) as f64,
            Action::CameraNumber(field) => {
                let lens = CameraLens::default();
                (match field {
                    0 => lens.fov_y_radians.to_degrees(), 1 => lens.ortho_size,
                    2 => lens.depth_of_field.focus_distance, 3 => lens.depth_of_field.f_stop,
                    4..=10 => lens.effects.field(field)?.0,
                    _ => return None,
                }) as f64
            }
            Action::Normal(NormalAction::Field(field)) => mesh_tools_controls::MeshToolsState::default_value(field)?,
            Action::Author(AuthorAction::PaintRadius) => weight_paint::WeightPaint::default().radius as f64,
            Action::Author(AuthorAction::PaintStrength) => weight_paint::WeightPaint::default().strength as f64,
            Action::Author(AuthorAction::Coordinate(_) | AuthorAction::RotateDegrees(_))=>0.,
            Action::Author(AuthorAction::ScaleFactor(_) | AuthorAction::ProportionalRadius)=>1.,
            Action::Author(AuthorAction::SnapStep)=>0.1,
            Action::Author(AuthorAction::ToolField(i))=>*modeling_workflow::DEFAULTS.get(i as usize)? as f64,
            Action::Author(AuthorAction::BridgeSegments)=>1.,
            Action::Author(AuthorAction::BridgeTwist|AuthorAction::BridgeSmooth)=>0.,
            Action::Author(AuthorAction::LoopPosition)=>50.,
            Action::Author(AuthorAction::InsetAmount)=>15.,
            Action::Author(AuthorAction::BevelHeight)=>0.,
            Action::Author(AuthorAction::LiveField(field)) => self.modeling_live.as_ref()?.default_value(field)?,
            Action::ColorPicker(control) => {
                let draft = &self.color_picker.as_ref()?.picker.draft;
                crate::color_picker::ColorDraft::new(draft.original_rgba(), draft.alpha_enabled()).value(control)? as f64
            }
            _ => return None,
        })
    }

    pub(super) fn tint_default_resets(&self, ui: &mut Ui) {
        let p = ui.palette();
        let dim = [p.muted[0] * 0.72, p.muted[1] * 0.72, p.muted[2] * 0.72, p.muted[3]];
        let mut resets = Vec::new();
        for (action, current) in &ui.number_values {
            let Some(default) = self.number_default(action) else { continue; };
            if (current - default).abs() > 1e-6 * default.abs().max(1.) { continue; }
            let reset = Action::ResetNumber(Box::new(action.clone()));
            for hit in ui.hits.iter().filter(|hit| hit.action == reset) {
                resets.push(hit.rect.map(|v| v * ui.scale));
            }
        }
        // Preview thumbnails may contain thousands of paint runs. Scan that
        // list once, and test bounds only for the two actual icon colours.
        for paint in &mut ui.rects {
            if paint.color != p.muted && paint.color != p.accent { continue; }
            let q = paint.rect;
            if resets.iter().any(|r| q[0] >= r[0] && q[1] >= r[1]
                && q[0] + q[2] <= r[0] + r[2] && q[1] + q[3] <= r[1] + r[3]) {
                paint.color = dim;
            }
        }
    }

    pub(super) fn reset_number(&mut self, action: Action) {
        if self.scene_write_pending() { return; }
        let Some(value) = self.number_default(&action) else {
            self.status = "У этого параметра нет значения для сброса.".into();
            self.redraw();
            return;
        };
        match self.apply_field_value(action, &value.to_string()) {
            Ok(()) => {
                self.update_dirty();
                self.invalidate();
            }
            Err(error) => self.status = error,
        }
        self.redraw();
    }
}
