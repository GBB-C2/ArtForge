use poseforge_core::*;
use std::collections::BTreeSet;

fn editor(mesh: Mesh) -> Editor {
    let mut node = Node::new(1, "Mesh");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![mesh],
        ..Default::default()
    })
}
fn strip(fold: f32) -> Editor {
    let mut mesh = PrimitiveSettings::new(PrimitiveKind::Plane)
        .mesh(2, None)
        .unwrap();
    mesh.vertices = vec![
        [0., 0., 0.],
        [1., 0., 0.],
        [2., 0., fold],
        [0., 1., 0.],
        [1., 1., 0.],
        [2., 1., fold],
    ]
    .into_iter()
    .enumerate()
    .map(|(i, position)| Vertex {
        position,
        uv: [i as f32 * 0.1, 0.5],
        ..Default::default()
    })
    .collect();
    let topology = mesh.topology.as_mut().unwrap();
    topology.faces = vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]].into();
    topology.face_materials = Default::default();
    topology.primitive = None;
    mesh.rebuild_polygon_indices().unwrap();
    editor(mesh)
}
fn inset(mode: FaceTreatment, depth: f32) -> MeshEdit {
    MeshEdit::InsetFaces {
        faces: BTreeSet::from([0, 1]),
        inset: 0.2,
        depth,
        mode,
    }
}
fn valid(e: &Editor) {
    e.project.validate().unwrap();
    let r = inspect_topology(e.editable_mesh(1).unwrap());
    assert_eq!(
        (
            r.degenerate_faces,
            r.duplicate_faces,
            r.winding_edges,
            r.nonmanifold_edges
        ),
        (0, 0, 0, 0),
        "{r:?}"
    );
}
#[test]
fn connected_inset_preserves_internal_edges_and_individual_inset_adds_two_borders() {
    let mut region = strip(0.);
    let source = region.project.clone();
    region
        .edit_mesh(1, inset(FaceTreatment::Region, 0.))
        .unwrap();
    valid(&region);
    assert_eq!(region.project.meshes[0].face_count(), 8); // two caps + six outside edges
    let graph = MeshConnectivity::new(&region.project.meshes[0]);
    assert!(graph
        .edge_faces
        .values()
        .any(|faces| faces.contains(&0) && faces.contains(&1)));
    let mut individual = strip(0.);
    individual
        .edit_mesh(1, inset(FaceTreatment::Individual, 0.))
        .unwrap();
    valid(&individual);
    assert_eq!(individual.project.meshes[0].face_count(), 10);
    let graph = MeshConnectivity::new(&individual.project.meshes[0]);
    assert!(!graph
        .edge_faces
        .values()
        .any(|faces| faces.contains(&0) && faces.contains(&1)));
    assert!(region.undo());
    assert_eq!(region.project, source);
}
#[test]
fn normal_inset_follows_the_fold_and_accepts_negative_depth() {
    for depth in [-0.15, 0.15] {
        let mut region = strip(0.8);
        let mut normals = strip(0.8);
        region
            .edit_mesh(1, inset(FaceTreatment::Region, depth))
            .unwrap();
        valid(&region);
        normals
            .edit_mesh(1, inset(FaceTreatment::Normals, depth))
            .unwrap();
        valid(&normals);
        assert_ne!(region.project.meshes, normals.project.meshes);
        assert_eq!(
            region.project.meshes[0].face_count(),
            normals.project.meshes[0].face_count()
        );
    }
}
#[test]
fn inset_multiple_islands_stays_local_and_retains_undo() {
    let mut e = strip(0.);
    let mesh = &mut e.project.meshes[0];
    let mut faces = mesh.topology.as_ref().unwrap().faces.to_vec();
    let old = mesh.vertices.to_vec();
    let n = old.len() as u32;
    for mut v in old {
        v.position[0] += 10.;
        mesh.vertices.push(v);
    }
    faces.extend(
        faces
            .clone()
            .into_iter()
            .map(|r| r.into_iter().map(|v| v + n).collect()),
    );
    mesh.topology.as_mut().unwrap().faces = faces.into();
    mesh.rebuild_polygon_indices().unwrap();
    e.edit_mesh(
        1,
        MeshEdit::InsetFaces {
            faces: (0..4).collect(),
            inset: 0.2,
            depth: 0.,
            mode: FaceTreatment::Region,
        },
    )
    .unwrap();
    valid(&e);
    let mesh = &e.project.meshes[0];
    for f in 0..4 {
        for &v in mesh.face_vertices(f).unwrap() {
            let x = mesh.vertices[v as usize].position[0];
            assert!(if f < 2 { x < 3. } else { x > 9. });
        }
    }
}
#[test]
fn bevel_modes_keep_segments_signed_profiles_and_closed_topology() {
    for mode in [
        FaceTreatment::Region,
        FaceTreatment::Normals,
        FaceTreatment::Individual,
    ] {
        for profile in [-0.8, 0., 0.8] {
            let mut e = editor(
                PrimitiveSettings::new(PrimitiveKind::Cube)
                    .mesh(2, None)
                    .unwrap(),
            );
            let source = e.project.clone();
            e.edit_mesh(
                1,
                MeshEdit::FaceBevelMode {
                    faces: BTreeSet::from([0, 2]),
                    inset: 0.15,
                    shift: 0.05,
                    segments: 4,
                    profile,
                    mode,
                },
            )
            .unwrap_or_else(|e| panic!("{mode:?} / {profile}: {e}"));
            valid(&e);
            assert_eq!(inspect_topology(&e.project.meshes[0]).boundary_edges, 0);
            assert!(e.project.meshes[0].face_count() > 12);
            assert!(e.undo());
            assert_eq!(e.project, source);
        }
    }
}
#[test]
fn invalid_mode_parameters_are_atomic() {
    let mut e = strip(0.);
    let source = e.project.clone();
    for operation in [
        MeshEdit::InsetFaces {
            faces: BTreeSet::from([0, 1]),
            inset: f32::NAN,
            depth: 0.,
            mode: FaceTreatment::Region,
        },
        MeshEdit::FaceBevelMode {
            faces: BTreeSet::from([0]),
            inset: 0.1,
            shift: 0.,
            segments: 0,
            profile: 0.,
            mode: FaceTreatment::Individual,
        },
    ] {
        assert!(e.edit_mesh(1, operation).is_err());
        assert_eq!(e.project, source);
        assert_eq!(e.undo_len(), 0);
    }
}

