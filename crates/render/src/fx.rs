//! Uniform-only volume updates: no mesh upload or scene acceleration rebuild.
use bytemuck::{Pod, Zeroable};
use poseforge_core::{FxKind, Mat4, Project, Vec3, MAX_FX};
use super::{GpuLight,MAX_LIGHTS};
use std::collections::BTreeMap;
#[repr(C)]
#[derive(Clone,Copy,Pod,Zeroable)]
pub(crate) struct GpuFx {
    inverse_model:[[f32;4];4],
    params:[f32;4], // type, extinction, emission, sampled time
    color:[f32;4],
    motion:[f32;4], // detail, turbulence, seed, cloud/burst flags
    shape:[f32;4], // width, height, edge softness, fire smoke
    flow:[f32;4], // local drift X/Z, burst duration, shared flame pulse
    bounds_low:[f32;4],bounds_high:[f32;4],guide_meta:[f32;4],
    pyro:[f32;4],soot:[f32;4],
    guides:[[[f32;4];6];16],
}
#[cfg(test)]
pub(crate) fn pack(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,time:f64)->([GpuFx;MAX_FX],usize) {
    pack_internal(project,globals,export,time,None,None)
}
pub(super) fn pack_scene(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,time:f64,state:&super::fx_interactions::State,gas:&super::fx_gas::State)->([GpuFx;MAX_FX],usize) {
    pack_internal(project,globals,export,time,Some(state),Some(gas))
}
fn pack_internal(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,time:f64,state:Option<&super::fx_interactions::State>,gas:Option<&super::fx_gas::State>)->([GpuFx;MAX_FX],usize) {
    let mut packed=[GpuFx::zeroed();MAX_FX];let mut count=0;
    let visible=project.visible_node_ids(export);
    for node in &project.nodes {
        let Some(fx)=node.fx.filter(|s|s.enabled && s.density>0. && s.visible_at(time) && s.validate().is_ok()) else{continue;};
        if !visible.contains(&node.id) {continue;}
        if state.is_some_and(|s|!s.available(node.id)) {continue;}
        if fx.gas && gas.is_none_or(|g|!g.available(node.id)) {continue;}
        let envelope=fx.envelope_at(time);
        if envelope.density<=0. || (!fx.gas && envelope.size<=0.) {continue;}
        let Some(model)=globals.get(&node.id).filter(|m|m.is_finite() && m.determinant().abs()>1e-8) else{continue;};
        if count==MAX_FX {break;}
        packed[count]=GpuFx {inverse_model:model.inverse().to_cols_array_2d(),
            params:[fx.kind.index() as f32,fx.density*envelope.density,fx.emission*envelope.emission,fx.sample_time(time)],
            color:[fx.color[0]*envelope.color[0],fx.color[1]*envelope.color[1],fx.color[2]*envelope.color[2],1.],motion:[fx.detail,fx.turbulence,fx.seed as f32,(u32::from(fx.cloud)+2*u32::from(fx.burst)) as f32],
            shape:[fx.width*if fx.gas {1.}else{envelope.size.max(0.0001)},fx.height*if fx.gas {1.}else{envelope.size.max(0.0001)},fx.softness,fx.smoke],flow:[fx.drift[0],fx.drift[1],fx.duration,fx.flame_pulse(time)],
            ..GpuFx::zeroed()};
        if let Some(p)=fx.pyro {packed[count].pyro=[1.,p.anisotropy,f32::from(u8::from(p.self_shadow)),f32::from(u8::from(p.temperature_color))];
            packed[count].soot=[p.smoke_color[0]*envelope.color[0],p.smoke_color[1]*envelope.color[1],p.smoke_color[2]*envelope.color[2],f32::from(u8::from(p.cinematic_blast && fx.burst))];}
        if let Some(guides)=state.and_then(|s|s.guides(node.id)) {
            packed[count].bounds_low=guides.low;packed[count].bounds_high=guides.high;
            packed[count].guide_meta=guides.meta;packed[count].guides=guides.points;
        }
        count+=1;
    }
    (packed,count)
}

