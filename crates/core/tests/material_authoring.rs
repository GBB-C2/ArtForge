use poseforge_core::{
    box_mesh, AlphaMode, Editor, Material, MaterialPreset, MorphTarget, Node, Project, Texture,
    Vec3,
};

fn scene() -> Project {
    let mut project = Project::default();
    project.textures.push(Texture {
        id: 50,
        name: "Image".into(),
        width: 1,
        height: 1,
        rgba8: vec![128, 160, 255, 255].into(),
    });
    project.materials.push(Material {
        id: 10,
        name: "Authored".into(),
        base_color: [0.2, 0.3, 0.4, 0.8],
        base_color_texture: Some(50),
        normal_texture: Some(50),
        alpha_mode: AlphaMode::Blend,
        emissive: [2., 1., 0.5],
        ior: 1.8,
        coat_weight: 0.6,
        coat_roughness: 0.2,
        ..Material::default()
    });
    project.materials.push(Material {
        id: 11,
        ..MaterialPreset::Metal.create()
    });
    let mut mesh = box_mesh(20, "Shared geometry", Vec3::splat(0.5), Some(10));
    mesh.morph_targets.push(MorphTarget {
        name: "Offset".into(),
        positions: vec![[0.1, 0., 0.]; mesh.vertices.len()].into(),
        normals: vec![[0.; 3]; mesh.vertices.len()].into(),
    });
    project.meshes.push(mesh);
    for id in 1..=3 {
        let mut node = Node::new(id, format!("Instance {id}"));
        node.mesh = Some(20);
        node.morph_weights = vec![0.];
        project.nodes.push(node);
    }
    project.nodes.push(Node::new(4, "Parent"));
    project.node_mut(3).unwrap().parent = Some(4);
    project.validate().unwrap();
    project
}

#[test]
fn fresh_material_isolates_selected_instance_and_is_one_undo_step() {
    let mut editor = Editor::new(scene());
    let before = editor.project.clone();
    let assigned = editor
        .create_material(20, &[1], MaterialPreset::Ceramic)
        .unwrap();
    assert_ne!(assigned.mesh, 20);
    assert_ne!(assigned.material, 10);
    assert_eq!(editor.project.node(1).unwrap().mesh, Some(assigned.mesh));
    for id in [2, 3] {
        assert_eq!(editor.project.node(id).unwrap().mesh, Some(20));
    }
    assert_eq!(editor.project.mesh(20), before.mesh(20));
    let mut expected_mesh = before.mesh(20).unwrap().clone();
    expected_mesh.id = assigned.mesh;
    expected_mesh.material = Some(assigned.material);
    assert_eq!(editor.project.mesh(assigned.mesh), Some(&expected_mesh));
    assert_eq!(
        editor
            .project
            .material(assigned.material)
            .unwrap()
            .coat_weight,
        0.75
    );
    assert!(editor
        .project
        .material(assigned.material)
        .unwrap()
        .base_color_texture
        .is_none());
    assert_eq!(editor.project.textures, before.textures);
    assert_eq!(editor.undo_len(), 1);
    let after = editor.project.clone();
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    assert!(editor.redo());
    assert_eq!(editor.project, after);
}

#[test]
fn assignment_reuses_a_material_but_copies_only_shared_mesh_ownership() {
    let mut editor = Editor::new(scene());
    let before = editor.project.clone();
    let assigned = editor.assign_material(20, &[1, 3], 11).unwrap();
    assert_eq!(assigned.material, 11);
    assert_eq!(editor.project.materials, before.materials);
    assert_eq!(editor.project.node(1).unwrap().mesh, Some(assigned.mesh));
    assert_eq!(editor.project.node(3).unwrap().mesh, Some(assigned.mesh));
    assert_eq!(editor.project.node(2).unwrap().mesh, Some(20));
    assert_eq!(editor.project.mesh(20).unwrap().material, Some(10));
    let after = editor.project.clone();
    assert_eq!(
        editor.assign_material(assigned.mesh, &[1, 3], 11).unwrap(),
        assigned
    );
    assert_eq!(editor.project, after);
    assert_eq!(editor.undo_len(), 1);
}

#[test]
fn unchanged_assignment_and_explicit_all_instances_avoid_extra_meshes() {
    let mut editor = Editor::new(scene());
    let before = editor.project.clone();
    assert_eq!(editor.assign_material(20, &[1], 10).unwrap().mesh, 20);
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), 0);
    assert!(!editor.modified);
    let assigned = editor.assign_material(20, &[], 11).unwrap();
    assert_eq!(assigned.mesh, 20);
    assert_eq!(editor.project.meshes.len(), 1);
    assert!(editor
        .project
        .nodes
        .iter()
        .filter(|n| n.mesh.is_some())
        .all(|n| n.mesh == Some(20)));
    assert_eq!(editor.project.mesh(20).unwrap().material, Some(11));
    assert_eq!(editor.undo_len(), 1);
}

#[test]
fn separate_material_keeps_all_maps_and_scalars_without_touching_outside_instances() {
    let mut editor = Editor::new(scene());
    let before = editor.project.clone();
    let assigned = editor.duplicate_material(20, &[1]).unwrap();
    let mut copy = editor.project.material(assigned.material).unwrap().clone();
    assert!(copy.name.contains("копия"));
    copy.id = 10;
    copy.name = "Authored".into();
    assert_eq!(Some(&copy), before.material(10));
    assert_eq!(editor.project.material(10), before.material(10));
    assert_eq!(editor.project.mesh(20), before.mesh(20));
    assert_eq!(editor.project.textures, before.textures);
    assert_eq!(editor.undo_len(), 1);
    assert!(editor.undo());
    assert_eq!(editor.project, before);
}

