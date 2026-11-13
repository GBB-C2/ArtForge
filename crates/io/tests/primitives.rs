use poseforge_core::*;
use poseforge_io::{load_project, save_project};
#[test]
fn editable_primitive_topology_and_parameters_roundtrip_and_invalid_save_is_atomic() {
    let mut p = Project::default();
    p.meshes.push(
        PrimitiveSettings::new(PrimitiveKind::Sphere)
            .mesh(2, None)
            .unwrap(),
    );
    let mut n = Node::new(1, "Sphere");
    n.mesh = Some(2);
    p.nodes.push(n);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quads.artforge");
    save_project(&p, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), p);
    let before = std::fs::read(&path).unwrap();
    p.meshes[0].topology.as_mut().unwrap().faces[0][0] = 999999;
    assert!(save_project(&p, &path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn schema_twelve_and_earlier_keep_triangle_meshes_and_source_bytes() {
    let mut p = Project::default();
    p.meshes.push(box_mesh(2, "Legacy", Vec3::ONE, None));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.artforge");
    for version in 1..=12 {
        let mut json = serde_json::to_value(&p).unwrap();
        json["version"] = version.into();
        json["meshes"][0]
            .as_object_mut()
            .unwrap()
            .remove("topology");
        let bytes = serde_json::to_vec(&json).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.meshes, p.meshes);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn schema_twelve_preserves_quad_primitives_without_the_new_normal_policy_field() {
    let mut p = Project::default();
    p.meshes.push(
        PrimitiveSettings::new(PrimitiveKind::Sphere)
            .mesh(2, None)
            .unwrap(),
    );
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 12.into();
    json["meshes"][0]["topology"]
        .as_object_mut()
        .unwrap()
        .remove("smooth_normals");
    let bytes = serde_json::to_vec(&json).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema12.artforge");
    std::fs::write(&path, &bytes).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert_eq!(loaded.meshes[0].vertices, p.meshes[0].vertices);
    assert_eq!(loaded.meshes[0].indices, p.meshes[0].indices);
    let topology = loaded.meshes[0].topology.as_ref().unwrap();
    assert_eq!(topology.faces, p.meshes[0].topology.as_ref().unwrap().faces);
    assert_eq!(
        topology.primitive,
        p.meshes[0].topology.as_ref().unwrap().primitive
    );
    assert!(loaded.meshes[0].smooth_normals_enabled());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    save_project(&loaded, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), loaded);
}

#[test]
fn fox_primitive_roundtrip_preserves_editable_quads_uvs_and_settings() {
    let mut p = Project::default();
    p.meshes.push(
        PrimitiveSettings::new(PrimitiveKind::Fox)
            .mesh(2, None)
            .unwrap(),
    );
    let mut n = Node::new(1, "Лиса");
    n.mesh = Some(2);
    p.nodes.push(n);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("fox.artforge");
    save_project(&p, &file).unwrap();
    assert_eq!(load_project(file).unwrap(), p);
}
