//! Blender files are evaluated by a separately installed background process.
//! The process exports a temporary GLB; the normal validated glTF importer then
//! prepares a detached project. We never save over the source .blend file.
use crate::{import_gltf_with_progress, ImportResult, IoError, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const EXPORT_SCRIPT: &str = include_str!("../scripts/export_blend.py");
const MAX_LOG_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REPORT_BYTES: u64 = 1024 * 1024;
const MAX_BLEND_BYTES: u64 = crate::import_limits::MAX_MODEL_BYTES;
const MAX_CONVERSION_TIME: Duration = Duration::from_secs(600);
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Geometry, materials, textures and deforming bones are always requested.
/// Baking source animation is optional because it evaluates the frame range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlendImportOptions {
    pub import_animations: bool,
}

/// Explicit paths override environment/PATH discovery. Invalid explicit paths
/// return an error rather than silently running another Blender installation.
pub fn locate_blender(configured: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = configured.filter(|path| !path.as_os_str().is_empty()) {
        return executable_at(path).ok_or_else(|| {
            IoError::Invalid(format!(
                "Blender не найден по указанному пути: {}",
                path.display()
            ))
        });
    }
    if let Some(path) = std::env::var_os("POSEFORGE_BLENDER").filter(|s| !s.is_empty()) {
        return executable_at(Path::new(&path)).ok_or_else(|| {
            IoError::Invalid(
                "POSEFORGE_BLENDER должен указывать на исполняемый файл Blender".into(),
            )
        });
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            if let Some(found) = executable_at(&directory.join(executable_name())) {
                return Ok(found);
            }
        }
    }
    // These locations are checked only on the relevant platform. No shell,
    // registry command or third-party application is launched during discovery.
    #[cfg(target_os = "windows")]
    for variable in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(variable) {
            let root = PathBuf::from(base).join("Blender Foundation");
            let mut installations: Vec<_> = fs::read_dir(&root)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(std::result::Result::ok)
                .map(|entry| entry.path())
                .collect();
            installations.sort_by_key(|path| version_parts(path));
            for directory in installations.into_iter().rev() {
                if let Some(found) = executable_at(&directory) {
                    return Ok(found);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(found) = executable_at(Path::new("/Applications/Blender.app")) {
        return Ok(found);
    }
    Err(IoError::Invalid(
        "Для .blend нужен установленный Blender 3.6 или новее: PoseForge запускает его в фоне без окна. Укажите путь в панели импорта (или POSEFORGE_BLENDER). GLB, glTF и OBJ импортируются без Blender.".into(),
    ))
}

fn executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "blender.exe"
    } else {
        "blender"
    }
}

fn executable_at(path: &Path) -> Option<PathBuf> {
    let candidate = if path.is_dir() {
        let direct = path.join(executable_name());
        if direct.is_file() {
            direct
        } else {
            path.join("Contents/MacOS/blender")
        }
    } else {
        path.to_path_buf()
    };
    if !candidate.is_file() {
        return None;
    }
    // Windows batch files can trigger implicit cmd.exe handling. Accept only
    // native executables so untrusted source paths always remain argv values.
    #[cfg(target_os = "windows")]
    if candidate
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case("exe"))
    {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(&candidate).ok()?.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    fs::canonicalize(candidate).ok()
}

