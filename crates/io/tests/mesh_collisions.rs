use poseforge_core::{box_mesh, DestructionSettings, FluidDomainSettings, Node, Project, Vec3, SCHEMA_VERSION};
use poseforge_io::{load_project, save_project};

fn scene() -> Project {
    let mut p = Project::default();
    p.meshes.push(box_mesh(10, "Cube", Vec3::ONE, None));
    let mut liquid = Node::new(1, "Liquid"); liquid.mesh = Some(10); liquid.fluid = Some(FluidDomainSettings::default());
    let mut destruction = Node::new(2, "Fracture"); destruction.mesh = Some(10); destruction.destruction = Some(DestructionSettings::default());
    p.nodes = vec![liquid, destruction]; p
}
#[test]
fn independent_collision_switches_roundtrip() {
    let mut p = scene();
    let directory = tempfile::tempdir().unwrap(); let file = directory.path().join("collisions.artforge");
    for (fluid, fracture) in [(true,false),(false,true),(false,false)] {
        p.nodes[0].fluid.as_mut().unwrap().mesh_collision = fluid;
        p.nodes[1].destruction.as_mut().unwrap().mesh_collision = fracture;
        save_project(&p, &file).unwrap();
        assert_eq!(load_project(&file).unwrap(), p);
    }
}
#[test]
fn schema_30_defaults_to_mesh_collisions_without_rewriting_original() {
    let mut json = serde_json::to_value(scene()).unwrap(); json["version"] = 30.into();
    json["nodes"][0]["fluid"].as_object_mut().unwrap().remove("mesh_collision");
    json["nodes"][1]["destruction"].as_object_mut().unwrap().remove("mesh_collision");
    let bytes = serde_json::to_vec(&json).unwrap();
    let directory = tempfile::tempdir().unwrap(); let file = directory.path().join("legacy.artforge");
    std::fs::write(&file, &bytes).unwrap();
    let loaded = load_project(&file).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert!(loaded.nodes[0].fluid.unwrap().mesh_collision);
    assert!(loaded.nodes[1].destruction.unwrap().mesh_collision);
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}
