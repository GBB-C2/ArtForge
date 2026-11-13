//! One layout describes painting, collapse ranges and the following path panel.
use super::*;
use crate::ui::{DropdownKind as D,FxAction as F};
use poseforge_core::{FxSettings,FxKind,FxEmission,CollisionKind};

pub(super) fn limits(index:u8)->Option<(f32,f32)> {
    [(0.,12.),(293.,3000.),(0.,4.),(300.,1800.),(0.05,20.),(0.,2500.),(0.,3.),
        (0.,5.),(0.,20.),(0.,12.),(0.02,5.),(0.,8.),(0.,8.),(-0.8,0.8)].get(index.checked_sub(64)? as usize).copied()
}
enum Row {
    Number(u8,&'static str,f64,f64,f64,bool),Check(&'static str,F,bool),Choice(D,String),
    Source,Transport,Color(bool),Defaults,Cache,Status,LightStatus,Envelope,Collisions,
}
struct Group {title:&'static str,start:f32,end:f32,rows:Vec<(f32,Row)>}
fn parameter(index:u8,label:&'static str,step:f64)->Row {
    let (lo,hi)=limits(index).expect("known Pyro field");Row::Number(index,label,lo as f64,hi as f64,step,true)
}
fn number(index:u8,label:&'static str,lo:f64,hi:f64,step:f64)->Row {Row::Number(index,label,lo,hi,step,true)}
fn layout(f:FxSettings,collision_shift:f32)->Vec<Group> {
    let Some(p)=f.pyro else{return vec![];};let fire=f.kind==FxKind::Fire;let fog=f.kind==FxKind::Fog;
    let mut sections=vec![("FX",vec![Row::Choice(D::FxKind,f.kind.label().into()),
        Row::Choice(D::FxPreset(f.kind.index() as u8),"Пресеты…".into()),Row::Transport,Row::Check("Включено",F::Enabled,f.enabled),Row::Defaults])];
    let mut source=vec![Row::Choice(D::FxEmission,p.mode.label().into()),Row::Source,
        parameter(64,if p.mode==FxEmission::Continuous {"Подача в секунду"}else{"Количество"},0.05),
        number(13,"Радиус источника",0.02,2.,0.01),number(6,"Ширина области",0.1,1.,0.01),number(7,"Высота области",0.1,1.,0.01)];
    if !(fog && f.source.is_none() && p.mode!=FxEmission::Burst) {source.push(Row::Check("Объёмный источник",F::Cloud,f.cloud));}
    if fog && f.source.is_none() && p.mode!=FxEmission::Burst {source.push(parameter(76,"Убывание по высоте",0.05));}
    source.push(parameter(65,"Температура, K",10.));sections.push(("Источник",source));
    if fire {sections.push(("Горение",vec![parameter(66,"Топливо",0.05),parameter(67,"Зажигание, K",10.),parameter(68,"Скорость горения",0.05),
        parameter(69,"Выделение тепла, K",10.),parameter(70,"Образование дыма",0.01),parameter(72,"Расширение газа",0.1)]));}
    if p.mode==FxEmission::Burst {let mut rows=vec![parameter(74,"Подача импульса, с",0.01)];if !fire {rows.push(parameter(72,"Расширение газа",0.1));}sections.push(("Импульс взрыва",rows));}
    sections.push(("Движение",vec![parameter(73,"Начальный толчок",0.05),parameter(75,"Завихрение потока",0.05),
        number(19,"Подъём горячего газа",0.,4.,0.05),parameter(71,"Остывание",0.05),number(18,"Рассеивание дыма",0.,3.,0.01),
        number(3,"Неоднородность подачи",0.,1.,0.01),number(2,"Частота неоднородности",1.,16.,0.1),
        number(10,"Подача по X",-0.5,0.5,0.01),number(11,"Подача по Z",-0.5,0.5,0.01),number(5,"Случайность",0.,999999.,1.)]));
    let mut optics=vec![number(0,"Плотность среды",0.,30.,0.05),Row::Color(false)];
    if fire {optics.extend([Row::Check("Цвет по температуре",F::TemperatureColor,p.temperature_color),Row::Color(true),
        number(1,"Яркость пламени",0.,30.,0.1),number(12,"Мерцание",0.,1.,0.01),Row::Check("Освещать сцену",F::Illuminate,f.illuminate),Row::LightStatus]);}
    optics.extend([Row::Check("Тени внутри объёма",F::SelfShadow,p.self_shadow),parameter(77,"Направленность света",0.01),number(8,"Мягкость края",0.02,1.,0.01)]);
    sections.push(("Освещение",optics));
    sections.push(("Время FX",vec![number(15,"Длительность, с",0.1,30.,0.1),number(16,"Начало, с",0.,86400.,0.1),
        number(4,"Скорость времени",0.,5.,0.01),Row::Check("Повторять",F::Loop,f.looped)]));
    sections.push(("Расчёт и качество",vec![Row::Choice(D::FxQuality,format!("Качество · {}",f.quality.label())),
        number(17,"Запас области потока",1.,4.,0.1),Row::Cache,Row::Status]));
    sections.push(("Изменение по времени",vec![Row::Envelope]));
    sections.push(("Силовые поля",vec![Row::Check("Учитывать поля",F::Fields,f.use_fields),Row::Number(14,"Влияние полей",0.,2.,0.01,f.use_fields)]));
    sections.push(("Столкновения",vec![Row::Check("Столкновения с мешами",F::MeshCollision,f.mesh_collision),
        Row::Check("Полые препятствия",F::Hollow,f.gas_hollow),Row::Collisions]));
    let mut y=526.;let mut result=Vec::new();
    for (title,rows) in sections {let start=y;y+=4.;let mut placed=Vec::new();for row in rows {
        let height=match &row {Row::Number(..)=>44.,Row::Check(..)=>36.,Row::Status=>48.,Row::LightStatus=>24.,
            Row::Envelope=>if f.envelope.enabled {512.}else{76.},Row::Collisions=>36.+collision_shift,_=>40.};
        placed.push((y,row));y+=height;
    }y+=8.;result.push(Group {title,start,end:y,rows:placed});}result
}
impl App {
    pub(super) fn pyro_property_groups(&self,f:FxSettings)->Vec<(f32,&'static str,f32)> {
        let mut groups=vec![(171.,"Объект",0.)];groups.extend(layout(f,self.collision_layout_shift(CollisionKind::Fx)).into_iter().map(|g|(g.start,g.title,0.)));groups
    }
    pub(super) fn pyro_panel_bottom(&self,f:FxSettings)->f32 {layout(f,self.collision_layout_shift(CollisionKind::Fx)).last().map_or(526.,|g|g.end)}
    pub(super) fn draw_pyro_properties(&self,u:&mut Ui,rx:f32,rw:f32,id:u64,f:FxSettings) {
        let x=rx+16.;let w=rw-32.;let editable=self.helper_editable(id);let half=(w-8.)*0.5;
        for group in layout(f,self.collision_layout_shift(CollisionKind::Fx)) {for (y,row) in group.rows {match row {
            Row::Number(i,label,min,max,step,enabled)=>{let value=fx_controls::number(f,i).unwrap_or_default();
                if editable && enabled {u.number_field([x,y,w,36.],label,value,Action::Fx(F::Field(i)),SliderRange::new(min,max,step));}
                else {u.disabled_button([x,y,w,36.],&format!("{label}: {}",ui::number_caption(value)));}}
            Row::Check(label,action,checked)=>{if editable {u.checkbox([x,y,w,28.],label,Action::Fx(action),checked);}else{u.disabled_checkbox([x,y,w,28.],label,checked);}}
            Row::Choice(kind,label)=>{if editable {self.draw_dropdown_button(u,[x,y,w,30.],kind,&label);}else{u.disabled_button([x,y,w,30.],&label);}}
            Row::Transport=>{let running=self.fx_preview.running && !f.playback_finished(self.fx_view_time());
                if editable {u.icon_button([x,y,half,30.],if running{"Пауза FX"}else{"Запустить FX"},Action::Fx(F::Play),running,if running{Icon::Pause}else{Icon::Play});
                    u.icon_button([x+half+8.,y,half,30.],"В начало",Action::Fx(F::Reset),false,Icon::Start);}
                else {u.disabled_icon_button([x,y,half,30.],"Запустить FX",Icon::Play);u.disabled_icon_button([x+half+8.,y,half,30.],"В начало",Icon::Start);}}
            Row::Source=>{let source=f.source.and_then(|id|self.editor.project.node(id));let label=source.map_or("Меш или кривая…",|n|n.name.as_str());
                if editable {u.button([x,y,w-76.,32.],"",Action::Collision(ui::CollisionAction::SourceOpen),self.collision_popup_open());
                    u.quiet_icon_button([x+w-70.,y,32.,32.],"",Action::Collision(ui::CollisionAction::SourcePick),false,Icon::Eyedropper);}
                else {u.disabled_button([x,y,w-76.,32.],"");u.disabled_icon_button([x+w-70.,y,32.,32.],"",Icon::Eyedropper);}
                u.fitted_label([x+8.,y+8.,w-92.,18.],label,11.,u.palette().text);
                if editable && source.is_some() {u.quiet_icon_button([x+w-34.,y,32.,32.],"",Action::Fx(F::Source(None)),false,Icon::Close);}
                else {u.disabled_icon_button([x+w-34.,y,32.,32.],"",Icon::Close);}}
            Row::Color(smoke)=>{let color=if smoke {f.pyro.unwrap().smoke_color}else{f.color};let action=if smoke{F::SmokeColor}else{F::Color};
                let label=format!("{} · {}",if smoke{"Цвет дыма"}else if f.kind==FxKind::Fire{"Оттенок пламени"}else{"Цвет среды"},surfaces::color_hex(color));
                if editable {u.button([x,y,w,30.],&label,Action::Fx(action),false);}else{u.disabled_button([x,y,w,30.],&label);}}
            Row::Defaults=>{let default=FxSettings{enabled:f.enabled,source:f.source,..fx_controls::defaults(f)};
                if editable && f!=default {u.quiet_icon_button([x,y,w,30.],"Сбросить параметры FX",Action::Fx(F::Defaults),false,Icon::Reset);}
                else {u.disabled_icon_button([x,y,w,30.],"Сбросить параметры FX",Icon::Reset);}}
            Row::Cache=>{if editable {u.icon_button([x,y,w,30.],"Очистить кэш",Action::Fx(F::ClearCache),false,Icon::Reset);}else{u.disabled_icon_button([x,y,w,30.],"Очистить кэш",Icon::Reset);}}
            Row::Status=>{let memory=self.renderer.as_ref().map_or(0,|r|r.fx_cache_bytes());let status=self.renderer.as_ref().and_then(|r|r.fx_status());
                u.fitted_label([x,y,w,20.],&format!("Кэш: {:.1} / 96 МБ · 30 кадров/с",memory as f64/1048576.),10.,u.palette().muted);
                u.fitted_label([x,y+22.,w,20.],status.unwrap_or("Размер области — масштабом объекта"),10.,if status.is_some(){u.palette().warn}else{u.palette().muted});}
            Row::LightStatus=>{let full=f.illuminate && f.enabled && !self.fx_light_has_slot(id);
                u.fitted_label([x,y,w,20.],if full{"Лимит света занят: 8 из 8"}else{"Свет по положению горячего пламени"},10.,if full{u.palette().warn}else{u.palette().muted});}
            Row::Envelope=>self.draw_fx_envelope(u,x,w,y,f,editable),
            Row::Collisions=>self.draw_collision_list(u,CollisionKind::Fx,x,y,w),
        }}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app()->App {let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.preferences=Preferences::default();a.preferences.right_panel=true;a.width=1440.;a.height=900.;a}
    #[test]
    fn new_creation_mode_controls_and_parameter_edits_are_atomic() {
        let mut a=app();a.fx_action(F::Create(0));let before=a.editor.project.clone();assert!(a.selected_fx().unwrap().1.pyro.is_some());let undo=a.editor.undo_len();
        a.fx_action(F::Emission(1));let f=a.selected_fx().unwrap().1;assert!(f.burst);assert_eq!(f.source_mode(),FxEmission::Burst);f.validate().unwrap();assert_eq!(a.editor.undo_len(),undo+1);
        a.editor.undo();assert_eq!(a.editor.project,before);
        for (i,text) in [(64,"NaN"),(65,"292"),(67,"2000"),(68,"0"),(74,"0"),(77,"1")] {assert!(a.fx_confirm(&Action::Fx(F::Field(i)),text).is_err());}
        assert_eq!(a.editor.project,before);a.fx_confirm(&Action::Fx(F::Field(66)),"2,25").unwrap();assert_eq!(a.selected_fx().unwrap().1.pyro.unwrap().fuel,2.25);
        a.editor.undo();assert_eq!(a.editor.project,before);a.fx_confirm(&Action::Fx(F::SmokeColor),"#5582FF").unwrap();a.editor.undo();assert_eq!(a.editor.project,before);
        a.fx_action(F::Gas);assert_eq!(a.editor.project,before);assert_eq!(a.number_default(&Action::Fx(F::Field(66))),Some(1.5));
        a.fx_action(F::Kind(2));assert_eq!(a.selected_fx().unwrap().1.source_mode(),FxEmission::Fill);a.selected_fx().unwrap().1.validate().unwrap();
    }
    #[test]
    fn creating_an_effect_later_starts_at_the_current_clock_without_a_second_undo() {
        let mut a=app();a.editor.time=12.5;let undo=a.editor.undo_len();a.fx_action(F::Create(1));
        let f=a.selected_fx().unwrap().1;assert_eq!(f.start,12.5);assert_eq!(f.local_time(a.fx_view_time()),0.);assert!(a.fx_preview_active());assert_eq!(a.editor.undo_len(),undo+1);
        a.editor.time=14.;a.editor.playing=true;a.fx_action(F::Create(0));assert_eq!(a.selected_fx().unwrap().1.start,14.);assert_eq!(a.fx_view_time(),14.);
    }
    #[test]
    fn legacy_upgrade_preserves_object_and_can_be_undone() {
        let mut a=app();let id=a.editor.create_fx(FxKind::Smoke).unwrap();a.action(Action::Select(id));
        let n=a.editor.project.node_mut(id).unwrap();n.transform.translation=Vec3::new(1.,2.,3.);n.fx.as_mut().unwrap().start=2.5;
        let before=a.editor.project.clone();let undo=a.editor.undo_len();a.fx_action(F::Upgrade);
        assert_eq!(a.editor.undo_len(),undo+1);assert!(a.selected_fx().unwrap().1.pyro.is_some());assert_eq!(a.editor.project.node(id).unwrap().transform,before.node(id).unwrap().transform);
        assert_eq!(a.fx_view_time(),2.5);a.editor.undo();assert_eq!(a.editor.project,before);
    }
    #[test]
    fn panels_follow_kind_and_mode_without_overlap_and_collapse_at_both_scales() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {for kind in 0..4 {
            let mut a=app();a.preferences.theme=theme;a.scale=scale;a.fx_action(F::Create(kind));a.fx_action(F::Emission(1));a.fx_action(F::Evolution);
            let f=a.selected_fx().unwrap().1;let groups=layout(f,a.collision_layout_shift(CollisionKind::Fx));
            assert!(groups.windows(2).all(|p|p[0].end==p[1].start));
            assert_eq!(groups.iter().any(|g|g.title=="Горение"),kind==0);assert!(groups.iter().any(|g|g.title=="Импульс взрыва"));
            let mut u=Ui::new(scale);u.theme=theme;a.draw_fx_properties(&mut u,100.,260.);
            assert!(u.hits.iter().all(|h|h.rect[0]>=116. && h.rect[0]+h.rect[2]<=344.001 && h.rect[1]+h.rect[3]<=a.fx_panel_bottom()), "out of bounds: {:?}",u.hits.iter().filter(|h|h.rect[0]<116. || h.rect[0]+h.rect[2]>344.001 || h.rect[1]+h.rect[3]>a.fx_panel_bottom()).map(|h|(&h.action,h.rect)).collect::<Vec<_>>());
            for (i,a) in u.hits.iter().enumerate() {for b in &u.hits[i+1..] {
                if matches!(a.action,Action::Slide(..)) || matches!(b.action,Action::Slide(..)) {continue;}
                let [x,y,w,h]=a.rect;let [bx,by,bw,bh]=b.rect;assert!(x+w<=bx || bx+bw<=x || y+h<=by || by+bh<=y,"overlapping FX controls: {:?} and {:?}",a.action,b.action);
            }}
            a.preferences.collapsed_sections.extend([2412,2413,2411]);let mut u=Ui::new(scale);a.draw_fx_properties(&mut u,100.,260.);a.draw_property_sections(&mut u,(0,0,0));
            assert!(!u.hits.iter().any(|h|matches!(h.action,Action::Fx(F::Field(66..=70)|F::Field(74)|F::Evolution))));
            assert!(u.hits.iter().any(|h|h.action==Action::Fx(F::MeshCollision)));assert_eq!(a.path_panel_start(),Some(a.fx_panel_bottom()));
        }}}
    }
    #[test]
    fn mode_dropdown_rejects_stale_selection_and_creates_one_undo() {
        let mut a=app();a.fx_action(F::Create(0));let first=a.selected_fx().unwrap().0;
        let open=|a:&mut App|{let mut u=Ui::new(1.);a.draw_fx_properties(&mut u,100.,260.);a.ui=u;a.open_dropdown(D::FxEmission);assert!(a.dropdown.is_some());};
        open(&mut a);let before=a.editor.project.clone();let undo=a.editor.undo_len();a.choose_dropdown(1);assert_eq!(a.editor.undo_len(),undo+1);a.editor.undo();assert_eq!(a.editor.project,before);
        let second=a.editor.create_pyro(FxKind::Smoke).unwrap();a.editor.selected=Some(first);open(&mut a);a.editor.selected=Some(second);let before=a.editor.project.clone();a.choose_dropdown(2);assert_eq!(a.editor.project,before);
    }
}
