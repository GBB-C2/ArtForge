//! Headless commands share the document evaluator and GPU renderer with the editor.
use poseforge_core::{BoneConfidence, BoneRole, NodeId, Project};
use poseforge_render::{Renderer, View};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const HELP: &str = concat!(
    "ArtForge Studio ",
    env!("CARGO_PKG_VERSION"),
    r#" — создание 3D artwork на Rust

Без аргументов: открыть рабочую область с новым проектом.
Один путь без ключей: открыть проект, GLB/glTF или OBJ в редакторе.

  --help
  --validate FILE
  --check-shaders
  --import-report MODEL [--animations]
  --rig-report RIG.txt [INPUT|demo]
  --detect-skeleton INPUT|demo [ROOT_ID]
  --convert-blend INPUT.blend OUTPUT.artforge [--animations]
  --demo OUTPUT.artforge
  --render INPUT|demo OUTPUT.png WIDTH HEIGHT [SECONDS] [--transparent]
  --sequence INPUT|demo DIRECTORY FIRST LAST WIDTH HEIGHT [--transparent] [--resume]
  --self-check [DIRECTORY]

INPUT: проект .artforge/.poseforge, glTF 2.0 (.glb/.gltf), OBJ/MTL или Blender (.blend).
--check-shaders проверяет встроенные шейдеры на CPU, без окна и видеокарты.
--import-report импортирует модель в памяти и выводит JSON: ресурсы и предупреждения.
.blend требует отдельно установленный Blender 3.6 или новее с экспортёром glTF.
POSEFORGE_BLENDER задаёт путь к Blender; иначе используется поиск установки.
Обычный импорт .blend переносит модель, поддерживаемые материалы/текстуры и скелет.
--animations в --convert-blend или --import-report дополнительно запекает готовую
анимацию активной сцены; этот флаг применим только к .blend. Управляющие
constraints/drivers не становятся контроллерами ArtForge Studio.
--convert-blend создаёт новый проект и отказывается заменять существующий файл.
--detect-skeleton выводит JSON с ролями, ID, всеми кандидатами и предупреждениями.
ROOT_ID ограничивает поиск одним персонажем; без него автоматический выбор возможен
только при однозначном скелете. Это частичное распознавание имён и иерархии,
а не гарантия анатомии, настройки IK или совпадения локальных осей. Сцена не меняется.
--rig-report проверяет текстовый список Armature/Bones и выводит JSON в stdout.
Без INPUT выводятся имя рига, число костей и корни; с INPUT — также все совпадения,
отсутствующие/неоднозначные имена и различия иерархии. Файлы не изменяются.
Это проверка метаданных: TXT не содержит геометрии или трансформаций костей.
FIRST и LAST — целые номера кадров включительно, время = номер / FPS проекта.
Для рендера выбирается первый клип, если он есть; иначе — статическая поза.
--resume продолжает только последовательность с теми же входными данными.
Для отмены последовательности создайте DIRECTORY/CANCEL; готовые кадры сохранятся.
Удалите CANCEL перед продолжением. Рендер требует доступного GPU/Vulkan/DirectX.
Динамика частиц не экспортируется; снимок поверхности в проекте выводится как обычный меш.
"#
);

