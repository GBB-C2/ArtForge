use poseforge_core::{box_mesh, DestructionSettings, Node, Project, Vec3, SCHEMA_VERSION};
use poseforge_io::{load_project, save_project};

#[test]
fn fracture_appearance_roundtrips_and_schema26_loads_without_rewriting_the_file() {
    let mut p = Project::default();
    let mut node = Node::new(1, "Fracture"); node.mesh = Some(2);
    node.destruction = Some(DestructionSettings { seam_percent: 1.25, cut_contrast: 0.9, ..Default::default() });
    p.nodes.push(node); p.meshes.push(box_mesh(2, "Cube", Vec3::ONE, None));
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("style.artforge");
    save_project(&p, &path).unwrap(); assert_eq!(load_project(&path).unwrap(), p);
    let mut old = serde_json::to_value(&p).unwrap(); old["version"] = 26.into();
    let settings = old["nodes"][0]["destruction"].as_object_mut().unwrap();
    settings.remove("seam_percent"); settings.remove("cut_contrast");
    let bytes = serde_json::to_vec(&old).unwrap(); std::fs::write(&path, &bytes).unwrap();
    let migrated = load_project(&path).unwrap();
    assert_eq!(migrated.version, SCHEMA_VERSION); assert_eq!(migrated.meshes, p.meshes);
    let defaults = DestructionSettings::default();
    assert_eq!(migrated.nodes[0].destruction.unwrap().seam_percent, defaults.seam_percent);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn invalid_fracture_appearance_is_rejected() {
    for (gap, contrast) in [(f64::NAN, 0.5), (-0.1, 0.5), (5.1, 0.5), (0., 1.1), (0., f64::INFINITY)] {
        assert!(DestructionSettings { seam_percent: gap, cut_contrast: contrast, ..Default::default() }.validate().is_err());
    }
}
