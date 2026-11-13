// Procedural volumes with ordered, gap-free integration. Opaque depth clips each ray.
fn fx_hash(p:vec3<f32>)->f32 {
    let q=fract(p*vec3(0.1031,0.1030,0.0973));let r=q+vec3(dot(q,q.yxz+vec3(33.33)));
    return fract((r.x+r.y)*r.z);
}
fn fx_noise(p:vec3<f32>)->f32 {
    let cell=floor(p);let f=fract(p);let u=f*f*(vec3(3.)-2.*f);
    return mix(mix(mix(fx_hash(cell),fx_hash(cell+vec3(1.,0.,0.)),u.x),
        mix(fx_hash(cell+vec3(0.,1.,0.)),fx_hash(cell+vec3(1.,1.,0.)),u.x),u.y),
        mix(mix(fx_hash(cell+vec3(0.,0.,1.)),fx_hash(cell+vec3(1.,0.,1.)),u.x),
        mix(fx_hash(cell+vec3(0.,1.,1.)),fx_hash(cell+vec3(1.,1.,1.)),u.x),u.y),u.z);
}
// Only copy scalar/vector properties; the 96 trajectory points stay in the uniform.
struct FxProperties {
    inverse_model:mat4x4<f32>,params:vec4<f32>,color:vec4<f32>,motion:vec4<f32>,shape:vec4<f32>,flow:vec4<f32>,
    bounds_low:vec4<f32>,bounds_high:vec4<f32>,guide_meta:vec4<f32>,pyro:vec4<f32>,soot:vec4<f32>,
};
fn fx_properties(volume:u32)->FxProperties {
    return FxProperties(display.fx[volume].inverse_model,display.fx[volume].params,display.fx[volume].color,
        display.fx[volume].motion,display.fx[volume].shape,display.fx[volume].flow,
        display.fx[volume].bounds_low,display.fx[volume].bounds_high,display.fx[volume].guide_meta,display.fx[volume].pyro,display.fx[volume].soot);
}
fn fx_interval(volume:u32,origin:vec3<f32>,ray:vec3<f32>,limit:f32)->vec2<f32> {
    let fx=fx_properties(volume);
    let o=(fx.inverse_model*vec4(origin,1.)).xyz;let d=(fx.inverse_model*vec4(ray,0.)).xyz;
    // Conservative shape bounds retain narrow effects without sampling the entire empty domain.
    let radius=fx.shape.x*0.6+fx.motion.y*0.09;
    let lower=max(vec2(-0.5),min(vec2(0.),fx.flow.xy)-vec2(radius));
    let upper=min(vec2(0.5),max(vec2(0.),fx.flow.xy)+vec2(radius));
    let lo=select(vec3(lower.x,0.,lower.y),fx.bounds_low.xyz,(fx.guide_meta.x>0. || fx.guide_meta.w>0.));
    let hi=select(vec3(upper.x,fx.shape.y,upper.y),fx.bounds_high.xyz,(fx.guide_meta.x>0. || fx.guide_meta.w>0.));var near=0.;var far=limit;
    for(var axis=0u;axis<3u;axis+=1u) {
        if abs(d[axis])<0.0000001 {if o[axis]<lo[axis] || o[axis]>hi[axis] {return vec2(1.,0.);}}
        else {let a=(lo[axis]-o[axis])/d[axis];let b=(hi[axis]-o[axis])/d[axis];near=max(near,min(a,b));far=min(far,max(a,b));}
    }
    return vec2(near,far);
}
struct FxSample {color:vec3<f32>,density:f32,emission:vec3<f32>};
struct FxCoordinate {xz:vec2<f32>,age:f32,phase:f32};
fn fx_temperature_color(k:f32)->vec3<f32> {
    if k<1400. {return mix(vec3(1.,0.005,0.),vec3(1.,0.12,0.01),clamp((k-650.)/750.,0.,1.));}
    if k<2200. {return mix(vec3(1.,0.12,0.01),vec3(1.,0.42,0.09),(k-1400.)/800.);}
    if k<3200. {return mix(vec3(1.,0.42,0.09),vec3(1.,0.72,0.35),(k-2200.)/1000.);}
    return mix(vec3(1.,0.72,0.35),vec3(0.92,0.91,1.),clamp((k-3200.)/1800.,0.,1.));
}
fn fx_medium_state(volume:u32,world:vec3<f32>)->vec4<f32> {
    let fx=fx_properties(volume);if fx.guide_meta.w<0.5 {return vec4(0.);}
    let p=(fx.inverse_model*vec4(world,1.)).xyz;let uv=(p-fx.bounds_low.xyz)/max(fx.bounds_high.xyz-fx.bounds_low.xyz,vec3(0.000001));
    if any(uv<vec3(0.)) || any(uv>=vec3(1.)) {return vec4(0.);}
    if fx.guide_meta.z>0.5 {let cell=clamp(vec3<i32>(floor(uv*32.)),vec3<i32>(0),vec3<i32>(31));
        if textureLoad(fx_collision_mask,vec3<i32>(cell.xy,cell.z+(i32(fx.guide_meta.z)-1)*32),0).r<0.5 {return vec4(0.);}}
    let q=clamp(uv,vec3(0.5/32.),vec3(1.-0.5/32.));
    return textureSampleLevel(fx_gas_density,fx_gas_sampler,vec3(q.xy,(q.z+fx.guide_meta.w-1.)/8.),0.);
}
fn fx_self_transmission(volume:u32,world:vec3<f32>,direction:vec3<f32>,distance:f32)->f32 {
    let fx=fx_properties(volume);let steps=u32(display.fx_meta.z);
    if fx.pyro.x<0.5 || fx.pyro.z<0.5 || steps==0u {return 1.;}
    let span=fx_interval(volume,world,direction,distance);
    let length=span.y-span.x;if length<=0.00001 {return 1.;}
    let step=length/f32(steps);var optical=0.;
    for(var j=0u;j<steps;j+=1u) {let field=fx_medium_state(volume,world+direction*(span.x+(f32(j)+0.5)*step));
        optical+=(field.r*fx.params.y+select(0.,field.b*0.12,fx.params.x<0.5))*step;}
    return exp(-min(optical,40.));
}
fn fx_coordinate(volume:u32,p:vec3<f32>)->FxCoordinate {
    var best=1e30;var result=FxCoordinate(vec2(0.),-1.,0.);
    let guide_info=display.fx[volume].guide_meta;
    for(var source=0u;source<u32(guide_info.x);source+=1u) {
        for(var j=0u;j<5u;j+=1u) {
            let a=display.fx[volume].guides[source*6u+j];let b=display.fx[volume].guides[source*6u+j+1u];let axis=b.xyz-a.xyz;let span=dot(axis,axis);
            if span<0.0000000001 {continue;}
            let t=clamp(dot(p-a.xyz,axis)/span,0.,1.);let delta=p-mix(a.xyz,b.xyz,t);let distance=dot(delta,delta);
            if distance>=best {continue;}
            best=distance;let tangent=axis/sqrt(span);
            let reference=select(vec3(0.,1.,0.),vec3(1.,0.,0.),abs(tangent.y)>0.9);
            let right=normalize(cross(tangent,reference));let front=cross(right,tangent);
            // Retain axial residual at a stopped endpoint: a bounded capsule,
            // not an infinite cylinder extending through the obstacle.
            let side=dot(delta,right);let axial=dot(delta,tangent);
            let capped=select(-1.,1.,side>=0.)*sqrt(side*side+axial*axial);
            result=FxCoordinate(vec2(capped,dot(delta,front))/max(guide_info.y,0.0001),mix(a.w,b.w,t),f32(source));
        }
    }
    return result;
}
// Tiny procedural radial ember trails and a low shock-dust ring. These only
// run for the cinematic preset; no particles, buffers or extra solver grids.
// They are art-direction accents, NOT a compressible shockwave simulation.
fn fx_blast_accents(fx:FxProperties,p:vec3<f32>)->vec2<f32> {
    let age=fx.params.w;
    if age>=2.7 || age<0. {return vec2(0.);}
    let center=vec3((fx.bounds_low.x+fx.bounds_high.x)*0.5,
        fx.bounds_low.y+(fx.bounds_high.y-fx.bounds_low.y)*0.07,
        (fx.bounds_low.z+fx.bounds_high.z)*0.5);
    let rel=p-center;let radial=length(rel.xz);
    if radial<0.03 {return vec2(0.);}
    let angle=atan2(rel.z,rel.x);let angle_step=0.26179939;
    let sector=floor((angle+3.14159265)/angle_step);
    let variation=fract(sin(sector*39.712+fx.motion.z*0.73)*43758.55);
    let direction=(sector+0.5+(variation-0.5)*0.65)*angle_step-3.14159265;
    let alignment=abs(sin(angle-direction))*radial;
    let front=0.12+age*(1.6+variation*0.65);
    let arc=0.12+radial*(0.34+variation*0.31)-radial*radial*0.24;
    let thickness=1.-smoothstep(0.01,0.037,alignment);
    let height=1.-smoothstep(0.045,0.13,abs(rel.y-arc));
    let tail=1.-smoothstep(front*0.67,front,radial);
    let tip=1.-smoothstep(0.035,0.12,abs(radial-front));
    let fade=(1.-smoothstep(1.6,2.7,age))*smoothstep(0.,0.08,age);
    let sparks=thickness*height*(0.36*tail+tip)*fade;
    let ring_radius=0.16+age*1.28;
    let ring=(1.-smoothstep(0.04,0.17,abs(radial-ring_radius)))
        *(1.-smoothstep(0.055,0.19,abs(rel.y-0.1)))*exp(-age*2.3);
    return vec2(sparks,ring);
}
fn fx_sample(volume:u32,world:vec3<f32>)->FxSample {
    let fx=fx_properties(volume);
    let p=(fx.inverse_model*vec4(world,1.)).xyz;
    if fx.guide_meta.z>0.5 {
        let uv=(p-fx.bounds_low.xyz)/max(fx.bounds_high.xyz-fx.bounds_low.xyz,vec3(0.000001));
        if any(uv<vec3(0.)) || any(uv>=vec3(1.)) {return FxSample(vec3(0.),0.,vec3(0.));}
        let cell=clamp(vec3<i32>(floor(uv*32.)),vec3<i32>(0),vec3<i32>(31));
        let atlas=vec3<i32>(cell.xy,cell.z+(i32(fx.guide_meta.z)-1)*32);
        if textureLoad(fx_collision_mask,atlas,0).r<0.5 {return FxSample(vec3(0.),0.,vec3(0.));}
    }
    if fx.guide_meta.w>0.5 {
        let uv=(p-fx.bounds_low.xyz)/max(fx.bounds_high.xyz-fx.bounds_low.xyz,vec3(0.000001));
        if any(uv<vec3(0.)) || any(uv>=vec3(1.)) {return FxSample(vec3(0.),0.,vec3(0.));}
        let cell_uv=clamp(uv,vec3(0.5/32.),vec3(1.-0.5/32.));
        let coord=vec3(cell_uv.xy,(cell_uv.z+fx.guide_meta.w-1.)/8.);
        let state=textureSampleLevel(fx_gas_density,fx_gas_sampler,coord,0.);
        let edge=min(min(min(uv.x,uv.y),uv.z),min(min(1.-uv.x,1.-uv.y),1.-uv.z));
        var density=fx.params.y*state.r*smoothstep(0.,mix(0.5,2.,fx.shape.z)/32.,edge);var color=fx.color.rgb;var emission=color*fx.params.z;
        if fx.pyro.x>0.5 && fx.params.x<0.5 {
            let fade=smoothstep(0.,mix(0.5,2.,fx.shape.z)/32.,edge);
            let soot=fx.params.y*state.r;let flame=state.b;let sigma=soot+flame*0.12;
            let temperature=293.+state.g*4400.;let thermal=select(vec3(1.),fx_temperature_color(temperature),fx.pyro.w>0.5);
            let radiant=pow(clamp((temperature-650.)/1200.,0.,2.),2.);
            density=sigma*fade;color=fx.soot.rgb*soot/max(sigma,0.00001);
            emission=fx.color.rgb*thermal*fx.params.z*flame*radiant*fx.flow.w/max(sigma,0.00001);
        }
        else if fx.params.x<0.5 {
            let heat=clamp(state.g*1.6,0.,1.);let hot=mix(fx.color.rgb,vec3(1.,0.92,0.65),heat*heat);
            color=mix(vec3(0.065,0.073,0.085)+fx.color.rgb*0.012,hot,heat);
            emission=hot*fx.params.z*heat*heat*fx.flow.w;density*=mix(fx.shape.w,1.,heat);
        }
        if fx.soot.w>0.5 && fx.pyro.x>0.5 && fx.params.x<0.5 {
            // High-contrast filament emission follows actual hot simulated cells.
            // Two inexpensive sine layers avoid another 3-D noise lookup per ray.
            let filaments=abs(sin(p.x*51.+p.y*64.+sin(p.z*37.+fx.params.w)*1.6));
            let veins=pow(filaments,8.);
            emission*=0.7+2.1*veins;
            let accents=fx_blast_accents(fx,p);
            let spark_density=accents.x*0.48;let dust_density=accents.y*0.28;
            let total=density+spark_density+dust_density;
            if total>0.00001 {
                emission=(emission*density+vec3(1.,0.32,0.045)*fx.params.z*10.*spark_density)/total;
                color=(color*density+vec3(1.,0.23,0.02)*spark_density
                    +fx.soot.rgb*dust_density)/total;
                density=total;
            }
        }
        if fx.motion.w>=2. {density*=1.-smoothstep(0.65,1.,clamp(fx.params.w/max(fx.flow.z,0.1),0.,1.));}
        return FxSample(color,density,emission);
    }
    let time=fx.params.w;let detail=fx.motion.x;let turbulence=fx.motion.y;
    let width=fx.shape.x;let softness=fx.shape.z;var seed=fract(fx.motion.z*vec3(0.173,0.317,0.071))*71.;
    var y=p.y/fx.shape.y;var xz=vec2(0.);var domain=1.;
    if fx.guide_meta.x>0. {
        let coordinate=fx_coordinate(volume,p);if coordinate.age<0. {return FxSample(vec3(0.),0.,vec3(0.));}
        y=coordinate.age;xz=coordinate.xz/width;seed+=coordinate.phase*vec3(0.13,0.37,0.19);
        domain=smoothstep(0.,0.025,y)*(1.-smoothstep(1.-softness*0.22,1.,y));
    } else {
        if any(p<vec3(-0.5,0.,-0.5)) || any(p>vec3(0.5,fx.shape.y,0.5)) {return FxSample(vec3(0.),0.,vec3(0.));}
        let bend=fx.flow.xy*y*y+sin(vec2(y*6.7-time*1.3+seed.x,y*8.3-time+seed.z))*turbulence*0.09*y;
        xz=(p.xz-bend)/width;
        domain=(1.-smoothstep(0.5-softness*0.16,0.5,max(abs(p.x),abs(p.z))))
            *smoothstep(0.,0.025,y)*(1.-smoothstep(1.-softness*0.22,1.,y));
    }
    let radial=length(xz);
    let q=vec3(xz.x,y,xz.y)*detail+seed-vec3(0.,time,0.);
    // One continuous density field at every quality level avoids changes of shape on export.
    let coarse=fx_noise(q*vec3(1.,0.65,1.));
    let fine=fx_noise(q*2.07+vec3(4.1));let noise=coarse*0.7+fine*0.3;
    var density=0.;var color=fx.color.rgb;var emission=vec3(0.);
    // Cloud and burst are modes of Fire/Smoke/Fog/Steam, not additional FX kinds.
    if fx.motion.w>0.5 {
        let burst=(u32(fx.motion.w)&2u)!=0u;
        let progress=clamp(time/max(fx.flow.z,0.1),0.,1.);
        if burst && (progress<=0. || progress>=1.) {return FxSample(vec3(0.),0.,vec3(0.));}
        let center=select(0.5,0.32+progress*0.14,burst);
        let puff=vec3(xz.x,y-center,xz.y);
        let distance=length(puff);
        let expansion=select(0.45,0.055+0.395*(1.-exp(-progress*10.)),burst);
        let edge=max(expansion+(coarse-0.5)*turbulence*0.12,0.002);
        let envelope=1.-smoothstep(max(edge*(1.-softness*0.75),0.001),edge,distance);
        let fade=select(1.,smoothstep(0.,0.03,progress)*pow(1.-progress,1.4),burst);
        density=fx.params.y*envelope*fade*smoothstep(0.16,0.75,noise)*domain;
        color*=mix(0.55,1.2,coarse);
        if fx.params.x<0.5 {
            let heat=clamp(1.-distance/max(expansion,0.001),0.,1.);
            let hot=mix(fx.color.rgb,vec3(1.,0.9,0.65),heat*heat);
            let flame=select(1.,exp(-progress*5.),burst);
            color=mix(vec3(0.07,0.075,0.08)+fx.color.rgb*0.02,hot,flame);
            emission=hot*fx.params.z*fx.flow.w*(0.4+heat*1.8);
        } else {emission=color*fx.params.z;}
        return FxSample(color,density,emission);
    }
    if fx.params.x<0.5 {
        var angle=0.;if dot(xz,xz)>0.0000000001 {angle=atan2(xz.y,xz.x);}
        let tongue=sin(angle*3.+y*8.-time*2.1+seed.x)*0.5+0.5;
        let radius=mix(0.34,0.025,clamp(y/0.78,0.,1.))+(coarse-0.5)*turbulence*0.19;
        let envelope=(1.-smoothstep(max(radius*(1.-softness*0.8),0.001),max(radius,0.002),radial));
        let tip=0.58+tongue*0.16+(coarse-0.5)*turbulence*0.16;
        let fire=envelope*(1.-smoothstep(tip-0.15,tip,y))*smoothstep(0.2,0.72,noise+(1.-y)*0.32);
        let heat=clamp((1.-y*1.45-radial*1.4+fine*0.32)*fire,0.,1.);
        let hot=mix(sqrt(fx.color.rgb),vec3(1.),heat*heat*0.85);
        let flame_color=mix(fx.color.rgb,hot,heat);
        let smoke_radius=mix(0.1,0.46,y)+(coarse-0.5)*turbulence*0.12;
        let smoke=(1.-smoothstep(smoke_radius*(1.-softness*0.7),smoke_radius,radial))
            *smoothstep(0.32,0.63,y)*smoothstep(0.22,0.78,noise)*fx.shape.w;
        let total=fire+smoke;
        let weight=fire/max(total,0.00001);
        color=mix(vec3(0.065,0.073,0.085)+fx.color.rgb*0.012,flame_color,weight);
        emission=flame_color*fx.params.z*(0.4+heat*1.8)*weight*fx.flow.w;
        density=total*fx.params.y*domain;
    } else if fx.params.x<1.5 || fx.params.x>2.5 {
        let steam=fx.params.x>2.5;
        let radius=mix(0.12,0.45,sqrt(y))+(coarse-0.5)*turbulence*0.2;
        let envelope=1.-smoothstep(max(radius*(1.-softness*0.8),0.001),max(radius,0.002),radial);
        let billow=smoothstep(select(0.24,0.33,steam),select(0.77,0.7,steam),noise);
        let dissipation=select(1.-y*0.22,exp(-y*1.6),steam);
        density=fx.params.y*envelope*billow*dissipation*domain;
        color*=mix(0.55,1.2,coarse);emission=color*fx.params.z;
    } else {
        let horizontal=select(max(abs(xz.x),abs(xz.y)),radial,(fx.guide_meta.x>0. || fx.guide_meta.w>0.));
        let edge=1.-smoothstep(0.5-softness*0.32,0.5,horizontal);
        let layer=0.82+0.18*sin(y*9.+coarse*2.+time*0.2);
        density=fx.params.y*edge*domain*exp(-y*2.4)*layer*mix(1.,noise,turbulence);
        emission=color*fx.params.z;
    }
    return FxSample(color,density,emission);
}
fn fx_illumination(volume:u32,world:vec3<f32>,ray:vec3<f32>)->vec3<f32> {
    let fx=fx_properties(volume);
    var energy=vec3(0.24);
    for(var i=0u;i<u32(display.light_meta.x);i+=1u) {
        let light=display.lights[i];var attenuation=1.;var l=-light.direction_range.xyz;var light_distance=100000.;
        if light.position_kind.w>0.5 {
            let delta=light.position_kind.xyz-world;let distance=max(length(delta),0.1);
            l=delta/distance;light_distance=distance;attenuation=1./(1.+distance*distance);
            if light.direction_range.w>0. {attenuation*=1.-smoothstep(light.direction_range.w*0.8,light.direction_range.w,distance);}
            if light.position_kind.w>1.5 && light.position_kind.w<2.5 {
                let cone=clamp((dot(-l,light.direction_range.xyz)-light.cone.y)/max(light.cone.x-light.cone.y,0.00001),0.,1.);
                attenuation*=cone*cone;
            } else if light.position_kind.w>2.5 {attenuation*=max(dot(-l,light.direction_range.xyz),0.);}
        }
        var phase=0.6+0.8*pow(clamp(0.5+0.5*dot(l,ray),0.,1.),2.);
        if fx.pyro.x>0.5 {let g=fx.pyro.y;let d=max(1.+g*g-2.*g*dot(l,ray),0.0001);phase=(1.-g*g)/(d*sqrt(d));}
        var occlusion=1.;if light.shape.w>0.5 {occlusion=visibility(i,world);}
        if i==u32(display.fx_meta.w) && attenuation>0.00001 {occlusion*=fx_self_transmission(volume,world,l,light_distance);}
        energy+=light.color_intensity.rgb*light.color_intensity.a*attenuation*phase*occlusion*0.12;
    }
    return min(energy,vec3(8.));
}
fn volume_fx(scene:vec4<f32>,pixel:vec2<f32>)->vec4<f32> {
    let count=u32(display.fx_meta.x);if count==0u {return scene;}
    let uv=(pixel-display.viewport.xy)/display.viewport.zw;
    if any(uv<vec2(0.)) || any(uv>vec2(1.)) {return scene;}
    let origin=select(display.eye.xyz,unproject(uv,0.),display.light_meta.z>0.5);
    let end=unproject(uv,min(unpack_depth(textureLoad(scene_depth,vec2<i32>(pixel),0)),0.999999));
    let delta=end-origin;let limit=length(delta);if limit<0.00001 {return scene;}
    let ray=delta/limit;
    var spans:array<vec2<f32>,8>;var regions:array<vec2<f32>,8>;var hits=0u;
    // Sort occupied intervals, then merge overlaps. Empty gaps consume no samples.
    for(var i=0u;i<count;i+=1u) {
        let span=fx_interval(i,origin,ray,limit);spans[i]=span;
        if span.y<=span.x {continue;}
        var slot=hits;
        loop {
            if slot==0u {break;}
            if regions[slot-1u].x<=span.x {break;}
            regions[slot]=regions[slot-1u];slot-=1u;
        }
        regions[slot]=span;hits+=1u;
    }
    if hits==0u {return scene;}
    var merged=0u;
    for(var i=0u;i<hits;i+=1u) {
        let span=regions[i];
        if merged==0u {regions[0]=span;merged=1u;}
        else if span.x>regions[merged-1u].y {regions[merged]=span;merged+=1u;}
        else {regions[merged-1u].y=max(regions[merged-1u].y,span.y);}
    }
    var occupied=0.;for(var i=0u;i<merged;i+=1u) {occupied+=regions[i].y-regions[i].x;}
    let jitter=fract(52.9829189*fract(dot(floor(pixel),vec2(0.06711056,0.00583715))));
    var transmission=1.;var accumulated=vec3(0.);
    for(var region=0u;region<merged;region+=1u) {
        let span=regions[region];let span_length=span.y-span.x;
        let steps=u32(max(4.,round(display.fx_meta.y*span_length/max(occupied,0.000001))));let step=span_length/f32(steps);
        for(var j=0u;j<steps;j+=1u) {
            let distance=span.x+(f32(j)+jitter)*step;let world=origin+ray*distance;
            var density=0.;var scatter=vec3(0.);var emission=vec3(0.);
            for(var i=0u;i<count;i+=1u) {
                if distance<spans[i].x || distance>=spans[i].y {continue;}
                let sample=fx_sample(i,world);
                if sample.density>0.00001 {density+=sample.density;scatter+=sample.color*sample.density*fx_illumination(i,world,ray);emission+=sample.emission*sample.density;}
            }
            if density>0.00001 {
                let absorption=1.-exp(-density*step);
                accumulated+=transmission*absorption*(scatter+emission)/density;
                transmission*=1.-absorption;
            }
            if transmission<0.002 {break;}
        }
        if transmission<0.002 {break;}
    }
    return vec4(accumulated+scene.rgb*transmission,1.-(1.-scene.a)*transmission);
}
