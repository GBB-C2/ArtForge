use poseforge_core::*;
fn editor(kind: PrimitiveKind) -> Editor {
    let mut node = Node::new(1, "Sections");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(kind).mesh(2, None).unwrap()],
        ..Default::default()
    })
}
fn cut(normal: Vec3, offset: f32, width: f32) -> PlaneCut {
    PlaneCut {
        normal: normal.normalize().to_array(),
        offset,
        width,
        close_gap: true,
        seed_face: 0,
    }
}
fn valid_closed(m: &Mesh) {
    m.validate_topology().unwrap();
    let r = inspect_topology(m);
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
fn complex_sections_and_crossing_closed_gaps_preserve_manifold_surfaces() {
    for kind in [
        PrimitiveKind::Cube,
        PrimitiveKind::Sphere,
        PrimitiveKind::Cylinder,
        PrimitiveKind::Torus,
        PrimitiveKind::Capsule,
        PrimitiveKind::Fox,
    ] {
        for width in [0., 0.05] {
            let mut e = editor(kind);
            let before = e.project.clone();
            let cuts = vec![
                cut(Vec3::X, 0.037, width),
                cut(Vec3::new(0.2, 1., 0.1), -0.083, width),
            ];
            e.edit_mesh(1, MeshEdit::Sections { cuts })
                .unwrap_or_else(|err| panic!("{kind:?} width {width}: {err}"));
            valid_closed(&e.project.meshes[0]);
            let after = e.project.clone();
            assert_ne!(before, after);
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, after);
        }
    }
}
#[test]
fn section_retains_quads_and_ngons_instead_of_exposing_render_triangles() {
    let mut e = editor(PrimitiveKind::Cube);
    e.edit_mesh(
        1,
        MeshEdit::Sections {
            cuts: vec![cut(Vec3::X, 0.17, 0.)],
        },
    )
    .unwrap();
    let m = &e.project.meshes[0];
    assert_eq!(m.face_count(), 10);
    assert!((0..m.face_count()).all(|f| m.face_vertices(f).unwrap().len() == 4));
    valid_closed(m);
    let mut e = editor(PrimitiveKind::Cylinder);
    e.edit_mesh(
        1,
        MeshEdit::Sections {
            cuts: vec![cut(Vec3::X, 0.08, 0.)],
        },
    )
    .unwrap();
    let m = &e.project.meshes[0];
    assert!(
        (0..m.face_count())
            .filter(|&f| m.face_vertices(f).unwrap().len() > 4)
            .count()
            >= 4
    );
    valid_closed(m);
}
#[test]
fn an_open_surface_can_be_cut_and_independent_parameters_rebuild_from_source() {
    let mut e = editor(PrimitiveKind::Plane);
    let original = e.project.meshes[0].clone();
    e.begin_edit("Разрез");
    let mut cuts = vec![cut(Vec3::X, -0.2, 0.), cut(Vec3::X, 0.2, 0.)];
    e.preview_mesh(1, &original, MeshEdit::Sections { cuts: cuts.clone() })
        .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 3);
    cuts[0].width = 0.05;
    cuts[0].offset = -0.3;
    e.preview_mesh(1, &original, MeshEdit::Sections { cuts })
        .unwrap();
    assert_eq!(e.project.meshes[0].face_count(), 3);
    e.cancel_edit();
    assert_eq!(e.project.meshes[0], original);
}

#[test]
fn a_cut_on_an_existing_boundary_does_not_duplicate_a_face() {
    let mut e = editor(PrimitiveKind::Cube);
    e.edit_mesh(
        1,
        MeshEdit::Sections {
            cuts: vec![cut(Vec3::X, 1., 0.), cut(Vec3::Z, 0., 0.)],
        },
    )
    .unwrap();
    valid_closed(&e.project.meshes[0]);
    assert_eq!(e.project.meshes[0].face_count(), 10);
}
#[test]
fn concave_polygon_islands_keep_the_boundary_instead_of_triangle_fans() {
    let mut e = editor(PrimitiveKind::Plane);
    let m = &mut e.project.meshes[0];
    m.vertices = [
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
    m.topology.as_mut().unwrap().faces = vec![(0..8).collect()].into();
    m.topology.as_mut().unwrap().primitive = None;
    m.rebuild_polygon_indices().unwrap();
    e.edit_mesh(
        1,
        MeshEdit::Sections {
            cuts: vec![cut(Vec3::Y, 2., 0.)],
        },
    )
    .unwrap();
    let m = &e.project.meshes[0];
    assert_eq!(m.face_count(), 3);
    assert!((0..m.face_count()).all(|f| m.face_vertices(f).unwrap().len() >= 4));
    let r = inspect_topology(m);
    assert_eq!(
        (r.nonmanifold_edges, r.degenerate_faces, r.duplicate_faces),
        (0, 0, 0)
    );
}

#[test]
fn section_preserves_detached_vertices_and_their_attributes() {
    let mut e = editor(PrimitiveKind::Cube);
    let vertex = Vertex {
        position: [8., 9., 10.],
        normal: [0., 0., -1.],
        uv: [0.3, 0.8],
        color: [0.2, 0.4, 0.6, 1.],
        ..Default::default()
    };
    e.project.meshes[0].vertices.push(vertex);
    e.edit_mesh(
        1,
        MeshEdit::Sections {
            cuts: vec![cut(Vec3::X, 0.1, 0.05)],
        },
    )
    .unwrap();
    assert!(e.project.meshes[0].vertices.contains(&vertex));
}
