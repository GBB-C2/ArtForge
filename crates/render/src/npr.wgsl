// Shared by viewport, PNG, previews and ray-hit shading. No screen derivatives
// here: the primary pass supplies a footprint; secondary rays use a fixed one.
struct Npr {
    ramp: vec4<f32>, // enabled, bands, threshold, softness
    shadow: vec4<f32>,
    highlight: vec4<f32>, // specular size/strength, rim width, surface ink width
    rim: vec4<f32>, // RGB and strength
    ink: vec4<f32>,
    palette_mid:vec4<f32>,
    palette_light:vec4<f32>,
    hatch:vec4<f32>,
    style:vec4<f32>, // wrapped diffuse, light color, halftone strength/density
};
struct NprLight { diffuse: vec3<f32>, specular: vec3<f32> };

fn npr_light(npr: Npr, n: vec3<f32>, eye: vec3<f32>, l: vec3<f32>, radiance: vec3<f32>, aa: f32) -> NprLight {
    let nl = clamp((dot(n, l) + npr.style.x) / (1. + npr.style.x), 0., 1.);
    let half_vector = l + eye;
    let h = half_vector / max(length(half_vector), 0.00001);
    let width = max(max(npr.ramp.w, aa), 0.0001);
    let threshold = 1. - npr.highlight.x;
    let specular = smoothstep(threshold - width, threshold + width, max(dot(n, h), 0.))
        * smoothstep(0., width, nl) * npr.highlight.y;
    return NprLight(radiance * nl, radiance * specular);
}

fn npr_ramp(npr: Npr, value: f32, aa: f32) -> f32 {
    let steps = max(npr.ramp.y - 1., 1.);
    let width = max(max(npr.ramp.w, aa), 0.0001);
    var result = 0.;
    for (var i = 0u; i < 7u; i += 1u) {
        if f32(i) >= steps { break; }
        let threshold = clamp((f32(i) + 0.5) / steps + npr.ramp.z - 0.5, 0.01, 0.99);
        result += smoothstep(threshold - width, threshold + width, value);
    }
    return result / steps;
}

fn npr_hatch(npr:Npr,p:vec2<f32>,light:f32,footprint:f32)->f32 {
    let rotation=vec2(cos(npr.hatch.z),sin(npr.hatch.z));
    let coordinate=vec2(dot(p,rotation),dot(p,vec2(-rotation.y,rotation.x)))*npr.hatch.y;
    let width=clamp(footprint*npr.hatch.y,0.001,0.5);
    let line=vec2(1.)-smoothstep(vec2(0.08-width),vec2(0.08+width),abs(fract(coordinate)-vec2(0.5)));
    let fade=1.-smoothstep(0.25,0.5,width);
    return max(line.x*(1.-smoothstep(0.25,0.75,light)),line.y*npr.hatch.w*(1.-smoothstep(0.05,0.35,light)))*fade;
}

// Footprint filtering suppresses subpixel dots instead of producing moire.
fn npr_dots(p:vec2<f32>,density:f32,light:f32,footprint:f32)->f32 {
    let distance=length(fract(p*density)-vec2(0.5));
    let radius=0.48*sqrt(clamp(1.-light,0.,1.));
    let aa=max(footprint*density,0.001);
    return (1.-smoothstep(max(radius-aa,0.),radius+aa,distance))*(1.-smoothstep(0.3,0.75,aa));
}

fn npr_finish(npr: Npr, base: vec3<f32>, n: vec3<f32>, eye: vec3<f32>,
              illumination: vec3<f32>, specular: vec3<f32>, aa: f32, world_position: vec3<f32>, footprint: f32) -> vec3<f32> {
    let energy = max(dot(illumination, vec3(0.2126, 0.7152, 0.0722)), 0.);
    let hue = mix(vec3(1.), illumination / max(energy, 0.0001), npr.style.y);
    let band = npr_ramp(npr, clamp(energy, 0., 1.), aa);
    // Keep authored bands while retaining light color, HDR intensity and darkness.
    let brightness = min(energy * 8., 1.) * max(energy, 1.);
    let middle=clamp(npr.palette_light.a,0.05,0.95);
    let palette=select(mix(npr.shadow.rgb,npr.palette_mid.rgb,band/middle),
        mix(npr.palette_mid.rgb,npr.palette_light.rgb,(band-middle)/(1.-middle)),band>=middle);
    var color = base * mix(mix(npr.shadow.rgb, vec3(1.), band),palette,npr.palette_mid.a) * hue * brightness + specular;
    if npr.hatch.x>0. {
        // Continuous triplanar lines avoid camera swimming and projection seams.
        let weights=abs(n)/max(dot(abs(n),vec3(1.)),0.0001);
        let marks=vec3(npr_hatch(npr,world_position.yz,band,footprint),
            npr_hatch(npr,world_position.xz,band,footprint),npr_hatch(npr,world_position.xy,band,footprint));
        color=mix(color,npr.ink.rgb*brightness,dot(marks,weights)*npr.hatch.x);
    }
    if npr.style.z > 0. {
        let weights=abs(n)/max(dot(abs(n),vec3(1.)),0.0001);
        let dots=vec3(npr_dots(world_position.yz,npr.style.w,band,footprint),
            npr_dots(world_position.xz,npr.style.w,band,footprint), npr_dots(world_position.xy,npr.style.w,band,footprint));
        color=mix(color,npr.ink.rgb*brightness,dot(dots,weights)*npr.style.z);
    }
    let nv = clamp(dot(n, eye), 0., 1.);
    let width = max(aa, 0.0001);
    let rim = 1. - smoothstep(max(npr.highlight.z - width, 0.), npr.highlight.z + width, nv);
    color += npr.rim.rgb * npr.rim.a * rim * min(energy * 8., 1.);
    if npr.highlight.w > 0. {
        let ink = 1. - smoothstep(max(npr.highlight.w - width, 0.), npr.highlight.w + width, nv);
        color = mix(color, npr.ink.rgb * brightness, ink);
    }
    return color;
}
