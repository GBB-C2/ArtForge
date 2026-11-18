// Bit-preserving depth in RGBA8 avoids raw depth loads and optional float render targets.
fn unpack_depth(color:vec4<f32>)->f32 {
    let b=vec4<u32>(round(color*255.));
    return bitcast<f32>(b.x | (b.y<<8u) | (b.z<<16u) | (b.w<<24u));
}
struct Light {
    position_kind: vec4<f32>, // position xyz, directional=0 / point=1 / spot=2 / area=3
    direction_range: vec4<f32>, // rays travel along direction; range 0 = unlimited
    color_intensity: vec4<f32>,
    cone: vec4<f32>, // cos(inner), cos(outer), area width, area height
    shape: vec4<f32>, // rectangle roll (shared GPU layout)
};
struct FxVolume {
    inverse_model:mat4x4<f32>,params:vec4<f32>,color:vec4<f32>,motion:vec4<f32>,shape:vec4<f32>,flow:vec4<f32>,
    bounds_low:vec4<f32>,bounds_high:vec4<f32>,guide_meta:vec4<f32>,pyro:vec4<f32>,soot:vec4<f32>,guides:array<vec4<f32>,96>,
};
struct Display {
    params: vec4<f32>, // exposure, tone map, transparent output, explicit sRGB encode
    background: vec4<f32>,
    viewport: vec4<f32>,
    checker: vec4<f32>, // physical cell size, linear dark/light gray, unused
    secondary: vec4<f32>,
    gradient: vec4<f32>,
    inverse_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    volume: vec4<f32>, // extinction, height, maximum distance, anisotropy
    volume_color: vec4<f32>,
    volume_lights: array<vec4<f32>,8>, // independent tint RGB and enabled strength
    light_meta: vec4<f32>, // count, fog steps, orthographic, volume pixel stride (0 = disabled)
    lights: array<Light,8>,
    dof:vec4<f32>,
    forward:vec4<f32>,
    contour:vec4<f32>,
    contour_color:vec4<f32>,
    selection:vec4<f32>,
    camera_grade:vec4<f32>,
    camera_lens:vec4<f32>,
    fx_meta:vec4<f32>,
    fx:array<FxVolume,8>,
};
// Written once before display and lens sampling. RGB is in-scattered radiance, A is transmittance.
@group(2) @binding(0) var integrated_volume: texture_2d<f32>;
@group(0) @binding(0) var hdr_scene: texture_2d<f32>;
@group(0) @binding(1) var<uniform> display: Display;
@group(0) @binding(2) var scene_depth: texture_2d<f32>;
@group(0) @binding(3) var focus_depth:texture_2d<f32>;
@group(0) @binding(4) var selection_depth:texture_2d<f32>;
@group(0) @binding(5) var fx_collision_mask:texture_3d<f32>;
@group(0) @binding(6) var fx_gas_density:texture_3d<f32>;
@group(0) @binding(7) var fx_gas_sampler:sampler;
struct ShadowLight {view_proj:array<mat4x4<f32>,6>,params:vec4<f32>};
struct Shadow {lights:array<ShadowLight,8>,params:vec4<f32>};
@group(1) @binding(0) var<uniform> shadow:Shadow;
@group(1) @binding(1) var shadow_map:texture_depth_2d_array;
@group(1) @binding(2) var shadow_sampler:sampler_comparison;


