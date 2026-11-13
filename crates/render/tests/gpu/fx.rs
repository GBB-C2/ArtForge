use super::*;
use poseforge_core::{Editor,FxKind,Node,Vec3,box_mesh};

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn gas_density_uploads_rewind_exactly_and_profile_density_controls_alpha() {
    let mut e=Editor::new(Project::default());e.project.lights.clear();let id=e.create_fx(FxKind::Smoke).unwrap();
    let n=e.project.node_mut(id).unwrap();n.transform.scale=Vec3::ONE;let f=n.fx.as_mut().unwrap();f.gas=true;f.speed=1.;f.density=20.;f.quality=poseforge_core::FxQuality::Fast;
    let view=View{eye:Vec3::new(0.,0.6,3.),target:Vec3::new(0.,0.6,0.),viewport:[0.,0.,160.,160.],..Default::default()};
    let mut r=pollster::block_on(Renderer::headless(160,160)).unwrap();r.set_effect_time(0.);
    let empty=material_picture(&mut r,&e.project,&view,"4728-gas-start");assert!(empty.pixels().all(|p|p[3]==0));
    r.set_effect_time(0.5);let first=material_picture(&mut r,&e.project,&view,"4728-gas-half");assert!(first.pixels().any(|p|p[3]>20));
    r.set_effect_time(0.8);let next=material_picture(&mut r,&e.project,&view,"4728-gas-later");assert!(first.pixels().zip(next.pixels()).any(|(a,b)|a!=b));
    r.set_effect_time(0.5);let rewind=material_picture(&mut r,&e.project,&view,"4728-gas-rewind");assert_images_equal(&first,&rewind,"rewinding must reuse the same density frame");
    r.clear_fx_cache();let rebuilt=material_picture(&mut r,&e.project,&view,"4728-gas-rebuilt");assert_images_equal(&first,&rebuilt,"cache eviction must not change the simulation");
    let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.envelope.enabled=true;for k in &mut f.envelope.keys {k.density=0.;}
    let hidden=material_picture(&mut r,&e.project,&view,"4728-gas-density-zero");assert!(hidden.pixels().all(|p|p[3]==0));
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn fx_volume_animates_exports_alpha_and_is_occluded_by_opaque_geometry() {
    let mut editor=Editor::new(Project::default());let id=editor.create_fx(FxKind::Fire).unwrap();
    let p=&mut editor.project;p.lights.clear();p.render_settings.shadows=false;
    // Isolate depth compositing from the optional light, which can illuminate the wall.
    p.node_mut(id).unwrap().fx.as_mut().unwrap().illuminate=false;
    let view=View {eye:Vec3::new(0.,1.,4.),target:Vec3::new(0.,1.,0.),viewport:[0.,0.,160.,160.],..Default::default()};
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    renderer.set_effect_time(0.);let first=material_picture(&mut renderer,p,&view,"4722-fire-0");
    assert!(first.pixels().filter(|p|p[3]>0).count()>100);assert_eq!(first.get_pixel(0,0)[3],0);
    renderer.set_effect_time(1.);let next=material_picture(&mut renderer,p,&view,"4722-fire-1");
    assert!(first.pixels().zip(next.pixels()).filter(|(a,b)|a!=b).count()>100);
    p.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=false;
    let off=material_picture(&mut renderer,p,&view,"4722-fire-off");assert!(off.pixels().all(|p|p[3]==0));
    p.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=true;
    let mesh=p.next_id();p.meshes.push(box_mesh(mesh,"Occluder",Vec3::new(2.,2.,0.1),None));
    let mut blocker=Node::new(p.next_id(),"Wall");blocker.mesh=Some(mesh);blocker.transform.translation=Vec3::new(0.,1.,1.);p.nodes.push(blocker);
    let blocked=material_picture(&mut renderer,p,&view,"4722-fire-blocked");
    p.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=false;
    let wall=material_picture(&mut renderer,p,&view,"4722-wall");
    assert_images_equal(&blocked,&wall,"opaque wall must completely hide the volume behind it");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn fire_light_illuminates_a_receiver_outside_the_volume_and_respects_visibility() {
    let (project,view)=material_scene();let mut editor=Editor::new(project);let id=editor.create_fx(FxKind::Fire).unwrap();
    let p=&mut editor.project;let node=p.node_mut(id).unwrap();node.transform.translation=Vec3::new(1.5,0.,1.);
    node.transform.scale=Vec3::ONE;let fx=node.fx.as_mut().unwrap();fx.illuminate=false;fx.flicker=0.;fx.emission=15.;
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let unlit=material_picture(&mut renderer,p,&view,"4723-fire-light-off");
    p.node_mut(id).unwrap().fx.as_mut().unwrap().illuminate=true;
    let lit=material_picture(&mut renderer,p,&view,"4723-fire-light-on");
    assert!(lit.get_pixel(80,80)[0] as i32>unlit.get_pixel(80,80)[0] as i32+20,"receiver must be lit without looking through the flame");
    p.node_mut(id).unwrap().render_visible=false;
    let hidden=material_picture(&mut renderer,p,&view,"4723-fire-hidden");
    assert_eq!(hidden.get_pixel(80,80),unlit.get_pixel(80,80));
    p.node_mut(id).unwrap().render_visible=true;p.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=false;
    let disabled=material_picture(&mut renderer,p,&view,"4723-fire-disabled");
    assert_images_equal(&hidden,&disabled,"hidden and disabled effects must contribute neither volume nor light");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn distant_volume_does_not_make_a_thin_foreground_volume_disappear() {
    let mut editor=Editor::new(Project::default());editor.project.lights.clear();let near=editor.create_fx(FxKind::Fog).unwrap();
    let node=editor.project.node_mut(near).unwrap();node.transform.scale=Vec3::new(1.,1.,0.1);
    let fx=node.fx.as_mut().unwrap();fx.density=12.;fx.turbulence=0.;fx.speed=0.;
    let view=View {eye:Vec3::new(0.,0.4,3.),target:Vec3::new(0.,0.4,0.),viewport:[0.,0.,160.,160.],..Default::default()};
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let alone=material_picture(&mut renderer,&editor.project,&view,"4723-thin-fog");
    let far=editor.create_fx(FxKind::Fog).unwrap();let node=editor.project.node_mut(far).unwrap();
    node.transform.translation=Vec3::new(0.,0.,-100.);node.transform.scale=Vec3::ONE;
    node.fx.as_mut().unwrap().density=0.000001;
    let separated=material_picture(&mut renderer,&editor.project,&view,"4723-separated-fog");
    let a=alone.get_pixel(80,80)[3] as i32;let b=separated.get_pixel(80,80)[3] as i32;
    assert!(a>25,"foreground setup must contain visible fog");assert!((a-b).abs()<12,"empty gap must not consume foreground sampling: {a} vs {b}");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn curve_emission_follows_geometry_and_wind_changes_the_rendered_flow() {
    use poseforge_core::{CurveData,CurveInterpolation,ForceFieldSettings,ForceFieldKind,Quat};
    let mut e=Editor::new(Project::default());e.project.lights.clear();let id=e.create_fx(FxKind::Smoke).unwrap();
    let mut curve=Node::new(20,"Line emitter");curve.curve=Some(CurveData{points:vec![[-0.8,0.,0.],[0.8,0.,0.]],interpolation:CurveInterpolation::Polyline,..Default::default()});
    e.project.nodes.push(curve);e.set_fx_source(id,Some(20)).unwrap();
    let n=e.project.node_mut(id).unwrap();n.transform.scale=Vec3::ONE;let f=n.fx.as_mut().unwrap();f.density=20.;f.source_radius=0.25;f.turbulence=0.;f.speed=0.;
    let mut field=Node::new(30,"Wind");field.empty=true;field.transform.rotation=Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    field.force_field=Some(ForceFieldSettings{kind:ForceFieldKind::Wind,strength:1.5,radius:20.,falloff:0.,fx:false,..Default::default()});e.project.nodes.push(field);
    let view=View{eye:Vec3::new(0.,0.8,5.),target:Vec3::new(0.,0.8,0.),viewport:[0.,0.,160.,160.],..Default::default()};
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let still=material_picture(&mut renderer,&e.project,&view,"4724-curve-still");
    let center=|image:&image::RgbaImage| {let mut sum=0.;let mut mass=0.;for (x,_,p) in image.enumerate_pixels() {sum+=x as f64*p[3] as f64;mass+=p[3] as f64;}assert!(mass>1000.);sum/mass};
    let pixels:Vec<_>=still.enumerate_pixels().filter(|(_,_,p)|p[3]>10).map(|(x,_,_)|x).collect();
    assert!(pixels.iter().max().unwrap()-pixels.iter().min().unwrap()>25,"line must emit across its length");
    e.project.node_mut(30).unwrap().force_field.as_mut().unwrap().fx=true;
    let blown=material_picture(&mut renderer,&e.project,&view,"4724-curve-wind");assert!(center(&blown)>center(&still)+2.,"wind must deflect the visible flow");
    e.project.node_mut(id).unwrap().fx.as_mut().unwrap().use_fields=false;
    let off=material_picture(&mut renderer,&e.project,&view,"4724-curve-field-off");assert_images_equal(&still,&off,"per-effect toggle must restore the flow");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn four_existing_kinds_support_cloud_burst_end_and_exact_repeat() {
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();
    let view=View {eye:Vec3::new(0.,0.5,3.),target:Vec3::new(0.,0.5,0.),viewport:[0.,0.,160.,160.],..Default::default()};
    for kind in FxKind::ALL {
        let mut e=Editor::new(Project::default());e.project.lights.clear();let id=e.create_fx(kind).unwrap();
        let n=e.project.node_mut(id).unwrap();n.transform.scale=Vec3::ONE;let f=n.fx.as_mut().unwrap();
        f.cloud=true;f.burst=false;f.speed=1.;f.density=15.;f.illuminate=false;f.start=1.;f.duration=2.;
        renderer.set_effect_time(1.25);let cloud=material_picture(&mut renderer,&e.project,&view,&format!("4727-cloud-{}",kind.index()));
        assert!(cloud.pixels().filter(|p|p[3]>5).count()>25);
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().burst=true;
        let early=material_picture(&mut renderer,&e.project,&view,&format!("4727-burst-{}",kind.index()));assert!(early.pixels().any(|p|p[3]>5));
        renderer.set_effect_time(2.);let later=material_picture(&mut renderer,&e.project,&view,&format!("4727-burst-late-{}",kind.index()));assert_ne!(early,later);
        renderer.set_effect_time(3.);let ended=material_picture(&mut renderer,&e.project,&view,&format!("4727-burst-end-{}",kind.index()));assert!(ended.pixels().all(|p|p[3]==0));
        e.project.node_mut(id).unwrap().fx.as_mut().unwrap().looped=true;
        renderer.set_effect_time(3.25);let again=material_picture(&mut renderer,&e.project,&view,&format!("4727-burst-loop-{}",kind.index()));
        assert_images_equal(&early,&again,"repeat must sample the same burst phase");
    }
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn transparent_selected_collider_clips_density_without_using_camera_depth() {
    use poseforge_core::{Material,AlphaMode,CollisionTarget};
    let mut e=Editor::new(Project::default());e.project.lights.clear();let id=e.create_fx(FxKind::Fog).unwrap();
    let n=e.project.node_mut(id).unwrap();n.transform.scale=Vec3::ONE;let f=n.fx.as_mut().unwrap();f.speed=0.;f.turbulence=0.;f.density=20.;
    e.project.materials.push(Material{id:23,base_color:[1.,1.,1.,0.],alpha_mode:AlphaMode::Blend,..Default::default()});
    e.project.meshes.push(box_mesh(21,"Transparent obstacle",Vec3::new(0.075,1.,1.),Some(23)));
    let mut obstacle=Node::new(20,"Transparent obstacle");obstacle.mesh=Some(21);obstacle.transform.translation=Vec3::new(0.24,0.5,0.);e.project.nodes.push(obstacle);
    e.project.node_mut(id).unwrap().collision_targets.fx=Some(vec![CollisionTarget{node:20,enabled:true}]);
    let view=View{eye:Vec3::new(0.,0.5,3.),target:Vec3::new(0.,0.5,0.),viewport:[0.,0.,160.,160.],orthographic:Some(0.8),..Default::default()};
    let mut renderer=pollster::block_on(Renderer::headless(160,160)).unwrap();let point=view.project(Vec3::new(0.24,0.5,0.)).unwrap();let (x,y)=(point[0] as u32,point[1] as u32);
    let through=material_picture(&mut renderer,&e.project,&view,"4727-unselected-transparent-collider");assert!(through.get_pixel(x,y)[3]>25);
    e.project.node_mut(id).unwrap().fx.as_mut().unwrap().mesh_collision=true;
    let clipped=material_picture(&mut renderer,&e.project,&view,"4727-selected-transparent-collider");
    assert_eq!(clipped.get_pixel(x,y)[3],0,"transparent obstacle must exclude FX density independently of opaque depth");
    assert!(clipped.pixels().any(|p|p[3]>25),"collision must not disable the whole plume");
    e.project.node_mut(id).unwrap().collision_targets.fx.as_mut().unwrap()[0].enabled=false;
    let restored=material_picture(&mut renderer,&e.project,&view,"4727-collider-eye-off");assert_images_equal(&through,&restored,"eye toggle restores original effect");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn pyro_fog_is_visible_at_start_self_shadows_and_replays_exactly() {
    use poseforge_core::{Light,LightKind,FxQuality};
    let mut e=Editor::new(Project::default());e.project.lights=vec![Light{kind:LightKind::Directional,direction:Vec3::new(-0.4,-1.,0.2).normalize(),intensity:4.,casts_shadow:false,..Default::default()}];
    let id=e.create_pyro(FxKind::Fog).unwrap();let n=e.project.node_mut(id).unwrap();n.transform.scale=Vec3::ONE;let f=n.fx.as_mut().unwrap();f.quality=FxQuality::Fast;f.density=12.;
    let view=View{eye:Vec3::new(0.,0.4,3.),target:Vec3::new(0.,0.4,0.),viewport:[0.,0.,160.,160.],..Default::default()};
    let mut r=pollster::block_on(Renderer::headless(160,160)).unwrap();r.set_effect_time(0.);
    let shaded=material_picture(&mut r,&e.project,&view,"480-fog-self-shadow");assert!(shaded.pixels().any(|p|p[3]>20));
    e.project.node_mut(id).unwrap().fx.as_mut().unwrap().pyro.as_mut().unwrap().self_shadow=false;
    let unshaded=material_picture(&mut r,&e.project,&view,"480-fog-no-self-shadow");assert_ne!(shaded,unshaded);
    e.project.node_mut(id).unwrap().fx.as_mut().unwrap().pyro.as_mut().unwrap().self_shadow=true;r.clear_fx_cache();
    let again=material_picture(&mut r,&e.project,&view,"480-fog-repeat");assert_images_equal(&shaded,&again,"fog initialization must survive cache clearing exactly");
}

#[test]
#[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
fn pyro_cold_fuel_is_invisible_but_ignited_soot_free_flame_is_visible() {
    use poseforge_core::{FxQuality,FxEmission};
    let mut e=Editor::new(Project::default());e.project.lights.clear();let id=e.create_pyro(FxKind::Fire).unwrap();
    let n=e.project.node_mut(id).unwrap();n.transform.scale=Vec3::ONE;let f=n.fx.as_mut().unwrap();f.quality=FxQuality::Fast;f.illuminate=false;f.cloud=true;f.emission=30.;
    let p=f.pyro.as_mut().unwrap();p.mode=FxEmission::Fill;p.source_rate=3.;p.source_temperature=293.;p.smoke_yield=0.;p.expansion=0.;p.vorticity=0.;
    let view=View{eye:Vec3::new(0.,0.25,2.),target:Vec3::new(0.,0.25,0.),viewport:[0.,0.,160.,160.],..Default::default()};
    let mut r=pollster::block_on(Renderer::headless(160,160)).unwrap();r.set_effect_time(0.2);
    let cold=material_picture(&mut r,&e.project,&view,"480-cold-fuel");assert!(cold.pixels().all(|p|p[3]==0));
    e.project.node_mut(id).unwrap().fx.as_mut().unwrap().pyro.as_mut().unwrap().source_temperature=3000.;
    let hot=material_picture(&mut r,&e.project,&view,"480-soot-free-fire");assert!(hot.pixels().any(|p|p[0]>20 && p[3]>0));
    r.clear_fx_cache();let again=material_picture(&mut r,&e.project,&view,"480-fire-replay");assert_images_equal(&hot,&again,"reaction must replay from fuel and temperature");
}
