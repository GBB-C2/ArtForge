//! Bounded Wavefront OBJ/MTL import. No subprocess or remote URI is used.
//! Polygons use a simple projection; UV and normal seams stay split.
use crate::{
    import::frame_project,
    import_limits::{MAX_MODEL_BYTES, MAX_TEXTURE_BYTES},
    ImportReport, ImportResult, IoError, Result,
};
use glam::{Vec2, Vec3};
use poseforge_core::{AlphaMode, Material, Mesh, Node, Project, Vertex};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

const MAX_VERTICES: usize = 2_000_000;
const MAX_FACE: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Corner {
    position: usize,
    uv: Option<usize>,
    normal: Option<usize>,
    smoothing: u64,
}

struct Group {
    name: String,
    material: String,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    lookup: BTreeMap<Corner, u32>,
    generated_normals: Vec<Option<(usize, u64)>>,
}
impl Group {
    fn new(name: &str, material: &str) -> Self {
        Self {
            name: name.into(),
            material: material.into(),
            vertices: vec![],
            indices: vec![],
            lookup: BTreeMap::new(),
            generated_normals: vec![],
        }
    }
    fn corner(
        &mut self,
        corner: Corner,
        positions: &[Vec3],
        uvs: &[Vec2],
        normals: &[Vec3],
    ) -> u32 {
        if let Some(index) = self.lookup.get(&corner) {
            return *index;
        }
        let index = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            position: positions[corner.position].to_array(),
            normal: corner
                .normal
                .map(|i| normals[i])
                .unwrap_or(Vec3::ZERO)
                .to_array(),
            uv: corner.uv.map(|i| uvs[i]).unwrap_or(Vec2::ZERO).to_array(),
            ..Vertex::default()
        });
        self.generated_normals.push(
            corner
                .normal
                .is_none()
                .then_some((corner.position, corner.smoothing)),
        );
        self.lookup.insert(corner, index);
        index
    }
    fn finish_normals(
        &mut self,
        normals: &BTreeMap<(usize, u64), Vec3>,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        for (index, (vertex, key)) in self
            .vertices
            .iter_mut()
            .zip(&self.generated_normals)
            .enumerate()
        {
            if index % 4096 == 0 {
                check_cancel(cancelled)?;
            }
            vertex.normal = key
                .and_then(|key| normals.get(&key).copied())
                .unwrap_or_else(|| Vec3::from_array(vertex.normal))
                .try_normalize()
                .unwrap_or(Vec3::Y)
                .to_array();
        }
        Ok(())
    }
}

#[derive(Default)]
struct MaterialSource {
    value: Material,
    base: PathBuf,
    color: Option<String>,
    emission: Option<String>,
    normal: Option<String>,
}

pub fn import_obj(path: impl AsRef<Path>) -> Result<ImportResult> {
    import_obj_with_progress(path, &AtomicBool::new(false), |_, _| {})
}

