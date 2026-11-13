# Renderer — ArtForge Studio 0.13.0 implemented profile

`wgpu 24.0.5`, `glyphon 0.8.0`, `winit 0.30.12`, WGSL. There is no widget toolkit.

## Geometry, materials and lighting

- GPU skinning with four influences and up to 65,536 joints per skin, subject to the scene budget. Matrix palettes use the skin's joint order and `inverse(mesh_global) * joint_global * inverse_bind`.
- Morph position/normal deltas are preblended on CPU per instance when weights change, before GPU skinning.
- Static geometry and per-instance transforms; indexed triangle meshes with depth testing. Depth and multisample attachments are reused between frames.
- Five PBR texture slots: sRGB base color and emission; linear metallic/roughness (G/B), normal and ambient occlusion (R). Full semantic mip chains use linear-light color filtering, linear data filtering and normal-vector filtering with the first-moment length preserved in mip alpha. Normal-map source alpha is ignored. Each mip reconstructs the previous vector moment before reduction, so high-frequency opposing normals retain their unresolved variance through the full chain. Blended/masked base color uses premultiplied-alpha filtering; opaque base color and emission ignore alpha as required by their material semantics. Alpha-mode changes select a new binding key. Area integration includes odd image edges. Only referenced image/semantic pairs are uploaded; color and data uses of one image retain separate filtered chains. Trilinear repeat sampling uses 16× anisotropy when the adapter supports it, otherwise 1×. Material bindings are cached by map references.
- Layered GGX metallic/roughness direct lighting with height-correlated Smith visibility and Schlick Fresnel. Dielectric F0 comes from the authored IOR (1–3); metallic F0 uses base color. A separate coat lobe (weight 0–1, roughness 0.03–1, fixed interface IOR 1.5) attenuates the base layer on incoming and outgoing paths. Derivative normal variance and the unresolved normal-map moment broaden undersampled highlights. Normal scale zero disables both the mapped direction and its unresolved-variance contribution. Dielectric direct diffuse uses a Burley response with empirical energy normalization, so roughness also changes diffuse grazing/retroreflection behavior; this is not a proof of exact energy conservation. Normal scale and occlusion strength remain independent. Normal mapping uses the deformed surface and UV0 derivatives with mirrored-UV handedness and a fallback for degenerate UVs; imported tangent vectors are not retained.
- Up to eight enabled directional, point, spot and rectangular area lights, including color, power, inverse-square distance falloff, optional smooth range and spot cone attenuation. Empty/all-disabled lighting produces no implicit replacement lamp. Disabled lamps remain in the project. Extremely high light colors/power and emission saturate at 1e8 in GPU packing to keep arithmetic finite.
- Linear `Rgba16Float` scene buffer: lights, emission, alpha blending and MSAA resolve retain HDR radiance. A separate fullscreen pass applies live EV exposure and ACES, Reinhard or linear/clipped tone mapping once, after compositing. UI colors and the unlit editor/export backdrop bypass the exposure curve. Coverage is removed before tone mapping and restored afterwards to protect antialiased and transparent edges. Half-float scene values are bounded to 65,000 to avoid overflow.
- Independently colored sky and ground provide an analytic gradient environment. Cosine-convolved diffuse irradiance, a fitted integrated GGX Fresnel/visibility response, roughness-dependent specular fill, approximate multiple scattering for rough metals and specular occlusion replace the earlier fixed specular multiplier. Clearcoat has a separately filtered environment lobe. This analytic fill is the fallback; optional HDRI and geometry reflection replace its inputs in 0.13. Settings apply to viewport and PNG exports.
- Opaque and alpha-mask materials; alpha blend with object-distance sorting. `double_sided` controls backface culling; reflected object transforms select the matching winding pipeline.
- Perspective and orthographic cameras; viewport-clipped transparent helper grid.

## Rectangular area emitters

`LightKind::Area` adds a one-sided rectangular emitter at `Light.position`, with its emitting normal following `Light.direction`. `Light.area_size` is `[width, height]` in world units; old scenes receive `[1, 1]`. Both dimensions must be finite and greater than zero, up to 10,000. The deterministic in-plane basis is formed from the normal and world Y (world Z near Y); area_rotation rotates that basis around the emitter normal.

