use poseforge_core::{box_mesh, FluidDomainSettings, Material, Node, PhysicsSettings, Project, Vec3, SCHEMA_VERSION};
use poseforge_io::{load_project, save_project};

fn liquid_project() -> Project {
    let mut p = Project::default();
    p.materials.push(Material { id: 3, ..Default::default() });
    p.meshes.push(box_mesh(2, "Container", Vec3::ONE, Some(3)));
    let mut node = Node::new(1, "Liquid");
    node.mesh = Some(2);
    node.fluid = Some(FluidDomainSettings::default());
    node.transform.translation = Vec3::new(4., 2., -3.);
    p.nodes.push(node);
    p
}

#[test]
fn object_liquid_roundtrips_and_invalid_save_preserves_previous_bytes() {
    let mut p = liquid_project();
    p.nodes[0].fluid.as_mut().unwrap().fill = 0.4;
    p.nodes[0].fluid.as_mut().unwrap().visible = false;
    p.nodes[0].fluid.as_mut().unwrap().simulation.viscosity = 0.16;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("liquid.artforge");
    save_project(&p, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), p);
    let bytes = std::fs::read(&path).unwrap();
    p.nodes[0].fluid.as_mut().unwrap().fill = 0.;
    assert!(save_project(&p, &path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn schema_twenty_opens_without_adding_liquid_or_rewriting_source() {
    let mut p = liquid_project();
    p.nodes[0].fluid = None;
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 20.into();
    json["nodes"][0].as_object_mut().unwrap().remove("fluid");
    let bytes = serde_json::to_vec(&json).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.artforge");
    std::fs::write(&path, &bytes).unwrap();
    let loaded = load_project(&path).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert_eq!(loaded, p);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn domain_requires_volume_and_excludes_cloth() {
    let mut p = liquid_project();
    p.validate().unwrap();
    p.nodes[0].physics = Some(PhysicsSettings::default());
    assert!(p.validate().is_err());
    p.nodes[0].physics = None;
    for vertex in &mut p.meshes[0].vertices { vertex.position[1] = 0.; }
    assert!(p.validate().is_err());
}

#[test]
fn schema_twenty_two_preserves_fill_and_authored_volume_lights() {
    let mut p = liquid_project();
    p.nodes[0].fluid.as_mut().unwrap().fill = 0.65;
    p.render_settings.atmosphere.density = 0.1;
    for light in &mut p.lights {
        light.volume.enabled = false;
    }
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 22.into();
    json["nodes"][0]["fluid"].as_object_mut().unwrap().remove("visible");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("liquid-v22.artforge");
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let restored = load_project(&path).unwrap();
    assert_eq!(restored, p);
    assert!(restored.nodes[0].fluid.unwrap().visible);
    assert_eq!(restored.nodes[0].fluid.unwrap().fill, 0.65);
    assert!(restored.lights.iter().all(|light| !light.volume.enabled));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
