struct Camera {view_proj:mat4x4<f32>,right:vec4<f32>,up:vec4<f32>};
@group(0) @binding(0) var<uniform> camera:Camera;
struct In {@location(0) center:vec3<f32>};
struct Out {@builtin(position) clip:vec4<f32>,@location(0) uv:vec2<f32>};
@vertex fn vs_main(v:In,@builtin(vertex_index) i:u32)->Out {
    let offsets=array<vec2<f32>,6>(vec2(-1.,-1.),vec2(1.,-1.),vec2(1.,1.),vec2(-1.,-1.),vec2(1.,1.),vec2(-1.,1.));
    let uv=offsets[i];
    let p=v.center+(camera.right.xyz*uv.x+camera.up.xyz*uv.y)*0.034;
    var o:Out;o.clip=camera.view_proj*vec4(p,1.);o.uv=uv;return o;
}
@fragment fn fs_main(v:Out)->@location(0) vec4<f32> {
    let r=dot(v.uv,v.uv);if r>1. {discard;}
    let n=vec3(v.uv,sqrt(1.-r));
    let light=max(dot(n,normalize(vec3(-0.5,0.8,1.))),0.);
    return vec4(vec3(0.04,0.32,0.58)*(0.4+light)+vec3(pow(max(n.z,0.),48.))*0.4,1.);
}