#[cfg(target_os = "windows")]
fn version_parts(path: &Path) -> Vec<u32> {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// Import a model without source animation clips. See `import_blend_with_options`
/// to opt into animation baking and for the progress callback contract.
pub fn import_blend_with_progress(
    path: impl AsRef<Path>,
    configured: Option<&Path>,
    cancelled: &AtomicBool,
    progress: impl FnMut(f32, &str),
) -> Result<ImportResult> {
    import_blend_with_options(
        path,
        configured,
        BlendImportOptions::default(),
        cancelled,
        progress,
    )
}

/// Run on a worker thread. A negative progress value means the current Blender
/// phase has no measurable fraction: display its label and elapsed time only.
/// Values in 0..=1 describe the subsequent GLB import. Cancellation kills and
/// reaps Blender before removing temporary resources; scene mutations belong
/// to the caller.
pub fn import_blend_with_options(
    path: impl AsRef<Path>,
    configured: Option<&Path>,
    options: BlendImportOptions,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(f32, &str),
) -> Result<ImportResult> {
    check_cancel(cancelled)?;
    let path = path.as_ref();
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case("blend"))
    {
        return Err(IoError::Invalid(
            "Выберите файл с расширением .blend".into(),
        ));
    }
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_BLEND_BYTES {
        return Err(IoError::Invalid(
            "Нужен обычный .blend-файл размером до 2 ГиБ".into(),
        ));
    }
    let input = fs::canonicalize(path)?;
    let blender = locate_blender(configured)?;
    let temp = TempWorkspace::new()?;
    let script = temp.path.join("export_blend.py");
    let output = temp.path.join("scene.glb");
    let report_path = temp.path.join("report.json");
    let log_path = temp.path.join("blender.log");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&script)?;
    file.write_all(EXPORT_SCRIPT.as_bytes())?;
    drop(file);
    let log = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&log_path)?;
    let mut phases = BlenderProgress::new(File::open(&log_path)?);
    progress(-1.0, "Запуск Blender · 0 с");
    check_cancel(cancelled)?;
    let mut command = Command::new(blender);
    command
        .args([
            "--background",
            "--factory-startup",
            "--disable-autoexec",
            "--python-exit-code",
            "1",
            "--python",
        ])
        .arg(&script)
        .arg("--")
        .arg(&input)
        .arg(&output)
        .arg(&report_path)
        .current_dir(&temp.path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    if options.import_animations {
        command.arg("--animations");
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = command
        .spawn()
        .map_err(|error| IoError::Invalid(format!("Не удалось запустить Blender: {error}")))?;
    let mut process = RunningChild(Some(child));
    let started = Instant::now();
    let mut last_progress = Instant::now();
    let status = loop {
        check_cancel(cancelled)?;
        if started.elapsed() > MAX_CONVERSION_TIME {
            return Err(IoError::Invalid("Blender не завершил импорт за 10 минут; процесс остановлен. Упростите сцену или экспортируйте GLB вручную.".into()));
        }
        if fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0) > MAX_LOG_BYTES {
            return Err(IoError::Invalid(
                "Blender создал слишком большой журнал ошибок; импорт остановлен.".into(),
            ));
        }
        let phase_changed = phases.poll()?;
        if phase_changed || last_progress.elapsed() >= Duration::from_secs(1) {
            progress(
                -1.0,
                &format!(
                    "{} · {} с",
                    phases.stage.label(),
                    started.elapsed().as_secs()
                ),
            );
            last_progress = Instant::now();
        }
        if let Some(status) = process.0.as_mut().unwrap().try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(50));
    };
    let _ = process.0.take(); // try_wait has already reaped the terminated process.
    check_cancel(cancelled)?;
    if !status.success() {
        return Err(IoError::Invalid(format!(
            "Blender не смог преобразовать файл. {}",
            log_tail(&log_path)
        )));
    }
    if !output.is_file() {
        return Err(IoError::Invalid(format!(
            "Blender завершился без GLB-файла. {}",
            log_tail(&log_path)
        )));
    }
    let report_size = fs::metadata(&report_path)?.len();
    if report_size > MAX_REPORT_BYTES {
        return Err(IoError::Invalid("Слишком большой отчёт Blender".into()));
    }
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let profile = if options.import_animations {
        "ANIMATION"
    } else {
        "MODEL"
    };
    if report["import_profile"].as_str() != Some(profile)
        || report["animations_exported"].as_bool() != Some(options.import_animations)
    {
        return Err(IoError::Invalid(
            "Отчёт Blender не соответствует выбранному режиму импорта.".into(),
        ));
    }
    progress(0.72, "Проверка временного GLB");
    let mut imported = import_gltf_with_progress(&output, cancelled, |value, message| {
        progress(0.72 + value * 0.28, message);
    })?;
    if !options.import_animations && !imported.project.clips.is_empty() {
        return Err(IoError::Invalid(
            "Blender вернул анимационные клипы при выключенном импорте анимации.".into(),
        ));
    }
    if let Some(fps) = report["fps"]
        .as_f64()
        .filter(|fps| fps.is_finite() && *fps > 0.)
    {
        imported.project.fps = (fps.round() as u32).clamp(1, 240);
        if (fps - imported.project.fps as f64).abs() > 0.001 {
            imported.report.warnings.push(format!(
                "Сетка кадров округлена с {fps:.3} до {} FPS.",
                imported.project.fps
            ));
        }
    }
    imported.project.title = input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    if let Some(warnings) = report["warnings"].as_array() {
        for warning in warnings
            .iter()
            .filter_map(serde_json::Value::as_str)
            .take(100)
        {
            imported
                .report
                .warnings
                .push(warning.chars().take(1000).collect());
        }
    }
    let content = if options.import_animations {
        "Перенесены модель, поддерживаемые материалы/текстуры, скелет и запечённый диапазон анимации активной сцены."
    } else {
        "Перенесены модель, поддерживаемые материалы/текстуры и скелет. Готовые анимационные клипы не запекались; скелет доступен для создания своей анимации."
    };
    imported.report.warnings.push(format!(
        "Фоновый импорт .blend через Blender {}. {content} Исходный .blend не изменён. Управляющие constraints/drivers не становятся контроллерами PoseForge.",
        report["blender_version"].as_str().unwrap_or("неизвестной версии")
    ));
    if let (Some(open), Some(export)) = (
        report["timings_seconds"]["open"].as_f64(),
        report["timings_seconds"]["export"].as_f64(),
    ) {
        if open.is_finite() && open >= 0. && export.is_finite() && export >= 0. {
            imported.report.warnings.push(format!(
                "Время Blender: открытие {open:.2} с; экспорт {export:.2} с."
            ));
        }
    }
    imported
        .project
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    check_cancel(cancelled)?;
    Ok(imported)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BlenderStage {
    Starting,
    Open,
    Prepare,
    ExportModel,
    ExportAnimation,
    Done,
}

