//! Procedural volumes share ordinary object selection, transforms and undo.
use super::*;
use crate::ui::{DropdownKind, FxAction as F};
use poseforge_core::{FxKind, FxSettings};

const COLLISION_START:f32=1900.;
pub(super) fn defaults(f:FxSettings)->FxSettings {if f.pyro.is_some(){FxSettings::modern(f.kind)}else{FxSettings::preset(f.kind)}}

#[derive(Default,Clone,Copy)]
pub(super) struct Preview {pub running:bool, time:Option<f64>,pub point:u8,pub channel:u8}
pub(super) fn fx_icon(kind:FxKind)->Icon {match kind {FxKind::Fire=>Icon::Fire,FxKind::Smoke|FxKind::Steam=>Icon::Smoke,FxKind::Fog=>Icon::Fog}}
pub(super) fn number(settings:FxSettings,index:u8)->Option<f64> {
    if (64..78).contains(&index) {return settings.pyro.map(|p|[p.source_rate,p.source_temperature,p.fuel,p.ignition,p.burn_rate,p.heat_release,p.smoke_yield,p.cooling,p.expansion,p.impulse,p.burst_seconds,p.vorticity,p.fog_falloff,p.anisotropy][index as usize-64] as f64);}
    if (32..48).contains(&index) {let k=settings.envelope.keys[(index as usize-32)/4];return Some([k.at,k.size,k.density,k.emission][(index as usize-32)%4] as f64*100.);}
    [settings.density as f64,settings.emission as f64,settings.detail as f64,
        settings.turbulence as f64,settings.speed as f64,settings.seed as f64,
        settings.width as f64,settings.height as f64,settings.softness as f64,settings.smoke as f64,
        settings.drift[0] as f64,settings.drift[1] as f64,settings.flicker as f64,
        settings.source_radius as f64,settings.field_influence as f64,settings.duration as f64,settings.start,
        settings.gas_domain as f64,settings.dissipation as f64,settings.buoyancy as f64].get(index as usize).copied()
}
impl App {
    pub(super) fn selected_fx(&self)->Option<(u64,FxSettings)> {self.selected_helper().and_then(|n|n.fx.map(|f|(n.id,f)))}
    pub(super) fn fx_view_time(&self)->f64 {if self.editor.playing {self.editor.time}else{self.fx_preview.time.unwrap_or(self.editor.time)}}
    fn completed_fx_start(&self,time:f64)->Option<f64> {
        let visible=self.editor.project.visible_node_ids(false);let mut start=f64::INFINITY;
        for n in &self.editor.project.nodes {
            let Some(f)=n.fx.filter(|f|f.enabled && f.speed>0. && f.density>0.) else{continue;};
            if !visible.contains(&n.id) || !self.in_isolation(n.id) {continue;}
            if !f.playback_finished(time) {return None;}
            start=start.min(f.start);
        }
        start.is_finite().then_some(start)
    }
    pub(super) fn start_fx_preview(&mut self) {
        let time=self.fx_view_time();self.fx_preview.time=Some(self.completed_fx_start(time).unwrap_or(time));self.fx_preview.running=true;
    }
    pub(super) fn pause_fx_preview(&mut self) {self.fx_preview.running=false;}
    pub(super) fn reset_fx_preview(&mut self) {self.fx_preview=Preview{running:false,time:Some(0.),..self.fx_preview};}
    pub(super) fn fx_timing(&self)->bool {self.selected_fx().is_some_and(|(_,f)|f.burst||f.gas||f.envelope.enabled)}
    pub(super) fn fx_advanced_heights(&self)->(f32,f32) {self.selected_fx().map_or((112.,76.),|(_,f)|(if f.gas {400.}else{156.},if f.envelope.enabled {512.}else{76.}))}
    pub(super) fn fx_property_y(&self,y:f32)->f32 {
        let (gas,evolution)=self.fx_advanced_heights();
        y+if y>=590. {44.}else{0.}+if y>=1016. {48.}else{0.}+if y>=1356. && self.fx_timing() {180.}else{0.}+if y>=1832. {gas+evolution}else{0.}
    }
    pub(super) fn fx_property_groups(&self)->Vec<(f32,&'static str,f32)> {
        if let Some((_,f))=self.selected_fx().filter(|(_,f)|f.pyro.is_some()) {return self.pyro_property_groups(f);}
        let mut groups=vec![(171.,"Объект",0.),(526.,"FX",0.),(808.,"Форма",0.),
            (1016.,"Движение",0.),(1356.,"Освещение",0.),(1526.,"Источник",0.),
            (1684.,"Силовые поля",0.),(1832.,"Столкновения",0.)];
        for group in &mut groups {group.0=self.fx_property_y(group.0);}
        if self.fx_timing() {groups.insert(4,(1448.,"Время FX",0.));}
        let (gas,evolution)=self.fx_advanced_heights();let start=self.fx_property_y(1832.)-gas-evolution;
        let index=groups.len()-1;groups.insert(index,(start,"Расчёт и качество",0.));groups.insert(index+1,(start+gas,"Изменение по времени",0.));
        groups
    }
    pub(super) fn fx_panel_bottom(&self)->f32 {if let Some((_,f))=self.selected_fx().filter(|(_,f)|f.pyro.is_some()){return self.pyro_panel_bottom(f);}self.fx_property_y(COLLISION_START)+36.+self.collision_layout_shift(poseforge_core::CollisionKind::Fx)}
    pub(super) fn fx_preview_active(&self)->bool {
        if !self.fx_preview.running || self.editor.playing || self.job.is_some() || self.has_modal()
            || self.renderer.as_ref().is_some_and(|r|r.fx_blocked()) {return false;}
        let visible=self.editor.project.visible_node_ids(false);
        self.editor.project.nodes.iter().any(|n|visible.contains(&n.id) && self.in_isolation(n.id)
            && n.fx.is_some_and(|f|f.enabled && f.speed>0. && f.density>0. && !f.playback_finished(self.fx_view_time())))
    }
    pub(super) fn tick_fx(&mut self,dt:f64) {
        if self.editor.playing {self.fx_preview=Preview{running:false,time:None,..self.fx_preview};return;}
        if self.fx_preview_active() {self.fx_preview.time=Some(self.fx_view_time()+dt.clamp(0.,0.1));}
        if self.fx_preview.running && self.completed_fx_start(self.fx_view_time()).is_some() {self.fx_preview.running=false;}
    }
    pub(super) fn fx_action(&mut self,action:F) {
        if self.job.is_some() || self.native_picker.is_some() || self.scene_write_pending() || self.editor.has_staged() {return;}
        if let F::Create(index)|F::CreatePreset(index,_)=action {
            let Some(&kind)=FxKind::ALL.get(index as usize) else{return;};
            let start=self.fx_view_time().clamp(0.,86400.);
            let result=if let F::CreatePreset(_,preset)=action {self.editor.create_fx_preset_at(kind,preset,start)}else{self.editor.create_pyro_at(kind,start)};
            if let Some(id)=self.result(result) {
                self.editor.playing=false;self.editor.mode=EditorMode::Scene;
                self.authoring=Default::default();self.curves=Default::default();self.primitive_panel=None;
                self.action(Action::Select(id));self.set_properties_tab(0);self.preferences.right_panel=true;
                self.preferences.viewport_shading=self.preferences.viewport_shading.max(2);self.tool=Tool::Move;
                self.fx_preview.time=Some(start);self.fx_preview.running=true;
                let name=&self.editor.project.node(id).unwrap().name;
                self.status=format!("{name} · размер задаётся масштабом объекта · запуск без ключей");self.redraw();
            }return;
        }
        if self.selected_fx().is_some() {match action {
            F::Point(i) if i<4=>{self.fx_preview.point=i;self.redraw();return;}
            F::Channel(i) if i<3=>{self.fx_preview.channel=i;self.redraw();return;}
            _=>{}
        }}
        let Some((id,settings))=self.selected_fx().filter(|(id,_)|self.helper_editable(*id)) else{return;};
        match action {
            F::ClearCache=>{if let Some(r)=self.renderer.as_mut(){r.clear_fx_cache();}self.status="Кэш газа очищен".into();self.redraw();return;}
            F::Source(source)=>{
                let result=self.editor.set_fx_source(id,source);
                if self.result(result).is_some() {self.invalidate();self.status=if source.is_some(){"Источник FX назначен · поток следует за мешем или кривой"}else{"Источник FX отвязан · положение сохранено"}.into();}
                self.redraw();return;
            }
            F::Play=>{
                if settings.playback_finished(self.fx_view_time()) {self.fx_preview.time=Some(settings.start);self.fx_preview.running=true;}
                else {self.fx_preview.time=Some(self.fx_view_time());self.fx_preview.running=!self.fx_preview.running;}
                self.redraw();return;
            }
            F::Reset=>{self.fx_preview=Preview {running:false,time:Some(if self.fx_timing() {settings.start}else{0.}),..self.fx_preview};self.redraw();return;}
            F::Field(index)=>{if let Some(value)=number(settings,index) {self.input(Action::Fx(action),ui::number_caption(value));}return;}
            F::SmokeColor=>{if let Some(p)=settings.pyro {self.input(Action::Fx(action),surfaces::color_hex(p.smoke_color));}return;}
            F::Color=>{self.input(Action::Fx(action),surfaces::color_hex(settings.color));return;}
            F::KeyColor(i)=>{if let Some(k)=settings.envelope.keys.get(i as usize){self.input(Action::Fx(action),surfaces::color_hex(k.color));}return;}
            _=>{}
        }
        let mut next=settings;
        match action {
            F::Kind(index)=>{let Some(&kind)=FxKind::ALL.get(index as usize) else{return;};next=if settings.pyro.is_some(){FxSettings::modern(kind)}else{FxSettings::preset(kind)};next.enabled=settings.enabled;next.seed=settings.seed;next.source=settings.source;
                next.cloud=settings.cloud;next.burst=settings.burst;if let Some(p)=&mut next.pyro {if settings.burst {p.mode=poseforge_core::FxEmission::Burst;p.expansion=8.;p.impulse=4.;}}next.start=settings.start;next.duration=settings.duration;next.looped=settings.looped;
                next.gas=settings.gas;next.gas_hollow=settings.gas_hollow;next.quality=settings.quality;next.envelope=settings.envelope;
                next.use_fields=settings.use_fields;next.field_influence=settings.field_influence;next.mesh_collision=settings.mesh_collision;next.source_radius=settings.source_radius;}
            F::Preset(i)=>{let Some(p)=settings.styled(i) else{return;};next=p;}
            F::Quality(i)=>{let Some(&q)=poseforge_core::FxQuality::ALL.get(i as usize) else{return;};next.quality=q;}
            F::Upgrade=>next=settings.upgrade_pyro(),
            F::Emission(i)=>{let Some(&mode)=poseforge_core::FxEmission::ALL.get(i as usize) else{return;};let Some(p)=&mut next.pyro else{return;};
                if p.mode==mode {return;}p.mode=mode;next.burst=mode==poseforge_core::FxEmission::Burst;
                if next.burst {p.expansion=p.expansion.max(6.);p.impulse=p.impulse.max(4.);}}
            F::SelfShadow=>{let Some(p)=&mut next.pyro else{return;};p.self_shadow=!p.self_shadow;}
            F::TemperatureColor=>{let Some(p)=&mut next.pyro else{return;};p.temperature_color=!p.temperature_color;}
            F::Gas if next.pyro.is_none()=>next.gas=!next.gas,F::Hollow=>next.gas_hollow=!next.gas_hollow,
            F::Evolution=>next.envelope.enabled=!next.envelope.enabled,F::Smooth=>next.envelope.smooth=!next.envelope.smooth,
            F::Enabled=>next.enabled=!next.enabled,
            F::Fields=>next.use_fields=!next.use_fields,
            F::Cloud=>next.cloud=!next.cloud,F::Burst=>{next.burst=!next.burst;if let Some(p)=&mut next.pyro {p.mode=if next.burst{poseforge_core::FxEmission::Burst}else{poseforge_core::FxEmission::Continuous};}},F::Loop=>next.looped=!next.looped,
            F::MeshCollision=>next.mesh_collision=!next.mesh_collision,
            F::Illuminate if settings.kind==FxKind::Fire=>next.illuminate=!next.illuminate,
            F::Defaults=>{next=defaults(settings);next.enabled=settings.enabled;next.source=settings.source;},
            _=>return,
        }
        if next==settings {return;}
        let result=self.editor.transact("Параметры FX",|p|p.node_mut(id).unwrap().fx=Some(next));
        if self.result(result).is_some() {
            if matches!(action,F::Preset(_)|F::Upgrade|F::Emission(_)) || matches!(action,F::Burst) && next.burst || matches!(action,F::Gas) && next.gas {self.fx_preview=Preview {running:true,time:Some(next.start),..self.fx_preview};}
            self.redraw();
        }
    }
    pub(super) fn fx_confirm(&mut self,action:&Action,text:&str)->Result<bool,String> {
        let Action::Fx(action)=*action else{return Ok(false);};
        if !matches!(action,F::Field(_)|F::Color|F::KeyColor(_)|F::SmokeColor) {return Ok(false);}
        let (id,settings)=self.selected_fx().filter(|(id,_)|self.helper_editable(*id)).ok_or("Выберите доступный FX")?;
        let mut next=settings;
        match action {
            F::SmokeColor=>{let p=next.pyro.as_mut().ok_or("Сначала переведите эффект на новый FX")?;p.smoke_color=surfaces::parse_color(text)?;}
            F::Color=>next.color=surfaces::parse_color(text)?,
            F::KeyColor(i)=>{let k=next.envelope.keys.get_mut(i as usize).ok_or("Выберите точку времени")?;k.color=surfaces::parse_color(text)?;}
            F::Field(index)=>match index {
                0=>next.density=surfaces::number(text,0.,30.)?,1=>next.emission=surfaces::number(text,0.,30.)?,
                2=>next.detail=surfaces::number(text,1.,16.)?,3=>next.turbulence=surfaces::number(text,0.,1.)?,
                4=>next.speed=surfaces::number(text,0.,5.)?,
                5=>{let value=surfaces::number(text,0.,999999.)?;if value.fract()!=0. {return Err("Случайность: целое число от 0 до 999999".into());}next.seed=value as u32;}
                6=>next.width=surfaces::number(text,0.1,1.)?,7=>next.height=surfaces::number(text,0.1,1.)?,
                8=>next.softness=surfaces::number(text,0.02,1.)?,9=>next.smoke=surfaces::number(text,0.,1.)?,
                10=>next.drift[0]=surfaces::number(text,-0.5,0.5)?,11=>next.drift[1]=surfaces::number(text,-0.5,0.5)?,
                12=>next.flicker=surfaces::number(text,0.,1.)?,
                13=>next.source_radius=surfaces::number(text,0.02,2.)?,14=>next.field_influence=surfaces::number(text,0.,2.)?,
                15=>next.duration=surfaces::number(text,0.1,30.)?,
                16=>{next.start=text.trim().replace(',',".").parse::<f64>().map_err(|_|"Введите время начала в секундах")?;},
                17=>next.gas_domain=surfaces::number(text,1.,4.)?,18=>next.dissipation=surfaces::number(text,0.,3.)?,19=>next.buoyancy=surfaces::number(text,0.,4.)?,
                32..=47=>{let i=(index as usize-32)/4;let property=(index as usize-32)%4;
                    let value=surfaces::number(text,0.,if property<2 {100.}else{200.})?/100.;
                    if property==0 && (i==0 && value!=0. || i==3 && value!=1.) {return Err("Начало и конец закреплены на 0% и 100%".into());}
                    let k=&mut next.envelope.keys[i];match property {0=>k.at=value,1=>k.size=value,2=>k.density=value,_=>k.emission=value};},
                64..=77=>{let p=next.pyro.as_mut().ok_or("Выберите новый FX")?;
                    let (min,max)=pyro_controls::limits(index).ok_or("Неизвестный параметр FX")?;let v=surfaces::number(text,min,max)?;
                    match index {64=>p.source_rate=v,65=>p.source_temperature=v,66=>p.fuel=v,67=>p.ignition=v,68=>p.burn_rate=v,69=>p.heat_release=v,
                        70=>p.smoke_yield=v,71=>p.cooling=v,72=>p.expansion=v,73=>p.impulse=v,74=>p.burst_seconds=v,75=>p.vorticity=v,76=>p.fog_falloff=v,_=>p.anisotropy=v};},
                _=>return Ok(false),
            },_=>return Ok(false),
        }
        next.validate().map_err(|e|e.to_string())?;
        if next!=settings {self.editor.transact("Параметры FX",|p|p.node_mut(id).unwrap().fx=Some(next)).map_err(|e|e.to_string())?;self.update_dirty();}
        Ok(true)
    }
    pub(super) fn fx_light_has_slot(&self,id:u64)->bool {
        let mut count=self.editor.project.lights.iter().enumerate().filter(|(i,_)|self.editor.project.light_visible(*i,false)).count();
        let visible=self.editor.project.visible_node_ids(false);
        if !visible.contains(&id) {return true;}
        for node in &self.editor.project.nodes {
            if !visible.contains(&node.id) {continue;}
            if node.id==id {return count<poseforge_core::MAX_ENABLED_LIGHTS;}
            if node.fx.is_some_and(|f|f.enabled && f.visible_at(self.fx_view_time()) && f.illuminate && f.kind==FxKind::Fire && f.density>0. && f.emission>0.) {count+=1;}
        }
        false
    }
    pub(super) fn draw_fx_properties(&self,u:&mut Ui,rx:f32,rw:f32) {
        let Some((id,settings))=self.selected_fx() else{return;};
        if settings.pyro.is_some() {self.draw_pyro_properties(u,rx,rw,id,settings);return;}
        let x=rx+16.;let w=rw-32.;let editable=self.helper_editable(id);let half=(w-8.)*0.5;
        let fire=settings.kind==FxKind::Fire;
        u.label(x,528.,"Тип эффекта",11.,u.palette().muted);
        if editable {self.draw_dropdown_button(u,[x,550.,w,30.],DropdownKind::FxKind,settings.kind.label());}
        else {u.disabled_button([x,550.,w,30.],settings.kind.label());}
        if editable {self.draw_dropdown_button(u,[x,590.,w,30.],DropdownKind::FxPreset(settings.kind.index() as u8),"Пресеты…");}
        else {u.disabled_button([x,590.,w,30.],"Пресеты…");}
        let running=self.fx_preview.running && !settings.playback_finished(self.fx_view_time());
        if editable {
            u.icon_button([x,self.fx_property_y(590.),half,30.],if running {"Пауза FX"}else{"Запустить FX"},Action::Fx(F::Play),running,if running {Icon::Pause}else{Icon::Play});
            u.icon_button([x+half+8.,self.fx_property_y(590.),half,30.],"В начало",Action::Fx(F::Reset),false,Icon::Start);
            u.checkbox([x,self.fx_property_y(632.),w,28.],"Включено",Action::Fx(F::Enabled),settings.enabled);
        } else {
            u.disabled_icon_button([x,self.fx_property_y(590.),half,30.],"Запустить FX",Icon::Play);
            u.disabled_icon_button([x+half+8.,self.fx_property_y(590.),half,30.],"В начало",Icon::Start);
            u.disabled_checkbox([x,self.fx_property_y(632.),w,28.],"Включено",settings.enabled);
        }
        for (index,label,y,min,max,step) in [
            (0,"Плотность",self.fx_property_y(674.),0.,30.,0.05),(1,"Свечение",self.fx_property_y(718.),0.,30.,0.05),
            (6,"Ширина",self.fx_property_y(836.),0.1,1.,0.01),(7,"Высота",self.fx_property_y(880.),0.1,1.,0.01),
            (8,"Мягкость края",self.fx_property_y(924.),0.02,1.,0.01),(9,"Дым над огнём",self.fx_property_y(968.),0.,1.,0.01),
            (2,"Детализация",self.fx_property_y(1044.),1.,16.,0.1),(3,"Завихрения",self.fx_property_y(1088.),0.,1.,0.01),
            (4,"Скорость",self.fx_property_y(1132.),0.,5.,0.01),(5,"Случайность",self.fx_property_y(1176.),0.,999999.,1.),
            (10,"Снос X",self.fx_property_y(1220.),-0.5,0.5,0.01),(11,"Снос Z",self.fx_property_y(1264.),-0.5,0.5,0.01),
            (12,"Мерцание огня",self.fx_property_y(1308.),0.,1.,0.01),
            (13,"Радиус источника",self.fx_property_y(1600.),0.02,2.,0.01),(14,"Влияние полей",self.fx_property_y(1752.),0.,2.,0.01),
        ] {
            let rect=[x,y,w,36.];let value=number(settings,index).unwrap();
            if editable && (fire || !matches!(index,9|12)) && (index!=2 || !settings.gas) && (index!=13 || settings.source.is_some()) && (index!=14 || settings.use_fields) {u.number_field(rect,label,value,Action::Fx(F::Field(index)),SliderRange::new(min,max,step));}
            else {u.disabled_button(rect,&format!("{label}: {}",ui::number_caption(value)));}
        }
        for (rect,label,action,checked,enabled) in [
            ([x,self.fx_property_y(1012.),half,28.],"Облако",F::Cloud,settings.cloud || settings.burst,!settings.burst),
            ([x+half+8.,self.fx_property_y(1012.),half,28.],"Взрыв",F::Burst,settings.burst,true),
        ] {
            if editable && enabled {u.checkbox(rect,label,Action::Fx(action),checked);}
            else {u.disabled_checkbox(rect,label,checked);}
        }
        if self.fx_timing() {
            for (index,label,y,min,max,step) in [(15,"Длительность, с",1476.,0.1,30.,0.1),(16,"Начало, с",1520.,0.,86400.,0.1)] {
                let rect=[x,y,w,36.];let value=number(settings,index).unwrap();
                if editable {u.number_field(rect,label,value,Action::Fx(F::Field(index)),SliderRange::new(min,max,step));}
                else {u.disabled_button(rect,&format!("{label}: {}",ui::number_caption(value)));}
            }
            if editable {u.checkbox([x,1564.,w,28.],"Повторять",Action::Fx(F::Loop),settings.looped);}
            else {u.disabled_checkbox([x,1564.,w,28.],"Повторять",settings.looped);}
        }
        let caption=format!("Цвет · {}",surfaces::color_hex(settings.color));
        if editable {u.button([x,self.fx_property_y(762.),w,30.],&caption,Action::Fx(F::Color),false);}else{u.disabled_button([x,self.fx_property_y(762.),w,30.],&caption);}
        if editable && fire {u.checkbox([x,self.fx_property_y(1384.),w,28.],"Освещать сцену",Action::Fx(F::Illuminate),settings.illuminate);}
        else {u.disabled_checkbox([x,self.fx_property_y(1384.),w,28.],"Освещать сцену",fire && settings.illuminate);}
        let no_slot=fire && settings.enabled && settings.visible_at(self.fx_view_time()) && settings.illuminate && settings.density>0. && settings.emission>0. && !self.fx_light_has_slot(id);
        u.fitted_label([x,self.fx_property_y(1422.),w,20.],if no_slot {"Лимит света занят: 8 из 8"}else if fire {"Свет огня · без собственных теней"}else{"Освещение сцены доступно у огня"},10.,if no_slot {u.palette().warn}else{u.palette().muted});
        u.fitted_label([x,self.fx_property_y(1446.),w,20.],"Размер области — масштабом объекта",10.,u.palette().muted);
        let defaults=FxSettings {enabled:settings.enabled,source:settings.source,..FxSettings::preset(settings.kind)};
        if editable && settings!=defaults {u.icon_button([x,self.fx_property_y(1480.),w,30.],"Сбросить параметры FX",Action::Fx(F::Defaults),false,Icon::Reset);}
        else {u.disabled_icon_button([x,self.fx_property_y(1480.),w,30.],"Сбросить параметры FX",Icon::Reset);}
        let source=settings.source.and_then(|id|self.editor.project.node(id));
        let caption=source.map_or("Выбрать меш или кривую…",|n|n.name.as_str());
        let field=[x,self.fx_property_y(1554.),w-76.,32.];let pick=[x+w-70.,self.fx_property_y(1554.),32.,32.];let clear=[x+w-34.,self.fx_property_y(1554.),32.,32.];
        if editable {
            u.button(field,"",Action::Collision(ui::CollisionAction::SourceOpen),self.collision_popup_open());
            u.quiet_icon_button(pick,"",Action::Collision(ui::CollisionAction::SourcePick),false,Icon::Eyedropper);
        } else {u.disabled_button(field,"");u.disabled_icon_button(pick,"",Icon::Eyedropper);}
        u.fitted_label([x+8.,self.fx_property_y(1562.),w-108.,18.],caption,11.,u.palette().text);
        if editable && source.is_some() {u.quiet_icon_button(clear,"",Action::Fx(F::Source(None)),false,Icon::Close);}
        else {u.disabled_icon_button(clear,"",Icon::Close);}
        u.fitted_label([x,self.fx_property_y(1646.),w,20.],"Меш — поверхность · кривая — линия потока",10.,u.palette().muted);
        for (y,label,action,checked) in [(self.fx_property_y(1712.),"Силовые поля",F::Fields,settings.use_fields),(self.fx_property_y(1860.),"Столкновения с мешами",F::MeshCollision,settings.mesh_collision)] {
            if editable {u.checkbox([x,y,w,28.],label,Action::Fx(action),checked);}else{u.disabled_checkbox([x,y,w,28.],label,checked);}
        }
        let status=self.renderer.as_ref().and_then(|r|r.fx_status());
        u.fitted_label([x,self.fx_property_y(1796.),w,20.],status.unwrap_or("Сила · ветер · вихрь · магнит"),10.,if status.is_some(){u.palette().warn}else{u.palette().muted});
        self.draw_fx_advanced(u,x,w,settings,editable);
        self.draw_collision_list(u,poseforge_core::CollisionKind::Fx,x,self.fx_property_y(COLLISION_START),w);
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    fn app()->App {let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.preferences=Preferences::default();a.preferences.right_panel=true;a.width=1440.;a.height=900.;a}
    #[test]
    fn mesh_bound_fx_keeps_simulation_and_collision_controls_reachable() {
        let mut a=app();a.create_primitive(0);let source=a.editor.selected.unwrap();
        a.fx_action(F::Create(0));a.fx_action(F::Source(Some(source)));
        assert!(a.path_panel_bottom().is_none());
        assert!(a.inspector_content_bottom()>=a.fx_panel_bottom());
        let max=a.inspector_max_scroll();let mut clear=false;let mut collision=false;
        for step in 0..=(max/24.).ceil() as usize {
            a.inspector_scroll=(step as f32*24.).min(max);let u=a.build_ui();
            clear|=u.action_point(&Action::Fx(F::ClearCache)).is_some();
            collision|=u.action_point(&Action::Fx(F::MeshCollision)).is_some();
        }
        assert!(clear && collision,"bound FX must retain the full scrollable inspector");
    }
    fn legacy(a:&mut App,index:u8) {a.fx_action(F::Create(index));let (id,f)=a.selected_fx().unwrap();a.editor.project.node_mut(id).unwrap().fx=Some(FxSettings::preset(f.kind));}
    #[test]
    fn cinematic_blast_is_an_undoable_fire_preset() {
        let mut a=app();a.fx_action(F::Create(0));let before=a.editor.project.clone();
        let undo=a.editor.undo_len();a.fx_action(F::Preset(3));
        let f=a.selected_fx().unwrap().1;
        assert!(f.pyro.unwrap().cinematic_blast && f.burst && f.gas);
        f.validate().unwrap();assert_eq!(a.editor.undo_len(),undo+1);
        a.editor.undo();assert_eq!(a.editor.project,before);
        a.fx_action(F::Create(1));let smoke=a.editor.project.clone();
        a.fx_action(F::Preset(3));assert_eq!(a.editor.project,smoke);
    }
    #[test]
    fn gas_presets_and_profiles_are_undoable_without_recording_animation() {
        let mut a=app();legacy(&mut a,1);let before=a.editor.project.clone();let undo=a.editor.undo_len();
        a.fx_action(F::Preset(1));assert!(a.selected_fx().unwrap().1.gas);assert_eq!(a.editor.undo_len(),undo+1);a.editor.undo();assert_eq!(a.editor.project,before);
        a.fx_action(F::Evolution);let enabled=a.editor.project.clone();let undo=a.editor.undo_len();
        a.fx_action(F::Point(2));a.fx_action(F::Channel(1));assert_eq!(a.editor.project,enabled);assert_eq!(a.editor.undo_len(),undo);
        for (index,text) in [(32,"5"),(44,"90"),(36,"90"),(37,"120"),(38,"NaN")] {assert!(a.fx_confirm(&Action::Fx(F::Field(index)),text).is_err());}
        assert!(a.fx_confirm(&Action::Fx(F::KeyColor(2)),"#xyzxyz").is_err());assert_eq!(a.editor.project,enabled);
        a.fx_confirm(&Action::Fx(F::Field(37)),"42,5").unwrap();assert_eq!(a.selected_fx().unwrap().1.envelope.keys[1].size,0.425);a.editor.undo();assert_eq!(a.editor.project,enabled);
        a.fx_confirm(&Action::Fx(F::KeyColor(2)),"#5582FF").unwrap();a.editor.undo();assert_eq!(a.editor.project,enabled);
        a.fx_action(F::Gas);let before=a.editor.project.clone();let undo=a.editor.undo_len();a.fx_action(F::Reset);a.fx_action(F::Play);a.tick_fx(0.1);a.fx_action(F::ClearCache);
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
    }
    #[test]
    fn gas_and_envelope_controls_remain_separate_and_collapse_at_both_dpi_scales() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
            let mut a=app();a.scale=scale;a.preferences.theme=theme;legacy(&mut a,0);a.fx_action(F::Gas);a.fx_action(F::Evolution);a.fx_action(F::Point(1));
            let sections=a.fx_property_groups();assert!(sections.windows(2).all(|p|p[0].0<p[1].0));
            let mut u=Ui::new(scale);u.theme=theme;a.draw_fx_properties(&mut u,100.,260.);
            let rect=|action|u.hits.iter().find(|h|h.action==action).unwrap().rect;
            let gas=rect(Action::Fx(F::Gas));let evo=rect(Action::Fx(F::Evolution));let color=rect(Action::Fx(F::KeyColor(1)));let collision=rect(Action::Fx(F::MeshCollision));
            assert!(gas[1]+gas[3]<evo[1]);assert!(evo[1]+evo[3]<color[1]);assert!(color[1]+color[3]<collision[1]);
            assert!(u.hits.iter().all(|h|h.rect[0]>=116. && h.rect[0]+h.rect[2]<=344.001), "out of bounds: {:?}",u.hits.iter().filter(|h|h.rect[0]<116. || h.rect[0]+h.rect[2]>344.001).map(|h|(&h.action,h.rect)).collect::<Vec<_>>());
            a.preferences.collapsed_sections.extend([2410,2411]);let mut u=Ui::new(scale);u.theme=theme;a.draw_fx_properties(&mut u,100.,260.);a.draw_property_sections(&mut u,(0,0,0));
            assert!(!u.hits.iter().any(|h|matches!(h.action,Action::Fx(F::Gas|F::Evolution|F::KeyColor(_)))));
            assert!(u.hits.iter().any(|h|h.action==Action::Fx(F::MeshCollision)));
        }}
    }
    #[test]
    fn preview_play_pause_reset_do_not_write_keys_or_undo_steps() {
        let mut a=app();legacy(&mut a,0);let before=a.editor.project.clone();let undo=a.editor.undo_len();
        a.tick_fx(0.05);assert!(a.fx_view_time()>0.);a.fx_action(F::Play);let paused=a.fx_view_time();
        a.tick_fx(0.05);assert_eq!(a.fx_view_time(),paused);a.fx_action(F::Reset);assert_eq!(a.fx_view_time(),0.);
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
        a.editor.undo();assert!(a.editor.project.nodes.is_empty());assert!(!a.fx_preview_active());
    }
    #[test]
    fn fx_parameters_reject_invalid_values_and_changes_are_undoable() {
        let mut a=app();legacy(&mut a,1);let before=a.editor.project.clone();
        for (field,text) in [(0,"NaN"),(0,"-1"),(3,"2"),(5,"1.5")] {assert!(a.fx_confirm(&Action::Fx(F::Field(field)),text).is_err());}
        assert!(a.fx_confirm(&Action::Fx(F::Color),"#XYZ").is_err());assert_eq!(a.editor.project,before);
        a.fx_confirm(&Action::Fx(F::Field(0)),"2,5").unwrap();assert_eq!(a.selected_fx().unwrap().1.density,2.5);
        a.editor.undo();assert_eq!(a.editor.project,before);
        a.fx_action(F::Kind(2));assert_eq!(a.selected_fx().unwrap().1,FxSettings::preset(FxKind::Fog));
        a.editor.undo();assert_eq!(a.editor.project,before);
    }
    #[test]
    fn shape_edits_and_preset_reset_are_atomic_and_preserve_transform_and_clock() {
        let mut a=app();legacy(&mut a,0);let id=a.selected_fx().unwrap().0;
        let before=a.editor.project.clone();
        for (field,text) in [(6,"0"),(7,"1.1"),(8,"NaN"),(9,"2"),(10,"-0.51"),(11,"inf"),(12,"-1")] {
            assert!(a.fx_confirm(&Action::Fx(F::Field(field)),text).is_err());
        }
        assert_eq!(a.editor.project,before);
        for (field,text) in [(6,"0,4"),(7,"0.6"),(8,"0.8"),(9,"0.9"),(10,"-0.3"),(11,"0.2"),(12,"0.7")] {
            a.fx_confirm(&Action::Fx(F::Field(field)),text).unwrap();
        }
        a.fx_action(F::Enabled);a.editor.project.node_mut(id).unwrap().transform.translation=Vec3::new(1.,2.,3.);
        a.fx_preview.time=Some(4.25);let edited=a.editor.project.clone();let undo=a.editor.undo_len();
        a.fx_action(F::Defaults);assert_eq!(a.editor.undo_len(),undo+1);
        assert_eq!(a.selected_fx().unwrap().1,FxSettings {enabled:false,..FxSettings::preset(FxKind::Fire)});
        assert_eq!(a.editor.project.node(id).unwrap().transform,edited.node(id).unwrap().transform);assert_eq!(a.fx_view_time(),4.25);
        a.fx_action(F::Defaults);assert_eq!(a.editor.undo_len(),undo+1);
        a.editor.undo();assert_eq!(a.editor.project,edited);
    }
    #[test]
    fn collapsed_fx_groups_remove_their_hits_and_keep_following_curve_settings_accessible() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
            let mut a=app();a.preferences.theme=theme;a.scale=scale;legacy(&mut a,0);
            assert_eq!(a.path_panel_start(),Some(a.fx_panel_bottom()));
            let initial=a.collapsed_content_bottom();let before=a.editor.project.clone();
            let shape=a.property_sections().into_iter().find(|s|s.id==2402).unwrap();
            a.action(Action::ToggleSection(shape.id));assert!(a.property_y(a.fx_property_y(836.)).is_none());assert!(a.property_y(a.fx_property_y(1044.)).is_some());
            assert!(a.collapsed_content_bottom()<initial);
            let mut u=Ui::new(scale);u.theme=theme;a.draw_fx_properties(&mut u,100.,260.);a.draw_property_sections(&mut u,(0,0,0));
            assert!(!u.hits.iter().any(|h|h.action==Action::Fx(F::Field(6))));
            assert!(u.hits.iter().any(|h|h.action==Action::Fx(F::Field(2))));
            assert_eq!(a.editor.project,before);
            a.action(Action::ToggleSection(shape.id));assert_eq!(a.collapsed_content_bottom(),initial);
            for source_y in [a.fx_property_y(836.),a.fx_property_y(1044.),a.fx_property_y(1384.),a.fx_panel_bottom()+24.] {
                let screen_y=a.property_y(source_y).unwrap();assert!((a.property_cursor_y(screen_y)-source_y).abs()<0.001);
            }
        }}
    }
    #[test]
    fn fx_creation_is_beside_field_and_controls_stay_inside_panel_at_both_scales() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
            let mut a=app();a.scale=scale;a.preferences.theme=theme;legacy(&mut a,0);
            let mut u=Ui::new(scale);u.theme=theme;a.draw_creation_toolbar(&mut u);
            let rect=|kind|u.hits.iter().find(|h|h.action==Action::DropdownOpen(kind)).unwrap().rect;
            let field=rect(DropdownKind::ForceCreate);let fx=rect(DropdownKind::FxCreate);
            assert_eq!(field[1],fx[1]);assert!(fx[0]>=field[0]+field[2]);
            let [x,_,w,_]=a.creation_toolbar_rect();assert!(fx[0]+fx[2]<=x+w);
            let mut u=Ui::new(scale);u.theme=theme;a.draw_fx_properties(&mut u,100.,260.);
            assert!(u.hits.iter().all(|h|h.rect[0]>=116. && h.rect[0]+h.rect[2]<=344.001 && h.rect[1]+h.rect[3]<=a.helper_panel_bottom()));
        }}
    }
    #[test]
    fn burst_restart_uses_selected_start_and_does_not_write_animation_or_history() {
        let mut a=app();legacy(&mut a,0);a.fx_confirm(&Action::Fx(F::Field(16)),"4,25").unwrap();
        a.fx_confirm(&Action::Fx(F::Field(15)),"0.2").unwrap();a.fx_action(F::Burst);
        assert_eq!(a.fx_view_time(),4.25);assert!(a.fx_preview_active());
        let before=a.editor.project.clone();let undo=a.editor.undo_len();
        for _ in 0..10 {a.tick_fx(0.1);}assert!(!a.fx_preview_active());
        a.start_fx_preview();assert_eq!(a.fx_view_time(),4.25);assert!(a.fx_preview_active());
        for _ in 0..10 {a.tick_fx(0.1);}assert!(!a.fx_preview_active());
        a.fx_action(F::Play);assert_eq!(a.fx_view_time(),4.25);assert!(a.fx_preview_active());
        a.tick_fx(0.1);a.fx_action(F::Reset);assert_eq!(a.fx_view_time(),4.25);assert!(!a.fx_preview_active());
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
        a.fx_action(F::Kind(1));let f=a.selected_fx().unwrap().1;assert!(f.burst);assert_eq!(f.start,4.25);assert_eq!(f.duration,0.2);
        for (index,value) in [(15,"0"),(15,"NaN"),(16,"-1"),(16,"inf"),(16,"86401")] {assert!(a.fx_confirm(&Action::Fx(F::Field(index)),value).is_err());}
        let field=a.editor.create_force_field(poseforge_core::ForceFieldKind::Wind).unwrap();
        a.editor.project.node_mut(field).unwrap().force_field.as_mut().unwrap().targets=[false;3];
        a.action(Action::Select(field));let before=a.editor.project.clone();let undo=a.editor.undo_len();
        a.helper_action(ui::HelperAction::Play);for _ in 0..10 {a.tick_fx(0.1);}assert!(!a.fx_preview.running);
        a.helper_action(ui::HelperAction::Play);assert!(a.fx_preview.running);assert_eq!(a.fx_view_time(),4.25);
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
    }
    #[test]
    fn burst_time_controls_collapse_without_overlapping_light_source_or_collision_rows() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {
            let mut a=app();a.scale=scale;a.preferences.theme=theme;legacy(&mut a,0);
            let normal_bottom=a.fx_panel_bottom();a.preferences.collapsed_sections.insert(2404);a.fx_action(F::Burst);assert_eq!(a.fx_panel_bottom(),normal_bottom+180.);
            let section=a.property_sections().into_iter().find(|s|s.id==2409).unwrap();
            assert!(a.property_y(a.fx_property_y(1384.)).is_none(),"lighting collapse state survives insertion of timing group");
            a.preferences.collapsed_sections.remove(&2404);
            let mut u=Ui::new(scale);u.theme=theme;a.draw_fx_properties(&mut u,100.,260.);
            for index in [15,16] {let hit=u.hits.iter().find(|h|h.action==Action::Fx(F::Field(index))).unwrap();assert!(hit.rect[1]>=1448. && hit.rect[1]+hit.rect[3]<1628.);}
            let before=a.editor.project.clone();a.action(Action::ToggleSection(section.id));
            let mut u=Ui::new(scale);u.theme=theme;a.draw_fx_properties(&mut u,100.,260.);a.draw_property_sections(&mut u,(0,0,0));
            assert!(!u.hits.iter().any(|h|matches!(h.action,Action::Fx(F::Field(15|16)|F::Loop))));
            assert!(u.hits.iter().any(|h|h.action==Action::Fx(F::Illuminate)));assert_eq!(a.editor.project,before);
            a.fx_action(F::Burst);assert_eq!(a.fx_panel_bottom(),normal_bottom);
        }}
    }

}
