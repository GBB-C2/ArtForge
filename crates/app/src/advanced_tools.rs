//! Interactive knife and proportional-edit state share the editor transaction.
use super::*;
use crate::ui::AuthorAction as A;
use poseforge_core::{EdgePoint, MeshEdit};
#[path = "smart_knife.rs"]
mod smart_knife;
#[path = "editable_sections.rs"]
mod editable_sections;
pub(super) use editable_sections::SectionGripDrag;

pub(super) struct KnifeState {
    node: u64,
    original: poseforge_core::Mesh,
    strokes: Vec<poseforge_core::KnifeCut>,
    sections: Vec<editable_sections::Panel>,
    pub(super) section_edit: bool,
    section_selected: Option<usize>,
    section_batch: Option<(EdgePoint,usize,usize)>,
    section_drag: Option<(usize,[f32;2],f32)>,
    pub(super) loop_mode:bool,
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
        if self.section_action(command) {return true;}
        match command {
            A::Knife | A::LoopCut => {
                self.cancel_modeling_live();
                let Some(node) = self.authoring.editing else {
                    return true;
                };
                let Ok(original)=self.editor.editable_mesh(node).cloned() else {return true;};
                self.editor.begin_edit(if command==A::LoopCut {"Разрез"} else {"Нож"});
                self.authoring.knife = Some(KnifeState {
                    node, original, strokes:Vec::new(), sections:Vec::new(),
                    section_edit:false,section_selected:None,section_batch:None,section_drag:None,
                    loop_mode: command==A::LoopCut,
                    first: None,
                    selection: component_selection::SelectionSnapshot::capture(&self.authoring),
                    cuts: 0,
                    press: None,
                });
                self.status = if command==A::LoopCut {"Разрез · наведите на грань и нажмите · Enter — применить · Esc — отменить"}else{"Нож · потяните линию через грани · Shift / C — угол · Enter — применить · Esc — отменить"}.into();
            }
            A::KnifeCloseGap if self.authoring.knife.as_ref().is_some_and(|k|k.loop_mode) => { self.section_toggle_cap(); }
            A::KnifeCloseGap => {
                self.authoring.workflow.close_gap=!self.authoring.workflow.close_gap;
                if let Err(e)=self.preview_knife_geometry() {self.authoring.workflow.close_gap=!self.authoring.workflow.close_gap;self.status=e;}
            }
            A::KnifeSnap => self.authoring.knife_snap = !self.authoring.knife_snap,
            A::KnifeAngle(mode) => self.authoring.knife_angle = mode.min(4),
            A::KnifeAccept => self.accept_knife(),
            A::KnifeCancel => { self.cancel_knife(); self.dismiss_modeling_tool_panel(); },
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
            if matches!(self.drag,Some(Drag::SectionGrip(_))) { self.drag=None; self.drag_button=None; }
            self.release_drag_pointer();
            self.editor.cancel_edit();
            k.selection.restore(&mut self.authoring);
            self.invalidate_geometry();
            self.update_dirty();
            self.status = "Нож · отменено".into();
        }
    }
    pub(super) fn knife_has_cuts(&self) -> bool {
        self.authoring.knife.as_ref().is_some_and(|k| k.cuts > 0)
    }
    fn accept_knife(&mut self) {
        if let Some(k) = self.authoring.knife.take() {
            if matches!(self.drag,Some(Drag::SectionGrip(_))) { self.drag=None; self.drag_button=None; }
            self.release_drag_pointer();
            if k.cuts == 0 {
                self.editor.cancel_edit();
                self.dismiss_modeling_tool_panel();
                self.status = "Разрезов нет".into();
                self.invalidate_geometry();
                self.update_dirty();
                return;
            }
            let r = self.editor.commit_edit();
            if self.result(r).is_some() {
                self.status = format!("{} · разрезов: {}", if k.loop_mode {"Разрез"}else{"Нож"}, k.cuts);
                self.dismiss_modeling_tool_panel();
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
            Action::ToggleSection(super::modeling_palette::TOOLS_SECTION | super::modeling_palette::PROPERTIES_SECTION) => false,
            Action::ScrollPanel(0) | Action::ToggleRightPanel => false,
            a if Self::preview_navigation_action(a) => false,
            Action::Author(A::SnapToggle|A::SnapMode(_)|A::SnapGridStep(_))
                | Action::DropdownOpen(ui::DropdownKind::SnapMode)|Action::DropdownDismiss=>false,
            Action::Author(A::SectionMode(_) | A::SectionSelect(_) | A::SectionDelete | A::SectionGrip(_) | A::ToolField(_) | A::LoopPosition | A::ToolPanelDrag | A::ToolPanelToggle | A::ToolPanelScroll | A::LiveBackdrop)=>false,
            Action::Slide(field,_,_) | Action::ResetNumber(field) if Self::workflow_preference_field(field)=>false,
            Action::Author(A::KnifeAccept | A::KnifeCancel | A::KnifeSnap | A::KnifeCloseGap | A::KnifeAngle(_))
            | Action::DropdownOpen(ui::DropdownKind::KnifeAngle)
            | Action::DropdownChoose(_) => false,
            Action::Cancel | Action::Undo | Action::Redo => {
                self.cancel_knife();
                self.dismiss_modeling_tool_panel();
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
            Key::Named(NamedKey::Delete) if self.authoring.knife.as_ref().is_some_and(|k|k.loop_mode&&k.section_edit) => {self.section_action(A::SectionDelete);}
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
            Key::Named(NamedKey::Escape) => { self.cancel_knife(); self.dismiss_modeling_tool_panel(); },
            Key::Character(c)
                if self.modifiers.control_key()
                    && (c.eq_ignore_ascii_case("z") || c.eq_ignore_ascii_case("y")) =>
            {
                self.cancel_knife(); self.dismiss_modeling_tool_panel();
            }
            _ => return false,
        }
        self.redraw();
        true
    }
    fn knife_pick(&self) -> Option<EdgePoint> {
        if self.authoring.knife.as_ref().is_some_and(|k|k.loop_mode){self.loop_pick()}else{self.smart_knife_pick(self.knife_cursor())}
    }
    fn loop_pick(&self)->Option<EdgePoint>{
        if let Some(hit)=self.smart_knife_pick(self.mouse){return Some(hit);}
        let k=self.authoring.knife.as_ref()?;let mesh=&k.original;
        let globals=self.editor.project.global_transforms().ok()?;let model=globals[&k.node];let projection=self.view().view_projection();let vp=self.viewport();
        let (origin,direction)=authoring_controls::ray(projection.inverse(),vp,self.mouse)?;let inverse=model.inverse();let cache=self.mesh_cache(mesh);
        let (triangle,_)=cache.hit(inverse.transform_point3(origin),inverse.transform_vector3(direction))?;let f=mesh.triangle_face(triangle)?;let ring=mesh.face_vertices(f)?;
        let cursor=glam::Vec2::from_array(self.mouse);
        (0..ring.len()).filter_map(|i|{let a=cache.graph.canonical[ring[i] as usize];let b=cache.graph.canonical[ring[(i+1)%ring.len()] as usize];let p=|v:usize|authoring_controls::project(projection,vp,model.transform_point3(Vec3::from_array(mesh.vertices[v].position))).map(glam::Vec2::from_array);
            let(pa,pb)=(p(a)?,p(b)?);let delta=pb-pa;let t=((cursor-pa).dot(delta)/delta.length_squared().max(1e-8)).clamp(0.,1.);Some(((pa+delta*t).distance_squared(cursor),EdgePoint{edge:(a.min(b),a.max(b)),t:if a<b {t} else {1.-t}}))
        }).min_by(|a,b|a.0.total_cmp(&b.0)).map(|(_,hit)|hit)
    }
    fn loop_cut_click(&mut self) {
        if self.authoring.knife.as_ref().is_some_and(|k|k.section_edit) {self.select_section_at_cursor();return;}
        let Some(hit)=self.loop_pick()else{return;};
        if let Err(e)=self.create_sections(hit) {self.status=e;}
    }
    fn loop_paths(&self,mesh:&poseforge_core::Mesh,hit:EdgePoint)->Result<Vec<poseforge_core::KnifeCut>,poseforge_core::CoreError> {
        let mut cuts=Vec::new();
        for panel in self.section_panels(hit).map_err(poseforge_core::CoreError::Invalid)? {
            cuts.extend(poseforge_core::plane_cut_path(mesh,&panel.cut())?);
        } Ok(cuts)
    }
    pub(super) fn preview_knife_geometry(&mut self)->Result<(),String> {
        let Some(k)=self.authoring.knife.as_ref() else {return Ok(());};
        if k.loop_mode {
            let cuts=k.sections.iter().map(|p|p.cut()).collect();
            self.editor.preview_mesh(k.node,&k.original,MeshEdit::Sections{cuts}).map_err(|e|e.to_string())?;
            let k=self.authoring.knife.as_mut().unwrap();k.cuts=k.sections.len();
            self.authoring.vertices.clear();self.authoring.edges.clear();self.authoring.faces.clear();
            self.invalidate_geometry();self.update_dirty();return Ok(());
        }
        let cuts=k.strokes.clone();
        if cuts.is_empty() {return Ok(());}
        let count=cuts.len();let width=self.authoring.workflow.values[usize::from(k.loop_mode)];
        let edit=if self.authoring.workflow.close_gap {MeshEdit::KnifeGapClosed{cuts,width}} else {MeshEdit::KnifeGap{cuts,width}};
        self.editor.preview_mesh(k.node,&k.original,edit).map_err(|e|e.to_string())?;
        self.authoring.knife.as_mut().unwrap().cuts=count;
        self.authoring.vertices.clear();self.authoring.edges.clear();self.authoring.faces.clear();
        self.invalidate_geometry();self.update_dirty();Ok(())
    }
    pub(super) fn knife_scroll(&mut self,delta:f32)->bool {
        if matches!(self.drag,Some(Drag::SectionGrip(_))) {return true;}
        if !self.authoring.knife.as_ref().is_some_and(|k|k.loop_mode) || !self.modifiers.control_key()
            || !self.canvas_input_at(self.mouse) || self.drag.is_some() || self.has_modal() {return false;}
        if let Some(k)=self.authoring.knife.as_ref().filter(|k|k.section_edit) {
            let len=k.sections.len();if len>0 {let next=(k.section_selected.unwrap_or(0) as isize+if delta>0.{1}else{-1}).rem_euclid(len as isize) as usize;self.section_action(A::SectionSelect(next));}
            self.redraw();return true;
        }
        let count=(self.authoring.workflow.values[9]+delta.signum()).clamp(1.,16.);
        let result=self.workflow_confirm(&Action::Author(A::ToolField(9)),&count.to_string());
        if let Err(e)=result {self.status=e;} self.redraw();true
    }
    pub(super) fn knife_click(&mut self) {
        if self.authoring.knife.as_ref().is_some_and(|k|k.loop_mode){ self.loop_cut_click(); }else{self.smart_knife_click();}
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
                Ok::<_,poseforge_core::CoreError>(&k.original),
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
                if k.loop_mode && !k.section_edit {
                    if let Some(hit)=self.knife_pick() {if let Ok(cuts)=self.loop_paths(mesh,hit) {for c in cuts {if let (Some(a),Some(b))=(project(globals[&k.node].transform_point3(Vec3::from_array(c.first))),project(globals[&k.node].transform_point3(Vec3::from_array(c.second)))) {u.mesh_segment(a,b,2.,p.accent);}}}}
                }
                let hover = if k.loop_mode {None}else{self.knife_pick().and_then(screen)};
                if let Some(a) = k.press.filter(|_|!k.loop_mode).or_else(|| {
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

        }
        self.draw_section_planes(u);
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
