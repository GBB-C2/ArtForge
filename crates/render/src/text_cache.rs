//! Reuse shaped label buffers across hover, scrolling, and camera-only frames.
use super::UiText;
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};
use std::collections::HashMap;
const LIMIT: usize = 512;

pub(super) fn font_system() -> FontSystem {
    let mut fonts = FontSystem::new();
    // Cosmic Text's default generic family may be absent on the host. Pin an
    // installed UI family so semibold Cyrillic keeps the regular label's face.
    let family = [
        "Segoe UI",
        "SF Pro Text",
        "Noto Sans",
        "DejaVu Sans",
        "Liberation Sans",
        "Arial",
    ]
    .into_iter()
    .find(|name| {
        fonts
            .db()
            .faces()
            .any(|face| face.families.iter().any(|(family, _)| family == name))
    });
    if let Some(family) = family {
        fonts.db_mut().set_sans_serif_family(family);
    }
    fonts
}
#[derive(Clone, Hash, PartialEq, Eq)]
pub(super) struct Key {
    text: String,
    size: u32,
    weight: u16,
    width: u32,
}
struct Entry {
    buffer: Buffer,
    epoch: u64,
}
#[derive(Default)]
pub(super) struct TextCache {
    entries: HashMap<Key, Entry>,
    epoch: u64,
}
impl TextCache {
    pub fn prepare(
        &mut self,
        fonts: &mut FontSystem,
        texts: &[UiText],
        width: u32,
    ) -> (Vec<Key>, usize) {
        self.epoch = self.epoch.wrapping_add(1);
        let mut shaped = 0;
        let keys: Vec<_> = texts
            .iter()
            .map(|text| {
                let available = (width as f32 - text.x).max(1.);
                let key = Key {
                    text: text.text.clone(),
                    size: text.size.to_bits(),
                    weight: text.weight,
                    width: available.to_bits(),
                };
                let entry = self.entries.entry(key.clone()).or_insert_with(|| {
                    // Cosmic Text 0.12 requires an exact weight for its preferred
                    // family. Resolve absent 600 weights within that family first.
                    let family = fonts.db().family_name(&Family::SansSerif);
                    let weight = fonts
                        .db()
                        .faces()
                        .filter(|face| {
                            Attrs::new().matches(face)
                                && face.families.iter().any(|(name, _)| name == family)
                        })
                        .min_by_key(|face| {
                            (
                                face.weight.0.abs_diff(text.weight),
                                std::cmp::Reverse(face.weight.0),
                            )
                        })
                        .map_or(text.weight, |face| face.weight.0);
                    let mut buffer = Buffer::new(fonts, Metrics::new(text.size, text.size * 1.25));
                    buffer.set_size(fonts, Some(available), Some(text.size * 1.5));
                    buffer.set_text(
                        fonts,
                        &text.text,
                        Attrs::new()
                            .family(Family::SansSerif)
                            .weight(Weight(weight)),
                        Shaping::Advanced,
                    );
                    buffer.shape_until_scroll(fonts, false);
                    shaped += 1;
                    Entry { buffer, epoch: 0 }
                });
                entry.epoch = self.epoch;
                key
            })
            .collect();
        // Keep this frame's labels alive while bounding older numeric/string variants.
        if self.entries.len() > LIMIT {
            let mut old: Vec<_> = self
                .entries
                .iter()
                .filter(|(_, e)| e.epoch != self.epoch)
                .map(|(k, e)| (e.epoch, k.clone()))
                .collect();
            old.sort_by_key(|(epoch, _)| *epoch);
            for (_, key) in old.into_iter().take(self.entries.len() - LIMIT) {
                self.entries.remove(&key);
            }
        }
        (keys, shaped)
    }
    pub fn get(&self, key: &Key) -> &Buffer {
        &self.entries[key].buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn label(text: &str) -> UiText {
        UiText {
            weight: 400,
            x: 12.,
            y: 20.,
            text: text.into(),
            size: 12.,
            color: [1.; 4],
        }
    }
    #[test]
    fn semibold_cyrillic_keeps_the_installed_sans_family() {
        let mut fonts = font_system();
        let family = fonts.db().family_name(&Family::SansSerif).to_owned();
        if !fonts
            .db()
            .faces()
            .any(|f| f.families.iter().any(|(n, _)| n == &family))
        {
            return;
        }
        let mut cache = TextCache::default();
        for weight in [400, 600] {
            let mut text = label("Сцена Материалы");
            text.weight = weight;
            let (keys, _) = cache.prepare(&mut fonts, &[text], 800);
            let glyphs: Vec<_> = cache
                .get(&keys[0])
                .layout_runs()
                .flat_map(|run| run.glyphs.iter().map(|g| g.font_id))
                .collect();
            assert!(!glyphs.is_empty());
            assert!(
                glyphs.iter().all(|id| fonts
                    .db()
                    .face(*id)
                    .unwrap()
                    .families
                    .iter()
                    .any(|(name, _)| name == &family)),
                "unexpected family at weight {weight}"
            );
        }
    }
    #[test]
    fn unchanged_text_reuses_layout_and_size_width_and_content_invalidate() {
        let mut fonts = FontSystem::new();
        let mut cache = TextCache::default();
        let mut text = label("Материал");
        assert_eq!(cache.prepare(&mut fonts, &[text.clone()], 800).1, 1);
        text.y += 200.;
        text.color = [0.5; 4];
        assert_eq!(cache.prepare(&mut fonts, &[text.clone()], 800).1, 0);
        text.text = "Стекло".into();
        assert_eq!(cache.prepare(&mut fonts, &[text.clone()], 800).1, 1);
        text.size = 24.;
        assert_eq!(cache.prepare(&mut fonts, &[text.clone()], 800).1, 1);
        text.weight = 600;
        assert_eq!(cache.prepare(&mut fonts, &[text.clone()], 800).1, 1);
        assert_eq!(cache.prepare(&mut fonts, &[text.clone()], 800).1, 0);
        assert_eq!(cache.prepare(&mut fonts, &[text], 1600).1, 1);
    }
    #[test]
    fn transient_numeric_labels_have_a_bounded_cache() {
        let mut fonts = FontSystem::new();
        let mut cache = TextCache::default();
        for i in 0..700 {
            cache.prepare(
                &mut fonts,
                &[label(&i.to_string()), label("Положение")],
                800,
            );
        }
        assert!(cache.entries.len() <= LIMIT);
        assert_eq!(
            cache
                .prepare(&mut fonts, &[label("699"), label("Положение")], 800)
                .1,
            0
        );
    }
    #[test]
    fn profile_text_layout_when_requested() {
        let Some(path) = std::env::var_os("ARTFORGE_PROFILE_TEXT") else {
            return;
        };
        let mut fonts = FontSystem::new();
        let texts: Vec<_> = (0..120)
            .map(|i| label(&format!("Настройка материала {i}")))
            .collect();
        let mut cache = TextCache::default();
        cache.prepare(&mut fonts, &texts, 1100);
        let measure = |cold: bool, cache: &mut TextCache, fonts: &mut FontSystem| {
            let mut samples = vec![];
            let mut layouts = 0;
            for _ in 0..50 {
                if cold {
                    cache.entries.clear();
                }
                let start = std::time::Instant::now();
                let (keys, count) = cache.prepare(fonts, &texts, 1100);
                std::hint::black_box(keys);
                layouts += count;
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            }
            samples.sort_by(f64::total_cmp);
            (samples[25], layouts)
        };
        let cold = measure(true, &mut cache, &mut fonts);
        let warm = measure(false, &mut cache, &mut fonts);
        std::fs::write(path,format!("{{\"labels\":120,\"frames\":50,\"uncached_ms_median\":{},\"cached_ms_median\":{},\"uncached_layouts\":{},\"cached_layouts\":{}}}",cold.0,warm.0,cold.1,warm.1)).unwrap();
    }
}
