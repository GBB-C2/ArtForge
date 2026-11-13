//! Keep GPU readback cancellable even when a mapping callback never arrives.
use std::{
    fmt::Display,
    sync::{atomic::{AtomicBool, Ordering}, mpsc::{Receiver, TryRecvError}},
    time::{Duration, Instant},
};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) fn wait_for_mapping<E: Display>(
    receiver: &Receiver<Result<(), E>>,
    cancel: &AtomicBool,
    timeout: Duration,
    mut poll: impl FnMut(),
) -> Result<(), String> {
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Чтение кадра с GPU отменено".into());
        }
        // Maintain::Wait would block inside the driver and never check cancel.
        poll();
        if cancel.load(Ordering::Relaxed) {
            return Err("Чтение кадра с GPU отменено".into());
        }
        match receiver.try_recv() {
            Ok(result) => return result.map_err(|error| format!("Не удалось прочитать кадр с GPU: {error}")),
            Err(TryRecvError::Disconnected) => return Err("GPU не вернул результат чтения кадра".into()),
            Err(TryRecvError::Empty) => {}
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err("Превышено время ожидания кадра с GPU; повторите экспорт или уменьшите разрешение".into());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(2)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn polls_until_the_mapping_completes() {
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        let mut polls = 0;
        wait_for_mapping(&rx, &AtomicBool::new(false), Duration::from_secs(5), || {
            polls += 1;
            if polls == 3 { tx.send(Ok(())).unwrap(); }
        }).unwrap();
        assert_eq!(polls, 3);
    }

    #[test]
    fn cancellation_does_not_wait_for_a_callback() {
        let (_tx, rx) = mpsc::channel::<Result<(), String>>();
        assert!(wait_for_mapping(&rx, &AtomicBool::new(true), TIMEOUT,
            || panic!("cancelled readback must not poll")).unwrap_err().contains("отменено"));
        let cancel = AtomicBool::new(false);
        assert!(wait_for_mapping(&rx, &cancel, TIMEOUT,
            || cancel.store(true, Ordering::Relaxed)).unwrap_err().contains("отменено"));
    }

    #[test]
    fn missing_callback_has_a_deadline() {
        let (_tx, rx) = mpsc::channel::<Result<(), String>>();
        let mut polls = 0;
        assert!(wait_for_mapping(&rx, &AtomicBool::new(false), Duration::ZERO,
            || polls += 1).unwrap_err().contains("время ожидания"));
        assert_eq!(polls, 1);
    }

    #[test]
    fn mapping_errors_and_disconnection_are_reported() {
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        tx.send(Err("device lost".into())).unwrap();
        assert!(wait_for_mapping(&rx, &AtomicBool::new(false), TIMEOUT, || {})
            .unwrap_err().contains("device lost"));
        drop(tx);
        assert!(wait_for_mapping(&rx, &AtomicBool::new(false), TIMEOUT, || {})
            .unwrap_err().contains("не вернул"));
    }
}
