use super::*;
use poseforge_core::{BoneShape, Editor, SkeletonTemplate};

fn fixture(display: BoneDisplay, view: &View) -> BoneOverlay {
    let mut root = Node::new(1, "Bone");
    root.is_joint = true;
    root.bone_display = Some(display);
    let mut tail = Node::new(2, "Tip");
    tail.is_joint = true;
    tail.parent = Some(1);
    tail.transform.translation = Vec3::Y;
    let project = Project {
        nodes: vec![root, tail],
        ..Default::default()
    };
    BoneOverlay::new(
        &project,
        &project.evaluated(None, 0.).unwrap(),
        view,
        view.viewport,
    )
}

fn view() -> View {
    View {
        eye: Vec3::new(0., 0.5, 5.),
        target: Vec3::Y * 0.5,
        viewport: [0., 0., 500., 500.],
        orthographic: Some(1.5),
        ..Default::default()
    }
}

fn export(name: &str, view: &View, ui: &Ui) {
    if let Some(dir) = std::env::var_os("ARTFORGE_QA_BONE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let json = serde_json::json!({"width":view.viewport[2],"height":view.viewport[3],
            "background":ui.palette().viewport,"source":"Actual BoneOverlay CPU primitives, not a GPU screenshot",
            "rectangles":ui.rects.iter().map(|r| serde_json::json!({"rect":r.rect,"color":r.color,
                "triangle":r.triangle,"segment":r.segment})).collect::<Vec<_>>(),
            "texts":ui.texts.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"text":t.text,"size":t.size,"color":t.color})).collect::<Vec<_>>()});
        std::fs::write(
            dir.join(format!("bones-{name}.json")),
            serde_json::to_vec(&json).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn every_shape_shares_rendering_and_picking_with_bounded_clipped_geometry() {
    for shape in BoneShape::ALL {
        for joint_shape in JointShape::ALL {
            for filled in [false, true] {
                for ortho in [None, Some(1.5)] {
                    let v = View {
                        eye: Vec3::new(2., 1.5, 5.),
                        orthographic: ortho,
                        ..view()
                    };
                    let mut overlay = fixture(
                        BoneDisplay {
                            shape,
                            joint_shape,
                            filled,
                            joint_filled: filled,
                            ..Default::default()
                        },
                        &v,
                    );
                    assert_eq!(overlay.bones.len(), 1, "{shape:?}");
                    let center = overlay.joints.iter().find(|j| j.id == 1).unwrap().center;
                    if filled {
                        assert_eq!(overlay.hit(center.to_array()), Some(1));
                    }
                    let joint = overlay.joints.iter().find(|j| j.id == 1).unwrap();
                    let outline = joint.polygon(JOINT_RADIUS);
                    assert_eq!(
                        overlay.hit(((outline[0] + outline[1]) * 0.5).to_array()),
                        Some(1)
                    );
                    overlay.joints.clear();
                    let body = &overlay.bones[0].shape;
                    let face = &body.faces[0];
                    let center = face.points.iter().map(|p| p.truncate()).sum::<Vec2>()
                        / face.points.len() as f32;
                    if filled {
                        assert_eq!(overlay.hit(center.to_array()), Some(1));
                    }
                    let edge = body.edges[0];
                    assert_eq!(overlay.hit(((edge[0] + edge[1]) * 0.5).to_array()), Some(1));
                    let mut ui = Ui::new(2.);
                    overlay.draw(&mut ui, None, None);
                    assert!(ui.rects.len() < 256, "unbounded primitives: {shape:?}");
                    assert_eq!(ui.rects.iter().any(|r| r.triangle.is_some()), filled);
                    assert!(ui
                        .rects
                        .iter()
                        .all(|r| r.rect.iter().all(|v| v.is_finite())));
                }
            }
        }
        let v = View {
            eye: Vec3::new(0., 0.5, 0.1),
            target: Vec3::new(0., 0.5, -1.),
            orthographic: None,
            ..view()
        };
        let mut root = Node::new(1, "Crossing near plane");
        root.is_joint = true;
        root.bone_tip = Some(Vec3::new(0.7, 1., -2.));
        root.bone_display = Some(BoneDisplay {
            shape,
            ..Default::default()
        });
        let p = Project {
            nodes: vec![root],
            ..Default::default()
        };
        let overlay = BoneOverlay::new(&p, &p.evaluated(None, 0.).unwrap(), &v, v.viewport);
        let mut ui = Ui::new(1.);
        overlay.draw(&mut ui, None, None);
        assert!(ui.rects.len() < 512);
        for rect in &ui.rects {
            assert!(rect.rect.iter().all(|v| v.is_finite()));
            if let Some(points) = rect.triangle {
                for p in points {
                    assert!((0. ..=500.).contains(&p[0]) && (0. ..=500.).contains(&p[1]));
                }
            }
        }
    }
}

#[test]
fn shape_gallery_exports_real_overlay_geometry() {
    if std::env::var_os("ARTFORGE_QA_BONE_DIR").is_none() {
        return;
    }
    let nodes = BoneShape::ALL
        .into_iter()
        .enumerate()
        .map(|(i, shape)| {
            let mut n = Node::new(i as u64 + 1, shape.label());
            n.is_joint = true;
            n.transform.translation = Vec3::new(i as f32 - 2.5, 0.2, 0.);
            n.bone_tip = Some(Vec3::Y * 1.2);
            n.bone_display = Some(BoneDisplay {
                shape,
                joint_shape: JointShape::ALL[i % 5],
                ..Default::default()
            });
            n
        })
        .collect();
    let p = Project {
        nodes,
        ..Default::default()
    };
    let v = View {
        eye: Vec3::new(2., 2.5, 8.),
        target: Vec3::Y * 0.8,
        viewport: [0., 0., 1200., 500.],
        orthographic: Some(1.5),
        ..Default::default()
    };
    for filled in [true, false] {
        let mut p = p.clone();
        for n in &mut p.nodes {
            let d = n.bone_display.as_mut().unwrap();
            d.filled = filled;
            d.joint_filled = filled;
        }
        let overlay = BoneOverlay::new(&p, &p.evaluated(None, 0.).unwrap(), &v, v.viewport);
        let mut ui = Ui::new(1.);
        overlay.draw(&mut ui, None, None);
        for n in &p.nodes {
            let pos = project_point(
                v.view_projection(),
                v.viewport,
                n.transform.translation - Vec3::Y * 0.24,
            )
            .unwrap()
            .0;
            ui.fitted_label(
                [pos.x - 75., pos.y, 150., 24.],
                &n.name,
                14.,
                ui.palette().text,
            );
        }
        export(
            if filled {
                "shapes-filled"
            } else {
                "shapes-outline"
            },
            &v,
            &ui,
        );
    }
}

#[test]
fn custom_colors_stay_visible_when_selected_and_wire_shapes_pick_only_their_outline() {
    let view = view();
    let d = BoneDisplay {
        color: [0.8, 0.25, 0.1],
        joint_color: [0.2, 0.8, 0.4],
        shape: BoneShape::Rectangle,
        joint_shape: JointShape::Rectangle,
        ..Default::default()
    };
    let overlay = fixture(d, &view);
    let center = overlay.joints[0].center;
    let body = overlay.bones[0].shape.faces[0]
        .points
        .iter()
        .map(|p| p.truncate())
        .sum::<Vec2>()
        / 4.;
    assert_eq!(overlay.hit(body.to_array()), Some(1));
    assert_eq!(
        overlay.hit((center + Vec2::new(4.5, 4.5)).to_array()),
        Some(1)
    );
    for selected in [None, Some(1)] {
        let mut ui = Ui::new(1.);
        overlay.draw(&mut ui, selected, None);
        assert!(ui
            .rects
            .iter()
            .any(|r| r.triangle.is_some() && r.color == [0.8, 0.25, 0.1, 1.]));
        assert!(ui.rects.iter().any(|r| r.color == [0.2, 0.8, 0.4, 1.]));
        export(
            if selected.is_some() {
                "custom-selected"
            } else {
                "custom-filled"
            },
            &view,
            &ui,
        );
    }
    let mut outline = fixture(
        BoneDisplay {
            filled: false,
            joint_filled: false,
            ..d
        },
        &view,
    );
    outline.joints.clear();
    assert_eq!(outline.hit(body.to_array()), None);
    let edge = outline.bones[0].shape.edges[0];
    assert_eq!(outline.hit(((edge[0] + edge[1]) * 0.5).to_array()), Some(1));
    let mut ui = Ui::new(1.);
    outline.draw(&mut ui, None, None);
    assert!(ui.rects.iter().all(|r| r.triangle.is_none()));
    let outline = fixture(
        BoneDisplay {
            filled: false,
            joint_filled: false,
            ..d
        },
        &view,
    );
    let mut ui = Ui::new(1.);
    outline.draw(&mut ui, None, None);
    export("custom-outline", &view, &ui);
}

#[test]
fn following_view_keeps_joints_facing_the_camera_and_rotates_body_about_its_axis() {
    let view = View {
        eye: Vec3::new(3., 2., 4.),
        ..view()
    };
    let d = BoneDisplay {
        shape: BoneShape::Rectangle,
        joint_shape: JointShape::Rectangle,
        follow_view: true,
        joint_follow_view: true,
        ..Default::default()
    };
    let facing = fixture(d, &view);
    assert_eq!(facing.joints[0].basis, [Vec2::X, Vec2::Y]);
    let fixed = fixture(
        BoneDisplay {
            follow_view: false,
            joint_follow_view: false,
            ..d
        },
        &view,
    );
    assert_ne!(fixed.joints[0].basis, facing.joints[0].basis);
    assert_ne!(
        fixed.bones[0].shape.faces[0].points,
        facing.bones[0].shape.faces[0].points
    );
    for (name, overlay) in [("view-facing", facing), ("local-plane", fixed)] {
        let mut ui = Ui::new(1.);
        overlay.draw(&mut ui, None, None);
        assert!(ui
            .rects
            .iter()
            .all(|r| r.rect.iter().all(|v| v.is_finite())));
        export(name, &view, &ui);
    }
}

#[test]
fn the_full_human_has_one_visible_body_per_authored_bone_including_finger_tips() {
    let mut editor = Editor::new(Project::default());
    editor
        .create_skeleton(SkeletonTemplate::Humanoid, None)
        .unwrap();
    let view = View {
        eye: Vec3::new(0., 1., 6.),
        target: Vec3::Y,
        viewport: [0., 0., 900., 820.],
        orthographic: Some(1.4),
        ..Default::default()
    };
    let overlay = BoneOverlay::new(
        &editor.project,
        &editor.evaluated().unwrap(),
        &view,
        view.viewport,
    );
    assert_eq!(overlay.bones.len(), poseforge_core::HUMANOID_JOINT_COUNT);
    assert_eq!(
        overlay
            .bones
            .iter()
            .map(|b| b.owner)
            .collect::<BTreeSet<_>>()
            .len(),
        poseforge_core::HUMANOID_JOINT_COUNT
    );
    let mut ui = Ui::new(1.);
    overlay.draw(&mut ui, None, None);
    export("human-67", &view, &ui);
    for (name, detail_view) in [
        (
            "human-67-angle",
            View {
                eye: Vec3::new(3., 1.7, 6.),
                ..view
            },
        ),
        (
            "human-hand",
            View {
                eye: Vec3::new(1.1, 2.3, 0.7),
                target: Vec3::new(0.94, 1.54, -0.04),
                viewport: [0., 0., 600., 520.],
                orthographic: Some(0.19),
                ..view
            },
        ),
    ] {
        let overlay = BoneOverlay::new(
            &editor.project,
            &editor.evaluated().unwrap(),
            &detail_view,
            detail_view.viewport,
        );
        let mut ui = Ui::new(1.);
        overlay.draw(&mut ui, None, None);
        export(name, &detail_view, &ui);
    }
}

#[test]
fn display_sizes_change_visible_and_pickable_geometry_without_moving_endpoints() {
    let view = view();
    let mut small = fixture(BoneDisplay { shape: BoneShape::Rectangle, size: 0.25, joint_size: 0.5, ..Default::default() }, &view);
    let mut large = fixture(BoneDisplay { shape: BoneShape::Rectangle, size: 3., joint_size: 3., ..Default::default() }, &view);
    let center = small.joints.iter().find(|j|j.id==1).unwrap().center;
    assert_eq!(center,large.joints.iter().find(|j|j.id==1).unwrap().center);
    // A point inside the large marker is outside the small marker and body.
    let point=(center+Vec2::new(-13.,0.)).to_array();
    assert_eq!(small.hit(point),None);assert_eq!(large.hit(point),Some(1));
    small.joints.clear();large.joints.clear();
    let mid=project_point(view.view_projection(),view.viewport,Vec3::Y*0.5).unwrap().0;
    let point=(mid+Vec2::new(20.,0.)).to_array();
    assert_eq!(small.hit(point),None);assert_eq!(large.hit(point),Some(1));
    let before:Vec<_>=small.bones[0].shape.faces.iter().flat_map(|f|f.points.iter()).map(|p|p.x).collect();
    let after:Vec<_>=large.bones[0].shape.faces.iter().flat_map(|f|f.points.iter()).map(|p|p.x).collect();
    assert!(after.iter().copied().fold(f32::NEG_INFINITY,f32::max)>before.iter().copied().fold(f32::NEG_INFINITY,f32::max));
}
