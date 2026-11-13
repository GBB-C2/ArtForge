# Renderer profile — ArtForge Studio 0.13.0

The renderer uses the project's pinned `wgpu`, `glyphon`, `winit` and WGSL stack. Viewport and PNG export share geometry, lights, materials and quality settings. See [the renderer README](../crates/render/README.md) for implementation details and [VALIDATION.md](VALIDATION.md) for executed checks.

## Geometry and materials

GPU skinning supports four influences and up to 65,536 joints per skin, subject to the scene budget. Morph position/normal deltas are preblended on CPU per instance when weights change. Indexed meshes use depth testing; perspective and orthographic cameras are supported.

Five texture maps are available: sRGB base color and emission, linear metallic/roughness, normal and AO. Tangent space is reconstructed from UV0 and deformed-surface derivatives. Materials use GGX with correlated Smith visibility, Fresnel from reflection IOR, energy-normalized Burley rough diffuse, a separate clearcoat lobe, normal filtering, alpha mask/blend and double-sided rendering. The diffuse normalization is an empirical approximation, not a proof of exact energy conservation. IOR also drives the screen-space transmission pass in 0.8.

Transparent objects are sorted by object distance and rendered with depth writes disabled. This does not correctly order every pair of intersecting transparent triangles. Diagnostic fluid particles are billboards; extracted fluid meshes enter the ordinary material/light/shadow pipeline.

## HDR and image filtering

The scene renders into a linear `Rgba16Float` target. Lighting, emission, transparent composition and MSAA resolve happen before one fullscreen exposure/tone-mapping pass. Transparent coverage is unpremultiplied before the nonlinear curve and restored afterwards; editor backdrops and UI display colors bypass scene exposure. Finite output is bounded to 65,000 before HDR storage.

Textures receive full mip chains by semantic: sRGB colors are filtered in linear light; mask/blend base color uses premultiplied alpha, while opaque base color and emission remain alpha-independent. Data channels stay linear. Normal mips retain both the normalized mean direction and its mean-vector length; subsequent reductions reconstruct that moment instead of losing unresolved variation. The shader folds residual normal variation into base and clearcoat roughness to reduce subpixel sparkle. Setting normal strength to zero also disables that map contribution. Odd image dimensions use area weighting. Only referenced image/semantic pairs are uploaded; explicitly typed allocations avoid requiring texture view-format reinterpretation. The repeat sampler uses trilinear filtering and 16× anisotropy when supported, otherwise 1×.

Hemispherical ambient is the fallback when no HDRI is active. Optional image-based lighting and geometry reflections are implemented in 0.13; see the section below.

## Lights and shadows

Up to eight enabled directional, point, spot and rectangular area lights contribute to direct lighting. Point/spot lights use inverse-square falloff with optional range attenuation; spot lights apply their inner/outer cones. Empty or disabled lighting does not insert a hidden lamp.

Area lights use a one-sided rectangle normal to their direction, with width and height in world units. Direct lighting integrates 16 deterministic samples with emitter cosine, inverse-square falloff and optional range attenuation. Intensity is radiance: at a fixed value, a larger area emits more total power. The rectangle basis is derived from direction and rotated by the authored area_rotation angle. Very sharp surfaces can reveal the finite sample pattern.

Each enabled light with `casts_shadow` can receive shadow layers. A directional light uses a fitted orthographic map, a spot light a perspective map, and a point light six views. Area lights also use six views from the emitter center and one shared visibility result for the integrated light; this is an approximation and does not reproduce the physical area penumbra. Very wide spot cones use six views while preserving cone attenuation.

The depth pass follows skinning, morphs, hidden parents, mirrored winding, double-sided state and alpha masks. Blended/transmissive objects receive shadows and cast partial scalar shadows using stochastic coverage; colored caustics are not modeled.

Requested shadow resolution is 512, 1024 or 2048 pixels. Allocation is limited to 128 MiB of depth texels, including compatibility layers; effective resolution can be reduced. This budget does not include all GPU memory. Weighted 5×5 tent PCF and receiver-plane bias smooth edges and reduce self-shadowing; they do not implement an area-light penumbra. Directional cascades are absent.

## Settings and output

Exposure, tone mapping (ACES/Reinhard/linear), hemispherical sky/ground fill, shadow controls and 1×/4× MSAA are stored in the scene. Effective antialiasing depends on adapter format/resolve capabilities. Material bindings and attachment resources are cached and rebuilt when required.

Camera objects use the ordinary scene-node transform hierarchy and a stored lens. The active camera supplies the evaluated pose for frame export; without an active camera, the saved free camera is used. The camera preview fits the output aspect ratio with a safe-frame mask and adjusts projection so the framed region matches export for perspective and orthographic lenses. Camera markers and frusta are viewport aids only. Renderer clipping remains fixed at 0.01–1000 scene units; per-object lens controls do not expose separate near/far planes.