/// Fracture dust uses spare volume slots, with the physics preview's own clock.
/// It never creates document objects or overwrites authored effects.
pub(super) fn append_destruction_dust(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,time:f64,
    preview_times:&BTreeMap<u64,f64>,packed:&mut[GpuFx;MAX_FX],count:&mut usize) {
    let visible=project.visible_node_ids(export);
    for node in &project.nodes {
        if *count>=MAX_FX {break;}
        let Some(d)=node.destruction.filter(|d|d.dust_enabled && d.visible && d.dust_density>0. && d.validate().is_ok()) else {continue;};
        let Some(root)=d.result.filter(|root|visible.contains(root)) else {continue;};
        let t=if export {time}else{preview_times.get(&node.id).copied().unwrap_or(time)};
        if !t.is_finite() || t<=0. || t>=d.dust_lifetime {continue;}
        let Some(root_model)=globals.get(&root).filter(|m|m.is_finite()) else {continue;};
        let age=(t/d.dust_lifetime) as f32;
        let size=d.dust_size as f32*(0.6+age*1.4);
        let center=root_model.w_axis.truncate()+Vec3::Y*(age*size*0.25-size*0.4);
        let model=Mat4::from_scale_rotation_translation(Vec3::splat(size),poseforge_core::Quat::IDENTITY,center);
        let fade=((t/0.12).min(1.) as f32)*(1.-age).powi(2);
        packed[*count]=GpuFx {inverse_model:model.inverse().to_cols_array_2d(),
            params:[FxKind::Smoke.index() as f32,d.dust_density as f32*fade,0.,t as f32],
            color:[0.55,0.50,0.43,1.],motion:[4.,0.6,d.values[1] as f32,1.],
            shape:[1.,1.,0.7,1.],flow:[0.,0.,d.dust_lifetime as f32,1.],..GpuFx::zeroed()};
        *count+=1;
    }
}

/// Authored lights retain their slots. FX use spare slots without modifying the document.
#[cfg(test)]
pub(crate) fn append_lights(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,time:f64,
    packed:&mut[GpuLight;MAX_LIGHTS],count:&mut usize) {
    append_lights_internal(project,globals,export,time,packed,count,None,None)
}
pub(super) fn append_scene_lights(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,time:f64,
    packed:&mut[GpuLight;MAX_LIGHTS],count:&mut usize,state:&super::fx_interactions::State,gas:&super::fx_gas::State) {
    append_lights_internal(project,globals,export,time,packed,count,Some(state),Some(gas))
}
fn append_lights_internal(project:&Project,globals:&BTreeMap<u64,Mat4>,export:bool,time:f64,
    packed:&mut[GpuLight;MAX_LIGHTS],count:&mut usize,state:Option<&super::fx_interactions::State>,gas:Option<&super::fx_gas::State>) {
    let visible=project.visible_node_ids(export);
    for node in &project.nodes {
        if *count>=MAX_LIGHTS {break;}
        let Some(fx)=node.fx.filter(|f|f.enabled && f.visible_at(time) && f.illuminate && f.kind==FxKind::Fire
            && f.density>0. && f.emission>0. && f.validate().is_ok()) else{continue;};
        if !visible.contains(&node.id) {continue;}
        if state.is_some_and(|s|!s.available(node.id)) {continue;}
        if fx.gas && gas.is_none_or(|g|!g.available(node.id)) {continue;}
        let envelope=fx.envelope_at(time);
        if envelope.density<=0. || (!fx.gas && envelope.size<=0.) {continue;}
        let Some(model)=globals.get(&node.id).filter(|m|m.is_finite() && m.determinant().abs()>1e-8) else{continue;};
        let center=gas.filter(|_|fx.pyro.is_some()).and_then(|g|g.light_center(node.id)).or_else(||state.and_then(|s|s.guides(node.id)).map(|guides|{
            if guides.meta[3]>0. {return (Vec3::from_slice(&guides.low[..3])+Vec3::from_slice(&guides.high[..3]))*0.5;}
            let count=guides.meta[0] as usize;
            (0..count).map(|i|Vec3::from_slice(&guides.points[i][1][..3])).sum::<Vec3>()/count.max(1) as f32
        })).unwrap_or(Vec3::new(fx.drift[0]*0.25,fx.height*0.25,fx.drift[1]*0.25));
        let position=model.transform_point3(center);
        let size=model.transform_vector3(Vec3::X*fx.width).length()
            .max(model.transform_vector3(Vec3::Y*fx.height).length())
            .max(model.transform_vector3(Vec3::Z*fx.width).length());
        let active=if fx.gas {gas.map_or(0.,|g|g.light(node.id))}else{1.};
        let power=(fx.emission*envelope.emission*(1.-(-fx.density*envelope.density*0.2).exp())*fx.flame_pulse(time)*size*0.8*active).min(1e6);
        if power<=0. {continue;}
        let thermal=if fx.pyro.is_some_and(|p|p.temperature_color) {temperature_color(gas.map_or(1100.,|g|g.light_temperature(node.id)))}else{[1.;3]};
        let color:[f32;3]=std::array::from_fn(|i|if fx.pyro.is_some(){fx.color[i]*envelope.color[i]*thermal[i]}else{(fx.color[i]*envelope.color[i])*0.8+0.2});
        packed[*count]=GpuLight {position_kind:position.extend(1.).to_array(),
            direction_range:[0.,-1.,0.,(size*4.).clamp(0.1,10000.)],
            color_intensity:[color[0],color[1],color[2],power],cone:[1.,0.,0.,0.],shape:[1.,0.,0.,0.]};
        *count+=1;
    }
}

