use poseforge_core::*;
use std::sync::atomic::AtomicBool;
fn cube() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(10, None)
        .unwrap()
}
fn plane() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Plane)
        .mesh(10, None)
        .unwrap()
}
fn stack(settings: ModifierSettings) -> Vec<MeshModifier> {
    vec![MeshModifier {
        id: 1,
        name: settings.kind().label().into(),
        enabled: true,
        settings,
    }]
}
fn evaluate(mesh: &Mesh, settings: ModifierSettings) -> Mesh {
    evaluate_modifier_mesh(mesh, &stack(settings), &AtomicBool::new(false)).unwrap()
}
fn editor() -> Editor {
    let mut a = Node::new(1, "One");
    a.mesh = Some(10);
    let mut b = Node::new(2, "Two");
    b.mesh = Some(10);
    Editor::new(Project {
        nodes: vec![a, b],
        meshes: vec![cube()],
        ..Default::default()
    })
}
fn bounds(mesh: &Mesh) -> (Vec3, Vec3) {
    mesh.vertices.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(lo, hi), v| {
            let p = Vec3::from_array(v.position);
            (lo.min(p), hi.max(p))
        },
    )
}

#[test]
fn mirror_preserves_source_corner_data_materials_and_outward_winding() {
    let mut source = cube();
    source.vertices[0].color = [0.1, 0.2, 0.3, 1.];
    source.material = Some(20);
    source.topology.as_mut().unwrap().face_materials = vec![Some(21); source.face_count()].into();
    let original = source.clone();
    let result = evaluate(&source, ModifierSettings::Mirror { axis: 0, plane: 2. });
    assert_eq!(source, original);
    assert!(source.vertices.shares_storage(&original.vertices));
    assert_eq!(result.vertices.len(), source.vertices.len() * 2);
    assert_eq!(result.face_count(), source.face_count() * 2);
    let v = result.vertices[source.vertices.len()];
    assert_eq!(v.color, source.vertices[0].color);
    assert_eq!(v.uv, source.vertices[0].uv);
    assert_eq!(v.position[0], 4. - source.vertices[0].position[0]);
    for face in 0..result.face_count() {
        assert_eq!(result.face_material(face), Some(21));
    }
    for t in result.indices.chunks_exact(3) {
        let p = |i: usize| Vec3::from_array(result.vertices[t[i] as usize].position);
        let n = Vec3::from_array(result.vertices[t[0] as usize].normal);
        assert!((p(1) - p(0)).cross(p(2) - p(0)).dot(n) > 0.);
    }
    result.validate_topology().unwrap();
}
#[test]
fn array_is_local_and_stack_order_changes_the_evaluated_result() {
    let source = cube();
    let mirror = MeshModifier::new(1, ModifierKind::Mirror);
    let mut array = MeshModifier::new(2, ModifierKind::Array);
    array.settings = ModifierSettings::Array {
        count: 3,
        offset: [3., 0., 0.],
    };
    let a = evaluate_modifier_mesh(
        &source,
        &[mirror.clone(), array.clone()],
        &AtomicBool::new(false),
    )
    .unwrap();
    let b = evaluate_modifier_mesh(&source, &[array, mirror], &AtomicBool::new(false)).unwrap();
    assert_eq!(a.vertices.len(), source.vertices.len() * 6);
    assert_eq!(b.vertices.len(), a.vertices.len());
    assert!(bounds(&b).0.x < bounds(&a).0.x - 5.);
    assert_eq!(bounds(&a).1.x, bounds(&source).1.x + 6.);
}
#[test]
fn catmull_clark_preserves_quads_and_simple_mode_preserves_bounds() {
    let source = cube();
    let simple = evaluate(
        &source,
        ModifierSettings::Subdivision {
            levels: 2,
            smooth: false,
        },
    );
    assert_eq!(simple.face_count(), source.face_count() * 16);
    assert_eq!(bounds(&simple), bounds(&source));
    assert!((0..simple.face_count()).all(|f| simple.face_vertices(f).unwrap().len() == 4));
    let smooth = evaluate(
        &source,
        ModifierSettings::Subdivision {
            levels: 2,
            smooth: true,
        },
    );
    assert_eq!(smooth.face_count(), simple.face_count());
    assert!(bounds(&smooth).1.x < bounds(&source).1.x);
    assert!(smooth
        .vertices
        .iter()
        .all(|v| Vec3::from_array(v.normal).is_finite()
            && (Vec3::from_array(v.normal).length() - 1.).abs() < 0.001));
    simple.validate_topology().unwrap();
    smooth.validate_topology().unwrap();
}
#[test]
fn subdivision_retains_color_uv_and_face_materials_across_seams() {
    let mut source = cube();
    source.material = Some(8);
    source.topology.as_mut().unwrap().face_materials = (0..source.face_count())
        .map(|i| Some(20 + i as u64))
        .collect();
    for v in &mut source.vertices {
        v.color = [0.2, 0.4, 0.6, 1.];
    }
    let result = evaluate(
        &source,
        ModifierSettings::Subdivision {
            levels: 1,
            smooth: true,
        },
    );
    for f in 0..source.face_count() {
        for i in 0..4 {
            assert_eq!(result.face_material(f * 4 + i), source.face_material(f));
        }
    }
    assert!(result
        .vertices
        .iter()
        .all(|v| (v.color[0] - 0.2).abs() < 1e-5 && v.uv.iter().all(|x| x.is_finite())));
}
#[test]
fn solidify_closes_open_boundaries_and_supports_centered_and_outside_shells() {
    let source = plane();
    let before = source.clone();
    let result = evaluate(
        &source,
        ModifierSettings::Solidify {
            thickness: 0.4,
            offset: 0.,
        },
    );
    assert_eq!(result.vertices.len(), source.vertices.len() * 2);
    assert_eq!(result.face_count(), 6);
    assert!(MeshConnectivity::new(&result)
        .edge_faces
        .values()
        .all(|f| f.len() == 2));
    let normal = Vec3::from_array(source.vertices[0].normal);
    let p = Vec3::from_array(source.vertices[0].position);
    assert!((Vec3::from_array(result.vertices[0].position) - p - normal * 0.2).length() < 1e-5);
    let outside = evaluate(
        &source,
        ModifierSettings::Solidify {
            thickness: 0.4,
            offset: 1.,
        },
    );
    assert_eq!(
        outside.vertices[source.vertices.len()].position,
        source.vertices[0].position
    );
    assert_eq!(source, before);
    result.validate_topology().unwrap();
}
#[test]
fn disable_delete_and_reorder_keep_shared_source_and_have_atomic_history() {
    let mut e = editor();
    let source = e.project.meshes.clone();
    let a = e.add_modifier(1, ModifierKind::Array).unwrap();
    let b = e.add_modifier(1, ModifierKind::Mirror).unwrap();
    let history = e.undo_len();
    e.transfer_modifier(1, b, 1, Some(a), false).unwrap();
    assert_eq!(e.undo_len(), history + 1);
    assert_eq!(e.project.nodes[0].modifiers[0].id, b);
    assert!(e.undo());
    assert_eq!(e.project.nodes[0].modifiers[0].id, a);
    assert!(e.redo());
    let mut m = e.project.nodes[0].modifiers[0].clone();
    m.enabled = false;
    e.update_modifier(1, m).unwrap();
    let result = e.evaluated().unwrap();
    assert_eq!(
        result.modifier_meshes[&1].vertices.len(),
        source[0].vertices.len() * 2
    );
    assert!(!result.modifier_meshes.contains_key(&2));
    assert_eq!(e.project.meshes, source);
    e.remove_modifier(1, a).unwrap();
    assert!(e.evaluated().unwrap().modifier_meshes.is_empty());
    assert_eq!(e.project.meshes, source);
}
#[test]
fn transfer_copy_reorder_noop_and_locked_target_are_transactional() {
    let mut e = editor();
    let a = e.add_modifier(1, ModifierKind::Array).unwrap();
    let original = e.project.clone();
    let history = e.undo_len();
    e.transfer_modifier(1, a, 1, Some(a), false).unwrap();
    assert_eq!(e.undo_len(), history);
    let b = e.transfer_modifier(1, a, 2, None, true).unwrap();
    assert_eq!(e.project.nodes[0], original.nodes[0]);
    assert_eq!(e.project.nodes[1].modifiers[0].id, b);
    assert!(e.undo());
    assert_eq!(e.project, original);
    e.transfer_modifier(1, a, 2, None, false).unwrap();
    assert!(e.project.nodes[0].modifiers.is_empty());
    assert_eq!(e.project.nodes[1].modifiers.len(), 1);
    assert!(e.undo());
    assert_eq!(e.project, original);
    e.project.node_mut(2).unwrap().locked = true;
    let before = e.project.clone();
    assert!(e.transfer_modifier(1, a, 2, None, false).is_err());
    assert_eq!(e.project, before);
    let c = e.transfer_modifier(1, a, 1, None, true).unwrap();
    assert_ne!(c, a);
    e.project.validate().unwrap();
}
#[test]
fn applying_stack_detaches_only_the_instance_and_undo_restores_everything() {
    let mut e = editor();
    e.add_modifier(1, ModifierKind::Array).unwrap();
    let before = e.project.clone();
    let result = e.evaluated().unwrap().modifier_meshes[&1].clone();
    e.apply_modifiers(1).unwrap();
    let id = e.project.nodes[0].mesh.unwrap();
    assert_ne!(id, 10);
    assert_eq!(e.project.mesh(id).unwrap().vertices, result.vertices);
    assert_eq!(e.project.node(2).unwrap().mesh, Some(10));
    assert_eq!(e.project.mesh(10), before.mesh(10));
    assert!(e.project.nodes[0].modifiers.is_empty());
    assert!(e.undo());
    assert_eq!(e.project, before);
    assert!(e.redo());
    e.project.validate().unwrap();
}
#[test]
fn cache_reuses_geometry_and_invalidates_source_materials_settings_and_undo() {
    let mut source = cube();
    let settings = stack(ModifierKind::Array.settings());
    let a = evaluate_modifier_mesh(&source, &settings, &AtomicBool::new(false)).unwrap();
    let b = evaluate_modifier_mesh(&source, &settings, &AtomicBool::new(false)).unwrap();
    assert_eq!(a.vertices.revision(), b.vertices.revision());
    assert!(a.vertices.shares_storage(&b.vertices));
    let original = source.clone();
    source.vertices[0].position[0] += 0.125;
    let c = evaluate_modifier_mesh(&source, &settings, &AtomicBool::new(false)).unwrap();
    assert_ne!(c.vertices.revision(), a.vertices.revision());
    let undo = evaluate_modifier_mesh(&original, &settings, &AtomicBool::new(false)).unwrap();
    assert!(undo.vertices.shares_storage(&a.vertices));
    source.topology.as_mut().unwrap().face_materials = vec![Some(99); source.face_count()].into();
    let c = evaluate_modifier_mesh(&source, &settings, &AtomicBool::new(false)).unwrap();
    assert_eq!(c.face_material(0), Some(99));
}
#[test]
fn invalid_parameters_growth_budget_cancel_and_duplicate_ids_are_rejected() {
    for settings in [
        ModifierSettings::Array {
            count: 0,
            offset: [0.; 3],
        },
        ModifierSettings::Mirror { axis: 3, plane: 0. },
        ModifierSettings::Solidify {
            thickness: f32::NAN,
            offset: 0.,
        },
        ModifierSettings::Subdivision {
            levels: 5,
            smooth: true,
        },
    ] {
        assert!(settings.validate().is_err());
    }
    let source = cube();
    let m = MeshModifier {
        settings: ModifierSettings::Array {
            count: 128,
            offset: [2., 0., 0.],
        },
        ..MeshModifier::new(1, ModifierKind::Array)
    };
    assert!(check_modifier_budget(&source, &[m.clone(), m.clone(), m]).is_err());
    assert!(evaluate_modifier_mesh(
        &source,
        &stack(ModifierKind::Subdivision.settings()),
        &AtomicBool::new(true)
    )
    .is_err());
    let mut e = editor();
    let id = e.add_modifier(1, ModifierKind::Array).unwrap();
    let m = e.project.nodes[0].modifiers[0].clone();
    e.project.nodes[0].modifiers.push(m);
    assert!(e.project.validate().is_err());
    e.project.nodes[0].modifiers.pop();
    assert_eq!(e.project.nodes[0].modifiers[0].id, id);
    e.project.nodes[0].physics = Some(PhysicsSettings::default());
    assert!(e.project.validate().is_err());
}