@vertex fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(vec2(-1., -1.), vec2(3., -1.), vec2(-1., 3.));
    return vec4(positions[index], 0., 1.);
}
fn tone_map(hdr: vec3<f32>) -> vec3<f32> {
    let x = clamp(hdr, vec3(0.), vec3(10000.));
    if display.params.y < 0.5 {
        return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3(0.), vec3(1.));
    }
    if display.params.y < 1.5 { return x / (vec3(1.) + x); }
    return clamp(x, vec3(0.), vec3(1.));
}
fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(linear, vec3(1. / 2.4)) - vec3(0.055),
        linear * 12.92, linear <= vec3(0.0031308));
}
fn cube_face(direction:vec3<f32>)->u32 {
    let a=abs(direction);
    if a.x>=a.y && a.x>=a.z {return select(1u,0u,direction.x>=0.);}
    if a.y>=a.z {return select(3u,2u,direction.y>=0.);}
    return select(5u,4u,direction.z>=0.);
}
fn visibility(index:u32,world:vec3<f32>)->f32 {
    let info=shadow.lights[index].params;
    if shadow.params.x<=0. || info.y<0.5 {return 1.;}
    var face=0u;
    if info.y>1.5 {face=cube_face(world-display.lights[index].position_kind.xyz);}
    let clip=shadow.lights[index].view_proj[face]*vec4(world,1.);
    if clip.w<=0. {return 1.;}
    let ndc=clip.xyz/clip.w;
    let uv=vec2(ndc.x*0.5+0.5,0.5-ndc.y*0.5);
    if ndc.z<=0. || ndc.z>=1. || any(uv<vec2(0.)) || any(uv>vec2(1.)) {return 1.;}
    var bias=shadow.params.y;
    if info.z>0. {bias*=info.z*info.w/max(info.w-info.z,0.00001)/max(clip.w,info.z);}
    let texel=1./vec2<f32>(textureDimensions(shadow_map));
    var sum=0.;
    for(var tap=0u;tap<4u;tap+=1u) {
        let offset=(vec2<f32>(f32(tap&1u),f32(tap>>1u))-vec2(0.5))*texel;
        sum+=textureSampleCompareLevel(shadow_map,shadow_sampler,uv+offset,i32(info.x)+i32(face),ndc.z-bias);
    }
    return mix(1.,sum*0.25,shadow.params.x);
}
fn unproject(uv:vec2<f32>,depth:f32)->vec3<f32> {
    let h=display.inverse_view_proj*vec4(uv.x*2.-1.,1.-uv.y*2.,depth,1.);
    return h.xyz/h.w;
}
fn scattering_phase(g:f32, cosine:f32)->f32 {
    let d=max(1.+g*g-2.*g*cosine,0.001);
    return (1.-g*g)/(12.5663706*d*sqrt(d));
}

// Exact column density for the constant medium below y=0 and the exponential
// height profile above it. Swapping endpoints also handles downward camera rays.
fn column_density(a:f32,b:f32,distance:f32)->f32 {
    let h=display.volume.y;
    let lo = min(a, b) / h;
    let hi = max(a, b) / h;
    var mean=1.;
    if hi>0. {
        if hi-lo<0.0005 {
            mean=exp(-max((hi+lo)*0.5,0.));
        } else {
            let top=max(lo,0.);
            mean=(max(-lo,0.)+exp(-top)*(1.-exp(-(hi-top))))/(hi-lo);
        }
    }
    return max(mean,0.)*distance*display.volume.x;
}