pub(super) fn temperature_color(kelvin:f32)->[f32;3] {
    // Artist-facing temperature ramp, shared with the volume shader; not a spectral renderer.
    let stops=[(650.,[1.,0.005,0.]),(1400.,[1.,0.12,0.01]),(2200.,[1.,0.42,0.09]),(3200.,[1.,0.72,0.35]),(5000.,[0.92,0.91,1.])];
    for pair in stops.windows(2) {if kelvin<=pair[1].0 {let t=((kelvin-pair[0].0)/(pair[1].0-pair[0].0)).clamp(0.,1.);return std::array::from_fn(|i|pair[0].1[i]+(pair[1].1[i]-pair[0].1[i])*t);}}
    stops[4].1
}
pub(super) fn shadow_quality(project:&Project,interactive:bool)->f32 {if interactive {return 0.;}
    project.nodes.iter().filter_map(|n|n.fx).filter(|f|f.enabled && f.pyro.is_some_and(|p|p.self_shadow))
        .map(|f|match f.quality {poseforge_core::FxQuality::Fast=>2,poseforge_core::FxQuality::Balanced=>3,poseforge_core::FxQuality::Fine=>4}).max().unwrap_or(0) as f32
}

pub(super) fn ray_steps(project:&Project,export:bool,interactive:bool,time:f64)->u32 {
    let visible=project.visible_node_ids(export);
    let steps=project.nodes.iter().filter(|n|visible.contains(&n.id)).filter_map(|n|n.fx)
        .filter(|f|f.enabled && f.density>0. && f.visible_at(time)).map(|f|f.quality.ray_steps(export)).max().unwrap_or(16);
    if !export && interactive {steps.min(16)}else{steps}
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseforge_core::{Editor,FxKind,Vec3};
    #[test]
    fn dust_uses_preview_clock_fades_honors_visibility_and_never_overwrites_fx() {
        let mut p=Project::default();let mut source=poseforge_core::Node::new(1,"Source");
        source.visible=false;source.render_visible=false;
        source.destruction=Some(poseforge_core::DestructionSettings {result:Some(2),dust_enabled:true,dust_lifetime:2.,..Default::default()});
        let mut root=poseforge_core::Node::new(2,"Shards");root.transform.translation=Vec3::new(2.,3.,4.);
        p.nodes=vec![source,root];let global=p.global_transforms().unwrap();let before=p.clone();
        let mut volumes=[GpuFx::zeroed();MAX_FX];let mut count=0;
        let clocks=BTreeMap::from([(1,0.25)]);
        append_destruction_dust(&p,&global,false,100.,&clocks,&mut volumes,&mut count);assert_eq!(count,1);
        assert_eq!(volumes[0].params[3],0.25);let density=volumes[0].params[1];assert!(density>0.);
        assert!(Mat4::from_cols_array_2d(&volumes[0].inverse_model).is_finite());
        count=0;append_destruction_dust(&p,&global,true,1.5,&clocks,&mut volumes,&mut count);assert_eq!(count,1);assert!(volumes[0].params[1]<density);
        for t in [0.,2.,5.,f64::NAN] {count=0;append_destruction_dust(&p,&global,true,t,&clocks,&mut volumes,&mut count);assert_eq!(count,0);}
        count=MAX_FX;let bytes=bytemuck::bytes_of(&volumes).to_vec();
        append_destruction_dust(&p,&global,false,0.25,&clocks,&mut volumes,&mut count);assert_eq!(bytemuck::bytes_of(&volumes),bytes);
        assert_eq!(p,before);p.node_mut(2).unwrap().render_visible=false;
        count=0;append_destruction_dust(&p,&global,true,0.25,&clocks,&mut volumes,&mut count);assert_eq!(count,0);
        p.node_mut(2).unwrap().visible=false;
        count=0;append_destruction_dust(&p,&global,false,0.25,&clocks,&mut volumes,&mut count);assert_eq!(count,0);
    }
    #[test]
    fn cinematic_uniform_flag_does_not_change_gpu_layout_or_normal_fire() {
        let mut e=Editor::new(Project::default());let id=e.create_pyro(FxKind::Fire).unwrap();
        let globals=e.project.global_transforms().unwrap();
        let mut prepared=super::super::fx_interactions::State::default();
        let mut gas=super::super::fx_gas::State::default();
        let cancel=std::sync::atomic::AtomicBool::new(false);
        prepared.update(&e.project,&globals,true).unwrap();
        gas.update(prepared.gas(),&e.project,0.,true,&cancel).unwrap();
        let (normal,count)=pack_scene(&e.project,&globals,true,0.,&prepared,&gas);
        assert_eq!(count,1);assert_eq!(normal[0].soot[3],0.);
        e.project.node_mut(id).unwrap().fx=Some(poseforge_core::FxSettings::modern(FxKind::Fire).styled(3).unwrap());
        prepared.update(&e.project,&globals,true).unwrap();
        gas.update(prepared.gas(),&e.project,0.,true,&cancel).unwrap();
        let (cinema,count)=pack_scene(&e.project,&globals,true,0.,&prepared,&gas);
        assert_eq!(count,1);assert_eq!(cinema[0].soot[3],1.);
        assert_eq!(std::mem::size_of::<GpuFx>(),std::mem::size_of_val(&normal[0]));
    }
    #[test]
    fn thermal_color_uniforms_and_interaction_shadow_budget_are_bounded() {
        let mut e=Editor::new(Project::default());e.create_pyro(FxKind::Fire).unwrap();let global=e.project.global_transforms().unwrap();
        let mut prepared=super::super::fx_interactions::State::default();prepared.update(&e.project,&global,true).unwrap();
        let mut gas=super::super::fx_gas::State::default();gas.update(prepared.gas(),&e.project,0.,true,&std::sync::atomic::AtomicBool::new(false)).unwrap();
        let (packed,count)=pack_scene(&e.project,&global,false,0.,&prepared,&gas);assert_eq!(count,1);assert_eq!(packed[0].pyro,[1.,0.15,1.,1.]);
        assert_eq!(shadow_quality(&e.project,true),0.);assert_eq!(shadow_quality(&e.project,false),3.);
        for k in (293..=5000).step_by(7) {assert!(temperature_color(k as f32).iter().all(|v|v.is_finite() && (0. ..=1.).contains(v)));}
        assert!(temperature_color(3200.)[1]>temperature_color(800.)[1]);
    }
    #[test]
    fn sampled_time_and_transforms_do_not_change_authored_volume_settings() {
        let mut editor=Editor::new(Project::default());let id=editor.create_fx(FxKind::Fire).unwrap();
        editor.project.node_mut(id).unwrap().transform.translation=Vec3::new(2.,3.,4.);
        let before=editor.project.clone();let globals=before.global_transforms().unwrap();
        let (a,count)=pack(&before,&globals,false,0.);let (b,_)=pack(&before,&globals,false,2.);
        assert_eq!(count,1);assert_ne!(a[0].params[3],b[0].params[3]);
        assert_eq!(a[0].inverse_model,globals[&id].inverse().to_cols_array_2d());assert_eq!(editor.project,before);
        editor.project.node_mut(id).unwrap().render_visible=false;
        assert_eq!(pack(&editor.project,&globals,false,0.).1,1);assert_eq!(pack(&editor.project,&globals,true,0.).1,0);
        editor.project.node_mut(id).unwrap().fx.as_mut().unwrap().enabled=false;
        assert_eq!(pack(&editor.project,&globals,false,0.).1,0);
    }
    #[test]
    fn shape_and_scene_light_share_clock_and_world_transform_without_document_changes() {
        let mut editor=Editor::new(Project::default());editor.project.lights.clear();
        let id=editor.create_fx(FxKind::Fire).unwrap();let node=editor.project.node_mut(id).unwrap();
        node.transform.translation=Vec3::new(2.,3.,4.);node.transform.rotation=poseforge_core::Quat::from_rotation_y(0.7);
        let fx=node.fx.as_mut().unwrap();fx.width=0.4;fx.height=0.6;fx.drift=[-0.2,0.3];fx.softness=0.7;fx.smoke=0.8;
        let fx=*fx;let before=editor.project.clone();let globals=before.global_transforms().unwrap();
        let (volumes,n)=pack(&before,&globals,false,1.25);assert_eq!(n,1);
        assert_eq!(volumes[0].shape,[0.4,0.6,0.7,0.8]);assert_eq!(volumes[0].flow,[-0.2,0.3,fx.duration,fx.flame_pulse(1.25)]);
        let mut lights=[GpuLight::zeroed();MAX_LIGHTS];let mut count=0;
        append_lights(&before,&globals,false,1.25,&mut lights,&mut count);assert_eq!(count,1);
        let expected=globals[&id].transform_point3(Vec3::new(-0.05,0.15,0.075));
        assert!(Vec3::from_slice(&lights[0].position_kind[..3]).abs_diff_eq(expected,1e-5));
        assert!(lights[0].color_intensity[3]>0.);assert_eq!(lights[0].shape[3],0.);
        let power=lights[0].color_intensity[3];count=0;append_lights(&before,&globals,false,2.5,&mut lights,&mut count);
        assert!((lights[0].color_intensity[3]/power-fx.flame_pulse(2.5)/fx.flame_pulse(1.25)).abs()<1e-5);
        assert_eq!(editor.project,before);
    }
    #[test]
    fn scene_lights_keep_priority_and_disabled_hidden_or_singular_fx_add_no_light() {
        let mut editor=Editor::new(Project::default());let id=editor.create_fx(FxKind::Fire).unwrap();
        let globals=editor.project.global_transforms().unwrap();
        editor.project.lights=vec![poseforge_core::Light::default();MAX_LIGHTS];
        let (mut lights,mut count)=super::super::pack_lights(&editor.project,false).unwrap();
        let before=bytemuck::bytes_of(&lights).to_vec();
        append_lights(&editor.project,&globals,false,0.,&mut lights,&mut count);
        assert_eq!(count,MAX_LIGHTS);assert_eq!(bytemuck::bytes_of(&lights),before.as_slice());
        editor.project.lights.clear();
        for mode in 0..6 {
            let mut p=editor.project.clone();let node=p.node_mut(id).unwrap();
            match mode {
                0=>node.fx.as_mut().unwrap().illuminate=false,1=>node.fx.as_mut().unwrap().enabled=false,
                2=>node.visible=false,3=>node.fx.as_mut().unwrap().emission=0.,
                4=>node.fx.as_mut().unwrap().kind=FxKind::Smoke,_=>node.transform.scale=Vec3::ZERO,
            }
            let transforms=p.global_transforms().unwrap();count=0;append_lights(&p,&transforms,false,0.,&mut lights,&mut count);assert_eq!(count,0,"mode {mode}");
        }
        editor.project.node_mut(id).unwrap().render_visible=false;
        count=0;append_lights(&editor.project,&globals,true,0.,&mut lights,&mut count);assert_eq!(count,0);
        count=0;append_lights(&editor.project,&globals,false,0.,&mut lights,&mut count);assert_eq!(count,1);
    }
    #[test]
    fn burst_uniform_and_scene_light_end_together_and_loop_restores_both() {
        let mut e=Editor::new(Project::default());let id=e.create_fx(FxKind::Fire).unwrap();
        let f=e.project.node_mut(id).unwrap().fx.as_mut().unwrap();f.cloud=true;f.burst=true;f.start=1.;f.duration=2.;f.speed=1.;
        let globals=e.project.global_transforms().unwrap();let before=e.project.clone();
        let (volumes,count)=pack(&e.project,&globals,false,1.25);assert_eq!(count,1);
        assert_eq!(volumes[0].motion[3],3.);assert_eq!(volumes[0].flow[2],2.);assert_eq!(volumes[0].params[3],0.25);
        let mut lights=[GpuLight::zeroed();MAX_LIGHTS];let mut count=0;
        append_lights(&e.project,&globals,false,1.25,&mut lights,&mut count);assert_eq!(count,1);let power=lights[0].color_intensity[3];assert!(power>0.);
        for time in [0.,3.,100.] {count=0;append_lights(&e.project,&globals,false,time,&mut lights,&mut count);assert_eq!(count,0);assert_eq!(pack(&e.project,&globals,false,time).1,0);}
        assert_eq!(e.project,before);e.project.node_mut(id).unwrap().fx.as_mut().unwrap().looped=true;
        let (again,count)=pack(&e.project,&globals,false,3.25);assert_eq!(count,1);assert_eq!(again[0].params,volumes[0].params);
        let mut count=0;append_lights(&e.project,&globals,false,3.25,&mut lights,&mut count);assert_eq!(count,1);assert_eq!(lights[0].color_intensity[3],power);
    }

}