#[test]
fn region_bevel_keeps_adjacent_caps_connected_on_closed_and_open_meshes() {
    for closed in [false, true] {
        for fold in [0., 0.8] {
            for segments in [1, 4] {
                for profile in [-0.8, 0.8] {
                    let source = if closed {
                        editor(
                            PrimitiveSettings::new(PrimitiveKind::Cube)
                                .mesh(2, None)
                                .unwrap(),
                        )
                    } else {
                        strip(fold)
                    };
                    let faces = if closed {
                        BTreeSet::from([0, 2])
                    } else {
                        BTreeSet::from([0, 1])
                    };
                    for mode in [
                        FaceTreatment::Region,
                        FaceTreatment::Normals,
                        FaceTreatment::Individual,
                    ] {
                        let mut e = Editor::new(source.project.clone());
                        e.edit_mesh(1,MeshEdit::FaceBevelMode{faces:faces.clone(),inset:0.15,shift:0.05,segments,profile,mode}).unwrap_or_else(|err|panic!("closed {closed}, fold {fold}, {mode:?}, segments {segments}, profile {profile}: {err}"));
                        valid(&e);
                        let mesh = e.editable_mesh(1).unwrap();
                        let graph = MeshConnectivity::new(mesh);
                        let joined = graph
                            .edge_faces
                            .values()
                            .any(|adj| faces.iter().all(|f| adj.contains(f)));
                        assert_eq!(
                            joined,
                            mode != FaceTreatment::Individual,
                            "closed {closed}, fold {fold}, {mode:?}"
                        );
                        if closed {
                            assert_eq!(inspect_topology(mesh).boundary_edges, 0);
                        } else {
                            assert_eq!(
                                mesh.face_count(),
                                2 + if mode == FaceTreatment::Individual {
                                    8
                                } else {
                                    6
                                } * segments as usize
                            );
                        }
                        assert!(e.undo());
                        assert_eq!(e.project, source.project);
                    }
                }
            }
        }
    }
}

