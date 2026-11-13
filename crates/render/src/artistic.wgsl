// Material-local handmade surface response. No textures, time input or screen overlay.
// Keep derivative calls in the primary fragment entry point, never in these helpers:
// the same inexpensive color response is also used by bounded secondary rays.
struct Artistic {
    params: vec4<f32>, // style (0 disabled), scale, bump, grain
    detail: vec4<f32>, // color variation, tool marks, cracks, wrapped-light amount
    paint: vec4<f32>,  // pigment density, paper, edge concentration, wash variation
    finish: vec4<f32>, // dry brush, seed, reserved, reserved
};
struct ArtSample {
    height: f32,
    patchiness: f32,
    grain: f32,
    cracks: f32,
    strokes: f32,
    fibers: f32,
};
fn art_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + vec3(33.33));
    return fract((q.x + q.y) * q.z);
}
fn art_noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f*f*(vec3(3.)-2.*f);
    return mix(mix(mix(art_hash(cell), art_hash(cell+vec3(1.,0.,0.)), u.x),
                   mix(art_hash(cell+vec3(0.,1.,0.)), art_hash(cell+vec3(1.,1.,0.)), u.x), u.y),
               mix(mix(art_hash(cell+vec3(0.,0.,1.)), art_hash(cell+vec3(1.,0.,1.)), u.x),
                   mix(art_hash(cell+vec3(0.,1.,1.)), art_hash(cell+vec3(1.,1.,1.)), u.x), u.y), u.z);
}
fn art_sample(a: Artistic, rest: vec3<f32>, footprint: f32) -> ArtSample {
    var out = ArtSample(0.,0.5,0.5,0.,0.5,0.5);
    if a.params.x < 0.5 { return out; }
    let scale = max(a.params.y,0.1);
    let seed = a.finish.y;
    let p = rest*scale + vec3(seed%251., floor(seed/251.), seed%113.)*0.173;
    let pixel = max(footprint*scale,0.00001);
    let broad_visibility = 1.-smoothstep(0.65,1.5,pixel*0.62);
    let fine_visibility = 1.-smoothstep(0.3,1.,pixel*5.3);
    // Exactly two value-noise evaluations. No multi-octave loops or CPU rebakes.
    out.patchiness = mix(0.5,art_noise(p*0.62),broad_visibility);
    out.grain = mix(0.5,art_noise(p*5.3+vec3(out.patchiness*1.7)),fine_visibility);
    let phase = (p.x + 0.36*p.y + 0.57*p.z)*12. + out.patchiness*6.;
    var marks = sin(phase);
    if a.params.x < 1.5 {
        // Imperfect elliptical fingerprint ridges; anchored before skinning.
        marks = sin(length(p.xz*vec2(0.7,1.))*24. + p.y*5. + out.patchiness*5.);
    }
    let ridge_visibility = 1.-smoothstep(0.35,1.3,pixel*4.);
    out.strokes = 0.5+0.5*marks*ridge_visibility;
    out.fibers = 0.5+0.5*sin(p.x*47.+p.y*2.+out.patchiness*7.)
        *sin(p.z*39.+out.grain*3.)*(1.-smoothstep(0.25,0.9,pixel*8.));
    out.cracks = (1.-smoothstep(0.012,0.045+pixel*0.3,abs(out.patchiness-0.49)))
        *broad_visibility*(1.-smoothstep(0.08,0.4,pixel));
    var height = (out.patchiness-0.5)*0.065
        + (out.grain-0.5)*a.params.w*0.025
        + (out.strokes-0.5)*a.detail.y*0.018
        - out.cracks*a.detail.z*0.015;
    if a.params.x > 6.5 { height += (out.fibers-0.5)*a.paint.y*0.012; }
    out.height = height*a.params.z/scale;
    return out;
}
fn art_base(a: Artistic, base: vec3<f32>, s: ArtSample) -> vec3<f32> {
    if a.params.x < 0.5 { return base; }
    let variation = 1. + (s.patchiness-0.5)*a.detail.x*0.65;
    let grain = 1. + (s.grain-0.5)*a.params.w*0.13;
    let cracks = 1.-s.cracks*a.detail.z*0.65;
    var color = base*variation*grain*cracks;
    if a.params.x > 6.5 && a.params.x < 7.5 {
        color *= 1.+(s.fibers-0.5)*a.paint.y*0.22;
    }
    return clamp(color,vec3(0.),vec3(1.));
}
fn art_roughness(a: Artistic, roughness: f32, s: ArtSample) -> f32 {
    if a.params.x < 0.5 { return roughness; }
    return clamp(roughness + (s.grain-0.5)*a.params.w*0.12 + s.cracks*a.detail.z*0.08,0.045,1.);
}
fn art_softness(a: Artistic) -> f32 {
    return select(0.,a.detail.w,a.params.x>0.5);
}
// Approximated reflected / primary radiance. Emission and alpha are deliberately
// outside this function; pigment density never makes the mesh transparent.
fn art_finish(a: Artistic, radiance: vec3<f32>, base: vec3<f32>, s: ArtSample,
              normal: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    let watercolor = a.params.x>2.5 && a.params.x<3.5;
    let gouache = a.params.x>3.5 && a.params.x<4.5;
    let pastel = a.params.x>7.5;
    if !watercolor && !gouache && !pastel { return radiance; }
    let rim = pow(clamp(1.-abs(dot(normal,eye)),0.,1.),3.);
    let wash = 1.+(s.patchiness-0.5)*a.paint.w*1.4;
    let granules = 1.+(s.grain-0.5)*a.params.w*0.9;
    let dry = a.finish.x*smoothstep(0.42,0.76, mix(s.fibers,s.grain,0.65));
    var coverage = clamp(a.paint.x*wash*granules*(1.-dry),0.,1.);
    if gouache { coverage=clamp(coverage*0.3+a.paint.x*0.7-dry*0.35,0.,1.); }
    if pastel { coverage=clamp(coverage*(0.75+0.25*s.strokes),0.,1.); }
    coverage=clamp(coverage+rim*a.paint.z*0.25*coverage,0.,1.);
    let light = clamp(dot(radiance,vec3(0.2126,0.7152,0.0722))
        /max(dot(base,vec3(0.2126,0.7152,0.0722)),0.02),0.,4.);
    let paper = vec3(0.94,0.91,0.84)*light
        *(1.+(s.fibers-0.5)*a.paint.y*0.16+(s.grain-0.5)*a.paint.y*0.12);
    let pigment = radiance*(1.-rim*a.paint.z*0.35);
    return max(mix(paper,pigment,coverage),vec3(0.));
}
