//! Closing must keep pumping window events while the recovery file is written.
//! Only a successful worker result permits exit; errors restore the open editor.
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Default)]
pub(super) struct CloseRecovery {
    receiver: Option<Receiver<Result<(), String>>>,
}

impl CloseRecovery {
    pub fn is_pending(&self) -> bool { self.receiver.is_some() }

    pub fn start(&mut self, save: impl FnOnce() -> Result<(), String> + Send + 'static) -> Result<(), String> {
        if self.is_pending() { return Ok(()); }
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new().name("close-recovery".into()).spawn(move || {
            let _ = tx.send(save());
        }).map_err(|e| e.to_string())?;
        self.receiver = Some(rx);
        Ok(())
    }

    pub fn poll(&mut self) -> Result<bool, String> {
        let Some(receiver) = &self.receiver else { return Ok(false); };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return Ok(false),
            Err(TryRecvError::Disconnected) => Err("Запись восстановления завершилась без результата.".into()),
        };
        self.receiver = None;
        result.map(|()| true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn finish(job: &mut CloseRecovery) -> Result<bool, String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = job.poll();
            if result != Ok(false) { return result; }
            assert!(Instant::now() < deadline, "worker never completed");
            std::thread::yield_now();
        }
    }

    #[test]
    fn blocked_disk_does_not_block_polling_or_start_a_second_save() {
        let mut job = CloseRecovery::default();
        let (release, wait) = mpsc::channel();
        job.start(move || { wait.recv().unwrap(); Ok(()) }).unwrap();
        assert!(job.is_pending());
        assert_eq!(job.poll(), Ok(false));
        job.start(|| panic!("duplicate close must not start another writer")).unwrap();
        release.send(()).unwrap();
        assert_eq!(finish(&mut job), Ok(true));
        assert!(!job.is_pending());
        assert_eq!(job.poll(), Ok(false));
    }

    #[test]
    fn write_error_keeps_editor_open_and_allows_retry() {
        let mut job = CloseRecovery::default();
        job.start(|| Err("disk full".into())).unwrap();
        assert_eq!(finish(&mut job), Err("disk full".into()));
        assert!(!job.is_pending());
        job.start(|| Ok(())).unwrap();
        assert_eq!(finish(&mut job), Ok(true));
    }

    #[test]
    fn worker_failure_does_not_leave_close_pending_forever() {
        let mut job = CloseRecovery::default();
        let (tx, rx) = mpsc::channel();
        job.receiver = Some(rx);
        drop(tx);
        assert!(job.poll().is_err());
        assert!(!job.is_pending());
        assert_eq!(job.poll(), Ok(false));
    }
}
