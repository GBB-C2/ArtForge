//! Convert interchange skin weights to the editor's four-influence vertices.
//! Iterate all sets together, with bounded scratch space and no per-vertex allocation.
use crate::{IoError, Result};
use poseforge_core::Vertex;
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

pub(crate) const MAX_INFLUENCE_SETS: usize = 32;

pub(crate) fn read(
    primitive: &gltf::Primitive<'_>,
    buffers: &[gltf::buffer::Data],
    vertices: &mut [Vertex],
    joint_limit: Option<usize>,
    cancel: &AtomicBool,
) -> Result<usize> {
    let sets: BTreeSet<_> = primitive
        .attributes()
        .filter_map(|(semantic, _)| match semantic {
            gltf::Semantic::Joints(s) | gltf::Semantic::Weights(s) => Some(s),
            _ => None,
        })
        .collect();
    if sets.is_empty() {
        if joint_limit.is_some() {
            return Err(IoError::Invalid("У skinned mesh отсутствуют JOINTS/WEIGHTS; экспортируйте веса привязки вместе со скелетом".into()));
        }
        return Ok(0);
    }
    if sets.len() > MAX_INFLUENCE_SETS {
        return Err(IoError::Limit(
            "Превышен лимит 32 наборов JOINTS/WEIGHTS".into(),
        ));
    }
    let reader = primitive.reader(|b| Some(buffers[b.index()].0.as_slice()));
    let mut streams = Vec::with_capacity(sets.len());
    for set in sets {
        let joints = reader
            .read_joints(set)
            .ok_or_else(|| IoError::Invalid(format!("Отсутствует JOINTS_{set}")))?
            .into_u16();
        let weights = reader
            .read_weights(set)
            .ok_or_else(|| IoError::Invalid(format!("Отсутствует WEIGHTS_{set}")))?
            .into_f32();
        if joints.len() != vertices.len() || weights.len() != vertices.len() {
            return Err(IoError::Invalid(
                "Количество JOINTS/WEIGHTS не совпадает с POSITION".into(),
            ));
        }
        streams.push((joints, weights));
    }
    let mut influences = Vec::<(u32, f64)>::with_capacity(streams.len() * 4);
    let mut reduced = 0;
    for (index, vertex) in vertices.iter_mut().enumerate() {
        if index.is_multiple_of(1024) && cancel.load(Ordering::Relaxed) {
            return Err(IoError::Cancelled);
        }
        influences.clear();
        for (joints, weights) in &mut streams {
            for (joint, weight) in joints
                .next()
                .unwrap()
                .into_iter()
                .zip(weights.next().unwrap())
            {
                if !weight.is_finite() || weight < 0. {
                    return Err(IoError::Invalid(
                        "Отрицательный или нечисловой вес skin".into(),
                    ));
                }
                if weight > 0. {
                    // Validate discarded influences too. Reduction must not hide
                    // invalid palette indices, including on shared mesh instances.
                    if joint_limit.is_some_and(|limit| joint as usize >= limit) {
                        return Err(IoError::Invalid("индекс сустава вне skin".into()));
                    }
                    influences.push((joint as u32, weight as f64));
                }
            }
        }
        // Some exporters duplicate a joint across sets. Sum before ranking so
        // a repeated strong influence is not lost among several smaller slots.
        influences.sort_unstable_by_key(|&(joint, _)| joint);
        let mut unique = 0;
        for i in 0..influences.len() {
            let (joint, weight) = influences[i];
            if unique > 0 && influences[unique - 1].0 == joint {
                influences[unique - 1].1 += weight;
            } else {
                influences[unique] = (joint, weight);
                unique += 1;
            }
        }
        influences.truncate(unique);
        if unique > 4 {
            reduced += 1;
            influences.select_nth_unstable_by(4, |a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            influences.truncate(4);
        }
        influences.sort_unstable_by_key(|&(joint, _)| joint);
        let sum: f64 = influences.iter().map(|(_, weight)| weight).sum();
        vertex.joints = [0; 4];
        vertex.weights = [0.; 4];
        for (i, &(joint, weight)) in influences.iter().enumerate() {
            vertex.joints[i] = joint;
            vertex.weights[i] = (weight / sum) as f32;
        }
    }
    Ok(reduced)
}
