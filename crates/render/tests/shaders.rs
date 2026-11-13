//! Validate both complete shader modules without requiring a graphics adapter.
#[test]
fn npr_and_pbr_shaders_parse_and_validate() {
    // The same assembler/validator runs before requesting the startup adapter.
    poseforge_render::validate_shaders().unwrap();
}
#[test]
fn antialiased_mesh_overlay_shader_validates() {
    let source = include_str!("../src/ui.wgsl");
    let module = wgpu::naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
}
