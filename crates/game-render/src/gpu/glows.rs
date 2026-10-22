//! Glows on the GPU (V51): [`crate::glow`]'s tinted triangles, each view its own list.
//!
//! The textures are uploaded once ([`Renderer::set_glow_textures`]); every frame the views' glows are
//! laid end to end in one vertex buffer ([`Renderer::set_glows`]) and drawn in runs of one texture
//! and one blend, after the view's models and particles. Depth read, never written; no fog; the
//! texel times the vertex's tint; repeating texture coordinates, as the zone's column scrolls its.

use super::{Renderer, VIEW_WGSL, depth_state, sampler_entry, texture_entry, upload_texture};
use crate::glow::Glow;
use crate::pst::Texture;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct Vertex {
    at: [f32; 3],
    _pad: f32,
    uv: [f32; 2],
    _pad2: [f32; 2],
    tint: [f32; 4],
}

/// A run of one view's vertices that share a texture and a blend.
struct Run {
    view: usize,
    texture: usize,
    additive: bool,
    first: u32,
    count: u32,
}

/// The glow textures and pipelines on the device, and this frame's runs.
pub(super) struct Glows {
    blended: wgpu::RenderPipeline,
    added: wgpu::RenderPipeline,
    binds: Vec<wgpu::BindGroup>,
    buffer: wgpu::Buffer,
    capacity: u64,
    runs: Vec<Run>,
}

impl Glows {
    /// Draw view `v`'s runs; how many draws that was.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, view_bind: &wgpu::BindGroup, offset: &[u32], v: usize) -> usize {
        let mut draws = 0;
        for run in self.runs.iter().filter(|r| r.view == v && r.count > 0) {
            let Some(bind) = self.binds.get(run.texture).or(self.binds.last()) else { continue };
            pass.set_pipeline(if run.additive { &self.added } else { &self.blended });
            pass.set_bind_group(0, view_bind, offset);
            pass.set_bind_group(1, bind, &[]);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            pass.draw(run.first..run.first + run.count, 0..1);
            draws += 1;
        }
        draws
    }
}

impl Renderer {
    /// The pictures glows are cut from, uploaded once; a second call replaces them. A white texel is
    /// added past the end, which a glow naming no texture of the list is drawn with.
    pub fn set_glow_textures(&mut self, textures: &[Texture]) {
        let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glows"),
            entries: &[texture_entry(0, true, wgpu::TextureViewDimension::D2), sampler_entry(1)],
        });
        let white = Texture { width: 1, height: 1, rgba: vec![255; 4] };
        let binds = textures
            .iter()
            .chain(std::iter::once(&white))
            .map(|t| {
                let view = upload_texture(&self.device, &self.queue, t);
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("glows"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    ],
                })
            })
            .collect();
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glows"),
            source: wgpu::ShaderSource::Wgsl(format!("{VIEW_WGSL}{GLOW_WGSL}").into()),
        });
        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glows"),
            bind_group_layouts: &[Some(&self.view_layout), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |blend: wgpu::BlendState| {
            self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("glows"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
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
                    targets: &[Some(wgpu::ColorTargetState { format: self.format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let blended = pipeline(wgpu::BlendState::ALPHA_BLENDING);
        let added = pipeline(wgpu::BlendState { color: add, alpha: wgpu::BlendComponent::OVER });
        let capacity = 1024;
        let buffer = self.glow_buffer(capacity);
        self.glows = Some(Glows { blended, added, binds, buffer, capacity, runs: Vec::new() });
    }

    /// The glows drawn from now on, until the next call: `views[v]` into view `v`. Nothing before
    /// [`Renderer::set_glow_textures`].
    pub fn set_glows(&mut self, views: &[Vec<Glow>]) {
        let mut vertices = Vec::new();
        let mut runs = Vec::new();
        for (v, glows) in views.iter().enumerate() {
            for g in glows.iter().filter(|g| !g.tris.is_empty()) {
                let first = vertices.len() as u32;
                for tri in &g.tris {
                    vertices.extend(tri.iter().map(|p| Vertex { at: p.at, uv: p.uv, tint: p.tint, ..Default::default() }));
                }
                runs.push(Run { view: v, texture: g.texture, additive: g.additive, first, count: vertices.len() as u32 - first });
            }
        }
        let n = vertices.len() as u64;
        if self.glows.as_ref().is_some_and(|s| s.capacity < n) {
            let capacity = n.next_power_of_two();
            let buffer = self.glow_buffer(capacity);
            let s = self.glows.as_mut().expect("checked above");
            (s.buffer, s.capacity) = (buffer, capacity);
        }
        let Some(s) = self.glows.as_mut() else {
            return;
        };
        s.runs = runs;
        if !vertices.is_empty() {
            self.queue.write_buffer(&s.buffer, 0, bytemuck::cast_slice(&vertices));
        }
    }

    fn glow_buffer(&self, capacity: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glows"),
            size: capacity * std::mem::size_of::<Vertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

pub(super) const GLOW_WGSL: &str = r#"
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) tint: vec4f,
}

@vertex fn vs(@location(0) at: vec4f, @location(1) uv: vec4f, @location(2) tint: vec4f) -> Out {
  var o: Out;
  o.clip = to_clip(at.xyz);
  o.uv = uv.xy;
  o.tint = tint;
  return o;
}

@fragment fn fs(i: Out) -> @location(0) vec4f {
  let t = textureSample(picture, samp, i.uv);
  return vec4f(t.rgb * i.tint.rgb, clamp(t.a * i.tint.a, 0.0, 1.0));
}
"#;
