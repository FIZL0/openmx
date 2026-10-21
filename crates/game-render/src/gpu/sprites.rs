//! Particles on the GPU (P58): squares that face the camera, cut out of one particle sheet.
//!
//! The original hands its particles to a VU1 program (`Vu1Particles.vuo`, named by the sheet's
//! `.psm`), which the port does not read. What the `.psm` does say is matched: the first pass blends
//! `(Cs − Cd) × As + Cd` — plain alpha blending — and writes no depth. A sprite is `size` feet across,
//! centred on its point, its texel's alpha times the sprite's own, doubled; the colour is the
//! texel's, unlit, fogged as the models are. Drawn in every view after the models, before the overlay.
//!
//! The doubling is by eye: the sheet's puffs top out near a quarter opaque, and the original's dive
//! smoke (S57's capture) is a good deal thicker. The GS lets a vertex's alpha go to 0xff, twice its
//! "opaque" 0x80, and a modulated texel twice as strong is what the capture looks like.

use super::{Renderer, VIEW_WGSL, depth_state, sampler_entry, texture_entry, upload_texture};
use crate::pst::Texture;
use bytemuck::{Pod, Zeroable};

/// One particle to draw.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Sprite {
    /// Its centre, in the world.
    pub at: [f32; 3],
    /// Feet across.
    pub size: f32,
    /// `[left, top, right, bottom]` of the sheet, `0..1`.
    pub uv: [f32; 4],
    /// How opaque, `0..1`, over the texel's own alpha.
    pub alpha: f32,
    pub _pad: [f32; 3],
}

/// The sheet on the device, and this frame's sprites.
pub(super) struct Sprites {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    capacity: u64,
    count: u32,
}

impl Sprites {
    /// Draw this frame's sprites into the view whose uniforms are at `offset`.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, view_bind: &wgpu::BindGroup, offset: &[u32]) -> bool {
        if self.count == 0 {
            return false;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, view_bind, offset);
        pass.set_bind_group(1, &self.bind, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..self.count as u64 * std::mem::size_of::<Sprite>() as u64));
        pass.draw(0..6, 0..self.count);
        true
    }
}

impl Renderer {
    /// The particle sheet every sprite is cut from, uploaded once; a second call replaces it.
    pub fn set_sprite_sheet(&mut self, sheet: &Texture) {
        let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprites"),
            entries: &[texture_entry(0, true, wgpu::TextureViewDimension::D2), sampler_entry(1)],
        });
        let view = upload_texture(&self.device, &self.queue, sheet);
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprites"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.clamp_sampler) },
            ],
        });
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprites"),
            source: wgpu::ShaderSource::Wgsl(format!("{VIEW_WGSL}{SPRITE_WGSL}").into()),
        });
        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprites"),
            bind_group_layouts: &[Some(&self.view_layout), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprites"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Sprite>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::Greater)),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let capacity = 256;
        let buffer = self.sprite_buffer(capacity);
        self.particles = Some(Sprites { pipeline, bind, buffer, capacity, count: 0 });
    }

    /// The sprites drawn from now on, until the next call; nothing without a sheet.
    pub fn set_sprites(&mut self, sprites: &[Sprite]) {
        let n = sprites.len() as u64;
        if self.particles.as_ref().is_some_and(|s| s.capacity < n) {
            let capacity = n.next_power_of_two();
            let buffer = self.sprite_buffer(capacity);
            let s = self.particles.as_mut().expect("checked above");
            (s.buffer, s.capacity) = (buffer, capacity);
        }
        let Some(s) = self.particles.as_mut() else {
            return;
        };
        s.count = sprites.len() as u32;
        if !sprites.is_empty() {
            self.queue.write_buffer(&s.buffer, 0, bytemuck::cast_slice(sprites));
        }
    }

    fn sprite_buffer(&self, capacity: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprites"),
            size: capacity * std::mem::size_of::<Sprite>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

const SPRITE_WGSL: &str = r#"
@group(1) @binding(0) var sheet: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) alpha: f32,
  @location(2) fog: f32,
}

@vertex fn vs(@builtin(vertex_index) i: u32, @location(0) at_size: vec4f, @location(1) uv: vec4f, @location(2) alpha: vec4f) -> Out {
  // Two triangles: corners (0,0) (1,0) (1,1), (0,0) (1,1) (0,1).
  var cx = array<f32, 6>(0.0, 1.0, 1.0, 0.0, 1.0, 0.0);
  var cy = array<f32, 6>(0.0, 0.0, 1.0, 0.0, 1.0, 1.0);
  let c = vec2f(cx[i], cy[i]);
  let half = at_size.w * 0.5;
  let right = normalize(view.right.xyz);
  let up = normalize(view.up.xyz);
  let world = at_size.xyz + right * (c.x * 2.0 - 1.0) * half + up * (1.0 - c.y * 2.0) * half;
  var o: Out;
  o.clip = to_clip(world);
  o.uv = vec2f(mix(uv.x, uv.z, c.x), mix(uv.y, uv.w, c.y));
  o.alpha = alpha.x;
  o.fog = fog_at(world);
  return o;
}

@fragment fn fs(i: Out) -> @location(0) vec4f {
  let t = textureSample(sheet, samp, i.uv);
  let rgb = t.rgb * (1.0 - i.fog) + view.fog_color.rgb / 255.0 * i.fog;
  return vec4f(rgb, min(t.a * i.alpha * 2.0, 1.0));
}
"#;
