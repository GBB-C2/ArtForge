//! Bounded local FFmpeg import; playback uses embedded JPEGs and the image decoder.
use crate::{IoError, Result};
use poseforge_core::{Texture, VideoTexture, video::*};
use std::{fs, io::Read, path::{Path, PathBuf}, process::{Command, Stdio},
    sync::atomic::{AtomicBool, AtomicU64, Ordering}, time::{Duration, Instant}};

pub const VIDEO_FILTER: &str = "fps=24,scale=w='max(2,trunc(min(min(iw*sar,1024),1024*dar)/2)*2)':h='max(2,trunc(ow/dar/2)*2)',setsar=1";
static TEMP_ID: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Result<Self> {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!("artforge-video-{}-{}", std::process::id(), TEMP_ID.fetch_add(1, Ordering::Relaxed)));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(IoError::Invalid("Не удалось создать временную папку видео".into()))
    }
}
impl Drop for Temporary { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

pub fn is_video_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e|
        matches!(e.to_ascii_lowercase().as_str(), "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" | "ogv" | "gif"))
}
fn ffmpeg() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ARTFORGE_FFMPEG").filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        if path.is_file() { return Ok(path); }
        return Err(IoError::Invalid("ARTFORGE_FFMPEG должен указывать на ffmpeg.exe / ffmpeg".into()));
    }
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) { let local = dir.join(name); if local.is_file() { return Ok(local); } }
    for dir in std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()) {
        let path = dir.join(name); if path.is_file() { return Ok(path); }
    }
    Err(IoError::Invalid("Для импорта видео нужен FFmpeg: добавьте его в PATH или задайте ARTFORGE_FFMPEG. Уже встроенные видео воспроизводятся без FFmpeg.".into()))
}
fn frames(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "jpg") { paths.push(path); }
    }
    paths.sort(); Ok(paths)
}
fn log_tail(path: &Path) -> String {
    let mut bytes = Vec::new();
    if let Ok(file) = fs::File::open(path) { let _ = file.take(8192).read_to_end(&mut bytes); }
    String::from_utf8_lossy(&bytes).trim().chars().take(1500).collect()
}

