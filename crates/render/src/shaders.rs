//! One assembly path for startup, headless rendering and CPU-only shader checks.
use std::sync::OnceLock;

// wgpu 24 documents that shader parsing can exceed a native thread's default stack.
pub(crate) const STACK_BYTES:usize=16*1024*1024;
pub(crate) struct Sources {pub scene:String,pub post:String,pub ui:&'static str,pub particles:&'static str}
static SOURCES:OnceLock<Result<Sources,String>>=OnceLock::new();

fn validate(name:&str,source:&str)->Result<(),String> {
    let module=wgpu::naga::front::wgsl::parse_str(source)
        .map_err(|e|format!("Шейдер {name}:\n{}",e.emit_to_string(source)))?;
    wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(),wgpu::naga::valid::Capabilities::all())
        .validate(&module).map_err(|e|format!("Шейдер {name}:\n{}",e.emit_to_string(source)))?;
    Ok(())
}
fn assemble()->Sources {
    Sources {
        scene:format!("{}\n{}\n{}\n{}\n{}",include_str!("artistic.wgsl"),include_str!("npr.wgsl"),include_str!("graph.wgsl"),
            include_str!("scene.wgsl"),include_str!("world.wgsl").replace("WORLD_GROUP","2")),
        post:format!("{}\n{}\n{}\n{}\n{}\n{}",include_str!("artistic.wgsl"),include_str!("npr.wgsl"),include_str!("graph.wgsl"),
            include_str!("post.wgsl"),include_str!("fx.wgsl"),include_str!("world.wgsl").replace("WORLD_GROUP","1")),
        ui:include_str!("ui.wgsl"),particles:include_str!("particles.wgsl"),
    }
}
pub(crate) fn sources()->Result<&'static Sources,String> {
    SOURCES.get_or_init(|| {
        // The CLI and tests may run on a small main/test stack too. Validate once
        // on a dedicated worker, then pass exactly these sources to wgpu.
        let worker=std::thread::Builder::new().name("shader-validation".into()).stack_size(STACK_BYTES).spawn(|| {
            let s=assemble();
            for (name,source) in [("scene.wgsl + NPR/PBR",s.scene.as_str()),
                ("post.wgsl + fx.wgsl",s.post.as_str()),("ui.wgsl",s.ui),("particles.wgsl",s.particles)] {
                eprintln!("GPU: проверка {name}");validate(name,source)?;
            }
            Ok(s)
        }).map_err(|e|format!("Не удалось запустить проверку шейдеров: {e}"))?;
        worker.join().unwrap_or_else(|_|Err("Поток проверки шейдеров остановлен; см. сообщение выше".into()))
    }).as_ref().map_err(Clone::clone)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_previous_fx_identifier_is_reported_before_driver_initialization() {
        let worker=std::thread::Builder::new().stack_size(STACK_BYTES).spawn(|| {
            let s=assemble();let broken=s.post.replace("guide_info","meta");
            let error=validate("post.wgsl + fx.wgsl",&broken).unwrap_err();
            assert!(error.contains("meta") && error.contains("fx.wgsl"));
        }).unwrap();worker.join().unwrap();
    }
    #[test]
    fn shader_type_errors_include_source_context_without_a_gpu() {
        let error=validate("broken.wgsl","fn broken() -> f32 { return true; }").unwrap_err();
        assert!(error.contains("broken.wgsl") && error.contains("return"));
    }
}
