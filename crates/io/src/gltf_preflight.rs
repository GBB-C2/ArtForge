//! Bound expanded data before reading buffers or allocating accessor vectors.
use crate::{IoError, Result};
use gltf::{animation::Property, mesh::Mode, Semantic};
use poseforge_core::{MAX_SKIN_JOINTS, MAX_SKIN_MATRICES};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn preflight(doc: &gltf::Document, cancel: &AtomicBool) -> Result<()> {
    let limit = |message: &str| IoError::Limit(message.into());
    let mut vertices = 0u64;
    let mut triangles = 0u64;
    let mut morph_bytes = 0u64;
    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            if cancel.load(Ordering::Relaxed) {
                return Err(IoError::Cancelled);
            }
            let count = primitive
                .get(&Semantic::Positions)
                .ok_or_else(|| IoError::Invalid("У примитива отсутствует POSITION".into()))?
                .count();
            vertices = vertices.saturating_add(count as u64);
            if vertices > 2_000_000 {
                return Err(limit("Превышен лимит импорта: 2 миллиона вершин"));
            }
            if primitive
                .attributes()
                .filter(|(s, _)| matches!(s, Semantic::Joints(_)))
                .count()
                > crate::skin_weights::MAX_INFLUENCE_SETS
                || primitive
                    .attributes()
                    .filter(|(s, _)| matches!(s, Semantic::Weights(_)))
                    .count()
                    > crate::skin_weights::MAX_INFLUENCE_SETS
            {
                return Err(limit("Превышен лимит 32 наборов JOINTS/WEIGHTS"));
            }
            for (_, accessor) in primitive.attributes() {
                if accessor.count() != count {
                    return Err(IoError::Invalid(
                        "Количество атрибутов не совпадает с POSITION".into(),
                    ));
                }
            }
            let indices = primitive.indices().map_or(count, |a| a.count()) as u64;
            let expanded =
                match primitive.mode() {
                    Mode::Triangles if indices.is_multiple_of(3) => indices / 3,
                    Mode::TriangleStrip | Mode::TriangleFan => indices.saturating_sub(2),
                    Mode::Triangles => {
                        return Err(IoError::Invalid("Число индексов не кратно трём".into()))
                    }
                    _ => return Err(IoError::Invalid(
                        "Импортируются треугольники, strip и fan; преобразуйте линии и точки в меш"
                            .into(),
                    )),
                };
            triangles = triangles.saturating_add(expanded);
            if triangles > 4_000_000 {
                return Err(limit("Превышен лимит 4 миллиона треугольников"));
            }
            for (index, target) in primitive.morph_targets().enumerate() {
                if index >= 64 {
                    return Err(limit("Превышен лимит 64 morph targets на примитив"));
                }
                let normals = target.normals();
                // Missing deltas are materialized as zero positions too.
                morph_bytes = morph_bytes
                    .saturating_add(count as u64 * 12 * (1 + u64::from(normals.is_some())));
                if morph_bytes > 256 * 1024 * 1024 {
                    return Err(limit("Morph targets превышают лимит 256 МиБ"));
                }
                for accessor in [target.positions(), normals, target.tangents()]
                    .into_iter()
                    .flatten()
                {
                    if accessor.count() != count {
                        return Err(IoError::Invalid(
                            "Количество morph-дельт не совпадает с POSITION".into(),
                        ));
                    }
                }
            }
        }
    }
    let mut nodes = doc.nodes().len();
    let mut skin_matrices = 0usize;
    for node in doc.nodes() {
        if cancel.load(Ordering::Relaxed) {
            return Err(IoError::Cancelled);
        }
        if let Some(mesh) = node.mesh() {
            let count = mesh.primitives().len();
            if count > 1 {
                nodes = nodes.saturating_add(count);
            }
        }
        if let Some(skin) = node.skin() {
            let copies = node.mesh().map_or(1, |m| m.primitives().len().max(1));
            skin_matrices =
                skin_matrices.saturating_add(skin.joints().len().saturating_mul(copies));
            if skin_matrices > MAX_SKIN_MATRICES {
                return Err(limit("Суммарные матрицы skin превышают бюджет 64 МиБ"));
            }
        }
    }
    if nodes > 10_000 {
        return Err(limit("Превышен лимит 10 000 узлов с учётом примитивов"));
    }
    for skin in doc.skins() {
        if skin.joints().len() > MAX_SKIN_JOINTS {
            return Err(limit("Превышен лимит 65 536 суставов в skin"));
        }
        if skin
            .inverse_bind_matrices()
            .is_some_and(|a| a.count() < skin.joints().len())
        {
            return Err(IoError::Invalid(
                "Количество bind-матриц меньше числа суставов".into(),
            ));
        }
    }
    let mut keys = 0u64;
    let mut value_bytes = 0u64;
    for animation in doc.animations() {
        for channel in animation.channels() {
            if cancel.load(Ordering::Relaxed) {
                return Err(IoError::Cancelled);
            }
            let sampler = channel.sampler();
            let input = sampler.input().count() as u64;
            let factor = if sampler.interpolation() == gltf::animation::Interpolation::CubicSpline {
                3
            } else {
                1
            };
            let mut copies = 1u64;
            let mut components = 1u64;
            if channel.target().property() == Property::MorphTargetWeights {
                let mesh = channel.target().node().mesh().ok_or_else(|| {
                    IoError::Invalid("Morph-анимация ссылается на узел без меша".into())
                })?;
                copies = mesh.primitives().len() as u64;
                components = mesh
                    .primitives()
                    .next()
                    .map_or(0, |p| p.morph_targets().count()) as u64;
                if components == 0 {
                    return Err(IoError::Invalid("Morph-анимация без morph targets".into()));
                }
            }
            keys = keys.saturating_add(input.saturating_mul(copies));
            if keys > 1_000_000 {
                return Err(limit("Превышен лимит 1 миллион ключевых кадров"));
            }
            let output = sampler.output();
            let expected = input.saturating_mul(factor).saturating_mul(components);
            if output.count() as u64 != expected {
                return Err(IoError::Invalid(
                    "Количество ключей не соответствует интерполяции".into(),
                ));
            }
            value_bytes = value_bytes.saturating_add(
                (output.count() as u64)
                    .saturating_mul(output.dimensions().multiplicity() as u64 * 4)
                    .saturating_mul(copies),
            );
            if value_bytes > 256 * 1024 * 1024 {
                return Err(limit("Значения анимации превышают лимит 256 МиБ"));
            }
        }
    }
    Ok(())
}
