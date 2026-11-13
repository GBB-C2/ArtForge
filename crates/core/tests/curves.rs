use poseforge_core::*;

#[test]
fn curve_presets_have_bounded_continuous_samples_and_optional_geometry() {
    for preset in CurvePreset::ALL {
        let mut curve = preset.curve();
        assert!(curve.geometry.is_none());
        let samples = curve.samples().unwrap();
        assert!((2..=MAX_CURVE_SAMPLES).contains(&samples.len()));
        assert!(samples.iter().all(|p| p.is_finite()));
        assert!(samples.windows(2).all(|p| p[0].distance_squared(p[1]) > 1e-12));
        if curve.closed { assert_eq!(samples.first(), samples.last()); }
        for profile in [CurveProfile::Tube, CurveProfile::Ribbon] {
            curve.geometry = Some(CurveGeometry { profile, ..Default::default() });
            let mesh = curve.mesh(1, None).unwrap();
            mesh.validate_topology().unwrap();
            assert!(mesh.vertices.len() <= MAX_EDIT_VERTICES);
            assert!(mesh.vertices.iter().all(|v| v.position.iter().chain(&v.normal).chain(&v.uv).all(|x| x.is_finite())));
            assert!(mesh.topology.as_ref().unwrap().primitive.is_none());
            assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()));
        }
    }
}

#[test]
fn source_points_geometry_and_conversion_have_independent_undo_steps() {
    let mut editor = Editor::new(Project::default());
    let id = editor.create_curve("Curve", CurvePreset::Bezier.curve()).unwrap();
    assert!(editor.project.node(id).unwrap().mesh.is_none());
    assert!(editor.project.meshes.is_empty());
    let line_only = editor.project.clone();
    let mut curve = editor.project.node(id).unwrap().curve.clone().unwrap();
    curve.geometry = Some(Default::default()); editor.set_curve(id, curve.clone()).unwrap();
    let mesh_id = editor.project.node(id).unwrap().mesh.unwrap();
    let geometry = editor.project.clone();
    assert!(editor.editable_mesh(id).is_err());
    assert!(editor.undo()); assert_eq!(editor.project, line_only);
    assert!(editor.redo()); assert_eq!(editor.project, geometry);
    curve.points[1][1] += 0.5; editor.set_curve(id, curve).unwrap();
    assert_eq!(editor.project.node(id).unwrap().mesh, Some(mesh_id));
    assert_ne!(editor.project.mesh(mesh_id), geometry.mesh(mesh_id));
    editor.convert_curve_to_mesh(id).unwrap();
    assert!(editor.project.node(id).unwrap().curve.is_none());
    assert!(editor.editable_mesh(id).is_ok());
    assert!(editor.undo()); assert!(editor.project.node(id).unwrap().curve.is_some());
}

#[test]
fn removing_geometry_cleans_only_the_derived_mesh_and_undo_restores_layers() {
    let mut editor = Editor::new(Project::default());
    let id = editor.create_curve("Curve", CurvePreset::Circle.curve()).unwrap();
    let mut curve = editor.project.node(id).unwrap().curve.clone().unwrap();
    curve.geometry = Some(Default::default()); editor.set_curve(id, curve.clone()).unwrap();
    editor.add_material_layer(id, None).unwrap();
    let before = editor.project.clone();
    curve.geometry = None; editor.set_curve(id, curve).unwrap();
    assert!(editor.project.meshes.is_empty());
    assert!(editor.project.node(id).unwrap().material_layers.is_empty());
    assert_eq!(editor.project.materials, before.materials);
    assert!(editor.undo()); assert_eq!(editor.project, before);
}

#[test]
fn editing_a_duplicate_detaches_derived_geometry_and_preserves_an_unassigned_material() {
    let mut editor = Editor::new(Project::default());
    let id = editor.create_curve("Source", CurvePreset::Line.curve()).unwrap();
    let mut curve = CurvePreset::Line.curve(); curve.geometry = Some(Default::default());
    editor.set_curve(id, curve.clone()).unwrap();
    let mesh = editor.project.node(id).unwrap().mesh.unwrap();
    let duplicate = editor.project.next_id(); let mut node = editor.project.node(id).unwrap().clone(); node.id = duplicate;
    editor.transact("Duplicate", |p| { p.nodes.push(node); p.meshes.iter_mut().find(|m| m.id == mesh).unwrap().material = None; }).unwrap();
    let original = editor.project.mesh(mesh).unwrap().clone();
    curve.points[0][1] += 1.; editor.set_curve(duplicate, curve).unwrap();
    assert_ne!(editor.project.node(duplicate).unwrap().mesh, Some(mesh));
    assert_eq!(editor.project.mesh(mesh), Some(&original));
    assert!(editor.project.mesh(editor.project.node(duplicate).unwrap().mesh.unwrap()).unwrap().material.is_none());
    editor.project.validate().unwrap();
}

#[test]
fn invalid_points_and_locked_edits_do_not_change_history_or_source() {
    let mut editor = Editor::new(Project::default());
    let id = editor.create_curve("Curve", CurvePreset::Bezier.curve()).unwrap();
    let before = editor.project.clone(); let undo = editor.undo_len();
    let mut invalid = CurvePreset::Bezier.curve(); invalid.points[1][0] = f32::NAN;
    assert!(editor.set_curve(id, invalid).is_err());
    assert_eq!(editor.project, before); assert_eq!(editor.undo_len(), undo);
    editor.project.node_mut(id).unwrap().locked = true;
    assert!(editor.set_curve(id, CurvePreset::Circle.curve()).is_err());
    assert!(editor.create_curve("Zero", CurveData { points: vec![[0.;3]; 2], ..Default::default() }).is_err());
    assert!(CurveData { points: vec![[0.;3]; MAX_CURVE_POINTS + 1], ..Default::default() }.validate().is_err());
}
