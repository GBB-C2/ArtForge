//! A single GPU initialization job. Polling and dropping it never wait for the
//! driver; native window creation and handle acquisition stay on the UI thread.
use std::{panic::{catch_unwind, AssertUnwindSafe}, sync::mpsc::{self, Receiver, TryRecvError}, time::Instant};

pub(super) struct Startup<T> {
    receiver: Receiver<Result<T,String>>,
    pub started: Instant,
}
impl<T: Send + 'static> Startup<T> {
    pub fn spawn(build: impl FnOnce() -> Result<T,String> + Send + 'static) -> Result<Self,String> {
        let (sender,receiver)=mpsc::channel();
        let started=Instant::now();
        let _ = std::thread::Builder::new().name("renderer-startup".into())
            .stack_size(poseforge_render::RENDERER_STACK_BYTES).spawn(move || {
            let result=catch_unwind(AssertUnwindSafe(build)).unwrap_or_else(|panic| {
                let detail=panic.downcast_ref::<String>().map(String::as_str)
                    .or_else(||panic.downcast_ref::<&str>().copied()).unwrap_or("неизвестная ошибка драйвера");
                Err(format!("Подготовка графики прервана: {detail}"))
            });
            let _=sender.send(result);
        }).map_err(|e|format!("Не удалось запустить подготовку графики: {e}"))?;
        Ok(Self{receiver,started})
    }
    pub fn poll(&self) -> Option<Result<T,String>> {
        match self.receiver.try_recv() {
            Ok(result)=>Some(result),
            Err(TryRecvError::Empty)=>None,
            Err(TryRecvError::Disconnected)=>Some(Err("Поток подготовки графики остановлен без результата".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait<T:Send+'static>(job:&Startup<T>)->Result<T,String> {
        let deadline=Instant::now()+std::time::Duration::from_secs(5);
        loop {if let Some(result)=job.poll(){return result;}
            assert!(Instant::now()<deadline,"startup test timed out");
            std::thread::sleep(std::time::Duration::from_millis(1));}
    }
    #[test]
    fn pending_startup_returns_control_and_drop_does_not_join_worker() {
        let (release,gate)=mpsc::channel();let (done,finished)=mpsc::channel();
        let job=Startup::spawn(move || {gate.recv().unwrap();done.send(()).unwrap();Ok(7u8)}).unwrap();
        assert!(job.poll().is_none());
        drop(job); // Must finish before the worker is released.
        release.send(()).unwrap();finished.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    }
    #[test]
    fn success_driver_failure_and_panic_reach_the_ui() {
        assert_eq!(wait(&Startup::spawn(||Ok(7u8)).unwrap()).unwrap(),7);
        let job=Startup::<u8>::spawn(||Err("driver unavailable".into())).unwrap();
        assert_eq!(wait(&job).unwrap_err(),"driver unavailable");
        let job=Startup::<u8>::spawn(||panic!("shader validation failed")).unwrap();
        assert!(wait(&job).unwrap_err().contains("shader validation failed"));
    }
    #[test]
    fn disconnected_worker_is_an_error_instead_of_infinite_loading() {
        let (sender,receiver)=mpsc::channel::<Result<u8,String>>();drop(sender);
        let job=Startup{receiver,started:Instant::now()};
        assert!(job.poll().unwrap().is_err());
    }
    #[test]
    fn native_renderer_can_be_returned_from_the_worker() {
        fn assert_send<T:Send>() {}
        assert_send::<poseforge_render::Renderer>();
        assert_send::<poseforge_render::RendererStartup>();
    }
}
