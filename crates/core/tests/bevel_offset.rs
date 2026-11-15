use poseforge_core::*;
use std::collections::BTreeSet;

fn cube() -> Mesh {
    PrimitiveSettings::new(PrimitiveKind::Cube)
        .mesh(2, None)
        .unwrap()
}
fn editor(mesh: Mesh) -> Editor {
    let mut node = Node::new(1, "Mesh");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![mesh],
        ..Default::default()
    })
}
fn closed(mesh: &Mesh) {
    mesh.validate_topology().unwrap();
    let r = inspect_topology(mesh);
    assert_eq!(
        (
            r.boundary_edges,
            r.nonmanifold_edges,
            r.winding_edges,
            r.degenerate_faces,
            r.duplicate_faces
        ),
        (0, 0, 0, 0, 0),
        "{r:?}"
    );
}

#[test]
fn wide_face_bevel_does_not_stop_at_a_fraction_of_a_neighbour_edge() {
    for inset in [0.4, 0.65, 0.9] {
        let mut e = editor(cube());
        e.edit_mesh(
            1,
            MeshEdit::FaceBevelMode {
                faces: BTreeSet::from([0]),
                inset,
                shift: 0.,
                segments: 4,
                profile: 0.5,
                mode: FaceTreatment::Region,
            },
        )
        .unwrap();
        let mesh = e.editable_mesh(1).unwrap();
        closed(mesh);
        for &v in mesh.face_vertices(0).unwrap() {
            let p = mesh.vertices[v as usize].position;
            assert!(
                (p[1].abs() - (1. - inset)).abs() < 1e-5,
                "inset {inset}, point {p:?}"
            );
            assert!(
                (p[2].abs() - (1. - inset)).abs() < 1e-5,
                "inset {inset}, point {p:?}"
            );
        }
    }
}

fn offset_edit(bevel: bool, faces: BTreeSet<usize>, offset: f32, mode: FaceTreatment) -> MeshEdit {
    if bevel {
        MeshEdit::FaceBevelOffset {
            faces,
            offset,
            shift: 0.,
            segments: 4,
            profile: 0.5,
            mode,
        }
    } else {
        MeshEdit::InsetFacesOffset {
            faces,
            offset,
            depth: 0.,
            mode,
        }
    }
}

#[test]
fn absolute_offsets_are_independent_of_boundary_and_interior_subdivision() {
    for count in [2, 4, 8, 32] {
        let mut source = editor(cube());
        source
            .edit_mesh(
                1,
                MeshEdit::SubdivideSelectedCount {
                    region: MeshRegion {
                        faces: BTreeSet::from([0]),
                        ..Default::default()
                    },
                    segments: count,
                },
            )
            .unwrap();
        let before = source.project.clone();
        let mesh = source.editable_mesh(1).unwrap();
        let selected: BTreeSet<_> = (0..mesh.face_count())
            .filter(|&f| {
                mesh.face_vertices(f)
                    .unwrap()
                    .iter()
                    .all(|&v| (mesh.vertices[v as usize].position[0] - 1.).abs() < 1e-6)
            })
            .collect();
        for bevel in [false, true] {
            for mode in [FaceTreatment::Region, FaceTreatment::Normals] {
                for amount in [0.4, 0.7, 0.9] {
                    let mut e = Editor::new(before.clone());
                    e.edit_mesh(1, offset_edit(bevel, selected.clone(), amount, mode))
                        .unwrap_or_else(|e| {
                            panic!("{count}, bevel {bevel}, {mode:?}, {amount}: {e}")
                        });
                    let result = e.editable_mesh(1).unwrap();
                    closed(result);
                    let mut max = 0_f32;
                    for &f in &selected {
                        for &v in result.face_vertices(f).unwrap() {
                            let p = result.vertices[v as usize].position;
                            max = max.max(p[1].abs()).max(p[2].abs());
                            assert!(
                                p[1].abs() <= 1. - amount + 2e-5
                                    && p[2].abs() <= 1. - amount + 2e-5,
                                "{count}, bevel {bevel}, {mode:?}, {amount}: {p:?}"
                            );
                        }
                    }
                    assert!((max - (1. - amount)).abs() < 2e-5);
                    assert!(e.undo());
                    assert_eq!(e.project, before);
                }
            }
        }
    }
}

