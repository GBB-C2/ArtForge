//! Per-map UV transforms and shared, timeline-driven video assets.
use super::*;
use crate::ui::MediaAction as M;
impl App {
    fn media_material(&self) -> Option<&poseforge_core::Material> { self.editor.project.material(self.selected_material_id()?) }
    fn media_video(&self, slot: usize) -> Option<&poseforge_core::VideoTexture> {
        let id = surfaces::texture_slot(self.media_material()?, slot)?;
        self.editor.project.video_textures.iter().find(|v| v.texture == id)
    }
    pub(super) fn material_map_details_height(&self) -> f32 {
        if !self.surface.maps { return 0.; }
        self.surface.media_slot.filter(|&s| s < 5).map_or(0., |s| 214. + if self.editor.project.video_textures.is_empty() {0.} else {34.} + if s == 2 { 32. } else { 0. } + if self.media_video(s).is_some() { 276. } else { 0. })
    }
    fn media_value(&self, a: M) -> Option<f64> {
        Some(match a {
            M::UvNumber(slot, field) => { let uv = self.media_material()?.texture_uvs.get(slot)?;
                match field { 0..=1 => uv.scale[field as usize] as f64, 2..=3 => uv.offset[field as usize-2] as f64, 4 => uv.rotation.to_degrees() as f64, _ => return None } }
            M::VideoNumber(slot, field) => { let v = &self.media_video(slot)?.playback; match field { 0 => v.start, 1 => v.offset, 2 => v.speed, _ => return None } }
            _ => return None,
        })
    }
    pub(super) fn media_action(&mut self, action: M) {
        if self.job.is_some() || self.scene_write_pending() || self.editor.playing || self.editor.has_staged() { return; }
        if let M::Slot(s) = action { if s < 5 { self.surface.media_slot = if self.surface.media_slot == Some(s) { None } else { Some(s) }; } return; }
        if matches!(action, M::UvNumber(..) | M::VideoNumber(..)) {
            if let Some(v) = self.media_value(action) { self.input(Action::Media(action), ui::number_caption(v)); } return;
        }
        if action == M::PruneVideos {
            let used: BTreeSet<_> = self.editor.project.materials.iter().flat_map(|m| {
                (0..5).filter_map(move |slot| surfaces::texture_slot(m,slot)).chain(m.graph.iter().flat_map(|g| g.nodes.iter().filter_map(|n| n.texture)))
            }).collect();
            let removed: BTreeSet<_> = self.editor.project.video_textures.iter().filter(|v| !used.contains(&v.texture)).map(|v| v.texture).collect();
            let result = self.editor.transact("Удалить неиспользуемые видео", |p| { p.video_textures.retain(|v| !removed.contains(&v.texture)); p.textures.retain(|t| !removed.contains(&t.id)); });
            self.result(result); self.update_dirty(); return;
        }
        let Some(id) = self.selected_material_id() else { return; };
        let video_id = match action { M::VideoLoop(s) | M::VideoEnabled(s) => self.media_video(s).map(|v| v.texture), _ => None };
        let result = self.editor.transact("Настройки материала", |p| {
            if let Some(m) = p.materials.iter_mut().find(|m| m.id == id) {
                match action {
                    M::ResetUv(s) => { if let Some(uv) = m.texture_uvs.get_mut(s) { *uv = Default::default(); } }
                    M::NormalFlip => m.normal_y_flip = !m.normal_y_flip,
                    M::NprPreset(i) => { if let Some(npr) = poseforge_core::NprSettings::preset(i) { m.npr = npr; } }
                    _ => {}
                }
            }
            if let Some(v) = p.video_textures.iter_mut().find(|v| Some(v.texture) == video_id) {
                match action { M::VideoLoop(_) => v.playback.looped = !v.playback.looped, M::VideoEnabled(_) => v.playback.enabled = !v.playback.enabled, _ => {} }
            }
        });
        self.result(result); self.update_dirty();
    }
    pub(super) fn media_confirm(&mut self, action: &Action, text: &str) -> Result<bool, String> {
        let Action::Media(a) = *action else { return Ok(false); };
        if !matches!(a, M::UvNumber(..) | M::VideoNumber(..)) { return Ok(false); }
        let id = self.selected_material_id().ok_or("Выберите материал")?;
        let (min, max) = match a {
            M::UvNumber(_,0..=3) => (-1000., 1000.), M::UvNumber(_,4) => (-180.,180.),
            M::VideoNumber(_,0) => (0.,86400.), M::VideoNumber(s,1) => (0.,self.media_video(s).ok_or("Видео удалено")?.duration() as f32),
            M::VideoNumber(_,2) => (0.1,4.), _ => return Err("Неизвестный параметр карты".into()),
        };
        let v = surfaces::number(text,min,max)?;
        let video_id = match a { M::VideoNumber(s,_) => Some(self.media_video(s).ok_or("Видео удалено")?.texture), _ => None };
        self.editor.transact("Параметр карты материала", |p| {
            if let M::UvNumber(slot,field) = a {
                if let Some(uv) = p.materials.iter_mut().find(|m| m.id == id).and_then(|m| m.texture_uvs.get_mut(slot)) {
                    match field { 0..=1 => uv.scale[field as usize] = v, 2..=3 => uv.offset[field as usize-2] = v, 4 => uv.rotation = v.to_radians(), _ => {} }
                }
            }
            if let M::VideoNumber(_,field) = a {
                if let Some(clip) = p.video_textures.iter_mut().find(|c| Some(c.texture) == video_id) {
                    match field { 0 => clip.playback.start = v as f64, 1 => clip.playback.offset = (v as f64).min(clip.duration()), 2 => clip.playback.speed = v as f64, _ => {} }
                }
            }
        }).map_err(|e| e.to_string())?;
        self.update_dirty(); Ok(true)
    }
    pub(super) fn draw_material_map_details(&self, u: &mut Ui, x: f32, y: f32, w: f32, slot: usize) {
        if self.surface.media_slot != Some(slot) { return; }
        let Some(m) = self.media_material() else { return; };
        let half = (w-6.)*0.5;
        for (field,label) in [(0,"Масштаб U"),(1,"Масштаб V"),(2,"Сдвиг U"),(3,"Сдвиг V")] {
            u.number_field([x+(field%2) as f32*(half+6.), y+(field/2) as f32*50.,half,44.],label,
                self.media_value(M::UvNumber(slot,field)).unwrap_or(0.),Action::Media(M::UvNumber(slot,field)),SliderRange::new(-10.,10.,0.01));
        }
        u.number_field([x,y+100.,w,44.],"Поворот карты, °",self.media_value(M::UvNumber(slot,4)).unwrap_or(0.),Action::Media(M::UvNumber(slot,4)),SliderRange::new(-180.,180.,1.));
        u.button([x,y+150.,w,28.],"Сбросить UV этой карты",Action::Media(M::ResetUv(slot)),false);
        if slot == 2 { u.checkbox([x,y+184.,w,28.],"Инвертировать Y нормалей",Action::Media(M::NormalFlip),m.normal_y_flip); }
        u.fitted_label([x,y+if slot==2 {218.} else {184.},w,18.],"Поверх общего преобразования UV",10.,u.palette().muted);
        let mut video_y = y+214.+if slot==2 {32.} else {0.};
        if !self.editor.project.video_textures.is_empty() {
            u.button([x,video_y,w,28.],"Удалить неиспользуемые видео",Action::Media(M::PruneVideos),false); video_y += 34.;
        }
        if let Some(v) = self.media_video(slot) {
            let y = video_y;
            u.fitted_label([x,y,w,18.],&format!("Видео · {:.2} с · {}×{} · 24 к/с",v.duration(),v.width,v.height),11.,u.palette().text);
            u.checkbox([x,y+24.,w,28.],"Воспроизводить по времени сцены",Action::Media(M::VideoEnabled(slot)),v.playback.enabled);
            u.checkbox([x,y+56.,w,28.],"Повторять",Action::Media(M::VideoLoop(slot)),v.playback.looped);
            for (field,label,max) in [(0,"Начало в сцене, с",60.),(1,"Смещение в клипе, с",v.duration()),(2,"Скорость",4.)] {
                u.number_field([x,y+90.+field as f32*50.,w,44.],label,self.media_value(M::VideoNumber(slot,field)).unwrap_or(0.),Action::Media(M::VideoNumber(slot,field)),SliderRange::new(if field==2 {0.1} else {0.},max,0.01));
            }
            let message = self.video_preview.error(v.texture).unwrap_or("Настройки общие для этого видео");
            u.fitted_label([x,y+244.,w,20.],message,10.,u.palette().muted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut p=Project::default(); p.materials.push(poseforge_core::Material {id:1,..Default::default()});
        p.meshes.push(poseforge_core::box_mesh(2,"Cube",Vec3::ONE,Some(1))); let mut n=poseforge_core::Node::new(3,"Cube"); n.mesh=Some(2); p.nodes.push(n);
        let mut a=App::new(None);a.editor=Editor::new(p);a.editor.selected=Some(3);a.surface.selection=Some(1);a
    }
    #[test]
    fn independent_uv_edits_reset_and_undo_without_touching_other_slots() {
        let mut a=app();assert_eq!(a.media_confirm(&Action::Media(M::UvNumber(2,0)),"-2"),Ok(true));
        let m=a.editor.project.material(1).unwrap();assert_eq!(m.texture_uvs[2].scale,[-2.,1.]);assert_eq!(m.texture_uvs[0].scale,[1.,1.]);
        a.media_action(M::ResetUv(2));assert_eq!(a.editor.project.material(1).unwrap().texture_uvs[2].scale,[1.,1.]);
        a.editor.undo();assert_eq!(a.editor.project.material(1).unwrap().texture_uvs[2].scale,[-2.,1.]);
        assert!(a.media_confirm(&Action::Media(M::UvNumber(0,4)),"NaN").is_err());
    }
    #[test]
    fn npr_preset_preserves_base_color_and_normal_flip_is_undoable() {
        let mut a=app();let color=a.editor.project.material(1).unwrap().base_color;
        a.media_action(M::NprPreset(3));let m=a.editor.project.material(1).unwrap();assert_eq!(m.base_color,color);assert!(m.npr.halftone_strength>0.);
        a.media_action(M::NormalFlip);assert!(a.editor.project.material(1).unwrap().normal_y_flip);a.editor.undo();assert!(!a.editor.project.material(1).unwrap().normal_y_flip);
    }
}
