//! Tool palette and complete settings in the Geometry inspector.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S, ModelingTool as M, ShapeAction};

pub(super) const TOOLS_SECTION: u16 = 740;
pub(super) const PROPERTIES_SECTION: u16 = 741;
const TOOL_PITCH: f32 = 48.;
const TOOL_SIZE: f32 = 44.;
const TOOLS: [M; 32] = [
    M::Select(0), M::Select(1), M::Select(2), M::Move, M::Rotate, M::Scale,
    M::Extrude, M::Inset, M::Bevel, M::LoopCut, M::Knife, M::Slide,
    M::Merge, M::Bridge, M::Dissolve, M::DeleteFaces, M::Subdivide, M::FillHoles, M::Triangulate, M::Separate,
    M::Smooth, M::Flat, M::Align, M::Relax,
    M::Sphere, M::Circle, M::Twist, M::Array, M::Mirror, M::Boolean, M::Cable, M::Inspect,
];

impl M {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Select(1) => "Выделение рамкой",
            Self::Select(2) => "Выделение лассо",
            Self::Select(_) => "Выделение щелчком",
            Self::Move => "Перемещение · G", Self::Rotate => "Вращение · R",
            Self::Scale => "Масштаб · S", Self::Extrude => "Экструдирование · E",
            Self::ExtrudeIndividual=>"Индивидуальный экструд", Self::FillHoles=>"Заполнить отверстия", Self::Triangulate=>"Триангулировать выделение",Self::Separate=>"Отделить выбранное",
            Self::Inset => "Вставка граней · I", Self::Bevel => "Фаска · B",
            Self::LoopCut => "Разрез · C",
            Self::Knife => "Нож · K", Self::Slide => "Сдвиг по поверхности · J",
            Self::Merge => "Объединить в центре", Self::Weld => "Сварка вершин",
            Self::Bridge => "Мост", Self::Dissolve => "Растворить", Self::Array=>"Массив",
            Self::BridgeFaces => "Мост из граней",
            Self::DeleteFaces => "Удалить", Self::Subdivide => "Плотность сетки",
            Self::Smooth => "Сгладить нормали", Self::Flat => "Плоские нормали",
            Self::Align => "Выравнивание", Self::Relax => "Релакс сетки",
            Self::Sphere => "Сферизация", Self::Circle => "Окружность",
            Self::Inflate => "Смещение по нормалям", Self::Twist => "Скручивание",
            Self::Inspect => "Проверить топологию", Self::Mirror=>"Зеркало · Mirror", Self::Boolean=>"Булева операция", Self::Cable=>"Кабель",
        }
    }
    pub(super) fn icon(self, simplify: bool) -> Icon {
        match self {
            Self::Select(1) => Icon::SelectBox, Self::Select(2) => Icon::SelectLasso,
            Self::Select(_) => Icon::SelectPointer,
            Self::Move => Icon::Move, Self::Rotate => Icon::Rotate, Self::Scale => Icon::Scale,
            Self::Mirror=>Icon::Mirror,Self::Boolean=>Icon::Boolean,Self::Cable=>Icon::Cable, Self::Array=>Icon::ModelArray, Self::ExtrudeIndividual=>Icon::ExtrudeIndividual,Self::FillHoles=>Icon::FillHoles,Self::Triangulate=>Icon::Triangulate,Self::Separate=>Icon::Separate,
            Self::Extrude => Icon::Extrude, Self::Inset => Icon::Inset,
            Self::Bevel => Icon::Bevel,
            Self::LoopCut => Icon::LoopCut, Self::Knife => Icon::Knife, Self::Slide => Icon::SlideEdge,
            Self::Merge => Icon::Merge, Self::Weld => Icon::Weld, Self::Bridge | Self::BridgeFaces => Icon::Bridge,
            Self::Dissolve => Icon::Dissolve, Self::DeleteFaces => Icon::DeleteFaces,
            Self::Subdivide if simplify => Icon::Simplify,
            Self::Subdivide => Icon::Subdivide, Self::Smooth => Icon::SmoothNormals, Self::Flat => Icon::FlatNormals,
            Self::Sphere => Icon::ModelSphere, Self::Circle => Icon::ModelCircle,
            Self::Inflate => Icon::Inflate, Self::Twist => Icon::Twist,
            Self::Align => Icon::Flatten, Self::Relax => Icon::Relax, Self::Inspect => Icon::InspectMesh,
        }
    }
    pub(super) fn command(self) -> Option<Action> {
        Some(match self {
            Self::Select(i) => Action::Author(A::SelectTool(i)),
            Self::Move => Action::Move, Self::Rotate => Action::Rotate, Self::Scale => Action::Scale,
            Self::Mirror=>Action::Author(A::Mirror),Self::Boolean=>Action::Construction(ui::ConstructionAction::Boolean),Self::Cable=>Action::Construction(ui::ConstructionAction::Cable), Self::Array=>Action::Author(A::ToolArray), Self::ExtrudeIndividual=>Action::Author(A::ExtrudeIndividual),Self::FillHoles=>Action::Author(A::FillHoles),Self::Triangulate=>Action::Author(A::Triangulate),Self::Separate=>Action::Author(A::Separate),
            Self::Extrude => Action::Author(A::Extrude), Self::Inset => Action::Author(A::Inset),
            Self::Bevel => Action::Author(A::Bevel),
            Self::LoopCut => Action::Author(A::LoopCut), Self::Knife => Action::Author(A::Knife),
            Self::Slide => Action::Author(A::Slide), Self::Merge => Action::Author(A::Merge),
            Self::Weld => Action::Author(A::Weld), Self::Bridge => Action::Author(A::Bridge),
            Self::BridgeFaces => Action::Author(A::BridgeFaces),
            Self::Dissolve => Action::Author(A::Dissolve), Self::DeleteFaces => Action::Author(A::DeleteFaces),
            Self::Subdivide => Action::Author(A::Subdivide), Self::Smooth => Action::Author(A::Smooth),
            Self::Flat => Action::Author(A::Flat), Self::Relax => Action::Shape(ShapeAction::Relax),
            Self::Sphere => Action::Shape(ShapeAction::Sphere), Self::Circle => Action::Shape(ShapeAction::Circle),
            Self::Inflate => Action::Shape(ShapeAction::Inflate), Self::Twist => Action::Shape(ShapeAction::Twist),
            Self::Inspect => Action::Author(A::Inspect), Self::Align => return None,
        })
    }
    pub(super) fn live(self) -> bool {
        matches!(self, Self::Bridge | Self::BridgeFaces | Self::Extrude | Self::ExtrudeIndividual | Self::Inset | Self::Bevel | Self::LoopCut | Self::Slide | Self::Array)
    }
}