#[test]
fn region_bevel_joins_profiles_across_split_uv_vertices() {
    let mut e = strip(0.8);
    let mesh = &mut e.project.meshes[0];
    for index in [1, 4] {
        let mut vertex = mesh.vertices[index];
        vertex.uv = [0.9, 0.1];
        let duplicate = mesh.vertices.len() as u32;
        mesh.vertices.push(vertex);
        for v in &mut mesh.topology.as_mut().unwrap().faces[1] {
            if *v == index as u32 {
                *v = duplicate;
            }
        }
    }
    mesh.rebuild_polygon_indices().unwrap();
    for mode in [FaceTreatment::Region, FaceTreatment::Normals] {
        for profile in [-1., 1.] {
            let mut result = Editor::new(e.project.clone());
            result
                .edit_mesh(
                    1,
                    MeshEdit::FaceBevelMode {
                        faces: BTreeSet::from([0, 1]),
                        inset: 0.2,
                        shift: -0.1,
                        segments: 5,
                        profile,
                        mode,
                    },
                )
                .unwrap();
            valid(&result);
            let mesh = result.editable_mesh(1).unwrap();
            assert_eq!(inspect_topology(mesh).boundary_edges, 6);
            assert!(MeshConnectivity::new(mesh)
                .edge_faces
                .values()
                .any(|fs| fs.contains(&0) && fs.contains(&1)));
            assert!(mesh
                .face_vertices(1)
                .unwrap()
                .iter()
                .all(|&v| mesh.vertices[v as usize].uv == [0.9, 0.1]
                    || mesh.vertices[v as usize].uv[1] == 0.5));
        }
    }
}

#[test]
fn region_bevel_supports_an_open_concave_polygon_without_internal_borders() {
    let mut mesh = PrimitiveSettings::new(PrimitiveKind::Plane)
        .mesh(2, None)
        .unwrap();
    mesh.vertices = [
        [0., 0., 0.],
        [3., 0., 0.],
        [3., 3., 0.],
        [2., 3., 0.],
        [2., 1., 0.],
        [1., 1., 0.],
        [1., 3., 0.],
        [0., 3., 0.],
    ]
    .into_iter()
    .map(|position| Vertex {
        position,
        ..Default::default()
    })
    .collect();
    mesh.topology.as_mut().unwrap().faces = vec![(0..8).collect()].into();
    mesh.topology.as_mut().unwrap().primitive = None;
    mesh.rebuild_polygon_indices().unwrap();
    for profile in [-1., 0., 1.] {
        let mut e = editor(mesh.clone());
        e.edit_mesh(
            1,
            MeshEdit::FaceBevelMode {
                faces: BTreeSet::from([0]),
                inset: 0.2,
                shift: 0.,
                segments: 4,
                profile,
                mode: FaceTreatment::Region,
            },
        )
        .unwrap();
        valid(&e);
        assert_eq!(
            inspect_topology(e.editable_mesh(1).unwrap()).boundary_edges,
            8
        );
        assert_eq!(e.editable_mesh(1).unwrap().face_count(), 33);
    }
}

