//! Read frame pixels and wait for export work without blocking in the driver.
use std::{
    fmt::Display,
    sync::{atomic::{AtomicBool, Ordering}, mpsc::{Receiver, TryRecvError}},
    time::{Duration, Instant},
};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(120);

// Readback stores premultiplied sRGB bytes. There are only 256 x 256 inputs
// to straight-alpha conversion, even for a frame with millions of fog pixels.
fn straight_alpha_table() -> &'static [[u8; 256]] {
    static TABLE: std::sync::OnceLock<Vec<[u8; 256]>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=255)
            .map(|alpha| {
                std::array::from_fn(|color| {
                    if alpha == 0 || alpha == 255 {
                        return color as u8;
                    }
                    let s = color as f32 / 255.;
                    let linear = if s <= 0.04045 {
                        s / 12.92
                    } else {
                        ((s + 0.055) / 1.055).powf(2.4)
                    };
                    let l = (linear / (alpha as f32 / 255.)).clamp(0., 1.);
                    let straight = if l <= 0.0031308 {
                        12.92 * l
                    } else {
                        1.055 * l.powf(1. / 2.4) - 0.055
                    };
                    (255. * straight).round() as u8
                })
            })
            .collect()
    })
}

pub(super) fn unpack_rgba(
    mapped: &[u8],
    row_bytes: usize,
    stride: usize,
    bgra: bool,
    straight_alpha: bool,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<u8>, String> {
    let table = straight_alpha.then(straight_alpha_table);
    let mut rgba = Vec::with_capacity((mapped.len() / stride) * row_bytes);
    for (index, row) in mapped.chunks_exact(stride).enumerate() {
        if index % 64 == 0 {
            if let Some(cancel) = cancel {
                super::volume_pass::check_cancel(cancel)?;
            }
        }
        let offset = rgba.len();
        rgba.extend_from_slice(&row[..row_bytes]);
        if bgra || straight_alpha {
            for pixel in rgba[offset..].chunks_exact_mut(4) {
                if bgra {
                    pixel.swap(0, 2);
                }
                if let Some(table) = table {
                    let colors = &table[pixel[3] as usize];
                    for c in &mut pixel[..3] {
                        *c = colors[*c as usize];
                    }
                }
            }
        }
    }
    Ok(rgba)
}

pub(super) fn save_image(
    path: &std::path::Path,
    rgba: &[u8],
    width: u32,
    height: u32,
    cancel: &AtomicBool,
) -> Result<(), String> {
    use std::io::{BufWriter, Write};
    super::volume_pass::check_cancel(cancel)?;
    let format = image::ImageFormat::from_path(path).map_err(|e| e.to_string())?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut builder = tempfile::Builder::new();
    builder.prefix(".artforge-render-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::metadata(path)
            .map(|m| m.permissions())
            .unwrap_or_else(|_| std::fs::Permissions::from_mode(0o666));
        builder.permissions(permissions);
    }
    let mut file = builder.tempfile_in(parent).map_err(|e| e.to_string())?;
    {
        let mut writer = BufWriter::new(file.as_file_mut());
        image::write_buffer_with_format(
            &mut writer,
            rgba,
            width,
            height,
            image::ColorType::Rgba8,
            format,
        )
        .map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
    }
    super::volume_pass::check_cancel(cancel)?;
    file.persist(path).map_err(|e| e.error.to_string())?;
    Ok(())
}

pub(crate) fn wait_for_gpu<E: Display>(
    receiver: &Receiver<Result<(), E>>,
    cancel: &AtomicBool,
    timeout: Duration,
    mut poll: impl FnMut(),
) -> Result<(), String> {
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Ожидание GPU отменено".into());
        }
        // Maintain::Wait would block inside the driver and never check cancel.
        poll();
        if cancel.load(Ordering::Relaxed) {
            return Err("Ожидание GPU отменено".into());
        }
        match receiver.try_recv() {
            Ok(result) => return result.map_err(|error| format!("Ошибка GPU при рендере кадра: {error}")),
            Err(TryRecvError::Disconnected) => return Err("GPU не вернул результат операции".into()),
            Err(TryRecvError::Empty) => {}
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err("Превышено время ожидания ответа GPU. Повторите экспорт или уменьшите разрешение.".into());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(2)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn padded_bgra_rows_preserve_coverage_and_use_linear_light_for_straight_alpha() {
        let rows = [
            0, 64, 96, 128, 40, 30, 20, 255, 13, 13, 13, 13,
            0, 0, 0, 0, 255, 128, 0, 1, 13, 13, 13, 13,
        ];
        let pixels = unpack_rgba(&rows, 8, 12, true, true, None).unwrap();
        assert_eq!(pixels, [133, 90, 0, 128, 20, 30, 40, 255, 0, 0, 0, 0, 0, 255, 255, 1]);
        assert!(unpack_rgba(&rows, 8, 12, false, false, Some(&AtomicBool::new(true))).is_err());
    }

    #[test]
    fn polls_until_the_mapping_completes() {
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        let mut polls = 0;
        wait_for_gpu(&rx, &AtomicBool::new(false), Duration::from_secs(5), || {
            polls += 1;
            if polls == 3 { tx.send(Ok(())).unwrap(); }
        }).unwrap();
        assert_eq!(polls, 3);
    }

    #[test]
    fn cancellation_does_not_wait_for_a_callback() {
        let (_tx, rx) = mpsc::channel::<Result<(), String>>();
        assert!(wait_for_gpu(&rx, &AtomicBool::new(true), TIMEOUT,
            || panic!("cancelled readback must not poll")).unwrap_err().contains("отменено"));
        let cancel = AtomicBool::new(false);
        assert!(wait_for_gpu(&rx, &cancel, TIMEOUT,
            || cancel.store(true, Ordering::Relaxed)).unwrap_err().contains("отменено"));
    }

    #[test]
    fn missing_callback_has_a_deadline() {
        let (_tx, rx) = mpsc::channel::<Result<(), String>>();
        let mut polls = 0;
        assert!(wait_for_gpu(&rx, &AtomicBool::new(false), Duration::ZERO,
            || polls += 1).unwrap_err().contains("время ожидания"));
        assert_eq!(polls, 1);
    }

    #[test]
    fn mapping_errors_and_disconnection_are_reported() {
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        tx.send(Err("device lost".into())).unwrap();
        assert!(wait_for_gpu(&rx, &AtomicBool::new(false), TIMEOUT, || {})
            .unwrap_err().contains("device lost"));
        drop(tx);
        assert!(wait_for_gpu(&rx, &AtomicBool::new(false), TIMEOUT, || {})
            .unwrap_err().contains("не вернул"));
    }
}