The interactive viewport uses its theme background or a DPI-aware checkerboard when project transparency is enabled. The checker is composed after tone mapping and has no lighting effect. Opaque PNGs use the project background; transparent PNGs keep authored meshes and write straight alpha. Both PNG modes omit the helper grid. The grid uses alpha-blended unlit lines with no depth writes and is excluded from shadow bounds; it never behaves as a physical floor. UI text, gizmos and editor controls are excluded from PNG output.

## Verification and limits

Executed checks belong in [VALIDATION.md](VALIDATION.md). Test sources exercise CPU packing/contracts and offscreen GPU rendering; a test's existence is not evidence of a passing run. A software adapter does not establish hardware performance or native Windows interaction.

Caustics, GI, full path tracing, TAA, per-triangle transparency sorting and GPU morph blending remain absent. HDRI, single-hit scene reflections and bounded colored ray transmittance for area shadows are available in 0.13. Hemispherical ambient is an approximation. Alpha-mask mip levels average alpha without cutoff-aware coverage preservation; very small distant cutouts can thin, disappear or fill at the cutoff. One UV set is stored per mesh; imported sampler choices, simultaneous alternate sets and arbitrary Blender node graphs are not reproduced. This implementation does not establish quality or speed superiority over Eevee.

## Transmission, atmosphere and backgrounds in 0.8

The opaque HDR scene is copied before the transparent pass. A single-sample depth prepass prevents foreground refraction leaks and terminates the 32-step height-fog ray march. Transmission uses Fresnel, an authored thickness, Beer attenuation and a roughness blur. Glass and AlphaBlend casters use deterministic stochastic coverage, filtered by PCF. Shadows are scalar, not colored caustics. Fog uses single scattering, HG phase and shadow-map visibility. Solid/linear/radial backgrounds are display colors and do not light geometry. Checkerboard and PNG alpha retain their separate roles.

The snapshot contains opaque geometry only. Refraction outside the viewport, nested transparent shells, multiple scattering and physically coupled glass/fog are not solved. Details: [artwork guide](ARTWORK.md).

The 4×4 area-light quadrature filters the angular footprint of each cell into GGX roughness to suppress separate point highlights on polished surfaces. This broadens sharp rectangular reflections approximately; it is not an exact LTC integral.

## Material response in 0.9

Direct light supports UV-aligned anisotropic GGX with correlated Smith visibility, strength 0–0.95 and rotation in degrees. Tangents are reconstructed from deformed positions and UV0; mirrored UVs retain handedness and degenerate UVs use a projected object axis. Clearcoat remains isotropic. Hemispherical specular uses a bent-normal approximation, not anisotropic scene reflections.

