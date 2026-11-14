//! Floating tool properties and explicit component construction settings.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S, ModelingTool as M};
pub(super) const DEFAULTS: [f32; 10] = [0., 0., 3., 1., 0., 0., 0., 0., 0., 1.];
pub(super) struct State {
    pub values: [f32; 10],
    pub extrude_mode: u8,
    pub panel_position: Option<[f32; 2]>,
    pub collapsed: bool,
    pub close_gap: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            values: DEFAULTS,
            extrude_mode: 0,
            panel_position: None,
            collapsed: false,
            close_gap: false,
        }
    }
}
impl App {
    pub(super) fn workflow_preference_field(a: &Action) -> bool {
        if *a == Action::MeshOpacity { return true; }
        matches!(
            a,
            Action::Author(
                A::ToolField(_)
                    | A::ProportionalRadius
                    | A::SnapStep
                    | A::BridgeSegments
                    | A::BridgeTwist
                    | A::BridgeSmooth
                    | A::LoopPosition
                    | A::InsetAmount
                    | A::BevelHeight
            )
        )
    }
    pub(super) fn workflow_action(&mut self, a: A) -> bool {
        match a {
            A::ToolField(i) => {
                if let Some(v) = self.authoring.workflow.values.get(i as usize) {
                    self.input(Action::Author(a), ui::number_caption(*v as f64));
                }
            }
            A::ExtrudeMode(mode) => self.change_extrude_mode(mode.min(3)),
            A::ToolPanelToggle => {
                self.authoring.workflow.collapsed = !self.authoring.workflow.collapsed
            }
            A::ToolPanelDrag => {
                if let Some(r) = self.tool_panel_rect() {
                    self.drag = Some(Drag::ToolPanel {
                        start: self.mouse,
                        origin: [r[0], r[1]],
                        original: self.authoring.workflow.panel_position,
                    });
                    self.drag_button = Some(MouseButton::Left);
                }
            }
            A::ToolArray => {
                let Some(id) = self.authoring.editing else {
                    return true;
                };
                let v = self.authoring.workflow.values;
                let result = self.editor.edit_mesh(
                    id,
                    poseforge_core::MeshEdit::Array {
                        region: self.mesh_region(),
                        count: v[2] as u32,
                        offset: Vec3::new(v[3], v[4], v[5]),
                        rotation: Vec3::new(v[6], v[7], v[8]),
                    },
                );
                if self.result(result).is_some() {
                    self.authoring.diagnostics = None;
                    self.invalidate_geometry();
                    self.status = format!("Массив · {} копии · Ctrl+Z — отменить", v[2] as u32);
                }
            }
            _ => return false,
        }
        true
    }
    pub(super) fn workflow_confirm(&mut self, a: &Action, text: &str) -> Result<bool, String> {
        let Action::Author(A::ToolField(i)) = *a else {
            return Ok(false);
        };
        let (min, max) = match i {
            0 | 1 => (0., 1000.),
            2 => (2., 128.),
            9 => (1.,16.),
            3..=5 => (-10000., 10000.),
            6..=8 => (-3600., 3600.),
            _ => return Err("Неизвестный параметр".into()),
        };
        let value = surfaces::number(text, min, max)?;
        if matches!(i,2|9) && value.fract() != 0. {
            return Err("Количество копий должно быть целым".into());
        }
        let previous=self.authoring.workflow.values[i as usize];
        self.authoring.workflow.values[i as usize] = value;
        if matches!(i,0|1|9) {
            if let Err(e)=self.preview_knife_geometry() { self.authoring.workflow.values[i as usize]=previous; return Err(e); }
        }
        Ok(true)
    }
    pub(super) fn workflow_bridge_edges(&self) -> BTreeSet<(usize, usize)> {
        if self.authoring.selection_mode == S::Edge {
            return self.authoring.edges.clone();
        }
        let Some(mesh) = self
            .authoring
            .editing
            .and_then(|id| self.editor.editable_mesh(id).ok())
        else {
            return BTreeSet::new();
        };
        let g = poseforge_core::MeshConnectivity::new(mesh);
        let selected: BTreeSet<_> = self
            .authoring
            .vertices
            .iter()
            .map(|&v| g.canonical[v])
            .collect();
        g.edges
            .iter()
            .copied()
            .filter(|e| selected.contains(&e.0) && selected.contains(&e.1))
            .collect()
    }
    pub(super) fn draw_workflow_field(&self, u: &mut Ui, x: f32, y: f32, w: f32, i: u8, h: f32) {
        let (label, min, max, step): (&str, f64, f64, f64) = match i {
            0 | 1 => ("Зазор, м", 0., 1., 0.001),
            9 => ("Количество линий",1.,16.,1.),
            2 => ("Копии, включая исходную", 2., 32., 1.),
            3 => ("Смещение X, м", -10., 10., 0.01),
            4 => ("Смещение Y, м", -10., 10., 0.01),
            5 => ("Смещение Z, м", -10., 10., 0.01),
            6 => ("Поворот X, °", -180., 180., 1.),
            7 => ("Поворот Y, °", -180., 180., 1.),
            _ => ("Поворот Z, °", -180., 180., 1.),
        };
        let v = self.authoring.workflow.values[i as usize] as f64;
        u.number_field(
            [x, y, w, h],
            label,
            v,
            Action::Author(A::ToolField(i)),
            SliderRange::new(min.min(v), max.max(v), step),
        );
    }
    pub(super) fn draw_array_properties(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        for i in 2..9 {
            self.draw_workflow_field(u, x, y, w, i, 36.);
            y += 38.;
        }
        if !self.edit_vertices().is_empty() {
            u.button(
                [x, y, w, 30.],
                "Создать массив",
                Action::Author(A::ToolArray),
                false,
            );
        } else {
            u.disabled_button([x, y, w, 30.], "Выберите компоненты");
        }
        u.fitted_label(
            [x, y + 34., w, 18.],
            "Смещение и поворот на каждую копию",
            10.,
            u.palette().muted,
        );
    }
    pub(super) fn draw_extrude_modes(&self, u: &mut Ui, x: f32, y: f32, w: f32) {
        u.fitted_label(
            [x, y, w, 18.],
            "Режим экструдирования",
            11.,
            u.palette().muted,
        );
        let width = (w - 4.) / 2.;
        for (i, label) in ["Область", "По нормалям", "Отдельно", "Распределительное"].into_iter().enumerate()
        {
            u.quiet_button(
                [x + (i%2) as f32 * (width + 4.), y + 22. + (i/2) as f32*32., width, 28.],
                label,
                Action::Author(A::ExtrudeMode(i as u8)),
                self.authoring.workflow.extrude_mode == i as u8,
            );
        }
    }
    pub(super) fn tool_panel_rect(&self) -> Option<[f32; 4]> {
        if self.authoring.editing.is_none()
            || self.authoring.modeling_tool.is_none()
            || (self.has_modal() && self.dropdown.is_none())
        {
            return None;
        }
        let area = self.modeling_area();
        if area[2] < 180. || area[3] < 60. {
            return None;
        }
        let w = 310_f32.min(area[2]);
        let h = if self.authoring.workflow.collapsed {
            38.
        } else {
            (self.modeling_panel_bottom() - self.modeling_properties_start()
                + 16.)
            .min(area[3])
        };
        let mut default = [area[0], area[1] + area[3] - h];
        if let Some(anchor) = self.modeling_panel_anchor() {
            let grip = [anchor[0] - 72., anchor[1] - 72., 144., 144.];
            let right = area[0] + area[2] - w;
            let bottom = area[1] + area[3] - h;
            let candidates = [
                default,
                [area[0], area[1]],
                [right, bottom],
                [right, area[1]],
            ];
            let overlap =
                |p: [f32; 2]| intersect_rect([p[0], p[1], w, h], grip).map_or(0., |r| r[2] * r[3]);
            for candidate in candidates {
                if overlap(candidate) < overlap(default) {
                    default = candidate;
                }
            }
        }
        let pos = self.authoring.workflow.panel_position.unwrap_or(default);
        Some([
            pos[0].clamp(area[0], (area[0] + area[2] - w).max(area[0])),
            pos[1].clamp(area[1], (area[1] + area[3] - h).max(area[1])),
            w,
            h,
        ])
    }
    pub(super) fn update_tool_panel_drag(
        &mut self,
        start: [f32; 2],
        origin: [f32; 2],
        original: Option<[f32; 2]>,
    ) {
        self.authoring.workflow.panel_position = Some([
            origin[0] + self.mouse[0] - start[0],
            origin[1] + self.mouse[1] - start[1],
        ]);
        if let Some(r) = self.tool_panel_rect() {
            self.authoring.workflow.panel_position = Some([r[0], r[1]]);
        }
        self.drag = Some(Drag::ToolPanel {
            start,
            origin,
            original,
        });
    }
    pub(super) fn draw_tool_panel(&self, u: &mut Ui) {
        let Some([x, y, w, h]) = self.tool_panel_rect() else {
            return;
        };
        let p = u.palette();
        let mut bg = p.panel;
        bg[3] = 0.82;
        u.outlined_surface([x, y, w, h], bg, p.border);
        u.hits.push(ui::Hit {
            rect: [x, y, w, h],
            action: Action::Author(A::LiveBackdrop),
        });
        u.hits.push(ui::Hit {
            rect: [x, y, w - 38., 36.],
            action: Action::Author(A::ToolPanelDrag),
        });
        if self.authoring.workflow.collapsed {
            u.fitted_label(
                [x + 12., y + 9., w - 120., 20.],
                self.authoring.modeling_tool.unwrap().title(),
                12.,
                p.text,
            );
        } else {
            let start = (u.rects.len(), u.texts.len(), u.hits.len());
            self.draw_modeling_tool_properties(u, x + 12., y + 5., w - 24.);
            clip_scrolled(u, start, [x + 1., y + 1., w - 2., h - 2.], 0.);
        }
        if self.authoring.workflow.collapsed
            && (self.modeling_live.is_some() || self.authoring.knife.is_some())
        {
            let knife = self.authoring.knife.is_some();
            u.quiet_icon_button(
                [x + w - 102., y + 3., 30., 30.],
                "",
                Action::Author(if knife { A::KnifeAccept } else { A::LiveAccept }),
                false,
                Icon::Check,
            );
            u.quiet_icon_button(
                [x + w - 68., y + 3., 30., 30.],
                "",
                Action::Author(if knife { A::KnifeCancel } else { A::LiveCancel }),
                false,
                Icon::Close,
            );
        }
        u.disclosure_button(
            [x + w - 34., y + 3., 30., 30.],
            Some(Action::Author(A::ToolPanelToggle)),
            !self.authoring.workflow.collapsed,
        );
    }
    pub(super) fn draw_tool_tooltip(&self, u: &mut Ui) {
        if self.drag.is_some() || self.has_modal() || self.menu.is_some() || self.input.is_some() {
            return;
        }
        let Some(Action::ModelingTool(tool)) = u.hit(self.mouse) else {
            return;
        };
        let detail = match tool {
            M::Extrude => "Область, по нормалям или отдельные грани",
            M::LoopCut => "Наведите на ребро и нажмите · Enter завершает",
            M::Knife => "Потяните линию · Shift — угол · Enter завершает",
            M::Bridge => "Соединяет грани, контуры и цепочки рёбер",
            M::Dissolve => "Убирает компоненты, сохраняя поверхность",
            M::FillHoles => "Выберите один элемент у края отверстия",
            M::Array => "Копирует выделение с шагом и поворотом",
            _ => "Выберите инструмент — свойства в панели",
        };
        let w = 310_f32.min(self.width - 24.);
        let x = (self.mouse[0] - w - 12.).max(12.);
        let y = (self.mouse[1] + 16.).min(self.height - 74.);
        let p = u.palette();
        u.outlined_surface([x, y, w, 56.], p.elevated, p.border);
        u.fitted_label([x + 10., y + 7., w - 20., 20.], tool.title(), 12., p.text);
        u.fitted_label([x + 10., y + 30., w - 20., 18.], detail, 10., p.muted);
    }
}

