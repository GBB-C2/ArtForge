//! Only completed video frames are published. Decoding never runs in the UI loop.
use super::*;
use poseforge_core::{Texture, VideoTexture};
#[derive(Clone, Copy)]
pub(super) enum Target { Slot(usize), Node(u32) }
#[derive(Clone)]
struct FrameRequest { clip: VideoTexture, index: usize }
struct Frame { clip: VideoTexture, index: usize, texture: Texture }
struct Failure { request: FrameRequest, message: String }
type FrameResults = Vec<(VideoTexture, usize, Result<Texture, String>)>;
struct Pending {
    receiver: Receiver<FrameResults>,
    cancel: Arc<AtomicBool>, time: f64, requests: Vec<FrameRequest>,
}
impl Drop for Pending { fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); } }
// Metadata can change without replacing the shared compressed byte buffer.
fn same_clip(a: &VideoTexture, b: &VideoTexture) -> bool {
    a.texture == b.texture && a.data.revision() == b.data.revision()
        && a.width == b.width && a.height == b.height && a.fps == b.fps
        && a.offsets == b.offsets && a.playback == b.playback
}
#[derive(Default)]
pub(super) struct VideoPreview {
    frames: BTreeMap<u64, Frame>, pending: Option<Pending>,
    failed: BTreeMap<u64, Failure>, last_time: f64,
}
impl VideoPreview {
    pub fn is_pending(&self) -> bool { self.pending.is_some() }
    fn poll(&mut self, clips: &[&VideoTexture], time: f64) {
        if let Some(job) = &self.pending {
            // Keep the receiver until the cancelled decoder has really exited.
            // Deleting/hiding videos and Undo cannot create parallel decoders.
            if !job.requests.iter().any(|r| clips.iter().any(|v| same_clip(v, &r.clip))) {
                job.cancel.store(true, Ordering::Relaxed);
            }
            let results = match job.receiver.try_recv() {
                Ok(results) => Some(results),
                Err(mpsc::TryRecvError::Disconnected) => Some(job.requests.iter().map(|r|
                    (r.clip.clone(), r.index, Err("Декодирование видео остановлено".into()))).collect()),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(results) = results {
                if !job.cancel.load(Ordering::Relaxed) && job.time <= time + 1e-8 {
                    for (clip, index, result) in results {
                        if !clips.iter().any(|v| same_clip(v, &clip)) { continue; }
                        match result {
                            Ok(texture) => { self.failed.remove(&clip.texture); self.frames.insert(clip.texture, Frame { clip, index, texture }); }
                            Err(message) => { self.failed.insert(clip.texture, Failure { request: FrameRequest { clip, index }, message }); }
                        }
                    }
                }
                self.pending = None;
            }
        }
    }
    pub fn update(&mut self, project: &Project, time: f64) {
        if time < self.last_time - 1e-8 {
            if let Some(job) = &self.pending { job.cancel.store(true, Ordering::Relaxed); }
            self.frames.clear(); self.failed.clear();
        }
        self.last_time = time;
        let clips = poseforge_io::video::active_video_textures(project, false);
        self.frames.retain(|_, frame| clips.iter().any(|v| same_clip(v, &frame.clip)));
        self.failed.retain(|_, failure| clips.iter().any(|v| same_clip(v, &failure.request.clip)));
        self.poll(&clips, time);
        if self.pending.is_some() { return; }
        let requests: Vec<_> = clips.into_iter().filter_map(|clip| {
            let index = clip.frame_at(time);
            let missing = self.frames.get(&clip.texture).is_none_or(|f| f.index != index)
                && self.failed.get(&clip.texture).is_none_or(|f| f.request.index != index);
            missing.then(|| FrameRequest { clip: clip.clone(), index })
        }).collect();
        if requests.is_empty() { return; }
        let work = requests.clone();
        let cancel = Arc::new(AtomicBool::new(false)); let c = cancel.clone();
        let (tx, receiver) = mpsc::channel();
        match std::thread::Builder::new().name("video-frames".into()).spawn(move || {
            let mut frames = vec![];
            for FrameRequest { clip, index } in work {
                if c.load(Ordering::Relaxed) { break; }
                let result = poseforge_io::decode_video_frame(&clip, index).map_err(|e| e.to_string());
                frames.push((clip, index, result));
            }
            let _ = tx.send(frames);
        }) {
            Ok(_) => self.pending = Some(Pending { receiver, cancel, time, requests }),
            Err(error) => {
                for request in requests {
                    self.failed.insert(request.clip.texture, Failure { request, message: error.to_string() });
                }
            }
        }
    }
    pub fn apply(&self, project: &mut Project) {
        for texture in &mut project.textures {
            if let Some(frame) = self.frames.get(&texture.id) {
                if project.video_textures.iter().any(|v| same_clip(v, &frame.clip)) {
                    texture.rgba8 = frame.texture.rgba8.clone();
                }
            }
        }
    }
    pub fn error(&self, texture: u64) -> Option<&str> { self.failed.get(&texture).map(|f| f.message.as_str()) }
}
impl App {
    pub(super) fn start_video_load(&mut self, path: PathBuf) {
        if self.job.is_some() { self.status = "Дождитесь загрузки или отмените её".into(); return; }
        let (material, target) = if let Some((material, node)) = self.node_texture_target.take() { (material, Target::Node(node)) }
            else if let Some((material, slot)) = self.texture_target.take() { (material, Target::Slot(slot)) } else { return; };
        let (tx, receiver) = mpsc::channel(); let cancel = Arc::new(AtomicBool::new(false)); let c = cancel.clone();
        match std::thread::Builder::new().name("video-import".into()).spawn(move || {
            let result = poseforge_io::load_video(&path, &c, |count| {
                let _ = tx.send(JobMessage::Progress(format!("Видео: подготовлено {count} кадров · Esc — отмена")));
            });
            let message = match result {
                Ok((texture, video)) => JobMessage::VideoLoaded(texture, video, material, target),
                Err(error) => JobMessage::Error(error.to_string()),
            };
            let _ = tx.send(message);
        }) {
            Ok(_) => { self.job = Some(Job { receiver, cancel, label: "Импорт видео".into() }); self.editor.playing = false; self.status = "Подготовка видео… Esc — отмена".into(); }
            Err(error) => self.status = format!("Не удалось начать импорт видео: {error}"),
        }
    }
    pub(super) fn finish_video_load(&mut self, mut texture: Texture, mut video: VideoTexture, material: u64, target: Target) {
        let cancelled = self.job.as_ref().is_none_or(|j| j.cancel.load(Ordering::Relaxed)); self.job = None;
        if cancelled { return; }
        let valid = self.editor.project.material(material).is_some_and(|m| match target {
            Target::Slot(slot) => slot < 5,
            Target::Node(node) => m.graph.as_ref().is_some_and(|g| g.nodes.iter().any(|n| n.id == node && n.kind == poseforge_core::MaterialNodeKind::Texture)),
        });
        if !valid { self.status = "Материал или узел удалён; видео не добавлено".into(); return; }
        texture.id = self.editor.project.next_id(); video.texture = texture.id;
        let id = texture.id; let duration = video.duration(); let count = video.frames();
        let result = self.editor.transact("Видео в материал", |p| {
            p.textures.push(texture); p.video_textures.push(video); p.duration = p.duration.max(duration);
            let m = p.materials.iter_mut().find(|m| m.id == material).unwrap();
            match target {
                Target::Slot(slot) => surfaces::set_texture_slot(m, slot, Some(id)),
                Target::Node(node) => m.graph.as_mut().unwrap().nodes.iter_mut().find(|n| n.id == node).unwrap().texture = Some(id),
            }
        });
        if self.result(result).is_some() {
            if let Target::Slot(slot) = target { self.surface.media_slot = Some(slot); self.surface.maps = true; }
            self.invalidate(); self.status = format!("Видео встроено: {count} кадров, {duration:.2} с · воспроизведение по времени сцены");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Material, Node, Vec3, box_mesh};
    fn scene() -> Project {
        let mut p = Project::default();
        p.materials.push(Material { id: 1, base_color_texture: Some(2), ..Default::default() });
        p.textures.push(Texture { id: 2, name: "Poster".into(), width: 1, height: 1, rgba8: vec![0,0,0,255].into() });
        p.video_textures.push(VideoTexture { texture: 2, width: 1, height: 1, fps: 24,
            offsets: vec![0,4,8], data: vec![255,216,255,217,255,216,255,217].into(), playback: Default::default() });
        p.meshes.push(box_mesh(3, "Receiver", Vec3::ONE, Some(1)));
        let mut node = Node::new(4, "Receiver"); node.mesh = Some(3); p.nodes.push(node); p
    }
    fn waiting(clip: &VideoTexture, time: f64) -> (VideoPreview, mpsc::Sender<FrameResults>, Arc<AtomicBool>) {
        let (sender, receiver) = mpsc::channel(); let cancel = Arc::new(AtomicBool::new(false));
        let pending = Pending { receiver, cancel: cancel.clone(), time,
            requests: vec![FrameRequest { clip: clip.clone(), index: clip.frame_at(time) }] };
        (VideoPreview { pending: Some(pending), last_time: time, ..Default::default() }, sender, cancel)
    }
    #[test]
    fn removing_or_hiding_video_waits_for_cancelled_decoder_without_spawning_another() {
        for remove in [true, false] {
            let p = scene(); let (mut preview, done, cancel) = waiting(&p.video_textures[0], 0.);
            let mut hidden = p.clone();
            if remove { hidden.video_textures.clear(); } else { hidden.nodes[0].visible = false; }
            preview.update(&hidden, 0.);
            assert!(cancel.load(Ordering::Relaxed)); assert!(preview.is_pending());
            // Undo while the old worker is still alive must not create a replacement.
            preview.update(&p, 0.);
            assert!(Arc::ptr_eq(&preview.pending.as_ref().unwrap().cancel, &cancel));
            done.send(vec![(p.video_textures[0].clone(),0,Ok(p.textures[0].clone()))]).unwrap();
            preview.update(&hidden, 0.);
            assert!(!preview.is_pending()); assert!(preview.frames.is_empty());
        }
    }
    #[test]
    fn disconnected_decoder_reports_error_once_for_the_same_frame() {
        let p = scene(); let (mut preview, done, _) = waiting(&p.video_textures[0], 0.); drop(done);
        for _ in 0..20 { preview.update(&p, 0.); assert!(!preview.is_pending()); }
        assert!(preview.error(2).unwrap().contains("остановлено"));
        let mut displayed = p.clone(); preview.apply(&mut displayed);
        assert_eq!(displayed.textures, p.textures);
    }
    #[test]
    fn obsolete_clip_metadata_and_cancelled_future_frames_are_not_published() {
        let p = scene(); let (mut preview, done, _) = waiting(&p.video_textures[0], 1. / 24.);
        let mut changed = p.video_textures[0].clone(); changed.playback.offset = 1. / 24.;
        done.send(vec![(p.video_textures[0].clone(),1,Ok(p.textures[0].clone()))]).unwrap();
        preview.poll(&[&changed], 1. / 24.);
        assert!(!preview.is_pending()); assert!(preview.frames.is_empty());
        let (mut preview, done, cancel) = waiting(&p.video_textures[0], 1. / 24.);
        preview.update(&p, 0.); assert!(cancel.load(Ordering::Relaxed));
        done.send(vec![(p.video_textures[0].clone(),1,Ok(p.textures[0].clone()))]).unwrap();
        preview.poll(&[&p.video_textures[0]], 0.);
        assert!(preview.frames.is_empty()); assert!(preview.failed.is_empty());
    }
    #[test]
    fn clip_metadata_change_invalidates_a_cached_frame_even_with_shared_bytes() {
        let mut p = scene(); let clip = p.video_textures[0].clone(); let mut preview = VideoPreview::default();
        let mut decoded = p.textures[0].clone(); decoded.rgba8 = vec![255,0,0,255].into();
        preview.frames.insert(2, Frame { clip: clip.clone(), index: 0, texture: decoded });
        p.video_textures[0].offsets = vec![0,8];
        assert!(p.video_textures[0].data.shares_storage(&clip.data));
        preview.apply(&mut p); assert_eq!(p.textures[0].rgba8.as_slice(), &[0,0,0,255]);
    }
}