impl App {
    pub(super) fn modeling_apply_action(&self, action: &Action) -> bool {
        self.authoring.modeling_tool.is_some_and(|tool| !tool.live()
            && !matches!(tool,M::Select(_)|M::Move|M::Rotate|M::Scale|M::Inspect|M::Knife)
            && (tool.command().as_ref()==Some(action) || (tool==M::Align && matches!(action,Action::Shape(ShapeAction::Flatten(_) | ShapeAction::Straighten | ShapeAction::Space)))))
    }
    pub(super) fn remember_modeling_tool(&mut self, action: &Action) {
        if self.authoring.editing.is_none() { return; }
        let tool = match action {
            Action::Author(A::EdgeBevel) => Some(M::Bevel),
            Action::Shape(ShapeAction::Flatten(_)) => {self.mesh_tools.alignment_mode=0;Some(M::Align)},
            Action::Shape(ShapeAction::Straighten) => {self.mesh_tools.alignment_mode=1;Some(M::Align)},
            Action::Shape(ShapeAction::Space) => {self.mesh_tools.alignment_mode=2;Some(M::Align)},
            Action::Author(A::SelectDefects) => Some(M::Inspect),
            Action::Author(A::Proportional | A::ProportionalRadius) if !matches!(self.authoring.modeling_tool, Some(M::Move | M::Rotate | M::Scale)) => Some(M::Move),
            _ => TOOLS.into_iter().find(|tool| tool.command().as_ref() == Some(action)),
        };
        if let Some(tool) = tool {
            if self.authoring.modeling_tool != Some(tool) {
                self.authoring.workflow.panel_scroll = 0.;
                self.authoring.workflow.collapsed = false;
            }
            self.authoring.modeling_tool = Some(tool);
        }
    }
    pub(super) fn choose_modeling_tool(&mut self, tool: M) {
        if matches!(tool,M::Boolean|M::Cable){self.start_construction(tool==M::Cable);return;}
        if self.authoring.editing.is_none() || self.job.is_some() || self.editor.playing
            || self.scene_write_pending() || self.editor.has_staged() { return; }
        if self.authoring.modeling_tool == Some(tool) && self.authoring.workflow.session.is_some() {
            self.authoring.workflow.collapsed = false;
            self.show_modeling_properties();
            return;
        }
        self.cancel_modeling_session();
        self.authoring.modeling_tool = Some(tool);
        self.authoring.workflow.panel_scroll = 0.;
        self.authoring.workflow.collapsed = false;
        self.authoring.workflow.pending_preview = (tool.live() && tool != M::LoopCut).then_some(tool);
        if !tool.live() && tool != M::Knife {
            self.begin_modeling_session(tool);
        } else if let Some(action) = tool.command() {
            if matches!(tool, M::Select(_) | M::Move | M::Rotate | M::Scale | M::Knife | M::LoopCut | M::Inspect)
                || tool.live() {
                self.action(action);
            }
        }
        self.show_modeling_properties();
    }
    pub(super) fn start_pending_modeling_preview(&mut self) {
        self.start_pending_modeling_session();
        if self.modifiers.shift_key() { return; }
        let Some(tool) = self.authoring.workflow.pending_preview else { return; };
        if self.authoring.modeling_tool != Some(tool) {
            self.authoring.workflow.pending_preview = None;
            return;
        }
        if self.modeling_live.is_none() && self.authoring.knife.is_none()
            && self.drag.is_none() && self.input.is_none() && self.modeling_tool_ready(tool) {
            // Dispatch at most once: a rejected geometric operation must never
            // re-enter action routing recursively while waiting for selection.
            self.authoring.workflow.pending_preview = None;
            if let Some(action) = tool.command() { self.action(action); }
        }
    }
    pub(super) fn modeling_tool_ready(&self, tool: M) -> bool {
        if self.authoring.editing.is_none() || self.job.is_some() || self.editor.playing
            || self.editor.has_staged() { return false; }
        match tool {
            M::Bevel => match self.authoring.selection_mode {
                S::Vertex => !self.authoring.vertices.is_empty(),
                S::Edge => !self.authoring.edges.is_empty(),
                S::Face => !self.authoring.faces.is_empty(),
            },
            M::Extrude | M::Inset | M::DeleteFaces | M::Subdivide => !self.edit_vertices().is_empty(),
            M::ExtrudeIndividual | M::Triangulate | M::Separate => !self.selected_face_region().is_empty(),
            M::BridgeFaces => self.selected_face_region().len()>=2,
            M::Bridge => (self.authoring.selection_mode == S::Face && self.selected_face_region().len() >= 2) || self.workflow_bridge_edges().len() >= 2,
            M::Mirror=>!self.selected_face_region().is_empty(),
            M::Dissolve | M::Array => !self.edit_vertices().is_empty(),
            M::LoopCut => true,
            M::Slide => !self.edit_vertices().is_empty(),
            M::Sphere | M::Circle => self.edit_vertices().len()>=3,
            M::Inflate | M::Twist | M::Merge | M::Weld | M::Align | M::Relax => self.edit_vertices().len() >= 2,
            _ => true,
        }
    }
    fn modeling_columns(&self) -> usize {
        ((self.right_width() - properties::RAIL_WIDTH - 28.) / TOOL_PITCH).floor().max(1.) as usize
    }
    pub(super) fn modeling_properties_start(&self) -> f32 {
        if self.authoring.editing.is_none() {
            return if self.selected_curve().is_some() {
                268.
            } else if self.selected_primitive().is_some() {
                374.
            } else {
                336.
            };
        }
        206. + TOOLS.len().div_ceil(self.modeling_columns()) as f32 * TOOL_PITCH + 8.
    }
    pub(super) fn show_modeling_properties(&mut self) {
        self.set_properties_tab(7);
        if !self.preferences.right_panel
            || self
                .preferences
                .collapsed_sections
                .contains(&PROPERTIES_SECTION)
        {
            return;
        }
        let Some(offset) = self.property_section_offset(PROPERTIES_SECTION) else {
            return;
        };
        let clip = self.properties_content_rect();
        let top = 171. + offset - properties::CONTENT_SHIFT;
        // Keep tools in view when room permits and reveal the settings heading on short panels.
        let visible = (self.modeling_dock_height() + 28.).min(clip[3] * 0.55);
        let minimum = (top + visible - clip[1] - clip[3]).max(0.);
        let maximum = (top - clip[1]).max(minimum);
        self.inspector_scroll = self
            .inspector_scroll
            .clamp(minimum, maximum)
            .min(self.inspector_max_scroll());
        self.surface.scroll_offsets[7] = self.inspector_scroll;
    }

