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
