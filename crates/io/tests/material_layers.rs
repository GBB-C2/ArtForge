use poseforge_core::*;
use poseforge_io::{load_project, merge_project, save_project};

#[test]
fn layer_order_visibility_and_library_membership_roundtrip_and_merge() {
    let mut node = Node::new(1, "Mesh"); node.mesh = Some(2);
    let mut e = Editor::new(Project { nodes: vec![node], meshes: vec![box_mesh(2, "Cube", Vec3::ONE, None)], ..Default::default() });
    let first = e.add_material_layer(1, None).unwrap();
    let second = e.add_material_layer(1, Some(first)).unwrap();
    e.set_material_library(second, true).unwrap();
    e.project.node_mut(1).unwrap().material_layers[0].visible = false;
    let directory = tempfile::tempdir().unwrap(); let path = directory.path().join("layers.artforge");
    save_project(&e.project, &path).unwrap();
    let loaded = load_project(&path).unwrap(); assert_eq!(loaded, e.project);
    let mut merged = e.project.clone(); let roots = merge_project(&mut merged, loaded).unwrap();
    let layers = &merged.node(roots[0]).unwrap().material_layers;
    assert_eq!(layers.len(), 2); assert!(!layers[0].visible && layers[1].visible);
    assert_ne!(layers[0].material, first); assert_ne!(layers[1].material, second);
    assert!(!merged.material(layers[0].material).unwrap().in_library);
    assert!(merged.material(layers[1].material).unwrap().in_library);
    merged.validate().unwrap();
}

#[test]
fn schema_24_materials_remain_in_library_and_migration_does_not_rewrite_source() {
    let mut p = Project::default(); p.materials.push(Material { id: 1, ..Default::default() });
    let mut json = serde_json::to_value(&p).unwrap(); json["version"] = 24.into();
    json["materials"][0].as_object_mut().unwrap().remove("in_library");
    let bytes = serde_json::to_vec(&json).unwrap(); let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.artforge"); std::fs::write(&path, &bytes).unwrap();
    let upgraded = load_project(&path).unwrap();
    assert_eq!(upgraded.version, SCHEMA_VERSION); assert!(upgraded.materials[0].in_library);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