pub fn load_video(path: &Path, cancel: &AtomicBool, mut progress: impl FnMut(usize)) -> Result<(Texture, VideoTexture)> {
    if cancel.load(Ordering::Relaxed) { return Err(IoError::Cancelled); }
    let executable = ffmpeg()?;
    let path = fs::canonicalize(path)?;
    if !is_video_path(&path) || !path.is_file() { return Err(IoError::Invalid("Выберите видео MP4, MOV, WebM, MKV, AVI, OGV или GIF".into())); }
    let temp = Temporary::new()?;
    let log = temp.0.join("decode.log");
    let output = temp.0.join("frame-%06d.jpg");
    let mut command = Command::new(executable);
    command.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-threads", "2", "-filter_threads", "1",
        "-protocol_whitelist", "file,pipe", "-i"]).arg(&path)
        .args(["-map", "0:v:0", "-an", "-sn", "-dn", "-vf", VIDEO_FILTER,
            "-frames:v", "1441", "-q:v", "3", "-threads", "2", "-f", "image2"])
        .arg(&output).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(fs::File::create(&log)?));
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| IoError::Invalid(format!("Не удалось запустить FFmpeg: {e}")))?;
    let start = Instant::now();
    let outcome = (|| {
        loop {
            if cancel.load(Ordering::Relaxed) { return Err(IoError::Cancelled); }
            if start.elapsed() > Duration::from_secs(180) { return Err(IoError::Limit("Импорт видео превысил 3 минуты; уменьшите длительность или разрешение".into())); }
            let paths = frames(&temp.0)?;
            let size: u64 = paths.iter().map(|p| fs::metadata(p).map_or(0, |m| m.len())).sum();
            if paths.len() > MAX_VIDEO_FRAMES { return Err(IoError::Limit("Видео длиннее 60 секунд; импортируйте короткий фрагмент".into())); }
            if size > MAX_VIDEO_BYTES as u64 { return Err(IoError::Limit("Сжатые кадры видео превышают 64 МиБ; уменьшите длительность".into())); }
            progress(paths.len());
            if let Some(status) = child.try_wait()? {
                if !status.success() { return Err(IoError::Invalid(format!("Не удалось прочитать видео: {}", log_tail(&log)))); }
                return frames(&temp.0);
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    })();
    if outcome.is_err() { let _ = child.kill(); let _ = child.wait(); }
    let paths = outcome?;
    if paths.is_empty() { return Err(IoError::Invalid("В файле не найдено видеокадров".into())); }
    let mut data = Vec::new(); let mut offsets = vec![0];
    for path in &paths {
        if cancel.load(Ordering::Relaxed) { return Err(IoError::Cancelled); }
        let length = fs::metadata(path)?.len() as usize;
        if data.len().saturating_add(length) > MAX_VIDEO_BYTES { return Err(IoError::Limit("Видео превышает 64 МиБ".into())); }
        fs::File::open(path)?.read_to_end(&mut data)?; offsets.push(data.len() as u32);
    }
    let mut poster = crate::texture::decode_texture(&data[..offsets[1] as usize], "video.jpg", 4 * MAX_VIDEO_EDGE as u64 * MAX_VIDEO_EDGE as u64)?;
    poster.name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let video = VideoTexture { texture: 0, width: poster.width, height: poster.height, fps: VIDEO_FPS,
        offsets, data: data.into(), playback: Default::default() };
    video.validate().map_err(|e| IoError::Invalid(e.to_string()))?;
    Ok((poster, video))
}

pub fn decode_video_frame(video: &VideoTexture, index: usize) -> Result<Texture> {
    let data = video.frame(index).ok_or_else(|| IoError::Invalid("Отсутствует кадр видео".into()))?;
    let mut texture = crate::texture::decode_texture(data, "video.jpg", 4 * MAX_VIDEO_EDGE as u64 * MAX_VIDEO_EDGE as u64)?;
    if texture.width != video.width || texture.height != video.height { return Err(IoError::Invalid("Размер кадра видео не совпадает с клипом".into())); }
    texture.id = video.texture; Ok(texture)
}

pub fn active_video_textures(project: &poseforge_core::Project, export: bool) -> Vec<&VideoTexture> {
    use std::collections::BTreeSet;
    if project.video_textures.is_empty() { return vec![]; }
    let visible = project.visible_node_ids(export);
    let mut materials = BTreeSet::new();
    for node in project.nodes.iter().filter(|n| visible.contains(&n.id)) {
        if let Some(mesh) = node.mesh.and_then(|id| project.mesh(id)) { materials.extend(mesh.assigned_materials()); }
        materials.extend(node.material_layers.iter().filter(|l| l.visible).map(|l| l.material));
    }
    let mut textures = BTreeSet::new();
    for material in project.materials.iter().filter(|m| m.visible && materials.contains(&m.id)) {
        textures.extend([material.base_color_texture, material.metallic_roughness_texture, material.normal_texture,
            material.occlusion_texture, material.emissive_texture].into_iter().flatten());
        if let Some(graph) = material.graph.as_ref().filter(|g| g.enabled) { textures.extend(graph.nodes.iter().filter_map(|n| n.texture)); }
    }
    project.video_textures.iter().filter(|v| textures.contains(&v.texture)).collect()
}

/// Used by deterministic PNG/sequence export. The cache owns at most one frame per active clip.
#[derive(Default)]
pub struct VideoFrameCache(std::collections::BTreeMap<u64, (u64, usize, Texture)>);
impl VideoFrameCache {
    pub fn apply(&mut self, project: &mut poseforge_core::Project, time: f64, export: bool, cancel: &AtomicBool) -> Result<()> {
        let clips: Vec<_> = active_video_textures(project, export).into_iter().cloned().collect();
        self.0.retain(|id, _| clips.iter().any(|v| v.texture == *id));
        for clip in clips {
            if cancel.load(Ordering::Relaxed) { return Err(IoError::Cancelled); }
            let frame = clip.frame_at(time); let revision = clip.data.revision();
            if self.0.get(&clip.texture).is_none_or(|(r, i, _)| *r != revision || *i != frame) {
                self.0.insert(clip.texture, (revision, frame, decode_video_frame(&clip, frame)?));
            }
            let decoded = &self.0[&clip.texture].2;
            if let Some(texture) = project.textures.iter_mut().find(|t| t.id == clip.texture) { texture.rgba8 = decoded.rgba8.clone(); }
        }
        Ok(())
    }
}