pub fn import_obj_with_progress(
    path: impl AsRef<Path>,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(f32, &str),
) -> Result<ImportResult> {
    check_cancel(cancelled)?;
    let path = path.as_ref();
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut budget = 0;
    progress(0.01, "Чтение файла OBJ");
    let file = fs::File::open(path)?;
    let file_size = file.metadata()?.len();
    if file_size > MAX_MODEL_BYTES {
        return invalid("OBJ и MTL превышают общий лимит 2 ГиБ");
    }
    let mut source = BufReader::new(file);
    let title = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut report = ImportReport::default();
    let (mut positions, mut uvs, mut normals) = (Vec::new(), Vec::new(), Vec::new());
    let (mut name, mut material) = (title.clone(), String::new());
    let mut groups = Vec::<Group>::new();
    let mut libraries = BTreeSet::new();
    let mut smoothing_names = BTreeMap::<String, u64>::new();
    let mut smoothing = 0;
    let mut face_number = 0u64;
    let mut expanded_vertices = 0usize;
    let mut total_indices = 0usize;
    let mut nonplanar_faces = 0usize;
    let mut cleaned_faces = 0usize;
    let mut skipped_faces = 0usize;
    let mut skipped_examples = Vec::new();
    let mut smooth_normals = BTreeMap::<(usize, u64), Vec3>::new();
    let mut parser_warnings = BTreeSet::new();
    let mut parsed_bytes = 0usize;
    progress(0.02, "Чтение OBJ");
    let mut line_number = 0usize;
    let mut raw = String::new();
    loop {
        check_cancel(cancelled)?;
        raw.clear();
        // Streaming avoids retaining a potentially multi-gigabyte OBJ string.
        // A bounded reader also prevents a malformed single line growing it.
        let count = (&mut source).take(1024 * 1024 + 3).read_line(&mut raw)?;
        if count == 0 {
            break;
        }
        parsed_bytes += count;
        budget += count as u64;
        if budget > MAX_MODEL_BYTES {
            return invalid("OBJ и MTL превышают общий лимит 2 ГиБ");
        }
        let raw = raw.trim_end_matches(['\n', '\r']);
        let raw = if line_number == 0 {
            raw.trim_start_matches('\u{feff}')
        } else {
            raw
        };
        if line_number.is_multiple_of(1024) {
            check_cancel(cancelled)?;
            progress(
                0.02 + 0.5 * (parsed_bytes as f32 / file_size.max(1) as f32).min(1.),
                "Геометрия OBJ",
            );
        }
        line_number += 1;
        if raw.len() > 1024 * 1024 {
            return invalid("Строка OBJ превышает 1 МиБ");
        }
        let line = crate::obj_resource::without_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let (command, value) = split_command(line);
        let fail =
            |message: String| IoError::Invalid(format!("OBJ, строка {line_number}: {message}"));
        match command {
            "v" => {
                let values = numbers(value).map_err(fail)?;
                if values.len() < 3 {
                    return Err(fail("ожидались координаты x y z".into()));
                }
                let mut p = Vec3::new(values[0], values[1], values[2]);
                if values.len() == 4 {
                    if values[3].abs() < 1e-12 {
                        return Err(fail("нулевая однородная координата".into()));
                    }
                    p /= values[3];
                } else if values.len() > 4 {
                    parser_warnings.insert(
                        "OBJ: цвета вершин не переносятся; используйте MTL или текстуру".into(),
                    );
                }
                if !p.is_finite() {
                    return Err(fail("координата вышла за числовой диапазон".into()));
                }
                positions.push(p);
            }
            "vt" => {
                let values = numbers(value).map_err(fail)?;
                if values.is_empty() {
                    return Err(fail("ожидалась UV-координата".into()));
                }
                // OBJ's V=0 is the bottom edge; raster image coordinates start at the top.
                uvs.push(Vec2::new(
                    values[0],
                    1. - values.get(1).copied().unwrap_or(0.),
                ));
            }
            "vn" => {
                let values = numbers(value).map_err(fail)?;
                if values.len() != 3 {
                    return Err(fail("ожидалась нормаль x y z".into()));
                }
                let normal = Vec3::new(values[0], values[1], values[2])
                    .try_normalize()
                    .ok_or_else(|| fail("нулевая нормаль".into()))?;
                normals.push(normal);
            }
            "o" | "g" => {
                name = if value.is_empty() {
                    title.clone()
                } else {
                    value.into()
                };
            }
            "usemtl" => {
                material = value.into();
            }
            "mtllib" => {
                libraries.extend(crate::obj_resource::library_references(base, value));
                if libraries.len() > 256 {
                    return invalid("OBJ содержит более 256 библиотек MTL");
                }
            }
            "s" => {
                smoothing = if value == "off" || value == "0" {
                    0
                } else {
                    let next = smoothing_names.len() as u64 + 1;
                    *smoothing_names.entry(value.into()).or_insert(next)
                };
            }
            "f" => {
                face_number += 1;
                let tokens: Vec<_> = value.split_whitespace().take(MAX_FACE + 1).collect();
                if tokens.len() < 3 || tokens.len() > MAX_FACE {
                    return Err(fail("грань должна содержать от 3 до 256 вершин".into()));
                }
                let mut corners = Vec::with_capacity(tokens.len());
                for token in tokens {
                    let parts: Vec<_> = token.split('/').collect();
                    if parts.len() > 3 {
                        return Err(fail("некорректная запись вершины грани".into()));
                    }
                    let position = index(parts[0], positions.len()).map_err(fail)?;
                    let uv = parts
                        .get(1)
                        .filter(|s| !s.is_empty())
                        .map(|s| index(s, uvs.len()))
                        .transpose()
                        .map_err(fail)?;
                    let normal = parts
                        .get(2)
                        .filter(|s| !s.is_empty())
                        .map(|s| index(s, normals.len()))
                        .transpose()
                        .map_err(fail)?;
                    corners.push(Corner {
                        position,
                        uv,
                        normal,
                        smoothing: if normal.is_some() {
                            0
                        } else if smoothing == 0 {
                            (1u64 << 63) | face_number
                        } else {
                            smoothing
                        },
                    });
                }
                let points: Vec<_> = corners.iter().map(|c| positions[c.position]).collect();
                let result = match crate::obj_polygon::triangulate(&points) {
                    Ok(result) => result,
                    Err(reason) => {
                        skipped_faces += 1;
                        // Keep diagnostics bounded even in a badly damaged export.
                        if skipped_examples.len() < 8 {
                            skipped_examples.push(format!("строка {line_number}: {reason}"));
                        }
                        continue;
                    }
                };
                nonplanar_faces += usize::from(result.nonplanar);
                cleaned_faces += usize::from(result.cleaned);
                let triangles = result.triangles;
                total_indices += triangles.len() * 3;
                if total_indices > 12_000_000 {
                    return invalid("OBJ превышает лимит 4 миллиона треугольников");
                }
                if groups
                    .last()
                    .is_none_or(|g| g.name != name || g.material != material)
                {
                    if groups.len() >= 10_000 {
                        return invalid("OBJ содержит более 10 000 групп мешей");
                    }
                    groups.push(Group::new(&name, &material));
                }
                let group = groups.last_mut().unwrap();
                let before = group.vertices.len();
                let mut used = vec![false; corners.len()];
                for &i in triangles.iter().flatten() {
                    used[i] = true;
                }
                let mapped: Vec<_> = corners
                    .iter()
                    .enumerate()
                    .map(|(i, &corner)| {
                        used[i].then(|| group.corner(corner, &positions, &uvs, &normals))
                    })
                    .collect();
                expanded_vertices += group.vertices.len() - before;
                if expanded_vertices > MAX_VERTICES {
                    return invalid("OBJ превышает 2 миллиона вершин с учётом UV/нормалей");
                }
                for triangle in triangles {
                    // A UV seam or material split must not split a smoothing
                    // group. Accumulate by the original position and group.
                    let [a, b, c] = triangle.map(|i| positions[corners[i].position]);
                    let normal = (b - a).cross(c - a);
                    for i in triangle {
                        let corner = corners[i];
                        if corner.normal.is_none() {
                            *smooth_normals
                                .entry((corner.position, corner.smoothing))
                                .or_default() += normal;
                        }
                    }
                    group.indices.extend(triangle.map(|i| mapped[i].unwrap()));
                }
            }
            "l" | "p" | "curv" | "curv2" | "surf" => {
                parser_warnings.insert("OBJ: линии, точки и параметрические поверхности пропущены; импортируются полигональные грани".into());
            }
            _ => {}
        }
        if positions.len().max(uvs.len()).max(normals.len()) > MAX_VERTICES {
            return invalid("OBJ превышает лимит 2 миллиона исходных атрибутов");
        }
    }
    if groups.is_empty() {
        if skipped_faces > 0 {
            return invalid(format!(
                "OBJ: нет пригодных граней; пропущено {skipped_faces}. {}",
                skipped_examples.join("; ")
            ));
        }
        return invalid("OBJ не содержит полигональных граней");
    }
    report.warnings.extend(parser_warnings);
    if nonplanar_faces > 0 {
        report.warnings.push(format!("Неплоские грани OBJ автоматически триангулированы: {nonplanar_faces}; исходные координаты сохранены"));
    }
    if cleaned_faces > 0 {
        report.warnings.push(format!("OBJ: очищены повторяющиеся соседние/замыкающие углы в {cleaned_faces} гранях; на нулевом ребре используется первый угол"));
    }
    if skipped_faces > 0 {
        report.warnings.push(format!(
            "OBJ: пропущено повреждённых граней: {skipped_faces}; в модели могут быть отверстия. Первые случаи: {}",
            skipped_examples.join("; ")
        ));
    }
    check_cancel(cancelled)?;
    progress(0.55, "Материалы MTL и текстуры");
    let mut definitions = BTreeMap::new();
    for library in libraries {
        check_cancel(cancelled)?;
        let resource = match crate::obj_resource::resolve(base, base, &library) {
            Ok(resource) => resource,
            Err(reason) => {
                report.warnings.push(format!(
                    "Библиотека MTL {library}: {reason}; используются материалы по умолчанию"
                ));
                continue;
            }
        };
        if resource.relocated {
            report.warnings.push(format!(
                "Ресурс OBJ/MTL {library} найден по новому пути: {}",
                resource.path.display()
            ));
        }
        let library_path = resource.path;
        let text = match read_text(&library_path, &mut budget, cancelled) {
            Ok(text) => text,
            Err(IoError::File(error)) => {
                report.warnings.push(format!(
                    "Библиотека MTL {library}: {error}; используются материалы по умолчанию"
                ));
                continue;
            }
            Err(error) => return Err(error),
        };
        read_mtl(
            &text,
            library_path.parent().unwrap_or(base),
            &mut definitions,
            &mut report,
            cancelled,
        )?;
    }
    let mut project = Project {
        title,
        lights: vec![],
        ..Project::default()
    };
    let mut next_id = 1u64;
    let mut material_ids = BTreeMap::<String, u64>::new();
    let mut texture_ids = BTreeMap::<(PathBuf, bool), u64>::new();
    let mut texture_bytes = 0usize;
    for mut group in groups {
        check_cancel(cancelled)?;
        let material_id = if let Some(id) = material_ids.get(&group.material) {
            *id
        } else {
            let source = definitions.remove(&group.material).unwrap_or_else(|| {
                if !group.material.is_empty() {
                    report
                        .warnings
                        .push(format!("Материал MTL не найден: {}", group.material));
                }
                MaterialSource {
                    value: Material {
                        name: if group.material.is_empty() {
                            "Материал OBJ".into()
                        } else {
                            group.material.clone()
                        },
                        ..Material::default()
                    },
                    base: base.to_owned(),
                    ..MaterialSource::default()
                }
            });
            let mut value = source.value;
            value.base_color_texture = load_map(
                source.color.as_deref(),
                &source.base,
                base,
                false,
                &mut project,
                &mut next_id,
                &mut texture_ids,
                &mut texture_bytes,
                &mut report,
            )?;
            check_cancel(cancelled)?;
            value.emissive_texture = load_map(
                source.emission.as_deref(),
                &source.base,
                base,
                false,
                &mut project,
                &mut next_id,
                &mut texture_ids,
                &mut texture_bytes,
                &mut report,
            )?;
            check_cancel(cancelled)?;
            value.normal_texture = load_map(
                source.normal.as_deref(),
                &source.base,
                base,
                true,
                &mut project,
                &mut next_id,
                &mut texture_ids,
                &mut texture_bytes,
                &mut report,
            )?;
            value.id = next_id;
            next_id += 1;
            let id = value.id;
            project.materials.push(value);
            material_ids.insert(group.material.clone(), id);
            id
        };
        group.finish_normals(&smooth_normals, cancelled)?;
        let mesh_id = next_id;
        next_id += 1;
        project.meshes.push(Mesh {
            id: mesh_id,
            name: group.name.clone(),
            vertices: group.vertices.into(),
            indices: group.indices.into(),
            material: Some(material_id),
            topology: None,
            morph_targets: vec![],
        });
        let mut node = Node::new(next_id, &group.name);
        next_id += 1;
        node.mesh = Some(mesh_id);
        project.nodes.push(node);
    }
    check_cancel(cancelled)?;
    progress(0.94, "Проверка OBJ");
    frame_project(&mut project)?;
    project
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    report.meshes = project.meshes.len();
    report.materials = project.materials.len();
    report.textures = project.textures.len();
    report.vertices = expanded_vertices;
    report.finish_texture_report(&project.textures, MAX_TEXTURE_BYTES);
    report.warnings.sort();
    report.warnings.dedup();
    report.missing_textures.sort();
    report.missing_textures.dedup();
    progress(1., "Готово");
    Ok(ImportResult { project, report })
}

