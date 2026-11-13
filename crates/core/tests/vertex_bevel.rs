use poseforge_core::*;
use std::collections::BTreeSet;

fn editor(kind: PrimitiveKind) -> Editor {
    let mut node = Node::new(1, "Test");
    node.mesh = Some(2);
    Editor::new(Project {
        nodes: vec![node],
        meshes: vec![PrimitiveSettings::new(kind).mesh(2, None).unwrap()],
        ..Default::default()
    })
}
fn valid(mesh: &Mesh, closed: bool) {
    mesh.validate_topology().unwrap();
    let graph = MeshConnectivity::new(mesh);
    assert!(graph.edge_faces.values().all(|f| f.len() == 2 || (!closed && f.len() == 1)));
    let report = inspect_topology(mesh);
    assert_eq!(report.nonmanifold_edges, 0);
    assert_eq!(report.winding_edges, 0);
    assert_eq!(report.degenerate_faces, 0);
    assert!(mesh.vertices.iter().all(|v| v.normal.iter().chain(&v.uv).chain(&v.color).all(|x| x.is_finite())));
}
#[test]
fn vertex_bevel_truncates_single_adjacent_and_all_corners_without_opening_cube() {
    let e = editor(PrimitiveKind::Cube);
    let graph = MeshConnectivity::new(&e.project.meshes[0]);
    let vertices: BTreeSet<_> = graph.canonical.iter().copied().collect();
    let &(a, b) = graph.edges.first().unwrap();
    let mut selections: Vec<_> = vertices.iter().map(|&v| BTreeSet::from([v])).collect();
    selections.push(BTreeSet::from([a, b]));
    selections.push(vertices);
    for selection in selections {
        for amount in [0.05, 0.15, 0.45] {
            let mut e = editor(PrimitiveKind::Cube);
            let original = e.project.clone();
            e.edit_mesh(1, MeshEdit::VertexBevel { vertices: selection.clone(), amount }).unwrap();
            let mesh = &e.project.meshes[0];
            valid(mesh, true);
            assert_eq!(mesh.face_count(), 6 + selection.len());
            assert!(mesh.topology.as_ref().unwrap().primitive.is_none());
            assert_eq!(e.undo_len(), 1);
            assert!(e.undo()); assert_eq!(e.project, original);
            assert!(e.redo()); valid(&e.project.meshes[0], true);
        }
    }
}
#[test]
fn plane_corners_make_an_octagon_and_uv_seams_materials_survive() {
    let mut e = editor(PrimitiveKind::Plane);
    e.project.materials.push(Material { id: 3, ..Default::default() });
    let mesh = &mut e.project.meshes[0];
    let g = MeshConnectivity::new(mesh);
    let vertices = g.canonical.iter().copied().collect();
    let &(a, b) = g.edges.first().unwrap();
    let topology = mesh.topology.as_mut().unwrap();
    topology.face_materials = vec![Some(3)].into();
    topology.uv_seams = vec![[a as u32, b as u32]];
    e.edit_mesh(1, MeshEdit::VertexBevel { vertices, amount: 0.2 }).unwrap();
    let mesh = &e.project.meshes[0];
    valid(mesh, false);
    assert_eq!(mesh.face_count(), 1);
    assert_eq!(mesh.face_vertices(0).unwrap().len(), 8);
    assert_eq!(&*mesh.topology.as_ref().unwrap().face_materials, &vec![Some(3)]);
    assert_eq!(uv_seams(mesh).len(), 1);
    assert_eq!(MeshConnectivity::new(mesh).edges.len(), 8);
    assert!(mesh.vertices.iter().all(|v| v.uv.iter().all(|x| (0.0..=1.0).contains(x))));
}
#[test]
fn new_caps_inherit_a_common_face_material_and_split_corner_normals() {
    let mut e = editor(PrimitiveKind::Cube);
    e.project.materials.push(Material { id: 3, ..Default::default() });
    e.project.meshes[0].topology.as_mut().unwrap().face_materials = vec![Some(3); 6].into();
    e.edit_mesh(1, MeshEdit::VertexBevel { vertices: BTreeSet::from([0]), amount: 0.15 }).unwrap();
    let mesh = &e.project.meshes[0];
    valid(mesh, true);
    assert_eq!(&*mesh.topology.as_ref().unwrap().face_materials, &vec![Some(3); 7]);
    let face = mesh.face_vertices(6).unwrap();
    assert_eq!(face.len(), 3);
    let normal = Vec3::from_array(mesh.vertices[face[0] as usize].normal);
    assert!(face.iter().all(|&i| normal.distance(Vec3::from_array(mesh.vertices[i as usize].normal)) < 1e-5));
    let loaded: Project = serde_json::from_slice(&serde_json::to_vec(&e.project).unwrap()).unwrap();
    assert_eq!(loaded, e.project);
}
#[test]
fn invalid_vertex_bevel_and_preview_cancellation_are_atomic() {
    let mut e = editor(PrimitiveKind::Cube);
    let original = e.project.clone();
    for amount in [f32::NAN, f32::INFINITY, -0.1, 0.46, 1e-10] {
        assert!(e.edit_mesh(1, MeshEdit::VertexBevel { vertices: BTreeSet::from([0]), amount }).is_err());
        assert_eq!(e.project, original); assert_eq!(e.undo_len(), 0);
    }
    for vertices in [BTreeSet::new(), BTreeSet::from([usize::MAX])] {
        assert!(e.edit_mesh(1, MeshEdit::VertexBevel { vertices, amount: 0.15 }).is_err());
        assert_eq!(e.project, original);
    }
    e.begin_edit("Vertex bevel");
    e.preview_mesh(1, &original.meshes[0], MeshEdit::VertexBevel { vertices: BTreeSet::from([0]), amount: 0.15 }).unwrap();
    let preview = e.project.clone();
    assert_ne!(preview, original); assert_eq!(e.undo_len(), 0);
    assert!(e.preview_mesh(1, &original.meshes[0], MeshEdit::VertexBevel { vertices: BTreeSet::from([0]), amount: 0.5 }).is_err());
    assert_eq!(e.project, preview);
    e.cancel_edit(); assert_eq!(e.project, original); assert_eq!(e.undo_len(), 0);
}

#[test]
fn nonmanifold_corner_is_rejected_without_partial_geometry_or_history() {
    let mut e = editor(PrimitiveKind::Cube);
    let mesh = &mut e.project.meshes[0];
    let topology = mesh.topology.as_mut().unwrap();
    let face = topology.faces[0].clone();
    let selected = face[0] as usize;
    topology.faces.push(face);
    mesh.rebuild_polygon_indices().unwrap();
    let original = e.project.clone();
    assert!(e.edit_mesh(1, MeshEdit::VertexBevel { vertices: BTreeSet::from([selected]), amount: 0.15 }).is_err());
    assert_eq!(e.project, original); assert_eq!(e.undo_len(), 0);
}
