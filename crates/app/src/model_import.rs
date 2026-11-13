//! Expensive import finishing stays on the worker; the UI only publishes the
//! validated snapshot and camera bounds as one reversible edit.
use glam::Vec3;
use poseforge_core::{PreparedProjectEdit, Project};
use poseforge_io::{ImportResult, IoError};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct PreparedImport {
    pub edit: PreparedProjectEdit,
    pub roots: Vec<u64>,
    pub framing: Option<(Vec3, f32)>,
    pub has_skeleton: bool,
    pub report: String,
}

pub(super) fn prepare_import(
    before: Project,
    imported: ImportResult,
    clip: Option<usize>,
    time: f64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f32, &str),
) -> Result<PreparedImport, IoError> {
    let check = || {
        if cancel.load(Ordering::Relaxed) {
            Err(IoError::Cancelled)
        } else {
            Ok(())
        }
    };
    check()?;
    let warning_count = imported.report.warnings.len() + imported.report.missing_textures.len();
    let heading = if warning_count == 0 {
        "Импорт".to_owned()
    } else {
        format!("Импорт с предупреждениями ({warning_count})")
    };
    let has_skeleton = imported.project.nodes.iter().any(|node| node.is_joint);
    let mut report = format!(
        "{heading}: {} мешей, {} суставов, {} материалов, {} текстур, {} клипов, {} источников света.\n{}",
        imported.report.meshes, imported.report.joints, imported.report.materials,
        imported.report.textures, imported.report.clips, imported.project.lights.len(),
        [imported.report.warnings.join("\n"), imported.report.missing_textures.join("\n")].join("\n")
    );
    if !has_skeleton {
        report.push_str("\nСкелета в импортируемой модели нет. OBJ не хранит кости и веса. Для переноса скелета используйте GLB/glTF с skin или .blend с привязанной к арматуре моделью.");
    }
    progress(0.91, "Объединение сцены");
    let mut after = before.clone();
    let roots = poseforge_io::merge_project(&mut after, imported.project)?;
    check()?;
    progress(0.95, "Подготовка вида");
    let evaluated = poseforge_core::evaluate(&after, clip, time, &BTreeMap::new())
        .map_err(|e| IoError::Invalid(e.to_string()))?;
    let meshes: BTreeMap<_, _> = after.meshes.iter().map(|mesh| (mesh.id, mesh)).collect();
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for node in &after.nodes {
        check()?;
        let Some(global) = evaluated.global.get(&node.id) else {
            continue;
        };
        if let Some(mesh) = node.mesh.and_then(|id| meshes.get(&id)) {
            for (i, vertex) in mesh.vertices.iter().enumerate() {
                if i % 4096 == 0 {
                    check()?;
                }
                let point = global.transform_point3(Vec3::from_array(vertex.position));
                min = min.min(point);
                max = max.max(point);
            }
        } else {
            min = min.min(global.w_axis.truncate());
            max = max.max(global.w_axis.truncate());
        }
    }
    let framing = (min.is_finite() && max.is_finite())
        .then(|| ((min + max) * 0.5, (max - min).length().max(0.5) * 1.8));
    progress(0.98, "Проверка объединённой сцены");
    let edit =
        PreparedProjectEdit::new(before, after).map_err(|e| IoError::Invalid(e.to_string()))?;
    check()?;
    Ok(PreparedImport {
        edit,
        roots,
        framing,
        has_skeleton,
        report,
    })
}
