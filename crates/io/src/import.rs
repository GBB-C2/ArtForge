use crate::{
    import_limits::{MAX_MODEL_BYTES, MAX_TEXTURE_BYTES},
    IoError, Result,
};
use glam::{Mat4, Quat, Vec3};
use poseforge_core::{
    AlphaMode, Camera, Channel, Clip, Interpolation, Keyframe, Light, LightKind, Material, Mesh,
    MorphTarget, Node, Project, Property, Skin, Texture, TextureUvTransform, Transform, Vertex,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const MAX_VERTICES: usize = 2_000_000;
const MAX_MORPHS: usize = 64;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub meshes: usize,
    pub materials: usize,
    pub joints: usize,
    pub clips: usize,
    pub textures: usize,
    #[serde(default)]
    pub lights: usize,
    pub vertices: usize,
    pub missing_textures: Vec<String>,
    pub warnings: Vec<String>,
    /// Detailed original-to-final dimensions, without repeated intermediate sizes.
    #[serde(default)]
    pub texture_reductions: Vec<crate::TextureReduction>,
}
impl ImportReport {
    pub(crate) fn record_texture_reductions(&mut self, changes: Vec<crate::TextureReduction>) {
        for change in changes {
            if let Some(previous) = self
                .texture_reductions
                .iter_mut()
                .find(|r| r.texture_id == change.texture_id)
            {
                previous.imported_size = change.imported_size;
            } else {
                self.texture_reductions.push(change);
            }
        }
    }

    pub(crate) fn finish_texture_report(&mut self, textures: &[Texture], budget: usize) {
        if self.texture_reductions.is_empty() {
            return;
        }
        self.texture_reductions.sort_by_key(|r| r.texture_id);
        let mut groups = BTreeMap::<([u32; 2], [u32; 2]), usize>::new();
        for change in &self.texture_reductions {
            *groups
                .entry((change.original_size, change.imported_size))
                .or_default() += 1;
        }
        let sizes = groups
            .into_iter()
            .map(|(([w, h], [nw, nh]), count)| format!("{w}×{h} → {nw}×{nh}: {count}"))
            .collect::<Vec<_>>()
            .join("; ");
        let bytes: usize = textures.iter().map(|t| t.rgba8.len()).sum();
        self.warnings.push(format!(
            "Для бюджета текстур {} МиБ уменьшено карт: {} (в памяти {:.1} МиБ). {}. Исходные файлы не изменены.",
            budget / (1024 * 1024), self.texture_reductions.len(), bytes as f64 / (1024. * 1024.), sizes
        ));
    }
}

#[derive(Debug)]
pub struct ImportResult {
    pub project: Project,
    pub report: ImportReport,
}

pub fn import_gltf(path: impl AsRef<Path>) -> Result<ImportResult> {
    import_gltf_with_progress(path, &AtomicBool::new(false), |_, _| {})
}

