use poseforge_core::*;
fn scene() -> Editor {
    let mesh = PrimitiveSettings::new(PrimitiveKind::Cube).mesh(2,None).unwrap();
    let mut n=Node::new(1,"Base"); n.mesh=Some(2);
    Editor::new(Project { nodes:vec![n],meshes:vec![mesh],..Default::default() })
}
#[test]
fn morph_creation_isolates_instances_and_undo_restores_shared_mesh() {
    let mut e=scene(); let mut copy=e.project.nodes[0].clone(); copy.id=3; e.project.nodes.push(copy);
    let before=e.project.clone(); assert_eq!(e.create_morph(1).unwrap(),0);
    assert_ne!(e.project.node(1).unwrap().mesh,e.project.node(3).unwrap().mesh);
    assert!(e.project.mesh(2).unwrap().morph_targets.is_empty());
    assert_eq!(e.project.node(1).unwrap().morph_weights,vec![0.]);
    e.project.validate().unwrap(); assert!(e.undo()); assert_eq!(e.project,before); assert!(e.redo()); e.project.validate().unwrap();
}
#[test]
fn shape_capture_uses_local_vertices_and_rejects_topology_changes() {
    let mut e=scene(); let copy=e.copy_morph_shape(1,None).unwrap();
    let mid=e.project.node(copy).unwrap().mesh.unwrap();
    let base=e.project.mesh(2).unwrap().vertices[0].position;
    assert_eq!(e.project.mesh(2).unwrap().vertices[0].normal,e.project.mesh(mid).unwrap().vertices[0].normal);
    e.project.meshes.iter_mut().find(|m|m.id==mid).unwrap().vertices[0].position[1]+=0.25;
    e.project.node_mut(copy).unwrap().transform.translation=Vec3::splat(100.);
    assert_eq!(e.capture_morph(1,copy,None).unwrap(),0);
    let m=e.project.mesh(e.project.node(1).unwrap().mesh.unwrap()).unwrap();
    assert_eq!(m.morph_targets[0].positions[0],[0.,0.25,0.]); assert_eq!(m.vertices[0].position,base);
    e.project.meshes.iter_mut().find(|m|m.id==mid).unwrap().indices.swap(0,1);
    let before=e.project.clone(); assert!(e.capture_morph(1,copy,None).is_err()); assert_eq!(e.project,before);
}
#[test]
fn animation_weights_stage_key_discard_and_undo_as_a_single_gesture() {
    let mut e=scene(); e.create_morph(1).unwrap(); e.mode=EditorMode::Animation;
    let before=e.undo_len(); e.begin_edit("Weights");
    e.set_morph_weight(1,0,0.2).unwrap(); e.set_morph_weight(1,0,0.7).unwrap(); e.commit_edit().unwrap();
    assert_eq!(e.undo_len(),before+1); assert!(e.has_staged());
    assert_eq!(e.project.nodes[0].morph_weights,vec![0.]); assert_eq!(e.evaluated().unwrap().morph_weights[&1],vec![0.7]);
    assert!(e.set_time(1.).is_err()); e.key_morph_weights(1).unwrap(); assert!(!e.has_staged());
    assert_eq!(e.project.clips[0].channels[0].keys[0].value,vec![0.7]);
    e.undo(); assert!(e.has_staged()); e.resolve_staged(StagedResolution::Discard).unwrap(); assert!(!e.has_staged());
    e.set_morph_weight(1,0,0.4).unwrap(); e.resolve_staged(StagedResolution::Commit).unwrap(); assert!(!e.has_staged());
    assert_eq!(e.project.clips[0].channels[0].keys[0].value,vec![0.4]);
    e.selected=Some(1);e.set_morph_weight(1,0,0.9).unwrap();e.key_selected().unwrap();assert!(!e.has_staged());assert_eq!(e.evaluated().unwrap().morph_weights[&1],vec![0.9]);
}
#[test]
fn auto_key_changes_evaluated_weights_and_rejects_invalid_numbers() {
    let mut e=scene(); e.create_morph(1).unwrap(); e.mode=EditorMode::Animation; e.auto_key=true;
    e.set_morph_weight(1,0,0.3).unwrap(); assert!(!e.has_staged()); assert_eq!(e.evaluated().unwrap().morph_weights[&1],vec![0.3]);
    for v in [f32::NAN,-0.1,1.1] { assert!(e.set_morph_weight(1,0,v).is_err()); }
    assert!(e.set_morph_weight(1,1,0.5).is_err()); e.project.validate().unwrap();
}
#[test]
fn deleting_a_target_updates_cubic_keys_and_tangents_in_all_clips() {
    let mut e=scene(); e.create_morph(1).unwrap(); e.create_morph(1).unwrap();
    let mut clip=Clip::new("Shapes",2.); clip.channels.push(Channel {node:1,property:Property::Weights,interpolation:Interpolation::CubicSpline,
        keys:vec![Keyframe {time:0.,value:vec![0.2,0.8],in_tangent:Some(vec![1.,2.]),out_tangent:Some(vec![3.,4.])}]});
    e.project.clips.push(clip.clone()); e.project.clips.push(clip);
    e.delete_morph(1,0).unwrap(); for c in &e.project.clips {let k=&c.channels[0].keys[0]; assert_eq!(k.value,vec![0.8]); assert_eq!(k.in_tangent,Some(vec![2.])); assert_eq!(k.out_tangent,Some(vec![4.]));}
    e.project.validate().unwrap(); e.delete_morph(1,0).unwrap(); assert!(e.project.clips.iter().all(|c|c.channels.is_empty())); e.project.validate().unwrap();
}
#[test]
fn form_rename_and_copy_roundtrip_through_native_document() {
    let mut e=scene(); e.create_morph(1).unwrap(); e.rename_morph(1,0,"Smile").unwrap();
    let id=e.project.node(1).unwrap().mesh.unwrap(); e.project.meshes.iter_mut().find(|m|m.id==id).unwrap().morph_targets[0].positions[0]=[0.,0.2,0.];
    let original=e.project.mesh(id).unwrap().vertices[0].position;
    let copy=e.copy_morph_shape(1,Some(0)).unwrap(); let n=e.project.node(copy).unwrap(); let m=e.project.mesh(n.mesh.unwrap()).unwrap();
    assert!(m.morph_targets.is_empty()); assert!((m.vertices[0].position[1]-original[1]-0.2).abs()<1e-6);
    let decoded:Project=serde_json::from_slice(&serde_json::to_vec(&e.project).unwrap()).unwrap(); decoded.validate().unwrap(); assert_eq!(decoded,e.project);
    e.project.node_mut(1).unwrap().locked=true; assert!(e.set_morph_weight(1,0,0.2).is_err()); assert!(e.create_morph(1).is_err());
}