fn fog(scene:vec4<f32>,pixel:vec2<f32>)->vec4<f32> {
    if display.volume.x<=0. {return scene;}
    let uv=(pixel-display.viewport.xy)/display.viewport.zw;
    if any(uv<vec2(0.)) || any(uv>vec2(1.)) {return scene;}
    let origin=select(display.eye.xyz,unproject(uv,0.),display.light_meta.z>0.5);
    let end=unproject(uv,min(unpack_depth(textureLoad(scene_depth,vec2<i32>(pixel),0)),0.999999));
    let delta = end - origin;
    let distance = min(length(delta), display.volume.z);
    if distance<0.0001 {return scene;}
    let ray=normalize(delta);
    let g=display.volume.w;
    let emitters=select(0u,world_emission_count(),display.volume_color.w>0.5);
    var constant_light=vec3(0.);
    var local_mask=0u;
    for(var i=0u;i<u32(display.light_meta.x);i+=1u) {
        let volume = display.volume_lights[i];
        let light = display.lights[i];
        if volume.a<=0. || light.color_intensity.a<=0. {continue;}
        if light.position_kind.w<0.5 && (shadow.params.x<=0. || shadow.lights[i].params.y<0.5) {
            let phase=scattering_phase(g,dot(-light.direction_range.xyz,ray));
            constant_light+=light.color_intensity.rgb*volume.rgb*volume.a*light.color_intensity.a*phase;
        } else {
            local_mask|=1u<<i;
        }
    }
    if local_mask==0u && emitters==0u {
        let t=exp(-column_density(origin.y,origin.y+ray.y*distance,distance));
        return vec4(scene.rgb*t+(1.-t)*constant_light*display.volume_color.rgb,1.-(1.-scene.a)*t);
    }
    let steps=u32(clamp(display.light_meta.y,16.,64.));
    let linear_step=distance/f32(steps);
    let jitter=fract(52.9829189*fract(dot(floor(pixel),vec2(0.06711056,0.00583715))));
    let moving=display.light_meta.w>1.;
    var transmission = 1.;
    var scatter = vec3(0.);
    for(var j=0u;j<steps;j+=1u) {
        if transmission<0.001 {break;}
        var step=linear_step;
        var sample_distance=(f32(j)+jitter)*linear_step;
        if moving {
            // Short nearby segments resolve local lights even when a background
            // ray extends for hundreds of units. Limit chromatic noise during motion.
            let scale=linear_step/f32(steps);
            step=f32(2u*j+1u)*scale;
            sample_distance=f32(j*j)*scale+(0.5+(jitter-0.5)*0.2)*step;
        }
        let world_pos=origin+ray*sample_distance;
        let density=display.volume.x*exp(-max(world_pos.y,0.)/display.volume.y);
        let extinction=exp(-density*step);
        var illumination=constant_light;
        var remaining=local_mask;
        while remaining!=0u {
            let i=firstTrailingBit(remaining);
            remaining&=remaining-1u;
            let light = display.lights[i];
            let volume = display.volume_lights[i];
            var l = -light.direction_range.xyz;
            var attenuation = 1.;
            if light.position_kind.w>0.5 {
                let to_light=light.position_kind.xyz-world_pos;
                let distance2=max(dot(to_light,to_light),0.01);
                let range2=light.direction_range.w*light.direction_range.w;
                if range2>0. && distance2>=range2 {continue;}
                l=to_light*inverseSqrt(distance2);
                let emitter=select(0.01,max(light.cone.z*light.cone.w*0.25,0.01),light.position_kind.w>2.5);
                attenuation=1./max(distance2,emitter);
                if range2 > 0. {
                    let r = distance2 / range2;
                    let fade = 1. - r * r;
                    attenuation *= fade * fade;
                }
                if light.position_kind.w > 2.5 {
                    attenuation *= max(dot(light.direction_range.xyz, -l), 0.) * light.cone.z * light.cone.w;
                } else if light.position_kind.w > 1.5 {
                    let cone=clamp((dot(-l,light.direction_range.xyz)-light.cone.y)/max(light.cone.x-light.cone.y,0.00001),0.,1.);
                    attenuation*=cone*cone;
                }
                if attenuation<=0. {continue;}
            }
            let phase=scattering_phase(g,dot(l,ray));
            illumination += light.color_intensity.rgb * volume.rgb * volume.a * light.color_intensity.a
                * attenuation * phase * visibility(i, world_pos);
        }
        if emitters>0u {
            let index=(j+u32(jitter*f32(emitters)))%emitters;
            let sample=mesh_emission(index,world_pos,vec3(0.),true);
            illumination+=sample.radiance*scattering_phase(g,dot(sample.direction,ray))*f32(emitters);
        }
        scatter+=transmission*(1.-extinction)*illumination*display.volume_color.rgb;
        transmission*=extinction;
    }
    return vec4(scene.rgb*transmission+scatter,1.-(1.-scene.a)*transmission);
}
fn background(uv:vec2<f32>)->vec3<f32> {
    var factor=0.;
    if display.secondary.w>1.5 {factor=clamp(dot(uv-vec2(0.5),display.gradient.xy)+0.5,0.,1.);}
    if display.secondary.w>2.5 {factor=clamp(length((uv-vec2(0.5))*2.),0.,1.);}
    return mix(display.background.rgb,display.secondary.rgb,factor);
}
fn focus_distance_at(pixel:vec2<f32>)->f32 {
    let uv=(pixel-display.viewport.xy)/display.viewport.zw;
    let p=unproject(uv,min(unpack_depth(textureLoad(focus_depth,vec2<i32>(pixel),0)),0.999999));
    return max(dot(p-display.eye.xyz,display.forward.xyz),0.001);
}
fn defocus(pixel:vec2<f32>)->vec4<f32>{
    let original=textureLoad(hdr_scene,vec2<i32>(pixel),0);
    if display.dof.y<=0. {return original;}
    let z=focus_distance_at(pixel);let radius=min(24.,abs(display.dof.y*(z-display.dof.x)/z));
    if radius<0.5 {return original;}
    var sum=original;var weight=1.;
    for(var i=0u;i<32u;i+=1u) {
        let r=sqrt((f32(i)+0.5)/32.)*radius;let angle=f32(i)*2.39996323;
        let sample=pixel+vec2(cos(angle),sin(angle))*r;
        if any(sample<display.viewport.xy)||any(sample>=display.viewport.xy+display.viewport.zw){continue;}
        let depth=focus_distance_at(sample);let sample_radius=abs(display.dof.y*(depth-display.dof.x)/depth);
        // A sharp foreground silhouette must not bleed into a blurred background.
        if depth<z-max(0.02,z*0.01)&&sample_radius<r {continue;}
        let w=select(1.,0.25,depth>z+max(0.02,z*0.01));
        sum+=textureLoad(hdr_scene,vec2<i32>(sample),0)*w;weight+=w;
    }
    return sum/weight;
}
// Geometric normal from the shortest one-sided depth differences. This avoids
// bridging silhouettes when constructing a normal at an object boundary.
fn contour_pixel(p:vec2<f32>)->vec2<f32> {
    return clamp(floor(p)+vec2(0.5),display.viewport.xy+vec2(0.5),display.viewport.xy+display.viewport.zw-vec2(0.5));
}
fn contour_depth(p:vec2<f32>)->f32 {return unpack_depth(textureLoad(scene_depth,vec2<i32>(contour_pixel(p)),0));}
fn contour_world(p:vec2<f32>)->vec3<f32> {
    let pixel=contour_pixel(p);
    return unproject((pixel-display.viewport.xy)/display.viewport.zw,min(contour_depth(pixel),0.999999));
}
fn contour_normal(p:vec2<f32>)->vec3<f32> {
    let c=contour_world(p);
    let l=c-contour_world(p-vec2(1.,0.));let r=contour_world(p+vec2(1.,0.))-c;
    let d=c-contour_world(p-vec2(0.,1.));let u=contour_world(p+vec2(0.,1.))-c;
    let dx=select(l,r,dot(r,r)<dot(l,l));let dy=select(d,u,dot(u,u)<dot(d,d));
    let normal=cross(dx,dy);return normal/max(length(normal),0.00000001);
}
fn screen_contour(p:vec2<f32>)->f32 {
    if display.contour.x<=0. || any(p<display.viewport.xy) || any(p>=display.viewport.xy+display.viewport.zw) {return 0.;}
    if contour_depth(p)>=0.999999 {return 0.;}
    let center=contour_world(p);let normal=contour_normal(p);
    let z=max(abs(dot(center-display.eye.xyz,display.forward.xyz)),0.01);
    let directions=array<vec2<f32>,8>(vec2(1.,0.),vec2(-1.,0.),vec2(0.,1.),vec2(0.,-1.),
        vec2(0.7071,0.7071),vec2(-0.7071,0.7071),vec2(0.7071,-0.7071),vec2(-0.7071,-0.7071));
    var edge=0.;
    for(var i=0u;i<8u;i+=1u) {
        let q=p+directions[i]*max(display.contour.x,1.);
        if any(q<display.viewport.xy)||any(q>=display.viewport.xy+display.viewport.zw){continue;}
        if contour_depth(q)>=0.999999 {edge=1.;continue;}
        let point=contour_world(q);let delta=point-center;
        // Plane distance avoids marking a tilted, continuous plane as a depth edge.
        let jump=abs(dot(normal,delta))/z;
        let crease=dot(normal,contour_normal(q));
        edge=max(edge,smoothstep(display.contour.y,display.contour.y*1.5,jump));
        if dot(normal,normal)>0.5 {edge=max(edge,1.-smoothstep(display.contour.z-0.04,display.contour.z+0.04,crease));}
    }
    return edge*min(display.contour.x,1.);
}

