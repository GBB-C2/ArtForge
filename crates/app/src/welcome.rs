//! Startup navigation stays outside the document and undo history.
use super::*;
use crate::localization::Language;
use std::sync::OnceLock;

#[derive(Default)]
pub(super) struct Welcome {
    pub open: bool,
    pub focus: Option<Action>,
}

pub(super) fn artwork() -> Option<&'static poseforge_core::Texture> {
    static IMAGE:OnceLock<Option<poseforge_core::Texture>>=OnceLock::new();
    IMAGE.get_or_init(||poseforge_io::load_texture_bytes(include_bytes!("../../../docs/screenshots/fox.jpg"),"fox.jpg").ok()).as_ref()
}

impl App {
    pub(super) fn welcome_active(&self)->bool {
        self.welcome.open && self.dialog.is_none() && self.input.is_none() && self.overwrite.is_none()
            && self.job.is_none() && self.native_picker.is_none() && !self.close_recovery.is_pending()
    }
    pub(super) fn welcome_action(&mut self, action:&Action)->bool {
        match action {
            Action::ShowWelcome=>{
                if self.job.is_some() || !self.resolve_input() {return true;}
                self.cancel_drag();self.cancel_color_picker();self.dropdown=None;self.menu=None;
                self.welcome.open=true;self.welcome.focus=Some(Action::New);
            }
            Action::CloseWelcome=>self.welcome.open=false,
            Action::Language(language)=>{self.preferences.language=*language;self.persist_preferences();}
            Action::OpenRecent(index)=>{
                if self.job.is_some() {return true;}
                if self.dirty || self.editor.has_staged() {self.status="Сначала сохраните текущий проект.".into();}
                else if let Some(path)=self.preferences.recent_projects.get(*index).cloned() {self.start_load(path,false);}
            }
            _=>return false,
        }
        self.redraw();true
    }
    pub(super) fn welcome_key(&mut self,key:&Key,repeat:bool)->bool {
        if !self.welcome_active() {return false;}
        match key {
            Key::Named(NamedKey::Escape)=>self.action(Action::CloseWelcome),
            Key::Named(NamedKey::Tab | NamedKey::ArrowDown | NamedKey::ArrowUp)=>{
                self.ui=self.build_ui();
                let actions:Vec<_>=self.ui.hits.iter().map(|h|h.action.clone()).collect();
                if !actions.is_empty() {
                    let previous=*key==Key::Named(NamedKey::ArrowUp) || (*key==Key::Named(NamedKey::Tab) && self.modifiers.shift_key());
                    let at=self.welcome.focus.as_ref().and_then(|a|actions.iter().position(|v|v==a));
                    let next=match (at,previous) {(Some(i),false)=>(i+1)%actions.len(),(Some(i),true)=>(i+actions.len()-1)%actions.len(),(None,false)=>0,(None,true)=>actions.len()-1};
                    self.welcome.focus=Some(actions[next].clone());
                }
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) if !repeat=>self.action(self.welcome.focus.clone().unwrap_or(Action::New)),
            Key::Character(ch) if self.modifiers.control_key() || self.modifiers.super_key()=>match ch.to_lowercase().as_str(){"o"=>self.action(Action::Open),"n"=>self.action(Action::New),_=>{}},
            _=>{},
        }
        self.redraw();true
    }
    pub(super) fn draw_welcome(&self,u:&mut Ui) {
        let p=u.palette();let lang=self.preferences.language;
        u.rects.clear();u.texts.clear();u.hits.clear();u.number_values.clear();
        u.rect([0.,0.,self.width,self.height],p.bg);
        let w=(self.width-32.).min(920.).max(280.);let h=(self.height-32.).min(560.).max(400.);
        let x=(self.width-w)*0.5;let y=(self.height-h)*0.5;
        u.surface([x,y,w,h],p.panel);
        let wide=w>=680.;let art=if wide {[x,y,w*0.46,h]}else{[x,y,w,132.]};
        u.rect(art,[0.15,0.16,0.17,1.]);
        if let Some(image)=artwork() {
            // Crop out the screenshot chrome and preserve the fox's proportions.
            let mut crop=[352.,190.,580.,640.];let ratio=art[2]/art[3];
            if crop[2]/crop[3]>ratio {let width=crop[3]*ratio;crop[0]+=(crop[2]-width)*0.5;crop[2]=width;}
            else {let height=crop[2]/ratio;crop[1]+=(crop[3]-height)*0.5;crop[3]=height;}
            u.image(art,[crop[0]/image.width as f32,crop[1]/image.height as f32,crop[2]/image.width as f32,crop[3]/image.height as f32]);
        }
        if wide {
            u.rect([art[0],art[1]+art[3]-78.,art[2],78.],[0.09,0.10,0.12,0.92]);
            u.label(art[0]+24.,art[1]+art[3]-58.,"ARTFORGE",24.,[0.96,0.96,0.97,1.]);
            u.label(art[0]+24.,art[1]+art[3]-26.,"3D artwork studio",12.,[0.70,0.72,0.76,1.]);
        }
        let cx=if wide {x+art[2]+28.}else{x+24.};let cy=if wide {y+30.}else{y+148.};
        let cw=x+w-cx-28.;
        u.label(cx,cy,"ArtForge Studio",24.,p.text);
        u.label(cx,cy+34.,format!("v{}",env!("CARGO_PKG_VERSION")),12.,p.muted);
        u.quiet_button([x+w-36.,y+8.,28.,28.],"×",Action::CloseWelcome,false);
        u.fitted_label([cx,cy+68.,cw-176.,20.],"Язык",12.,p.muted);
        for (i,(label,language)) in [("Русский",Language::Russian),("English β",Language::English)].into_iter().enumerate() {
            u.quiet_button([cx+cw-172.+i as f32*88.,cy+60.,84.,32.],label,Action::Language(language),lang==language);
        }
        u.icon_button([cx,cy+108.,cw,36.],"Новый проект",Action::New,true,Icon::File);
        u.icon_button([cx,cy+152.,cw,36.],"Открыть проект…",Action::Open,false,Icon::Folder);
        u.section_label([cx,cy+214.,cw,20.],"Последние проекты",12.);
        let count=if wide {3}else{1};
        if self.preferences.recent_projects.is_empty() {u.fitted_label([cx,cy+248.,cw,22.],"Здесь появятся открытые и сохранённые проекты",11.,p.muted);}
        for (i,path) in self.preferences.recent_projects.iter().take(count).enumerate() {
            let row=cy+244.+i as f32*60.;
            u.quiet_icon_button([cx,row,cw,30.],&path.file_name().unwrap_or_default().to_string_lossy(),Action::OpenRecent(i),false,Icon::File);
            u.raw_fitted_label([cx+28.,row+32.,cw-28.,16.],&path.to_string_lossy(),10.,p.muted);
        }
        if wide {u.fitted_label([cx,y+h-52.,cw,18.],"Esc — перейти в рабочую область",11.,p.muted);}
        u.fitted_label([cx,y+h-28.,cw,16.],&self.status,10.,p.muted);
        if let Some(rect)=self.welcome.focus.as_ref().and_then(|a|u.hits.iter().find(|hit|&hit.action==a).map(|hit|hit.rect)) {u.surface_outline(rect,p.accent);}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_is_modal_has_real_version_and_fits_both_languages_themes_and_dpi() {
        for language in [Language::Russian,Language::English] {for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,1.5,2.] {
            let mut a=App::new(None);a.width=1100.;a.height=760.;a.scale=scale;a.welcome.open=true;
            a.preferences.theme=theme;a.preferences.language=language;
            a.preferences.recent_projects=vec![PathBuf::from("/projects/Моя лиса.artforge")];
            let before=a.editor.project.clone();let ui=a.build_ui();
            assert!(ui.texts.iter().any(|t|t.text==format!("v{}",env!("CARGO_PKG_VERSION"))));
            assert!(ui.texts.iter().any(|t|t.text==language.text("Последние проекты")));
            assert!(ui.texts.iter().any(|t|t.text=="Моя лиса.artforge"));
            for h in &ui.hits {assert!(h.rect[0]>=0. && h.rect[1]>=0. && h.rect[0]+h.rect[2]<=a.width && h.rect[1]+h.rect[3]<=a.height);}
            assert!(!ui.hits.iter().any(|h|matches!(h.action,Action::Select(_)|Action::Import)));
            assert_eq!(ui.rects.iter().filter(|r|r.image_uv.is_some()).count(),1);
            a.ui=ui;a.mouse=[12.,12.];a.mouse_down(MouseButton::Left);assert!(a.drag.is_none());
            assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),0);
        }}}
        assert!(artwork().is_some());
    }
    #[test]
    fn startup_language_keyboard_and_new_project_are_functional() {
        let mut a=App::new(None);a.welcome.open=true;
        a.action(Action::Language(Language::English));assert_eq!(a.preferences.language,Language::English);
        assert!(a.welcome_key(&Key::Named(NamedKey::Tab),false));assert!(a.welcome.focus.is_some());
        a.welcome.focus=Some(Action::New);a.welcome_key(&Key::Named(NamedKey::Enter),false);
        assert!(!a.welcome.open);assert!(a.editor.project.nodes.is_empty());
        let ui=a.build_ui();assert!(!ui.texts.iter().any(|t|t.text=="Импорт модели" || t.text=="Import model"));
        a.action(Action::ShowWelcome);assert!(a.welcome.open);
        a.welcome_key(&Key::Named(NamedKey::Escape),false);assert!(!a.welcome.open);
    }
    #[test]
    fn recent_project_success_closes_welcome_but_missing_file_keeps_it_available() {
        let mut a=App::new(None);a.welcome.open=true;a.preferences.recent_projects=vec![PathBuf::from("/__artforge_missing__/missing.artforge")];
        let before=a.editor.project.clone();a.action(Action::OpenRecent(0));
        let limit=Instant::now()+std::time::Duration::from_secs(5);
        while a.job.is_some() {a.poll_jobs();assert!(Instant::now()<limit);std::thread::yield_now();}
        assert!(a.welcome.open);assert_eq!(a.editor.project,before);
        let (tx,receiver)=mpsc::channel();a.job=Some(Job {receiver,cancel:Arc::new(AtomicBool::new(false)),label:"Load".into()});
        tx.send(JobMessage::Loaded(Project::default(),PathBuf::from("/projects/fox.artforge"))).unwrap();a.poll_jobs();
        assert!(!a.welcome.open);assert_eq!(a.preferences.recent_projects[0],PathBuf::from("/projects/fox.artforge"));
    }
}
