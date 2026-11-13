//! Paint and paper controls for material presets, using the normal edit/undo path.
use super::*;
use crate::ui::{Action, SliderRange, Ui};

pub(super) const PAINT_CONTROLS_HEIGHT: f32 = 304.;

impl App {
    pub(super) fn artistic_enabled(&self) -> bool {
        self.selected_material_id().and_then(|id| self.editor.project.material(id))
            .is_some_and(|m| m.artistic.enabled)
    }
    pub(super) fn draw_paint_controls(&self, u: &mut Ui, x: f32, width: f32, y: f32) {
        let Some(m) = self.selected_material_id().and_then(|id| self.editor.project.material(id)) else { return; };
        let a = m.artistic;
        let muted = u.palette().muted;
        if !a.enabled { return; }
        for (row,(label,field)) in [
            ("Плотность пигмента",44), ("Фактура бумаги",45), ("Пигмент по краям",46),
            ("Неоднородность заливки",47), ("Сухая кисть",48),
        ].into_iter().enumerate() {
            surfaces::numeric(u,[x,y+8.+row as f32*40.,width,30.],label,
                surfaces::material_number(m,field),Action::SurfaceMaterialNumber(field),SliderRange::new(0.,1.,0.01));
        }
        surfaces::numeric(u,[x,y+212.,width,30.],"Вариант фактуры",a.seed as f32,
            Action::SurfaceMaterialNumber(49),SliderRange::new(0.,65535.,1.));
        u.fitted_label([x,y+254.,width,18.], "Краска и бумага: акварель, гуашь, пастель.", 10., muted);
        u.fitted_label([x,y+276.,width,18.], "Акварель — без растекания пигмента.", 10., muted);
    }
}
