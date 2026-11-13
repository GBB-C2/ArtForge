use super::*;
use poseforge_core::{box_mesh, Material, Mesh, Node, Vertex, Vec3};

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn textured_mesh_emission_lights_surfaces_casts_shadows_and_scatter_is_opt_in() {
    let (mut p,view)=material_scene();
    p.render_settings.shadows=true;
    p.render_settings.ray_shadows=false; // emission still needs its own occlusion
    p.render_settings.ray_reflections=false;
    p.render_settings.shadow_strength=1.;
    p.materials.push(Material{id:30,emissive:[25.;3],emissive_texture:Some(10),..Material::default()});
    p.meshes.push(Mesh{id:31,name:"Emitting patch".into(),
        vertices:[[0.65,0.65,1.],[0.65,1.35,1.],[1.35,0.65,1.]].into_iter()
            .map(|position|Vertex{position,normal:[0.,0.,-1.],..Vertex::default()}).collect(),
        indices:vec![0,1,2].into(),material:Some(30),topology:None,morph_targets:vec![]});
    let mut emitter=Node::new(32,"Emitter");emitter.mesh=Some(31);p.nodes.push(emitter);
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let red=material_picture(&mut renderer,&p,&view,"4716-red-emission");
    let c=red.get_pixel(80,80).0;
    assert!(c[0]>c[2]+10,"red emission must reach the separate receiver: {c:?}");
    p.materials[1].emissive_texture=Some(11);
    let blue=material_picture(&mut renderer,&p,&view,"4716-blue-emission");
    let c=blue.get_pixel(80,80).0;
    assert!(c[2]>c[0]+10,"changing the emissive map must change the light: {c:?}");
    p.materials[1].visible=false;
    let off=material_picture(&mut renderer,&p,&view,"4716-hidden-emission");
    assert!(off.get_pixel(80,80)[2]<blue.get_pixel(80,80)[2]);
    p.materials[1].visible=true;
    p.meshes.push(box_mesh(40,"Occluder",Vec3::new(0.3,0.3,0.08),None));
    let mut blocker=Node::new(41,"Shadow only");blocker.mesh=Some(40);
    blocker.transform.translation=Vec3::new(0.5,0.5,0.5);blocker.shadow_only=true;p.nodes.push(blocker);
    let blocked=material_picture(&mut renderer,&p,&view,"4716-emission-shadow");
    assert!(blocked.get_pixel(80,80)[2]<blue.get_pixel(80,80)[2],"a shadow-only mesh must occlude emission");
    p.nodes.pop();p.meshes.pop();
    p.render_settings.atmosphere.density=0.06;
    let no_volume=material_picture(&mut renderer,&p,&view,"4716-no-emissive-volume");
    assert_images_equal(&blue,&no_volume,"fog is inactive until an emitter opts in");
    p.materials[1].emission_volume=true;
    let volume=material_picture(&mut renderer,&p,&view,"4716-emissive-volume");
    assert!(volume.pixels().zip(no_volume.pixels()).filter(|(a,b)|a!=b).count()>100);
}
