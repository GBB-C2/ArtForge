struct GraphNode { op_inputs:vec4<i32>,value:vec4<f32>,secondary:vec4<f32>,image_info:vec4<i32> }
struct Graph { info:vec4<u32>,outputs:array<vec4<i32>,2>,nodes:array<GraphNode,32> }
struct GraphResult { color:vec4<f32>,properties:vec4<f32>,emission:vec4<f32>,surface:vec4<f32>,normal:vec4<f32> }
struct NodeSamples { v:vec4<f32>,x:vec4<f32>,y:vec4<f32> }
fn graph_hash(p:vec2<f32>)->f32 { return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
fn graph_noise(uv:vec2<f32>)->f32 {
    let cell=floor(uv);let f=fract(uv);let t=f*f*(3.-2.*f);
    return mix(mix(graph_hash(cell),graph_hash(cell+vec2(1.,0.)),t.x),mix(graph_hash(cell+vec2(0.,1.)),graph_hash(cell+vec2(1.)),t.x),t.y);
}
fn graph_normal(v:vec3<f32>)->vec3<f32> { if dot(v,v)<0.000001{return vec3(0.,0.,1.);}return normalize(v); }
fn graph_image(uv:vec2<f32>,layer:i32,data:i32)->vec4<f32> {
    let raw=textureSampleLevel(graph_images,environment_sampler,fract(uv),layer,0.);
    if data!=0 {return raw;}
    return vec4(select(pow((raw.rgb+0.055)/1.055,vec3(2.4)),raw.rgb/12.92,raw.rgb<=vec3(0.04045)),raw.a);
}
fn graph_value(node:GraphNode,uv:vec2<f32>,texture:vec4<f32>,a:vec4<f32>,b:vec4<f32>,t:vec4<f32>)->vec4<f32> {
    switch node.op_inputs.x {
        case 1:{return vec4(node.value.x);}
        case 2:{if node.image_info.x>0 {return graph_image(uv,node.image_info.x,node.image_info.y);}return texture;}
        case 3:{let p=floor(uv*node.value.x);return select(a,b,(i32(p.x)+i32(p.y))%2!=0);}
        case 4:{return vec4(vec3(graph_noise(uv*node.value.x)),1.);}
        case 5:{return mix(a,b,clamp(t.x,0.,1.));}
        case 6:{return a*b;}
        case 7:{return mix(node.value,node.secondary,clamp(a.x,0.,1.));}
        case 9:{let n=graph_normal(a.xyz*2.-vec3(1.));return vec4(graph_normal(vec3(n.xy*node.value.x,n.z)),1.);}
        case 10:{return vec4(uv*node.value.xy+node.value.zw,0.,1.);}
        default:{return node.value;}
    }
}
fn graph_evaluate(graph:Graph,uv:vec2<f32>,texture:vec4<f32>,base:vec4<f32>,properties:vec4<f32>,emission:vec4<f32>,surface:vec4<f32>)->GraphResult {
    var values:array<NodeSamples,32>;
    let epsilon=0.001;
    for(var i=0u;i<graph.info.x;i+=1u){
        let node=graph.nodes[i];var av=vec4(0.);var bv=vec4(1.);var tv=vec4(node.value.x);
        if node.op_inputs.x==3 {av=vec4(1.);bv=vec4(0.,0.,0.,1.);}
        if node.op_inputs.x==6 {av=vec4(1.);bv=vec4(node.value.x);}
        if node.op_inputs.x==9 {av=vec4(0.5,0.5,1.,1.);}
        var a=NodeSamples(av,av,av);var b=NodeSamples(bv,bv,bv);var t=NodeSamples(tv,tv,tv);
        if node.op_inputs.y>=0 {a=values[u32(node.op_inputs.y)];}
        if node.op_inputs.z>=0 {b=values[u32(node.op_inputs.z)];}
        if node.op_inputs.w>=0 {t=values[u32(node.op_inputs.w)];}
        var coord=NodeSamples(vec4(uv,0.,1.),vec4(uv+vec2(epsilon,0.),0.,1.),vec4(uv+vec2(0.,epsilon),0.,1.));
        if (node.op_inputs.x==2 || node.op_inputs.x==4) && node.op_inputs.y>=0 {coord=a;}
        if node.op_inputs.x==3 && node.op_inputs.w>=0 {coord=t;}
        var result=NodeSamples(graph_value(node,coord.v.xy,texture,a.v,b.v,t.v),graph_value(node,coord.x.xy,texture,a.x,b.x,t.x),graph_value(node,coord.y.xy,texture,a.y,b.y,t.y));
        if node.op_inputs.x==8 {
            let normal=vec4(graph_normal(vec3(-(a.x.x-a.v.x)*node.value.x/epsilon,-(a.y.x-a.v.x)*node.value.x/epsilon,1.)),1.);
            result=NodeSamples(normal,normal,normal);
        }
        values[i]=result;
    }
    var out=GraphResult(base,properties,emission,surface,vec4(0.,0.,1.,0.));
    let p=graph.outputs[0];let extra=graph.outputs[1];
    if p.x>=0 {out.color=clamp(values[u32(p.x)].v,vec4(0.),vec4(1.));}
    if p.y>=0 {out.properties.x=clamp(values[u32(p.y)].v.x,0.,1.);}
    if p.z>=0 {out.properties.y=clamp(values[u32(p.z)].v.x,0.045,1.);}
    if p.w>=0 {out.emission=clamp(values[u32(p.w)].v,vec4(0.),vec4(10000.));}
    if extra.x>=0 {out.color.a=clamp(values[u32(extra.x)].v.x,0.,1.);}
    if extra.y>=0 {out.surface.w=clamp(values[u32(extra.y)].v.x,0.,1.);}
    if extra.z>=0 {out.surface.x=clamp(values[u32(extra.z)].v.x,1.,3.);}
    if extra.w>=0 {out.normal=vec4(graph_normal(values[u32(extra.w)].v.xyz),1.);}
    return out;
}