The Charlie/Neubelt fiber lobe is normalized by max(1, raw directional albedo of view, raw directional albedo of light). A checked-in 64×64 RGBA8 LUT stores raw and normalized albedo as two 16-bit values; its generator performs deterministic 4096-sample cosine-weighted quadrature. Layer compensation removes substrate energy before adding fibers. Ambient fibers use a hemispherical approximation. Sheen is weighted by the opaque nonmetal fraction and does not emit light. See `scripts/generate_sheen_lut.py` and [Filament shading equations](https://google.github.io/filament/main/filament.html).

Optional `attenuation_color` overrides the sampled base color only for Beer absorption in refraction and scalar shadow coverage. `None` preserves legacy base-color/texture absorption. No new glTF layer extensions, anisotropy texture, fiber geometry, colored shadows or caustics are implemented. These material settings were introduced in schema 6. Current schema 15 also persists HDRI, ray settings, material graphs, NPR, primitive parameters, polygon topology and the smoothing policy; schemas 1–14 migrate in memory. UI and examples: [MATERIALS.md](MATERIALS.md).

## Rough refraction in 0.10

The transmission filter uses 32 weighted Vogel-disk samples with a radius derived from authored optical path, squared perceptual roughness, IOR and projected world-space derivatives. The radius is capped at 64 pixels. Bilinear reconstruction accepts only in-viewport texels at or behind the transmitting surface, accumulates premultiplied HDR RGBA, and renormalizes valid weights. The sharp center fades out as blur becomes active rather than remaining visible beneath the frosted image.

When effective MSAA is greater than one, a conservative 4×4 depth neighborhood rejects sample footprints that could contain a resolved foreground color. If the entire footprint is unavailable, unblurred local reconstruction preserves the existing antialiased silhouette. Near viewport edges, displacement smoothly returns to local sampling; the shader no longer clamps to one border column. The fixed sample count and conservative fallback can show sampling structure or reduced blur near silhouettes. This remains an opaque-snapshot approximation with authored rather than geometry-measured thickness, not multilayer transport. No hardware performance claim is made.

The regression tests vary foreground color at 1×/4× MSAA (including an oblique subpixel edge), thickness, IOR = 1 and output resolution. See [VALIDATION.md](VALIDATION.md) and the editable [rough-glass.artforge](../examples/rough-glass.artforge) example.

## Visibility and conditional passes in 0.11

Camera-space mesh culling uses conservative world bounds including morphs and skinning. Shadow submissions use each light face's clip volume independently, so a camera-invisible mesh can still cast a visible shadow. AABB corners are tested against homogeneous clip planes; non-finite input is kept conservatively. Light-face bounds are computed once per shadow stage.

Only visible mesh-assigned transmission materials request the opaque HDR snapshot. The opaque depth prepass is skipped when neither visible transmission nor volumetric fog needs it. Unused glass materials no longer trigger these passes. `Renderer::frame_stats()` reports candidate, culled, scene/depth/shadow draw counts and pass flags.

GPU regressions compare a scene with 32 offscreen meshes byte-for-byte against its reference, then move one into view, enable fog and verify shadow-face draw reduction without losing the actual shadow. All 31 GPU scenarios pass on software Vulkan; physical GPU FPS is not measured. These changes reduce submitted work, while material/refraction limitations remain unchanged.

## Materials and lighting in 0.12

All five material maps first share the editor mapping `offset + rotation * (UV0 * scale)`. Each map then applies its imported `KHR_texture_transform` independently, using two affine rows in the draw uniform. Color, alpha mask, depth and shadow passes use the base-map coordinates; the normal map reconstructs its tangent frame from its own UVs. The ray atlas applies the base-map transform too. Uniform updates reuse texture bindings. A common alternate glTF UV set is remapped to mesh UV0 on import; simultaneous different sets for different maps are not represented.

Light temperature optionally multiplies the authored linear tint. A 50 K process-wide table integrates Planck spectra from 380 to 780 nm with 5 nm steps and the CIE 1931 piecewise approximation of Wyman/Sloan/Shirley. RGB is clipped to gamut and normalized to unit luminance; the shader remains RGB. Area roll is packed into both scene/post light layouts; direct quadrature rotates the actual rectangle, while center-origin shadow visibility stays approximate.

Contact AO requests the existing opaque depth prepass and samples eight screen directions at two distances. Geometric-normal bias, world-space distance falloff, viewport rejection and a 96 px footprint limit constrain the estimate. The per-direction maximum attenuates ambient diffuse/specular, not direct light or emission. Blend/transmissive surfaces are excluded. There is no temporal history, offscreen geometry, denoising or full horizon integration; halos and sampling structure remain possible. Quality presets set strength to 0 or 0.7; default/migrated settings retain 0. Physical GPU performance is not established.

The GPU tests verify ambient-only darkening and unchanged alpha in both projections, mapped UV against baked geometry including cutout shadows, and temperature/roll changes with exact reset. Current results are in the validation guide; the editable example is `examples/lookdev-studio.artforge`.

## HDRI, triangle rays and material graphs in 0.13

`World` retains linear HDR panoramas, nine diffuse SH coefficients, an eight-layer 256×128 GGX prefilter, a triangle BVH and a bounded material/color atlas. The original panorama remains available for sharp samples and the background. The prefilter uses 64 Hammersley samples per texel and is rebuilt only when the image changes; strength/rotation are uniforms. SH uses solid-angle weights and a maximum 256×128 integration grid. These are finite approximations, not an exact environment integral or environment visibility solution. Radiance import preserves values above one. PNG/JPEG imports decode sRGB. EXR is unsupported.

The stackless BVH uses preorder escape indices and median splits into leaves of at most four triangles. It includes offscreen instances and evaluated morph/skinning/transform data, with separate viewport/export visibility. Geometry hashes and explicit invalidation avoid rebuilding static geometry each frame. Vertex edits still use the existing renderer invalidation API. The cap is 1,000,000 visible triangles, also constrained by the adapter storage-buffer binding limit; exceeding either returns an error. No hardware ray-tracing API is required.

Reflections take one opaque/alpha-masked geometry hit, shade its base texture/graph, GGX, diffuse and emission, then stop. Misses sample the environment. Rough surfaces use 1/4/8/16 rays and deterministic GGX sampling; nearly polished surfaces use one. There is no temporal accumulation, denoising, multiple reflection bounce or transmissive hit shading. Secondary base maps are a 256×256 sRGB atlas (at most max_texture_array_layers−1 maps); secondary normal/coat/sheen and other texture slots are not fully reproduced. The reflected emitter hit is independent of ambient intensity.

When ray shadows are enabled, primary area lighting evaluates separate visibility along each emitter sample segment: 2×2, 4×4 or 8×8 quadrature. Triangle visibility replaces the center-origin map result for that surface/light, but the finite integral may still show structure. Bounded ray transmittance applies material alpha, Fresnel and colored Beer absorption with authored thickness through at most eight surface intersections. It does not measure a closed volume's thickness or refract a light path. Primary point/spot/sun and fog retain shadow maps; secondary-hit direct area lighting is approximated at the emitter center.

The material DAG compiles at most 16 nodes to topologically ordered instructions. Seven operations feed color RGBA, metallic R, roughness R and emission RGB. Unconnected outputs preserve existing material/map values. Graph instructions are appended to DrawUniform (1760 bytes total) and shared with ray materials; the same graph evaluation controls raster color, alpha depth and shadow coverage. IDs are stable, cycles/dangling IDs/invalid values are rejected. This is a bounded graph evaluator, not arbitrary user WGSL or Blender node compatibility.

Schema 9 embeds HDRI radiance and graphs. Images are shared by Arc across history snapshots. Schemas 1–8 migrate with no panorama, no graph and ray effects disabled. UI and operational limits: [LIGHTING.md](LIGHTING.md).

Skin palettes are allocated to the actual joint count (one placeholder matrix for unskinned draws), resized when that count changes and checked against both GPU buffer limits before upload. The aggregate project budget is 1,048,576 matrices (64 MiB); glTF additionally retains its 10,000-node import limit. Zero-weight slots are not read by the shader; removing a skin disables deformation explicitly. GPU execution of the new palette-resize regression requires an adapter.


## OpenGL depth compatibility (0.23)

The opaque and focus depth prepasses keep Depth32Float attachments for hardware depth testing and also encode the identical fragment depth bit pattern into RGBA8Unorm color attachments. UI occlusion, refraction and postprocessing read and unpack those color textures using integer textureLoad coordinates. No filtered depth sampling or float render target extension is required for these passes. The existing shadow comparison textures are unchanged. Two additional single-sample color targets cost eight bytes per viewport pixel. Validation scopes report unsupported pipeline initialization as a startup error rather than an uncaptured panic.

Actual software OpenGL image regression results and native-platform limits are recorded in [VALIDATION.md](VALIDATION.md).


## Mesh emission · 0.47.16

Actual BVH triangle surfaces provide area-stratified candidates, prepared on the existing cancellable worker and retained independently of material intensity. A cached importance table uses 8 samples for viewport shading and 16 for PNG; secondary hits use two strata and volume marching uses one sample per step. Surface normals, two-sided emission, alpha coverage, material visibility, polygon assignments and the physical layer chain are evaluated at each sample. Shadow-only triangles occlude but do not emit. Mesh emission uses its own table, preserving all eight authored light slots.

Ray materials now also sample the emission map with its own UV transform, wrapping and filtering; base and emission maps share the bounded 256×256 sRGB ray atlas. The current video frame follows normal texture revision uploads. Graph emission and layer coverage use the same evaluator. Occlusion uses the existing transparent BVH walk when scene shadows are enabled. Finite patch regularization avoids near-source singularities. Volumetric emission uses per-material `emission_volume` and the shared atmosphere.

This is bounded direct-light quadrature, not unbiased path tracing or global illumination. Small bright regions can be missed; rough reflection and fog subsampling can show structure. There is no multiple diffuse bounce, caustic solver, denoiser or temporal accumulation. The new GPU regression is opt-in and was not executed in the preparation environment; no performance or visual-equivalence claim is made.


## Cache and startup corrections · 0.47.17

Window/surface acquisition stays on the UI thread. Adapter/device/pipeline/font initialization is a single detached job; the event loop polls its one-result channel at a bounded interval, handles resize/close, and installs the completed renderer at the current window size. Initialization errors propagate to the process result. Startup time is excluded from the simulation clock.

Material appearance changes refresh authored and generated layer materials, cutoff masks and ray material data without recreating polygon batches or their buffer revisions. Non-mesh node changes update metadata and global matrices without rebuilding these batches. Structural changes still rebuild. BVH keys use mesh/skin matrices and inverse bind matrices; unrelated camera/helper transforms do not cancel completed geometry. Resource tables avoid repeated linear mesh/material lookups in these paths.

Transient presentation loss/timeout schedules up to three retries rather than leaving an idle window without a new frame. Resize or a successful frame resets recovery. Interactive adaptive emission uses 2 samples, settled preview 8, export 16; authored render settings are unchanged. These code-path corrections have prepared regression tests, but no runtime GPU measurements were available during preparation.