The shader integrates emitted radiance using 16 deterministic rectangular sample positions, emitter cosine, inverse-square falloff and the existing smooth range cutoff. `intensity` is radiance for this type: a larger rectangle at equal intensity emits more total power. Light dimensions, orientation, color, range and intensity update without rebuilding material bindings. This is an actual finite emitter integration, not a point light with a renamed UI control. Very glossy, smooth surfaces may resolve the finite quadrature samples; 16 samples are a bounded realtime approximation, not an analytic LTC solution.

Area shadows use six depth faces from the emitter center and the existing PCF kernel. The center visibility multiplies the integrated illumination, so shadow edges do not physically widen with the emitter size and off-center visibility is not integrated. This limit is intentional and explicit; accurate area visibility/penumbrae are future work. Area emitters are authorable in ArtForge Studio; the glTF punctual-light extension does not encode them.

## Shadows for directional, point, spot and area lights

Every enabled light with `casts_shadow=true` may cast a shadow, up to the existing eight-light limit. Disabling a light's shadow leaves its direct illumination enabled. Directional lights and ordinary spots use one depth layer; point and area lights use six perspective faces. Spots with outer half-angle at least 89 degrees use six faces to avoid a near-180-degree perspective singularity while retaining their spot attenuation.

Depth maps are a `texture_depth_2d_array` with working layers and, when required for GLES compatibility, empty service layers. The physical allocation uses at least two layers and adds one when that allocation would otherwise be a multiple of six. Thus eight point lights use 48 working layers in a 49-layer allocation. The requested side length is 512, 1024 or 2048. A 128 MiB calculated depth-texel budget includes this padding and halves the effective size when required; the authored request remains unchanged. This budget does not measure or limit total GPU memory: image textures, geometry, other render targets, uniform buffers and internal driver resources are additional.

`Renderer::shadow_stats()` reports `requested_resolution`, `effective_resolution`, `shadow_lights`, `layers` and `bytes` for the most recently rendered frame. `layers` counts working faces; `bytes` counts the physically allocated `Depth32Float` texel storage, including service layers. At 512 pixels, 48 working/49 physical layers occupy 51,380,224 bytes. With shadows disabled, `layers` is zero and a 1×1×2 fallback allocation occupies 8 bytes. The UI shows effective resolution and an automatic-reduction marker.

A weighted 5×5 tent kernel filters depth comparisons (PCF); `shadow_filter_radius` controls its unchanged 0–4 texel radius. The finer taps reduce the regular 3×3 box pattern; the weights sum to 81. Zero uses one filtered comparison. Each tap's reference depth is corrected using the geometric receiver plane derived from position derivatives evaluated before `discard`. This correction does not use the normal map's shading normal and also accounts for the hardware bilinear comparison footprint, reducing false self-shadowing on sloped surfaces. Point-light taps are reprojected across cube-face boundaries. Strength and receiver bias are adjustable, with a small fixed raster depth bias. Filtering is not a physically varying area-light penumbra.

Each shadow pass has its own camera uniform buffer/bind group so recording several passes cannot accidentally make every face use the last uploaded matrix. All lights sample the corresponding immutable per-frame camera/layer table.

The shadow pass shares the main pass's morphed vertex data, joint palette, material alpha cutoff, winding and double-sided state. Alpha-blended and transmissive materials receive shadows and cast partial scalar shadows through stochastic depth coverage; there are no colored shadow caustics. Masked materials cast their texture cutouts. Viewport and inherited export visibility select their respective casters; hidden meshes do not cast.

Directional cameras fit visible mesh bounds, with padding and quantized extents. Cached per-joint bounds follow animation without reskinning every vertex on the CPU. The helper grid is excluded from shadow fitting and never receives shadows. Local-light projections use near/far limits derived for their position/range and visible scene bounds. Very large scenes, thin geometry and extreme distances remain limited by depth precision and effective resolution.

These are conventional maps, without virtual shadow pages or directional cascades. Up to 48 working depth layers may be required, so several shadowed point lights cost more geometry passes than a single sun. Area-light center visibility has the approximation described above when ray shadows are disabled.

## Antialiasing and adapter compatibility

The project selects 1× or 4× MSAA. The renderer creates 4× pipelines only if the HDR scene format supports both 4× samples and resolve, and the depth format supports 4× samples. `Renderer::effective_antialiasing(requested)` reports the applied count; unsupported 4× requests use 1×. The display transform and UI/glyphon remain at a single sample after the HDR scene resolve.

