//! Shared fields keep keyboard editing, cancellation, Undo and theme behavior.
use super::*;
use crate::ui::{DropdownKind,FxAction as F};
use poseforge_core::FxSettings;

impl App {
    pub(super) fn draw_fx_advanced(&self,u:&mut Ui,x:f32,w:f32,f:FxSettings,editable:bool) {
        let (height,evolution)=self.fx_advanced_heights();let start=self.fx_property_y(1832.)-height-evolution;
        if editable {u.button([x,start+8.,w,30.],"Перевести в новый FX",Action::Fx(F::Upgrade),false);}else{u.disabled_button([x,start+8.,w,30.],"Перевести в новый FX");}
        let y=start+44.;
        let quality=format!("Качество · {}",f.quality.label());
        if editable {self.draw_dropdown_button(u,[x,y+28.,w,30.],DropdownKind::FxQuality,&quality);}
        else {u.disabled_button([x,y+28.,w,30.],&quality);}
        check(u,[x,y+68.,w,28.],"Расчёт газа",F::Gas,f.gas,editable);
        if f.gas {
            for (i,label,offset,min,max,step) in [(17,"Область потока",108.,1.,4.,0.1),(18,"Рассеивание",152.,0.,3.,0.01),(19,"Подъём газа",196.,0.,4.,0.05)] {
                field(u,[x,y+offset,w,36.],label,i,f,min,max,step,editable);
            }
            check(u,[x,y+240.,w,28.],"Полые препятствия",F::Hollow,f.gas_hollow,editable);
            if editable {u.icon_button([x,y+280.,w,30.],"Очистить кэш",Action::Fx(F::ClearCache),false,Icon::Reset);}
            else {u.disabled_icon_button([x,y+280.,w,30.],"Очистить кэш",Icon::Reset);}
            let memory=self.renderer.as_ref().map_or(0,|r|r.fx_cache_bytes());
            u.fitted_label([x,y+318.,w,20.],&format!("Кэш: {:.1} / 96 МБ · 30 кадров/с",memory as f64/1048576.),10.,u.palette().muted);
        }
        self.draw_fx_envelope(u,x,w,start+height,f,editable);
    }
    pub(super) fn draw_fx_envelope(&self,u:&mut Ui,x:f32,w:f32,y:f32,f:FxSettings,editable:bool) {
        check(u,[x,y+28.,w,28.],"Изменять параметры",F::Evolution,f.envelope.enabled,editable);
        if !f.envelope.enabled {return;}
        check(u,[x,y+64.,w,28.],"Плавные переходы",F::Smooth,f.envelope.smooth,editable);
        let channel=self.fx_preview.channel.min(2);let point=self.fx_preview.point.min(3);
        let rect=[x,y+104.,w,96.];let p=u.palette();u.outlined_surface(rect,p.bg,p.border);
        let left=x+8.;let top=y+112.;let width=(w-16.).max(1.);let height=80.;
        for fraction in [0.25,0.5,0.75] {u.smooth_segment([left+width*fraction,top],[left+width*fraction,top+height],1.,p.border);}
        let sample=|t:f32|{let k=f.envelope.sample(t);let value=match channel {0=>k.size,1=>k.density/2.,_=>k.emission/2.};[left+width*t,top+height*(1.-value)]};
        for i in 0..64 {u.smooth_segment(sample(i as f32/64.),sample((i+1) as f32/64.),2.,p.accent);}
        for (i,k) in f.envelope.keys.iter().enumerate() {let pos=sample(k.at);let r=if i==point as usize {3.5}else{2.};u.rect([pos[0]-r,pos[1]-r,r*2.,r*2.],if i==point as usize {p.text}else{p.accent});}
        let cell=(w-8.)/3.;
        for (i,label) in ["Размер","Плотн.","Свет"].into_iter().enumerate() {u.button([x+i as f32*(cell+4.),y+208.,cell,28.],label,Action::Fx(F::Channel(i as u8)),i==channel as usize);}
        let cell=(w-12.)/4.;
        for i in 0..4 {u.button([x+i as f32*(cell+4.),y+244.,cell,28.],&format!("{}",i+1),Action::Fx(F::Point(i)),i==point);}
        let index=32+point*4;let keys=f.envelope.keys;
        let min=if point==0 {0.}else{(keys[point as usize-1].at+0.01) as f64*100.};
        let max=if point==3 {100.}else{(keys[point as usize+1].at-0.01) as f64*100.};
        field(u,[x,y+284.,w,36.],"Время, %",index,f,min,max,1.,editable && point>0 && point<3);
        for (offset,label,slot,max) in [(328.,"Размер, %",1,100.),(372.,"Плотность, %",2,200.),(416.,"Свечение, %",3,200.)] {
            field(u,[x,y+offset,w,36.],label,index+slot,f,0.,max,1.,editable);
        }
        let label=format!("Оттенок · {}",surfaces::color_hex(keys[point as usize].color));
        if editable {u.button([x,y+460.,w,30.],&label,Action::Fx(F::KeyColor(point)),false);}
        else {u.disabled_button([x,y+460.,w,30.],&label);}
    }
}
fn check(u:&mut Ui,r:[f32;4],label:&str,action:F,value:bool,enabled:bool) {
    if enabled {u.checkbox(r,label,Action::Fx(action),value);}else{u.disabled_checkbox(r,label,value);}
}
#[allow(clippy::too_many_arguments)]
fn field(u:&mut Ui,r:[f32;4],label:&str,index:u8,f:FxSettings,min:f64,max:f64,step:f64,enabled:bool) {
    let value=fx_controls::number(f,index).unwrap_or_default();
    if enabled {u.number_field(r,label,value,Action::Fx(F::Field(index)),SliderRange::new(min,max,step));}
    else {u.disabled_button(r,&format!("{label}: {}",ui::number_caption(value)));}
}