fn read_text(path: &Path, total: &mut u64, cancelled: &AtomicBool) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let remaining = MAX_MODEL_BYTES.saturating_sub(*total);
    if file.metadata()?.len() > remaining {
        return invalid("OBJ и MTL превышают общий лимит 2 ГиБ");
    }
    let mut data = Vec::new();
    let mut chunk = vec![0; 256 * 1024];
    loop {
        check_cancel(cancelled)?;
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        if data.len() as u64 + count as u64 > remaining {
            return invalid("OBJ и MTL превышают общий лимит 2 ГиБ");
        }
        data.extend_from_slice(&chunk[..count]);
    }
    *total += data.len() as u64;
    if *total > MAX_MODEL_BYTES {
        return invalid("OBJ и MTL превышают общий лимит 2 ГиБ");
    }
    let mut text = String::from_utf8(data)
        .map_err(|_| IoError::Invalid("OBJ/MTL должен содержать текст UTF-8".into()))?;
    if text.starts_with('\u{feff}') {
        text.drain(..3);
    }
    Ok(text)
}
fn check_cancel(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(IoError::Cancelled)
    } else {
        Ok(())
    }
}
fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(IoError::Invalid(message.into()))
}
fn split_command(line: &str) -> (&str, &str) {
    line.find(char::is_whitespace)
        .map(|i| (&line[..i], line[i..].trim()))
        .unwrap_or((line, ""))
}
fn numbers(text: &str) -> std::result::Result<Vec<f32>, String> {
    text.split_whitespace()
        .map(|s| {
            s.parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("нечисловое значение {s}"))
        })
        .collect()
}
fn index(text: &str, count: usize) -> std::result::Result<usize, String> {
    let value = text
        .parse::<i64>()
        .map_err(|_| "некорректный индекс OBJ".to_owned())?;
    let resolved = if value > 0 {
        value - 1
    } else {
        count as i64 + value
    };
    if value == 0 || resolved < 0 || resolved >= count as i64 {
        return Err("индекс OBJ выходит за число объявленных атрибутов".into());
    }
    Ok(resolved as usize)
}
fn read_mtl(
    text: &str,
    base: &Path,
    definitions: &mut BTreeMap<String, MaterialSource>,
    report: &mut ImportReport,
    cancelled: &AtomicBool,
) -> Result<()> {
    let mut current = None::<String>;
    for (line_number, line) in text.lines().enumerate() {
        if line_number.is_multiple_of(1024) {
            check_cancel(cancelled)?;
        }
        if line.len() > 1024 * 1024 {
            return invalid("Строка MTL превышает 1 МиБ");
        }
        let line = crate::obj_resource::without_comment(line).trim();
        let (command, value) = split_command(line);
        if command == "newmtl" {
            if value.is_empty() {
                return invalid("MTL: материал без имени");
            }
            if definitions.len() >= 10_000 {
                return invalid("MTL содержит более 10 000 материалов");
            }
            if definitions.contains_key(value) {
                report.warnings.push(format!(
                    "MTL: повторное определение материала {value}; используется последнее"
                ));
            }
            definitions.insert(
                value.into(),
                MaterialSource {
                    value: Material {
                        name: value.into(),
                        ..Material::default()
                    },
                    base: base.to_owned(),
                    ..MaterialSource::default()
                },
            );
            current = Some(value.into());
            continue;
        }
        let Some(source) = current.as_ref().and_then(|name| definitions.get_mut(name)) else {
            continue;
        };
        let fail = |m: String| IoError::Invalid(format!("MTL, строка {}: {m}", line_number + 1));
        match command {
            "Kd" | "Ke" => {
                let v = numbers(value).map_err(fail)?;
                if v.len() != 3 || v.iter().any(|x| *x < 0.) {
                    return Err(fail("ожидался неотрицательный RGB".into()));
                }
                if command == "Kd" {
                    source.value.base_color[..3].copy_from_slice(&[
                        v[0].min(1.),
                        v[1].min(1.),
                        v[2].min(1.),
                    ]);
                } else {
                    source.value.emissive.copy_from_slice(&v);
                }
            }
            "Ns" | "Pr" | "Pm" | "d" | "Tr" => {
                let numbers =
                    numbers(value.strip_prefix("-halo ").unwrap_or(value)).map_err(fail)?;
                if numbers.len() != 1 {
                    return Err(fail("ожидался один числовой параметр".into()));
                }
                let v = numbers[0];
                match command {
                    "Ns" => source.value.roughness = (2. / (v.max(0.) + 2.)).sqrt().clamp(0., 1.),
                    "Pr" => source.value.roughness = v.clamp(0., 1.),
                    "Pm" => source.value.metallic = v.clamp(0., 1.),
                    _ => {
                        source.value.base_color[3] =
                            (if command == "Tr" { 1. - v } else { v }).clamp(0., 1.);
                        source.value.alpha_mode = if source.value.base_color[3] < 1. {
                            AlphaMode::Blend
                        } else {
                            AlphaMode::Opaque
                        };
                    }
                }
            }
            "map_Kd" | "map_Ke" | "norm" => {
                if value.starts_with('-') {
                    report.warnings.push(format!("MTL {command}: параметры смещения/масштаба карты не поддерживаются; карта пропущена"));
                } else {
                    match command {
                        "map_Kd" => source.color = Some(value.into()),
                        "map_Ke" => source.emission = Some(value.into()),
                        _ => source.normal = Some(value.into()),
                    }
                }
            }
            "bump" | "map_Bump" | "map_bump" | "map_Pr" | "map_Pm" | "map_Ks" | "map_d"
            | "disp" | "refl" => {
                report.warnings.push(format!("MTL {command}: карта пока не поддерживается; используйте glTF для полного набора PBR-карт"));
            }
            _ => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn load_map(
    reference: Option<&str>,
    base: &Path,
    model_base: &Path,
    normal_map: bool,
    project: &mut Project,
    next_id: &mut u64,
    cache: &mut BTreeMap<(PathBuf, bool), u64>,
    total: &mut usize,
    report: &mut ImportReport,
) -> Result<Option<u64>> {
    let Some(reference) = reference else {
        return Ok(None);
    };
    let resolved =
        crate::obj_resource::resolve(base, model_base, reference).map_err(IoError::Invalid);
    if let Ok(resource) = &resolved {
        if resource.relocated {
            report.warnings.push(format!(
                "Ресурс OBJ/MTL {reference} найден по новому пути: {}",
                resource.path.display()
            ));
        }
    }
    let key = resolved.as_ref().ok().map(|resource| {
        (
            fs::canonicalize(&resource.path).unwrap_or_else(|_| resource.path.clone()),
            normal_map,
        )
    });
    if let Some(id) = key.as_ref().and_then(|key| cache.get(key)) {
        return Ok(Some(*id));
    }
    let mut texture = match resolved.and_then(|resource| {
        crate::texture::decode_texture(
            &crate::texture::read_image(&resource.path)?,
            &resource.path.to_string_lossy(),
            MAX_TEXTURE_BYTES as u64,
        )
    }) {
        Ok(texture) => texture,
        Err(error @ IoError::Limit(_)) => return Err(error),
        Err(error) => {
            report.missing_textures.push(reference.into());
            report
                .warnings
                .push(format!("Текстура {reference}: {error}; используются цвет и числовые параметры материала. Поместите изображение рядом с моделью/MTL или назначьте карту в свойствах материала."));
            return Ok(None);
        }
    };
    texture.name = reference.into();
    // The OBJ V axis was flipped to the raster convention above. Its normal
    // map (+Y / OpenGL convention) must flip the tangent-space Y channel too.
    // Keep this derived asset separate from an albedo that uses the same file.
    if normal_map {
        for pixel in texture.rgba8.as_chunks_mut::<4>().0 {
            pixel[1] = 255 - pixel[1];
        }
        texture.name.push_str(" (OBJ normal +Y)");
    }
    texture.id = *next_id;
    *next_id += 1;
    let id = texture.id;
    project.textures.push(texture);
    report.record_texture_reductions(crate::texture::fit_texture_budget(
        &mut project.textures,
        MAX_TEXTURE_BYTES,
    )?);
    *total = project.textures.iter().map(|t| t.rgba8.len()).sum();
    if let Some(key) = key {
        cache.insert(key, id);
    }
    Ok(Some(id))
}