Material images use independently typed sRGB/linear allocations for each referenced semantic, including adapters without `VIEW_FORMATS`. Different semantics cannot share mip storage because their correct downsampling differs. An image used in only one semantic is uploaded once; unused chains are retired. `Renderer::uses_compatible_texture_uploads()` returns true for this portable path, and `adapter_name` includes the backend.

A full 2D mip chain costs approximately one third more texture storage than the base image. Linear HDR costs eight bytes per pixel per sample plus the single-sample resolved buffer. 4× MSAA therefore adds a 32-byte-per-pixel HDR attachment; depth and driver allocations are additional. The 5×5 shadow filter also performs more comparisons than the former 3×3 kernel. These quality improvements are not a claim of higher speed.

## UI, particles and export

- Rectangular UI draw lists and Unicode shaping/rasterization through glyphon/cosmic-text.
- Instanced round particle diagnostics for the fluid solver. Extracted fluid meshes use the regular material/light/shadow pipeline; diagnostic particles remain simple billboards and render in the opaque phase so translucent foreground surfaces can attenuate them.
- Offscreen PNG using the same geometry, lighting, shadow, tone-map and antialiasing settings as the viewport. UI is omitted. Export reads the project's background color; the interactive viewport uses its chosen theme, solid or gradient background.
- Transparent export clears to zero alpha and omits the editor's generated grid. Existing scene meshes, including an imported floor, retain their authored export visibility.
- Aligned GPU readback and BGRA-to-RGBA conversion where required; linear-light alpha unpremultiplication for transparent PNG.
- Project output width, height and transparency are persistent authoring settings. `Renderer::export_png` still accepts dimensions and transparency explicitly, so callers can deliberately render a different output size.

The host must call `Renderer::invalidate_assets()` after replacing geometry/texture pixels under existing IDs or loading another project. Scalar material edits, texture-slot reassignment, global transforms, lights, render settings and morph weights are read every frame. Unchanged image data is not re-uploaded while cached.

## Scope limits

HDR-image IBL, single-hit geometry reflections and area-light ray visibility are implemented in 0.13. Full path tracing, caustics, correct per-triangle alpha sorting, TAA, GPU morph targets and device-loss recovery are not implemented. Alpha-masked mipmaps average alpha but do not preserve coverage for the authored cutoff; fine distant cutouts can thin or disappear. The hemispherical fallback is an approximation; an assigned HDRI supplies image-based lighting. Tone mapping follows HDR composition, but there is no bloom or temporal history. Material slots preserve glTF wrapping and mag/min/mip filters independently, including no-mip NEAREST/LINEAR. Defaults retain trilinear/aniso repeat. A common alternate UV set is remapped to UV0; mixed per-map UV sets are still not reproduced. Linear vertex RGBA multiplies the base color in raster/depth/shadow passes and ray hits. Ray triangles pack vertex color as linear RGBA8 (raster colors stay f32) to retain the one-million-triangle bound within 128 MiB. Ray base-color sampling remains a level-zero 256×256 atlas approximation; it honors wrapping and magnification filtering, without ray-differential minification/mip selection. The bounded material DAG has seven node kinds and four PBR outputs; arbitrary shader code and Blender graph import remain outside this profile. Authored IOR/coat fields do not imply import support for KHR_materials_ior or KHR_materials_clearcoat; optional occurrences still produce explicit importer warnings and required unsupported extensions are rejected. See the [material authoring guide](../../docs/MATERIALS_RENDERING.md).

## Validation

```sh
cargo test -p poseforge-render --lib --locked
cargo test -p poseforge-render --test gpu --locked -- --ignored --nocapture --test-threads=1
```

