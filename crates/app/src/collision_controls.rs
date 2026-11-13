//! Searchable per-object mesh references, shared by fluid and fracture.
use super::*;
use crate::ui::CollisionAction as C;
use poseforge_core::{CollisionKind as K, CollisionTarget, PhysicsKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Owner { node: u64, kind: K, source: bool }
struct Popup {
    owner: Owner,
    anchor: [f32;4],
    entries: Vec<(u64,String)>,
    candidate: Option<u64>,
    offset: usize,
    query: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Node,FluidDomainSettings,DestructionSettings,box_mesh};
    fn app(kind: K) -> App {
        let mut a=App::new(None);let mut p=Project::default();
        p.meshes.push(box_mesh(20,"Cube",Vec3::splat(0.3),None));
        for id in 1..=4 {let mut n=Node::new(id,if id==1 {"Источник"} else {"Препятствие"});n.mesh=Some(20);n.transform.translation.x=(id-1) as f32;p.nodes.push(n);}
        match kind {
            K::Fluid => p.node_mut(1).unwrap().fluid=Some(FluidDomainSettings {enabled:false,..Default::default()}),
            K::Destruction => p.node_mut(1).unwrap().destruction=Some(DestructionSettings::default()),
            K::Cloth => p.node_mut(1).unwrap().physics=Some(poseforge_core::PhysicsSettings::default()),
            K::Fx => {let n=p.node_mut(1).unwrap();n.mesh=None;n.empty=true;n.fx=Some(poseforge_core::FxSettings::default());},
        }
        p.node_mut(1).unwrap().collision_targets.set(kind,vec![]);
        p.node_mut(4).unwrap().locked=true;
        a.editor=Editor::new(p);a.reset_workflow_selection();a.preferences=Preferences::default();a.preferences.right_panel=true;
        a.width=1440.;a.height=900.;a.editor.selected=Some(1);
        a.set_properties_tab(match kind {K::Fluid=>4,K::Destruction=>13,K::Cloth=>11,K::Fx=>0});a.frame_all();a
    }
    fn show(a: &mut App, action: C) -> [f32;2] {
        let max=a.inspector_max_scroll();
        for step in 0..=(max/24.).ceil() as usize {
            a.inspector_scroll=(step as f32*24.).min(max);a.ui=a.build_ui();
            if let Some(point)=a.ui.action_point(&Action::Collision(action)) {return point;}
        }
        panic!("collision control must remain reachable");
    }
    #[test]
    fn cloth_has_the_shared_list_and_remove_button_without_type_dropdown() {
        let mut a=app(K::Cloth);let before=a.editor.project.clone();let max=a.inspector_max_scroll();let mut remove=false;
        for step in 0..=(max/24.).ceil() as usize {
            a.inspector_scroll=(step as f32*24.).min(max);let u=a.build_ui();
            assert!(u.action_point(&Action::DropdownOpen(ui::DropdownKind::PhysicsType)).is_none());
            remove|=u.action_point(&Action::Studio(ui::StudioAction::Physics(0))).is_some();
        }
        assert!(remove);a.action(Action::Studio(ui::StudioAction::Physics(0)));assert!(a.editor.project.node(1).unwrap().physics.is_none());
        a.editor.undo();assert_eq!(a.editor.project,before);
    }
    #[test]
    fn search_and_keyboard_add_mesh_and_eye_cross_are_independent_undo_steps() {
        for kind in [K::Fluid,K::Destruction,K::Cloth,K::Fx] {
            let mut a=app(kind);let before=a.editor.project.clone();let owner=a.collision_context().unwrap();
            a.mouse=show(&mut a,C::Open);a.mouse_down(MouseButton::Left);a.mouse_up(MouseButton::Left);
            assert!(a.collision_popup_open());
            a.input.as_mut().unwrap().insert("Препятствие");a.ui=a.build_ui();
            let popup=a.collisions.popup.as_ref().unwrap();
            assert_eq!(popup.entries.iter().map(|e|e.0).collect::<Vec<_>>(),vec![2,3]);
            assert!(popup.entries.iter().all(|e|e.1.contains('№')));
            a.collision_key(&Key::Named(NamedKey::ArrowDown));
            a.collision_key(&Key::Named(NamedKey::Enter));
            assert!(!a.collision_popup_open());assert_eq!(a.editor.selected,Some(1));
            assert_eq!(a.editor.project.resolved_collision_targets(1,kind),vec![CollisionTarget {node:3,enabled:true}]);
            assert_eq!(a.editor.project.node(3),before.node(3));assert_eq!(a.editor.undo_len(),1);
            a.collision_action(C::Toggle(1,3));
            assert!(!a.editor.project.resolved_collision_targets(1,kind)[0].enabled);
            assert!(a.editor.project.node(3).unwrap().visible);assert_eq!(a.editor.undo_len(),2);
            a.collision_action(C::Remove(1,3));
            assert!(a.editor.project.resolved_collision_targets(1,kind).is_empty());assert!(a.editor.project.node(3).is_some());
            assert_eq!(a.editor.undo_len(),3);assert_eq!(a.collision_context(),Some(owner));
            for _ in 0..3 {a.editor.undo();}
            assert_eq!(a.editor.project,before);a.fluid_domains.clear();
        }
    }
    #[test]
    fn picker_selects_outliner_mesh_without_changing_receiver_and_escape_is_clean() {
        let mut a=app(K::Destruction);let before=a.editor.project.clone();
        a.collision_action(C::Pick);a.action(Action::Select(4));
        assert!(a.collision_picking());assert_eq!(a.editor.project,before);
        a.action(Action::Select(2));assert!(!a.collision_picking());
        assert_eq!(a.editor.selected,Some(1));assert_eq!(a.editor.project.node(2),before.node(2));
        let chosen=a.editor.project.clone();
        a.collision_action(C::Pick);a.action(Action::Cancel);
        assert!(!a.collision_picking());assert_eq!(a.editor.project,chosen);
        a.collision_action(C::Add(2));assert_eq!(a.editor.undo_len(),1);
    }
    #[test]
    fn picker_hits_viewport_mesh_and_does_not_click_through_unrelated_controls() {
        let mut a=app(K::Destruction);a.collision_action(C::Pick);a.ui=a.build_ui();
        let point=a.editor.evaluated().unwrap().global[&3].w_axis.truncate();
        a.mouse=a.project_point(point).unwrap();assert!(a.canvas_input_at(a.mouse));
        assert!(a.collision_pointer(MouseButton::Left));
        assert_eq!(a.editor.project.resolved_collision_targets(1,K::Destruction).len(),1);
        assert_eq!(a.editor.selected,Some(1));let chosen=a.editor.project.clone();
        a.collision_action(C::Pick);a.ui.hits.clear();
        a.ui.hits.push(ui::Hit {rect:[0.,0.,30.,30.],action:Action::Delete});a.mouse=[12.,12.];
        assert!(a.collision_pointer(MouseButton::Left));assert_eq!(a.editor.project,chosen);
        assert!(!a.collision_picking());
    }
    #[test]
    fn stale_owner_and_removed_candidate_never_assign_to_another_mesh() {
        let mut a=app(K::Destruction);a.mouse=show(&mut a,C::Open);a.collision_action(C::Open);
        a.editor.delete_node(2).unwrap();let before=a.editor.project.clone();
        a.collision_action(C::Add(2));assert_eq!(a.editor.project,before);
        a.close_collision_selection();a.mouse=show(&mut a,C::Open);a.collision_action(C::Open);
        a.editor.selected=Some(3);a.collision_action(C::Add(3));
        assert_eq!(a.editor.project,before);assert!(!a.collision_popup_open());
        // A delayed eye/cross hit belongs to the original receiver, even when
        // the newly selected receiver contains the same obstacle.
        a.editor.project.node_mut(3).unwrap().destruction=Some(DestructionSettings::default());
        a.editor.project.node_mut(3).unwrap().collision_targets.destruction=Some(vec![CollisionTarget {node:4,enabled:true}]);
        let before=a.editor.project.clone();
        a.collision_action(C::Toggle(1,4));a.collision_action(C::Remove(1,4));
        assert_eq!(a.editor.project,before);
    }
    #[test]
    fn dropdown_empty_search_cancel_and_scroll_keep_document_and_camera_unchanged() {
        let mut a=app(K::Destruction);let before=a.editor.project.clone();let distance=a.distance;
        a.mouse=show(&mut a,C::Open);a.collision_action(C::Open);
        a.input.as_mut().unwrap().insert("Несуществующий меш");a.ui=a.build_ui();
        assert!(a.collisions.popup.as_ref().unwrap().entries.is_empty());
        a.collision_key(&Key::Named(NamedKey::Enter));assert_eq!(a.editor.project,before);
        a.scroll(10.);assert_eq!(a.distance,distance);
        a.collision_key(&Key::Named(NamedKey::Escape));assert!(!a.collision_popup_open());assert!(a.input.is_none());
        assert_eq!(a.editor.undo_len(),0);
    }
    #[test]
    fn growing_lists_and_popups_stay_reachable_in_both_themes_and_dpi() {
        for kind in [K::Fluid,K::Destruction,K::Cloth,K::Fx] {for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
            let mut a=app(kind);a.scale=scale;a.preferences.theme=theme;a.preferences.right_width=Some(300.);
            let owner=a.collision_context().unwrap();a.edit_collision(owner,C::Add(2));let first=a.inspector_max_scroll();a.edit_collision(owner,C::Add(3));
            assert!(a.inspector_max_scroll()>first);
            for action in [C::Toggle(1,2),C::Remove(1,2),C::Toggle(1,3),C::Remove(1,3),C::Pick] {show(&mut a,action);}
            a.mouse=show(&mut a,C::Open);a.collision_action(C::Open);a.ui=a.build_ui();
            assert!(a.ui.action_point(&Action::Collision(C::Search)).is_some());
            let rect=a.collision_popup_rect(a.collisions.popup.as_ref().unwrap());
            assert!(rect[0]>=0.&&rect[1]>=0.&&rect[0]+rect[2]<=a.width&&rect[1]+rect[3]<=a.height);
            a.close_collision_selection();a.fluid_domains.clear();
        }}}
    }
    #[test]
    fn fx_source_search_and_picker_accept_curves_and_keep_receiver_selected() {
        let mut a=app(K::Fx);let mut curve=Node::new(30,"Кривая источника");
        curve.curve=Some(poseforge_core::CurveData{points:vec![[0.,0.,0.],[2.,0.,0.]],..Default::default()});a.editor.project.nodes.push(curve);
        let before=a.editor.project.clone();a.mouse=show(&mut a,C::SourceOpen);a.collision_action(C::SourceOpen);
        a.input.as_mut().unwrap().insert("Кривая");a.ui=a.build_ui();
        assert_eq!(a.collisions.popup.as_ref().unwrap().entries.iter().map(|e|e.0).collect::<Vec<_>>(),vec![30]);
        a.collision_key(&Key::Named(NamedKey::Enter));
        assert_eq!(a.editor.selected,Some(1));assert_eq!(a.selected_fx().unwrap().1.source,Some(30));assert_eq!(a.path_target(),None);
        a.editor.undo();assert_eq!(a.editor.project,before);
        a.collision_action(C::SourcePick);a.action(Action::Select(2));
        assert_eq!(a.editor.selected,Some(1));assert_eq!(a.selected_fx().unwrap().1.source,Some(2));
        let chosen=a.editor.project.clone();a.collision_action(C::SourcePick);a.action(Action::Cancel);assert_eq!(a.editor.project,chosen);
        a.fx_action(ui::FxAction::Defaults);assert_eq!(a.selected_fx().unwrap().1.source,Some(2));
        a.fx_action(ui::FxAction::Kind(2));assert_eq!(a.selected_fx().unwrap().1.source,Some(2));
        a.fx_action(ui::FxAction::Source(None));assert!(a.selected_fx().unwrap().1.source.is_none());assert!(a.path_target().is_some());
    }
    #[test]
    fn fx_source_picker_cancels_when_receiver_changes_and_rejects_deleted_targets() {
        let mut a=app(K::Fx);a.collision_action(C::SourcePick);a.editor.delete_node(2).unwrap();
        let before=a.editor.project.clone();a.action(Action::Select(2));assert_eq!(a.editor.project,before);assert!(a.collision_picking());
        a.editor.selected=Some(3);a.action(Action::Select(3));assert!(!a.collision_picking());assert_eq!(a.editor.project,before);
    }
}
#[derive(Default)]
pub(super) struct State {
    popup: Option<Popup>,
    picking: Option<Owner>,
    query: String,
}

