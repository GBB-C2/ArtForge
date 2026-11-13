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
    shape: vec4<f32>, // area rotation cosine/sine
};
struct Draw {
    model: mat4x4<f32>,
    normal_model: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    base_color: vec4<f32>,
    params: vec4<f32>, // metallic, roughness, alpha cutoff (-1 opaque/-2 blend), grid
    emissive: vec4<f32>,
    maps: vec4<f32>, // normal scale, occlusion strength, has normal map, invert normal Y
    surface: vec4<f32>, // dielectric IOR, coat weight, coat roughness, reserved
    environment: vec4<f32>, // exposure multiplier, ambient strength, light count, tone mapping
    sky: vec4<f32>,
    ground: vec4<f32>,
    lights: array<Light, 8>,
    transmission: vec4<f32>, // thickness, attenuation distance
    viewport: vec4<f32>,
    background: vec4<f32>,
    secondary: vec4<f32>, // second color, background mode
    gradient: vec4<f32>, // background direction, transparency, MSAA guard // direction, transparent background
    anisotropy: vec4<f32>, // strength, cosine/sine of UV-space rotation
    fiber: vec4<f32>, // sheen weight, perceptual roughness
    absorption: vec4<f32>, // linear absorption color, override flag
    uv_transform: vec4<f32>, // scale xy, offset zw
    uv_rotation: vec4<f32>, // cosine, sine
    map_uvs: array<vec4<f32>, 10>, // two affine rows per material map
    ambient_occlusion: vec4<f32>, // strength, world radius
    inverse_view_proj: mat4x4<f32>,
    graph: Graph,
    npr: Npr,
    artistic: Artistic,
};
@group(0) @binding(0) var<uniform> draw: Draw;
@group(0) @binding(1) var<storage, read> joints: array<mat4x4<f32>>;
@group(1) @binding(0) var base_texture: texture_2d<f32>;
@group(1) @binding(1) var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(2) var normal_texture: texture_2d<f32>;
@group(1) @binding(3) var occlusion_texture: texture_2d<f32>;
@group(1) @binding(4) var emissive_texture: texture_2d<f32>;
@group(1) @binding(5) var material_sampler: sampler;
@group(1) @binding(7) var base_sampler: sampler;
@group(1) @binding(8) var mr_sampler: sampler;
@group(1) @binding(9) var normal_sampler: sampler;
@group(1) @binding(10) var ao_sampler: sampler;
@group(1) @binding(11) var emission_sampler: sampler;
@group(1) @binding(6) var sheen_albedo: texture_2d<f32>;
struct ShadowLight {
    view_proj: array<mat4x4<f32>, 6>,
    params: vec4<f32>, // first array layer, face count, near, far
};
struct Shadow { lights: array<ShadowLight, 8>, params: vec4<f32> };
struct ShadowCamera { view_proj: mat4x4<f32> };
@group(2) @binding(0) var<uniform> shadow: Shadow;
@group(2) @binding(1) var shadow_map: texture_depth_2d_array;
@group(2) @binding(2) var shadow_sampler: sampler_comparison;
@group(2) @binding(3) var<uniform> shadow_camera: ShadowCamera;

@group(3) @binding(0) var opaque_scene: texture_2d<f32>;
@group(3) @binding(1) var opaque_depth: texture_2d<f32>;

