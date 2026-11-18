//! Integrate participating media once per view, before camera colour effects.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};

type Rect = [u32; 4];

pub(super) fn viewport_rect(viewport: [f32; 4], size: [u32; 2], stride: u32) -> Rect {
    let [x, y, w, h] = viewport;
    let limits = |a: f32, length: f32, bound: u32| {
        let start = a.floor().clamp(0., bound as f32) as u32 / stride;
        let end = ((a + length).ceil().clamp(0., bound as f32) as u32).div_ceil(stride);
        (start, end.saturating_sub(start))
    };
    let (x, w) = limits(x, w, size[0]);
    let (y, h) = limits(y, h, size[1]);
    [x, y, w, h]
}

fn tiles([x, y, w, h]: Rect, edge: u32) -> impl Iterator<Item = Rect> {
    (0..h).step_by(edge as usize).flat_map(move |dy| {
        (0..w)
            .step_by(edge as usize)
            .map(move |dx| [x + dx, y + dy, (w - dx).min(edge), (h - dy).min(edge)])
    })
}

pub(super) fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Рендер кадра отменён".into());
    }
    Ok(())
}

pub(super) type Progress = dyn FnMut(u32, u32) + Send + Sync;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::TextureView,
    post: &wgpu::BindGroup,
    world: &wgpu::BindGroup,
    rect: Rect,
    tile_edge: u32,
    export: bool,
    cancel: &AtomicBool,
    mut progress: Option<&mut Progress>,
) -> Result<u32, String> {
    let edge = if export {
        tile_edge
    } else {
        rect[2].max(rect[3]).max(1)
    };
    let total = rect[2].div_ceil(edge) * rect[3].div_ceil(edge);
    let mut completed = 0;
    for [x, y, width, height] in tiles(rect, edge) {
        if export {
            check_cancel(cancel)?;
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("volume scattering and transmittance"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if completed == 0 {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, post, &[]);
            pass.set_bind_group(1, world, &[]);
            pass.set_scissor_rect(x, y, width, height);
            pass.draw(0..3, 0..1);
        }
        completed += 1;
        // Never fence the window thread. On the export worker, leave at most one
        // tile in flight. Each completed batch starts a fresh GPU work deadline.
        if export && total > 1 {
            let next = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("next export batch"),
            });
            queue.submit([std::mem::replace(encoder, next).finish()]);
            let (tx, rx) = mpsc::channel::<Result<(), String>>();
            queue.on_submitted_work_done(move || {
                let _ = tx.send(Ok(()));
            });
            super::readback::wait_for_gpu(&rx, cancel, super::readback::TIMEOUT, || {
                device.poll(wgpu::Maintain::Poll);
            })?;
            if let Some(report) = progress.as_mut() {
                report(completed, total);
            }
        }
    }
    if export {
        check_cancel(cancel)?;
    }
    Ok(completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_cover_odd_offset_and_clipped_viewports_once() {
        assert_eq!(
            viewport_rect([9.5, 11., 18., 17.], [26, 40], 2),
            [4, 5, 9, 9]
        );
        assert_eq!(
            viewport_rect([-12., -3., 17., 21.], [8, 16], 1),
            [0, 0, 5, 16]
        );
        let rect = [5, 7, 259, 131];
        let mut pixels = vec![0; (rect[2] * rect[3]) as usize];
        for [x, y, w, h] in tiles(rect, 128) {
            for j in y..y + h {
                for i in x..x + w {
                    pixels[((j - rect[1]) * rect[2] + i - rect[0]) as usize] += 1;
                }
            }
        }
        assert!(pixels.into_iter().all(|hits| hits == 1));
    }
}