#[test]
fn bad_owner_missing_resources_and_locked_ancestors_leave_everything_unchanged() {
    let mut editor = Editor::new(scene());
    let before = editor.project.clone();
    for (mesh, owners, material) in [
        (999, vec![1], 11),
        (20, vec![999], 11),
        (20, vec![4], 11),
        (20, vec![1, 1], 11),
        (20, vec![1], 999),
    ] {
        assert!(editor.assign_material(mesh, &owners, material).is_err());
        assert_eq!(editor.project, before);
        assert_eq!(editor.undo_len(), 0);
    }
    editor.project.node_mut(4).unwrap().locked = true;
    let locked = editor.project.clone();
    assert!(editor
        .create_material(20, &[3], MaterialPreset::Matte)
        .is_err());
    assert!(editor.duplicate_material(20, &[3]).is_err());
    assert!(editor.assign_material(20, &[], 11).is_err());
    assert_eq!(editor.project, locked);
    assert_eq!(editor.undo_len(), 0);
    // A locked outside instance does not prevent an isolated edit elsewhere.
    assert!(editor.assign_material(20, &[1], 11).is_ok());
    assert_eq!(editor.project.node(3).unwrap().mesh, Some(20));
}

#[test]
fn exhausted_ids_fail_before_material_or_mesh_publication() {
    let mut project = scene();
    project.materials.push(Material {
        id: u64::MAX,
        ..Material::default()
    });
    let mut editor = Editor::new(project);
    let before = editor.project.clone();
    assert!(editor
        .create_material(20, &[1], MaterialPreset::Matte)
        .is_err());
    assert!(editor.duplicate_material(20, &[1]).is_err());
    assert!(editor.assign_material(20, &[1], 11).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo_len(), 0);
    // Resource reuse needs no fresh ID when the assignment is already correct.
    assert!(editor.assign_material(20, &[1], 10).is_ok());
}

#[test]
fn preset_response_preserves_authored_images_color_emission_alpha_and_name() {
    for preset in MaterialPreset::ALL {
        let mut editor = Editor::new(scene());
        let original = editor.project.material(10).unwrap().clone();
        editor.apply_material_preset(10, preset).unwrap();
        let changed = editor.project.material(10).unwrap();
        let mut response_only = original.clone();
        preset.apply(&mut response_only);
        assert_eq!(changed, &response_only);
        assert_eq!(changed.base_color, original.base_color);
        assert_eq!(changed.normal_texture, original.normal_texture);
        assert_eq!(changed.alpha_mode, AlphaMode::Blend);
        assert_eq!(changed.emissive, original.emissive);
        assert_eq!(editor.undo_len(), 1);
        editor.apply_material_preset(10, preset).unwrap();
        assert_eq!(editor.undo_len(), 1);
        assert!(editor.undo());
        assert_eq!(editor.project.material(10), Some(&original));
        let mut project = Project::default();
        project.materials.push(preset.create());
        project.validate().unwrap();
    }
}

#[test]
fn unicode_rename_rejects_invalid_input_and_ignores_unchanged_names() {
    let mut editor = Editor::new(scene());
    editor.rename_material(10, "  Керамика · белая  ").unwrap();
    assert_eq!(
        editor.project.material(10).unwrap().name,
        "Керамика · белая"
    );
    let renamed = editor.project.clone();
    editor.rename_material(10, "Керамика · белая").unwrap();
    assert_eq!(editor.undo_len(), 1);
    for name in ["", " \t ", "line\nbreak", &"я".repeat(129)] {
        assert!(editor.rename_material(10, name).is_err());
        assert_eq!(editor.project, renamed);
        assert_eq!(editor.undo_len(), 1);
    }
    assert!(editor.rename_material(999, "Missing").is_err());
    assert!(editor
        .apply_material_preset(999, MaterialPreset::Plastic)
        .is_err());
}

#[test]
fn removing_assignment_isolates_shared_mesh_and_preserves_library_with_atomic_undo() {
    let mut e = Editor::new(scene());
    let before = e.project.clone();
    let detached = e.unassign_material(20, &[1]).unwrap();
    assert_ne!(detached, 20);
    assert_eq!(e.project.node(1).unwrap().mesh, Some(detached));
    assert_eq!(e.project.mesh(detached).unwrap().material, None);
    assert_eq!(e.project.mesh(20).unwrap().material, Some(10));
    assert_eq!(e.project.materials, before.materials);
    e.project.validate().unwrap();
    assert_eq!(e.undo_len(), 1);
    assert_eq!(e.unassign_material(detached, &[1]).unwrap(), detached);
    assert_eq!(e.undo_len(), 1);
    assert!(e.undo());
    assert_eq!(e.project, before);
    assert!(e.redo());
    assert_eq!(e.project.mesh(detached).unwrap().material, None);
}
#[test]
fn unassign_rejects_locked_owners_and_pruning_preserves_all_mesh_references() {
    let mut e = Editor::new(scene());
    e.project.node_mut(4).unwrap().locked = true;
    let before = e.project.clone();
    assert!(e.unassign_material(20, &[3]).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.remove_unused_materials().unwrap(), 1);
    assert!(e.project.material(10).is_some());
    assert!(e.project.material(11).is_none());
    e.project.validate().unwrap();
    assert_eq!(e.remove_unused_materials().unwrap(), 0);
    assert_eq!(e.undo_len(), 1);
    assert!(e.undo());
    assert_eq!(e.project, before);
}
