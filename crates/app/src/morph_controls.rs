//! Shape creation uses regular editable meshes; all deltas remain document data.
use super::*;
use crate::ui::MorphAction as M;
const PAGE: usize = 6;
#[derive(Default)]
pub(super) struct MorphState {
    node: Option<u64>, selected: Option<usize>, page: usize, sources: bool, replace: bool,
    copy: Option<(u64,u64,Option<usize>)>, copy_revision: Option<u64>,
}
impl App {
    fn morph_owner(&self) -> Option<(u64,&poseforge_core::Mesh)> {
        let n = self.editor.project.node(self.editor.selected?)?;
        if n.curve.is_some() || n.physics.is_some() || n.fluid.is_some() || n.destruction.is_some() { return None; }
        Some((n.id,self.editor.project.mesh(n.mesh?)?))
    }
    pub(super) fn has_morph_panel(&self) -> bool { self.authoring.editing.is_none() && self.morph_owner().is_some() }
    pub(super) fn morph_panel_start(&self) -> f32 { if self.selected_primitive().is_some() { 278. } else { 232. } }
    fn morph_sources(&self) -> Vec<(u64,String)> {
        let Some((id,mesh))=self.morph_owner() else { return vec![]; };
        self.editor.project.nodes.iter().filter(|n| n.id != id && n.skin.is_none() && n.curve.is_none() && n.physics.is_none() && n.fluid.is_none() && n.destruction.is_none())
            .filter(|n| n.mesh.and_then(|m| self.editor.project.mesh(m)).is_some_and(|m| m.morph_targets.is_empty() && poseforge_core::morph_compatible(mesh,m)))
            .map(|n| (n.id,n.name.clone())).collect()
    }
    pub(super) fn morph_panel_bottom(&self) -> f32 {
        let Some((node,mesh))=self.morph_owner() else { return 222.; };
        let sources=self.morphs.node==Some(node) && self.morphs.sources;
        self.morph_panel_start()+260.+ if sources { self.morph_sources().len().min(PAGE) as f32*34.+38. } else { mesh.morph_targets.len().min(PAGE) as f32*82. }
    }
    fn morph_weight(&self, id:u64,index:usize) -> f32 {
        self.evaluated_interactive().ok().and_then(|e| e.morph_weights.get(&id).and_then(|w| w.get(index)).copied()).unwrap_or(0.)
    }
    pub(super) fn morph_action(&mut self, a:M) {
        if self.job.is_some() || self.scene_write_pending() || self.editor.playing || self.authoring.editing.is_some() { return; }
        let Some((id,_))=self.morph_owner() else { return; };
        if self.morphs.node != Some(id) { self.morphs.node=Some(id); self.morphs.selected=None; self.morphs.page=0; self.morphs.sources=false; self.morphs.replace=false; }
        let selected=self.morphs.selected;
        match a {
            M::Select(i) => { self.morphs.selected=Some(i); return; }
            M::Weight(i) => { self.input(Action::Morph(a),ui::number_caption(self.morph_weight(id,i) as f64)); return; }
            M::Rename(i) => { if let Some(t)=self.morph_owner().and_then(|(_,m)| m.morph_targets.get(i)) { self.input(Action::Morph(a),t.name.clone()); } return; }
            M::Sources => { self.morphs.sources=!self.morphs.sources; self.morphs.page=0; return; }
            M::Replace => { self.morphs.replace=!self.morphs.replace; return; }
            M::Page(step) => {
                let count=if self.morphs.sources {self.morph_sources().len()} else {self.morph_owner().unwrap().1.morph_targets.len()};
                self.morphs.page=(self.morphs.page as isize+step as isize).clamp(0,count.saturating_sub(1).div_euclid(PAGE) as isize) as usize; return;
            }
            M::Original => { if let Some((base,_,_))=self.morphs.copy { if self.editor.project.node(base).is_some() { self.action(Action::Select(base)); } } return; }
            _ => {}
        }
        if self.editor.has_staged() && a != M::Key { self.status="Сначала запишите или отмените изменения позы и морфов".into(); return; }
        let result=match a {
            M::Add => self.editor.create_morph(id).map(|i| { self.morphs.selected=Some(i); self.morphs.page=i/PAGE; self.morphs.sources=false; }),
            M::Delete(i) => self.editor.delete_morph(id,i).map(|()| { self.morphs.selected=None; self.morphs.page=0; }),
            M::Copy => {
                let revision=selected.and_then(|i| self.morph_owner()?.1.morph_targets.get(i).map(|t| t.positions.revision()));
                match self.editor.copy_morph_shape(id,selected) {
                    Ok(copy) => { self.morphs.copy=Some((id,copy,selected)); self.morphs.copy_revision=revision; self.action(Action::Select(copy)); self.set_properties_tab(7); self.status="Правьте вершины копии, затем в режиме объекта нажмите «Записать копию в морф»".into(); Ok(()) }
                    Err(e) => Err(e),
                }
            }
            M::Source(source) => self.editor.capture_morph(id,source,if self.morphs.replace {selected} else {None}).map(|i| { self.morphs.selected=Some(i); self.morphs.sources=false; self.morphs.page=i/PAGE; }),
            M::ApplyCopy => {
                if let Some((base,copy,index))=self.morphs.copy {
                    let revision=index.and_then(|i| self.editor.project.node(base).and_then(|n| n.mesh).and_then(|m| self.editor.project.mesh(m)).and_then(|m| m.morph_targets.get(i)).map(|t| t.positions.revision()));
                    if index.is_some() && revision != self.morphs.copy_revision { self.status="Исходная форма изменена или удалена. Выберите её заново через «Из объекта…»".into(); return; }
                    self.editor.capture_morph(base,copy,index).map(|i| { self.action(Action::Select(base)); self.morphs.node=Some(base); self.morphs.selected=Some(i); self.morphs.sources=false; self.morphs.page=i/PAGE; self.morphs.copy=Some((base,copy,Some(i))); self.morphs.copy_revision=self.morph_owner().and_then(|(_,m)| m.morph_targets.get(i)).map(|t| t.positions.revision()); })
                } else { return; }
            }
            M::Key => self.editor.key_morph_weights(id),
            _ => return,
        };
        self.result(result); self.invalidate_geometry(); self.update_dirty();
    }
    pub(super) fn morph_confirm(&mut self, a:&Action,text:&str) -> Result<bool,String> {
        let Action::Morph(a)=*a else { return Ok(false); };
        let Some((id,_))=self.morph_owner() else { return Err("Выберите меш".into()); };
        match a {
            M::Weight(i) => { let v=surfaces::number(text,0.,1.)?; self.editor.set_morph_weight(id,i,v).map_err(|e| e.to_string())?; }
            M::Rename(i) => self.editor.rename_morph(id,i,text).map_err(|e| e.to_string())?,
            _ => return Ok(false),
        }
        self.invalidate_geometry(); self.update_dirty(); Ok(true)
    }
    pub(super) fn draw_morph_panel(&self,u:&mut Ui,x:f32,w:f32) {
        if !self.has_morph_panel() { return; }
        let (node,mesh)=self.morph_owner().unwrap(); let y=self.morph_panel_start()+26.; let p=u.palette(); let half=(w-6.)*0.5;
        let same=self.morphs.node==Some(node); let selected=if same {self.morphs.selected} else {None}; let sources=same && self.morphs.sources;
        u.button([x,y,half,28.],"+ Форма",Action::Morph(M::Add),false);
        u.button([x+half+6.,y,half,28.],"Из объекта…",Action::Morph(M::Sources),sources);
        u.button([x,y+34.,w,28.],if selected.is_some() {"Копия выбранной формы"} else {"Копия текущего состояния"},Action::Morph(M::Copy),false);
        u.button([x,y+68.,w,28.],"Записать веса в ключ",Action::Morph(M::Key),false);
        if self.morphs.copy.is_some_and(|(a,b,_)| node==a || node==b) {
            u.button([x,y+102.,w,28.],"Записать копию в морф",Action::Morph(M::ApplyCopy),false);
            u.button([x,y+136.,w,28.],"Выбрать исходный объект",Action::Morph(M::Original),false);
        } else {
            u.fitted_label([x,y+104.,w,18.],"Форма = смещения тех же вершин.",11.,p.muted);
            u.fitted_label([x,y+128.,w,18.],"Копию можно править инструментами меша.",10.,p.muted);
            u.fitted_label([x,y+150.,w,18.],"Добавление граней меняет топологию.",10.,p.muted);
        }
        let count=if sources {self.morph_sources().len()} else {mesh.morph_targets.len()};
        let page=if same {self.morphs.page.min(count.saturating_sub(1)/PAGE)} else {0};
        u.quiet_icon_button([x,y+172.,26.,26.],"Назад",Action::Morph(M::Page(-1)),false,Icon::Previous);
        u.fitted_label([x+32.,y+176.,w-64.,18.],&format!("{} · {} / {}",if sources {"Формы из объектов"} else {"Морфы"},page+1,count.div_ceil(PAGE).max(1)),11.,p.muted);
        u.quiet_icon_button([x+w-26.,y+172.,26.,26.],"Далее",Action::Morph(M::Page(1)),false,Icon::Next);
        if sources {
            u.checkbox([x,y+208.,w,28.],"Заменить выбранную форму",Action::Morph(M::Replace),self.morphs.replace);
            let items=self.morph_sources();
            if items.is_empty() { u.fitted_label([x,y+244.,w,20.],"Нет копий с такой же топологией",11.,p.muted); }
            for (i,(id,name)) in items.iter().skip(page*PAGE).take(PAGE).enumerate() {
                u.button([x,y+244.+i as f32*34.,w,28.],name,Action::Morph(M::Source(*id)),false);
            }
        } else {
            let evaluated=self.evaluated_interactive().ok();
            if count==0 { u.fitted_label([x,y+210.,w,20.],"Создайте копию и измените её вершины.",10.,p.muted); }
            for (row,(i,target)) in mesh.morph_targets.iter().enumerate().skip(page*PAGE).take(PAGE).enumerate() {
                let y=y+208.+row as f32*82.;
                u.button([x,y,w-70.,28.],&target.name,Action::Morph(M::Select(i)),selected==Some(i));
                u.quiet_button([x+w-66.,y,38.,28.],"Имя",Action::Morph(M::Rename(i)),false);
                u.quiet_icon_button([x+w-26.,y,26.,28.],"Удалить форму",Action::Morph(M::Delete(i)),false,Icon::Close);
                let weight=evaluated.as_ref().and_then(|e| e.morph_weights.get(&node)).and_then(|v| v.get(i)).copied().unwrap_or(0.);
                u.number_field([x,y+32.,w,44.],"Вес",weight as f64,Action::Morph(M::Weight(i)),SliderRange::new(0.,1.,0.01));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_edit_capture_workflow_selects_original_and_updates_shape() {
        let mut p=Project::default(); p.meshes.push(poseforge_core::box_mesh(2,"Cube",Vec3::ONE,None)); let mut n=poseforge_core::Node::new(1,"Cube");n.mesh=Some(2);p.nodes.push(n);
        let mut a=App::new(None);a.editor=Editor::new(p);a.editor.selected=Some(1);
        a.morph_action(M::Copy);let copy=a.editor.selected.unwrap();assert_ne!(copy,1);
        let mid=a.editor.project.node(copy).unwrap().mesh.unwrap();a.editor.project.meshes.iter_mut().find(|m|m.id==mid).unwrap().vertices[0].position[0]+=0.4;
        a.morph_action(M::ApplyCopy);assert_eq!(a.editor.selected,Some(1));
        let mesh=a.editor.project.mesh(a.editor.project.node(1).unwrap().mesh.unwrap()).unwrap();assert!((mesh.morph_targets[0].positions[0][0]-0.4).abs()<1e-6);
        assert_eq!(a.morph_confirm(&Action::Morph(M::Weight(0)),"0.75"),Ok(true));assert_eq!(a.editor.project.node(1).unwrap().morph_weights,vec![0.75]);
    }
}