impl BlenderStage {
    fn from_line(line: &[u8]) -> Option<Self> {
        match line.strip_suffix(b"\r").unwrap_or(line) {
            b"POSEFORGE_STAGE:OPEN" => Some(Self::Open),
            b"POSEFORGE_STAGE:PREPARE" => Some(Self::Prepare),
            b"POSEFORGE_STAGE:EXPORT_MODEL" => Some(Self::ExportModel),
            b"POSEFORGE_STAGE:EXPORT_ANIMATION" => Some(Self::ExportAnimation),
            b"POSEFORGE_STAGE:DONE" => Some(Self::Done),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Starting => "Запуск Blender",
            Self::Open => "Открытие .blend",
            Self::Prepare => "Подготовка модели в Blender",
            Self::ExportModel => "Экспорт модели, текстур и скелета",
            Self::ExportAnimation => "Экспорт модели и запекание анимации",
            Self::Done => "Завершение Blender",
        }
    }
}

/// Read appended output incrementally. Only exact private protocol lines affect
/// the UI; long lines, paths and Blender's arbitrary log text are never echoed.
/// Both a partial line and each polling read are bounded.
struct BlenderProgress {
    file: File,
    line: Vec<u8>,
    overlong: bool,
    stage: BlenderStage,
}

impl BlenderProgress {
    fn new(file: File) -> Self {
        Self {
            file,
            line: Vec::with_capacity(64),
            overlong: false,
            stage: BlenderStage::Starting,
        }
    }

