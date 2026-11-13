//! Optional GPU timestamps. Mapping is asynchronous and never stalls the editor.
use std::sync::mpsc::{self, Receiver};
struct Slot {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    pending: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
}
pub(super) struct GpuTimer {
    slots: Vec<Slot>,
    pub milliseconds: Option<f64>,
}
impl GpuTimer {
    pub fn features() -> wgpu::Features {
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
    }
    pub fn new(device: &wgpu::Device) -> Self {
        let slots = if device.features().contains(Self::features()) {
            (0..3)
                .map(|_| Slot {
                    query: device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: Some("Frame GPU timer"),
                        ty: wgpu::QueryType::Timestamp,
                        count: 2,
                    }),
                    resolve: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Timestamp resolve"),
                        size: 16,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }),
                    readback: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Timestamp readback"),
                        size: 16,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    pending: None,
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            slots,
            milliseconds: None,
        }
    }
    pub fn begin(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Option<usize> {
        device.poll(wgpu::Maintain::Poll);
        for slot in &mut self.slots {
            if let Some(result) = slot.pending.as_ref().and_then(|rx| rx.try_recv().ok()) {
                if result.is_ok() {
                    let bytes = slot.readback.slice(..).get_mapped_range();
                    let a = u64::from_ne_bytes(bytes[..8].try_into().unwrap());
                    let b = u64::from_ne_bytes(bytes[8..16].try_into().unwrap());
                    if b >= a {
                        self.milliseconds =
                            Some((b - a) as f64 * queue.get_timestamp_period() as f64 / 1_000_000.);
                    }
                    drop(bytes);
                    slot.readback.unmap();
                }
                slot.pending = None;
            }
        }
        let i = self.slots.iter().position(|s| s.pending.is_none())?;
        encoder.write_timestamp(&self.slots[i].query, 0);
        Some(i)
    }
    pub fn end(&self, encoder: &mut wgpu::CommandEncoder, slot: Option<usize>) {
        if let Some(i) = slot {
            let s = &self.slots[i];
            encoder.write_timestamp(&s.query, 1);
            encoder.resolve_query_set(&s.query, 0..2, &s.resolve, 0);
            encoder.copy_buffer_to_buffer(&s.resolve, 0, &s.readback, 0, 16);
        }
    }
    pub fn submitted(&mut self, slot: Option<usize>) {
        if let Some(i) = slot {
            let (tx, rx) = mpsc::channel();
            self.slots[i]
                .readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
            self.slots[i].pending = Some(rx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a Vulkan, DX12, Metal or OpenGL adapter"]
    fn timestamp_readback_is_asynchronous_and_finite_when_supported() {
        let renderer = pollster::block_on(crate::Renderer::headless(32, 32)).unwrap();
        let d = &renderer.device;
        let q = &renderer.queue;
        let mut timer = GpuTimer::new(d);
        let buffer = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        for _ in 0..4 {
            let mut e = d.create_command_encoder(&Default::default());
            let slot = timer.begin(d, q, &mut e);
            e.clear_buffer(&buffer, 0, None);
            timer.end(&mut e, slot);
            q.submit([e.finish()]);
            timer.submitted(slot);
            d.poll(wgpu::Maintain::Wait);
        }
        if d.features().contains(GpuTimer::features()) {
            assert!(timer.milliseconds.is_some_and(|v| v.is_finite() && v >= 0.));
        } else {
            assert!(timer.milliseconds.is_none());
        }
    }
}
