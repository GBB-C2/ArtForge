use poseforge_core::*;
use poseforge_io::{load_project, merge_project, save_project};
#[test]
fn cable_settings_physics_and_collision_targets_roundtrip_and_merge() {
    let settings = CableSettings {
        pin_end: false,
        radius: 0.08,
        ..Default::default()
    };
    let mut node = Node::new(1, "Кабель");
    node.mesh = Some(2);
    node.cable = Some(settings);
    node.physics = Some(PhysicsSettings {
        kind: PhysicsKind::Cable,
        ..Default::default()
    });
    node.collision_targets.cloth = Some(vec![CollisionTarget {
        node: 3,
        enabled: true,
    }]);
    let mut obstacle = Node::new(3, "Препятствие");
    obstacle.mesh = Some(4);
    let p = Project {
        nodes: vec![node, obstacle],
        meshes: vec![
            settings.mesh(2, None).unwrap(),
            PrimitiveSettings::new(PrimitiveKind::Cube)
                .mesh(4, None)
                .unwrap(),
        ],
        ..Default::default()
    };
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("cable.artforge");
    save_project(&p, &file).unwrap();
    let loaded = load_project(&file).unwrap();
    assert_eq!(loaded, p);
    let mut merged = p.clone();
    let ids = merge_project(&mut merged, loaded).unwrap();
    let n = merged.node(ids[0]).unwrap();
    assert_eq!(n.cable, Some(settings));
    assert_eq!(n.collision_targets.cloth.as_ref().unwrap()[0].node, ids[1]);
    merged.validate().unwrap();
}
#[test]
fn schema_45_loads_with_no_cable_and_without_rewriting() {
    let mut old = serde_json::to_value(Project::default()).unwrap();
    old["version"] = 45.into();
    let bytes = serde_json::to_vec(&old).unwrap();
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("old.artforge");
    std::fs::write(&file, &bytes).unwrap();
    let loaded = load_project(&file).unwrap();
    assert_eq!(loaded.version, SCHEMA_VERSION);
    assert!(loaded.nodes.iter().all(|n| n.cable.is_none()));
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}

#[test]
fn all_cable_profiles_and_extended_settings_roundtrip() {
    for kind in CableKind::ALL {
        let settings = CableSettings {
            kind,
            strands: 5,
            turns: -9.,
            spread: 0.2,
            groove: 0.6,
            ribs: 30,
            width: 0.4,
            thickness: 0.06,
            taper: 0.5,
            rotation: 25.,
            uv_repeat: 6.,
            collision_margin: 0.02,
            ..Default::default()
        };
        let mut node = Node::new(1, "Кабель");
        node.cable = Some(settings);
        node.mesh = Some(2);
        node.physics = Some(PhysicsSettings {
            kind: PhysicsKind::Cable,
            ..Default::default()
        });
        let p = Project {
            nodes: vec![node],
            meshes: vec![settings.mesh(2, None).unwrap()],
            ..Default::default()
        };
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("profile.artforge");
        save_project(&p, &file).unwrap();
        assert_eq!(load_project(&file).unwrap(), p);
    }
}
#[test]
fn schema_46_cable_gets_round_profile_defaults_without_rewriting() {
    let settings = CableSettings::default();
    let mut node = Node::new(1, "Кабель");
    node.cable = Some(settings);
    node.mesh = Some(2);
    let p = Project {
        nodes: vec![node],
        meshes: vec![settings.mesh(2, None).unwrap()],
        ..Default::default()
    };
    let mut old = serde_json::to_value(&p).unwrap();
    old["version"] = 46.into();
    old["nodes"][0]["cable"]
        .as_object_mut()
        .unwrap()
        .retain(|k, _| {
            [
                "start",
                "end",
                "radius",
                "segments",
                "sides",
                "sag",
                "pin_start",
                "pin_end",
            ]
            .contains(&k.as_str())
        });
    let bytes = serde_json::to_vec(&old).unwrap();
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("legacy.artforge");
    std::fs::write(&file, &bytes).unwrap();
    assert_eq!(load_project(&file).unwrap(), p);
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}
