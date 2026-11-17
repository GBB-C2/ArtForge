use poseforge_core::*;
fn closed(mesh: &Mesh) {
    let r = inspect_topology(mesh);
    assert_eq!(
        (
            r.boundary_edges,
            r.nonmanifold_edges,
            r.winding_edges,
            r.degenerate_faces
        ),
        (0, 0, 0, 0),
        "{r:?}"
    );
}
#[test]
fn every_profile_is_closed_and_keeps_topology_while_bending() {
    let mut shapes = Vec::new();
    for kind in CableKind::ALL {
        let s = CableSettings {
            kind,
            ..Default::default()
        };
        let mesh = s.mesh(2, None).unwrap();
        closed(&mesh);
        let mut points = s.centers().unwrap();
        let last = points.len() - 1;
        for (i, p) in points.iter_mut().enumerate() {
            p.z += 0.3 * (std::f32::consts::PI * i as f32 / last as f32).sin();
        }
        let moved = s.mesh_at(&points, 2, None).unwrap();
        closed(&moved);
        assert_eq!(mesh.vertices.len(), moved.vertices.len());
        assert_eq!(mesh.indices, moved.indices);
        assert_eq!(
            mesh.topology.as_ref().unwrap().faces,
            moved.topology.as_ref().unwrap().faces
        );
        assert_ne!(mesh.vertices, moved.vertices);
        assert!(shapes
            .iter()
            .all(|other: &Mesh| other.vertices != mesh.vertices));
        shapes.push(mesh);
    }
}
#[test]
fn round_profile_retains_legacy_geometry_and_missing_fields_defaults() {
    let legacy = r#"{"start":[-1,2,0],"end":[1,2,0],"radius":0.04,"segments":48,"sides":8,"sag":0.25,"pin_start":true,"pin_end":true}"#;
    let s: CableSettings = serde_json::from_str(legacy).unwrap();
    assert_eq!(s, CableSettings::default());
    let old = CurveData {
        points: s.centers().unwrap().iter().map(|p| p.to_array()).collect(),
        interpolation: CurveInterpolation::Polyline,
        resolution: 1,
        closed: false,
        geometry: Some(CurveGeometry {
            radius: s.radius,
            sides: s.sides,
            ..Default::default()
        }),
    }
    .mesh(2, None)
    .unwrap();
    let current = s.mesh(2, None).unwrap();
    assert_eq!(old.vertices, current.vertices);
    assert_eq!(old.indices, current.indices);
}
#[test]
fn details_caps_taper_and_collision_envelopes_follow_the_profile() {
    for kind in CableKind::ALL {
        let s = CableSettings {
            kind,
            segments: 128,
            sides: 12,
            strands: 4,
            turns: -12.,
            ribs: 40,
            taper: 0.5,
            rotation: 38.,
            uv_repeat: 3.,
            ..Default::default()
        };
        let m = s.mesh(2, None).unwrap();
        closed(&m);
        let centers = s.centers().unwrap();
        assert_eq!(centers[0].to_array(), s.start);
        assert_eq!(centers.last().unwrap().to_array(), s.end);
        let open = CableSettings { caps: false, ..s }.mesh(2, None).unwrap();
        let r = inspect_topology(&open);
        assert_eq!(
            r.boundary_edges,
            2 * s.sides as usize
                * if matches!(kind, CableKind::Twisted | CableKind::Bundle) {
                    s.strands as usize
                } else {
                    1
                }
        );
        assert!(open.vertices.iter().any(|v| v.uv[0] > 2.9));
        assert!(s.collision_radius() >= s.radius * s.taper.max(1.) || kind == CableKind::Ribbon);
    }
}
#[test]
fn largest_profiles_stay_within_render_and_physics_budgets() {
    for kind in CableKind::ALL {
        let s = CableSettings {
            kind,
            segments: 256,
            sides: 16,
            strands: 6,
            turns: 32.,
            ribs: 64,
            ..Default::default()
        };
        let mesh = s.mesh(2, None).unwrap();
        assert!(mesh.vertices.len() <= MAX_EDIT_VERTICES);
        let mut n = Node::new(1, "Cable");
        n.mesh = Some(2);
        n.cable = Some(s);
        n.physics = Some(PhysicsSettings {
            kind: PhysicsKind::Cable,
            ..Default::default()
        });
        Project {
            nodes: vec![n],
            meshes: vec![mesh],
            ..Default::default()
        }
        .validate()
        .unwrap();
    }
}
#[test]
fn invalid_profile_values_are_rejected() {
    for mut s in [
        CableSettings {
            radius: f32::NAN,
            ..Default::default()
        },
        CableSettings {
            turns: 33.,
            ..Default::default()
        },
        CableSettings {
            strands: 7,
            ..Default::default()
        },
        CableSettings {
            groove: 1.,
            ..Default::default()
        },
    ] {
        assert!(s.mesh(2, None).is_err());
        s.start = s.end;
        assert!(s.centers().is_err());
    }
}