// The complete selected union stays visible through foreground objects. Scene
// depth must not introduce contours along intersections with other meshes.
fn selected_outline(p:vec2<f32>)->f32 {
    if display.selection.w<=0. || any(p<display.viewport.xy) || any(p>=display.viewport.xy+display.viewport.zw) {return 0.;}
    let selected_z=unpack_depth(textureLoad(selection_depth,vec2<i32>(p),0));
    if selected_z>=0.999999 {return 0.;}
    let directions=array<vec2<f32>,8>(vec2(1.,0.),vec2(-1.,0.),vec2(0.,1.),vec2(0.,-1.),vec2(0.7071,0.7071),vec2(-0.7071,0.7071),vec2(0.7071,-0.7071),vec2(-0.7071,-0.7071));
    let dims=vec2<i32>(textureDimensions(selection_depth));
    for(var i=0u;i<8u;i+=1u) {
        let q=p+directions[i]*display.selection.w;
        if any(q<display.viewport.xy) || any(q>=display.viewport.xy+display.viewport.zw) {continue;}
        if unpack_depth(textureLoad(selection_depth,clamp(vec2<i32>(q),vec2(0),dims-vec2(1)),0))>=0.999999 {return 1.;}
    }
    return 0.;
}

// A moving preview can shade one ray per 2x2 pixels. Pick the same depth texel
// at both stages, including at odd-sized edges and in an offset editor viewport.
fn volume_pixel(cell:vec2<i32>,stride:i32)->vec2<f32> {
    let dimensions=vec2<i32>(textureDimensions(scene_depth));
    let first=clamp(vec2<i32>(ceil(display.viewport.xy-0.5)),vec2(0),dimensions-1);
    let last=clamp(vec2<i32>(ceil(display.viewport.xy+display.viewport.zw-0.5))-1,first,dimensions-1);
    return vec2<f32>(clamp(cell*stride+stride/2,first,last))+0.5;
}