/// Prepares a detached document. The caller commits it through one editor
/// transaction after success. Call on a worker thread; progress callbacks run
/// on that thread. Cancellation is checked between bounded import stages.
pub fn import_gltf_with_progress(
    path: impl AsRef<Path>,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(f32, &str),
) -> Result<ImportResult> {
    let check = || {
        if cancelled.load(Ordering::Relaxed) {
            Err(IoError::Cancelled)
        } else {
            Ok(())
        }
    };
    check()?;
    let path = path.as_ref();
    if fs::metadata(path)?.len() > MAX_MODEL_BYTES {
        return invalid(
            "Файл превышает лимит импорта 2 ГиБ. Разделите модель на несколько файлов.",
        );
    }
    progress(0.02, "Чтение glTF");
    let file = gltf::Gltf::open(path)?;
    let doc = &file.document;
    crate::gltf_preflight::preflight(doc, cancelled)?;
    if doc.nodes().len() > 10_000 {
        return invalid("Превышен лимит 10 000 узлов сцены");
    }
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut report = ImportReport::default();
    // No extension is claimed as implemented merely because the parser can
    // read it. Optional extension fallback is recorded in the import report.
    let supported_extension = |name: &str| {
        matches!(
            name,
            "KHR_lights_punctual" | "KHR_materials_emissive_strength" | "KHR_texture_transform"
        )
    };
    let required: Vec<_> = doc
        .extensions_required()
        .filter(|name| !supported_extension(name))
        .collect();
    if !required.is_empty() {
        return invalid(format!(
            "Обязательные расширения glTF пока не поддерживаются: {}",
            required.join(", ")
        ));
    }
    for extension in doc.extensions_used() {
        if supported_extension(extension) {
            continue;
        }
        report.warnings.push(format!(
            "Необязательное расширение {extension}: используется базовое представление glTF"
        ));
    }
    // Count the actual payload too: a small byteLength must not hide a large
    // external file, or several references to the same large resource.
    let mut payload_bytes = 0u64;
    for b in doc.buffers() {
        check()?;
        let actual = match b.source() {
            gltf::buffer::Source::Uri(uri) => check_uri(uri, base)?.unwrap_or(0),
            gltf::buffer::Source::Bin => file.blob.as_ref().map_or(0, |blob| blob.len() as u64),
        };
        payload_bytes = payload_bytes
            .checked_add(actual.max(b.length() as u64))
            .ok_or_else(|| IoError::Invalid("Переполнение размера буфера".into()))?;
        if payload_bytes > MAX_MODEL_BYTES {
            return invalid(
                "Суммарные буферы glTF превышают 2 ГиБ. Разделите модель на несколько файлов.",
            );
        }
    }
    let mut buffers = Vec::with_capacity(doc.buffers().len());
    let mut blob = file.blob;
    let mut loaded_bytes = 0u64;
    for buffer in doc.buffers() {
        check()?;
        let data =
            gltf::buffer::Data::from_source_and_blob(buffer.source(), Some(base), &mut blob)?;
        if data.len() < buffer.length() {
            return Err(gltf::Error::BufferLength {
                buffer: buffer.index(),
                expected: buffer.length(),
                actual: data.len(),
            }
            .into());
        }
        loaded_bytes = loaded_bytes
            .checked_add(data.len() as u64)
            .ok_or_else(|| IoError::Invalid("Переполнение размера буфера".into()))?;
        if loaded_bytes > MAX_MODEL_BYTES {
            return invalid("Суммарные буферы glTF превышают 2 ГиБ");
        }
        buffers.push(data);
        progress(
            0.04 + 0.1 * buffers.len() as f32 / doc.buffers().len().max(1) as f32,
            "Загрузка буферов glTF",
        );
    }
    check()?;
    for view in doc.views() {
        let end = view
            .offset()
            .checked_add(view.length())
            .ok_or_else(|| IoError::Invalid("Переполнение bufferView".into()))?;
        if end > buffers[view.buffer().index()].len() {
            return invalid("bufferView выходит за границы буфера");
        }
    }
    // Accessor bounds need checking against the actual payload, not only JSON
    // indices. This also prevents reader panics on truncated external buffers.
    for accessor in doc.accessors() {
        if let Some(view) = accessor.view() {
            let stride = view.stride().unwrap_or(accessor.size());
            let count = accessor.count();
            let length = if count == 0 {
                0
            } else {
                (count - 1)
                    .checked_mul(stride)
                    .and_then(|x| x.checked_add(accessor.size()))
                    .ok_or_else(|| IoError::Invalid("Переполнение accessor".into()))?
            };
            if accessor
                .offset()
                .checked_add(length)
                .map(|x| x > view.length())
                .unwrap_or(true)
            {
                return invalid("Accessor выходит за границы bufferView");
            }
        }
        if let Some(sparse) = accessor.sparse() {
            if sparse.count() > accessor.count() {
                return invalid("Число sparse элементов превышает размер accessor");
            }
            let indices = sparse.indices();
            let values = sparse.values();
            for (offset, count, size, limit) in [
                (
                    indices.offset(),
                    sparse.count(),
                    indices.index_type().size(),
                    indices.view().length(),
                ),
                (
                    values.offset(),
                    sparse.count(),
                    accessor.size(),
                    values.view().length(),
                ),
            ] {
                if count
                    .checked_mul(size)
                    .and_then(|n| offset.checked_add(n))
                    .map(|n| n > limit)
                    .unwrap_or(true)
                {
                    return invalid("Sparse accessor выходит за границы bufferView");
                }
            }
            let view = indices.view();
            let begin = view.offset() + indices.offset();
            let size = indices.index_type().size();
            let raw = &buffers[view.buffer().index()].0[begin..begin + sparse.count() * size];
            let mut previous = None;
            for bytes in raw.chunks_exact(size) {
                let index = match size {
                    1 => bytes[0] as usize,
                    2 => u16::from_le_bytes([bytes[0], bytes[1]]) as usize,
                    _ => u32::from_le_bytes(bytes.try_into().unwrap()) as usize,
                };
                if index >= accessor.count() || previous.map(|p| index <= p).unwrap_or(false) {
                    return invalid("Индексы sparse должны возрастать и попадать в accessor");
                }
                previous = Some(index);
            }
        }
    }
    check()?;
    progress(0.14, "Геометрия и материалы");
    let mut project = Project {
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        duration: 4.,
        // An interchange model without lights must not add a duplicate studio
        // key on every import into the existing document.
        lights: vec![],
        ..Project::default()
    };
    let mut next_id = doc.nodes().len() as u64 + 1;
    let mut alloc = || {
        let id = next_id;
        next_id += 1;
        id
    };
    let mut image_ids = BTreeMap::new();
    for image in doc.images() {
        check()?;
        let name = image
            .name()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Текстура {}", image.index() + 1));
        let mut texture = match decode_gltf_image(
            image.source(),
            base,
            &buffers,
            &name,
            MAX_TEXTURE_BYTES,
            &mut report,
        ) {
            Ok(data) => data,
            Err(error @ IoError::Limit(_)) => return Err(error),
            Err(error) => {
                let reference = match image.source() {
                    gltf::image::Source::Uri { uri, .. } => uri.to_owned(),
                    _ => name.clone(),
                };
                report.missing_textures.push(reference.clone());
                report.warnings.push(format!(
                    "Текстура {reference}: {error}; используются цвет и числовые параметры материала. Поместите изображение рядом с моделью или назначьте карту в свойствах материала."
                ));
                continue;
            }
        };
        texture.id = alloc();
        texture.name = name;
        image_ids.insert(image.index(), texture.id);
        project.textures.push(texture);
        report.record_texture_reductions(crate::texture::fit_texture_budget(
            &mut project.textures,
            MAX_TEXTURE_BYTES,
        )?);
    }
    let mut material_ids = BTreeMap::new();
    let mut material_uvsets = BTreeMap::new();
    for mat in doc.materials() {
        let id = alloc();
        material_ids.insert(mat.index(), id);
        let pbr = mat.pbr_metallic_roughness();
        let material_name = mat.name().unwrap_or("без имени");
        let color_info = pbr.base_color_texture();
        let mr_info = pbr.metallic_roughness_texture();
        let normal_info = mat.normal_texture();
        let ao_info = mat.occlusion_texture();
        let emission_info = mat.emissive_texture();
        let mappings = [
            color_info
                .as_ref()
                .map(texture_info_mapping)
                .unwrap_or_default(),
            mr_info
                .as_ref()
                .map(texture_info_mapping)
                .unwrap_or_default(),
            normal_info
                .as_ref()
                .map(|t| {
                    raw_texture_mapping(t.tex_coord(), t.extension_value("KHR_texture_transform"))
                })
                .transpose()?
                .unwrap_or_default(),
            ao_info
                .as_ref()
                .map(|t| {
                    raw_texture_mapping(t.tex_coord(), t.extension_value("KHR_texture_transform"))
                })
                .transpose()?
                .unwrap_or_default(),
            emission_info
                .as_ref()
                .map(texture_info_mapping)
                .unwrap_or_default(),
        ];
        if mappings.iter().any(|(_, mapping)| !mapping.valid()) {
            return invalid(format!(
                "Материал {material_name}: некорректное преобразование KHR_texture_transform"
            ));
        }
        let used = [
            color_info.is_some(),
            mr_info.is_some(),
            normal_info.is_some(),
            ao_info.is_some(),
            emission_info.is_some(),
        ];
        // A common alternate set is remapped to the editor's UV0 on the mesh.
        // Mixed sets retain the existing explicit per-map fallback warning.
        let uvsets: BTreeSet<_> = mappings
            .iter()
            .zip(used)
            .filter_map(|((set, _), used)| used.then_some(*set))
            .collect();
        let uvset = uvsets.first().copied().unwrap_or(0);
        material_uvsets.insert(mat.index(), uvset);
        let mut texture_samplers = [poseforge_core::TextureSampler::default(); 5];
        let mut texture_slot = |texture: gltf::texture::Texture<'_>, index: usize, slot: &str| {
            let tex_coord = mappings[index].0;
            if tex_coord != uvset {
                report.warnings.push(format!(
                    "Материал {material_name}, {slot}: TEXCOORD_{tex_coord} отличается от общего TEXCOORD_{uvset}; карта пропущена, используется числовой параметр"
                ));
                return None;
            }
            texture_samplers[index] = import_sampler(texture.sampler());
            // Missing maps retain authored factors. Never create a fake pink
            // texture or count a missing image as a successfully loaded asset.
            image_ids.get(&texture.source().index()).copied()
        };
        let base_color_texture =
            color_info.and_then(|t| texture_slot(t.texture(), 0, "Base color"));
        let metallic_roughness_texture =
            mr_info.and_then(|t| texture_slot(t.texture(), 1, "Metallic / Roughness"));
        let normal_texture = normal_info.and_then(|t| texture_slot(t.texture(), 2, "Normal"));
        let occlusion_texture = ao_info.and_then(|t| texture_slot(t.texture(), 3, "AO"));
        let emissive_texture = emission_info.and_then(|t| texture_slot(t.texture(), 4, "Emission"));
        project.materials.push(Material {
            id,
            name: mat.name().unwrap_or("Материал").into(),
            base_color: pbr.base_color_factor(),
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            ior: Material::default().ior,
            coat_weight: 0.,
            coat_roughness: Material::default().coat_roughness,
            emissive: mat
                .emissive_factor()
                .map(|v| v * mat.emissive_strength().unwrap_or(1.)),
            base_color_texture,
            metallic_roughness_texture,
            normal_texture,
            occlusion_texture,
            emissive_texture,
            texture_uvs: mappings.map(|(_, mapping)| mapping),
            texture_samplers,
            normal_scale: mat.normal_texture().map(|t| t.scale()).unwrap_or(1.),
            occlusion_strength: mat.occlusion_texture().map(|t| t.strength()).unwrap_or(1.),
            alpha_mode: match mat.alpha_mode() {
                gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => AlphaMode::Mask,
                gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            },
            alpha_cutoff: mat.alpha_cutoff().unwrap_or(0.5),
            double_sided: mat.double_sided(),
            transmission: 0.,
            thickness: 1.,
            attenuation_distance: 1.,
            ..Material::default()
        });
    }
    if doc
        .meshes()
        .any(|m| m.primitives().any(|p| p.material().index().is_none()))
    {
        let id = alloc();
        material_ids.insert(None, id);
        project.materials.push(Material {
            id,
            name: "Материал glTF по умолчанию".into(),
            base_color: [1.; 4],
            metallic: 1.,
            roughness: 1.,
            ..Material::default()
        });
    }
    let mut skin_limits = BTreeMap::<usize, usize>::new();
    for node in doc.nodes() {
        if let (Some(mesh), Some(skin)) = (node.mesh(), node.skin()) {
            skin_limits
                .entry(mesh.index())
                .and_modify(|limit| *limit = (*limit).min(skin.joints().len()))
                .or_insert_with(|| skin.joints().len());
        }
    }
    let mut primitive_ids: BTreeMap<usize, Vec<u64>> = BTreeMap::new();
    for source in doc.meshes() {
        for (primitive_index, primitive) in source.primitives().enumerate() {
            check()?;
            let reader = primitive.reader(|b| Some(&buffers[b.index()].0));
            let positions = reader
                .read_positions()
                .ok_or_else(|| IoError::Invalid("У примитива отсутствует POSITION".into()))?;
            report.vertices += positions.len();
            if report.vertices > MAX_VERTICES {
                return invalid("Превышен лимит импорта: 2 миллиона вершин");
            }
            let mut vertices: Vec<_> = collect_checked(
                positions.map(|position| Vertex {
                    position,
                    ..Vertex::default()
                }),
                cancelled,
            )?;
            if let Some(normals) = reader.read_normals() {
                if normals.len() != vertices.len() {
                    return invalid("Количество NORMAL не совпадает с POSITION");
                }
                for (index, (v, n)) in vertices.iter_mut().zip(normals).enumerate() {
                    if index % 4096 == 0 {
                        check()?;
                    }
                    v.normal = n;
                }
            }
            let uvset = material_uvsets
                .get(&primitive.material().index())
                .copied()
                .unwrap_or(0);
            if let Some(uvs) = reader.read_tex_coords(uvset) {
                let uvs = uvs.into_f32();
                if uvs.len() != vertices.len() {
                    return invalid("Количество TEXCOORD_0 не совпадает с POSITION");
                }
                for (index, (v, uv)) in vertices.iter_mut().zip(uvs).enumerate() {
                    if index % 4096 == 0 {
                        check()?;
                    }
                    v.uv = uv;
                }
            } else if uvset != 0 {
                return invalid(format!(
                    "Материал требует TEXCOORD_{uvset}, но у примитива нет этого набора UV"
                ));
            }
            if let Some(colors) = reader.read_colors(0) {
                let colors = colors.into_rgba_f32();
                if colors.len() != vertices.len() {
                    return invalid("Количество COLOR_0 не совпадает с POSITION");
                }
                for (index, (vertex, color)) in vertices.iter_mut().zip(colors).enumerate() {
                    if index % 4096 == 0 {
                        check()?;
                    }
                    if color
                        .iter()
                        .any(|c| !c.is_finite() || !(0. ..=1.).contains(c))
                    {
                        return invalid("COLOR_0 содержит нечисловой цвет или значение вне 0–1");
                    }
                    vertex.color = color;
                }
            }
            let reduced = crate::skin_weights::read(
                &primitive,
                &buffers,
                &mut vertices,
                skin_limits.get(&source.index()).copied(),
                cancelled,
            )?;
            if reduced > 0 {
                report.warnings.push(format!(
                    "{} / {}: для {reduced} вершин сохранены четыре самых сильных влияния костей, веса нормализованы; деформация может отличаться от исходной",
                    source.name().unwrap_or("Mesh"), primitive_index + 1,
                ));
            }
            let original_indices = if let Some(indices) = reader.read_indices() {
                collect_checked(indices.into_u32(), cancelled)?
            } else {
                collect_checked(0..vertices.len() as u32, cancelled)?
            };
            let indices = triangles(primitive.mode(), &original_indices, cancelled)?;
            if indices.iter().any(|&i| i as usize >= vertices.len()) {
                return invalid("Индекс треугольника выходит за число вершин");
            }
            if reader.read_normals().is_none() {
                generate_normals(&mut vertices, &indices, cancelled)?;
            }
            let mut morph_targets = Vec::new();
            for (index, (positions, normals, _)) in reader.read_morph_targets().enumerate() {
                if index >= MAX_MORPHS {
                    return invalid("Превышен лимит 64 morph targets на примитив");
                }
                morph_targets.push(MorphTarget {
                    name: format!("Morph {}", index + 1),
                    positions: positions
                        .map(|x| collect_checked(x, cancelled))
                        .transpose()?
                        .unwrap_or_else(|| vec![[0.; 3]; vertices.len()])
                        .into(),
                    normals: normals
                        .map(|x| collect_checked(x, cancelled))
                        .transpose()?
                        .unwrap_or_default()
                        .into(),
                });
            }
            let id = alloc();
            primitive_ids.entry(source.index()).or_default().push(id);
            project.meshes.push(Mesh {
                id,
                name: format!(
                    "{} / {}",
                    source.name().unwrap_or("Mesh"),
                    primitive_index + 1
                ),
                vertices: vertices.into(),
                indices: indices.into(),
                material: material_ids.get(&primitive.material().index()).copied(),
                topology: None,
                morph_targets,
            });
            progress(
                0.14 + 0.4 * (source.index() + 1) as f32 / doc.meshes().len().max(1) as f32,
                "Геометрия glTF",
            );
        }
    }
    check()?;
    progress(0.55, "Иерархия и скелеты");
    let mut parents = BTreeMap::new();
    for node in doc.nodes() {
        for child in node.children() {
            if parents.insert(child.index(), node.index()).is_some() {
                return invalid("У узла glTF несколько родителей");
            }
        }
    }
    let joint_ids: BTreeSet<_> = doc
        .skins()
        .flat_map(|s| s.joints().map(|j| j.index() as u64 + 1).collect::<Vec<_>>())
        .collect();
    let mut morph_nodes: BTreeMap<usize, Vec<u64>> = BTreeMap::new();
    for source in doc.nodes() {
        check()?;
        let (translation, rotation, scale) = source.transform().decomposed();
        let rotation = Quat::from_array(rotation);
        if !rotation.is_finite() || rotation.length_squared() < 1e-8 {
            return invalid("Некорректный кватернион узла glTF");
        }
        let transform = Transform {
            translation: Vec3::from_array(translation),
            rotation: rotation.normalize(),
            scale: Vec3::from_array(scale),
        };
        // Decomposing matrices with shear would silently alter the model.
        let original = Mat4::from_cols_array_2d(&source.transform().matrix());
        if original
            .to_cols_array()
            .iter()
            .zip(transform.matrix().to_cols_array())
            .any(|(a, b)| (a - b).abs() > 1e-3)
        {
            return invalid("Матрица узла содержит shear или неразложимую трансформацию; примените трансформации перед экспортом");
        }
        let id = source.index() as u64 + 1;
        let mut node = Node::new(id, source.name().unwrap_or("Объект"));
        node.parent = parents.get(&source.index()).map(|i| *i as u64 + 1);
        node.transform = transform;
        node.is_joint = joint_ids.contains(&id);
        let skin = if let Some(skin) = source.skin() {
            let joints: Vec<_> = skin.joints().map(|j| j.index() as u64 + 1).collect();
            let inverse_bind_matrices = skin
                .reader(|b| Some(&buffers[b.index()].0))
                .read_inverse_bind_matrices()
                .map(|i| {
                    i.take(joints.len())
                        .map(|m| Mat4::from_cols_array_2d(&m))
                        .collect()
                })
                .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);
            Some(Skin {
                joints,
                inverse_bind_matrices,
            })
        } else {
            None
        };
        if let Some(mesh) = source.mesh() {
            let meshes = primitive_ids
                .get(&mesh.index())
                .ok_or_else(|| IoError::Invalid("Mesh не содержит примитивов".into()))?;
            if meshes.is_empty() {
                return invalid("Mesh не содержит примитивов");
            }
            if skin.is_some() {
                for mesh_id in meshes {
                    let mesh = project.meshes.iter().find(|m| m.id == *mesh_id).unwrap();
                    if mesh
                        .vertices
                        .iter()
                        .any(|v| v.weights.iter().sum::<f32>() <= 1e-8)
                    {
                        return invalid("Skinned mesh содержит вершины без положительных весов");
                    }
                }
            }
            let weights = source
                .weights()
                .or_else(|| mesh.weights())
                .map(|w| w.to_vec())
                .unwrap_or_else(|| {
                    vec![
                        0.;
                        project
                            .meshes
                            .iter()
                            .find(|m| m.id == meshes[0])
                            .map(|m| m.morph_targets.len())
                            .unwrap_or(0)
                    ]
                });
            if meshes.len() == 1 {
                node.mesh = Some(meshes[0]);
                node.skin = skin;
                node.morph_weights = weights;
                morph_nodes.insert(source.index(), vec![id]);
            } else {
                for (index, &mesh_id) in meshes.iter().enumerate() {
                    let child_id = alloc();
                    let mut child = Node::new(child_id, format!("{} / {}", node.name, index + 1));
                    child.parent = Some(id);
                    child.mesh = Some(mesh_id);
                    child.skin = skin.clone();
                    child.morph_weights = weights.clone();
                    morph_nodes
                        .entry(source.index())
                        .or_default()
                        .push(child_id);
                    project.nodes.push(child);
                }
            }
        }
        project.nodes.push(node);
    }
    if doc.scenes().len() > 1 {
        report.warnings.push(
            "Файл содержит несколько сцен: в прототип импортированы все узлы документа".into(),
        );
    }
    project.nodes.sort_by_key(|n| n.id);
    progress(0.72, "Ключевые кадры");
    for animation in doc.animations() {
        check()?;
        let mut clip = Clip {
            name: animation
                .name()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Анимация {}", animation.index() + 1)),
            duration: 0.,
            channels: Vec::new(),
        };
        for channel in animation.channels() {
            check()?;
            let reader = channel.reader(|b| Some(&buffers[b.index()].0));
            let times: Vec<_> = collect_checked(
                reader
                    .read_inputs()
                    .ok_or_else(|| IoError::Invalid("Анимация без входных ключей".into()))?
                    .map(|v| v as f64),
                cancelled,
            )?;
            if times.is_empty() {
                continue;
            }
            clip.duration = clip.duration.max(*times.last().unwrap());
            let interpolation = match channel.sampler().interpolation() {
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            };
            let property = match channel.target().property() {
                gltf::animation::Property::Translation => Property::Translation,
                gltf::animation::Property::Rotation => Property::Rotation,
                gltf::animation::Property::Scale => Property::Scale,
                gltf::animation::Property::MorphTargetWeights => Property::Weights,
            };
            let factor = if matches!(interpolation, Interpolation::CubicSpline) {
                3
            } else {
                1
            };
            let values: Vec<Vec<f32>> = match reader
                .read_outputs()
                .ok_or_else(|| IoError::Invalid("Анимация без выходных ключей".into()))?
            {
                gltf::animation::util::ReadOutputs::Translations(v) => {
                    collect_checked(v.map(|a| a.to_vec()), cancelled)?
                }
                gltf::animation::util::ReadOutputs::Rotations(v) => {
                    collect_checked(v.into_f32().map(|a| a.to_vec()), cancelled)?
                }
                gltf::animation::util::ReadOutputs::Scales(v) => {
                    collect_checked(v.map(|a| a.to_vec()), cancelled)?
                }
                gltf::animation::util::ReadOutputs::MorphTargetWeights(v) => {
                    let flat = collect_checked(v.into_f32(), cancelled)?;
                    let groups = times.len() * factor;
                    if flat.is_empty() || !flat.len().is_multiple_of(groups) {
                        return invalid("Некорректное число morph-ключей");
                    }
                    collect_checked(
                        flat.chunks(flat.len() / groups).map(|x| x.to_vec()),
                        cancelled,
                    )?
                }
            };
            if values.len() != times.len() * factor {
                return invalid("Количество ключей не соответствует интерполяции");
            }
            let keys: Vec<_> = collect_checked(
                times.into_iter().enumerate().map(|(i, time)| {
                    if factor == 3 {
                        Keyframe {
                            time,
                            value: values[i * 3 + 1].clone(),
                            in_tangent: Some(values[i * 3].clone()),
                            out_tangent: Some(values[i * 3 + 2].clone()),
                        }
                    } else {
                        Keyframe {
                            time,
                            value: values[i].clone(),
                            in_tangent: None,
                            out_tangent: None,
                        }
                    }
                }),
                cancelled,
            )?;
            let targets = if matches!(property, Property::Weights) {
                morph_nodes
                    .get(&channel.target().node().index())
                    .cloned()
                    .ok_or_else(|| {
                        IoError::Invalid("Morph-анимация ссылается на узел без меша".into())
                    })?
            } else {
                vec![channel.target().node().index() as u64 + 1]
            };
            for node in targets {
                clip.channels.push(Channel {
                    node,
                    property,
                    interpolation,
                    keys: keys.clone(),
                });
            }
        }
        project.duration = project.duration.max(clip.duration);
        project.clips.push(clip);
    }
    // KHR_lights_punctual instances inherit their node's world position and
    // local -Z direction. PoseForge stores a static light setup; keep an honest
    // warning when an animated hierarchy would otherwise imply moving lights.
    let globals = project
        .global_transforms()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    for node in doc.nodes() {
        if let Some(source) = node.light() {
            let transform = globals[&(node.index() as u64 + 1)];
            let (kind, inner_cone_angle, outer_cone_angle) = match source.kind() {
                gltf::khr_lights_punctual::Kind::Directional => {
                    (LightKind::Directional, 0., std::f32::consts::FRAC_PI_4)
                }
                gltf::khr_lights_punctual::Kind::Point => {
                    (LightKind::Point, 0., std::f32::consts::FRAC_PI_4)
                }
                gltf::khr_lights_punctual::Kind::Spot {
                    inner_cone_angle,
                    outer_cone_angle,
                } => (LightKind::Spot, inner_cone_angle, outer_cone_angle),
            };
            let direction = transform
                .transform_vector3(-Vec3::Z)
                .try_normalize()
                .ok_or_else(|| {
                    IoError::Invalid("Свет glTF имеет вырожденное направление".into())
                })?;
            project.lights.push(Light {
                curve_binding: None,
                target: None,
                name: node
                    .name()
                    .or_else(|| source.name())
                    .unwrap_or("Свет glTF")
                    .into(),
                kind,
                position: transform.transform_point3(Vec3::ZERO),
                direction,
                color: source.color(),
                intensity: source.intensity(),
                range: source.range().unwrap_or(0.),
                area_size: [1., 1.],
                area_rotation: 0.,
                temperature_kelvin: 6500.,
                use_temperature: false,
                inner_cone_angle,
                outer_cone_angle,
                enabled: true,
                casts_shadow: true,
                volume: Default::default(),
            });
            if project.lights.len() > 8 {
                return invalid(
                    "Файл содержит более 8 источников света; уменьшите их число перед импортом",
                );
            }
        }
    }
    if !project.lights.is_empty() && !project.clips.is_empty() {
        report.warnings.push("Источники света импортированы в исходном положении; анимация освещения пока не переносится".into());
    }
    // Preserve every glTF camera as an ordinary transform node. The first
    // camera is the initial render view; later camera switching uses stable IDs.
    for node in doc.nodes().filter(|n| n.camera().is_some()) {
        let id = node.index() as u64 + 1;
        let m = globals[&id];
        let eye = m.transform_point3(Vec3::ZERO);
        let mut c = Camera {
            eye,
            target: eye + m.transform_vector3(-Vec3::Z),
            up: m.transform_vector3(Vec3::Y).normalize(),
            ..Camera::default()
        };
        match node.camera().unwrap().projection() {
            gltf::camera::Projection::Perspective(p) => {
                c.fov_y_radians = p.yfov();
                c.near = p.znear();
                c.far = p.zfar().unwrap_or(10000.);
            }
            gltf::camera::Projection::Orthographic(o) => {
                c.orthographic = true;
                c.ortho_size = o.ymag();
                c.near = o.znear().max(0.001);
                c.far = o.zfar();
            }
        }
        project.node_mut(id).unwrap().camera = Some(poseforge_core::CameraLens::from_camera(&c));
        if project.active_camera.is_none() {
            project.camera = c;
            project.active_camera = Some(id);
        }
    }
    if project.active_camera.is_none() {
        frame_project(&mut project)?;
    }
    check()?;
    progress(0.94, "Проверка проекта");
    project
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    report.meshes = project.meshes.len();
    report.materials = project.materials.len();
    report.textures = project.textures.len();
    report.lights = project.lights.len();
    report.joints = joint_ids.len();
    report.clips = project.clips.len();
    report.finish_texture_report(&project.textures, MAX_TEXTURE_BYTES);
    report.warnings.sort();
    report.warnings.dedup();
    progress(1., "Готово");
    Ok(ImportResult { project, report })
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(IoError::Invalid(message.into()))
}
fn check_uri(uri: &str, base: &Path) -> Result<Option<u64>> {
    if uri.starts_with("data:") {
        let encoded = uri.split_once(',').map_or("", |(_, bytes)| bytes);
        let padding = encoded
            .bytes()
            .rev()
            .take_while(|&byte| byte == b'=')
            .count();
        let size = (encoded.len() as u64).div_ceil(4) * 3 - padding.min(2) as u64;
        return Ok(Some(size));
    }
    let decoded = urlencoding::decode(uri)
        .map_err(|_| IoError::Invalid("Некорректный URI ресурса glTF".into()))?;
    let local = decoded.replace('\\', "/");
    if local.contains(':') || local.starts_with('/') || local.contains('\0') {
        return invalid("Поддерживаются встроенные данные и относительные пути glTF; сетевые URI и file: запрещены");
    }
    let path = base.join(decoded.as_ref());
    // Mandatory geometry buffers never fall back to unrelated nearby files.
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() > MAX_MODEL_BYTES {
            return invalid("Внешний ресурс glTF превышает 2 ГиБ");
        }
        return Ok(Some(metadata.len()));
    }
    Ok(None)
}
fn import_sampler(sampler: gltf::texture::Sampler<'_>) -> poseforge_core::TextureSampler {
    use gltf::texture::{MagFilter, MinFilter, WrappingMode};
    use poseforge_core::{
        TextureMagFilter as Mag, TextureMinFilter as Min, TextureSampler, TextureWrap,
    };
    let wrap = |mode| match mode {
        WrappingMode::Repeat => TextureWrap::Repeat,
        WrappingMode::MirroredRepeat => TextureWrap::MirroredRepeat,
        WrappingMode::ClampToEdge => TextureWrap::ClampToEdge,
    };
    TextureSampler {
        wrap_s: wrap(sampler.wrap_s()),
        wrap_t: wrap(sampler.wrap_t()),
        mag_filter: match sampler.mag_filter() {
            Some(MagFilter::Nearest) => Mag::Nearest,
            _ => Mag::Linear,
        },
        min_filter: match sampler.min_filter() {
            Some(MinFilter::Nearest) => Min::Nearest,
            Some(MinFilter::Linear) => Min::Linear,
            Some(MinFilter::NearestMipmapNearest) => Min::NearestMipmapNearest,
            Some(MinFilter::LinearMipmapNearest) => Min::LinearMipmapNearest,
            Some(MinFilter::NearestMipmapLinear) => Min::NearestMipmapLinear,
            Some(MinFilter::LinearMipmapLinear) | None => Min::LinearMipmapLinear,
        },
    }
}

