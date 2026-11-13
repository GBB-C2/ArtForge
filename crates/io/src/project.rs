use crate::{IoError, Result};
use poseforge_core::{Project, SCHEMA_VERSION};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_PROJECT_BYTES: u64 = 512 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The JSON is self-contained, including embedded geometry and RGBA textures.
/// Write and sync a unique sibling first, then atomically rename it. A failed
/// serialization or write cannot truncate the user's previous project.
pub fn save_project(project: &Project, path: impl AsRef<Path>) -> Result<()> {
    save_project_limited(project, path.as_ref(), MAX_PROJECT_BYTES)
}
fn save_project_limited(project: &Project, path: &Path, limit: u64) -> Result<()> {
    project
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| IoError::Invalid("Укажите имя файла проекта".into()))?;
    let temp = parent.join(format!(
        ".{}.{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, project)?;
        writer.flush()?;
        if writer.get_ref().metadata()?.len() > limit {
            return Err(IoError::Invalid("Проект превышает лимит чтения 512 МиБ; уменьшите кэш или встроенные изображения. Предыдущий файл сохранён.".into()));
        }
        writer.get_ref().sync_all()?;
        drop(writer);
        // Rust maps this to a replace-existing rename on Windows. A same-dir
        // temporary avoids a cross-filesystem rename and keeps failure safe.
        fs::rename(&temp, path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub fn load_project(path: impl AsRef<Path>) -> Result<Project> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_PROJECT_BYTES {
        return Err(IoError::Invalid(
            "Проект превышает лимит прототипа 512 МиБ".into(),
        ));
    }
    migrate_loaded_project(serde_json::from_reader(BufReader::new(file))?)
}

/// Decode embedded or already-read project data through the same migration and
/// validation path as files. Loading never rewrites the supplied data.
pub fn load_project_bytes(bytes: &[u8]) -> Result<Project> {
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err(IoError::Invalid("Проект превышает лимит прототипа 512 МиБ".into()));
    }
    migrate_loaded_project(serde_json::from_slice(bytes)?)
}

fn migrate_loaded_project(mut project: Project) -> Result<Project> {
    match project.version {
        // Schema 2 adds persistent rig mappings; schema 3 adds PBR maps,
        // multiple light types and render settings; schema 4 adds camera nodes;
        // schema 5 adds artwork/physics/layers; schema 6 adds fiber/anisotropy
        // and an optional independent absorption color (None retains legacy tint).
        // Schema 29 adds mesh shadow-only visibility, defaulting to false.
        // Schema 30 adds optional independent curve bindings to nodes and lights.
        // Schema 31 adds enabled-by-default mesh collision switches to fluid/fracture.
        // Schema 32 adds optional explicit collider lists. Missing lists preserve
        // legacy scene-collider membership; an empty explicit list means none.
        // Schema 33 adds editable text and per-cloth collision lists. Missing cloth
        // lists keep the legacy scene colliders; old geometry is unchanged.
        // Serde supplies defaults
        // that preserve the meaning of existing materials and lights.
        // Migration is in memory only, never an implicit rewrite of the input.
        // Schema 36 adds helper shapes and object-owned force fields.
        // Schema 37 adds FX volumes, interior materials and opt-in shard contacts.
        // Schema 38 adds FX shape, drift, smoke and opt-in scene illumination.
        // Schema 39 adds FX sources, force response and explicit mesh obstacles.
        // Schema 40 adds cloud/burst modes to the four FX kinds and explicit burst timing.
        // Schema 41 adds opt-in gas simulation, quality and four-point time profiles.
        // Schema 42 adds explicit combustion settings; absent Pyro preserves the old solver.
        // Schema 43 adds opt-in artistic surfaces; absent settings are disabled.
        // Schema 44 adds non-destructive per-object modifier stacks; absent stacks are empty.
        1..=43 => {
            // Earlier projects scattered every source in their shared fog.
            if project.version <= 21 && project.render_settings.atmosphere.density > 0. {
                for light in &mut project.lights { light.volume.enabled = true; }
            }
            project.migrate_legacy_curves();
            project.version = SCHEMA_VERSION;
        },
        SCHEMA_VERSION => {}
        version => {
            return Err(IoError::Invalid(format!(
                "Версия проекта {version} не поддерживается. Поддерживаются 1–{SCHEMA_VERSION}."
            )));
        }
    }
    project
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    Ok(project)
}

/// Merge prepared data in one transaction. Incoming IDs are remapped even when
/// importing the same file twice; its nodes, skin joints, channels and rig
/// profiles remain independent. Validation precedes destination replacement.
pub fn merge_project(destination: &mut Project, incoming: Project) -> Result<Vec<u64>> {
    destination
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    incoming
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    let mut candidate = destination.clone();
    let mut next = candidate
        .nodes
        .iter()
        .map(|x| x.id)
        .chain(candidate.meshes.iter().map(|x| x.id))
        .chain(candidate.materials.iter().map(|x| x.id))
        .chain(candidate.textures.iter().map(|x| x.id))
        .chain(candidate.collections.iter().map(|x| x.id))
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| IoError::Invalid("Закончились идентификаторы ресурсов".into()))?;
    fn ids(source: impl Iterator<Item = u64>, next: &mut u64) -> Result<BTreeMap<u64, u64>> {
        source
            .map(|id| {
                let allocated = *next;
                *next = next
                    .checked_add(1)
                    .ok_or_else(|| IoError::Invalid("Закончились идентификаторы".into()))?;
                Ok((id, allocated))
            })
            .collect()
    }
    let nodes = ids(incoming.nodes.iter().map(|x| x.id), &mut next)?;
    let meshes = ids(incoming.meshes.iter().map(|x| x.id), &mut next)?;
    let materials = ids(incoming.materials.iter().map(|x| x.id), &mut next)?;
    let textures = ids(incoming.textures.iter().map(|x| x.id), &mut next)?;
    let collections = ids(incoming.collections.iter().map(|x| x.id), &mut next)?;
    let roots = incoming
        .nodes
        .iter()
        .filter(|n| n.parent.is_none())
        .map(|n| nodes[&n.id])
        .collect();
    for mut texture in incoming.textures {
        texture.id = textures[&texture.id];
        candidate.textures.push(texture);
    }
    for mut video in incoming.video_textures {
        video.texture = textures[&video.texture];
        candidate.video_textures.push(video);
    }
    for mut material in incoming.materials {
        material.id = materials[&material.id];
        material.base_color_texture = material.base_color_texture.map(|id| textures[&id]);
        material.metallic_roughness_texture =
            material.metallic_roughness_texture.map(|id| textures[&id]);
        material.normal_texture = material.normal_texture.map(|id| textures[&id]);
        material.occlusion_texture = material.occlusion_texture.map(|id| textures[&id]);
        material.emissive_texture = material.emissive_texture.map(|id| textures[&id]);
        if let Some(graph) = &mut material.graph {
            for node in &mut graph.nodes {
                node.texture = node.texture.map(|id| textures[&id]);
            }
        }
        candidate.materials.push(material);
    }
    for mut mesh in incoming.meshes {
        mesh.id = meshes[&mesh.id];
        mesh.material = mesh.material.map(|id| materials[&id]);
        if let Some(t) = &mut mesh.topology {
            for material in &mut t.face_materials {
                *material = material.map(|id| materials[&id]);
            }
        }
        candidate.meshes.push(mesh);
    }
    for mut node in incoming.nodes {
        node.id = nodes[&node.id];
        node.parent = node.parent.map(|id| nodes[&id]);
        node.collision_targets.remap(&nodes);
        if let Some(fx)=&mut node.fx {fx.source=fx.source.map(|id|nodes[&id]);}
        if let Some(binding) = &mut node.curve_binding { binding.curve = nodes[&binding.curve]; }
        node.mesh = node.mesh.map(|id| meshes[&id]);
        for layer in &mut node.material_layers { layer.material = materials[&layer.material]; }
        if let Some(settings) = &mut node.destruction {
            settings.result = settings.result.map(|id| nodes[&id]);
            settings.interior_material = settings.interior_material.map(|id| materials[&id]);
        }
        if let Some(skin) = &mut node.skin {
            for joint in &mut skin.joints {
                *joint = nodes[joint];
            }
        }
        candidate.nodes.push(node);
    }
    let light_offset = candidate.lights.len();
    for mut collection in incoming.collections {
        for index in &mut collection.light_members {
            *index += light_offset;
        }
        collection.id = collections[&collection.id];
        collection.parent = collection.parent.map(|id| collections[&id]);
        for id in &mut collection.members {
            *id = nodes[id];
        }
        candidate.collections.push(collection);
    }
    let clip_offset = candidate.clips.len();
    candidate
        .animation_layers
        .extend(incoming.animation_layers.into_iter().map(|mut l| {
            l.clip += clip_offset;
            l
        }));
    for mut clip in incoming.clips {
        for channel in &mut clip.channels {
            channel.node = nodes[&channel.node];
        }
        candidate.clips.push(clip);
    }
    for mut pose in incoming.poses {
        pose.transforms = pose
            .transforms
            .into_iter()
            .map(|(id, transform)| {
                nodes
                    .get(&id)
                    .copied()
                    .map(|new| (new, transform))
                    .ok_or_else(|| {
                        IoError::Invalid("Поза ссылается на отсутствующий объект".into())
                    })
            })
            .collect::<Result<_>>()?;
        candidate.poses.push(pose);
    }
    for mut profile in incoming.rig_profiles {
        profile.root = profile.root.map(|id| nodes[&id]);
        for id in profile.assignments.values_mut() {
            *id = nodes[&*id];
        }
        candidate.rig_profiles.push(profile);
    }
    candidate.duration = candidate.duration.max(incoming.duration);
    // Retain every imported light. Validation rejects a ninth enabled light
    // before replacing the destination; nothing is silently truncated.
    // Camera and render settings remain those of the destination scene.
    candidate
        .outliner
        .order
        .extend(incoming.outliner.order.into_iter().filter_map(|item| {
            use poseforge_core::SceneItem as S;
            match item {
                S::Node(id) => nodes.get(&id).map(|id| S::Node(*id)),
                S::Folder(id) => collections.get(&id).map(|id| S::Folder(*id)),
                S::Light(index) => Some(S::Light(index + light_offset)),
            }
        }));
    for (id, order) in incoming.outliner.materials {
        if let Some(node) = nodes.get(&id) {
            candidate.outliner.materials.insert(
                *node,
                order
                    .iter()
                    .filter_map(|m| materials.get(m).copied())
                    .collect(),
            );
        }
    }
    for mut light in incoming.lights {
        if let Some(binding) = &mut light.curve_binding { binding.curve = nodes[&binding.curve]; }
        if let Some(target) = &mut light.target {
            target.node = nodes[&target.node];
        }
        candidate.lights.push(light);
    }
    candidate.clean_outliner();
    candidate
        .validate()
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    *destination = candidate;
    Ok(roots)
}

#[cfg(test)]
mod save_limit_tests {
    use super::*;
    #[test]
    fn oversized_write_keeps_previous_file_and_removes_temporary() {
        let folder =
            std::env::temp_dir().join(format!("artforge-size-limit-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("keep.artforge");
        fs::write(&path, b"original").unwrap();
        assert!(save_project_limited(&Project::default(), &path, 16).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }
}