    pub(super) fn modeling_panel_bottom(&self) -> f32 {
        if self.has_morph_panel() {
            return self.morph_panel_bottom();
        }
        self.modeling_properties_start() + self.modeling_dock_height()
    }

    pub(super) fn modeling_dock_height(&self) -> f32 {
        if self.construction.session.is_some() {
            self.construction_tabs_height() + self.construction_content_height() + 76.
        } else if self.authoring.editing.is_some() && self.authoring.modeling_tool.is_some() {
            self.modeling_tool_properties_height() + 16.
        } else {
            0.
        }
    }

    pub(super) fn modeling_properties_title(&self) -> Option<&'static str> {
        self.construction
            .session
            .as_ref()
            .map(|s| match s.kind {
                construction_tools::Kind::Cable => "Кабель",
                construction_tools::Kind::Boolean => "Булева операция",
            })
            .or_else(|| {
                self.authoring
                    .modeling_tool
                    .filter(|_| self.authoring.editing.is_some())
                    .map(M::title)
            })
    }

    pub(super) fn draw_modeling_dock_properties(&self, u: &mut Ui, x: f32, w: f32) {
        if self.preferences.collapsed_sections.contains(&PROPERTIES_SECTION) { return; }
        let y = self.modeling_properties_start() + 4.;
        if self.construction.session.is_some() {
            self.draw_construction_properties(u, x, y, w);
        } else if self.authoring.editing.is_some() && self.authoring.modeling_tool.is_some() {
            self.draw_modeling_tool_properties(u, x, y, w);
            if self.modeling_live.is_none() && self.authoring.knife.is_none() {
                let footer = y + self.modeling_tool_properties_height() - 36.;
                if let Some(message) = self.modeling_feedback() {
                    u.fitted_label([x, footer - 25., w, 20.], message, 10., u.palette().muted);
                }
                self.draw_modeling_confirmation(u, x, footer, w);
            }
        }
    }

    fn modeling_tool_properties_height(&self) -> f32 {
        (if self.modeling_live.is_some() {
            self.modeling_live_properties_height()
                + if self.authoring.modeling_tool == Some(M::Bevel) { 22. } else if self.authoring.modeling_tool==Some(M::Extrude) {90.} else { 0. }
                + if self.face_tool_modes_visible() { 90. } else { 0. }
        } else {
            (match self.authoring.modeling_tool {
                None => 32.,
                Some(M::Select(_)) => 208.,
                Some(M::Sphere|M::Inflate) => 152.,
                Some(M::Circle|M::Twist) => 188.,
                Some(M::Move) => 260. + if self.authoring.proportional { 34. } else { 0. },
                Some(M::Rotate | M::Scale) => 224. + if self.authoring.proportional { 34. } else { 0. },
                Some(M::Relax) => 202.,
                Some(M::Bevel) => if self.authoring.selection_mode == S::Face { 250. } else { 100. },
                Some(M::Inspect) => 380.,
                Some(M::LoopCut) => if self.authoring.knife.as_ref().is_some_and(|k|k.section_edit) {355.}else{270.},
                Some(M::Knife) => 208.,
                Some(M::Array)=>236.,
                Some(M::Extrude)=>210.,
                Some(M::Align) => if self.mesh_tools.alignment_mode==0 {146.} else {124.},
                Some(M::Weld) => 132.,
                Some(M::Bridge | M::BridgeFaces) => 250.,
                Some(M::Inset) => 194.,
                Some(M::Subdivide) => 180., Some(M::Mirror)=>238.,
                _ => 98.,
            }) + if self.authoring.knife.is_none() {44.} else {0.}
        }) + self.tool_merge_height()
    }
    pub(super) fn draw_modeling_palette(&self, u: &mut Ui, x: f32, w: f32) {
        let p = u.palette();
        if self.authoring.editing.is_none() {
            if self.selected_curve().is_some() {
                u.fitted_label([x, 182., w, 20.], "Точки и геометрия кривой — в объекте", 11., p.muted);
                u.icon_button([x, 218., w, 32.], "Свойства кривой", Action::SurfaceTab(0), false, Icon::Curve); return;
            }
            u.fitted_label([x, 182., w, 20.], "Инструменты объектов", 11., p.muted);
            for (i,(label,action,icon)) in [("Зеркало · Mirror",ui::ConstructionAction::Mirror,Icon::Mirror),("Булева операция",ui::ConstructionAction::Boolean,Icon::Boolean),("Кабель",ui::ConstructionAction::Cable,Icon::Cable)].into_iter().enumerate(){u.icon_button([x,210.+i as f32*38.,w,32.],label,Action::Construction(action),false,icon);}
            if self.selected_primitive().is_some() {
                u.icon_button([x, 330., w, 32.], "Параметры фигуры",
                    Action::Primitive(ui::PrimitiveAction::Open), false, Icon::Settings);
            }
            return;
        }
        let count = match self.authoring.selection_mode {
            S::Vertex => self.authoring.vertices.len(), S::Edge => self.authoring.edges.len(), S::Face => self.authoring.faces.len(),
        };
        u.fitted_label([x, 178., w, 18.], &format!("Выбрано компонентов: {count}"), 11., p.muted);
        let columns = self.modeling_columns();
        for (i, tool) in TOOLS.into_iter().enumerate() {
            let rect = [x + (i % columns) as f32 * TOOL_PITCH, 206. + (i / columns) as f32 * TOOL_PITCH, TOOL_SIZE, TOOL_SIZE];
            u.modeling_icon_button(rect, Action::ModelingTool(tool), tool.icon(self.authoring.workflow.simplify), self.authoring.modeling_tool == Some(tool),
                self.job.is_none() && !self.editor.playing && !self.editor.has_staged());
        }
    }
    pub(super) fn draw_pending_modeling_field(&self, u: &mut Ui, x: f32, y: f32, w: f32, field: A) {
        let (label, value, range) = match field {
            A::BridgeSegments => (
                "Сегменты",
                self.authoring.bridge_segments as f64,
                SliderRange::new(1., 32., 1.),
            ),
            A::BridgeTwist => (
                "Скручивание, шаги",
                self.authoring.bridge_twist as f64,
                SliderRange::new(-32., 32., 1.),
            ),
            A::BridgeSmooth => (
                "Плавность, %",
                (self.authoring.bridge_smooth * 100.) as f64,
                SliderRange::new(0., 100., 1.),
            ),
            A::InsetAmount => {
                let value = self.authoring.inset.unwrap_or(0.15) as f64;
                (
                    "Отступ, м",
                    value,
                    SliderRange::new(0., (value * 1.2).clamp(1., 1000.), 0.01),
                )
            }
            A::BevelHeight => (
                "Высота, м",
                self.authoring.bevel_height as f64,
                SliderRange::new(-1., 1., 0.01),
            ),
            A::WeldDistance => (
                "Расстояние, м",
                self.authoring.weld_distance as f64,
                SliderRange::new(0., 1., 0.001),
            ),
            _ => return,
        };
        u.number_field([x, y, w, 44.], label, value, Action::Author(field), range);
    }

    pub(super) fn draw_modeling_tool_properties(&self,u:&mut Ui,x:f32,mut y:f32,w:f32) {
        let p=u.palette();
        let Some(tool) = self.authoring.modeling_tool else { return; };
        if tool == M::Bevel {
            u.fitted_label([x, y, w, 18.], match self.authoring.selection_mode {
                S::Vertex => "Вершины · срез углов",
                S::Edge => "Рёбра · ширина, сегменты, профиль",
                S::Face => "Грани · сегменты и профиль",
            }, 10., p.muted);
            y += 22.;
        }
        if tool==M::Extrude { self.draw_extrude_modes(u,x,y,w); y+=90.; }
        if self.face_tool_modes_visible() { self.draw_face_tool_modes(u,x,y,w); y+=90.; }
        if self.tool_merge_visible() { self.draw_tool_merge(u,x,y,w); y+=self.tool_merge_height(); }
        if self.modeling_live.is_some() {
            self.draw_modeling_live_properties(u, x, y, w);
            return;
        }
        match tool {
            M::Select(_) => {
                for (i,(a,label)) in [
                    (A::SelectAll,"Выделить всё"),(A::Clear,"Снять выделение"),(A::Grow,"Расширить"),(A::Shrink,"Сузить"),
                    (A::Connected,"Связные"),(A::Invert,"Инверсия"),(A::EdgeLoop,"Петля"),(A::EdgeRing,"Кольцо"),
                ].into_iter().enumerate() {
                    let half=(w-6.)*0.5; let r=[x+(i%2) as f32*(half+6.),y+(i/2) as f32*34.,half,30.];
                    if matches!(a,A::EdgeLoop|A::EdgeRing) && (self.authoring.selection_mode!=S::Edge || self.authoring.edges.is_empty()) {
                        u.disabled_button(r,label);
                    } else { u.quiet_button(r,label,Action::Author(a),false); }
                }
                y+=140.;
                u.checkbox([x, y, w, 26.], "Сквозное выделение · X-Ray", Action::Author(A::Xray), self.authoring.xray);
            }
            M::Move | M::Rotate | M::Scale => { self.draw_modeling_transform_properties(u, x, y, w, tool); }
            M::Align => {
                self.draw_alignment_mode(u,x,y,w);
                y+=38.;
                if self.mesh_tools.alignment_mode!=0 {
                    u.fitted_label([x,y,w,30.],"Предпросмотр выравнивания",11.,p.muted);
                    u.fitted_label([x,y+36.,w,18.],"Концы цепочки остаются на месте",10.,p.muted);
                    return;
                }
                u.fitted_label([x, y, w, 18.], "Плоскость по локальной оси", 11., p.muted);
                for (i, name) in ["X", "Y", "Z"].into_iter().enumerate() {
                    let r = [x + i as f32 * (w + 6.) / 3., y + 26., (w - 12.) / 3., 28.];
                    if self.modeling_tool_ready(tool) { u.quiet_button(r, name, Action::Shape(ShapeAction::Flatten(i as u8)), false); }
                    else { u.disabled_button(r, name); }
                }
            }
            M::Relax => { self.draw_relax_properties(u, x, y, w); }
            M::Sphere|M::Circle|M::Inflate|M::Twist => self.draw_deform_properties(u,x,y,w,tool),
            M::Array => self.draw_array_properties(u,x,y,w),
            M::Subdivide|M::Mirror => self.draw_density_mirror(u,x,y,w,tool==M::Mirror),
            M::LoopCut if self.authoring.knife.is_some() => self.draw_section_properties(u,x,y,w),
            M::Knife if self.authoring.knife.is_some() => {
                u.fitted_label([x,y,w,18.],"Backspace — новая линия · Enter — завершить",11.,p.muted);y+=26.;
                self.draw_workflow_field(u,x,y,w,0,38.);y+=46.;
                u.checkbox([x,y,w,26.],"Закрыть торцы зазора",Action::Author(A::KnifeCloseGap),self.authoring.workflow.close_gap);y+=34.;
                u.checkbox([x,y,w,26.],"Вершины и середины рёбер",Action::Author(A::KnifeSnap),self.authoring.knife_snap);y+=34.;
                self.draw_dropdown_button(u,[x,y,w,30.],ui::DropdownKind::KnifeAngle,["Свободный угол","Шаг 15°","Шаг 30°","Шаг 45°","Шаг 90°"][self.authoring.knife_angle as usize]);y+=10.;
                self.draw_modeling_confirmation(u, x, y + 28., w);
            }
            _ => {
                let fields: &[A] = match tool {
                    M::Weld => &[A::WeldDistance],
                    M::Bridge | M::BridgeFaces => &[A::BridgeSegments, A::BridgeTwist, A::BridgeSmooth],
                    M::Inset => &[A::InsetAmount],
                    M::Bevel if self.authoring.selection_mode == S::Face => &[A::InsetAmount, A::BevelHeight],
                    _ => &[],
                };
                for &field in fields {
                    self.draw_pending_modeling_field(u,x,y,w,field);
                    y += 52.;
                }
                if let Some(action) = tool.command() {
                    if tool.live() {
                        if self.authoring.workflow.pending_preview.is_none() {
                            u.fitted_label([x,y,w,30.], "Shift — изменить выделение", 10.,p.muted);
                        }
                    }
                    else if tool == M::Inspect { u.quiet_button([x,y,w,30.],"Проверить геометрию",action,false); }
                    else if self.authoring.workflow.session.as_ref().map_or_else(||self.modeling_tool_ready(tool),|session|session.error.is_none()) { u.fitted_label([x,y,w,30.],"Предпросмотр результата",11.,p.muted); }
                    y += 38.;
                }
                if tool == M::Inspect {
                    self.draw_dropdown_button(u,[x,y,w,30.],ui::DropdownKind::RepairKind,poseforge_core::RepairKind::ALL[self.authoring.repair_kind as usize].label());y+=38.;
                    u.quiet_button([x,y,w,28.],"Выделить ошибки категории",Action::Author(A::SelectIssue),false);y+=34.;
                    u.quiet_button([x,y,w,28.],"Исправить выбранные ошибки",Action::Author(A::RepairIssue),false);y+=34.;
                    self.draw_topology_report(u, x, y, w);
                } else if !tool.live() {
                    u.fitted_label([x, y, w, 18.], "Shift — изменить выделение", 10., p.muted);
                }
            }
        }
    }
}

#[cfg(test)]
mod icon_tests {
    use super::*;

    #[test]
    fn modeling_icons_follow_the_density_mode() {
        for tool in TOOLS {
            if tool == M::Subdivide {
                assert_eq!(tool.icon(false), Icon::Subdivide);
                assert_eq!(tool.icon(true), Icon::Simplify);
            } else {
                assert_eq!(tool.icon(false), tool.icon(true));
            }
        }
    }
}