impl App {
    fn collision_owner(&self, kind: K) -> Option<Owner> {
        let id = match kind { K::Fluid => self.selected_fluid()?.0, K::Destruction => self.destruction_source()?, K::Cloth|K::Fx => self.editor.selected? };
        let node = self.editor.project.node(id)?;
        let present = match kind { K::Fluid => node.fluid.is_some(), K::Destruction => node.destruction.is_some(), K::Cloth => node.physics.is_some_and(|s|s.kind==PhysicsKind::Cloth), K::Fx=>node.fx.is_some() };
        present.then_some(Owner { node:id,kind,source:false })
    }
    fn collision_context(&self) -> Option<Owner> {
        match self.surface.tab { 0=>self.collision_owner(K::Fx), 4 => self.collision_owner(K::Fluid), 6 => self.collision_owner(K::Fluid).or_else(||self.collision_owner(K::Cloth)), 11 => self.collision_owner(K::Cloth), 13 => self.collision_owner(K::Destruction), _ => None }
    }
    fn collision_editable(&self, owner: Owner) -> bool {
        self.collision_context() == Some(Owner{source:false,..owner}) && !self.editor.project.node_locked(owner.node)
            && self.job.is_none() && self.native_picker.is_none() && !self.scene_write_pending()
            && !self.editor.playing && !self.editor.has_staged()
    }
    pub(super) fn collision_popup_open(&self) -> bool { self.collisions.popup.is_some() }
    pub(super) fn collision_picking(&self) -> bool { self.collisions.picking.is_some() }
    pub(super) fn close_collision_selection(&mut self) {
        if self.input.as_ref().is_some_and(|i| i.purpose == Action::Collision(C::Search)) { self.end_input(); }
        self.collisions.popup = None; self.collisions.picking = None;
    }
    pub(super) fn collision_layout_shift(&self, kind: K) -> f32 {
        self.collision_owner(kind).map_or(0.,|o| self.collision_list_height(o)-20.)
    }
    fn collision_list_height(&self, owner: Owner) -> f32 {
        40. + self.editor.project.resolved_collision_targets(owner.node,owner.kind).len().max(1) as f32*32.
    }
    pub(super) fn draw_collision_list(&self, u: &mut Ui, kind: K, x: f32, y: f32, w: f32) {
        let Some(owner) = self.collision_owner(kind) else { return; };
        let p = u.palette(); let editable = self.collision_editable(owner);
        let field = [x,y,w-38.,32.]; let pick = [x+w-32.,y,32.,32.];
        if editable {
            u.button(field,"",Action::Collision(C::Open),self.collision_popup_open());
            u.quiet_icon_button(pick,"",Action::Collision(C::Pick),self.collisions.picking==Some(owner),Icon::Eyedropper);
        } else {
            u.disabled_button(field,""); u.disabled_icon_button(pick,"",Icon::Eyedropper);
        }
        Icon::Object.draw(u,[x+8.,y+8.,16.,16.],p.muted);
        u.fitted_label([x+30.,y+8.,w-96.,18.],"Добавить меш…",12.,if editable {p.text} else {p.disabled});
        let cx=x+w-52.;
        u.smooth_segment([cx-4.,y+13.],[cx,y+17.],1.5,p.muted);
        u.smooth_segment([cx,y+17.],[cx+4.,y+13.],1.5,p.muted);
        let entries = self.editor.project.resolved_collision_targets(owner.node,kind);
        let top=y+40.;
        u.outlined_surface([x,top,w,entries.len().max(1) as f32*32.],p.panel,p.border);
        if entries.is_empty() {
            u.fitted_label([x+10.,top+8.,w-20.,18.],"Добавьте меши для столкновений",11.,p.muted);
        }
        let nodes: BTreeMap<_,_> = self.editor.project.nodes.iter().map(|n| (n.id,n)).collect();
        let mut names=BTreeMap::new();
        for entry in &entries {if let Some(node)=nodes.get(&entry.node) {*names.entry(node.name.as_str()).or_insert(0usize)+=1;}}
        for (row,entry) in entries.iter().enumerate() {
            let row_y=top+row as f32*32.;
            if row>0 { u.rect([x+1.,row_y,w-2.,1.],p.border); }
            Icon::Object.draw(u,[x+8.,row_y+8.,16.,16.],if entry.enabled {p.text} else {p.muted});
            let name=nodes.get(&entry.node).map_or("Меш удалён",|n| n.name.as_str());
            let name=if names.get(name).copied().unwrap_or(0)>1 {format!("{name} · №{}",entry.node)} else {name.to_owned()};
            u.fitted_label([x+30.,row_y+8.,w-96.,18.],&name,12.,if entry.enabled {p.text} else {p.muted});
            if editable {
                u.eye_button([x+w-62.,row_y+2.,28.,28.],Action::Collision(C::Toggle(owner.node,entry.node)),entry.enabled);
                u.quiet_icon_button([x+w-31.,row_y+2.,28.,28.],"",Action::Collision(C::Remove(owner.node,entry.node)),false,Icon::Close);
            } else {
                u.disabled_icon_button([x+w-62.,row_y+2.,28.,28.],"",Icon::Eye);
                u.disabled_icon_button([x+w-31.,row_y+2.,28.,28.],"",Icon::Close);
            }
        }
    }
    fn collision_candidates(&self, owner: Owner, query: &str) -> Vec<(u64,String)> {
        let added: BTreeSet<_> = self.editor.project.resolved_collision_targets(owner.node,owner.kind).into_iter().map(|e|e.node).collect();
        let selectable=self.editor.project.selectable_node_ids();
        let candidates=self.editor.project.collision_candidate_ids(owner.node);
        let query=query.trim().chars().take(128).collect::<String>().to_lowercase();
        let mut counts=BTreeMap::new();
        for n in self.editor.project.nodes.iter().filter(|n| n.mesh.is_some() || (owner.source && n.curve.is_some())) { *counts.entry(n.name.as_str()).or_insert(0usize)+=1; }
        self.editor.project.nodes.iter().filter(|n| (n.mesh.is_some() || (owner.source && n.curve.is_some())) && (owner.source || !added.contains(&n.id))
            && selectable.contains(&n.id) && self.in_isolation(n.id)
            && (query.is_empty() || n.name.to_lowercase().contains(&query) || n.id.to_string().contains(&query))
            && if owner.source {self.editor.project.fx_source_allowed(owner.node,n.id)}else{candidates.contains(&n.id)})
            .map(|n| (n.id,if counts.get(n.name.as_str()).copied().unwrap_or(0)>1 {format!("{} · №{}",n.name,n.id)} else {n.name.clone()})).collect()
    }
    fn collision_query(&self) -> &str {
        self.input.as_ref().filter(|i| i.purpose==Action::Collision(C::Search)).map_or(self.collisions.query.as_str(),|i|i.value.as_str())
    }
    fn refresh_collision_popup(&mut self) {
        let Some(owner)=self.collisions.popup.as_ref().map(|p|p.owner) else { return; };
        if !self.collision_editable(owner) { self.close_collision_selection(); return; }
        let query=self.collision_query().to_owned(); let entries=self.collision_candidates(owner,&query);
        let popup=self.collisions.popup.as_mut().unwrap();
        if popup.query!=query { popup.offset=0; popup.candidate=None; }
        if popup.candidate.is_none_or(|id| !entries.iter().any(|e|e.0==id)) { popup.candidate=entries.first().map(|e|e.0); }
        popup.query=query; popup.entries=entries;
    }
    fn collision_popup_rect(&self, popup: &Popup) -> [f32;4] {
        let w=popup.anchor[2].max(260.).min((self.width-16.).max(1.));
        let h=(52.+popup.entries.len().clamp(1,7) as f32*32.).min((self.height-16.).max(1.));
        let x=popup.anchor[0].clamp(8.,(self.width-w-8.).max(8.));
        let below=popup.anchor[1]+popup.anchor[3]+4.;
        let y=if below+h<=self.height-8. {below} else {(popup.anchor[1]-h-4.).max(8.)};
        [x,y,w,h]
    }
    pub(super) fn draw_collision_popup(&mut self, u: &mut Ui) {
        if !self.collision_popup_open() { return; }
        let open=if self.collisions.popup.as_ref().is_some_and(|p|p.owner.source) {C::SourceOpen}else{C::Open};
        if let Some(hit)=u.hits.iter().find(|h| h.action==Action::Collision(open)) {
            self.collisions.popup.as_mut().unwrap().anchor=hit.rect;
        } else { self.close_collision_selection(); return; }
        self.refresh_collision_popup();
        let Some(popup)=&self.collisions.popup else {return;};
        let [x,y,w,h]=self.collision_popup_rect(popup); let p=u.palette();
        let physical=[x,y,w,h].map(|v|v*u.scale);
        u.texts.retain(|t| t.x>=physical[0]+physical[2] || t.x+t.text.chars().count() as f32*t.size<=physical[0]
            || t.y>=physical[1]+physical[3] || t.y+t.size*1.5<=physical[1]);
        u.hits.clear();
        u.hits.push(ui::Hit {rect:[0.,0.,self.width,self.height],action:Action::Collision(C::Close)});
        u.outlined_surface([x,y,w,h],p.panel,p.border);
        Icon::Search.draw(u,[x+9.,y+15.,16.,16.],p.muted);
        let query=self.collision_query();
        u.field([x+32.,y+8.,w-40.,30.],if query.is_empty(){if popup.owner.source {"Поиск мешей и кривых"}else{"Поиск мешей"}}else{query},Action::Collision(C::Search));
        let page=((h-52.)/32.).floor().max(1.) as usize;
        let offset=popup.offset.min(popup.entries.len().saturating_sub(page));
        if popup.entries.is_empty() { u.fitted_label([x+12.,y+54.,w-24.,18.],"Подходящих объектов нет",12.,p.muted); }
        for (row,(id,name)) in popup.entries.iter().skip(offset).take(page).enumerate() {
            let r=[x+6.,y+46.+row as f32*32.,w-20.,30.];
            u.menu_row(r,Action::Collision(C::Add(*id)),popup.candidate==Some(*id));
            let icon=if self.editor.project.node(*id).is_some_and(|n|n.curve.is_some()) {Icon::Curve}else{Icon::Object};
            icon.draw(u,[r[0]+6.,r[1]+7.,16.,16.],p.muted);
            u.fitted_label([r[0]+30.,r[1]+7.,r[2]-38.,18.],name,12.,p.text);
        }
        if popup.entries.len()>page {
            let track=h-52.; let thumb=track*page as f32/popup.entries.len() as f32;
            let top=(track-thumb)*offset as f32/(popup.entries.len()-page) as f32;
            u.rounded_rect([x+w-8.,y+46.+top,3.,thumb],p.muted,1.5);
        }
    }
    pub(super) fn collision_search_confirm(&mut self, text: &str) {
        self.collisions.query=text.chars().take(128).collect();
    }
    pub(super) fn route_collision_action(&mut self, action: &Action) -> bool {
        if let Some(owner)=self.collisions.picking {
            if let Action::Select(id)=action { self.edit_collision(owner,C::Add(*id)); return true; }
            if *action==Action::Cancel { self.close_collision_selection(); self.redraw(); return true; }
            if !matches!(action,Action::Collision(_)) { self.close_collision_selection(); }
        }
        if self.collision_popup_open() && !matches!(action,Action::Collision(_) | Action::ConfirmInput) {
            self.close_collision_selection(); self.redraw();
            if *action==Action::Cancel { return true; }
        }
        false
    }
    pub(super) fn collision_action(&mut self, action: C) {
        if action==C::Close { self.close_collision_selection(); self.redraw(); return; }
        // A menu row from an already closed popup must not become a new edit.
        if matches!(action,C::Add(_)) && !self.collision_popup_open() && !self.collision_picking() {return;}
        let owner=if matches!(action,C::SourceOpen|C::SourcePick) {self.collision_owner(K::Fx).map(|o|Owner{source:true,..o})}
            else {self.collisions.popup.as_ref().map(|p|p.owner).or(self.collisions.picking).or_else(||self.collision_context())};
        let Some(owner)=owner.filter(|o|self.collision_editable(*o)) else {self.close_collision_selection();return;};
        match action {
            C::Open|C::SourceOpen => {
                if self.collision_popup_open() {self.close_collision_selection();self.redraw();return;}
                if self.has_modal() {return;}
                let Some(anchor)=self.ui.hits.iter().find(|h|h.action==Action::Collision(action)).map(|h|h.rect) else {return;};
                self.collisions.picking=None; self.collisions.query.clear();
                self.collisions.popup=Some(Popup {owner,anchor,entries:Vec::new(),candidate:None,offset:0,query:String::new()});
                self.input(Action::Collision(C::Search),String::new()); self.refresh_collision_popup();
            }
            C::Pick|C::SourcePick => {
                let was=self.collisions.picking==Some(owner); self.close_collision_selection();
                if !was && !self.has_modal() {
                    self.cancel_drag(); self.authoring.reference_pick=None; self.focus_pick=None; self.light_aim=None;
                    self.collisions.picking=Some(owner);
                    self.status=if owner.source {"Источник FX: выберите меш или кривую · Esc — отмена"}else{"Пипетка столкновений: выберите меш в просмотре или сцене · Esc — отмена"}.into();
                }
            }
            C::Search => if self.collision_popup_open() {self.input(Action::Collision(C::Search),self.collisions.query.clone());},
            C::ClearSearch => if self.collision_popup_open() {self.collisions.query.clear();self.input(Action::Collision(C::Search),String::new());},
            C::Add(_) | C::Toggle(..) | C::Remove(..) => self.edit_collision(owner,action),
            C::Close => {}
        }
        self.redraw();
    }
    fn edit_collision(&mut self, owner: Owner, action: C) {
        if !self.collision_editable(owner) {self.close_collision_selection();return;}
        if owner.source {
            let C::Add(id)=action else{return;};
            if !self.node_selectable(id) || !self.editor.project.fx_source_allowed(owner.node,id) {
                self.status="Выберите доступный меш или кривую · Esc — отмена".into();self.redraw();return;
            }
            self.close_collision_selection();self.fx_action(ui::FxAction::Source(Some(id)));return;
        }
        let mut entries=self.editor.project.resolved_collision_targets(owner.node,owner.kind);
        match action {
            C::Add(id) => {
                if entries.iter().any(|e|e.node==id) {self.status="Этот меш уже добавлен в столкновения.".into();self.redraw();return;}
                if entries.len()>=owner.kind.limit() || !self.editor.project.collision_target_allowed(owner.node,id) || !self.node_selectable(id) {
                    self.status=format!("Выберите другой доступный меш · не более {} мешей в списке · Esc — отмена",owner.kind.limit());self.redraw();return;
                }
                entries.push(CollisionTarget {node:id,enabled:true});
            }
            C::Toggle(source,id) => {if source!=owner.node{return;}let Some(entry)=entries.iter_mut().find(|e|e.node==id) else{return;};entry.enabled=!entry.enabled;}
            C::Remove(source,id) => {if source!=owner.node{return;}let len=entries.len();entries.retain(|e|e.node!=id);if entries.len()==len{return;}}
            _ => return,
        }
        self.close_collision_selection();
        let result=self.editor.transact("Меши для столкновений",|p|{
            let node=p.node_mut(owner.node).unwrap();node.collision_targets.set(owner.kind,entries);
            if owner.kind==K::Fx && matches!(action,C::Add(_)) {if let Some(f)=&mut node.fx {f.mesh_collision=true;}}
        });
        if self.result(result).is_some() {
            if owner.kind==K::Fluid {self.tick_fluid_domains();}
            if owner.kind==K::Cloth {self.studio.physics.reset();}
            self.status=if owner.kind==K::Fluid {"Список столкновений обновлён; поверхность жидкости обновляется автоматически."}
                else if owner.kind==K::Cloth {"Столкновения ткани обновлены; расчёт начнётся с исходной формы."}
                else if owner.kind==K::Fx {"Столкновения FX обновлены; поток подготавливается."}
                else {"Список столкновений обновлён. Перезапишите падение осколков."}.into();
            self.invalidate();
        }
        self.redraw();
    }
    pub(super) fn collision_pointer(&mut self, button: MouseButton) -> bool {
        if self.collision_popup_open() {
            self.refresh_collision_popup();
            if button!=MouseButton::Left {self.close_collision_selection();self.redraw();return true;}
            match self.ui.hit(self.mouse) {
                Some(Action::Collision(C::Search)) => {
                    if let Some(rect)=self.active_field_rect() {if let Some(input)=&mut self.input {
                        input.click(rect,self.mouse,self.modifiers.shift_key());self.text_selecting=true;
                    }}
                }
                Some(Action::Collision(action)) => self.collision_action(action),
                _ => self.close_collision_selection(),
            }
            self.redraw();return true;
        }
        let Some(owner)=self.collisions.picking else {return false;};
        if !self.collision_editable(owner) {self.close_collision_selection();return true;}
        if button==MouseButton::Right {self.close_collision_selection();self.redraw();return true;}
        if button!=MouseButton::Left {return false;}
        if let Some(action)=self.ui.hit(self.mouse) {
            if let Action::Select(id)=action {self.edit_collision(owner,C::Add(id));}
            else {self.close_collision_selection();}
        } else if self.canvas_input_at(self.mouse) {
            if owner.source {if let Some(id)=self.pick_curve_at(self.mouse) {self.edit_collision(owner,C::Add(id));self.redraw();return true;}}
            if let Ok(eval)=self.editor.evaluated() {
                let project=self.picking_project();
                let hit=crate::picking::pick_mesh(&project,&eval,self.view().view_projection(),self.viewport(),self.mouse);
                drop(project);
                if let Some(id)=hit {self.edit_collision(owner,C::Add(id));}
                else {self.status="Здесь нет меша. Пипетка остаётся активной · Esc — отмена".into();}
            }
        } else {self.close_collision_selection();}
        self.redraw();true
    }
    pub(super) fn collision_scroll(&mut self, delta: f32) -> bool {
        if !self.collision_popup_open() {return false;}
        self.refresh_collision_popup();
        let Some(popup)=&self.collisions.popup else{return true;};
        let rect=self.collision_popup_rect(popup);let page=((rect[3]-52.)/32.).floor().max(1.) as usize;
        if delta.is_finite() && delta!=0. && ui::contains(rect,self.mouse) {
            let max=popup.entries.len().saturating_sub(page);
            let offset=(popup.offset as isize-delta.signum() as isize*delta.abs().round().max(1.) as isize).clamp(0,max as isize) as usize;
            self.collisions.popup.as_mut().unwrap().offset=offset;
        }
        self.redraw();true
    }
    pub(super) fn collision_pointer_move(&mut self) {
        if let Some(Action::Collision(C::Add(id)))=self.ui.hit(self.mouse) {
            if let Some(popup)=&mut self.collisions.popup {
                if popup.entries.iter().any(|e|e.0==id) {popup.candidate=Some(id);}
            }
        }
    }
    pub(super) fn collision_key(&mut self, key: &Key) -> bool {
        if !self.collision_popup_open() {return false;}
        if self.input.as_ref().is_some_and(|i|!i.preedit.is_empty()) {return false;}
        self.refresh_collision_popup();
        let Some(popup)=&self.collisions.popup else{return true;};
        let page=((self.collision_popup_rect(popup)[3]-52.)/32.).floor().max(1.) as usize;
        match key {
            Key::Named(NamedKey::Escape | NamedKey::F4) => self.close_collision_selection(),
            Key::Named(NamedKey::Enter) => if let Some(id)=popup.candidate {self.collision_action(C::Add(id));},
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::PageUp | NamedKey::PageDown) => {
                let index=popup.entries.iter().position(|e|Some(e.0)==popup.candidate).unwrap_or(0);
                let step=if matches!(key,Key::Named(NamedKey::PageUp | NamedKey::PageDown)){page}else{1};
                let next=if matches!(key,Key::Named(NamedKey::ArrowDown | NamedKey::PageDown)){(index+step).min(popup.entries.len().saturating_sub(1))}else{index.saturating_sub(step)};
                let popup=self.collisions.popup.as_mut().unwrap();popup.candidate=popup.entries.get(next).map(|e|e.0);
                if next<popup.offset {popup.offset=next;} else if next>=popup.offset+page {popup.offset=next+1-page;}
            }
            Key::Named(NamedKey::Tab) => {},
            _ => return false,
        }
        self.redraw();true
    }
}