#[test]
fn offset_units_modes_validation_preview_and_undo_remain_consistent() {
    let mut source = cube();
    for v in &mut source.vertices {
        for p in &mut v.position {
            *p *= 4.;
        }
    }
    for bevel in [false, true] {
        for mode in [
            FaceTreatment::Region,
            FaceTreatment::Normals,
            FaceTreatment::Individual,
        ] {
            let mut e = editor(source.clone());
            let before = e.project.clone();
            e.begin_edit("Offset");
            for amount in [1.2, 2.4, 3.8] {
                e.preview_mesh(
                    1,
                    &source,
                    offset_edit(bevel, BTreeSet::from([0]), amount, mode),
                )
                .unwrap();
                let mesh = e.editable_mesh(1).unwrap();
                closed(mesh);
                for &v in mesh.face_vertices(0).unwrap() {
                    let p = mesh.vertices[v as usize].position;
                    assert!(
                        (p[1].abs() - (4. - amount)).abs() < 1e-5
                            && (p[2].abs() - (4. - amount)).abs() < 1e-5
                    );
                }
            }
            let preview = e.project.clone();
            for amount in [-0.1, f32::NAN, f32::INFINITY, 1001.] {
                assert!(e
                    .preview_mesh(
                        1,
                        &source,
                        offset_edit(bevel, BTreeSet::from([0]), amount, mode)
                    )
                    .is_err());
                assert_eq!(e.project, preview);
            }
            e.commit_edit().unwrap();
            assert_eq!(e.undo_len(), 1);
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, preview);
        }
    }
}

#[test]
fn absolute_offsets_keep_complex_caps_closed_in_every_mode() {
    for kind in [
        PrimitiveKind::Cube,
        PrimitiveKind::Sphere,
        PrimitiveKind::Torus,
        PrimitiveKind::Cone,
        PrimitiveKind::Fox,
    ] {
        let source = PrimitiveSettings::new(kind).mesh(2, None).unwrap();
        for faces in [BTreeSet::from([0]), BTreeSet::from([0, 1])] {
            let offset = faces
                .iter()
                .map(|&f| {
                    let ring = source.face_vertices(f).unwrap();
                    let p = |i: usize| Vec3::from_array(source.vertices[ring[i] as usize].position);
                    let area = (1..ring.len() - 1)
                        .map(|i| (p(i) - p(0)).cross(p(i + 1) - p(0)))
                        .sum::<Vec3>()
                        .length();
                    let perimeter = (0..ring.len())
                        .map(|i| p(i).distance(p((i + 1) % ring.len())))
                        .sum::<f32>();
                    area / perimeter * 0.15
                })
                .reduce(f32::min)
                .unwrap();
            for mode in [
                FaceTreatment::Region,
                FaceTreatment::Normals,
                FaceTreatment::Individual,
            ] {
                for bevel in [false, true] {
                    let mut e = editor(source.clone());
                    e.edit_mesh(1, offset_edit(bevel, faces.clone(), offset, mode))
                        .unwrap_or_else(|error| {
                            panic!("{kind:?}, {faces:?}, {mode:?}, bevel {bevel}: {error}")
                        });
                    closed(e.editable_mesh(1).unwrap());
                }
            }
        }
    }
}

#[test]
fn inset_rejects_a_collapsed_or_reversed_cap_without_changing_the_preview() {
    for mode in [
        FaceTreatment::Region,
        FaceTreatment::Normals,
        FaceTreatment::Individual,
    ] {
        let source = cube();
        let mut e = editor(source.clone());
        e.begin_edit("Inset");
        e.preview_mesh(
            1,
            &source,
            offset_edit(false, BTreeSet::from([0]), 0.9, mode),
        )
        .unwrap();
        let preview = e.project.clone();
        for offset in [1., 1.1, 2.] {
            assert!(e
                .preview_mesh(
                    1,
                    &source,
                    offset_edit(false, BTreeSet::from([0]), offset, mode)
                )
                .is_err());
            assert_eq!(e.project, preview);
        }
        e.cancel_edit();
        assert_eq!(e.editable_mesh(1).unwrap(), &source);
        assert_eq!(e.undo_len(), 0);
    }
}