    fn poll(&mut self) -> std::io::Result<bool> {
        let previous = self.stage;
        let mut bytes = [0; 4096];
        for _ in 0..16 {
            let count = self.file.read(&mut bytes)?;
            if count == 0 {
                break;
            }
            self.accept(&bytes[..count]);
        }
        Ok(self.stage != previous)
    }

    fn accept(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                if !self.overlong {
                    if let Some(next) = BlenderStage::from_line(&self.line) {
                        self.stage = self.stage.max(next);
                    }
                }
                self.line.clear();
                self.overlong = false;
            } else if !self.overlong {
                if self.line.len() < 64 {
                    self.line.push(byte);
                } else {
                    self.line.clear();
                    self.overlong = true;
                }
            }
        }
    }
}

fn check_cancel(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(IoError::Cancelled)
    } else {
        Ok(())
    }
}

fn log_tail(path: &Path) -> String {
    let text = (|| -> std::io::Result<String> {
        let mut file = File::open(path)?;
        let start = file.metadata()?.len().saturating_sub(4096);
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::new();
        file.take(4096).read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes)
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect())
    })();
    text.unwrap_or_else(|_| "Журнал недоступен".into())
}

struct RunningChild(Option<Child>);
impl Drop for RunningChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct TempWorkspace {
    path: PathBuf,
}
impl TempWorkspace {
    fn new() -> Result<Self> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..32 {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "poseforge-blend-{}-{time}-{serial}",
                std::process::id()
            ));
            #[cfg(unix)]
            let created = {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new().mode(0o700).create(&path)
            };
            #[cfg(not(unix))]
            let created = fs::create_dir(&path);
            match created {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(IoError::Invalid(
            "Не удалось создать временную папку импорта Blender".into(),
        ))
    }
}
impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reads_appended_partial_protocol_lines_without_repeating_them() {
        let mut log = tempfile::NamedTempFile::new().unwrap();
        let mut progress = BlenderProgress::new(File::open(log.path()).unwrap());
        log.write_all(b"Blender log\nPOSEFORGE_STAGE:OP").unwrap();
        log.flush().unwrap();
        assert!(!progress.poll().unwrap());
        assert_eq!(progress.stage, BlenderStage::Starting);
        log.write_all(b"EN\r\nPOSEFORGE_STAGE:PREPARE\n").unwrap();
        log.flush().unwrap();
        assert!(progress.poll().unwrap());
        assert_eq!(progress.stage, BlenderStage::Prepare);
        assert!(!progress.poll().unwrap());
        log.write_all(b"POSEFORGE_STAGE:EXPORT_MODEL\nPOSEFORGE_STAGE:DONE\n")
            .unwrap();
        log.flush().unwrap();
        assert!(progress.poll().unwrap());
        assert_eq!(progress.stage, BlenderStage::Done);
    }

    #[test]
    fn progress_ignores_unknown_overlong_and_backwards_markers() {
        let mut progress = BlenderProgress::new(tempfile::tempfile().unwrap());
        progress.accept(b"POSEFORGE_STAGE:UNKNOWN\nnot POSEFORGE_STAGE:OPEN\n");
        assert_eq!(progress.stage, BlenderStage::Starting);
        progress.accept(&[b'x'; 4096]);
        assert!(progress.line.len() <= 64);
        progress.accept(b"POSEFORGE_STAGE:DONE\nPOSEFORGE_STAGE:EXPORT_ANIMATION\n");
        assert_eq!(progress.stage, BlenderStage::ExportAnimation);
        progress.accept(b"POSEFORGE_STAGE:OPEN\nPOSEFORGE_STAGE:EXPORT_MODEL\n");
        assert_eq!(progress.stage, BlenderStage::ExportAnimation);
        progress.accept(b"POSEFORGE_STAGE:DONE\n");
        assert_eq!(progress.stage, BlenderStage::Done);
    }
}
