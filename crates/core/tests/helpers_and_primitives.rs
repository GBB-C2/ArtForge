use poseforge_core::*;
use std::collections::BTreeMap;
#[test]
fn helper_creation_preserves_geometry_and_is_atomic_and_undoable() {
    let mut e=Editor::new(Project::default());let before=e.project.clone();
    for shape in EmptyShape::ALL {let id=e.create_empty_shape(shape).unwrap();assert_eq!(e.project.node(id).unwrap().empty_shape,shape);assert!(e.project.node(id).unwrap().mesh.is_none());assert!(e.undo());assert_eq!(e.project,before);}
    for kind in ForceFieldKind::ALL {let id=e.create_force_field(kind).unwrap();let f=e.project.node(id).unwrap().force_field.unwrap();assert_eq!(f.kind,kind);assert!(f.enabled);assert!(e.project.meshes.is_empty());assert!(e.undo());assert_eq!(e.project,before);}
    e.create_force_field(ForceFieldKind::Force).unwrap();let before=e.project.clone();let history=e.undo_len();
    assert!(e.transact("Invalid field",|p|p.nodes[0].force_field.as_mut().unwrap().strength=f32::NAN).is_err());assert_eq!(e.project,before);assert_eq!(e.undo_len(),history);
}
#[test]
fn new_primitives_are_closed_outward_and_keep_polygon_topology_and_finite_uvs() {
    for kind in [PrimitiveKind::Cylinder,PrimitiveKind::Cone,PrimitiveKind::Torus,PrimitiveKind::Capsule,PrimitiveKind::Pyramid] {
        for smooth in [false,true] {
            let s=PrimitiveSettings{smooth,..PrimitiveSettings::new(kind)};let mesh=s.mesh(2,None).unwrap();let mut node=Node::new(1,"Shape");node.mesh=Some(2);
            Project{nodes:vec![node],meshes:vec![mesh.clone()],..Default::default()}.validate().unwrap();
            let mut edges=BTreeMap::new();let mut volume=0.;
            for face in &mesh.topology.as_ref().unwrap().faces {for i in 0..face.len() {let a=position_key(mesh.vertices[face[i] as usize].position);let b=position_key(mesh.vertices[face[(i+1)%face.len()] as usize].position);assert_ne!(a,b);let entry=edges.entry((a.min(b),a.max(b))).or_insert((0,0));entry.0+=1;entry.1+=if a<b {1}else{-1};}}
            assert!(edges.values().all(|v|*v==(2,0)),"{kind:?}");
            for t in mesh.indices.chunks_exact(3) {let a=Vec3::from_array(mesh.vertices[t[0] as usize].position);let b=Vec3::from_array(mesh.vertices[t[1] as usize].position);let c=Vec3::from_array(mesh.vertices[t[2] as usize].position);assert!((b-a).cross(c-a).length_squared()>1e-12);volume+=a.dot(b.cross(c))/6.;}
            assert!(volume>0.,"{kind:?}");assert!(mesh.vertices.iter().all(|v|v.uv.iter().all(|x|x.is_finite())&&(Vec3::from_array(v.normal).length()-1.).abs()<0.001));
            assert_eq!(mesh.topology.as_ref().unwrap().primitive,Some(s));
        }
    }
}
#[test]
fn coupled_dimensions_and_segment_limits_reject_degenerate_shapes() {
    let mut s=PrimitiveSettings::new(PrimitiveKind::Torus);s.size[0]=s.radius;assert!(s.mesh(1,None).is_err());
    s=PrimitiveSettings::new(PrimitiveKind::Capsule);s.size[1]=s.radius;assert!(s.mesh(1,None).is_err());
    for kind in [PrimitiveKind::Cylinder,PrimitiveKind::Cone,PrimitiveKind::Torus,PrimitiveKind::Capsule] {let mut s=PrimitiveSettings::new(kind);s.segments[0]=2;assert!(s.mesh(1,None).is_err());s.segments[0]=65;assert!(s.mesh(1,None).is_err());}
}
