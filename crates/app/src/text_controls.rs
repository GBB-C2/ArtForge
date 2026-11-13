//! Object text parameters share the regular text editor, sliders and undo capture.
use super::*;
use crate::ui::TextAction as T;
use poseforge_core::TextSettings;
pub(super) const PANEL_BOTTOM:f32=1092.;
impl App {
    pub(super) fn selected_text(&self)->Option<(u64,&TextSettings)> {
        let id=self.editor.selected?;Some((id,self.editor.project.node(id)?.text.as_ref()?))
    }
    pub(super) fn text_editable(&self,id:u64)->bool {
        self.node_selectable(id) && self.job.is_none() && !self.scene_write_pending() && !self.editor.has_staged()
            && self.editor.project.node(id).is_some_and(|n|n.skin.is_none()&&n.physics.is_none()&&n.fluid.is_none()&&n.destruction.is_none() && n.mesh.and_then(|m|self.editor.project.mesh(m)).is_some_and(|m|m.morph_targets.is_empty()))
    }
    pub(super) fn text_action(&mut self,action:T) {
        if self.job.is_some() || self.scene_write_pending() || self.editor.has_staged() {return;}
        if action==T::Create {
            let result=self.editor.create_text(TextSettings::default());
            if let Some(id)=self.result(result) {
                self.editor.mode=EditorMode::Scene;self.editor.playing=false;self.authoring=Default::default();
                self.primitive_panel=None;self.curves=Default::default();self.action(Action::Select(id));
                self.set_properties_tab(0);self.preferences.right_panel=true;self.tool=Tool::Move;
                self.preferences.collapsed_sections.remove(&2201);
                self.inspector_scroll=self.property_section_offset(2201).unwrap_or(0.).min(self.inspector_max_scroll());
                self.invalidate();self.status="Текст создан · измените надпись, размер и толщину в объекте".into();
            }
            return;
        }
        let Some((id,settings))=self.selected_text() else{return;};
        if !self.node_selectable(id) {return;}
        match action {
            T::Content if self.text_editable(id) => self.input(Action::Text(action),settings.content.replace('\n',"\\n")),
            T::Field(field) if field<5 && self.text_editable(id) => {
                let value=[settings.size,settings.depth,settings.spacing,settings.bevel,settings.bevel_segments as f32][field as usize];
                self.input(Action::Text(action),ui::number_caption(value as f64));
            }
            T::Font(_) | T::OnCurve if self.text_editable(id) => {
                let mut settings=settings.clone();
                match action {
                    T::Font(index)=>{let Some(&font)=poseforge_core::TextFont::ALL.get(index as usize) else{return;};settings.font=font;}
                    T::OnCurve=>settings.on_curve=!settings.on_curve,
                    _=>{}
                }
                let waiting=settings.on_curve&&self.path_binding().is_none();
                let result=self.editor.set_text(id,settings);
                if self.result(result).is_some() {
                    self.invalidate_geometry();self.update_dirty();
                    if waiting {self.status="Выберите кривую в разделе «Привязка» ниже".into();}
                }
            }
            T::Convert => {let r=self.editor.convert_text_to_mesh(id);if self.result(r).is_some() {self.invalidate();self.status="Текст преобразован в меш · доступны вершины, рёбра и грани · Undo возвращает надпись".into();}},
            _=>{}
        }
    }
    pub(super) fn text_confirm(&mut self,action:&Action,value:&str)->Result<bool,String> {
        let Action::Text(action)=action else{return Ok(false);};
        let (id,source)=self.selected_text().ok_or("Выберите текст")?;
        if !self.text_editable(id) {return Err("Параметры текста сейчас недоступны".into());}
        let mut settings=source.clone();
        match action {
            T::Content=>settings.content=value.replace("\r\n","\n").replace("\\n","\n"),
            T::Field(0)=>settings.size=surfaces::number(value,0.01,100.)?,
            T::Field(1)=>settings.depth=surfaces::number(value,0.,10.)?,
            T::Field(2)=>settings.spacing=surfaces::number(value,0.1,4.)?,
            T::Field(3)=>settings.bevel=surfaces::number(value,0.,10.)?,
            T::Field(4)=>{let n=surfaces::number(value,1.,6.)?;if n.fract()!=0. {return Err("Число сегментов должно быть целым".into());}settings.bevel_segments=n as u8;},
            _=>return Ok(false),
        }
        self.editor.set_text(id,settings).map_err(|e|e.to_string())?;
        self.invalidate_geometry();self.update_dirty();Ok(true)
    }
    pub(super) fn draw_text_properties(&self,u:&mut Ui,rx:f32,rw:f32) {
        let Some((id,s))=self.selected_text() else{return;};let x=rx+16.;let w=rw-32.;let p=u.palette();
        let editable=self.text_editable(id);let content=s.content.replace('\n',"\\n");
        u.fitted_label([x,528.,w,18.],"Надпись",11.,p.muted);
        if editable {u.field([x,550.,w,32.],&content,Action::Text(T::Content));}
        else {u.disabled_button([x,550.,w,32.],&content);}
        if editable {self.draw_dropdown_button(u,[x,590.,w,30.],ui::DropdownKind::TextFont,s.font.label());}
        else {u.disabled_button([x,590.,w,30.],s.font.label());}
        for (i,(label,value,min,max,step)) in [("Размер, м",s.size,0.01,10.,0.01),("Толщина, м",s.depth,0.,1.,0.01),("Интервал букв",s.spacing,0.1,4.,0.01),("Bevel, м",s.bevel,0.,0.1,0.001),("Сегменты bevel",s.bevel_segments as f32,1.,6.,1.)].into_iter().enumerate() {
            let r=[x,632.+i as f32*46.,w,36.];
            if editable {u.number_field(r,label,value as f64,Action::Text(T::Field(i as u8)),SliderRange::new(min,(max as f64).max(value as f64),step));}
            else {u.disabled_button(r,&format!("{label}: {}",ui::number_caption(value as f64)));}
        }
        if editable {u.checkbox([x,878.,w,30.],"Текст по кривой",Action::Text(T::OnCurve),s.on_curve);}
        else {u.disabled_button([x,878.,w,30.],"Текст по кривой");}
        u.fitted_label([x,918.,w,18.],if s.on_curve && self.path_binding().is_none() {"Выберите путь в разделе привязки ниже"} else {"Bevel ограничен толщиной и контурами букв"},10.,p.muted);
        if self.node_selectable(id)&&self.job.is_none()&&!self.scene_write_pending()&&!self.editor.has_staged() {
            u.icon_button([x,966.,w,30.],"Преобразовать в меш",Action::Text(T::Convert),false,Icon::Object);
            u.checkbox([x,1006.,w,30.],"Только тень",Action::SceneShadowOnly(id),self.editor.project.node(id).unwrap().shadow_only);
        } else {u.disabled_button([x,966.,w,30.],"Преобразовать в меш");}
        u.fitted_label([x,1050.,w,18.],"Кириллица · латиница · \\n — новая строка",10.,p.muted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app()->App {let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.preferences=Preferences::default();a.width=1440.;a.height=900.;a}
    #[test]
    fn text_menu_adds_at_origin_and_edit_convert_and_undo_keep_materials() {
        let mut a=app();a.ui=a.build_ui();a.action(Action::Text(T::Create));
        let id=a.editor.selected.unwrap();assert_eq!(a.editor.project.node(id).unwrap().transform.translation,Vec3::ZERO);
        assert!(a.selected_text().is_some());let before=a.editor.project.clone();
        a.text_confirm(&Action::Text(T::Content),"Привет\\nМир").unwrap();a.text_confirm(&Action::Text(T::Field(1)),"0,2").unwrap();
        assert_eq!(a.selected_text().unwrap().1.content,"Привет\nМир");assert_eq!(a.editor.project.materials,before.materials);
        let geometry=a.editor.project.meshes.clone();a.text_action(T::Convert);assert!(a.selected_text().is_none());assert_eq!(a.editor.project.meshes,geometry);
        a.editor.undo();assert!(a.selected_text().is_some());
    }
    #[test]
    fn text_properties_and_reset_are_reachable_in_both_themes_and_dpi() {
        for theme in [ui::Theme::Light,ui::Theme::Dark] {for scale in [1.,2.] {
            let mut a=app();a.preferences.theme=theme;a.scale=scale;a.text_action(T::Create);
            assert_eq!(a.number_default(&Action::Text(T::Field(1))),Some(0.08));
            for action in [T::Content,T::Field(0),T::Field(1),T::Field(2),T::Field(3),T::Field(4),T::OnCurve,T::Convert] {
                let max=a.inspector_max_scroll();let mut found=false;
                for step in 0..=(max/24.).ceil() as usize {a.inspector_scroll=(step as f32*24.).min(max);let u=a.build_ui();found|=u.action_point(&Action::Text(action)).is_some();}
                assert!(found,"{action:?}");
            }
            let before=a.editor.project.clone();a.text_action(T::Content);a.input.as_mut().unwrap().insert("🙂");assert!(!a.confirm_input());assert_eq!(a.editor.project,before);a.action(Action::Cancel);assert!(a.input.is_none());
        }}
    }
}
#[cfg(test)]
mod extended_tests {
    use super::*;
    #[test]
    fn font_picker_commits_once_and_invalid_bevel_and_stale_selection_do_not_write() {
        let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.width=1440.;a.height=900.;a.text_action(T::Create);
        // Scroll the shared inspector to the actual font control before opening.
        a.inspector_scroll=a.property_section_offset(2201).unwrap_or(0.).min(a.inspector_max_scroll());a.ui=a.build_ui();
        let before=a.editor.project.clone();a.action(Action::DropdownOpen(ui::DropdownKind::TextFont));assert!(a.dropdown_is_open(ui::DropdownKind::TextFont));a.action(Action::DropdownChoose(1));assert_eq!(a.selected_text().unwrap().1.font,poseforge_core::TextFont::Serif);a.editor.undo();assert_eq!(a.editor.project,before);
        assert!(a.text_confirm(&Action::Text(T::Field(4)),"2.5").is_err());assert_eq!(a.editor.project,before);
        a.text_action(T::OnCurve);assert!(a.selected_text().unwrap().1.on_curve);a.text_confirm(&Action::Text(T::Field(3)),"0,015").unwrap();assert_eq!(a.selected_text().unwrap().1.bevel,0.015);
        a.text_action(T::Content);a.input.as_mut().unwrap().insert("  Надпись");assert!(a.confirm_input());assert_eq!(a.selected_text().unwrap().1.content,"  Надпись");
        a.ui=a.build_ui();a.action(Action::DropdownOpen(ui::DropdownKind::TextFont));a.create_empty_object();let p=a.editor.project.clone();a.action(Action::DropdownChoose(2));assert_eq!(a.editor.project,p);
    }
}
