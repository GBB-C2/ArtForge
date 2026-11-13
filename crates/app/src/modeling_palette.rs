//! Icon tools and contextual properties use the existing modeling commands.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S, ModelingTool as M, ShapeAction};

pub(super) const TOOLS_SECTION: u16 = 740;
pub(super) const PROPERTIES_SECTION: u16 = 741;
const TOOL_PITCH: f32 = 48.;
const TOOL_SIZE: f32 = 44.;
const TOOLS: [M; 28] = [
    M::Select(0), M::Select(1), M::Select(2), M::Move, M::Rotate, M::Scale,
    M::Extrude, M::Inset, M::Bevel, M::LoopCut, M::Knife, M::Slide,
    M::Merge, M::Weld, M::Bridge, M::BridgeFaces, M::Dissolve, M::DeleteFaces, M::Subdivide,
    M::Smooth, M::Flat, M::Align, M::Relax,
    M::Sphere, M::Circle, M::Inflate, M::Twist, M::Inspect,
];

impl M {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Select(1) => "Выделение рамкой",
            Self::Select(2) => "Выделение лассо",
            Self::Select(_) => "Выделение щелчком",
            Self::Move => "Перемещение · G", Self::Rotate => "Вращение · R",
            Self::Scale => "Масштаб · S", Self::Extrude => "Экструдирование · E",
            Self::Inset => "Вставка граней · I", Self::Bevel => "Фаска · B",
            Self::LoopCut => "Разрез петлёй · C",
            Self::Knife => "Нож · K", Self::Slide => "Сдвиг по поверхности · J",
            Self::Merge => "Объединить в центре", Self::Weld => "Сварка вершин",
            Self::Bridge => "Мост между границами", Self::Dissolve => "Растворить рёбра",
            Self::BridgeFaces => "Мост из граней",
            Self::DeleteFaces => "Удалить", Self::Subdivide => "Подразделить выделение",
            Self::Smooth => "Сгладить нормали", Self::Flat => "Плоские нормали",
            Self::Align => "Выравнивание", Self::Relax => "Релакс сетки",
            Self::Sphere => "Сферизация", Self::Circle => "Окружность",
            Self::Inflate => "Смещение по нормалям", Self::Twist => "Скручивание",
            Self::Inspect => "Проверить топологию",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Self::Select(1) => Icon::SelectBox, Self::Select(2) => Icon::SelectLasso,
            Self::Select(_) => Icon::SelectPointer,
            Self::Move => Icon::Move, Self::Rotate => Icon::Rotate, Self::Scale => Icon::Scale,
            Self::Extrude => Icon::Extrude, Self::Inset => Icon::Inset,
            Self::Bevel => Icon::Bevel,
            Self::LoopCut => Icon::LoopCut, Self::Knife => Icon::Knife, Self::Slide => Icon::SlideEdge,
            Self::Merge => Icon::Merge, Self::Weld => Icon::Weld, Self::Bridge | Self::BridgeFaces => Icon::Bridge,
            Self::Dissolve => Icon::Dissolve, Self::DeleteFaces => Icon::DeleteFaces,
            Self::Subdivide => Icon::Subdivide, Self::Smooth => Icon::SmoothNormals, Self::Flat => Icon::FlatNormals,
            Self::Sphere => Icon::ModelSphere, Self::Circle => Icon::ModelCircle,
            Self::Inflate => Icon::Inflate, Self::Twist => Icon::Twist,
            Self::Align => Icon::Flatten, Self::Relax => Icon::Relax, Self::Inspect => Icon::InspectMesh,
        }
    }
    fn command(self) -> Option<Action> {
        Some(match self {
            Self::Select(i) => Action::Author(A::SelectTool(i)),
            Self::Move => Action::Move, Self::Rotate => Action::Rotate, Self::Scale => Action::Scale,
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
    fn live(self) -> bool {
        matches!(self, Self::Extrude | Self::Inset | Self::Bevel | Self::LoopCut | Self::Slide)
    }
}

impl App {
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
            self.authoring.modeling_tool = Some(tool);
            self.preferences.collapsed_sections.remove(&PROPERTIES_SECTION);
        }
    }
    pub(super) fn choose_modeling_tool(&mut self, tool: M) {
        if self.authoring.editing.is_none() || self.job.is_some() || self.editor.playing
            || self.scene_write_pending() || self.editor.has_staged() { return; }
        self.authoring.modeling_tool = Some(tool);
        self.preferences.collapsed_sections.remove(&PROPERTIES_SECTION);
        if let Some(action) = tool.command() {
            if matches!(tool, M::Select(_) | M::Move | M::Rotate | M::Scale | M::Knife | M::Inspect)
                || (tool.live() && self.modeling_tool_ready(tool)) {
                self.action(action);
            }
        }
        self.set_properties_tab(7);
        // Reveal the properties heading, including at narrow panel widths.
        let top = self.modeling_properties_start() + 28. - properties::CONTENT_SHIFT;
        let viewport = self.properties_content_rect();
        if top - self.inspector_scroll > viewport[1] + viewport[3] - 120. {
            self.inspector_scroll = (top - viewport[1] - 40.).max(0.).min(self.inspector_max_scroll());
        }
    }
    fn modeling_tool_ready(&self, tool: M) -> bool {
        if self.authoring.editing.is_none() || self.job.is_some() || self.editor.playing
            || self.editor.has_staged() { return false; }
        match tool {
            M::Bevel => match self.authoring.selection_mode {
                S::Vertex => !self.authoring.vertices.is_empty(),
                S::Edge => !self.authoring.edges.is_empty(),
                S::Face => !self.authoring.faces.is_empty(),
            },
            M::Extrude | M::Inset | M::DeleteFaces | M::Subdivide => !self.edit_vertices().is_empty(),
            M::BridgeFaces => self.selected_face_region().len()>=2,
            M::Bridge | M::Dissolve => self.authoring.selection_mode == S::Edge && !self.authoring.edges.is_empty(),
            M::LoopCut => self.authoring.selection_mode == S::Edge && self.authoring.edges.len() == 1,
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
        206. + TOOLS.len().div_ceil(self.modeling_columns()) as f32 * TOOL_PITCH + 8.
    }
    pub(super) fn modeling_panel_bottom(&self) -> f32 {
        if self.has_morph_panel() { return self.morph_panel_bottom(); }
        if self.authoring.editing.is_none() { return if self.selected_primitive().is_some() || self.selected_curve().is_some() { 268. } else { 222. }; }
        let height = if self.modeling_live.is_some() {
            self.modeling_live_properties_height() + 30.
                + if self.authoring.modeling_tool == Some(M::Bevel) { 22. } else { 0. }
        } else {
            match self.authoring.modeling_tool {
                None => 32.,
                Some(M::Select(_)) => 208.,
                Some(M::Sphere|M::Inflate) => 152.,
                Some(M::Circle|M::Twist) => 188.,
                Some(M::Move) => 260. + if self.authoring.proportional { 34. } else { 0. },
                Some(M::Rotate | M::Scale) => 224. + if self.authoring.proportional { 34. } else { 0. },
                Some(M::Relax) => 202.,
                Some(M::Bevel) => if self.authoring.selection_mode == S::Face { 160. } else { 100. },
                Some(M::Inspect) => 198.,
                Some(M::Knife) => 100.,
                Some(M::Align) => if self.mesh_tools.alignment_mode==0 {146.} else {124.},
                Some(M::Weld | M::Bridge | M::BridgeFaces) => 132.,
                Some(M::Inset | M::LoopCut) => 104.,
                _ => 98.,
            }
        };
        self.modeling_properties_start() + height
    }
    pub(super) fn draw_modeling_palette(&self, u: &mut Ui, x: f32, w: f32) {
        let p = u.palette();
        if self.authoring.editing.is_none() {
            if self.selected_curve().is_some() {
                u.fitted_label([x, 182., w, 20.], "Точки и геометрия кривой — в объекте", 11., p.muted);
                u.icon_button([x, 218., w, 32.], "Свойства кривой", Action::SurfaceTab(0), false, Icon::Curve); return;
            }
            u.fitted_label([x, 182., w, 20.], "Выберите вершины, рёбра или грани", 11., p.muted);
            if self.selected_primitive().is_some() {
                u.icon_button([x, 218., w, 32.], "Параметры фигуры",
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
            u.modeling_icon_button(rect, Action::ModelingTool(tool), tool.icon(), self.authoring.modeling_tool == Some(tool),
                self.job.is_none() && !self.editor.playing && !self.editor.has_staged());
        }
        let mut y = self.modeling_properties_start();
        let Some(tool) = self.authoring.modeling_tool else { return; };
        u.fitted_label([x, y + 4., w, 20.], tool.title(), 12., p.text);
        y += 30.;
        if tool == M::Bevel {
            u.fitted_label([x, y, w, 18.], match self.authoring.selection_mode {
                S::Vertex => "Вершины · срез углов",
                S::Edge => "Рёбра · ширина, сегменты, профиль",
                S::Face => "Грани · отступ и высота",
            }, 10., p.muted);
            y += 22.;
        }
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
                self.draw_dropdown_button(u,[x,y,w,30.],ui::DropdownKind::Alignment,["В плоскость","В цепочку","Равномерный шаг"][self.mesh_tools.alignment_mode as usize]);
                y+=38.;
                if self.mesh_tools.alignment_mode!=0 {
                    let action=Action::Shape(if self.mesh_tools.alignment_mode==1 {ShapeAction::Straighten} else {ShapeAction::Space});
                    if self.modeling_tool_ready(tool) {u.button([x,y,w,30.],"Применить",action,false);} else {u.disabled_button([x,y,w,30.],"Выберите цепочку");}
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
            M::Knife if self.authoring.knife.is_some() => {
                u.fitted_label([x, y, w, 18.], "Продолжайте разрез · Backspace — новая линия", 11., p.muted);
                let half = (w - 6.) * 0.5;
                u.button([x, y + 28., half, 30.], "Применить", Action::Author(A::KnifeAccept), true);
                u.quiet_button([x + half + 6., y + 28., half, 30.], "Отмена", Action::Author(A::KnifeCancel), false);
            }
            _ => {
                for (label, value, action) in match tool {
                    M::Weld => vec![("Расстояние, м", self.authoring.weld_distance, A::WeldDistance)],
                    M::Bridge | M::BridgeFaces => vec![("Сегменты", self.authoring.bridge_segments as f32, A::BridgeSegments)],
                    M::Inset => vec![("Отступ, %", self.authoring.inset * 100., A::InsetAmount)],
                    M::Bevel if self.authoring.selection_mode == S::Face => vec![("Отступ, %", self.authoring.inset * 100., A::InsetAmount), ("Высота, м", self.authoring.bevel_height, A::BevelHeight)],
                    M::LoopCut => vec![("Положение, %", self.authoring.loop_position * 100., A::LoopPosition)],
                    _ => vec![],
                } {
                    u.fitted_label([x, y + 6., w * 0.55, 20.], label, 11., p.muted);
                    u.field([x + w * 0.58, y, w * 0.42, 28.], &ui::number_caption(value as f64), Action::Author(action));
                    y += 34.;
                }
                if let Some(action) = tool.command() {
                    let label = if tool.live() { "Начать предпросмотр" } else { "Применить" };
                    if self.modeling_tool_ready(tool) { u.button([x, y, w, 30.], label, action, false); }
                    else { u.disabled_button([x, y, w, 30.], "Выберите нужные компоненты"); }
                    y += 38.;
                }
                if tool == M::Inspect {
                    u.quiet_button([x, y, w, 28.], "Выбрать дефекты", Action::Author(A::SelectDefects), false);
                    y += 34.;
                    self.draw_topology_report(u, x, y, w);
                } else if !tool.live() {
                    u.fitted_label([x, y, w, 18.], if tool == M::Subdivide { "Только выделенные компоненты · Undo" } else { "Изменение можно отменить · Undo" }, 10., p.muted);
                }
            }
        }
    }
}
