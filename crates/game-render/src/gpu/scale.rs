//! The render scale (V30): the views drawn into a texture a fraction of the frame's size and
//! stretched over it, filtered, before the overlay is laid on at full size. At 4K the 3D is most of
//! a frame's GPU time and the fill rate is what it buys back; the HUD and menus stay sharp.
//!
//! [`AutoScale`] picks the fraction from the frames' measured time, so a machine that holds its
//! refresh at full size keeps it, and one that does not trades pixels for frames.

use super::{DEPTH, depth_state, sampler_entry, texture_entry, uniform_entry};
use bytemuck::{Pod, Zeroable};
use std::time::Duration;

/// The smallest fraction the scale goes to: below half the picture is mush.
pub const MIN_SCALE: f32 = 0.5;
/// The fractions are kept to sixteenths, so a scale that hovers does not remake its texture each
/// time it moves.
const STEP: f32 = 1.0 / 16.0;

/// The scale `x` would be, kept to [`MIN_SCALE`]..=1 in sixteenths (`None`/NaN read as 1).
pub fn quantise(x: f32) -> f32 {
    if !x.is_finite() {
        return 1.0;
    }
    ((x / STEP).round() * STEP).clamp(MIN_SCALE, 1.0)
}

/// The size of the texture a `size` frame's views are drawn into at `scale`.
pub fn scaled(size: (u32, u32), scale: f32) -> (u32, u32) {
    let s = |n: u32| ((n as f32 * scale).round() as u32).clamp(1, n.max(1));
    (s(size.0), s(size.1))
}

/// The render scale picked from the frames' time: down when the GPU's part of a frame overruns its
/// budget, up again when it has room to spare. The GPU's cost goes with the pixels, the square of the
/// scale, so one step down is sized to land under the budget, and steps up are one sixteenth at a
/// time so it does not overshoot and bounce.
#[derive(Clone, Debug)]
pub struct AutoScale {
    pub scale: f32,
    budget: Duration,
    sum: Duration,
    frames: u32,
}

/// How many frames make one judgement.
const WINDOW: u32 = 30;

impl AutoScale {
    /// A scale that aims to keep the GPU's part of a frame under `budget` (a refresh, say).
    pub fn new(budget: Duration) -> AutoScale {
        AutoScale { scale: 1.0, budget, sum: Duration::ZERO, frames: 0 }
    }

    /// One frame's GPU time (the draw and the wait for the surface); the scale to draw the next at.
    pub fn frame(&mut self, spent: Duration) -> f32 {
        self.sum += spent;
        self.frames += 1;
        if self.frames < WINDOW {
            return self.scale;
        }
        let mean = self.sum.as_secs_f32() / self.frames as f32;
        let budget = self.budget.as_secs_f32();
        (self.sum, self.frames) = (Duration::ZERO, 0);
        if mean > budget {
            // To the scale whose pixels fit the budget, a little under.
            let to = self.scale * (budget / mean).sqrt() * 0.95;
            self.scale = quantise(to.min(self.scale - STEP));
        } else if mean < budget * 0.5 && self.scale < 1.0 {
            self.scale = quantise(self.scale + STEP);
        }
        self.scale
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Stretch {
    /// The frame's top left and size, in the target's pixels.
    origin: [f32; 2],
    size: [f32; 2],
}

/// The low texture and how to stretch it over the frame.
pub(super) struct Upscale {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    stretch: wgpu::Buffer,
    low: Option<Low>,
}

struct Low {
    size: (u32, u32),
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

impl Upscale {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Upscale {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("upscale"),
            entries: &[
                texture_entry(0, true, wgpu::TextureViewDimension::D2),
                sampler_entry(1),
                uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("upscale"),
            source: wgpu::ShaderSource::Wgsl(UPSCALE_WGSL.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("upscale"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("upscale"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::Always)),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let stretch = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("upscale"),
            size: std::mem::size_of::<Stretch>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Upscale { pipeline, layout, stretch, low: None }
    }

    /// The low texture of `size` and its depth, made when the size changes.
    pub(super) fn low(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sampler: &wgpu::Sampler,
        size: (u32, u32),
    ) -> (wgpu::TextureView, wgpu::TextureView) {
        if self.low.as_ref().is_none_or(|l| l.size != size) {
            let texture = |label, format, usage| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let view = texture(
                "low",
                format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let depth = texture("low depth", DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT);
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("upscale"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.stretch.as_entire_binding() },
                ],
            });
            self.low = Some(Low { size, view, depth, bind });
        }
        let low = self.low.as_ref().expect("made above");
        (low.view.clone(), low.depth.clone())
    }

    /// Stretch the low texture over `frame` (x, y, width, height in the target's pixels).
    pub(super) fn draw(&self, queue: &wgpu::Queue, pass: &mut wgpu::RenderPass, frame: [u32; 4]) {
        let Some(low) = &self.low else { return };
        let stretch = Stretch {
            origin: [frame[0] as f32, frame[1] as f32],
            size: [frame[2].max(1) as f32, frame[3].max(1) as f32],
        };
        queue.write_buffer(&self.stretch, 0, bytemuck::bytes_of(&stretch));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &low.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// A triangle over the target that reads the low texture, filtered, across the frame's rectangle.
pub(super) const UPSCALE_WGSL: &str = r#"
struct Stretch {
  origin: vec2f,
  size: vec2f,
}
@group(0) @binding(0) var low: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> stretch: Stretch;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  return vec4f(f32((i & 1u) << 2u) - 1.0, f32((i & 2u) << 1u) - 1.0, 0.5, 1.0);
}

@fragment fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let uv = (pos.xy - stretch.origin) / stretch.size;
  return vec4f(textureSampleLevel(low, samp, uv, 0.0).rgb, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_keeps_to_sixteenths_between_half_and_whole() {
        assert_eq!(quantise(1.3), 1.0);
        assert_eq!(quantise(0.1), MIN_SCALE);
        assert_eq!(quantise(0.74), 0.75);
        assert_eq!(quantise(f32::NAN), 1.0);
        assert_eq!(scaled((3840, 2160), 0.5), (1920, 1080));
        assert_eq!(scaled((3, 1), 0.5), (2, 1));
    }

    #[test]
    fn the_auto_scale_drops_to_fit_the_budget_and_climbs_back() {
        let budget = Duration::from_micros(6_944);
        let mut auto = AutoScale::new(budget);
        // A GPU four times over the budget at full size: one judgement takes it to about half.
        for _ in 0..WINDOW {
            auto.frame(budget * 4);
        }
        assert_eq!(auto.scale, MIN_SCALE);
        // Just over: one step down at least, and never below the floor.
        let mut auto = AutoScale::new(budget);
        for _ in 0..WINDOW {
            auto.frame(budget.mul_f32(1.1));
        }
        assert!(auto.scale < 1.0 && auto.scale >= 0.875, "{}", auto.scale);
        // Plenty of room: back up a sixteenth a judgement until whole.
        let was = auto.scale;
        for _ in 0..WINDOW {
            auto.frame(budget / 4);
        }
        assert_eq!(auto.scale, was + STEP);
        for _ in 0..WINDOW * 16 {
            auto.frame(budget / 4);
        }
        assert_eq!(auto.scale, 1.0);
        // In between it stays put.
        let mut auto = AutoScale::new(budget);
        for _ in 0..WINDOW * 4 {
            auto.frame(budget.mul_f32(0.7));
        }
        assert_eq!(auto.scale, 1.0);
    }
}
