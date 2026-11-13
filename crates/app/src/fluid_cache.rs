//! Bounded restart snapshots shared by preview, timeline seeking and baking.
use poseforge_simulation::{FluidSnapshot, FluidWorld};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub(super) const CACHE_BUDGET: usize = 128 * 1024 * 1024;
#[derive(Clone, Default)]
pub(super) struct FluidCache {
    signature: Option<u64>,
    pub frames: BTreeMap<u64, Arc<FluidSnapshot>>,
    pub bytes: usize,
}
impl FluidCache {
    pub fn reset_for(&mut self, world: &FluidWorld) {
        if self.signature != Some(world.signature()) {
            self.clear();
            self.signature = Some(world.signature());
        }
    }
    pub fn clear(&mut self) {
        self.frames.clear();
        self.bytes = 0;
        self.signature = None;
    }
    fn cost(snapshot: &FluidSnapshot) -> usize {
        (snapshot.positions.len() + snapshot.velocities.len()) * std::mem::size_of::<glam::Vec3>()
            + 128
    }
    pub fn record(&mut self, world: &FluidWorld) {
        self.reset_for(world);
        if self.frames.contains_key(&world.tick()) {
            return;
        }
        let snapshot = world.snapshot();
        let cost = Self::cost(&snapshot);
        if cost > CACHE_BUDGET {
            return;
        }
        while self.bytes + cost > CACHE_BUDGET {
            // Keep the initial restart state and the most recently visited region.
            let key = self
                .frames
                .keys()
                .copied()
                .filter(|&k| k != 0)
                .max_by_key(|k| k.abs_diff(world.tick()))
                .or_else(|| self.frames.keys().next().copied());
            let Some(key) = key else { break };
            self.bytes -= Self::cost(&self.frames.remove(&key).unwrap());
        }
        self.frames.insert(world.tick(), Arc::new(snapshot));
        self.bytes += cost;
    }
    pub fn trim_to(&mut self, budget: usize) {
        while self.bytes > budget {
            let key = self.frames.keys().copied().find(|&k| k != 0);
            let Some(key) = key else { break };
            if let Some(frame) = self.frames.remove(&key) {
                self.bytes = self.bytes.saturating_sub(Self::cost(&frame));
            }
        }
    }
    pub fn restore_nearest(&self, world: &mut FluidWorld, tick: u64) -> bool {
        if self.signature != Some(world.signature()) {
            return false;
        }
        self.frames
            .range(..=tick)
            .next_back()
            .is_some_and(|(_, state)| world.restore(state).is_ok())
    }
    pub fn nearest_tick(&self, tick: u64) -> Option<u64> {
        self.frames
            .range(..=tick)
            .next_back()
            .map(|(&tick, _)| tick)
    }
}