/// `args` excludes the executable name. Returns false only for GUI startup.
pub fn run(args: &[String]) -> Result<bool, String> {
    if args.is_empty() || (args.len() == 1 && !args[0].starts_with('-')) { return Ok(false); }
    // GUI dispatch stays on the native main thread. CLI GPU commands need the
    // same shader-compilation stack reservation as window/export workers.
    let args = args.to_vec();
    std::thread::Builder::new().name("artforge-cli".into())
        .stack_size(poseforge_render::RENDERER_STACK_BYTES)
        .spawn(move || run_command(&args))
        .map_err(|e| format!("Не удалось запустить команду: {e}"))?
        .join().unwrap_or_else(|_| Err("Команда остановлена; см. сообщение об ошибке выше".into()))
}
fn run_command(args: &[String]) -> Result<bool, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(false);
    };
    if args.len() == 1 && !command.starts_with('-') {
        return Ok(false);
    }
    match command {
        "--help" | "-h" => println!("{HELP}"),
        "--check-shaders" => {
            exact_len(args,1)?;
            poseforge_render::validate_shaders()?;
            println!("Шейдеры сцены, FX, интерфейса и частиц корректны. GPU не использовался.");
        }
        "--import-report" => {
            let positional = blend_command_arguments(args, 2)?;
            let path = Path::new(positional[1]);
            let animations = args.iter().any(|arg| arg == "--animations");
            let imported = if path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("blend"))
            {
                load_blend_with_options(
                    path,
                    poseforge_io::BlendImportOptions {
                        import_animations: animations,
                    },
                )?
            } else {
                if animations {
                    return Err("--animations применяется только к файлам .blend".into());
                }
                poseforge_io::import_model(path).map_err(err)?
            };
            imported.project.validate().map_err(err)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schema": 1,
                    "kind": "model_import",
                    "project_schema": imported.project.version,
                    "project": imported.project.title,
                    "nodes": imported.project.nodes.len(),
                    "lights": imported.project.lights.len(),
                    "report": imported.report,
                    "source_modified": false
                }))
                .map_err(err)?
            );
        }
        "--validate" => {
            exact_len(args, 2)?;
            let project = load_input(&args[1])?;
            project.validate().map_err(err)?;
            println!(
                "Проект корректен: {}\nОбъекты: {}; меши: {}; суставы: {}; клипы: {}; FPS: {}",
                project.title,
                project.nodes.len(),
                project.meshes.len(),
                project.nodes.iter().filter(|n| n.is_joint).count(),
                project.clips.len(),
                project.fps
            );
        }
        "--rig-report" => {
            if !(2..=3).contains(&args.len()) {
                return Err("Нужно: --rig-report RIG.txt [INPUT|demo]".into());
            }
            let manifest = read_rig_manifest(Path::new(&args[1]))?;
            let project = args.get(2).map(|path| load_input(path)).transpose()?;
            let report = rig_report_value(&manifest, project.as_ref());
            println!("{}", serde_json::to_string_pretty(&report).map_err(err)?);
        }
        "--detect-skeleton" => {
            if !(2..=3).contains(&args.len()) {
                return Err("Нужно: --detect-skeleton INPUT|demo [ROOT_ID]".into());
            }
            let root = args
                .get(2)
                .map(|value| number::<NodeId>(value, "ROOT_ID"))
                .transpose()?;
            let project = load_input(&args[1])?;
            project.validate().map_err(err)?;
            if let Some(id) = root {
                if project.node(id).is_none() {
                    return Err(format!("ROOT_ID {id} отсутствует в сцене"));
                }
            }
            let report = skeleton_report_value(&project, root);
            println!("{}", serde_json::to_string_pretty(&report).map_err(err)?);
        }
        "--convert-blend" => {
            let positional = blend_command_arguments(args, 3)?;
            let input = Path::new(positional[1]);
            let output = Path::new(positional[2]);
            check_blend_conversion_paths(input, output)?;
            let options = poseforge_io::BlendImportOptions {
                import_animations: args.iter().any(|arg| arg == "--animations"),
            };
            let imported = load_blend_with_options(input, options)?;
            save_new_project(&imported.project, output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schema": 1,
                    "kind": "blend_conversion",
                    "output": output.to_string_lossy(),
                    "project_schema": imported.project.version,
                    "project": imported.project.title,
                    "nodes": imported.project.nodes.len(),
                    "import": imported.report,
                    "animations_requested": options.import_animations,
                    "source_modified": false
                }))
                .map_err(err)?
            );
        }
        "--demo" => {
            exact_len(args, 2)?;
            poseforge_io::save_project(&Project::demo(), &args[1]).map_err(err)?;
            println!("Демонстрационный проект записан: {}", args[1]);
        }
        "--render" => {
            let positional = arguments_without_flags(args, &["--transparent"])?;
            if !(5..=6).contains(&positional.len()) {
                return Err(
                    "Нужно: --render INPUT OUTPUT.png WIDTH HEIGHT [SECONDS] [--transparent]"
                        .into(),
                );
            }
            let project = load_input(positional[1])?;
            let width = dimension(positional[3])?;
            let height = dimension(positional[4])?;
            let time = if positional.len() == 6 {
                number::<f64>(positional[5], "время")?
            } else {
                0.
            };
            if !time.is_finite() || time < 0. {
                return Err("Время должно быть конечным и неотрицательным".into());
            }
            let output = Path::new(positional[2]);
            if output
                .extension()
                .and_then(|x| x.to_str())
                .is_none_or(|x| !x.eq_ignore_ascii_case("png"))
            {
                return Err("Формат вывода — PNG; укажите расширение .png".into());
            }
            create_parent(output)?;
            let mut renderer = pollster::block_on(Renderer::headless(width, height))?;
            eprintln!("Адаптер: {}", renderer.adapter_name);
            render_frame(
                &mut renderer,
                &project,
                time,
                output,
                [width, height],
                args.iter().any(|a| a == "--transparent"),
                &mut crate::app::ExportDomains::default(),
                &mut poseforge_io::video::VideoFrameCache::default(),
            )?;
            println!(
                "PNG записан: {} ({} × {}, {:.6} с)",
                output.display(),
                width,
                height,
                time
            );
        }
        "--sequence" => sequence(args)?,
        "--self-check" => {
            if args.len() > 2 {
                return Err("Нужно: --self-check [DIRECTORY]".into());
            }
            self_check(
                args.get(1)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("output/self-check")),
            )?;
        }
        _ => {
            return Err(format!(
                "Неизвестный аргумент {command}. Используйте --help."
            ))
        }
    }
    Ok(true)
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn read_rig_manifest(path: &Path) -> Result<poseforge_io::RigManifest, String> {
    const LIMIT: u64 = 16 * 1024 * 1024;
    let file = fs::File::open(path).map_err(err)?;
    let metadata = file.metadata().map_err(err)?;
    if !metadata.is_file() {
        return Err("Список рига должен быть обычным текстовым файлом".into());
    }
    if metadata.len() > LIMIT {
        return Err("Список рига превышает 16 МиБ".into());
    }
    let mut text = String::new();
    // The bounded read also handles a file that grows after the metadata check.
    file.take(LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(err)?;
    if text.len() as u64 > LIMIT {
        return Err("Список рига превышает 16 МиБ".into());
    }
    poseforge_io::parse_rig_manifest(&text).map_err(err)
}
fn rig_report_value(manifest: &poseforge_io::RigManifest, project: Option<&Project>) -> Value {
    let roots: Vec<_> = manifest
        .bones
        .iter()
        .filter(|bone| bone.parent.is_none())
        .map(|bone| bone.name.as_str())
        .collect();
    let comparison = project.map(|project| {
        let report = manifest.compare(project);
        let matched: Vec<_> = report
            .matched
            .iter()
            .map(|(name, id)| json!({"bone": name, "node_id": id}))
            .collect();
        json!({
            "project": project.title,
            "counts": {
                "matched": matched.len(),
                "missing": report.missing.len(),
                "ambiguous": report.ambiguous.len(),
                "hierarchy_mismatches": report.hierarchy_mismatches.len()
            },
            "matched": matched,
            "missing": report.missing,
            "ambiguous": report.ambiguous,
            "hierarchy_mismatches": report.hierarchy_mismatches
        })
    });
    json!({
        "schema": 1,
        "kind": "rig_manifest_report",
        "armature": manifest.armature,
        "declared_bones": manifest.declared_count,
        "bones": manifest.bones.len(),
        "root_count": roots.len(),
        "roots": roots,
        "metadata_only": true,
        "comparison": comparison
    })
}
fn skeleton_report_value(project: &Project, root: Option<NodeId>) -> Value {
    let report = poseforge_core::detect_humanoid(project, root);
    let roots: Vec<_> = poseforge_core::character_roots(project)
        .into_iter()
        .map(|id| json!({"node_id": id, "name": project.node(id).map(|node| node.name.as_str())}))
        .collect();
    let roles: Vec<_> = BoneRole::ALL
        .into_iter()
        .map(|role| {
            let candidates: Vec<_> = report
                .candidates
                .get(&role)
                .into_iter()
                .flatten()
                .map(|candidate| {
                    let confidence = match candidate.confidence {
                        BoneConfidence::High => "high",
                        BoneConfidence::Review => "review",
                        BoneConfidence::Ignored => "ignored_helper",
                    };
                    json!({
                        "node_id": candidate.node,
                        "name": candidate.name,
                        "confidence": confidence,
                        "confidence_label": candidate.confidence.label(),
                        "reason": candidate.reason
                    })
                })
                .collect();
            json!({
                "role": format!("{role:?}"),
                "label": role.label(),
                "resolved_node_id": report.resolved.assignments.get(&role),
                "candidates": candidates
            })
        })
        .collect();
    let resolved: Vec<_> = report
        .resolved
        .assignments
        .iter()
        .map(|(role, id)| json!({"role": format!("{role:?}"), "node_id": id}))
        .collect();
    json!({
        "schema": 1,
        "kind": "skeleton_detection",
        "project": project.title,
        "root": report.root,
        "root_candidates": roots,
        "resolved": resolved,
        "roles": roles,
        "warnings": report.warnings,
        "project_modified": false,
        "suggestions_only": true,
        "method": "partial_name_and_hierarchy_heuristics",
        "limitations": "Распознавание не доказывает анатомию, готовность IK или совместимость локальных осей; подтвердите соответствия в редакторе."
    })
}
fn check_blend_conversion_paths(input: &Path, output: &Path) -> Result<(), String> {
    if input
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case("blend"))
    {
        return Err("Исходный файл должен иметь расширение .blend".into());
    }
    if output
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case("poseforge") && !s.eq_ignore_ascii_case("artforge"))
    {
        return Err("Выходной файл должен иметь расширение .artforge или .poseforge".into());
    }
    match fs::symlink_metadata(output) {
        Ok(_) => {
            Err("Выходной файл уже существует; укажите новое имя. Перезапись отключена.".into())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
fn load_blend(path: &Path) -> Result<poseforge_io::ImportResult, String> {
    load_blend_with_options(path, poseforge_io::BlendImportOptions::default())
}

fn load_blend_with_options(
    path: &Path,
    options: poseforge_io::BlendImportOptions,
) -> Result<poseforge_io::ImportResult, String> {
    let cancelled = AtomicBool::new(false);
    let imported = poseforge_io::import_blend_with_options(
        path,
        None,
        options,
        &cancelled,
        |progress, message| {
            if progress.is_finite() && progress >= 0. {
                eprintln!("Blender {:.0}% · {message}", progress * 100.);
            } else {
                eprintln!("Blender · {message}");
            }
        },
    )
    .map_err(err)?;
    for warning in &imported.report.warnings {
        eprintln!("Импорт: {warning}");
    }
    for missing in &imported.report.missing_textures {
        eprintln!("Не найдена текстура: {missing}");
    }
    Ok(imported)
}

fn blend_command_arguments(args: &[String], expected: usize) -> Result<Vec<&str>, String> {
    if args
        .iter()
        .filter(|arg| arg.as_str() == "--animations")
        .count()
        > 1
    {
        return Err("Укажите --animations один раз.".into());
    }
    let positional = arguments_without_flags(args, &["--animations"])?;
    if positional.len() != expected {
        return Err(if expected == 3 {
            "Нужно: --convert-blend INPUT.blend OUTPUT.artforge [--animations]"
        } else {
            "Нужно: --import-report MODEL [--animations]"
        }
        .into());
    }
    Ok(positional)
}

static NEXT_CONVERSION_OUTPUT: AtomicU64 = AtomicU64::new(0);

struct PendingProject(PathBuf);
impl Drop for PendingProject {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Save completely before a create-only publication. A destination created by
/// another process during Blender conversion is never overwritten.
fn save_new_project(project: &Project, output: &Path) -> Result<(), String> {
    create_parent(output)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(err)?
        .as_nanos();
    let mut temporary = None;
    for _ in 0..100 {
        let sequence = NEXT_CONVERSION_OUTPUT.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".poseforge-convert-{}-{time}-{sequence}.poseforge",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                drop(file);
                temporary = Some(PendingProject(path));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    let temporary = temporary.ok_or("Не удалось создать временный файл проекта")?;
    poseforge_io::save_project(project, &temporary.0).map_err(err)?;
    fs::hard_link(&temporary.0, output).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            "Выходной файл появился во время импорта; существующий файл сохранён без изменений.".into()
        } else {
            format!("Не удалось записать новый проект без перезаписи: {error}. Выберите локальную папку на файловой системе с поддержкой жёстких ссылок.")
        }
    })?;
    Ok(())
}
fn exact_len(args: &[String], count: usize) -> Result<(), String> {
    if args.len() == count {
        Ok(())
    } else {
        Err("Неверное число аргументов; см. --help".into())
    }
}
fn number<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("Некорректное значение: {name} = {value}"))
}
fn dimension(value: &str) -> Result<u32, String> {
    let n = number(value, "размер изображения")?;
    if !(16..=8192).contains(&n) {
        return Err("Размер изображения должен быть от 16 до 8192 пикселей; предел GPU проверяется отдельно".into());
    }
    Ok(n)
}
fn arguments_without_flags<'a>(
    args: &'a [String],
    allowed: &[&str],
) -> Result<Vec<&'a str>, String> {
    args.iter()
        .enumerate()
        .filter_map(|(i, a)| {
            if allowed.contains(&a.as_str()) {
                None
            } else if i > 0 && a.starts_with("--") {
                Some(Err(format!("Неизвестный параметр: {a}")))
            } else {
                Some(Ok(a.as_str()))
            }
        })
        .collect()
}
fn load_input(path: &str) -> Result<Project, String> {
    if path == "demo" {
        return Ok(Project::demo());
    }
    let path = Path::new(path);
    match path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "blend" => Ok(load_blend(path)?.project),
        "gltf" | "glb" | "obj" => {
            let result = poseforge_io::import_model(path).map_err(err)?;
            for warning in result
                .report
                .warnings
                .iter()
                .chain(&result.report.missing_textures)
            {
                eprintln!("Импорт: {warning}");
            }
            Ok(result.project)
        }
        _ => poseforge_io::load_project(path).map_err(err),
    }
}
fn view(
    project: &Project,
    globals: &std::collections::BTreeMap<u64, glam::Mat4>,
    width: u32,
    height: u32,
) -> View {
    let c = project.render_camera(globals);
    View {
        eye: c.eye,
        target: c.target,
        up: c.up,
        fov_y: c.fov_y_radians,
        viewport: [0., 0., width as f32, height as f32],
        orthographic: c.orthographic.then_some(c.ortho_size),
        depth_of_field: c.depth_of_field,
        effects: c.effects,
    }
}
fn create_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(err)?;
    }
    Ok(())
}
fn render_frame(
    renderer: &mut Renderer,
    project: &Project,
    time: f64,
    path: &Path,
    size: [u32; 2],
    transparent: bool,
    fluids: &mut crate::app::ExportDomains,
    videos: &mut poseforge_io::video::VideoFrameCache,
) -> Result<(), String> {
    let [width, height] = size;
    let scene = project
        .evaluated((!project.clips.is_empty()).then_some(0), time)
        .map_err(err)?;
    let mut evaluated = project.clone();
    evaluated.apply_text_meshes(&scene.text_meshes);
    evaluated.apply_text_meshes(&scene.modifier_meshes);
    evaluated.lights.clone_from(&scene.lights);
    for node in &mut evaluated.nodes {
        if let Some(weights) = scene.morph_weights.get(&node.id) {
            node.morph_weights = weights.clone();
        }
    }
    let name = path.file_name().ok_or("Укажите имя PNG")?.to_string_lossy();
    fluids.apply(&mut evaluated, &scene.global, time, &AtomicBool::new(false))?;
    videos.apply(&mut evaluated, time, true, &AtomicBool::new(false)).map_err(|e| e.to_string())?;
    let pending = path.with_file_name(format!(".{name}.{}.pending.png", std::process::id()));
    let result = (|| {
        renderer.set_effect_time(time);
        renderer.export_png(
            &evaluated,
            &scene.global,
            &view(project, &scene.global, width, height),
            &[],
            &pending,
            width,
            height,
            transparent,
        )?;
        fs::File::open(&pending)
            .map_err(err)?
            .sync_all()
            .map_err(err)?;
        fs::rename(&pending, path).map_err(err)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}
fn sequence(args: &[String]) -> Result<(), String> {
    let p = arguments_without_flags(args, &["--transparent", "--resume"])?;
    if p.len() != 7 {
        return Err(
            "Нужно: --sequence INPUT DIRECTORY FIRST LAST WIDTH HEIGHT [--transparent] [--resume]"
                .into(),
        );
    }
    let project = load_input(p[1])?;
    let directory = Path::new(p[2]);
    let first: u32 = number(p[3], "первый кадр")?;
    let last: u32 = number(p[4], "последний кадр")?;
    if last < first || last - first > 100_000 {
        return Err("Диапазон кадров должен возрастать и содержать не более 100001 кадра".into());
    }
    let width = dimension(p[5])?;
    let height = dimension(p[6])?;
    let transparent = args.iter().any(|a| a == "--transparent");
    let resume = args.iter().any(|a| a == "--resume");
    fs::create_dir_all(directory).map_err(err)?;
    let manifest_path = directory.join("render-manifest.json");
    let settings = json!({"schema":1,"scene_hash":digest(&serde_json::to_vec(&project).map_err(err)?),"fps":project.fps,"first":first,"last":last,"width":width,"height":height,"transparent":transparent,"renderer":concat!("artforge-", env!("CARGO_PKG_VERSION"))});
    let mut manifest = if manifest_path.exists() {
        if !resume {
            return Err("В папке уже есть журнал. Используйте --resume или другую папку".into());
        }
        let previous: Value =
            serde_json::from_slice(&fs::read(&manifest_path).map_err(err)?).map_err(err)?;
        if previous.get("settings") != Some(&settings) {
            return Err(
                "Входной проект или параметры отличаются от журнала. Выберите другую папку".into(),
            );
        }
        if !previous.get("completed").is_some_and(Value::is_object) {
            return Err("Повреждён журнал последовательности".into());
        }
        previous
    } else {
        if resume {
            return Err("Нет render-manifest.json для продолжения".into());
        }
        for frame in first..=last {
            if directory.join(frame_name(frame)).exists() {
                return Err("В папке есть кадры без журнала; выберите пустую папку".into());
            }
        }
        json!({"settings":settings,"status":"running","completed":{}})
    };
    manifest["status"] = json!("running");
    atomic_json(&manifest_path, &manifest)?;
    let mut renderer = pollster::block_on(Renderer::headless(width, height))?;
    let mut fluids = crate::app::ExportDomains::default();
    let mut videos = poseforge_io::video::VideoFrameCache::default();
    for frame in first..=last {
        if directory.join("CANCEL").exists() {
            manifest["status"] = json!("cancelled");
            atomic_json(&manifest_path, &manifest)?;
            println!("Отменено. Готовые кадры сохранены; удалите CANCEL и используйте --resume.");
            return Ok(());
        }
        let name = frame_name(frame);
        let output = directory.join(&name);
        let recorded = manifest["completed"].get(&name).and_then(Value::as_str);
        if let Some(expected) = recorded {
            if output.exists() && digest(&fs::read(&output).map_err(err)?) == expected {
                println!("Кадр {frame}: уже готов");
                continue;
            }
        }
        let time = frame as f64 / project.fps as f64;
        render_frame(
            &mut renderer,
            &project,
            time,
            &output,
            [width, height],
            transparent,
            &mut fluids,
            &mut videos,
        )?;
        manifest["completed"][&name] = json!(digest(&fs::read(&output).map_err(err)?));
        atomic_json(&manifest_path, &manifest)?;
        println!("Кадр {frame}/{last}: {:.6} с → {name}", time);
    }
    manifest["status"] = json!("complete");
    atomic_json(&manifest_path, &manifest)?;
    println!("Последовательность готова: {}", directory.display());
    Ok(())
}
fn frame_name(frame: u32) -> String {
    format!("frame_{frame:06}.png")
}
/// Stable non-cryptographic checksum for resume consistency, not an authenticity claim.
fn digest(bytes: &[u8]) -> String {
    let mut state = 0xcbf29ce484222325u64;
    for &b in bytes {
        state ^= b as u64;
        state = state.wrapping_mul(0x100000001b3);
    }
    format!("{state:016x}")
}
fn atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    let temporary = path.with_file_name(format!(".render-manifest.{}.pending", std::process::id()));
    let mut file = fs::File::create(&temporary).map_err(err)?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(err)?)
        .map_err(err)?;
    file.sync_all().map_err(err)?;
    drop(file);
    fs::rename(&temporary, path).map_err(err)
}
fn self_check(directory: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&directory).map_err(err)?;
    let project = Project::demo();
    project.validate().map_err(err)?;
    let file = directory.join("self-check.poseforge");
    poseforge_io::save_project(&project, &file).map_err(err)?;
    let reopened = poseforge_io::load_project(&file).map_err(err)?;
    if project != reopened {
        return Err("Сохранённый проект не совпадает с исходным".into());
    }
    let clip = (!project.clips.is_empty()).then_some(0);
    let direct = project.evaluated(clip, 1.0).map_err(err)?;
    for frame in 0..project.fps {
        project
            .evaluated(clip, frame as f64 / project.fps as f64)
            .map_err(err)?;
    }
    let after = project.evaluated(clip, 1.0).map_err(err)?;
    if direct.global != after.global {
        return Err("Вычисление позы зависит от истории времени".into());
    }
    let mut water = poseforge_simulation::FluidWorld::demo();
    for _ in 0..120 {
        water.step();
    }
    if water.positions().iter().any(|p| !p.is_finite()) {
        return Err("Симуляция создала нечисловые координаты".into());
    }
    println!("Проверено: валидация демосцены, сохранение/открытие, независимость позы от истории времени, 120 шагов CPU PBF ({} частиц).\nGPU и интерфейс этой командой не проверяются.",water.positions().len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimensions_reject_invalid_and_excessive_values() {
        assert!(dimension("0").is_err());
        assert!(dimension("8193").is_err());
        assert!(dimension("nan").is_err());
        assert_eq!(dimension("1920").unwrap(), 1920);
    }

    #[test]
    fn rig_json_report_preserves_complete_large_manifest() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/grace-rig.txt");
        let manifest = read_rig_manifest(&fixture).unwrap();
        let metadata = rig_report_value(&manifest, None);
        assert_eq!(metadata["armature"], "Grace_rig");
        assert_eq!(metadata["bones"], 623);
        assert_eq!(metadata["root_count"], 28);
        assert!(metadata["comparison"].is_null());
        let report = rig_report_value(&manifest, Some(&Project::default()));
        let encoded = serde_json::to_string_pretty(&report).unwrap();
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        let missing = decoded["comparison"]["missing"].as_array().unwrap();
        assert_eq!(missing.len(), 623);
        assert_eq!(missing.first().unwrap(), "c_pos");
        assert_eq!(missing.last().unwrap(), "Hair.003.L.002");
        assert_eq!(decoded["comparison"]["counts"]["missing"], 623);
        assert_eq!(decoded["comparison"]["counts"]["matched"], 0);
    }

    #[test]
    fn skeleton_json_preserves_ambiguous_and_helper_candidates_without_mutation() {
        let mut project = Project::default();
        project.nodes.push(poseforge_core::Node::new(1, "Armature"));
        for (id, name) in [(2, "hand.L"), (3, "Hand.L"), (4, "c_hand.L")] {
            let mut node = poseforge_core::Node::new(id, name);
            node.parent = Some(1);
            node.is_joint = true;
            project.nodes.push(node);
        }
        let before = project.clone();
        let report = skeleton_report_value(&project, Some(1));
        let encoded = serde_json::to_string(&report).unwrap();
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        let hand = decoded["roles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|role| role["role"] == "LeftHand")
            .unwrap();
        assert!(hand["resolved_node_id"].is_null());
        let candidates = hand["candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 3);
        let ids: std::collections::BTreeSet<_> = candidates
            .iter()
            .map(|candidate| candidate["node_id"].as_u64().unwrap())
            .collect();
        assert_eq!(ids, std::collections::BTreeSet::from([2, 3, 4]));
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate["confidence"] == "ignored_helper")
                .count(),
            1
        );
        assert!(!decoded["warnings"].as_array().unwrap().is_empty());
        assert_eq!(decoded["root"], 1);
        assert_eq!(decoded["suggestions_only"], true);
        assert_eq!(decoded["project_modified"], false);
        assert_eq!(project, before);
    }

    #[test]
    fn skeleton_json_scopes_stable_ids_to_one_character() {
        let mut project = Project::default();
        for (root, hand) in [(1, 2), (10, 11)] {
            project
                .nodes
                .push(poseforge_core::Node::new(root, "Armature"));
            let mut node = poseforge_core::Node::new(hand, "hand.L");
            node.parent = Some(root);
            node.is_joint = true;
            project.nodes.push(node);
        }
        let unscoped = skeleton_report_value(&project, None);
        assert!(unscoped["resolved"].as_array().unwrap().is_empty());
        assert_eq!(unscoped["root_candidates"].as_array().unwrap().len(), 2);
        let scoped = skeleton_report_value(&project, Some(10));
        let hand = scoped["roles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|role| role["role"] == "LeftHand")
            .unwrap();
        assert_eq!(hand["resolved_node_id"], 11);
        assert_eq!(hand["candidates"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn conversion_rejects_wrong_extensions_before_launching_blender() {
        assert!(check_blend_conversion_paths(
            Path::new("character.fbx"),
            Path::new("new.poseforge")
        )
        .is_err());
        assert!(check_blend_conversion_paths(
            Path::new("character.blend"),
            Path::new("character.blend")
        )
        .is_err());
    }

    #[test]
    fn blend_animation_flag_is_explicit_and_does_not_replace_paths() {
        for arguments in [
            vec![
                "--convert-blend",
                "персонаж с пробелами.blend",
                "new.poseforge",
            ],
            vec![
                "--convert-blend",
                "--animations",
                "персонаж с пробелами.blend",
                "new.poseforge",
            ],
            vec![
                "--convert-blend",
                "персонаж с пробелами.blend",
                "new.poseforge",
                "--animations",
            ],
        ] {
            let args: Vec<String> = arguments.iter().map(|s| s.to_string()).collect();
            let paths = blend_command_arguments(&args, 3).unwrap();
            assert_eq!(
                paths,
                [
                    "--convert-blend",
                    "персонаж с пробелами.blend",
                    "new.poseforge"
                ]
            );
        }
        let args: Vec<String> = ["--import-report", "character.blend", "--animations"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            blend_command_arguments(&args, 2).unwrap(),
            ["--import-report", "character.blend"]
        );
    }

    #[test]
    fn invalid_animation_flags_are_rejected_before_reading_models() {
        for args in [
            vec![
                "--convert-blend",
                "source.blend",
                "out.poseforge",
                "--animations",
                "--animations",
            ],
            vec![
                "--convert-blend",
                "source.blend",
                "out.poseforge",
                "--unknown",
            ],
            vec!["--convert-blend", "source.blend", "--animations"],
            vec!["--import-report", "missing.glb", "--animations"],
        ] {
            let args: Vec<String> = args.into_iter().map(str::to_string).collect();
            let error = run(&args).unwrap_err();
            assert!(
                error.contains("--animations") || error.contains("--unknown"),
                "{error}"
            );
        }
    }

    #[test]
    fn create_only_project_publication_never_replaces_existing_output() {
        struct Directory(PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = Directory(std::env::temp_dir().join(format!(
            "poseforge-cli-output-{}-{nonce}-{}",
            std::process::id(),
            NEXT_CONVERSION_OUTPUT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&directory.0).unwrap();
        let output = directory.0.join("existing.poseforge");
        fs::write(&output, b"preserve existing bytes").unwrap();
        assert!(check_blend_conversion_paths(Path::new("character.blend"), &output).is_err());
        assert!(save_new_project(&Project::default(), &output).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"preserve existing bytes");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
        let new_output = directory.0.join("new.poseforge");
        save_new_project(&Project::default(), &new_output).unwrap();
        assert_eq!(
            poseforge_io::load_project(new_output).unwrap(),
            Project::default()
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }
}