#[test]
fn pose_commands_do_not_depend_on_procedural_geometry_calculation() {
    let mut e = editor();
    // A valid document can contain a nonmanifold surface that subdivision refuses.
    let mesh = &mut e.project.meshes[0];
    let face = mesh.topology.as_ref().unwrap().faces[0].clone();
    mesh.topology.as_mut().unwrap().faces.push(face);
    mesh.rebuild_polygon_indices().unwrap();
    e.project.validate().unwrap();
    e.add_modifier(1, ModifierKind::Subdivision).unwrap();
    assert!(e.evaluated().is_err());
    assert!(e.evaluated_base().is_ok());
    e.selected = Some(1);
    e.copy_selected_pose().unwrap();
    e.mode = EditorMode::Animation;
    e.key_transform(1).unwrap();
    assert!(!e.project.clips.is_empty());
    e.project.validate().unwrap();
}

#[test]
fn uv_seam_edits_invalidate_cached_copies_without_changing_source_geometry() {
    let mut source = cube();
    let settings = stack(ModifierKind::Array.settings());
    let cancel = AtomicBool::new(false);
    let before = evaluate_modifier_mesh(&source, &settings, &cancel).unwrap();
    let indices = source.indices.revision();
    source.topology.as_mut().unwrap().uv_seams = vec![[0, 1]];
    assert_eq!(source.indices.revision(), indices);
    let result = evaluate_modifier_mesh(&source, &settings, &cancel).unwrap();
    let stride = source.vertices.len() as u32;
    assert_eq!(
        result.topology.as_ref().unwrap().uv_seams,
        vec![[0, 1], [stride, stride + 1]]
    );
    assert!(before.topology.as_ref().unwrap().uv_seams.is_empty());
    result.validate_topology().unwrap();
}