fn texture_info_mapping(info: &gltf::texture::Info<'_>) -> (u32, TextureUvTransform) {
    info.texture_transform().map_or(
        (info.tex_coord(), TextureUvTransform::default()),
        |transform| {
            (
                transform.tex_coord().unwrap_or(info.tex_coord()),
                TextureUvTransform {
                    offset: transform.offset(),
                    rotation: transform.rotation(),
                    scale: transform.scale(),
                },
            )
        },
    )
}

fn raw_texture_mapping(
    tex_coord: u32,
    value: Option<&serde_json::Value>,
) -> Result<(u32, TextureUvTransform)> {
    // gltf 1.4 exposes typed transforms on texture::Info only; normal and AO
    // retain their extension under the generic JSON extension map.
    #[derive(Deserialize)]
    struct Mapping {
        #[serde(flatten)]
        transform: TextureUvTransform,
        #[serde(default, rename = "texCoord")]
        tex_coord: Option<u32>,
    }
    let Some(value) = value else {
        return Ok((tex_coord, TextureUvTransform::default()));
    };
    let mapping: Mapping = serde_json::from_value(value.clone())
        .map_err(|e| IoError::Invalid(format!("Некорректный KHR_texture_transform: {e}")))?;
    Ok((mapping.tex_coord.unwrap_or(tex_coord), mapping.transform))
}

