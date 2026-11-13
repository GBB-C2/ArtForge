//! Portable, JPEG-compressed video frames. Playback never depends on source paths.
use crate::{CoreError, SharedBuffer};
use serde::{Deserialize, Serialize};

pub const VIDEO_FPS: u32 = 24;
pub const MAX_VIDEO_FRAMES: usize = 60 * VIDEO_FPS as usize;
pub const MAX_VIDEO_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_VIDEO_TOTAL_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_VIDEO_EDGE: u32 = 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VideoPlayback {
    pub enabled: bool,
    pub looped: bool,
    pub start: f64,
    pub offset: f64,
    pub speed: f64,
}
impl Default for VideoPlayback {
    fn default() -> Self { Self { enabled: true, looped: true, start: 0., offset: 0., speed: 1. } }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VideoTexture {
    /// Existing texture ID; its RGBA image is the portable poster/fallback.
    pub texture: u64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Frame boundaries, including the final end offset.
    pub offsets: Vec<u32>,
    #[serde(with = "encoded")]
    pub data: SharedBuffer<u8>,
    #[serde(default)]
    pub playback: VideoPlayback,
}
impl VideoTexture {
    pub fn frames(&self) -> usize { self.offsets.len().saturating_sub(1) }
    pub fn duration(&self) -> f64 { self.frames() as f64 / self.fps.max(1) as f64 }
    pub fn frame(&self, index: usize) -> Option<&[u8]> {
        let start = *self.offsets.get(index)? as usize;
        let end = *self.offsets.get(index.checked_add(1)?)? as usize;
        self.data.get(start..end)
    }
    pub fn frame_at(&self, scene_time: f64) -> usize {
        if self.frames() == 0 || !scene_time.is_finite() { return 0; }
        let p = self.playback;
        let elapsed = if p.enabled { (scene_time - p.start).max(0.) * p.speed } else { 0. };
        let time = elapsed + p.offset;
        let duration = self.duration();
        let time = if p.looped { time.rem_euclid(duration) } else { time.clamp(0., duration) };
        ((time * self.fps as f64 + 1e-8).floor() as usize).min(self.frames() - 1)
    }
    pub fn validate(&self) -> Result<(), CoreError> {
        let p = self.playback;
        let finite = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if self.width == 0 || self.height == 0 || self.width > MAX_VIDEO_EDGE || self.height > MAX_VIDEO_EDGE
            || self.fps != VIDEO_FPS || !(1..=MAX_VIDEO_FRAMES).contains(&self.frames())
            || self.data.len() > MAX_VIDEO_BYTES || self.offsets.first() != Some(&0)
            || self.offsets.last().copied().map(|n| n as usize) != Some(self.data.len())
            || self.offsets.windows(2).any(|p| p[0] >= p[1])
            || !finite(p.start, 0., 86400.) || !finite(p.offset, 0., self.duration()) || !finite(p.speed, 0.1, 4.) {
            return Err(CoreError::Invalid("Некорректное видео: до 60 с, 1024 px и 64 МиБ; скорость 0,1–4".into()));
        }
        for i in 0..self.frames() {
            let frame = self.frame(i).ok_or_else(|| CoreError::Invalid("Повреждены границы видеокадра".into()))?;
            if !frame.starts_with(&[0xff, 0xd8]) || !frame.ends_with(&[0xff, 0xd9]) {
                return Err(CoreError::Invalid("Повреждён видеокадр JPEG".into()));
            }
        }
        Ok(())
    }
}
mod encoded {
    use super::*;
    use serde::{Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(data: &SharedBuffer<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::encode(&data[..]))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<SharedBuffer<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() > MAX_VIDEO_BYTES.div_ceil(3) * 4 { return Err(D::Error::custom("Встроенное видео превышает 64 МиБ")); }
        let bytes = base64::decode(text).map_err(D::Error::custom)?;
        if bytes.len() > MAX_VIDEO_BYTES { return Err(D::Error::custom("Встроенное видео превышает 64 МиБ")); }
        Ok(bytes.into())
    }
}
