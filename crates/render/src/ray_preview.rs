//! One cancellable CPU geometry job. GPU uploads stay on the render thread.
use super::world::{build_geometry_cancellable, RayNode, RayTriangle};
use glam::Mat4;
use poseforge_core::Project;
use std::{collections::BTreeMap, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}};

type Geometry = (Vec<RayNode>, Vec<RayTriangle>, Vec<super::emissive::Surface>);
struct Job {
    key: u64,
    cancel: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Result<Geometry, String>>,
}
impl Drop for Job {
    fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}
#[derive(Default)]
pub(super) struct RayPreview {
    job: Option<Job>,
    failed: Option<u64>,
}
impl RayPreview {
    pub fn pending(&self) -> bool { self.job.is_some() }
    pub fn cancel(&mut self) {
        self.failed = None;
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
            if !matches!(job.receiver.try_recv(), Err(mpsc::TryRecvError::Empty)) { self.job = None; }
        }
    }
    pub fn update(&mut self, key: u64, project: &Project, globals: &BTreeMap<u64, Mat4>)
        -> Result<Option<Geometry>, String> {
        if let Some(job) = &self.job {
            if job.key != key { job.cancel.store(true, Ordering::Relaxed); }
            let result = match job.receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return Ok(None),
                Err(mpsc::TryRecvError::Disconnected) => Err("Подготовка лучевых эффектов остановлена".into()),
            };
            let current = job.key == key && !job.cancel.load(Ordering::Relaxed);
            self.job = None;
            if current {
                if result.is_err() { self.failed = Some(key); }
                return result.map(Some);
            }
        }
        if self.failed == Some(key) { return Ok(None); }
        // Do not clone another scene until the previous worker has actually exited.
        let project = project.clone(); let globals = globals.clone();
        let cancel = Arc::new(AtomicBool::new(false)); let c = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new().name("ray-geometry".into()).spawn(move || {
            let result = build_geometry_cancellable(&project, &globals, false, &c).and_then(|(nodes,triangles)| {
                let emission = super::emissive::prepare(&triangles,&c)?;
                Ok((nodes,triangles,emission))
            });
            let _ = sender.send(result);
        }) {
            Ok(_) => { self.job = Some(Job { key, cancel, receiver }); Ok(None) }
            Err(error) => { self.failed = Some(key); Err(format!("Не удалось подготовить лучевые эффекты: {error}")) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn obsolete_jobs_retire_before_another_scene_is_cloned() {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut preview = RayPreview { job: Some(Job { key: 1, cancel: cancel.clone(), receiver }), failed: None };
        assert!(preview.update(2, &Project::default(), &BTreeMap::new()).unwrap().is_none());
        assert!(cancel.load(Ordering::Relaxed));
        assert_eq!(preview.job.as_ref().unwrap().key, 1);
        sender.send(Ok((vec![], vec![], vec![]))).unwrap();
        // Canceled work is never published, even when it completes successfully.
        preview.cancel(); assert!(!preview.pending());
    }
    #[test]
    fn failed_geometry_is_reported_once_instead_of_starting_an_endless_retry_loop() {
        let (sender, receiver) = mpsc::channel();
        let mut preview = RayPreview { job: Some(Job { key: 1, cancel: Arc::new(AtomicBool::new(false)), receiver }), failed: None };
        sender.send(Err("invalid geometry".into())).unwrap();
        assert!(preview.update(1, &Project::default(), &BTreeMap::new()).is_err());
        assert!(preview.update(1, &Project::default(), &BTreeMap::new()).unwrap().is_none());
        assert!(!preview.pending());
    }
    #[test]
    fn disconnected_ray_worker_can_be_retired() {
        let (sender, receiver) = mpsc::channel(); drop(sender);
        let mut preview = RayPreview { job: Some(Job { key: 1, cancel: Arc::new(AtomicBool::new(false)), receiver }), failed: None };
        assert!(preview.update(1, &Project::default(), &BTreeMap::new()).is_err());
        assert!(!preview.pending());
    }
}