pub(super) fn bake(
    world: &FluidWorld,
    duration: f64,
    fps: f64,
    speed: f64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> Result<FluidCache, String> {
    if !duration.is_finite()
        || !(0. ..=3600.).contains(&duration)
        || !fps.is_finite()
        || !(1. ..=240.).contains(&fps)
        || !speed.is_finite()
        || !(0.1..=2.).contains(&speed)
    {
        return Err("Некорректный диапазон запекания жидкости".into());
    }
    let frames = (duration * fps).ceil() as usize + 1;
    let cost = world
        .positions()
        .len()
        .saturating_mul(24)
        .saturating_add(128);
    if frames > 20_000 || frames.saturating_mul(cost) > CACHE_BUDGET {
        return Err(
            "Кэш превысит 128 МиБ или 20 000 кадров: сократите длительность, FPS или число частиц"
                .into(),
        );
    }
    let mut work = world.clone();
    work.reset();
    let mut cache = FluidCache::default();
    let dt = work.config().time_step as f64;
    for frame in 0..frames {
        let time = (frame as f64 / fps).min(duration) * speed;
        let tick = ((time + 1e-10) / dt).floor() as u64;
        while work.tick() < tick {
            if cancel.load(Ordering::Relaxed) {
                return Err("Запекание жидкости отменено".into());
            }
            work.step();
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Запекание жидкости отменено".into());
        }
        cache.record(&work);
        progress(frame + 1, frames);
    }
    Ok(cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> FluidWorld {
        FluidWorld::new(Default::default(), vec![glam::Vec3::new(0., 0.8, 0.)]).unwrap()
    }
    #[test]
    fn bake_rewinds_exactly_invalidates_on_inputs_and_cancels_atomically() {
        let world = world();
        let cancel = AtomicBool::new(false);
        let cache = bake(&world, 2., 24., 1., &cancel, |_, _| {}).unwrap();
        assert_eq!(cache.frames.len(), 49);
        let mut restored = world.clone();
        let tick = *cache.frames.keys().nth(12).unwrap();
        assert!(cache.restore_nearest(&mut restored, tick));
        let mut direct = world.clone();
        for _ in 0..tick {
            direct.step();
        }
        assert_eq!(restored.positions(), direct.positions());
        let mut config = restored.config().clone();
        config.viscosity += 0.01;
        restored.set_config(config).unwrap();
        assert!(!cache.restore_nearest(&mut restored, tick));
        cancel.store(true, Ordering::Relaxed);
        assert!(bake(&world, 2., 24., 1., &cancel, |_, _| {}).is_err());
        assert_eq!(world.tick(), 0);
        assert!(bake(&world, 3600., 240., 1., &AtomicBool::new(false), |_, _| {}).is_err());
    }
}

impl FluidCache {
    /// Compact little-endian restart states. Publish only after a complete write.
    pub fn save(&self, path: &std::path::Path, cancel: &AtomicBool) -> Result<(), String> {
        use std::io::Write;
        if self.frames.is_empty() {
            return Err("Сначала рассчитайте или запеките жидкость".into());
        }
        let temporary = path.with_extension(format!(
            "afcache.{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        ));
        let result = (|| -> Result<(), String> {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|e| e.to_string())?;
            let mut out = std::io::BufWriter::new(file);
            let write = |out: &mut std::io::BufWriter<std::fs::File>, bytes: &[u8]| {
                out.write_all(bytes).map_err(|e| e.to_string())
            };
            write(&mut out, b"AFCACHE1")?;
            write(&mut out, &self.signature.unwrap().to_le_bytes())?;
            write(&mut out, &(self.frames.len() as u32).to_le_bytes())?;
            let version = &self.frames.values().next().unwrap().solver_version;
            write(&mut out, &(version.len() as u16).to_le_bytes())?;
            write(&mut out, version.as_bytes())?;
            for snapshot in self.frames.values() {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Сохранение кэша отменено".into());
                }
                write(&mut out, &snapshot.tick.to_le_bytes())?;
                write(&mut out, &(snapshot.positions.len() as u32).to_le_bytes())?;
                for v in snapshot.positions.iter().chain(&snapshot.velocities) {
                    for f in v.to_array() {
                        write(&mut out, &f.to_le_bytes())?;
                    }
                }
            }
            out.flush().map_err(|e| e.to_string())?;
            out.get_ref().sync_all().map_err(|e| e.to_string())?;
            drop(out);
            if cancel.load(Ordering::Relaxed) {
                return Err("Сохранение кэша отменено".into());
            }
            std::fs::rename(&temporary, path).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        result
    }
    pub fn load(
        path: &std::path::Path,
        world: &FluidWorld,
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        use std::io::Read;
        fn bytes<const N: usize>(r: &mut impl Read) -> Result<[u8; N], String> {
            let mut a = [0; N];
            r.read_exact(&mut a).map_err(|e| e.to_string())?;
            Ok(a)
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > CACHE_BUDGET as u64 + 1024 {
            return Err("Кэш больше 128 МиБ".into());
        }
        let mut input = std::io::BufReader::new(file);
        if bytes::<8>(&mut input)? != *b"AFCACHE1" {
            return Err("Неизвестный формат кэша жидкости".into());
        }
        let signature = u64::from_le_bytes(bytes(&mut input)?);
        if signature != world.signature() {
            return Err("Кэш не соответствует настройкам и источнику жидкости".into());
        }
        let frames = u32::from_le_bytes(bytes(&mut input)?) as usize;
        if frames == 0 || frames > 20000 {
            return Err("Недопустимое число кадров кэша".into());
        }
        let len = u16::from_le_bytes(bytes(&mut input)?) as usize;
        if len > 256 {
            return Err("Некорректная версия решателя".into());
        }
        let mut version = vec![0; len];
        input.read_exact(&mut version).map_err(|e| e.to_string())?;
        let version = String::from_utf8(version).map_err(|e| e.to_string())?;
        let mut result = Self {
            signature: Some(signature),
            ..Default::default()
        };
        let mut validation = world.clone();
        for _ in 0..frames {
            if cancel.load(Ordering::Relaxed) {
                return Err("Загрузка кэша отменена".into());
            }
            let tick = u64::from_le_bytes(bytes(&mut input)?);
            let count = u32::from_le_bytes(bytes(&mut input)?) as usize;
            let cost = count.saturating_mul(24).saturating_add(128);
            if count != world.positions().len()
                || cost + result.bytes > CACHE_BUDGET
                || tick > 10_000_000
                || result.frames.contains_key(&tick)
            {
                return Err("Некорректный размер или кадр кэша".into());
            }
            let mut vectors = Vec::with_capacity(count * 2);
            for _ in 0..count * 2 {
                vectors.push(glam::Vec3::new(
                    f32::from_le_bytes(bytes(&mut input)?),
                    f32::from_le_bytes(bytes(&mut input)?),
                    f32::from_le_bytes(bytes(&mut input)?),
                ));
            }
            let velocities = vectors.split_off(count);
            let snapshot = FluidSnapshot {
                solver_version: version.clone(),
                signature,
                tick,
                positions: vectors,
                velocities,
            };
            validation.restore(&snapshot).map_err(|e| e.to_string())?;
            result.bytes += cost;
            result.frames.insert(tick, Arc::new(snapshot));
        }
        if input.read(&mut [0]).map_err(|e| e.to_string())? != 0 {
            return Err("Лишние данные в кэше".into());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    #[test]
    fn cache_file_roundtrip_rejects_wrong_signature_corruption_and_cancel_without_replacement() {
        let world =
            FluidWorld::new(Default::default(), vec![glam::Vec3::new(0., 0.8, 0.)]).unwrap();
        let cancel = AtomicBool::new(false);
        let cache = bake(&world, 0.25, 24., 1., &cancel, |_, _| {}).unwrap();
        let path = std::env::temp_dir().join(format!(
            "artforge-fluid-{}-{}.afcache",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        cache.save(&path, &cancel).unwrap();
        let original = std::fs::read(&path).unwrap();
        let restored = FluidCache::load(&path, &world, &cancel).unwrap();
        assert_eq!(restored.frames.len(), cache.frames.len());
        for (tick, frame) in &cache.frames {
            assert_eq!(restored.frames[tick].positions, frame.positions);
            assert_eq!(restored.frames[tick].velocities, frame.velocities);
        }
        assert!(cache.save(&path, &AtomicBool::new(true)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let mut wrong = world.clone();
        let mut config = wrong.config().clone();
        config.gravity.x = 1.;
        wrong.reconfigure(config).unwrap();
        assert!(FluidCache::load(&path, &wrong, &cancel).is_err());
        std::fs::write(&path, &original[..original.len() - 1]).unwrap();
        assert!(FluidCache::load(&path, &world, &cancel).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