#[test]
fn retained_preview_bakes_after_cache_eviction_and_rejects_a_stale_source() {
    let mut e = editor();
    e.add_modifier(1, ModifierKind::Array).unwrap();
    let stack = e.project.nodes[0].modifiers.clone();
    let source = e.project.mesh(10).unwrap().clone();
    let key = modifier_key(&source, &stack);
    let preview = evaluate_modifier_mesh(&source, &stack, &AtomicBool::new(false)).unwrap();
    for count in 3..80 {
        let mut other = stack.clone();
        other[0].settings = ModifierSettings::Array {
            count,
            offset: [2., 0., 0.],
        };
        evaluate_modifier_mesh(&source, &other, &AtomicBool::new(false)).unwrap();
    }
    assert!(cached_modifier_mesh(&source, &stack).is_none());
    let original = e.project.clone();
    e.apply_evaluated_modifiers(1, &key, preview.clone())
        .unwrap();
    assert!(e.project.nodes[0].modifiers.is_empty());
    assert!(e.undo());
    assert_eq!(e.project, original);
    e.project.meshes[0].vertices[0].position[0] += 0.25;
    let changed = e.project.clone();
    let history = e.undo_len();
    assert!(e.apply_evaluated_modifiers(1, &key, preview).is_err());
    assert_eq!(e.project, changed);
    assert_eq!(e.undo_len(), history);
}