New GPU regressions verify that 25%-opaque HDR4 over black matches opaque HDR1 under all three display curves, that transparent PNG retains straight-alpha color, and that a heavily minified black/white checker converges to sRGB188 rather than encoded-space128 or aliased noise. Additional GPU cases verify particle ordering under translucent surfaces, rough white-metal energy under a uniform white environment, alpha-independent emission/opaque mips, and live opaque/mask mode changes. Nine CPU mipmap tests cover gamma, transparency, alpha-independent color, normal direction and moment preservation across levels, ignored source alpha, odd dimensions and constant preservation. Additional GPU tests verify rectangular emitter size/orientation/range and live changes, normal variance against a broadened-lobe reference (including zero normal scale), and diffuse roughness response with IOR 1 removing the specular interface. The explicit GPU tests also create PNGs and check animated skinning/morphs, image dimensions/alpha, all five PBR slots, live slot reassignment, light enable/range/cones, exposure, extreme HDR saturation, mirrored/double-sided culling, tone curves, colored ambient, typed color/data uploads, shadow occlusion/strength/resolution/visibility/alpha masks, multiple light types and shadow toggles, layered material response, and the effective antialiasing mode. They require a real or software graphics adapter; the absence of an adapter is a test failure. CPU tests check the Rust/WGSL uniform ABI, projection containment and face selection, bounded shadow allocation, background policy and explicit light-count behavior. Test definitions are not execution claims; consult the [release validation report](../../docs/VALIDATION.md) for actual results and the tested adapter.

