//! Borrow static document assets; material/UI/camera changes do not need deep snapshots.
use poseforge_core::{EvaluatedScene, Project};
use std::borrow::Cow;

pub(super) fn render_project<'a>(
    project: &'a Project,
    evaluated: &EvaluatedScene,
    physics: bool,
) -> Cow<'a, Project> {
    let changed_morphs = project.nodes.iter().any(|n| {
        evaluated
            .morph_weights
            .get(&n.id)
            .is_some_and(|weights| *weights != n.morph_weights)
    });
    let changed_lights = project.lights != evaluated.lights;
    if !physics && !changed_morphs && !changed_lights && evaluated.text_meshes.is_empty() && evaluated.modifier_meshes.is_empty() {
        return Cow::Borrowed(project);
    }
    let mut sampled = project.clone();
    sampled.apply_text_meshes(&evaluated.text_meshes);
    sampled.apply_text_meshes(&evaluated.modifier_meshes);
    if changed_lights { sampled.lights.clone_from(&evaluated.lights); }
    if changed_morphs {
        for node in &mut sampled.nodes {
            if let Some(weights) = evaluated.morph_weights.get(&node.id) {
                node.morph_weights.clone_from(weights);
            }
        }
    }
    Cow::Owned(sampled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Editor, Node, Texture};
    #[test]
    fn static_and_camera_only_frames_borrow_assets_while_morph_and_physics_are_isolated() {
        let mut p = Project::default();
        let mut node = Node::new(1, "Morph");
        node.morph_weights = vec![0.];
        p.nodes.push(node);
        p.textures.push(Texture {
            id: 2,
            name: "Image".into(),
            width: 1,
            height: 1,
            rgba8: vec![255; 4].into(),
        });
        let e = Editor::new(p);
        let mut eval = e.evaluated().unwrap();
        let frame = render_project(&e.project, &eval, false);
        assert!(matches!(frame, Cow::Borrowed(_)));
        assert_eq!(
            frame.textures[0].rgba8.as_ptr(),
            e.project.textures[0].rgba8.as_ptr()
        );
        eval.morph_weights.insert(1, vec![0.75]);
        let morph = render_project(&e.project, &eval, false);
        assert!(matches!(morph, Cow::Owned(_)));
        assert_eq!(morph.nodes[0].morph_weights, vec![0.75]);
        assert_eq!(e.project.nodes[0].morph_weights, vec![0.]);
        assert!(matches!(
            render_project(&e.project, &e.evaluated().unwrap(), true),
            Cow::Owned(_)
        ));
    }
    #[test]
    fn profile_static_frame_snapshot_when_requested() {
        let Some(path) = std::env::var_os("ARTFORGE_PROFILE_SCENE") else {
            return;
        };
        let mut p = Project::default();
        p.nodes.push(Node::new(1, "Object"));
        p.textures.push(Texture {
            id: 2,
            name: "4K RGBA".into(),
            width: 4096,
            height: 4096,
            rgba8: vec![128; 4096 * 4096 * 4].into(),
        });
        let e = Editor::new(p);
        let eval = e.evaluated().unwrap();
        let measure = |legacy: bool| {
            let mut samples = vec![];
            for _ in 0..20 {
                let start = std::time::Instant::now();
                let frame = if legacy {
                    {
                        let mut deep = e.project.clone();
                        for t in &mut deep.textures {
                            t.rgba8 = t.rgba8.to_vec().into();
                        }
                        Cow::Owned(deep)
                    }
                } else {
                    render_project(&e.project, &eval, false)
                };
                std::hint::black_box(&frame);
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            }
            samples.sort_by(f64::total_cmp);
            samples[10]
        };
        let old = measure(true);
        let new = measure(false);
        let mut samples = vec![];
        for _ in 0..20 {
            let start = std::time::Instant::now();
            let frame = render_project(&e.project, &eval, true);
            std::hint::black_box(&frame);
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        let dynamic = samples[10];
        std::fs::write(path,format!("{{\"texture_bytes\":67108864,\"frames\":20,\"legacy_copy_ms_median\":{old},\"borrowed_ms_median\":{new},\"dynamic_cow_ms_median\":{dynamic},\"copied_asset_bytes_per_static_frame\":0}}" )).unwrap();
    }
}