struct VIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) joints: vec4<u32>,
    @location(4) weights: vec4<f32>,
    @location(5) color: vec4<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(4) rest: vec3<f32>,
};
fn safe_normalize(value: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let length_squared = dot(value, value);
    if length_squared < 0.00000001 { return fallback; }
    return value * inverseSqrt(length_squared);
}
fn deform(v: VIn) -> VOut {
    var skin = mat4x4<f32>(vec4(1.,0.,0.,0.),vec4(0.,1.,0.,0.),vec4(0.,0.,1.,0.),vec4(0.,0.,0.,1.));
    let weight_sum = dot(v.weights, vec4(1.));
    if draw.maps.w > 0.5 && weight_sum > 0.0001 {
        // Zero-weight slots may contain arbitrary indices in older projects.
        // Do not read them, especially now that palettes use their exact size.
        skin = mat4x4<f32>(vec4(0.), vec4(0.), vec4(0.), vec4(0.));
        for (var i=0u; i<4u; i++) {
            if v.weights[i] > 0. {
                skin += joints[v.joints[i]] * v.weights[i];
            }
        }
    }
    let world = draw.model * skin * vec4(v.position, 1.);
    var out: VOut;
    out.clip = draw.view_proj * world;
    out.world = world.xyz;
    out.rest = v.position;
    out.color = v.color;
    // Inverse-transpose of the blended skin's linear part, including nonuniform scale.
    let a=skin[0].xyz; let b=skin[1].xyz; let c=skin[2].xyz;
    let cofactor=mat3x3<f32>(cross(b,c),cross(c,a),cross(a,b));
    let determinant=dot(a,cross(b,c));
    var skin_normal=v.normal;
    if abs(determinant)>0.000001 { skin_normal=(cofactor*v.normal)/determinant; }
    out.normal = safe_normalize((draw.normal_model * vec4(skin_normal,0.)).xyz, vec3(0.,1.,0.));
    let uv = v.uv * draw.uv_transform.xy;
    out.uv = vec2(uv.x * draw.uv_rotation.x - uv.y * draw.uv_rotation.y,
        uv.x * draw.uv_rotation.y + uv.y * draw.uv_rotation.x) + draw.uv_transform.zw;
    return out;
}
@vertex fn vs_main(v: VIn) -> VOut { return deform(v); }
@vertex fn vs_shadow(v: VIn) -> VOut {
    var out = deform(v);
    out.clip = shadow_camera.view_proj * vec4(out.world, 1.);
    return out;
}
fn texture_uv(uv: vec2<f32>, slot: u32) -> vec2<f32> {
    let p = vec3(uv, 1.);
    return vec2(dot(draw.map_uvs[slot * 2u].xyz, p), dot(draw.map_uvs[slot * 2u + 1u].xyz, p));
}
@fragment fn fs_depth(v: VOut) -> @location(0) vec4<f32> {
    let map=textureSample(base_texture,base_sampler,texture_uv(v.uv, 0u));
    let alpha=graph_evaluate(draw.graph,v.uv,map,map*draw.base_color*v.color,draw.params,draw.emissive,draw.surface).color.a;
    if draw.params.z >= 0. && alpha < draw.params.z { discard; }
    let b=bitcast<u32>(v.clip.z);
    return vec4<f32>(f32(b&255u), f32((b>>8u)&255u), f32((b>>16u)&255u), f32(b>>24u))/255.;
}
// Deterministic stochastic coverage makes PCF average partial transmission.
// This is scalar visibility, not colored caustics or multiple scattering.
@fragment fn fs_shadow(v: VOut) {
    let map=textureSample(base_texture,base_sampler,texture_uv(v.uv, 0u));
    let graph_surface=graph_evaluate(draw.graph,v.uv,map,map*draw.base_color*v.color,draw.params,draw.emissive,draw.surface);let sampled=graph_surface.color;
    if draw.params.z >= 0. && sampled.a < draw.params.z { discard; }
    let absorption_color=select(sampled.rgb,draw.absorption.rgb,draw.absorption.w>0.5);
    let tint=pow(clamp(absorption_color,vec3(0.001),vec3(1.)),vec3(draw.transmission.x/max(draw.transmission.y,0.001)));
    let f0=dielectric_f0(graph_surface.surface.x);
    let transmitted=select(graph_surface.surface.w,0.,draw.npr.ramp.x>0.5)*(1.-draw.params.x)*(1.-f0)*dot(tint,vec3(1./3.));
    let coverage=(1.-transmitted)*select(1.,sampled.a,draw.params.z == -2.);
    let cell=vec2<u32>(v.clip.xy);
    var hash=cell.x*1597334677u ^ cell.y*3812015801u;
    hash=(hash^(hash>>16u))*2246822519u;hash=hash^(hash>>13u);
    if f32(hash & 65535u)/65536. >= coverage {discard;}
}
fn cube_face(direction: vec3<f32>) -> u32 {
    let a = abs(direction);
    if a.x >= a.y && a.x >= a.z { return select(1u, 0u, direction.x >= 0.); }
    if a.y >= a.z { return select(3u, 2u, direction.y >= 0.); }
    return select(5u, 4u, direction.z >= 0.);
}
// Inverse of the six RH look-at cameras. Remapping filter taps across faces
// avoids clamping a point-light kernel to an unrelated edge depth.
fn cube_direction(face: u32, p: vec2<f32>) -> vec3<f32> {
    switch face {
        case 0u: { return vec3(1., -p.y, -p.x); }
        case 1u: { return vec3(-1., -p.y, p.x); }
        case 2u: { return vec3(p.x, 1., p.y); }
        case 3u: { return vec3(p.x, -1., -p.y); }
        case 4u: { return vec3(p.x, -p.y, 1.); }
        default: { return vec3(-p.x, -p.y, -1.); }
    }
}
fn shadow_uv(clip: vec4<f32>) -> vec2<f32> {
    let p = clip.xy / clip.w;
    return vec2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
}
fn shadow_depth_gradient(matrix: mat4x4<f32>, clip: vec4<f32>,
        world_dx: vec3<f32>, world_dy: vec3<f32>) -> vec2<f32> {
    // The depth of the geometric receiver plane is linear in shadow-map UV,
    // even under perspective projection. Differentiate that projection rather
    // than using a normal map, which does not describe the triangle's plane.
    let dx = matrix * vec4(world_dx, 0.);
    let dy = matrix * vec4(world_dy, 0.);
    let projected = clip.xyz / clip.w;
    let px = (dx.xyz - projected * dx.w) / clip.w;
    let py = (dy.xyz - projected * dy.w) / clip.w;
    let ux = vec2(px.x * 0.5, -px.y * 0.5);
    let uy = vec2(py.x * 0.5, -py.y * 0.5);
    let determinant = ux.x * uy.y - ux.y * uy.x;
    let scale = max(length(ux) * length(uy), 0.00000000000000000001);
    if abs(determinant) <= scale * 0.00001 { return vec2(0.); }
    return vec2(px.z * uy.y - py.z * ux.y, py.z * ux.x - px.z * uy.x) / determinant;
}
fn shadow_compare(index: u32, face: u32, clip: vec4<f32>, uv: vec2<f32>,
        nl: f32, depth_gradient: vec2<f32>) -> f32 {
    if clip.w <= 0. { return 1.; }
    let depth = clip.z / clip.w + dot(uv - shadow_uv(clip), depth_gradient);
    if depth <= 0. || depth >= 1. || any(uv < vec2(0.)) || any(uv > vec2(1.)) { return 1.; }
    let info = shadow.lights[index].params;
    var bias = shadow.params.y * (1. + 2. * (1. - nl));
    if info.z > 0. {
        // Perspective depth is nonlinear. Convert a relative world-space
        // receiver offset through dz/dd instead of using the sun's depth bias.
        bias *= (info.z * info.w / max(info.w - info.z, 0.0000001)) / max(clip.w, info.z);
    }
    // Each bilinear comparison includes adjacent texel centers. Account for
    // their depth variation as well as the explicit PCF tap displacement.
    bias += dot(abs(depth_gradient), vec2(shadow.params.z));
    return textureSampleCompareLevel(shadow_map, shadow_sampler, uv,
        i32(info.x) + i32(face), depth - bias);
}
fn light_visibility(index: u32, world: vec3<f32>, n: vec3<f32>, l: vec3<f32>,
        world_dx: vec3<f32>, world_dy: vec3<f32>) -> f32 {
    let info = shadow.lights[index].params;
    if shadow.params.x <= 0. || info.y < 0.5 { return 1.; }
    let point_faces = info.y > 1.5;
    let delta = world - draw.lights[index].position_kind.xyz;
    var face = 0u;
    if point_faces { face = cube_face(delta); }
    let clip = shadow.lights[index].view_proj[face] * vec4(world, 1.);
    if clip.w <= 0. { return 1.; }
    let uv = shadow_uv(clip);
    let gradient = shadow_depth_gradient(shadow.lights[index].view_proj[face], clip, world_dx, world_dy);
    let nl = max(dot(n, l), 0.);
    if shadow.params.w <= 0. {
        return mix(1., shadow_compare(index, face, clip, uv, nl, gradient), shadow.params.x);
    }
    var visibility = 0.;
    let footprint = shadow.params.z * shadow.params.w * 0.5;
    for (var y = -2; y <= 2; y += 1) {
        for (var x = -2; x <= 2; x += 1) {
            // A separable tent reduces the regular 3x3 box pattern without
            // widening the authored filter radius. Weights sum to 9 * 9.
            let weight = f32((3 - abs(x)) * (3 - abs(y)));
            let tap_uv = uv + vec2(f32(x), f32(y)) * footprint;
            if point_faces {
                let ray = normalize(cube_direction(face, vec2(tap_uv.x * 2. - 1., 1. - tap_uv.y * 2.)));
                let tap_face = cube_face(ray);
                if tap_face == face {
                    visibility += weight * shadow_compare(index, face, clip, tap_uv, nl, gradient);
                } else {
                    let matrix = shadow.lights[index].view_proj[tap_face];
                    let tap_world = draw.lights[index].position_kind.xyz + ray * length(delta);
                    let tap_clip = matrix * vec4(tap_world, 1.);
                    let receiver_clip = matrix * vec4(world, 1.);
                    let tap_gradient = shadow_depth_gradient(matrix, receiver_clip, world_dx, world_dy);
                    // The remapped UV follows the cube ray; its comparison depth
                    // follows the receiver plane, not a sphere around the lamp.
                    visibility += weight * shadow_compare(index, tap_face, receiver_clip, shadow_uv(tap_clip), nl, tap_gradient);
                }
            } else {
                visibility += weight * shadow_compare(index, face, clip, tap_uv, nl, gradient);
            }
        }
    }
    return mix(1., visibility / 81., shadow.params.x);
}
fn schlick(f0: vec3<f32>, cosine: f32) -> vec3<f32> {
    return f0 + (vec3(1.) - f0) * pow(1. - clamp(cosine, 0., 1.), 5.);
}
fn dielectric_f0(ior: f32) -> f32 {
    let r = (ior - 1.) / (ior + 1.);
    return r * r;
}
fn ggx_d(roughness: f32, nh: f32) -> f32 {
    let alpha = roughness * roughness;
    let a2 = alpha * alpha;
    let denominator = nh * nh * (a2 - 1.) + 1.;
    return a2 / (3.14159265 * max(denominator * denominator, 0.000000000001));
}
fn smith_ggx_visibility(roughness: f32, nv: f32, nl: f32) -> f32 {
    let a2 = pow(roughness, 4.);
    // Height-correlated Smith visibility already includes 1/(4*Nv*Nl).
    let gv = nl * sqrt(nv * nv * (1. - a2) + a2);
    let gl = nv * sqrt(nl * nl * (1. - a2) + a2);
    return 0.5 / max(gv + gl, 0.000001);
}
fn rough_diffuse(roughness: f32, nv: f32, nl: f32, vh: f32) -> f32 {
    // Energy-normalized Disney/Burley rough diffuse. Rough surfaces have a broad
    // retroreflection response instead of the previous roughness-independent Lambert term.
    let bias = 0.5 * roughness;
    let fd90 = bias + 2. * vh * vh * roughness;
    let light_scatter = 1. + (fd90 - 1.) * pow(1. - nl, 5.);
    let view_scatter = 1. + (fd90 - 1.) * pow(1. - nv, 5.);
    return light_scatter * view_scatter * mix(1., 1. / 1.51, roughness);
}
// Return the raw and normalized directional albedo of Charlie/Neubelt.
// Both 16-bit values are bilinearly decoded from RGBA8, avoiding optional
// float-texture filtering features and per-frame integration.
fn sheen_energy(nv: f32) -> vec2<f32> {
    let uv=(clamp(vec2(nv,draw.fiber.y),vec2(0.),vec2(1.))*63.+vec2(0.5))/64.;
    let e=textureSampleLevel(sheen_albedo,material_sampler,uv,0.);
    return vec2((e.r*256.+e.g)*8., e.b*256.+e.a)/257.;
}
fn anisotropic_specular(r: f32, n: vec3<f32>, t: vec3<f32>, b: vec3<f32>,
        v: vec3<f32>, l: vec3<f32>, h: vec3<f32>) -> f32 {
    let aspect=sqrt(1.-0.9*draw.anisotropy.x);
    let ax=max(r*r/aspect,0.002);
    let ay=max(r*r*aspect,0.002);
    let nh=max(dot(n,h),0.);
    let q=vec3(dot(t,h)/ax,dot(b,h)/ay,nh);
    let d=1./max(3.14159265*ax*ay*dot(q,q)*dot(q,q),0.000000000001);
    let nv=max(dot(n,v),0.0001); let nl=max(dot(n,l),0.);
    let a=nl*length(vec3(ax*dot(t,v),ay*dot(b,v),nv));
    let c=nv*length(vec3(ax*dot(t,l),ay*dot(b,l),nl));
    return d*0.5/max(a+c,0.000001);
}
fn brdf(base: vec3<f32>, metallic: f32, roughness: f32, coat_roughness: f32,
        n: vec3<f32>, eye: vec3<f32>, l: vec3<f32>, t: vec3<f32>, b: vec3<f32>,optics:vec4<f32>) -> vec3<f32> {
    let half_v = safe_normalize(l + eye, n);
    let nl = max(dot(n, l), 0.);
    let nv = max(dot(n, eye), 0.0001);
    let nh = max(dot(n, half_v), 0.);
    let vh = max(dot(eye, half_v), 0.);
    let f0 = mix(vec3(dielectric_f0(optics.x)), base, metallic);
    let fresnel = schlick(f0, vh);
    var specular = ggx_d(roughness, nh) * smith_ggx_visibility(roughness, nv, nl) * fresnel;
    if draw.anisotropy.x>0.00001 {
        specular=anisotropic_specular(roughness,n,t,b,eye,l,half_v)*fresnel;
    }
    let diffuse = (vec3(1.) - fresnel) * (1. - metallic) * (1.-optics.w) * base
        * rough_diffuse(roughness, nv, nl, vh) / 3.14159265;
    var layered=diffuse+specular;
    let sheen_weight=draw.fiber.x*(1.-metallic)*(1.-optics.w);
    if sheen_weight>0.00001 {
        let ev=sheen_energy(nv); let el=sheen_energy(nl);
        let color=sqrt(max(base,vec3(0.)))*sheen_weight;
        let maximum=max(color.r,max(color.g,color.b));
        let inverse_alpha=1./max(draw.fiber.y*draw.fiber.y,0.0049);
        let distribution=(2.+inverse_alpha)*pow(max(1.-nh*nh,0.),0.5*inverse_alpha)/(2.*3.14159265);
        let visibility=1./max(4.*(nv+nl-nv*nl),0.000001);
        // Reciprocal normalization prevents a grazing-energy spike from the
        // approximate cloth visibility. The underlying layer loses that energy.
        let cloth=distribution*visibility/max(1.,max(ev.x,el.x));
        layered=layered*max(1.-maximum*max(ev.y,el.y),0.)+color*cloth;
    }
    // A dielectric coating has its own microfacet lobe. Its interface attenuates
    // both the incoming and outgoing base layer rather than simply adding energy.
    let coat_v = schlick(vec3(0.04), nv).x * draw.surface.y;
    let coat_l = schlick(vec3(0.04), nl).x * draw.surface.y;
    let coat = ggx_d(coat_roughness, nh) * smith_ggx_visibility(coat_roughness, nv, nl)
        * schlick(vec3(0.04), vh) * draw.surface.y;
    let response=(layered*(1.-coat_v)*(1.-coat_l)+coat)*nl;
    let soft=art_softness(draw.artistic);
    if soft<=0. { return response; }
    let wrapped=max(dot(n,l)+0.5,0.)/2.25;
    return response + diffuse*soft*(wrapped-nl)*(1.-coat_v)*(1.-coat_l);
}
// Analytic fit to the integrated GGX Fresnel/visibility terms. The environment
// remains an authored sky/ground gradient, not traced scene geometry or an HDR map.
fn environment_brdf(roughness: f32, nv: f32) -> vec2<f32> {
    let r = vec4(-1., -0.0275, -0.572, 0.022) * roughness + vec4(1., 0.0425, 1.04, -0.04);
    let a004 = min(r.x * r.x, exp2(-9.28 * nv)) * r.x + r.y;
    return max(vec2(-1.04, 1.04) * a004 + r.zw, vec2(0.));
}
fn gradient_radiance(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    // The progressively broader lobe tends to the gradient's spherical average.
    let blend = clamp(0.5 + 0.5 * direction.y * (1. - roughness * roughness), 0., 1.);
    return mix(draw.ground.rgb, draw.sky.rgb, blend);
}
fn local_attenuation(to_light: vec3<f32>, range: f32) -> f32 {
    let distance_squared = max(dot(to_light, to_light), 0.0001);
    var attenuation = 1. / distance_squared;
    if range > 0. {
        let ratio = sqrt(distance_squared) / range;
        let fade = max(1. - ratio * ratio * ratio * ratio, 0.);
        attenuation *= fade * fade;
    }
    return attenuation;
}
fn area_lighting(light: Light, world: vec3<f32>, base: vec3<f32>, metallic: f32,
        roughness: f32, coat_roughness: f32, n: vec3<f32>, eye: vec3<f32>, surface_t: vec3<f32>, surface_b: vec3<f32>,optics:vec4<f32>) -> vec3<f32> {
    let normal = light.direction_range.xyz;
    let helper = select(vec3(0.,1.,0.), vec3(0.,0.,1.), abs(normal.y) > 0.95);
    let t = normalize(cross(normal, helper));
    let b = cross(normal, t);
    let tangent = t * light.shape.x + b * light.shape.y;
    let bitangent = b * light.shape.x - t * light.shape.y;
    // Filter each quadrature cell's angular footprint. Without this integration
    // filter a polished surface resolves the 4x4 sample lattice as sixteen lamps.
    let side=select(4,i32(sqrt(world_settings_samples())),world_ray_shadows());
    let cell_angle=max(light.cone.z,light.cone.w)/(f32(side)*2.*max(length(light.position_kind.xyz-world),0.001));
    let footprint=min(cell_angle*cell_angle,0.25);
    let filtered_roughness=pow(min(pow(roughness,4.)+footprint,1.),0.25);
    let filtered_coat=pow(min(pow(coat_roughness,4.)+footprint,1.),0.25);
    var result = vec3(0.);
    // Deterministic 4x4 quadrature integrates emitted radiance over the actual
    // rectangle. Intensity is radiance: enlarging the emitter increases total power.
    for (var y = 0; y < side; y += 1) {
        for (var x = 0; x < side; x += 1) {
            let uv = (vec2(f32(x), f32(y)) + vec2(0.5)) / f32(side) - vec2(0.5);
            let sample_position = light.position_kind.xyz
                + tangent * (uv.x * light.cone.z) + bitangent * (uv.y * light.cone.w);
            let delta = sample_position - world;
            let l = safe_normalize(delta, n);
            let emitter_cosine = max(dot(normal, -l), 0.);
            var visibility=vec3(1.);
            if world_ray_shadows() && light.shape.w>0.5 {visibility=mix(vec3(1.),ray_transmittance(world,n,l,length(delta)),world_shadow_strength());}
            result += brdf(base, metallic, filtered_roughness, filtered_coat, n, eye, l, surface_t, surface_b,optics)
                * emitter_cosine * local_attenuation(delta, light.direction_range.w) * visibility;
        }
    }
    return result * (light.cone.z * light.cone.w / f32(side*side));
}
fn npr_area(light: Light, position: vec3<f32>, n: vec3<f32>, eye: vec3<f32>, aa: f32) -> NprLight {
    let normal = light.direction_range.xyz;
    let helper = select(vec3(0.,1.,0.), vec3(0.,0.,1.), abs(normal.y) > 0.95);
    let t = normalize(cross(normal, helper));
    let b = cross(normal, t);
    let tangent = t * light.shape.x + b * light.shape.y;
    let bitangent = b * light.shape.x - t * light.shape.y;
    let side = select(4, i32(sqrt(world_settings_samples())), world_ray_shadows());
    var result = NprLight(vec3(0.), vec3(0.));
    for (var y = 0; y < side; y += 1) {
        for (var x = 0; x < side; x += 1) {
            let uv = (vec2(f32(x), f32(y)) + vec2(0.5)) / f32(side) - vec2(0.5);
            let delta = light.position_kind.xyz + tangent * uv.x * light.cone.z
                + bitangent * uv.y * light.cone.w - position;
            let l = safe_normalize(delta, n);
            var visibility = vec3(1.);
            if world_ray_shadows() && light.shape.w > 0.5 {
                visibility = mix(vec3(1.), ray_transmittance(position, n, l, length(delta)), world_shadow_strength());
            }
            let radiance = light.color_intensity.rgb * light.color_intensity.a
                * max(dot(normal, -l), 0.) * local_attenuation(delta, light.direction_range.w) * visibility;
            let sample = npr_light(draw.npr, n, eye, l, radiance, aa);
            result.diffuse += sample.diffuse;
            result.specular += sample.specular;
        }
    }
    let weight = light.cone.z * light.cone.w / f32(side * side);
    return NprLight(result.diffuse * weight, result.specular * weight);
}
fn backdrop_hdr(uv:vec2<f32>) -> vec3<f32> {
    if world.environment.w>0.5 {
        let far=draw.inverse_view_proj*vec4(uv.x*2.-1.,1.-uv.y*2.,1.,1.);
        let near=draw.inverse_view_proj*vec4(uv.x*2.-1.,1.-uv.y*2.,0.,1.);
        return world_environment(normalize(far.xyz/far.w-near.xyz/near.w),0.);
    }
    var blend=0.;
    if draw.secondary.w>1.5 {blend=clamp(dot(uv-vec2(0.5),draw.gradient.xy)+0.5,0.,1.);}
    if draw.secondary.w>2.5 {blend=clamp(length((uv-vec2(0.5))*2.),0.,1.);}
    let y=clamp(mix(draw.background.rgb,draw.secondary.rgb,blend),vec3(0.),vec3(0.999));
    var x=y;
    if draw.environment.w<0.5 {
        let a=2.43*y-vec3(2.51);let b=0.59*y-vec3(0.03);let c=0.14*y;
        x=(-b-sqrt(max(b*b-4.*a*c,vec3(0.))))/(2.*a);
    } else if draw.environment.w<1.5 {x=y/(vec3(1.)-y);}
    return x/max(draw.environment.x,0.00001);
}
struct RefractionSample { radiance: vec4<f32>, weight: f32 }
// Check all four bilinear texels against the glass surface before filtering.
// Opaque foreground colors must never enter a rough transmission footprint.
fn refraction_sample(pixel:vec2<f32>, surface_depth:f32, conservative:bool) -> RefractionSample {
    let dimensions=vec2<i32>(textureDimensions(opaque_scene));
    let origin=vec2<i32>(floor(pixel-vec2(0.5)));
    let fraction=fract(pixel-vec2(0.5));
    var result=RefractionSample(vec4(0.),0.);
    if conservative && draw.gradient.w>0.5 {
        // Resolved MSAA color can contain foreground coverage where the single
        // depth sample sees the background. Reject a one-texel neighborhood
        // around the bilinear footprint so that coverage cannot become a halo.
        for(var y=-1;y<=2;y+=1) {
            for(var x=-1;x<=2;x+=1) {
                let q=origin+vec2(x,y);
                if all(q>=vec2(0)) && all(q<dimensions) {
                    if unpack_depth(textureLoad(opaque_depth,q,0))<surface_depth-0.00001 {return result;}
                }
            }
        }
    }
    for(var y=0;y<2;y+=1) {
        for(var x=0;x<2;x+=1) {
            let texel=origin+vec2(x,y);
            let center=vec2<f32>(texel)+vec2(0.5);
            if any(texel<vec2(0)) || any(texel>=dimensions)
                || any(center<draw.viewport.xy) || any(center>=draw.viewport.xy+draw.viewport.zw) {continue;}
            if unpack_depth(textureLoad(opaque_depth,texel,0))<surface_depth-0.00001 {continue;}
            let w=select(1.-fraction.x,fraction.x,x==1)*select(1.-fraction.y,fraction.y,y==1);
            var color=textureLoad(opaque_scene,texel,0);
            if draw.gradient.z<0.5 {
                let uv=(center-draw.viewport.xy)/draw.viewport.zw;
                color=vec4(color.rgb+backdrop_hdr(uv)*(1.-color.a),1.);
            }
            result.radiance+=color*w;
            result.weight+=w;
        }
    }
    return result;
}
fn scene_behind(pixel:vec2<f32>,fallback:vec2<f32>,surface_depth:f32,radius:f32)->vec4<f32> {
    var gathered=refraction_sample(pixel,surface_depth,true);
    if radius>0.5 {
        // A weighted 32-sample disk avoids a sharp center image and sparse
        // cross-shaped replicas in frosted glass. Fade from the bilinear sample
        // continuously at subpixel radii; no noise changes between frames.
        let blur=smoothstep(0.5,2.,radius);
        gathered.radiance*=1.-blur;
        gathered.weight*=1.-blur;
        for(var i=0;i<32;i+=1) {
            let fraction=(f32(i)+0.5)/32.;
            let angle=f32(i)*2.39996323;
            let offset=vec2(cos(angle),sin(angle))*sqrt(fraction)*radius;
            let weight=exp(-2.*fraction)*blur/(32.*0.43233236);
            let tap=refraction_sample(pixel+offset,surface_depth,true);
            gathered.radiance+=tap.radiance*weight;
            gathered.weight+=tap.weight*weight;
        }
    }
    // An unblurred local fallback retains the original antialiased silhouette
    // if the conservative footprint has no usable background samples.
    if gathered.weight<0.00001 {gathered=refraction_sample(fallback,surface_depth,false);}
    if gathered.weight>0.00001 {return gathered.radiance/gathered.weight;}
    if draw.gradient.z<0.5 {
        return vec4(backdrop_hdr((fallback-draw.viewport.xy)/draw.viewport.zw),1.);
    }
    return vec4(0.);
}
// Bounded screen-space obscurance: sixteen depth samples, world-space falloff,
// per-direction maximum and a geometric-normal bias. Only ambient light is affected.
// This is a depth approximation, not ray-traced visibility or global illumination.
fn contact_occlusion(world: vec3<f32>, normal: vec3<f32>, pixel: vec2<f32>,
        dx: vec3<f32>, dy: vec3<f32>) -> f32 {
    if draw.ambient_occlusion.x <= 0. || draw.params.z == -2. || (draw.surface.w > 0. && draw.npr.ramp.x < 0.5) { return 1.; }
    let radius = draw.ambient_occlusion.y;
    let footprint = clamp(vec2(radius) / max(vec2(length(dx), length(dy)), vec2(0.00001)), vec2(1.), vec2(96.));
    var sum = 0.;
    for (var direction = 0; direction < 8; direction += 1) {
        let angle = (f32(direction) + 0.375) * 0.7853981634;
        let axis = vec2(cos(angle), sin(angle));
        var horizon = 0.;
        for (var step = 1; step <= 2; step += 1) {
            let coord = floor(pixel + axis * footprint * (f32(step) * 0.35));
            if any(coord < draw.viewport.xy) || any(coord >= draw.viewport.xy + draw.viewport.zw) { continue; }
            let depth = unpack_depth(textureLoad(opaque_depth, vec2<i32>(coord), 0));
            if depth >= 0.999999 { continue; }
            let uv = (coord + vec2(0.5) - draw.viewport.xy) / draw.viewport.zw;
            let homogeneous = draw.inverse_view_proj * vec4(uv.x * 2. - 1., 1. - uv.y * 2., depth, 1.);
            let delta = homogeneous.xyz / homogeneous.w - world;
            let distance_squared = dot(delta, delta);
            if distance_squared < 0.00000001 || distance_squared >= radius * radius { continue; }
            let cosine = max(dot(normal, delta) - radius * 0.025, 0.) * inverseSqrt(distance_squared);
            horizon = max(horizon, cosine * (1. - distance_squared / (radius * radius)));
        }
        sum += horizon;
    }
    return clamp(1. - draw.ambient_occlusion.x * sum * 0.25, 0., 1.);
}
@fragment fn fs_main(v: VOut, @builtin(front_facing) front:bool) -> @location(0) vec4<f32> {
    // All implicit texture derivatives are evaluated before any divergent discard.
    let color_map=textureSample(base_texture,base_sampler,texture_uv(v.uv, 0u));
    let sampled_raw=color_map*draw.base_color*v.color;
    let mr = textureSample(metallic_roughness_texture,mr_sampler,texture_uv(v.uv, 1u));
    let normal_filtered = textureSample(normal_texture,normal_sampler,texture_uv(v.uv, 2u));
    var normal_sample = normal_filtered.xyz*2.-vec3(1.);
    normal_sample.y *= select(1., -1., draw.maps.w > 0.5);
    let ao_sample = textureSample(occlusion_texture,ao_sampler,texture_uv(v.uv, 3u)).r;
    let emission_sample = textureSample(emissive_texture,emission_sampler,texture_uv(v.uv, 4u)).rgb;
    let graph_surface=graph_evaluate(draw.graph,v.uv,color_map,sampled_raw,vec4(draw.params.x*mr.b,draw.params.y*mr.g,draw.params.zw),vec4(draw.emissive.rgb*emission_sample,0.),draw.surface);
    let sampled=graph_surface.color;
    let optics=graph_surface.surface;
    if graph_surface.normal.w>0.5 {normal_sample=graph_surface.normal.xyz;}
    let position_dx=dpdx(v.world); let position_dy=dpdy(v.world);
    // Imported normal maps have their own UV basis, including mirrored scale.
    let normal_uv = select(v.uv, texture_uv(v.uv, 2u), draw.maps.z>0.5 && graph_surface.normal.w<=0.5);
    let uv_dx=dpdx(normal_uv); let uv_dy=dpdy(normal_uv);
    let grid_width=max(fwidth(v.world.xz), vec2(0.0001));
    // Evaluate all derivatives before alpha discard and divergent light loops.
    let rest_dx = dpdx(v.rest); let rest_dy = dpdy(v.rest);
    let art = art_sample(draw.artistic,v.rest,max(length(rest_dx),length(rest_dy)));
    let height_dx = dpdx(art.height); let height_dy = dpdy(art.height);
    var base = art_base(draw.artistic,sampled.rgb,art);
    if draw.params.w > 0.5 && draw.params.w < 1.5 {
        // Grid is composited after physical captures and masked by every visible
        // surface, including glass and surfaces below the navigation plane.
        if unpack_depth(textureLoad(opaque_depth,vec2<i32>(v.clip.xy),0))<1. {discard;}
        // Navigation guide only: no opaque plane, lighting, depth writes or shadows.
        let uv = v.world.xz;
        let line = min(abs(fract(uv-0.5)-0.5)/grid_width, vec2(1.));
        let grid = 1. - min(line.x,line.y);
        let axis_x=1.-smoothstep(0.,grid_width.y*1.7,abs(uv.y));
        let axis_z=1.-smoothstep(0.,grid_width.x*1.7,abs(uv.x));
        let fade=1.-smoothstep(20.,65.,length(v.world.xz-draw.eye.xz));
        var color=mix(vec3(0.16,0.20,0.22),vec3(0.42,0.10,0.08),axis_x);
        color=mix(color,vec3(0.08,0.20,0.46),axis_z);
        return vec4(color,max(grid*0.30,max(axis_x,axis_z)*0.65)*fade);
    }
    if draw.params.w > 1.5 {
        let normal=safe_normalize(v.normal,vec3(0.,1.,0.));
        let view=safe_normalize(draw.eye.xyz-v.world,vec3(0.,0.,1.));
        let diffuse=0.25+0.7*abs(dot(normal,view));
        return vec4(vec3(0.42,0.45,0.5)*diffuse,1.);
    }
    var n=safe_normalize(v.normal, vec3(0.,1.,0.));
    let helper=select(vec3(0.,1.,0.),vec3(0.,0.,1.),abs(n.y)>0.95);
    let fallback_t=safe_normalize(cross(helper,n),vec3(1.,0.,0.));
    var tangent=safe_normalize(draw.model[0].xyz-n*dot(n,draw.model[0].xyz),fallback_t);
    var handedness=1.;
    let determinant=uv_dx.x*uv_dy.y-uv_dx.y*uv_dy.x;
    if abs(determinant)>0.0000000001 {
        // Reconstruct tangent space from the deformed surface and UV0. Mirrored UVs
        // preserve their handedness; missing/degenerate UVs retain the surface normal.
        let tangent_raw=(position_dx*uv_dy.y-position_dy*uv_dx.y)/determinant;
        let bitangent_raw=(position_dy*uv_dx.x-position_dx*uv_dy.x)/determinant;
        let orthogonal=tangent_raw-n*dot(n,tangent_raw);
        if dot(orthogonal,orthogonal)>0.00000001 && dot(bitangent_raw,bitangent_raw)>0.00000001 {
            tangent=normalize(orthogonal);
            handedness=select(-1.,1.,dot(cross(n,tangent),bitangent_raw)>=0.);
            if draw.maps.z>0.5 || graph_surface.normal.w>0.5 {
                let bitangent=cross(n,tangent)*handedness;
                let mapped=safe_normalize(vec3(normal_sample.xy*draw.maps.x,normal_sample.z),vec3(0.,0.,1.));
                n=safe_normalize(tangent*mapped.x+bitangent*mapped.y+n*mapped.z,n);
            }
        }
    }
    if draw.artistic.params.x>0.5 && draw.artistic.params.z>0. {
        // Surface-gradient bump: no UV seams, tangent dependency or mesh edits.
        let r1=cross(position_dy,n); let r2=cross(n,position_dx);
        let det=dot(position_dx,r1);
        if abs(det)>0.000000000001 {
            let rest_area=max(length(cross(rest_dx,rest_dy)),0.000000000001);
            let relief_scale=clamp(sqrt(length(cross(position_dx,position_dy))/rest_area),0.001,1000.);
            let slope=(r1*height_dx+r2*height_dy)*(relief_scale/det);
            // Limit extreme derivatives on degenerate/deforming triangles.
            n=safe_normalize(n-slope/max(1.,length(slope)/0.65),n);
        }
    }
    n*=select(-1.,1.,front);
    tangent=safe_normalize(tangent-n*dot(tangent,n),fallback_t);
    let bitangent=cross(n,tangent)*handedness;
    let surface_t=tangent*draw.anisotropy.y+bitangent*draw.anisotropy.z;
    let surface_b=bitangent*draw.anisotropy.y-tangent*draw.anisotropy.z;
    let eye=safe_normalize(draw.eye.xyz-v.world,n);
    let metallic=clamp(graph_surface.properties.x,0.,1.);
    // Derivative-based specular filtering reduces unstable subpixel highlights
    // from curved surfaces and normal maps. Evaluate derivatives before discard.
    let normal_dx = dpdx(n); let normal_dy = dpdy(n);
    let geometric_variance = max(dot(normal_dx, normal_dx), dot(normal_dy, normal_dy));
    // Normal mip alpha stores the unresolved first-moment length. Also retain
    // cancellation from interpolation between neighboring encoded directions.
    let moment_length = clamp(normal_filtered.a * min(length(normal_sample), 1.), 0., 1.);
    let mapped_variance = (1. - moment_length * moment_length) * min(draw.maps.x * draw.maps.x, 16.);
    // An overriding graph normal must not inherit blur from an unused image map.
    let variance = min(geometric_variance + select(0., mapped_variance, draw.maps.z > 0.5 && graph_surface.normal.w <= 0.5), 0.25);
    let roughness = pow(clamp(pow(clamp(art_roughness(draw.artistic,graph_surface.properties.y,art), 0.045, 1.), 4.) + variance, 0.0000041, 1.), 0.25);
    let coat_roughness = pow(clamp(pow(draw.surface.z, 4.) + variance, 0.00000081, 1.), 0.25);
    // Derivatives are evaluated before alpha discard and before divergent light loops.
    let npr_aa = clamp(length(normal_dx) + length(normal_dy), 0.001, 0.04);
    if draw.params.z >= 0. && sampled.a < draw.params.z { discard; }
    var direct=vec3(0.);
    var npr_direct = NprLight(vec3(0.), vec3(0.));
    for (var i=0u; i<u32(draw.environment.z); i+=1u) {
        let light=draw.lights[i];
        if light.position_kind.w > 2.5 {
            let center_l = safe_normalize(light.position_kind.xyz - v.world, n);
            // A center-origin shadow map is an explicit approximation for area
            // visibility; the illumination itself is integrated across the emitter.
            let visibility = select(light_visibility(i, v.world, n, center_l, position_dx, position_dy),1.,world.counts.z>0u);
            if draw.npr.ramp.x > 0.5 {
                let sample = npr_area(light, v.world, n, eye, npr_aa);
                npr_direct.diffuse += sample.diffuse * visibility;
                npr_direct.specular += sample.specular * visibility;
                continue;
            }
            direct += area_lighting(light, v.world, base, metallic, roughness, coat_roughness, n, eye, surface_t, surface_b,optics)
                * light.color_intensity.rgb * light.color_intensity.a * visibility;
            continue;
        }
        var l=-light.direction_range.xyz;
        var attenuation=1.;
        if light.position_kind.w>0.5 {
            let to_light=light.position_kind.xyz-v.world;
            let distance_squared=max(dot(to_light,to_light),0.0001);
            let distance=sqrt(distance_squared);
            l=safe_normalize(to_light,n);
            attenuation=1./distance_squared;
            if light.direction_range.w>0. {
                let ratio=distance/light.direction_range.w;
                let fade=max(1.-ratio*ratio*ratio*ratio,0.);
                attenuation*=fade*fade;
            }
            if light.position_kind.w>1.5 {
                let angle=dot(-l,light.direction_range.xyz);
                let cone=clamp((angle-light.cone.y)/max(light.cone.x-light.cone.y,0.00001),0.,1.);
                attenuation*=cone*cone;
            }
        }
        let visibility = light_visibility(i, v.world, n, l, position_dx, position_dy);
        if draw.npr.ramp.x > 0.5 {
            let sample = npr_light(draw.npr, n, eye, l,
                light.color_intensity.rgb * light.color_intensity.a * attenuation * visibility, npr_aa);
            npr_direct.diffuse += sample.diffuse;
            npr_direct.specular += sample.specular;
            continue;
        }
        direct+=brdf(base,metallic,roughness,coat_roughness,n,eye,l,surface_t,surface_b,optics)*light.color_intensity.rgb*
            light.color_intensity.a*attenuation*visibility;
    }
    for(var i=0u;i<world_emission_count();i+=1u) {
        let sample=mesh_emission(i,v.world,n,false);
        if max(sample.radiance.r,max(sample.radiance.g,sample.radiance.b))<=0. {continue;}
        if draw.npr.ramp.x>0.5 {
            let light=npr_light(draw.npr,n,eye,sample.direction,sample.radiance,npr_aa);
            npr_direct.diffuse+=light.diffuse;npr_direct.specular+=light.specular;
        } else {
            direct+=brdf(base,metallic,roughness,coat_roughness,n,eye,sample.direction,surface_t,surface_b,optics)*sample.radiance;
        }
    }
    if draw.npr.ramp.x > 0.5 {
        let geometric_normal = safe_normalize(select(-v.normal, v.normal, front), n);
        let ao = mix(1., ao_sample, clamp(draw.maps.y, 0., 1.))
            * contact_occlusion(v.world, geometric_normal, v.clip.xy, position_dx, position_dy);
        let illumination = npr_direct.diffuse + world_irradiance(n) * draw.environment.y * ao;
        let illustrated = npr_finish(draw.npr, base, n, eye, illumination, npr_direct.specular, npr_aa, v.world, max(length(position_dx), length(position_dy)));
        let color = art_finish(draw.artistic,illustrated,base,art,n,eye) + graph_surface.emission.rgb;
        return vec4(clamp(color, vec3(0.), vec3(65000.)), select(1., sampled.a, draw.params.z == -2.));
    }
    let f0 = mix(vec3(dielectric_f0(optics.x)), base, metallic);
    let nv = max(dot(n, eye), 0.0001);
    // Bent-normal approximation for the authored hemisphere environment.
    // Direct lights use the full anisotropic distribution above.
    let anisotropic_normal=safe_normalize(cross(cross(surface_b,eye),surface_b),n);
    let reflection_normal=safe_normalize(mix(n,anisotropic_normal,draw.anisotropy.x*(1.-roughness)),n);
    // Exact cosine convolution of a linear sky/ground gradient.
    let irradiance = world_irradiance(n) * draw.environment.y;
    let ab = environment_brdf(roughness, nv);
    let single_scatter = f0 * ab.x + vec3(ab.y);
    // Approximate missing microfacet interreflection: rough metals retain energy
    // instead of darkening with the old fixed 0.25 specular multiplier.
    let missing_energy = clamp(1. - ab.x - ab.y, 0., 0.999);
    let average_fresnel = f0 + (vec3(1.) - f0) / 21.;
    let multiple_scatter = single_scatter * average_fresnel * missing_energy
        / max(vec3(1.) - average_fresnel * missing_energy, vec3(0.0001));
    let reflected = scene_reflection(v.world,reflection_normal,eye,roughness) * single_scatter
        + irradiance * multiple_scatter;
    let diffuse_weight = max(vec3(1.) - single_scatter - multiple_scatter, vec3(0.));
    let geometric_normal = safe_normalize(select(-v.normal, v.normal, front), n);
    let ao = mix(1., ao_sample, clamp(draw.maps.y, 0., 1.))
        * contact_occlusion(v.world, geometric_normal, v.clip.xy, position_dx, position_dy);
    let specular_ao = clamp(pow(nv + ao, exp2(-16. * roughness - 1.)) - 1. + ao, 0., 1.);
    let coat_ab = environment_brdf(coat_roughness, nv);
    let coat_reflectance = clamp(0.04 * coat_ab.x + coat_ab.y, 0., 1.) * draw.surface.y;
    var substrate = base * (1. - metallic) * (1.-optics.w) * diffuse_weight * irradiance * ao
        + reflected * specular_ao;
    let sheen_weight=draw.fiber.x*(1.-metallic)*(1.-optics.w);
    if sheen_weight>0. {
        let fiber_color=sqrt(max(base,vec3(0.)))*sheen_weight;
        let fiber_energy=sheen_energy(nv).y;
        // Hemispherical approximation for the normalized fiber lobe. Its energy
        // comes out of the substrate before adding the environment response.
        substrate=substrate*(1.-max(fiber_color.r,max(fiber_color.g,fiber_color.b))*fiber_energy)
            + irradiance*fiber_color*fiber_energy*ao;
    }
    var coat_radiance=vec3(0.);
    if draw.surface.y>0. {coat_radiance=scene_reflection(v.world,n,eye,coat_roughness);}
    let ambient = substrate * (1. - coat_reflectance)
        + coat_radiance * coat_reflectance * specular_ao;
    let lit = art_finish(draw.artistic,direct+ambient,base,art,n,eye) + graph_surface.emission.rgb;
    if optics.w>0. {
        let eta=select(optics.x,1./optics.x,front);
        let direction=refract(-eye,n,eta);
        let path=draw.transmission.x/max(abs(dot(n,direction)),0.1);
        // Project an approximate transmission cone in world units. It follows
        // thickness, IOR and image scale instead of using a fixed pixel blur.
        let world_per_pixel=max(max(length(position_dx),length(position_dy)),0.000001);
        let radius=clamp(path*roughness*roughness*(1.-1./optics.x)/world_per_pixel,0.,64.);
        let projected=draw.view_proj*vec4(v.world+direction*path,1.);
        var pixel=v.clip.xy;
        if projected.w>0. && dot(direction,direction)>0.0001 {
            let uv=vec2(projected.x/projected.w*0.5+0.5,0.5-projected.y/projected.w*0.5);
            let candidate=draw.viewport.xy+uv*draw.viewport.zw;
            let edge=min(candidate-draw.viewport.xy,draw.viewport.xy+draw.viewport.zw-candidate);
            // Fade unsupported offscreen refraction to the local view. Clamping
            // all rays to the border would stretch a single column across glass.
            let confidence=smoothstep(0.,max(radius,2.),min(edge.x,edge.y));
            pixel=mix(v.clip.xy,candidate,confidence);
        }
        let absorption_color=select(base,draw.absorption.rgb,draw.absorption.w>0.5);
        var tint=pow(clamp(absorption_color,vec3(0.001),vec3(1.)),vec3(path/max(draw.transmission.y,0.001)));
        var behind=scene_behind(pixel,v.clip.xy,v.clip.z,radius);
        if draw.transmission.z>0.5&&world.counts.x>0u&&dot(direction,direction)>0.0001 {
            let origin=v.world+direction*world_bias(v.world);
            let exit_hit=ray_query(origin,direction,100000.,false,draw.transmission.w);
            if !front || (exit_hit.triangle!=0xffffffffu&&ray_triangles[exit_hit.triangle].c.w!=0.) {
                let coefficient= -log(clamp(absorption_color,vec3(0.001),vec3(1.)))/max(draw.transmission.y,0.001);
                behind=ray_radiance(origin,direction,roughness,select(0.,draw.transmission.w,front),vec4(coefficient,optics.x),draw.gradient.z>0.5);
                tint=vec3(1.);
            }
        }
        var transmission=optics.w*(1.-metallic)*(vec3(1.)-schlick(vec3(dielectric_f0(optics.x)),nv))*tint;
        if dot(direction,direction)<0.0001 {transmission=vec3(0.);}
        let coverage=clamp(1.-dot(transmission,vec3(1./3.))*(1.-behind.a),0.00001,1.);
        let radiance=lit+transmission*behind.rgb;
        return vec4(clamp(radiance/coverage,vec3(0.),vec3(65000.)),coverage*select(1.,sampled.a,draw.params.z == -2.));
    }
    // Rgba16Float preserves radiance through compositing and MSAA resolve.
    // Clamp only to the finite half-float range; exposure/tone mapping is a later pass.
    return vec4(clamp(lit, vec3(0.), vec3(65000.)), select(1., sampled.a, draw.params.z == -2.));
}

@fragment fn fs_wire(v: VOut) -> @location(0) vec4<f32> {
    return vec4(draw.base_color.rgb,1.);
}
