@group(0) @binding(0) var source:texture_2d<f32>;
@group(0) @binding(1) var filtering:sampler;
@group(0) @binding(2) var result:texture_storage_2d_array<rgba16float,write>;
fn panorama(direction:vec3<f32>)->vec2<f32>{return vec2(atan2(direction.z,direction.x)/6.2831853+0.5,acos(clamp(direction.y,-1.,1.))/3.14159265);}
fn radical_inverse(i:u32)->f32{return f32(reverseBits(i))*2.3283064365386963e-10;}
@compute @workgroup_size(8,8,1) fn main(@builtin(global_invocation_id) id:vec3<u32>){
    if id.x>=256u||id.y>=128u||id.z>=8u{return;}
    let uv=(vec2<f32>(id.xy)+vec2(0.5))/vec2(256.,128.);let theta=uv.y*3.14159265;let phi=(uv.x-0.5)*6.2831853;
    let n=vec3(sin(theta)*cos(phi),cos(theta),sin(theta)*sin(phi));
    if id.z==0u{textureStore(result,vec2<i32>(id.xy),i32(id.z),vec4(textureSampleLevel(source,filtering,uv,0.).rgb,1.));return;}
    let rough=f32(id.z)/7.;let a=rough*rough;let a2=a*a;
    let helper=select(vec3(0.,1.,0.),vec3(0.,0.,1.),abs(n.y)>0.95);let t=normalize(cross(helper,n));let b=cross(n,t);
    var color=vec3(0.);var weight=0.;
    for(var i=0u;i<64u;i+=1u){
        let u=vec2((f32(i)+0.5)/64.,radical_inverse(i));let angle=u.x*6.2831853;
        let cos_theta=sqrt((1.-u.y)/(1.+(a2-1.)*u.y));let sin_theta=sqrt(max(1.-cos_theta*cos_theta,0.));
        let h=t*(cos(angle)*sin_theta)+b*(sin(angle)*sin_theta)+n*cos_theta;let l=reflect(-n,h);let nl=max(dot(n,l),0.);
        if nl>0.{color+=textureSampleLevel(source,filtering,panorama(l),0.).rgb*nl;weight+=nl;}
    }
    textureStore(result,vec2<i32>(id.xy),i32(id.z),vec4(min(color/max(weight,0.00001),vec3(65000.)),1.));
}