@fragment fn fs_volume(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let pixel=volume_pixel(vec2<i32>(position.xy),i32(display.light_meta.w));
    let v=volume_fx(fog(vec4(0.),pixel),pixel);
    return vec4(min(v.rgb,vec3(65504.)),clamp(1.-v.a,0.,1.));
}

fn volume_at(pixel:vec2<f32>)->vec4<f32> {
    let stride=i32(display.light_meta.w);
    if stride==1 {return textureLoad(integrated_volume,vec2<i32>(pixel),0);}
    let uv=(pixel-vec2(f32(stride/2)+0.5))/f32(stride);
    let base = vec2<i32>(floor(uv));
    let f = fract(uv);
    let size=vec2<f32>(textureDimensions(scene_depth));
    let low=vec2<i32>(floor(clamp(display.viewport.xy,vec2(0.),size)/f32(stride)));
    let high=max(low,vec2<i32>(ceil(clamp(display.viewport.xy+display.viewport.zw,vec2(0.),size)/f32(stride)))-1);
    let z=unpack_depth(textureLoad(scene_depth,vec2<i32>(pixel),0));
    let tolerance=select(max(0.000001,(1.-z)*0.025),0.0002,display.light_meta.z>0.5);
    var sum = vec4(0.);
    var weights = 0.;
    for(var i=0u;i<4u;i+=1u) {
        let offset=vec2<i32>(i32(i&1u),i32(i>>1u));
        let cell=clamp(base+offset,low,high);
        let sample_z=unpack_depth(textureLoad(scene_depth,vec2<i32>(volume_pixel(cell,stride)),0));
        let error=(z-sample_z)/tolerance;
        let xy=select(vec2(1.)-f,f,offset!=vec2(0));
        let weight=xy.x*xy.y/(1.+error*error);
        sum+=textureLoad(integrated_volume,cell,0)*weight;
        weights+=weight;
    }
    return sum/max(weights,1e-20);
}

fn scene_at(pixel:vec2<f32>)->vec4<f32> {
    let scene=defocus(pixel);
    if display.light_meta.w < 1. || any(pixel < display.viewport.xy)
        || any(pixel >= display.viewport.xy + display.viewport.zw) {
        return scene;
    }
    let v=volume_at(pixel);
    return vec4(scene.rgb*v.a+v.rgb,1.-(1.-scene.a)*v.a);
}