#[derive(Clone)]
pub(super) struct ComponentSlider {
    node: u64,
    source: poseforge_core::Mesh,
    operation: poseforge_core::MeshEdit,
    center: Vec3,
    pivot: Vec3,
}
impl App {
    pub(super) fn component_slider_source(&self, action: &Action) -> Option<ComponentSlider> {
        if !matches!(
            action,
            Action::Author(A::Coordinate(_) | A::RotateDegrees(_) | A::ScaleFactor(_))
        ) {
            return None;
        }
        let node = self.authoring.editing?;
        let source = self.editor.editable_mesh(node).ok()?.clone();
        let center = self.edit_center()?;
        Some(ComponentSlider {
            node,
            source,
            operation: if matches!(action, Action::Author(A::Coordinate(_))) {
                poseforge_core::MeshEdit::Transform {
                    vertices: self.edit_vertices(),
                    matrix: Mat4::IDENTITY,
                }
            } else {
                self.component_transform(Mat4::IDENTITY).ok()?
            },
            center,
            pivot: if self.authoring.pivot_origin {
                Vec3::ZERO
            } else {
                center
            },
        })
    }
    pub(super) fn preview_component_slider(
        &mut self,
        s: &ComponentSlider,
        action: &Action,
        value: f64,
    ) -> Result<(), String> {
        let Action::Author(command) = *action else {
            return Err("Неизвестное поле".into());
        };
        let matrix = match command {
            A::Coordinate(axis) => {
                let mut delta = Vec3::ZERO;
                delta[axis.min(2) as usize] = value as f32 - s.center[axis.min(2) as usize];
                Mat4::from_translation(delta)
            }
            A::RotateDegrees(axis) => {
                Mat4::from_translation(s.pivot)
                    * Mat4::from_axis_angle(
                        [Vec3::X, Vec3::Y, Vec3::Z][axis.min(2) as usize],
                        (value as f32).to_radians(),
                    )
                    * Mat4::from_translation(-s.pivot)
            }
            A::ScaleFactor(axis) => {
                let mut scale = Vec3::splat(value as f32);
                if axis < 3 {
                    scale = Vec3::ONE;
                    scale[axis as usize] = value as f32;
                }
                Mat4::from_translation(s.pivot)
                    * Mat4::from_scale(scale)
                    * Mat4::from_translation(-s.pivot)
            }
            _ => return Err("Неизвестное поле".into()),
        };
        let mut operation = s.operation.clone();
        match &mut operation {
            poseforge_core::MeshEdit::Transform { matrix: m, .. }
            | poseforge_core::MeshEdit::WeightedTransform { matrix: m, .. } => *m = matrix,
            _ => return Err("Недоступная трансформация".into()),
        };
        self.editor
            .preview_mesh(s.node, &s.source, operation)
            .map_err(|e| e.to_string())?;
        self.invalidate_geometry();
        Ok(())
    }
    pub(super) fn component_slider_value(&self, command: A, default: f64) -> f64 {
        if let Some(Drag::Slider(s)) = &self.drag {
            if s.action == Action::Author(command) {
                return s.last_value;
            }
        }
        default
    }
}