#[test]
fn region_bevel_distance_moves_the_entire_profile_in_both_directions() {
    let source = PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap();
    let faces = BTreeSet::from([0, 2]);
    let direction = (Vec3::X + Vec3::Y).normalize();
    for profile in [-0.8, 0., 0.8] {
        let operation = |shift| MeshEdit::FaceBevelMode {
            faces: faces.clone(),
            inset: 0.15,
            shift,
            segments: 4,
            profile,
            mode: FaceTreatment::Region,
        };
        let mut base = editor(source.clone());
        base.edit_mesh(1, operation(0.)).unwrap();
        for shift in [-0.3, -0.05, 0.05, 0.3] {
            let mut result = editor(source.clone());
            let before = result.project.clone();
            result
                .edit_mesh(1, operation(shift))
                .unwrap_or_else(|e| panic!("profile {profile}, shift {shift}: {e}"));
            valid(&result);
            let mesh = result.editable_mesh(1).unwrap();
            assert_eq!(inspect_topology(mesh).boundary_edges, 0);
            let original = base.editable_mesh(1).unwrap();
            assert_eq!(mesh.face_count(), original.face_count());
            let mut intermediate = 0;
            for vertex in &mesh.vertices {
                let point = Vec3::from_array(vertex.position);
                // Every point remains on its original profile's displacement
                // line; changing height must not leave only the cap moving.
                let matching = original
                    .vertices
                    .iter()
                    .filter_map(|v| {
                        let delta = point - Vec3::from_array(v.position);
                        let height = delta.dot(direction);
                        ((delta - direction * height).length() < 1e-5
                            && (height / shift) >= -1e-5
                            && (height / shift) <= 1.00001)
                            .then_some(height / shift)
                    })
                    .collect::<Vec<_>>();
                assert!(
                    !matching.is_empty(),
                    "profile {profile}, distance {shift}: {point:?}"
                );
                if matching.iter().all(|&t| t > 0.01 && t < 0.99) {
                    intermediate += 1;
                }
            }
            assert!(
                intermediate > 0,
                "the segmented profile stayed still at distance {shift}"
            );
            let graph = MeshConnectivity::new(mesh);
            assert!(graph
                .edge_faces
                .values()
                .any(|adj| adj.contains(&0) && adj.contains(&2)));
            assert!(result.undo());
            assert_eq!(result.project, before);
        }
    }
}

#[test]
fn region_bevel_distance_uses_the_source_direction_at_different_widths() {
    let mut source = PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap();
    for vertex in &mut source.vertices {
        let p = Vec3::from_array(vertex.position) * Vec3::new(1., 1.7, 0.6);
        vertex.position = [p.x + p.y * 0.4, p.y, p.z];
    }
    let faces = BTreeSet::from([0, 2, 4]);
    let mut direction = Vec3::ZERO;
    for &f in &faces {
        let face = source.face_vertices(f).unwrap();
        let p = |i: usize| Vec3::from_array(source.vertices[face[i] as usize].position);
        for i in 1..face.len() - 1 {
            direction += (p(i) - p(0)).cross(p(i + 1) - p(0));
        }
    }
    direction = direction.normalize();
    for inset in [0.05, 0.35] {
        let operation = |shift| MeshEdit::FaceBevelMode {
            faces: faces.clone(),
            inset,
            shift,
            segments: 5,
            profile: 0.6,
            mode: FaceTreatment::Region,
        };
        let mut base = editor(source.clone());
        base.edit_mesh(1, operation(0.)).unwrap();
        for shift in [-0.1, 0.1] {
            let mut result = editor(source.clone());
            result
                .edit_mesh(1, operation(shift))
                .unwrap_or_else(|e| panic!("inset {inset}, shift {shift}: {e}"));
            valid(&result);
            let original = base.editable_mesh(1).unwrap();
            let mesh = result.editable_mesh(1).unwrap();
            for &f in &faces {
                for &v in original.face_vertices(f).unwrap() {
                    let expected = Vec3::from_array(original.vertices[v as usize].position)
                        + direction * shift;
                    assert!(
                        mesh.face_vertices(f).unwrap().iter().any(|&v| {
                            Vec3::from_array(mesh.vertices[v as usize].position).distance(expected)
                                < 1e-5
                        }),
                        "inset {inset}, shift {shift}, face {f}: {expected:?}"
                    );
                }
            }
        }
    }
}