Primary references: [wgpu 24 texture format capabilities](https://docs.rs/wgpu/24.0.5/wgpu/struct.TextureFormatFeatureFlags.html), [WGSL derivative functions](https://www.w3.org/TR/WGSL/#derivative-builtin-functions), and [NVIDIA GPU Gems: shadow-map antialiasing / percentage-closer filtering](https://developer.nvidia.com/gpugems/gpugems/part-ii-lighting-and-shadows/chapter-11-shadow-map-antialiasing).

The new example `examples/material-lab.poseforge` demonstrates six surface samples plus a base material under point/spot illumination with no sun. A shared-resource edit affects every material user; the Core authoring API isolates selected mesh instances when creating, assigning or duplicating a material. Older project files default to IOR 1.5, zero coat and enabled per-light shadow flags.

PBR references: [Filament: GGX, correlated Smith visibility, Fresnel and layered materials](https://google.github.io/filament/main/filament.html), [Blender Principled BSDF](https://docs.blender.org/manual/en/latest/render/shader_nodes/shader/principled.html). This renderer implements reflective and screen-space transmissive approximations, not full Principled parity. [EEVEE's shadow techniques](https://docs.blender.org/manual/en/latest/render/eevee/light_settings.html) and [ray tracing/probes](https://docs.blender.org/manual/en/latest/render/eevee/render_settings/raytracing.html) identify capabilities that a future comparison must account for. No performance or quality lead over EEVEE is claimed without matched-scene, matched-hardware measurements.


Rough-diffuse and normal-filtering references: [Disney/Burley, Physically Based Shading at Disney](https://media.disneyanimation.com/uploads/production/publication_asset/48/asset/s2012_pbs_disney_brdf_notes_v3.pdf) and [Toksvig, Mipmapping Normal Maps](https://developer.download.nvidia.com/assets/gamedev/docs/Mipmapping_Normal_Maps.pdf). The variance-to-roughness mapping and diffuse normalization here are bounded realtime approximations inspired by these principles.

## Artwork viewport in 0.7

The helper grid uses alpha-blended unlit lines, has no depth writes, and does not contribute to shadow bounds. Both opaque and transparent artwork exports omit the grid. Authored floor meshes keep their normal visibility, light and shadows.

Project transparency enables a display-only checkerboard inside the scene viewport. It is composed after exposure and tone mapping, uses 12 logical-pixel cells scaled by DPI, and leaves the UI outside the viewport opaque. Export still writes a straight-alpha PNG without the checker. `capture_viewport_png` is a diagnostic capture of the real GPU scene and glyphon UI path; it is not an OS-window screenshot.

## Transmission, atmosphere and backgrounds in 0.8

The opaque HDR scene is copied before the transparent pass. A single-sample depth prepass prevents foreground refraction leaks and terminates the 32-step height-fog ray march. Transmission uses Fresnel, an authored thickness, Beer attenuation and a roughness blur. Glass and AlphaBlend casters use deterministic stochastic coverage, filtered by PCF. Shadows are scalar, not colored caustics. Fog uses single scattering, HG phase and shadow-map visibility. Solid/linear/radial backgrounds are display colors and do not light geometry. Checkerboard and PNG alpha retain their separate roles.

The snapshot contains opaque geometry only. Refraction outside the viewport, nested transparent shells, multiple scattering and physically coupled glass/fog are not solved. Details: [artwork guide](../../docs/ARTWORK.md).

The 4×4 area-light quadrature filters the angular footprint of each cell into GGX roughness to suppress separate point highlights on polished surfaces. This broadens sharp rectangular reflections approximately; it is not an exact LTC integral.

## Current material and transmission model

Anisotropic GGX, compensated fiber sheen, independent absorption color and the depth-aware rough-transmission filter are documented in [RENDER_PROFILE.md](../../docs/RENDER_PROFILE.md). The current filter uses 32 weighted samples and an MSAA foreground guard; executed checks are recorded in [VALIDATION.md](../../docs/VALIDATION.md).

## Visibility and conditional passes in 0.11

Camera-space mesh culling uses conservative world bounds including morphs and skinning. Shadow submissions use each light face's clip volume independently, so a camera-invisible mesh can still cast a visible shadow. AABB corners are tested against homogeneous clip planes; non-finite input is kept conservatively. Light-face bounds are computed once per shadow stage.

Only visible mesh-assigned transmission materials request the opaque HDR snapshot. The opaque depth prepass is skipped when neither visible transmission nor volumetric fog needs it. Unused glass materials no longer trigger these passes. `Renderer::frame_stats()` reports candidate, culled, scene/depth/shadow draw counts and pass flags.

GPU regressions compare a scene with 32 offscreen meshes byte-for-byte against its reference, then move one into view, enable fog and verify shadow-face draw reduction without losing the actual shadow. All 31 GPU scenarios pass on software Vulkan; physical GPU FPS is not measured. These changes reduce submitted work, while material/refraction limitations remain unchanged.

## Materials and lighting in 0.12

All five material maps first share the editor mapping `offset + rotation * (UV0 * scale)`. Each map then applies its imported `KHR_texture_transform` independently, using two affine rows in the draw uniform. Color, alpha mask, depth and shadow passes use the base-map coordinates; the normal map reconstructs its tangent frame from its own UVs. The ray atlas applies the base-map transform too. Uniform updates reuse texture bindings. A common alternate glTF UV set is remapped to mesh UV0 on import; simultaneous different sets for different maps are not represented.

Light temperature optionally multiplies the authored linear tint. A 50 K process-wide table integrates Planck spectra from 380 to 780 nm with 5 nm steps and the CIE 1931 piecewise approximation of Wyman/Sloan/Shirley. RGB is clipped to gamut and normalized to unit luminance; the shader remains RGB. Area roll is packed into both scene/post light layouts; direct quadrature rotates the actual rectangle, while center-origin shadow visibility stays approximate.

Contact AO requests the existing opaque depth prepass and samples eight screen directions at two distances. Geometric-normal bias, world-space distance falloff, viewport rejection and a 96 px footprint limit constrain the estimate. The per-direction maximum attenuates ambient diffuse/specular, not direct light or emission. Blend/transmissive surfaces are excluded. There is no temporal history, offscreen geometry, denoising or full horizon integration; halos and sampling structure remain possible. Quality presets set strength to 0 or 0.7; default/migrated settings retain 0. Physical GPU performance is not established.

The GPU tests verify ambient-only darkening and unchanged alpha in both projections, mapped UV against baked geometry including cutout shadows, and temperature/roll changes with exact reset. Current results are in the validation guide; the editable example is `examples/lookdev-studio.artforge`.

## HDRI, geometry rays and graphs in 0.13

`world.rs/world.wgsl` implement environment resources, SH/GGX filtering, a stackless triangle BVH, single-hit reflections and per-sample area-shadow segments. `graph.rs/graph.wgsl` evaluate bounded material programs in surface, alpha-depth/shadow and secondary-hit shading. `environment.wgsl` prefilters the panorama once per image. The precise algorithms, budgets and remaining approximations are in [RENDER_PROFILE.md](../../docs/RENDER_PROFILE.md#hdri-triangle-rays-and-material-graphs-in-013); controls and examples are in [LIGHTING.md](../../docs/LIGHTING.md).

Skin palettes are allocated to the actual joint count (one placeholder matrix for unskinned draws), resized when that count changes and checked against both GPU buffer limits before upload. The aggregate project budget is 1,048,576 matrices (64 MiB); glTF additionally retains its 10,000-node import limit. Zero-weight slots are not read by the shader; removing a skin disables deformation explicitly. GPU execution of the new palette-resize regression requires an adapter.
