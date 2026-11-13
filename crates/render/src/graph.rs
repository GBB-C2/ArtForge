use bytemuck::{Pod, Zeroable};
use poseforge_core::{Material, MAX_MATERIAL_NODES};
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuInstruction {
    pub op_inputs: [i32; 4],
    pub value: [f32; 4],
    pub secondary: [f32; 4],
    pub resource: [i32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuGraph {
    pub meta: [u32; 4],
    pub outputs: [[i32; 4]; 2],
    pub nodes: [GpuInstruction; MAX_MATERIAL_NODES],
}
impl Default for GpuGraph {
    fn default() -> Self {
        Self {
            outputs: [[-1; 4]; 2],
            ..Self::zeroed()
        }
    }
}
impl GpuGraph {
    pub fn new(
        material: &Material,
        images: &std::collections::BTreeMap<u64, u32>,
    ) -> Result<Self, String> {
        let mut gpu = Self::default();
        if let Some(graph) = &material.graph {
            let program = graph.compile().map_err(|e| e.to_string())?;
            if graph.enabled {
                gpu.meta[0] = program.nodes.len() as u32;
                gpu.outputs = [
                    program.outputs[..4].try_into().unwrap(),
                    program.outputs[4..].try_into().unwrap(),
                ];
                for (dst, src) in gpu.nodes.iter_mut().zip(program.nodes) {
                    *dst = GpuInstruction {
                        op_inputs: src.op_inputs,
                        value: src.value,
                        secondary: src.secondary,
                        resource: [
                            src.texture
                                .and_then(|id| images.get(&id).copied())
                                .unwrap_or(0) as i32,
                            src.data_texture as i32,
                            0,
                            0,
                        ],
                    };
                }
            }
        }
        Ok(gpu)
    }
}
