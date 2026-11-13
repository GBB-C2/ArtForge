use poseforge_core::*;
use poseforge_io::{load_project, merge_project, save_project};
use std::collections::BTreeSet;
#[test]
fn face_materials_save_load_and_merge_remap_every_reference() {
    let mut n = Node::new(1, "Cube");
    n.mesh = Some(2);
    let mut e = Editor::new(Project {
        nodes: vec![n],
        meshes: vec![PrimitiveSettings::new(PrimitiveKind::Cube)
            .mesh(2, Some(3))
            .unwrap()],
        materials: vec![
            Material {
                id: 3,
                ..Material::default()
            },
            Material {
                id: 4,
                ..Material::default()
            },
        ],
        ..Project::default()
    });
    e.assign_face_material(2, &[1], &BTreeSet::from([2, 3]), 4)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("materials.artforge");
    save_project(&e.project, &path).unwrap();
    let p = load_project(&path).unwrap();
    assert_eq!(p, e.project);
    let mut merged = p.clone();
    let roots = merge_project(&mut merged, p).unwrap();
    let m = merged
        .mesh(merged.node(roots[0]).unwrap().mesh.unwrap())
        .unwrap();
    assert_ne!(m.face_material(2), Some(4));
    assert_ne!(m.material, Some(3));
    assert!(m
        .assigned_materials()
        .iter()
        .all(|&id| merged.material(id).is_some()));
    merged.validate().unwrap();
}
#[test]
fn schema_fifteen_without_assignments_migrates_without_modifying_file() {
    let p = Project::default();
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 15.into();
    let bytes = serde_json::to_vec(&json).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.artforge");
    std::fs::write(&path, &bytes).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
