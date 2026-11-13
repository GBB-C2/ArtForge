//! Non-rendering transform helpers use the same visibility, selection and undo rules as objects.
use super::*;
use crate::gizmo;
use glam::Vec2;
use poseforge_core::{EmptyShape as E, ForceFieldKind as F, ForceFieldSettings};
use crate::ui::HelperAction as H;
pub(super) fn field_icon(kind:F)->Icon {match kind {F::Force=>Icon::Force,F::Wind=>Icon::Wind,F::Vortex=>Icon::Vortex,F::Magnet=>Icon::Magnet}}
impl App {
    pub(super) fn create_empty_object(&mut self) {self.helper_action(H::Empty(0));}
    pub(super) fn selected_helper(&self)->Option<&poseforge_core::Node> {self.editor.project.node(self.editor.selected?).filter(|n|n.empty)}
    pub(super) fn helper_editable(&self,id:u64)->bool {self.node_selectable(id)&&self.job.is_none()&&self.native_picker.is_none()&&!self.scene_write_pending()&&!self.editor.has_staged()&&!self.editor.playing}
    pub(super) fn helper_panel_bottom(&self)->f32 {if self.selected_fx().is_some() {return self.fx_panel_bottom();}if self.selected_helper().is_some_and(|n|n.force_field.is_some()) {1056.}else{656.}}
    pub(super) fn helper_action(&mut self,action:H) {
        if self.job.is_some() || self.native_picker.is_some() || self.scene_write_pending() || self.editor.has_staged() {return;}
        if matches!(action,H::Play|H::Reset) { self.force_preview_action(action==H::Reset); return; }
        if matches!(action,H::Empty(_)|H::CreateField(_)) {
            let result=match action {
                H::Empty(i)=>{let Some(&shape)=E::ALL.get(i as usize) else{return;};self.editor.create_empty_shape(shape)},
                H::CreateField(i)=>{let Some(&kind)=F::ALL.get(i as usize) else{return;};self.editor.create_force_field(kind)},_=>unreachable!(),
            };
            if let Some(id)=self.result(result) {
                self.editor.playing=false;self.editor.mode=EditorMode::Scene;self.authoring=Default::default();self.curves=Default::default();self.primitive_panel=None;
                self.action(Action::Select(id));self.set_properties_tab(0);self.preferences.right_panel=true;self.tool=Tool::Move;
                self.status=if matches!(action,H::CreateField(_)) {"Силовое поле создано · параметры в «Объект» · ветер и вихрь направлены по локальной Z"}else{"Пустышка создана в центре координат · можно использовать как родителя"}.into();self.redraw();
            } return;
        }
        let Some(node)=self.selected_helper().filter(|n|self.helper_editable(n.id)).cloned() else{return;};
        if action==H::Size {self.input(Action::Helper(action),ui::number_caption(node.empty_size as f64));return;}
        if let H::Number(i)=action {if let Some(f)=node.force_field {if let Some(v)=[f.strength,f.radius,f.falloff,f.damping].get(i as usize) {self.input(Action::Helper(action),ui::number_caption(*v as f64));}}return;}
        let mut next=node.clone();match action {
            H::Shape(i)=>{let Some(&shape)=E::ALL.get(i as usize) else{return;};next.empty_shape=shape;},
            H::Kind(i)=>{let Some(&kind)=F::ALL.get(i as usize) else{return;};let Some(f)=&mut next.force_field else{return;};f.kind=kind;},
            H::Enabled=>{let Some(f)=&mut next.force_field else{return;};f.enabled=!f.enabled;},
            H::Target(i)=>{let Some(f)=&mut next.force_field else{return;};if i==3 {f.fx=!f.fx;}else{let Some(t)=f.targets.get_mut(i as usize) else{return;};*t=!*t;}},_=>return,
        }
        if next==node {return;}let result=self.editor.transact("Параметры вспомогательного объекта",|p|*p.node_mut(node.id).unwrap()=next);
        if self.result(result).is_some() {self.redraw();}
    }
    pub(super) fn helper_confirm(&mut self,action:&Action,text:&str)->Result<bool,String> {
        let Action::Helper(action)=*action else{return Ok(false);};
        if !matches!(action,H::Size|H::Number(_)) {return Ok(false);}
        let node=self.selected_helper().filter(|n|self.helper_editable(n.id)).ok_or("Объект больше недоступен")?.clone();let mut next=node.clone();
        match action {
            H::Size=>next.empty_size=surfaces::number(text,0.01,1000.)?,
            H::Number(i)=>{let f=next.force_field.as_mut().ok_or("Выберите силовое поле")?;
                match i {0=>f.strength=surfaces::number(text,-1000.,1000.)?,1=>f.radius=surfaces::number(text,0.01,10000.)?,2=>f.falloff=surfaces::number(text,0.,8.)?,3=>f.damping=surfaces::number(text,0.,20.)?,_=>return Ok(false)}f.validate().map_err(|e|e.to_string())?;},_=>return Ok(false),
        }
        if next!=node {self.editor.transact("Параметры вспомогательного объекта",|p|*p.node_mut(node.id).unwrap()=next).map_err(|e|e.to_string())?;self.update_dirty();}
        Ok(true)
    }
    pub(super) fn draw_helper_properties(&self,u:&mut Ui,rx:f32,rw:f32) {
        if self.selected_fx().is_some() {self.draw_fx_properties(u,rx,rw);return;}
        let Some(n)=self.selected_helper() else{return;};let x=rx+16.;let w=rw-32.;let editable=self.helper_editable(n.id);
        let (kind,label)=if let Some(f)=n.force_field {(ui::DropdownKind::ForceKind,f.kind.label())}else{(ui::DropdownKind::EmptyShape,n.empty_shape.label())};
        u.label(x,528.,if n.force_field.is_some() {"Тип поля"}else{"Форма пустышки"},11.,u.palette().muted);
        if editable {self.draw_dropdown_button(u,[x,550.,w,30.],kind,label);}else{u.disabled_button([x,550.,w,30.],label);}
        if let Some(f)=n.force_field {
            let running=self.force_preview_running(f);let half=(w-8.)*0.5;
            let controls=self.force_preview_editable(n.id);
            if controls && (running || (f.enabled && (f.fx || f.targets.iter().any(|enabled|*enabled)))) {
                u.icon_button([x,590.,half,30.],if running {"Пауза"}else{"Запустить"},Action::Helper(H::Play),running,if running {Icon::Pause}else{Icon::Play});
            } else {u.disabled_icon_button([x,590.,half,30.],"Запустить",Icon::Play);}
            if controls && (f.fx || f.targets.iter().any(|enabled|*enabled)) {u.icon_button([x+half+8.,590.,half,30.],"В начало",Action::Helper(H::Reset),false,Icon::Start);}
            else {u.disabled_icon_button([x+half+8.,590.,half,30.],"В начало",Icon::Start);}
            if editable {u.checkbox([x,632.,w,28.],"Включено",Action::Helper(H::Enabled),f.enabled);}else{u.disabled_button([x,632.,w,28.],"Включено");}
            for (i,(label,value,min,max,step)) in [("Сила, м/с²",f.strength,-100.,100.,0.1),("Радиус, м",f.radius,0.01,20.,0.01),("Спад к краю",f.falloff,0.,8.,0.05),("Торможение магнита",f.damping,0.,20.,0.05)].into_iter().enumerate() {
                let r=[x,674.+i as f32*46.,w,36.];
                if editable && (i!=3 || f.kind==F::Magnet) {u.number_field(r,label,value as f64,Action::Helper(H::Number(i as u8)),SliderRange::new((min as f64).min(value as f64),(max as f64).max(value as f64),step));}
                else {u.disabled_button(r,&format!("{label}: {}",ui::number_caption(value as f64)));}
            }
            for (i,label) in ["Жидкость","Ткань и мягкие тела","Осколки разрушения","FX"].into_iter().enumerate() {
                let checked=if i==3 {f.fx}else{f.targets[i]};
                let r=[x,874.+i as f32*34.,w,28.];if editable {u.checkbox(r,label,Action::Helper(H::Target(i as u8)),checked);}else{u.disabled_checkbox(r,label,checked);}
            }
            u.fitted_label([x,1020.,w,22.],"Предпросмотр выбранных типов · без ключей",10.,u.palette().muted);
        } else {
            if editable {u.number_field([x,594.,w,36.],"Размер, м",n.empty_size as f64,Action::Helper(H::Size),SliderRange::new(0.01,10_f64.max(n.empty_size as f64),0.01));}
            else {u.disabled_button([x,594.,w,36.],"Размер пустышки");}
        }
    }
    fn empty_segments(&self)->Vec<(u64,[f32;2],[f32;2],usize,f32)> {
        if self.editor.mode==EditorMode::Render || !self.editor.project.nodes.iter().any(|n|n.empty) {return vec![];}
        if self.camera_object_view().is_some() {return vec![];}
        let visible=self.editor.project.visible_node_ids(false);
        let nodes:Vec<_>=self.editor.project.nodes.iter().filter(|n|n.empty&&visible.contains(&n.id)&&self.in_isolation(n.id)).take(512).collect();
        if nodes.is_empty() {return vec![];}
        let Ok(eval)=self.evaluated_interactive() else{return vec![];};
        let view=self.view();let vp=self.viewport();let mut out=vec![];
        for node in nodes {
            let Some(model)=eval.global.get(&node.id) else{continue;};let center=model.transform_point3(Vec3::ZERO);
            let mut segments=if let Some(f)=node.fx {
                let (low,high)=if f.gas {self.renderer.as_ref().and_then(|r|r.fx_bounds(node.id)).unwrap_or_else(|| {
                    let radius=if f.source.is_some() {f.source_radius*f.width}else{f.width*if f.cloud||f.burst {0.4}else{0.18}};
                    let pad=(f.width*f.gas_domain*0.5).max(radius*1.25);
                    (Vec3::new(-pad,-f.height*0.12,-pad),Vec3::new(pad,f.height*f.gas_domain,pad))
                })}else{(Vec3::new(-0.5,0.,-0.5),Vec3::new(0.5,1.,0.5))};
                helper_segments(E::Cube,1.,None).into_iter().map(|(a,b,axis)|((a+Vec3::splat(0.5))*(high-low)+low,(b+Vec3::splat(0.5))*(high-low)+low,axis)).collect()}
                else {helper_segments(node.empty_shape,node.empty_size,node.force_field)};
            if self.editor.selected==Some(node.id) {if let Some(f)=node.force_field {
                // Radius follows maximum world scale, exactly as the solver does.
                let scale=model.to_scale_rotation_translation().0.abs().max_element();let inverse=model.inverse();
                for (a,b,axis) in helper_segments(E::Sphere,f.radius*2.*scale,None) {segments.push((inverse.transform_vector3(a),inverse.transform_vector3(b),axis));}
            }}
            for (a,b,axis) in segments {
                if let Some((a,b))=camera_controls::screen_segment(&view,vp,model.transform_point3(a),model.transform_point3(b)) {out.push((node.id,a,b,axis,center.distance(view.eye)));}
            }
        }
        out
    }
    pub(super) fn draw_empty_overlay(&self,u:&mut Ui) {
        for (id,a,b,axis,_) in self.empty_segments() {
            let selected=self.editor.selected==Some(id);let color=if selected {u.palette().accent}else{gizmo::axis_color(u.theme,axis)};
            u.smooth_segment(a,b,if selected {2.}else{1.3},color);
        }
    }
    pub(super) fn empty_pointer_down(&mut self)->bool {
        if !self.editor.project.nodes.iter().any(|n|n.empty) {return false;}
        if self.has_modal() || !self.canvas_input_at(self.mouse) || self.drag.is_some() || self.selected_gizmo().and_then(|g|g.hit(self.mouse)).is_some() {return false;}
        let pointer=Vec2::from_array(self.mouse);
        let picked=self.empty_segments().into_iter().filter(|(id,_,_,_,_)|self.node_selectable(*id)).filter_map(|(id,a,b,_,depth)| {
            let a=Vec2::from_array(a);let delta=Vec2::from_array(b)-a;
            let t=((pointer-a).dot(delta)/delta.length_squared().max(1e-6)).clamp(0.,1.);
            let d=pointer.distance(a+delta*t);(d<=6.).then_some((id,d,depth))
        }).min_by(|a,b|a.1.total_cmp(&b.1).then_with(||a.2.total_cmp(&b.2))).map(|p|p.0);
        let Some(id)=picked else{return false;};self.action(Action::Select(id));self.set_properties_tab(0);self.preferences.right_panel=true;self.redraw();true
    }
}
fn helper_segments(shape:E,size:f32,field:Option<ForceFieldSettings>)->Vec<(Vec3,Vec3,usize)> {
    let mut out=Vec::new();let r=size*0.5;
    let line=|out:&mut Vec<_>,a,b,axis|out.push((a,b,axis));
    let circle=|out:&mut Vec<_>,u:Vec3,v:Vec3,r:f32,axis| {for i in 0..48 {let a=i as f32/48.*std::f32::consts::TAU;let b=(i+1) as f32/48.*std::f32::consts::TAU;out.push(((u*a.cos()+v*a.sin())*r,(u*b.cos()+v*b.sin())*r,axis));}};
    let arrow=|out:&mut Vec<_>,a:Vec3,b:Vec3,axis| {out.push((a,b,axis));let d=(b-a).normalize_or_zero();let side=d.any_orthonormal_vector()*size*0.1;out.push((b,b-d*size*0.2+side,axis));out.push((b,b-d*size*0.2-side,axis));};
    if let Some(f)=field {match f.kind {
        F::Force|F::Magnet=>{circle(&mut out,Vec3::X,Vec3::Y,r,2);for d in [Vec3::X,-Vec3::X,Vec3::Y,-Vec3::Y,Vec3::Z,-Vec3::Z] {let (a,b)=if f.kind==F::Force {(d*r*0.3,d*r*1.2)}else{(d*r*1.2,d*r*0.3)};arrow(&mut out,a,b,2);}}
        F::Wind=>{for offset in [Vec3::ZERO,Vec3::X*r,-Vec3::X*r] {arrow(&mut out,offset-Vec3::Z*r,offset+Vec3::Z*r,2);}}
        F::Vortex=>{for i in 0..64 {let a=i as f32/64.*std::f32::consts::TAU*2.;let b=(i+1) as f32/64.*std::f32::consts::TAU*2.;line(&mut out,Vec3::new(a.cos(),a.sin(),i as f32/64.-0.5)*r,Vec3::new(b.cos(),b.sin(),(i+1) as f32/64.-0.5)*r,2);}arrow(&mut out,Vec3::ZERO,Vec3::Z*size,2);}
    }return out;}
    match shape {
        E::Axes|E::Arrows=>for (axis,d) in [Vec3::X,Vec3::Y,Vec3::Z].into_iter().enumerate() {if shape==E::Arrows {arrow(&mut out,Vec3::ZERO,d*r,axis);}else{line(&mut out,-d*r*0.7,d*r*0.7,axis);}},
        E::Circle=>circle(&mut out,Vec3::X,Vec3::Y,r,2),
        E::Sphere=>{circle(&mut out,Vec3::X,Vec3::Y,r,2);circle(&mut out,Vec3::X,Vec3::Z,r,1);circle(&mut out,Vec3::Y,Vec3::Z,r,0);}
        E::Cube=>for axis in 0..3 {for a in [-r,r] {for b in [-r,r] {let mut p=Vec3::ZERO;p[axis]=-r;p[(axis+1)%3]=a;p[(axis+2)%3]=b;let mut q=p;q[axis]=r;line(&mut out,p,q,axis);}}},
        E::Cone=>{circle(&mut out,Vec3::X,Vec3::Y,r,2);for d in [Vec3::X,Vec3::Y,-Vec3::X,-Vec3::Y] {line(&mut out,d*r,Vec3::Z*size,2);}}
    }out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creation_rows_align_text_under_mesh_and_empty_under_curves() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
            let mut a=App::new(None);a.preferences.theme=theme;a.scale=scale;a.width=1440.;a.height=900.;a.preferences.right_panel=true;
            let mut u=Ui::new(a.scale);u.theme=theme;a.draw_creation_toolbar(&mut u);
            let rect=|action|u.hits.iter().find(|h|h.action==action).unwrap().rect;
            let mesh=rect(Action::DropdownOpen(ui::DropdownKind::MeshCreate));let curve=rect(Action::DropdownOpen(ui::DropdownKind::CurveCreate));
            let text=rect(Action::Text(ui::TextAction::Create));let empty=rect(Action::DropdownOpen(ui::DropdownKind::EmptyCreate));
            assert!((mesh[0]-text[0]).abs()<0.01);assert!((curve[0]-empty[0]).abs()<0.01);assert!(text[1]>=mesh[1]+mesh[3]);assert!(a.properties_content_rect()[1]>=a.creation_toolbar_rect()[1]+a.creation_toolbar_rect()[3]);
        }}
    }
    #[test]
    fn empty_is_selectable_and_hidden_from_camera_and_render_and_undoable() {
        let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.create_empty_object();
        let id=a.editor.selected.unwrap();assert!(a.editor.project.node(id).unwrap().empty);assert!(!a.rig_context());assert!(a.editor.project.meshes.is_empty());
        assert!(!a.empty_segments().is_empty());a.editor.mode=EditorMode::Render;assert!(a.empty_segments().is_empty());
        a.editor.undo();assert!(a.editor.project.node(id).is_none());
    }
}
