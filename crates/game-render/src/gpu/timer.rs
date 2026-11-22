//! The GPU's own time for a frame's views (V122): a timestamp at the start of the views' pass and one
//! at the end of the frame's, read back a few frames later, so [`super::AutoScale`] judges the render scale on what
//! the pixels cost and not on the wall time of a draw, which under VSync is mostly the wait for the
//! display and on a busy CPU the CPU's lateness.
//!
//! Only where the adapter has `TIMESTAMP_QUERY` (Vulkan, RADV among them; Metal; not WebGL2). The
//! timestamps ride on the frame's own render passes (P346): Metal writes none from an encoder with
//! no work in it, so the empty marker passes V122 used timed no frame on the iPhone. And they are
//! resolved only once the frame's work is done, in a submit of their own (P346): resolved in the
//! frame's own command buffer, the iPhone read the last pass's end before the GPU had written it — the
//! slot's end from its last frame, 25 ms before the start — so all but a frame in hundreds read as
//! nothing.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// How many frames' timings can be in flight at once. A frame that finds none free goes untimed.
const SLOTS: usize = 8;

const FREE: u8 = 0;
const PENDING: u8 = 1;
const MAPPED: u8 = 2;
const FAILED: u8 = 3;
/// Submitted, the GPU not done with it yet.
const DRAWING: u8 = 4;
/// The GPU done with the frame: its timestamps are written and can be resolved.
const DRAWN: u8 = 5;

pub(super) struct Timer {
    queries: wgpu::QuerySet,
    /// Nanoseconds per tick of a timestamp.
    period: f32,
    slots: Vec<Slot>,
    /// The frames gone untimed since [`Timer::misses`] last asked, and why (P346).
    misses: Misses,
}

/// The frames a [`Timer`] could not time: `busy` found every slot in flight; `unusable` came back with
/// timestamps that give no time (`raw` the last such pair of ticks, as read).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Misses {
    pub busy: u32,
    pub unusable: u32,
    pub raw: Option<[u64; 2]>,
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
        Some(Timer { queries, period: queue.get_timestamp_period(), slots, misses: Misses::default() })
    }

    /// The frames gone untimed since the last call.
    pub(super) fn misses(&mut self) -> Misses {
        std::mem::take(&mut self.misses)
    }

    /// A free slot for this frame's timestamps, with its `scale` kept beside; `None` when every slot
    /// is still in flight.
    pub(super) fn begin(&mut self, scale: f32) -> Option<usize> {
        let Some(k) = self.slots.iter().position(|s| s.state.load(Ordering::Acquire) == FREE) else {
            self.misses.busy += 1;
            return None;
        };
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

    /// Slot `k`'s frame is submitted: its timestamps are resolved once the GPU is done with it.
    pub(super) fn submitted(&mut self, queue: &wgpu::Queue, k: usize) {
        let state = self.slots[k].state.clone();
        state.store(DRAWING, Ordering::Release);
        queue.on_submitted_work_done(move || state.store(DRAWN, Ordering::Release));
    }

    /// The frames drawn since the last call, each with the scale it was drawn at; and the frames the
    /// GPU has finished since, their timestamps resolved and copied out for reading by a later call.
    pub(super) fn take(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<(Duration, f32)> {
        let _ = device.poll(wgpu::PollType::Poll);
        let drawn: Vec<usize> = (0..SLOTS).filter(|&k| self.slots[k].state.load(Ordering::Acquire) == DRAWN).collect();
        if !drawn.is_empty() {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame time") });
            for &k in &drawn {
                let slot = &self.slots[k];
                encoder.resolve_query_set(&self.queries, 2 * k as u32..2 * k as u32 + 2, &slot.resolve, 0);
                encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.read, 0, 2 * wgpu::QUERY_SIZE as u64);
            }
            queue.submit([encoder.finish()]);
            for &k in &drawn {
                let state = self.slots[k].state.clone();
                state.store(PENDING, Ordering::Release);
                self.slots[k].read.map_async(wgpu::MapMode::Read, .., move |r| {
                    state.store(if r.is_ok() { MAPPED } else { FAILED }, Ordering::Release);
                });
            }
        }
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
                        } else {
                            self.misses.unusable += 1;
                            self.misses.raw = Some([ticks[0], ticks[1]]);
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
