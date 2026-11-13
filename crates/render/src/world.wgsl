struct World {
    counts:vec4<u32>,environment:vec4<f32>,sh:array<vec4<f32>,9>,quality:vec4<f32>,sky:vec4<f32>,ground:vec4<f32>,lights:array<Light,8>,emitters:array<vec4<f32>,16>,emission_meta:vec4<u32>,
}
struct RayNode { low:vec4<f32>,high:vec4<f32>,info:vec4<u32> }
struct RayTriangle { a:vec4<f32>,b:vec4<f32>,c:vec4<f32>,na:vec4<f32>,nb:vec4<f32>,nc:vec4<f32>,uv_y:vec4<f32>,colors:vec4<u32> }
struct RayMaterial {base:vec4<f32>,properties:vec4<f32>,emission:vec4<f32>,uv:vec4<f32>,rotation:vec4<f32>,base_uv:array<vec4<f32>,2>,sampling:vec4<u32>,emission_uv:array<vec4<f32>,2>,emission_sampling:vec4<u32>,absorption:vec4<f32>,optics:vec4<f32>,graph:Graph,npr:Npr,artistic:Artistic,layers:vec4<u32>}
@group(WORLD_GROUP) @binding(4) var<uniform> world:World;
@group(WORLD_GROUP) @binding(5) var<storage,read> ray_nodes:array<RayNode>;
@group(WORLD_GROUP) @binding(6) var<storage,read> ray_triangles:array<RayTriangle>;
@group(WORLD_GROUP) @binding(7) var<storage,read> ray_materials:array<RayMaterial>;
@group(WORLD_GROUP) @binding(8) var environment_source:texture_2d<f32>;
@group(WORLD_GROUP) @binding(9) var environment_filtered:texture_2d_array<f32>;
@group(WORLD_GROUP) @binding(10) var environment_sampler:sampler;
@group(WORLD_GROUP) @binding(11) var ray_color_maps:texture_2d_array<f32>;
@group(WORLD_GROUP) @binding(12) var graph_images:texture_2d_array<f32>;
fn world_panorama(n:vec3<f32>)->vec2<f32>{return vec2(atan2(n.z,n.x)/6.2831853+0.5,acos(clamp(n.y,-1.,1.))/3.14159265);}
fn world_direction(n:vec3<f32>)->vec3<f32>{return vec3(n.x*world.environment.y+n.z*world.environment.z,n.y,n.z*world.environment.y-n.x*world.environment.z);}
fn world_environment(n:vec3<f32>,roughness:f32)->vec3<f32>{
    if world.ground.w<0.5{return mix(world.ground.rgb,world.sky.rgb,clamp(0.5+0.5*n.y*(1.-roughness*roughness),0.,1.));}
    let uv=world_panorama(world_direction(n));
    if roughness<0.02{return textureSampleLevel(environment_source,environment_sampler,uv,0.).rgb*world.environment.x;}
    let layer=clamp(roughness,0.,1.)*7.;let low=i32(floor(layer));let high=min(low+1,7);
    return mix(textureSampleLevel(environment_filtered,environment_sampler,uv,low,0.).rgb,textureSampleLevel(environment_filtered,environment_sampler,uv,high,0.).rgb,fract(layer))*world.environment.x;
}
fn world_irradiance(direction:vec3<f32>)->vec3<f32>{
    if world.ground.w<0.5{return mix(world.ground.rgb,world.sky.rgb,clamp(0.5+direction.y/3.,0.,1.));}
    let n=world_direction(direction);let b=array<f32,9>(0.282095,0.488603*n.y,0.488603*n.z,0.488603*n.x,1.092548*n.x*n.y,1.092548*n.y*n.z,0.315392*(3.*n.z*n.z-1.),1.092548*n.x*n.z,0.546274*(n.x*n.x-n.y*n.y));
    var result=vec3(0.);for(var i=0u;i<9u;i+=1u){result+=world.sh[i].rgb*b[i];}return max(result,vec3(0.))*world.environment.x;
}
fn world_bias(position:vec3<f32>)->f32{return max(world.quality.z,length(position)*0.000002);}
struct RayHit {distance:f32,triangle:u32,barycentric:vec2<f32>}
// Ray colors retain the existing fixed-resolution, level-zero atlas. Address
// individual texels so linear filtering wraps across Repeat seams correctly.
fn ray_texel_coordinate(p:i32, size:i32, mode:u32)->i32 {
    if mode==2u { return clamp(p,0,size-1); }
    let period=select(size,size*2,mode==1u);
    let wrapped=((p%period)+period)%period;
    return select(wrapped,period-1-wrapped,mode==1u && wrapped>=size);
}
fn ray_texel(p:vec2<i32>, layer:i32, modes:vec2<u32>)->vec4<f32> {
    let size=vec2<i32>(textureDimensions(ray_color_maps));
    let q=vec2(ray_texel_coordinate(p.x,size.x,modes.x),ray_texel_coordinate(p.y,size.y,modes.y));
    // Keep an explicit sampler on GL as well; at texel centres filtering is exact.
    return textureSampleLevel(ray_color_maps,environment_sampler,(vec2<f32>(q)+0.5)/vec2<f32>(size),layer,0.);
}
fn ray_sample(uv:vec2<f32>,layer:i32,settings:vec4<u32>)->vec4<f32> {
    if layer==0 {return vec4(1.);}
    // Reduce large UVs before integer conversion, retaining the wrap period.
    var safe_uv=uv;
    for(var i=0u;i<2u;i+=1u) {
        if settings[i]==2u {safe_uv[i]=clamp(uv[i],0.,1.);}
        else {let period=select(1.,2.,settings[i]==1u);safe_uv[i]=uv[i]-floor(uv[i]/period)*period;}
    }
    let p=safe_uv*vec2<f32>(textureDimensions(ray_color_maps));
    if settings.z==1u {return ray_texel(vec2<i32>(floor(p)),layer,settings.xy);}
    let base=vec2<i32>(floor(p-0.5));let f=fract(p-0.5);
    return mix(mix(ray_texel(base,layer,settings.xy),ray_texel(base+vec2(1,0),layer,settings.xy),f.x),
        mix(ray_texel(base+vec2(0,1),layer,settings.xy),ray_texel(base+vec2(1,1),layer,settings.xy),f.x),f.y);
}
fn ray_material_surface(hit:RayHit,index:u32)->GraphResult{
    let t=ray_triangles[hit.triangle];let m=ray_materials[index];let weights=vec3(1.-hit.barycentric.x-hit.barycentric.y,hit.barycentric);
    var uv=vec2(dot(vec3(t.na.w,t.nb.w,t.nc.w),weights),dot(t.uv_y.xyz,weights))*m.uv.xy;
    uv=vec2(uv.x*m.rotation.x-uv.y*m.rotation.y,uv.x*m.rotation.y+uv.y*m.rotation.x)+m.uv.zw;
    let base_uv=vec2(dot(m.base_uv[0].xyz,vec3(uv,1.)),dot(m.base_uv[1].xyz,vec3(uv,1.)));
    let sampled=ray_sample(base_uv,i32(m.rotation.z),m.sampling);
    let color=unpack4x8unorm(t.colors.x)*weights.x+unpack4x8unorm(t.colors.y)*weights.y+unpack4x8unorm(t.colors.z)*weights.z;
    let emission_uv=vec2(dot(m.emission_uv[0].xyz,vec3(uv,1.)),dot(m.emission_uv[1].xyz,vec3(uv,1.)));
    let emission_map=ray_sample(emission_uv,i32(m.emission_sampling.w),m.emission_sampling);
    var surface = graph_evaluate(m.graph,uv,sampled,m.base*sampled*color,m.properties,
        vec4(m.emission.rgb*emission_map.rgb,m.emission.w),vec4(m.emission.w,0.,0.1,m.properties.w));
    if m.npr.ramp.x > 0.5 { surface.surface.w = 0.; }
    return surface;
}
fn ray_surface(hit:RayHit)->GraphResult {
    var index=u32(ray_triangles[hit.triangle].a.w);
    if ray_materials[index].layers.x==0u { return ray_material_surface(hit,index); }
    var result:GraphResult;var coverage=0.;
    for(var layer=0u;layer<65u;layer+=1u) {
        let m=ray_materials[index];let surface=ray_material_surface(hit,index);
        var alpha=clamp(surface.color.a,0.,1.);
        if m.properties.z>=0. {alpha=select(0.,1.,alpha>=m.properties.z);}
        else if m.layers.y==0u && m.properties.z> -1.5 {alpha=1.;}
        if m.layers.z==0u {alpha=0.;}
        let next_coverage=alpha+coverage*(1.-alpha);
        let weight=alpha/max(next_coverage,0.000001);
        result.color=vec4(mix(result.color.rgb,surface.color.rgb,weight),next_coverage);
        result.properties=mix(result.properties,surface.properties,weight);
        result.emission=mix(result.emission,surface.emission,weight);
        result.surface=mix(result.surface,surface.surface,weight);
        result.normal=mix(result.normal,surface.normal,weight);
        coverage=next_coverage;
        if m.layers.x==0u {break;}index=m.layers.x;
    }
    return result;
}
fn ray_normal(hit:RayHit)->vec3<f32>{
    let t=ray_triangles[hit.triangle];let w=vec3(1.-hit.barycentric.x-hit.barycentric.y,hit.barycentric);
    let n=t.na.xyz*w.x+t.nb.xyz*w.y+t.nc.xyz*w.z;
    if dot(n,n)<0.0000001{return normalize(cross(t.b.xyz-t.a.xyz,t.c.xyz-t.a.xyz));}return normalize(n);
}
fn ray_query(origin:vec3<f32>,direction:vec3<f32>,maximum:f32,reflection:bool,object_id:f32)->RayHit{
    if !ARTFORGE_RAY_TRACING {return RayHit(maximum,0xffffffffu,vec2(0.));}
    var hit=RayHit(maximum,0xffffffffu,vec2(0.));var index=0u;
    let inv=1./select(vec3(0.00000000000000000001),direction,abs(direction)>vec3(0.00000000000000000001));
    loop{
        if index>=world.counts.x{break;}
        let node=ray_nodes[index];let a=(node.low.xyz-origin)*inv;let b=(node.high.xyz-origin)*inv;let near=min(a,b);let far=max(a,b);
        if max(max(near.x,near.y),max(near.z,0.))>min(min(far.x,far.y),min(far.z,hit.distance)){index=node.info.x;continue;}
        if node.info.z==0u{index+=1u;continue;}
        for(var j=0u;j<node.info.z;j+=1u){
            let triangle=node.info.y+j;let t=ray_triangles[triangle];if object_id>0.&&t.b.w!=object_id{continue;}let e1=t.b.xyz-t.a.xyz;let e2=t.c.xyz-t.a.xyz;
            // Reflection/refraction rays skip shadow-only surfaces; shadow queries keep them.
            if reflection&&(t.colors.w&1u)!=0u{continue;}
            let p=cross(direction,e2);let determinant=dot(e1,p);if abs(determinant)<0.00000001{continue;}
            let inv_det=1./determinant;let s=origin-t.a.xyz;let u=dot(s,p)*inv_det;if u<0.||u>1.{continue;}
            let q=cross(s,e1);let v=dot(direction,q)*inv_det;if v<0.||u+v>1.{continue;}
            let distance=dot(e2,q)*inv_det;if distance<=0.00001||distance>=hit.distance{continue;}
            let candidate=RayHit(distance,triangle,vec2(u,v));let material=ray_materials[u32(t.a.w)];
            if reflection&&material.rotation.w<0.5&&material.properties.w<=0.&&material.graph.outputs[1].y<0&&dot(ray_normal(candidate),direction)>0.{continue;}
            if material.layers.x!=0u {if ray_surface(candidate).color.a<0.001{continue;}}
            else if material.properties.z>=0.&&ray_surface(candidate).color.a<material.properties.z{continue;}
            hit=candidate;
        }
        index=node.info.x;
    }
    return hit;
}
fn ray_nearest(origin:vec3<f32>,direction:vec3<f32>,maximum:f32,reflection:bool)->RayHit{
    return ray_query(origin,direction,maximum,reflection,0.);
}
fn ray_geometric_normal(hit:RayHit)->vec3<f32>{
    let t=ray_triangles[hit.triangle];return normalize(cross(t.b.xyz-t.a.xyz,t.c.xyz-t.a.xyz))*select(1.,t.c.w,t.c.w!=0.);
}
fn ray_absorption(m:RayMaterial)->vec3<f32>{return -log(clamp(m.absorption.rgb,vec3(0.001),vec3(1.)))/max(m.optics.y,0.001);}
fn ray_fresnel(eta_i:f32,eta_t:f32,cosine:f32)->f32 {
    let f=pow((eta_t-eta_i)/(eta_t+eta_i),2.);return f+(1.-f)*pow(1.-abs(cosine),5.);
}
// Up to eight nested media and sixteen surface crossings. Inner volumes
// displace outer media. Open surfaces use their authored thin-sheet thickness.
fn ray_transmittance(position:vec3<f32>,normal:vec3<f32>,direction:vec3<f32>,maximum:f32)->vec3<f32>{
    if !ARTFORGE_RAY_TRACING {return vec3(1.);}
    let bias=world_bias(position);var origin=position+normal*bias;var remaining=maximum-bias;var throughput=vec3(1.);
    var objects:array<f32,8>;var media:array<vec4<f32>,8>;var count=0u;
    for(var layer=0;layer<16;layer+=1){
        let hit=ray_nearest(origin,direction,remaining,false);
        if hit.triangle==0xffffffffu {
            if count>0u {throughput*=exp(-media[count-1u].rgb*remaining);}return throughput;
        }
        let t=ray_triangles[hit.triangle];let m=ray_materials[u32(t.a.w)];let surface=ray_surface(hit);
        let alpha=select(1.,surface.color.a,m.properties.z< -1.5||m.layers.x!=0u);let transmission=surface.surface.w*(1.-surface.properties.x);
        if transmission<=0.&&alpha>=0.999{return vec3(0.);}
        let closed=m.optics.z>0.5&&t.c.w!=0.;let entering=dot(ray_geometric_normal(hit),direction)<0.;
        var eta_i=1.;if count>0u {let current=media[count-1u];throughput*=exp(-current.rgb*hit.distance);eta_i=current.a;}
        var eta_t=surface.surface.x;
        if closed&&!entering {eta_t=1.;if count>1u {eta_t=media[count-2u].a;}}
        let fresnel=ray_fresnel(eta_i,eta_t,dot(ray_normal(hit),direction));
        var absorption=vec3(1.);
        if !closed {absorption=exp(-ray_absorption(m)*m.optics.x/max(abs(dot(ray_normal(hit),direction)),0.1));}
        throughput*=mix(vec3(1.),absorption*transmission*(1.-fresnel),alpha);
        if closed&&transmission>0.&&alpha>0.95 {
            if entering {if count>=8u{return vec3(0.);}objects[count]=t.b.w;media[count]=vec4(ray_absorption(m),surface.surface.x);count+=1u;}
            else if count>0u {for(var j=0u;j<count;j+=1u){if objects[j]==t.b.w {for(var k=j;k+1u<count;k+=1u){objects[k]=objects[k+1u];media[k]=media[k+1u];}count-=1u;break;}}}
        }
        if max(throughput.r,max(throughput.g,throughput.b))<0.001{return vec3(0.);}
        origin+=direction*(hit.distance+bias);remaining-=hit.distance+bias;if remaining<=0.{return throughput;}
    }
    return vec3(0.);
}
// Mesh emitters share the ray geometry and evaluate authored textures, graphs,
// visibility, polygon assignments and material layers at actual surface points.
struct MeshEmission { direction:vec3<f32>, radiance:vec3<f32> }
fn world_emission_count()->u32 { return select(0u,world.emission_meta.x,ARTFORGE_RAY_TRACING); }
fn emitted_radiance(hit:RayHit,outgoing:vec3<f32>,volume:bool)->vec3<f32> {
    let t=ray_triangles[hit.triangle];
    var normal=normalize(cross(t.b.xyz-t.a.xyz,t.c.xyz-t.a.xyz));
    if dot(normal,t.na.xyz+t.nb.xyz+t.nc.xyz)<0. {normal=-normal;}
    let facing=dot(normal,outgoing);
    var index=u32(t.a.w);var emission=vec3(0.);
    for(var layer=0u;layer<65u;layer+=1u) {
        let m=ray_materials[index];let surface=ray_material_surface(hit,index);
        var alpha=clamp(surface.color.a,0.,1.);
        if m.properties.z>=0. {alpha=select(0.,1.,alpha>=m.properties.z);}
        else if m.layers.y==0u && m.properties.z> -1.5 {alpha=1.;}
        if m.layers.z==0u {alpha=0.;}
        let cosine=select(max(facing,0.),abs(facing),m.rotation.w>0.5);
        let enabled=!volume || m.layers.w!=0u;
        emission=emission*(1.-alpha)+select(vec3(0.),surface.emission.rgb*cosine*alpha,enabled);
        if m.layers.x==0u {break;}index=m.layers.x;
    }
    return emission;
}
fn mesh_emission(index:u32,position:vec3<f32>,normal:vec3<f32>,volume:bool)->MeshEmission {
    if !ARTFORGE_RAY_TRACING {return MeshEmission(vec3(0.,1.,0.),vec3(0.));}
    var result=MeshEmission(vec3(0.,1.,0.),vec3(0.));
    if index>=world.emission_meta.x {return result;}
    let sample=world.emitters[index];
    let hit=RayHit(0.,u32(sample.x),sample.yz);let t=ray_triangles[hit.triangle];
    let point=t.a.xyz*(1.-sample.y-sample.z)+t.b.xyz*sample.y+t.c.xyz*sample.z;
    let delta=point-position;let distance=length(delta);let bias=max(world_bias(position),world_bias(point));
    if distance<=bias*8. {return result;}
    result.direction=delta/distance;
    if dot(normal,normal)>0.5 && dot(normal,result.direction)<=0. {return result;}
    let emission=emitted_radiance(hit,-result.direction,volume);
    if max(emission.r,max(emission.g,emission.b))<=0. {return result;}
    var visibility=vec3(1.);
    if world.emission_meta.y!=0u {
        visibility=mix(vec3(1.),ray_transmittance(position,normal,result.direction,distance-bias*4.),world.quality.w);
    }
    // Integrating a finite patch avoids singularities when a receiver is close.
    result.radiance=emission*sample.w/(distance*distance+sample.w/3.14159265)*visibility;
    return result;
}
fn ray_brdf(base:vec3<f32>,metallic:f32,roughness:f32,ior:f32,transmission:f32,n:vec3<f32>,v:vec3<f32>,l:vec3<f32>)->vec3<f32>{
    let nv=max(dot(n,v),0.0001);let nl=max(dot(n,l),0.);if nl<=0.{return vec3(0.);}let h=normalize(v+l);let nh=max(dot(n,h),0.);let vh=max(dot(v,h),0.);
    let a2=pow(max(roughness,0.045),4.);let d=a2/(3.14159265*pow(nh*nh*(a2-1.)+1.,2.));
    let visibility=0.5/max(nl*sqrt(nv*nv*(1.-a2)+a2)+nv*sqrt(nl*nl*(1.-a2)+a2),0.00001);
    let f0=mix(vec3(pow((ior-1.)/(ior+1.),2.)),base,metallic);let f=f0+(vec3(1.)-f0)*pow(1.-vh,5.);
    return (base*(1.-metallic)*(1.-transmission)*(vec3(1.)-f)/3.14159265+d*visibility*f)*nl;
}
fn ray_shade(hit:RayHit,position:vec3<f32>,incoming:vec3<f32>)->vec3<f32>{
    let material=ray_materials[u32(ray_triangles[hit.triangle].a.w)];let surface=ray_surface(hit);
    // Secondary rays use bounded, coarser world-space detail (no extra triangle data).
    let art=art_sample(material.artistic,position,0.04);
    let base=art_base(material.artistic,surface.color.rgb,art);
    var n=ray_normal(hit);if dot(n,incoming)>0.{n= -n;}let v= -incoming;let metal=surface.properties.x;
    let rough=art_roughness(material.artistic,surface.properties.y,art);
    let f0=mix(vec3(pow((surface.surface.x-1.)/(surface.surface.x+1.),2.)),base,metal);
    var color=(base*(1.-metal)*(1.-surface.surface.w)*(vec3(1.)-f0)*world_irradiance(n)+f0*world_environment(reflect(incoming,n),rough))*world.sky.w+surface.emission.rgb;
    var npr_diffuse = world_irradiance(n) * world.sky.w;
    var npr_specular = vec3(0.);
    for(var i=0u;i<world.counts.y;i+=1u){
        let light=world.lights[i];var delta= -light.direction_range.xyz*100000.;var attenuation=1.;
        if light.position_kind.w>0.5{delta=light.position_kind.xyz-position;let distance=max(length(delta),0.001);attenuation=1./(distance*distance);
            if light.direction_range.w>0.{let r=distance/light.direction_range.w;let fade=max(1.-r*r*r*r,0.);attenuation*=fade*fade;}
        }
        let distance=max(length(delta),0.001);let l=delta/distance;
        if light.position_kind.w>2.5{attenuation*=light.cone.z*light.cone.w*max(dot(light.direction_range.xyz,-l),0.);}
        else if light.position_kind.w>1.5{let cone=clamp((dot(-l,light.direction_range.xyz)-light.cone.y)/max(light.cone.x-light.cone.y,0.00001),0.,1.);attenuation*=cone*cone;}
        var visibility=vec3(1.);if light.shape.w>0.5&&world.counts.z>0u{visibility=mix(vec3(1.),ray_transmittance(position,n,l,distance),world.quality.w);}
        if material.npr.ramp.x > 0.5 {
            let sample = npr_light(material.npr, n, v, l,
                light.color_intensity.rgb * light.color_intensity.a * attenuation * visibility, 0.001);
            npr_diffuse += sample.diffuse;
            npr_specular += sample.specular;
            continue;
        }
        color+=ray_brdf(base,metal,rough,surface.surface.x,surface.surface.w,n,v,l)*light.color_intensity.rgb*light.color_intensity.a*attenuation*visibility;
    }
    // Keep secondary rays bounded: two strata represent the complete emitter table.
    let emission_samples=min(world.emission_meta.x,2u);
    for(var j=0u;j<emission_samples;j+=1u) {
        let i=min(u32((f32(j)+0.5)*f32(world.emission_meta.x)/f32(emission_samples)),world.emission_meta.x-1u);
        let sample=mesh_emission(i,position,n,false);
        let radiance=sample.radiance*f32(world.emission_meta.x)/f32(emission_samples);
        if max(radiance.r,max(radiance.g,radiance.b))<=0. {continue;}
        if material.npr.ramp.x>0.5 {
            let light=npr_light(material.npr,n,v,sample.direction,radiance,0.001);
            npr_diffuse+=light.diffuse;npr_specular+=light.specular;
        } else {color+=ray_brdf(base,metal,rough,surface.surface.x,surface.surface.w,n,v,sample.direction)*radiance;}
    }
    if material.npr.ramp.x > 0.5 {
        color = npr_finish(material.npr, base, n, v, npr_diffuse, npr_specular, 0.001, position, 0.001) + surface.emission.rgb;
    }
    if material.artistic.params.x>0.5 {
        color=art_finish(material.artistic,max(color-surface.emission.rgb,vec3(0.)),base,art,n,v)+surface.emission.rgb;
    }
    return min(color,vec3(65000.));
}
// Bounded transmission walk for reflected objects and camera refraction.
// The specular branch uses the environment; this is not a full path tracer.
fn ray_radiance(start:vec3<f32>,initial:vec3<f32>,roughness:f32,seed_object:f32,seed_medium:vec4<f32>,transparent:bool)->vec4<f32>{
    if !ARTFORGE_RAY_TRACING {return select(vec4(world_environment(initial,roughness)*world.sky.w,1.),vec4(0.),transparent);}
    var origin=start;var direction=initial;var throughput=vec3(1.);var color=vec3(0.);
    var objects:array<f32,8>;var media:array<vec4<f32>,8>;var count=0u;
    if seed_object>0. {objects[0]=seed_object;media[0]=seed_medium;count=1u;}
    for(var crossing=0;crossing<16;crossing+=1){
        let hit=ray_nearest(origin,direction,100000.,true);
        if hit.triangle==0xffffffffu {
            if !transparent {color+=throughput*world_environment(direction,roughness)*world.sky.w;return vec4(color,1.);}
            return vec4(color,clamp(1.-dot(throughput,vec3(1./3.)),0.,1.));
        }
        let position=origin+direction*hit.distance;let t=ray_triangles[hit.triangle];let m=ray_materials[u32(t.a.w)];let surface=ray_surface(hit);
        let alpha=select(1.,surface.color.a,m.properties.z< -1.5||m.layers.x!=0u);let transmission=surface.surface.w*(1.-surface.properties.x);
        var eta_i=1.;if count>0u {throughput*=exp(-media[count-1u].rgb*hit.distance);eta_i=media[count-1u].a;}
        let closed=m.optics.z>0.5&&t.c.w!=0.;let entering=dot(ray_geometric_normal(hit),direction)<0.;
        var n=ray_normal(hit);if dot(n,direction)>0.{n= -n;}
        var eta_t=surface.surface.x;
        if closed&&!entering {eta_t=1.;if count>1u {eta_t=media[count-2u].a;}}
        var outgoing=direction;
        if closed&&transmission>0.&&alpha>0.95 {
            outgoing=refract(direction,n,eta_i/eta_t);
            if dot(outgoing,outgoing)<0.00001 {direction=reflect(direction,n);origin=position+direction*world_bias(position);continue;}
        }
        color+=throughput*ray_shade(hit,position,direction)*alpha;
        if transmission<=0.&&alpha>=0.999 {return vec4(color,1.);}
        let fresnel=ray_fresnel(eta_i,eta_t,dot(n,direction));var tint=vec3(1.);
        if !closed {tint=exp(-ray_absorption(m)*m.optics.x/max(abs(dot(n,direction)),0.1));}
        throughput*=mix(vec3(1.),tint*transmission*(1.-fresnel),alpha);
        if closed&&transmission>0.&&alpha>0.95 {
            if entering {if count>=8u {return vec4(color,1.);}objects[count]=t.b.w;media[count]=vec4(ray_absorption(m),surface.surface.x);count+=1u;}
            else if count>0u {for(var j=0u;j<count;j+=1u){if objects[j]==t.b.w {for(var k=j;k+1u<count;k+=1u){objects[k]=objects[k+1u];media[k]=media[k+1u];}count-=1u;break;}}}
        }
        if max(throughput.r,max(throughput.g,throughput.b))<0.001 {return vec4(color,1.);}
        direction=outgoing;origin=position+direction*world_bias(position);
    }
    return vec4(color,1.);
}
fn scene_reflection(position:vec3<f32>,normal:vec3<f32>,view:vec3<f32>,roughness:f32)->vec3<f32>{
    let reflected=reflect(-view,normal);
    if !ARTFORGE_RAY_TRACING || world.counts.w==0u{return world_environment(reflected,roughness)*world.sky.w;}
    let helper=select(vec3(0.,1.,0.),vec3(0.,0.,1.),abs(normal.y)>0.95);let tangent=normalize(cross(helper,normal));let bitangent=cross(normal,tangent);
    let samples=select(u32(world.quality.x),1u,roughness<0.08);let a2=pow(max(roughness,0.001),4.);let origin=position+normal*world_bias(position);var color=vec3(0.);var weight=0.;
    for(var i=0u;i<samples;i+=1u){
        var direction=reflected;
        if samples>1u{let u=vec2((f32(i)+0.5)/f32(samples),f32(reverseBits(i))*2.3283064365386963e-10);let angle=u.x*6.2831853;let ct=sqrt((1.-u.y)/(1.+(a2-1.)*u.y));let st=sqrt(max(1.-ct*ct,0.));let h=tangent*(cos(angle)*st)+bitangent*(sin(angle)*st)+normal*ct;direction=reflect(-view,h);}
        let nl=max(dot(normal,direction),0.);if nl<=0.{continue;}
        let radiance=ray_radiance(origin,direction,roughness,0.,vec4(0.,0.,0.,1.),false);
        color+=radiance.rgb*nl;weight+=nl;
    }
    if weight<=0.00001{return world_environment(reflected,roughness)*world.sky.w;}return color/weight;
}

fn world_ray_shadows()->bool{return ARTFORGE_RAY_TRACING && world.counts.z>0u;}
fn world_settings_samples()->f32{return world.quality.y;}
fn world_shadow_strength()->f32{return world.quality.w;}