@fragment fn fs_main(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    // Resolve and all alpha blending have already happened in a linear HDR buffer.
    // Remove coverage before the nonlinear display curve and restore it afterwards:
    // this keeps MSAA edges and transparent PNG boundaries free from dark fringes.
    var scene = scene_at(pixel.xy);
    let effects = display.camera_lens.w > 0.5 && all(pixel.xy >= display.viewport.xy)
        && all(pixel.xy < display.viewport.xy + display.viewport.zw);
    if effects && display.camera_lens.z > 0. {
        let uv = (pixel.xy - display.viewport.xy) / display.viewport.zw;
        let radial = (uv - vec2(0.5)) * 2.;
        let shift = radial * dot(radial, radial) * display.camera_lens.z * display.viewport.w / 1080.;
        let low = display.viewport.xy + vec2(0.5); let high = display.viewport.xy + display.viewport.zw - vec2(0.5);
        let rp = clamp(pixel.xy + shift, low, high); let bp = clamp(pixel.xy - shift, low, high);
        let red = scene_at(rp); let blue = scene_at(bp);
        // Keep the original coverage so lens effects cannot create opaque PNG fringes.
        scene.r = red.r / max(red.a, 0.000001) * scene.a;
        scene.b = blue.b / max(blue.a, 0.000001) * scene.a;
    }
    let coverage = clamp(scene.a, 0., 1.);
    let straight_hdr = scene.rgb / max(coverage, 0.000001);
    let camera_exposure = select(1., exp2(display.camera_grade.x), effects);
    let mapped = mix(tone_map(straight_hdr * display.params.x * camera_exposure),display.contour_color.rgb,screen_contour(pixel.xy));
    var rgb = mapped * coverage;
    var alpha = coverage;
    if display.params.z < 0.5 || display.params.z > 1.5 {
        // Display-only backgrounds are composed after exposure and tone mapping.
        var backdrop = background((pixel.xy-display.viewport.xy)/display.viewport.zw);
        let inside = all(pixel.xy >= display.viewport.xy) &&
            all(pixel.xy < display.viewport.xy + display.viewport.zw);
        if display.params.z<0.5 && inside && world.environment.w>0.5 {
            let uv=(pixel.xy-display.viewport.xy)/display.viewport.zw;
            let far=display.inverse_view_proj*vec4(uv.x*2.-1.,1.-uv.y*2.,1.,1.);
            let near=display.inverse_view_proj*vec4(uv.x*2.-1.,1.-uv.y*2.,0.,1.);
            backdrop=tone_map(world_environment(normalize(far.xyz/far.w-near.xyz/near.w),0.)*display.params.x*camera_exposure);
        }
        if display.params.z > 1.5 && inside {
            let cell = vec2<i32>(floor((pixel.xy - display.viewport.xy) / max(display.checker.x, 1.)));
            let level = select(display.checker.y, display.checker.z, (cell.x + cell.y) % 2 == 1);
            backdrop = vec3(level);
        }
        rgb += backdrop * (1. - coverage);
        alpha = 1.;
    }
    if effects {
        var color = rgb / max(alpha, 0.000001);
        let temperature = display.camera_grade.w;
        color *= vec3(1. + temperature * 0.18, 1., 1. - temperature * 0.18);
        let luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
        color = mix(vec3(luma), color, display.camera_grade.z);
        color = (color - vec3(0.18)) * display.camera_grade.y + vec3(0.18);
        let uv = (pixel.xy - display.viewport.xy) / display.viewport.zw;
        let radius = length((uv - vec2(0.5)) * 1.41421356);
        color *= 1. - display.camera_lens.x * smoothstep(0.2, 1., radius);
        // A stable sensor pattern avoids animation flicker and remains reproducible on export.
        let seed = floor((pixel.xy - display.viewport.xy) * 1080. / max(display.viewport.w, 1.));
        let noise = fract(sin(dot(seed, vec2(12.9898, 78.233))) * 43758.5453) - 0.5;
        color += vec3(noise * display.camera_lens.y * (0.25 + sqrt(max(luma, 0.))));
        let effect_mask = select(1., coverage, display.params.z > 1.5);
        rgb = mix(rgb, clamp(color, vec3(0.), vec3(1.)) * alpha, effect_mask);
    }
    rgb = mix(rgb,display.selection.rgb,selected_outline(pixel.xy));
    if display.params.w > 0.5 { rgb = linear_to_srgb(clamp(rgb, vec3(0.), vec3(1.))); }
    return vec4(rgb, alpha);
}