#[test]
fn scene_growth_limit_rolls_back_the_whole_transfer_before_allocating_geometry() {
    let mut e = editor();
    let id = e.add_modifier(1, ModifierKind::Subdivision).unwrap();
    let mut m = e.project.nodes[0].modifiers[0].clone();
    m.settings = ModifierSettings::Subdivision {
        levels: 4,
        smooth: false,
    };
    e.update_modifier(1, m).unwrap();
    let array = e.add_modifier(1, ModifierKind::Array).unwrap();
    let mut m = e.project.nodes[0]
        .modifiers
        .iter()
        .find(|m| m.id == array)
        .unwrap()
        .clone();
    m.settings = ModifierSettings::Array {
        count: 32,
        offset: [2., 0., 0.],
    };
    e.update_modifier(1, m).unwrap();
    // Each owner estimates 196608 vertices, within the per-object limit.
    for n in 2..=5 {
        if n > 2 {
            let mut node = Node::new(n, "Copy");
            node.mesh = Some(10);
            e.project.nodes.push(node);
        }
        e.transfer_modifier(1, id, n, None, true).unwrap();
        e.transfer_modifier(1, array, n, None, true).unwrap();
    }
    let mut node = Node::new(6, "Limit");
    node.mesh = Some(10);
    e.project.nodes.push(node);
    e.transfer_modifier(1, id, 6, None, true).unwrap();
    let before = e.project.clone();
    let history = e.undo_len();
    assert!(e.transfer_modifier(1, array, 6, None, true).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.undo_len(), history);
}
