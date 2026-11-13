// Bit-preserving depth in RGBA8 avoids raw depth loads and optional float render targets.
fn unpack_depth(color:vec4<f32>)->f32 {
    let b=vec4<u32>(round(color*255.));
    return bitcast<f32>(b.x | (b.y<<8u) | (b.z<<16u) | (b.w<<24u));
}
@group(0) @binding(0) var scene_depth:texture_2d<f32>;
@group(0) @binding(1) var interface_image:texture_2d<f32>;
@group(0) @binding(2) var image_sampler:sampler;
struct In { @location(0) rect:vec4<f32>, @location(1) basis:vec4<f32>, @location(2) color:vec4<f32>, @location(3) clip:vec4<f32>, @location(4) depth:vec4<f32>, @location(5) image:vec4<f32> };
struct Out { @builtin(position) position:vec4<f32>, @location(0) color:vec4<f32>, @location(1) local:vec2<f32>, @location(2) clip:vec4<f32>, @location(3) depth:vec2<f32>, @location(4) image:vec3<f32> };
@vertex fn vs_main(v:In,@builtin(vertex_index) index:u32)->Out {
    let corners=array<vec2<f32>,6>(vec2(0.,0.),vec2(1.,0.),vec2(1.,1.),vec2(0.,0.),vec2(1.,1.),vec2(0.,1.));
    var uv=corners[index];
    if v.basis.w>0.5 {
        let triangle=array<vec2<f32>,6>(vec2(0.,0.),vec2(1.,0.),vec2(0.,1.),vec2(0.,0.),vec2(0.,1.),vec2(0.,1.));
        uv=triangle[index];
    }
    var o:Out;o.position=vec4(v.rect.xy+uv.x*v.rect.zw+uv.y*v.basis.xy,0.,1.);
    o.color=v.color;o.local=vec2(uv.y*v.basis.z,v.basis.z);o.clip=v.clip;
    var depth=mix(v.depth.x,v.depth.y,uv.x);
    if v.basis.w>0.5 { depth=v.depth.x+uv.x*(v.depth.y-v.depth.x)+uv.y*(v.depth.w-v.depth.x); }
    o.depth=vec2(depth,v.depth.z);
    o.image=vec3(v.image.xy+uv*v.image.zw,select(0.,1.,v.image.z>0. && v.image.w>0.));return o;
}
@fragment fn fs_main(v:Out)->@location(0) vec4<f32> {
    // Screen-space depth is linear along each projected edge. The small slope
    // bias keeps the visible stroke on its surface, without revealing rear edges.
    let bias=max(0.000001,2.*(abs(dpdx(v.depth.x))+abs(dpdy(v.depth.x))));
    if v.depth.y>0.5 && v.depth.x>unpack_depth(textureLoad(scene_depth,vec2<i32>(v.position.xy),0))+bias { discard; }
    let c=v.color.rgb;
    let linear=select(pow((c+0.055)/1.055,vec3(2.4)),c/12.92,c<=vec3(0.04045));
    if any(v.position.xy<v.clip.xy) || any(v.position.xy>=v.clip.zw) { discard; }
    if v.image.z>0.5 {let sample=textureSampleLevel(interface_image,image_sampler,v.image.xy,0.);return vec4(sample.rgb*linear,sample.a*v.color.a);}
    var coverage=1.;
    if v.local.y>0. { coverage=clamp((v.local.y-2.)*0.5+0.5-abs(v.local.x-v.local.y*0.5),0.,1.); }
    return vec4(linear,v.color.a*coverage);
}
