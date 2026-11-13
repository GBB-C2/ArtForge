//! NPR controls reuse material transactions, color sessions and panel geometry.
use super::*;
use poseforge_core::Material;

pub(super) fn npr_color(m: &Material, field: usize) -> Option<[f32; 3]> {
    match field {
        0 => Some(m.npr.shadow_color),
        1 => Some(m.npr.rim_color),
        2 => Some(m.npr.ink_color),
        3 => Some(m.npr.palette_mid),
        4 => Some(m.npr.palette_light),
        _ => None,
    }
}

pub(super) fn set_npr_color(m: &mut Material, field: usize, color: [f32; 3]) {
    match field {
        0 => m.npr.shadow_color = color,
        1 => m.npr.rim_color = color,
        2 => m.npr.ink_color = color,
        3 => m.npr.palette_mid = color,
        4 => m.npr.palette_light = color,
        _ => {}
    }
}

impl App {
    pub(super) fn npr_enabled(&self) -> bool {
        self.selected_material_id()
            .and_then(|id| self.editor.project.material(id))
            .is_some_and(|m| m.npr.enabled)
    }

    pub(super) fn draw_npr_details(&self, u: &mut Ui, x: f32, width: f32, y: f32) {
        let Some(m) = self
            .selected_material_id()
            .and_then(|id| self.editor.project.material(id))
        else {
            return;
        };
        for (field, label, offset) in [
            (0, "Цвет тени", 24.),
            (1, "Цвет края", 64.),
            (2, "Цвет контура", 342.),
        ] {
            u.color_field(
                [x, y + offset, width, 30.],
                label,
                Action::SurfaceNprColor(field),
                npr_color(m, field).unwrap().map(surfaces::linear_to_srgb),
            );
        }
        for (field, label, offset, min, max) in [
            (24, "Сила подсветки края", 108., 0., 2.),
            (25, "Ширина подсветки", 168., 0.01, 1.),
            (26, "Ширина контура", 386., 0., 0.5),
        ] {
            u.number_field(
                [x, y + offset, width, 48.],
                label,
                surfaces::material_number(m, field) as f64,
                Action::SurfaceMaterialNumber(field),
                SliderRange::new(min, max, 0.01),
            );
        }
        let muted = u.palette().muted;
        u.fitted_label(
            [x, y + 240., width, 18.],
            "Цвет тени умножается на основной цвет.",
            10.,
            muted,
        );
        u.fitted_label(
            [x, y + 262., width, 18.],
            "Стекло и PBR-блики доступны без NPR.",
            10.,
            muted,
        );
        u.fitted_label(
            [x, y + 458., width, 18.],
            "0 — выключить контур.",
            10.,
            muted,
        );
        u.fitted_label(
            [x, y + 480., width, 18.],
            "Затемняет край внутри поверхности.",
            10.,
            muted,
        );
        u.fitted_label(
            [x, y + 502., width, 18.],
            "На угловатых мешах видны грани.",
            10.,
            muted,
        );

        for (field, label, offset) in [(3, "Полутон", 892.), (4, "Свет", 932.)] {
            u.color_field(
                [x, y + offset, width, 30.],
                label,
                Action::SurfaceNprColor(field),
                npr_color(m, field).unwrap().map(surfaces::linear_to_srgb),
            );
        }
        for (field, label, offset, min, max, step) in [
            (27, "Сила палитры", 976., 0., 1., 0.01),
            (28, "Положение полутона", 1030., 0.05, 0.95, 0.01),
            (29, "Сила штриховки", 1114., 0., 1., 0.01),
            (30, "Линий на метр", 1168., 1., 200., 1.),
            (31, "Направление, °", 1222., -180., 180., 1.),
            (32, "Перекрёстные штрихи", 1276., 0., 1., 0.01),
        ] {
            u.number_field(
                [x, y + offset, width, 44.],
                label,
                surfaces::material_number(m, field) as f64,
                Action::SurfaceMaterialNumber(field),
                SliderRange::new(min, max, step),
            );
        }
        for (i,label) in ["Cel", "Мягкий", "Тушь", "Комикс"].into_iter().enumerate() {
            let half=(width-6.)*0.5;
            u.button([x+(i%2) as f32*(half+6.),y+1364.+(i/2) as f32*34.,half,28.],label,Action::Media(ui::MediaAction::NprPreset(i as u8)),false);
        }
        for (field,label,offset,max) in [(33,"Мягкое освещение",1440.,1.),(34,"Влияние цвета света",1490.,1.),(35,"Точечный растр",1540.,1.),(36,"Плотность растра",1590.,200.)] {
            u.number_field([x,y+offset,width,44.],label,surfaces::material_number(m,field) as f64,Action::SurfaceMaterialNumber(field),SliderRange::new(if field==36 {1.} else {0.},max,if field==36 {1.} else {0.01}));
        }

    }
}