fn triangles(mode: gltf::mesh::Mode, indices: &[u32], cancel: &AtomicBool) -> Result<Vec<u32>> {
    match mode {
        gltf::mesh::Mode::Triangles => collect_checked(indices.iter().copied(), cancel),
        gltf::mesh::Mode::TriangleStrip => collect_checked(
            (2..indices.len()).flat_map(|i| {
                if i % 2 == 0 {
                    [indices[i - 2], indices[i - 1], indices[i]]
                } else {
                    [indices[i - 1], indices[i - 2], indices[i]]
                }
            }),
            cancel,
        ),
        gltf::mesh::Mode::TriangleFan => collect_checked(
            (2..indices.len()).flat_map(|i| [indices[0], indices[i - 1], indices[i]]),
            cancel,
        ),
        _ => invalid("Импортируются треугольники, strip и fan; преобразуйте линии и точки в меш"),
    }
}
fn generate_normals(vertices: &mut [Vertex], indices: &[u32], cancel: &AtomicBool) -> Result<()> {
    let mut normals = vec![Vec3::ZERO; vertices.len()];
    for (index, t) in indices.as_chunks::<3>().0.iter().enumerate() {
        if index % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(IoError::Cancelled);
        }
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let n = (Vec3::from_array(vertices[b].position) - Vec3::from_array(vertices[a].position))
            .cross(Vec3::from_array(vertices[c].position) - Vec3::from_array(vertices[a].position));
        for i in [a, b, c] {
            normals[i] += n;
        }
    }
    for (v, n) in vertices.iter_mut().zip(normals) {
        v.normal = n.try_normalize().unwrap_or(Vec3::Y).to_array();
    }
    Ok(())
}
fn collect_checked<T>(items: impl Iterator<Item = T>, cancel: &AtomicBool) -> Result<Vec<T>> {
    // Growth is bounded by the preflight counts; cancellation is also checked
    // within a single large accessor rather than only between meshes.
    let mut out = Vec::new();
    for (index, item) in items.enumerate() {
        if index % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(IoError::Cancelled);
        }
        out.push(item);
    }
    Ok(out)
}
fn decode_gltf_image(
    source: gltf::image::Source<'_>,
    base: &Path,
    buffers: &[gltf::buffer::Data],
    name: &str,
    budget: usize,
    report: &mut ImportReport,
) -> Result<Texture> {
    use crate::texture::{decode_texture, read_image, MAX_IMAGE_FILE};
    match source {
        gltf::image::Source::View { view, .. } => {
            let data = &buffers[view.buffer().index()].0;
            decode_texture(
                &data[view.offset()..view.offset() + view.length()],
                name,
                budget as u64,
            )
        }
        gltf::image::Source::Uri { uri, .. } if uri.starts_with("data:") => {
            let payload = uri
                .split_once(',')
                .map(|(_, payload)| payload)
                .ok_or_else(|| IoError::Invalid("Некорректный data URI текстуры".into()))?;
            if (payload.len() as u64).saturating_mul(3) / 4 > MAX_IMAGE_FILE {
                return Err(IoError::Limit("Изображение превышает 64 МиБ".into()));
            }
            let bytes =
                gltf::buffer::Data::from_source(gltf::buffer::Source::Uri(uri), Some(base))?;
            decode_texture(&bytes.0, name, budget as u64)
        }
        gltf::image::Source::Uri { uri, .. } => {
            let resource =
                crate::obj_resource::resolve_image_uri(base, uri).map_err(IoError::Invalid)?;
            if resource.relocated {
                report.warnings.push(format!(
                    "Текстура glTF {uri} найдена по новому пути: {}",
                    resource.path.display()
                ));
            }
            decode_texture(
                &read_image(&resource.path)?,
                &resource.path.to_string_lossy(),
                budget as u64,
            )
        }
    }
}
pub(crate) fn frame_project(project: &mut Project) -> Result<()> {
    let globals = project
        .global_transforms()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for node in &project.nodes {
        if let Some(mesh) = node.mesh.and_then(|m| project.mesh(m)) {
            let m = globals[&node.id];
            for v in &mesh.vertices {
                let p = m.transform_point3(Vec3::from_array(v.position));
                min = min.min(p);
                max = max.max(p);
            }
        }
    }
    if min.is_finite() && max.is_finite() {
        let center = (min + max) * 0.5;
        let radius = (max - min).length().max(0.1) * 0.5;
        project.camera.target = center;
        project.camera.eye = center + Vec3::new(1.2, 0.7, 1.8).normalize() * radius * 3.2;
        project.camera.near = (radius / 1000.).max(0.001);
        project.camera.far = (radius * 100.).max(100.);
        project.camera.ortho_size = radius * 1.25;
    }
    Ok(())
}
