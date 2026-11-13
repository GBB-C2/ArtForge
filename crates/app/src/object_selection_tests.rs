use super::*;
use poseforge_core::{box_mesh, Material, Node, Skin};

fn app() -> App {
    let root = Node::new(1, "Armature");
    let mut lower = Node::new(2, "Lower");
    lower.is_joint = true;
    lower.parent = Some(1);
    lower.transform.translation = Vec3::new(0., -0.7, 0.);
    let mut upper = Node::new(3, "Upper");
    upper.is_joint = true;
    upper.parent = Some(2);
    upper.transform.translation = Vec3::new(0., 1.4, 0.);
    let mut owner = Node::new(10, "Skin");
    owner.mesh = Some(100);
    owner.parent = Some(1);
    owner.skin = Some(Skin {
        joints: vec![2, 3],
        inverse_bind_matrices: vec![
            Mat4::from_translation(Vec3::Y * 0.7),
            Mat4::from_translation(Vec3::Y * -0.7),
        ],
    });
    let mut mesh = box_mesh(100, "Skin", Vec3::new(1., 1.3, 0.25), Some(101));
    for v in &mut mesh.vertices {
        v.joints = [0; 4];
        v.weights = [1., 0., 0., 0.];
    }
    let project = Project {
        nodes: vec![root, lower, upper, owner],
        meshes: vec![mesh],
        materials: vec![Material {
            id: 101,
            double_sided: true,
            ..Default::default()
        }],
        lights: vec![],
        ..Default::default()
    };
    project.validate().unwrap();
    let mut a = App::new(None);
    a.editor = Editor::new(project);
    a.editor.mode = EditorMode::Scene;
    a.preferences = Preferences::default();
    a.preferences.left_panel = true;
    a.preferences.right_panel = true;
    a.width = 1280.;
    a.height = 900.;
    a.yaw = 0.;
    a.pitch = 0.;
    a.distance = 6.;
    a.target = Vec3::ZERO;
    a.saved_snapshot = a.editor.project.clone();
    a
}

#[test]
fn armature_selection_excludes_skin_owners_and_attached_meshes() {
    let mut a = app();
    let mut attached = a.editor.project.node(10).unwrap().clone();
    attached.id = 20;
    attached.parent = Some(3);
    a.editor.project.nodes.push(attached);
    let before = a.editor.project.clone();
    for id in [1, 2, 3] {
        a.action(Action::Select(id));
        assert_eq!(a.object_selection(), BTreeSet::from([1, 2, 3]));
        assert!(!a.object_selected(10) && !a.object_selected(20));
    }
    for id in [10, 20] {
        a.action(Action::Select(id));
        assert_eq!(a.object_selection(), BTreeSet::from([id]));
    }
    for modifier in [ModifiersState::SHIFT, ModifiersState::CONTROL] {
        a.modifiers = ModifiersState::empty();
        a.action(Action::Select(10));
        a.modifiers = modifier;
        a.action(Action::Select(2));
        assert_eq!(a.object_selection(), BTreeSet::from([1, 2, 3, 10]));
        a.action(Action::Select(3));
        assert_eq!(a.object_selection(), BTreeSet::from([10]));
    }
    a.modifiers = ModifiersState::empty();
    a.action(Action::Mode(1));
    a.action(Action::Select(3));
    assert_eq!(a.object_selection(), BTreeSet::from([3]));
    a.action(Action::Mode(0));
    assert_eq!(a.object_selection(), BTreeSet::from([1, 2, 3]));
    assert_eq!(a.editor.project, before);
    assert_eq!(a.editor.undo_len(), 0);
}

#[test]
fn viewport_selects_bones_and_skinned_surfaces_independently_with_every_tool() {
    for (tool_index, tool) in [Tool::Move, Tool::Rotate, Tool::Scale, Tool::Orbit]
        .into_iter()
        .enumerate()
    {
        for ortho in [false, true] {
            for scale in [1., 2.] {
                let mut a = app();
                a.tool = tool;
                a.orthographic = ortho;
                a.scale = scale;
                let before = a.editor.project.clone();
                for target in [10, 2, 10] {
                    a.ui = a.build_ui();
                    let candidates: Vec<_> = if target == 10 {
                        [0.45, 0.65, 0.85]
                            .into_iter()
                            .flat_map(|x| [-1.1, -0.9, 0.9, 1.1].map(|y| Vec3::new(x, y, 0.25)))
                            .collect()
                    } else {
                        [-0.7, -0.6, -0.4, -0.2, 0.2, 0.4, 0.6, 0.7]
                            .map(|y| Vec3::new(0., y, 0.))
                            .to_vec()
                    };
                    let point = candidates.into_iter().filter_map(|p| a.project_point(p))
                        .find(|p| a.canvas_input_at(*p) && a.ui.hit(*p).is_none()
                            && a.selected_gizmo().is_none_or(|g| g.hit(*p).is_none()))
                        .unwrap_or_else(|| panic!("no unobstructed click: tool={tool_index}, target={target}, ortho={ortho}, scale={scale}"));
                    a.mouse_move(point);
                    a.mouse_down(MouseButton::Left);
                    a.mouse_up(MouseButton::Left);
                    assert_eq!(
                        a.object_selection(),
                        if target == 10 {
                            BTreeSet::from([10])
                        } else {
                            BTreeSet::from([1, 2, 3])
                        },
                        "ortho={ortho}, scale={scale}"
                    );
                    assert!(a.drag.is_none());
                }
                assert_eq!(a.editor.project, before);
                assert_eq!(a.editor.undo_len(), 0);
            }
        }
    }
}

#[test]
fn mesh_parent_is_never_promoted_to_a_skeleton_or_highlights_its_child_bones() {
    let mut a = app();
    a.editor.project.nodes.retain(|n| n.id != 1);
    a.editor.project.node_mut(10).unwrap().parent = None;
    a.editor.project.node_mut(2).unwrap().parent = Some(10);
    a.editor.project.node_mut(3).unwrap().parent = Some(10);
    a.action(Action::Select(2));
    assert_eq!(a.object_selection(), BTreeSet::from([2, 3]));
    a.action(Action::Select(10));
    assert_eq!(a.object_selection(), BTreeSet::from([10]));
    a.tool = Tool::Orbit;
    a.mouse = [0., 0.];
    let ui = a.build_ui();
    let bodies: Vec<_> = ui.rects.iter().filter(|r| r.triangle.is_some()).collect();
    assert!(!bodies.is_empty());
    assert!(bodies
        .iter()
        .all(|r| (r.color[0] / r.color[2] - 0.58 / 0.64).abs() < 0.001));
    a.action(Action::SceneTool(ui::SceneTool::SelectAll));
    assert_eq!(a.object_selection(), BTreeSet::from([2, 3, 10]));
}

#[test]
fn hidden_and_locked_surfaces_remain_unselectable() {
    let mut a = app();
    a.mouse = a.project_point(Vec3::new(0.65, -0.95, 0.25)).unwrap();
    for locked in [true, false] {
        let node = a.editor.project.node_mut(10).unwrap();
        node.locked = locked;
        node.visible = locked;
        assert!(a
            .pick_viewport_node(&a.editor.evaluated().unwrap())
            .is_none());
    }
}
