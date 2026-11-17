//! The GPU's own time for a frame's views (V122): a timestamp at the start of the views' pass and one
//! at the end of the frame's, read back a few frames later, so [`super::AutoScale`] judges the render scale on what
//! the pixels cost and not on the wall time of a draw, which under VSync is mostly the wait for the
//! display and on a busy CPU the CPU's lateness.
//!
//! Only where the adapter has `TIMESTAMP_QUERY` (Vulkan, RADV among them; Metal; not WebGL2). The
//! timestamps ride on the frame's own render passes (P346): Metal writes none from an encoder with
//! no work in it, so the empty marker passes V122 used timed no frame on the iPhone.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// How many frames' timings can be in flight at once. A frame that finds none free goes untimed.
const SLOTS: usize = 4;

const FREE: u8 = 0;
const PENDING: u8 = 1;
const MAPPED: u8 = 2;
const FAILED: u8 = 3;

pub(super) struct Timer {
    queries: wgpu::QuerySet,
    /// Nanoseconds per tick of a timestamp.
    period: f32,
    slots: Vec<Slot>,
}

struct Slot {
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    state: Arc<AtomicU8>,
    /// The render scale the frame was drawn at.
    scale: f32,
}

impl Timer {
    /// A timer, where `device` was granted timestamps.
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Timer> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("frame time"),
            ty: wgpu::QueryType::Timestamp,
            count: 2 * SLOTS as u32,
        });
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 2 * wgpu::QUERY_SIZE as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        let slots = (0..SLOTS)
            .map(|_| Slot {
                resolve: buffer("frame time", wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
                read: buffer("frame time read", wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
                state: Arc::new(AtomicU8::new(FREE)),
                scale: 1.0,
            })
            .collect();
        Some(Timer { queries, period: queue.get_timestamp_period(), slots })
    }

    /// A free slot for this frame's timestamps, with its `scale` kept beside; `None` when every slot
    /// is still in flight.
    pub(super) fn begin(&mut self, scale: f32) -> Option<usize> {
        let k = self.slots.iter().position(|s| s.state.load(Ordering::Acquire) == FREE)?;
        self.slots[k].scale = scale;
        Some(k)
    }

    /// Slot `k`'s timestamps for a pass: at its start when it is the frame's `first`, at its end when
    /// the `last`.
    fn pass(&self, k: usize, first: bool, last: bool) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: first.then_some(2 * k as u32),
            end_of_pass_write_index: last.then_some(2 * k as u32 + 1),
        }
    }

    /// Both timestamps, written by the frame's passes, copied out for reading.
    pub(super) fn end(&mut self, encoder: &mut wgpu::CommandEncoder, k: usize) {
        let slot = &self.slots[k];
        encoder.resolve_query_set(&self.queries, 2 * k as u32..2 * k as u32 + 2, &slot.resolve, 0);
        encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.read, 0, 2 * wgpu::QUERY_SIZE as u64);
    }

    /// Slot `k`'s frame is submitted: read it back once the GPU has drawn it.
    pub(super) fn submitted(&mut self, k: usize) {
        let state = self.slots[k].state.clone();
        state.store(PENDING, Ordering::Release);
        self.slots[k].read.map_async(wgpu::MapMode::Read, .., move |r| {
            state.store(if r.is_ok() { MAPPED } else { FAILED }, Ordering::Release);
        });
    }

    /// The frames drawn since the last call, each with the scale it was drawn at.
    pub(super) fn take(&mut self, device: &wgpu::Device) -> Vec<(Duration, f32)> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut out = Vec::new();
        for slot in &self.slots {
            match slot.state.load(Ordering::Acquire) {
                MAPPED => {
                    if let Ok(data) = slot.read.get_mapped_range(..) {
                        let ticks: &[u64] = bytemuck::cast_slice(&data);
                        let ns = ticks[1].saturating_sub(ticks[0]) as f64 * self.period as f64;
                        // A frame the GPU clock wrapped or reset across reads as nothing.
                        if ns > 0.0 && ns < 1e9 {
                            out.push((Duration::from_nanos(ns as u64), slot.scale));
                        }
                    }
                    slot.read.unmap();
                    slot.state.store(FREE, Ordering::Release);
                }
                FAILED => slot.state.store(FREE, Ordering::Release),
                _ => {}
            }
        }
        out
    }
}

/// The timestamps for a frame's pass, where `timer` has the frame in slot `timed`: the first pass
/// writes the start and the last the end, one pass that is both writes both.
pub(super) fn writes(timer: &Option<Timer>, timed: Option<usize>, first: bool, last: bool) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
    timer.as_ref().zip(timed).map(|(t, k)| t.pass(k, first, last))
}
