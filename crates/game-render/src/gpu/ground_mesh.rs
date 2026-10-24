//! The roads, ruts and runways painted into the ground on the GPU (V67, [`crate::ground_mesh`]).
//!
//! The original draws a freeride scene's screen-space meshes from above into the ground's paint
//! target, after the splat's passes, and the ground samples that target. Here they are drawn from
//! above into two targets of their own, each of which holds what the meshes do to whatever paint is
//! under them — `over` in the colour and `keep` in the alpha, the paint becoming `paint * keep +
//! over` ([`GroundMeshes::affine`]) — and the ground's shader applies them to the splat's colour:
//!
//! * **far**: the whole of the meshes' rectangle, drawn once when the ground is set;
//! * **near**: [`NEAR_FEET`] round the first view's eye at [`NEAR_PER_FOOT`] texels a foot, drawn
//!   again whenever the eye has moved to another [`STEP`]-foot square, so the meshes under the rider
//!   are as sharp as their own textures.
//!
//! Each target is drawn into every mip level, the meshes' own textures sampled at that level's size,
//! so the far ground does not shimmer. Only the modulate blend is drawn, which is every blend the
//! disc's meshes have; a mesh with another is left out here (the software renderer draws any).

use super::{sampler_entry, texture_entry, uniform_entry, upload_texture};
use crate::ground_mesh::GroundMeshes;
use crate::mat::V3;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// The near target's side in texels, and its texels a foot: 512 feet across.
const NEAR_SIZE: u32 = 2048;
pub const NEAR_PER_FOOT: f32 = 4.0;
pub const NEAR_FEET: f32 = NEAR_SIZE as f32 / NEAR_PER_FOOT;
/// The far target's side in texels.
const FAR_SIZE: u32 = 2048;
/// The near target is centred on the eye's square of this many feet, so it is drawn again only when
/// the eye crosses into another, and always reaches at least `NEAR_FEET / 2 - STEP` feet ahead.
const STEP: f32 = 64.0;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    xz: [f32; 2],
    uv: [f32; 2],
    /// The texel's colour and alpha (0..1, alpha as decoded) are multiplied by these: the vertex
    /// colour and the pass's constant colour, and for the alpha the GS's scale as well.
    k: [f32; 4],
}

/// One target: its texture, a view of each mip level to draw into, and the world square it covers.
struct Target {
    levels: Vec<wgpu::TextureView>,
    view: wgpu::TextureView,
    rect: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

pub(super) struct GpuMeshes {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    /// Runs of triangles with one texture, in drawing order: texture, first vertex, vertex count.
    runs: Vec<(usize, u32, u32)>,
    textures: Vec<wgpu::BindGroup>,
    near: Target,
    far: Target,
    near_at: Option<[i32; 2]>,
    /// The ground shader's uniform: the near and far squares, each `x0, z0, 1 / side, 0`.
    pub(super) at: wgpu::Buffer,
    far_rect: [f32; 4],
}

/// The ground shader's declarations, from binding `first` of its group 1.
pub(super) fn wgsl_bindings(first: u32) -> String {
    format!(
        "struct MeshAt {{ near: vec4f, far: vec4f }}\n\
         @group(1) @binding({}) var mesh_near: texture_2d<f32>;\n\
         @group(1) @binding({}) var mesh_far: texture_2d<f32>;\n\
         @group(1) @binding({}) var<uniform> mesh_at: MeshAt;\n",
        first,
        first + 1,
        first + 2
    )
}

/// The ground shader's code that lays the meshes over `frame`, the splat's colour in 0..255, at the
/// world's `x` and `z`. Both targets are sampled everywhere, as the derivatives need, and the near
/// one is used where it reaches.
pub(super) const APPLY_WGSL: &str = "  let mesh_un = (vec2f(x, z) - mesh_at.near.xy) * mesh_at.near.z;\n\
  let mesh_uf = (vec2f(x, z) - mesh_at.far.xy) * mesh_at.far.z;\n\
  let mesh_n = textureSample(mesh_near, clamped, mesh_un);\n\
  let mesh_f = textureSample(mesh_far, clamped, mesh_uf);\n\
  let mesh_in_near = all(mesh_un > vec2f(0.001)) && all(mesh_un < vec2f(0.999));\n\
  let mesh_in_far = all(mesh_uf >= vec2f(0.0)) && all(mesh_uf <= vec2f(1.0));\n\
  let mesh = select(select(vec4f(0.0, 0.0, 0.0, 1.0), mesh_f, mesh_in_far), mesh_n, mesh_in_near);\n\
  frame = vec4f(frame.rgb * mesh.a + mesh.rgb * 255.0, frame.a);\n";

/// The bind group layout entries the ground's shader needs for [`wgsl_bindings`].
pub(super) fn layout_entries(first: u32) -> [wgpu::BindGroupLayoutEntry; 3] {
    [
        texture_entry(first, true, wgpu::TextureViewDimension::D2),
        texture_entry(first + 1, true, wgpu::TextureViewDimension::D2),
        uniform_entry(first + 2, wgpu::ShaderStages::FRAGMENT),
    ]
}

pub(super) const DRAW_WGSL: &str = r#"
struct Rect { at: vec4f }
@group(0) @binding(0) var<uniform> rect: Rect;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp: sampler;
struct Out { @builtin(position) clip: vec4f, @location(0) uv: vec2f, @location(1) k: vec4f }
@vertex fn vs(@location(0) xz: vec2f, @location(1) uv: vec2f, @location(2) k: vec4f) -> Out {
  var o: Out;
  let t = (xz - rect.at.xy) * rect.at.z;
  o.clip = vec4f(t.x * 2.0 - 1.0, 1.0 - t.y * 2.0, 0.0, 1.0);
  o.uv = uv;
  o.k = k;
  return o;
}
@fragment fn fs(i: Out) -> @location(0) vec4f {
  let t = textureSample(tex, smp, i.uv);
  return vec4f(t.rgb * i.k.rgb, clamp(t.a * i.k.a, 0.0, 1.0));
}
"#;

impl GpuMeshes {
    /// Upload the meshes, and draw the far target.
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, sampler: &wgpu::Sampler, m: &GroundMeshes) -> GpuMeshes {
        let mut vertices = Vec::with_capacity(m.tris.len() * 3);
        let mut runs: Vec<(usize, u32, u32)> = Vec::new();
        for t in m.tris.iter().filter(|t| t.pass.blend == Some([0, 1, 0, 1])) {
            let p = &t.pass;
            for c in 0..3 {
                let col = t.color[c];
                vertices.push(Vertex {
                    xz: t.xz[c],
                    uv: t.uv[c],
                    k: [
                        col[0] / 128.0 * p.rgba[0] / 128.0,
                        col[1] / 128.0 * p.rgba[1] / 128.0,
                        col[2] / 128.0 * p.rgba[2] / 128.0,
                        // A texel's alpha is decoded doubled: 255 is 127.5 to the GS, and 128 is one.
                        col[3] / 128.0 * p.rgba[3] / 128.0 * 255.0 * 0.5 / 128.0,
                    ],
                });
            }
            let at = vertices.len() as u32 - 3;
            match runs.last_mut() {
                Some(r) if r.0 == p.texture => r.2 += 3,
                _ => runs.push((p.texture, at, 3)),
            }
        }
        if vertices.is_empty() {
            // Nothing to draw, but a buffer can't be empty.
            vertices.push(Vertex::zeroed());
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ground meshes"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let rect_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ground mesh target"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ground mesh texture"),
            entries: &[texture_entry(0, true, wgpu::TextureViewDimension::D2), sampler_entry(1)],
        });
        let textures = m
            .textures
            .iter()
            .map(|t| {
                let view = upload_texture(device, queue, t);
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("ground mesh texture"),
                    layout: &texture_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                    ],
                })
            })
            .collect();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ground meshes"),
            source: wgpu::ShaderSource::Wgsl(DRAW_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ground meshes"),
            bind_group_layouts: &[Some(&rect_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ground meshes"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                // `paint * keep + over`: the colour is laid over by the source alpha, and the alpha,
                // `keep`, is what the source leaves of what is under it.
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let target = |size: u32, label: &str| {
            let levels = 32 - size.leading_zeros();
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let level_views = (0..levels)
                .map(|l| {
                    texture.create_view(&wgpu::TextureViewDescriptor {
                        base_mip_level: l,
                        mip_level_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect();
            let rect = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &rect_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: rect.as_entire_binding() }],
            });
            Target { levels: level_views, view: texture.create_view(&Default::default()), rect, bind }
        };
        let (near, far) = (target(NEAR_SIZE, "ground meshes near"), target(FAR_SIZE, "ground meshes far"));
        let (lo, hi) = m.bounds();
        let side = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0);
        let far_rect = [lo[0], lo[1], 1.0 / side, 0.0];
        let at = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ground meshes at"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let meshes = GpuMeshes { pipeline, vertices: vertex_buffer, runs, textures, near, far, near_at: None, at, far_rect };
        // Until the first frame, the near square is nowhere.
        queue.write_buffer(&meshes.at, 0, bytemuck::cast_slice(&[[f32::MAX, f32::MAX, 0.0, 0.0], far_rect]));
        queue.write_buffer(&meshes.far.rect, 0, bytemuck::cast_slice(&far_rect));
        let mut encoder = device.create_command_encoder(&Default::default());
        meshes.draw(&mut encoder, &meshes.far);
        queue.submit([encoder.finish()]);
        meshes
    }

    /// The near target, drawn again round `eye` if it has moved to another square.
    pub(super) fn follow(&mut self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, eye: V3) {
        let square = [(eye.x / STEP).floor() as i32, (eye.z / STEP).floor() as i32];
        if self.near_at == Some(square) {
            return;
        }
        self.near_at = Some(square);
        let centre = [(square[0] as f32 + 0.5) * STEP, (square[1] as f32 + 0.5) * STEP];
        let rect = [centre[0] - NEAR_FEET / 2.0, centre[1] - NEAR_FEET / 2.0, 1.0 / NEAR_FEET, 0.0];
        queue.write_buffer(&self.near.rect, 0, bytemuck::cast_slice(&rect));
        queue.write_buffer(&self.at, 0, bytemuck::cast_slice(&[rect, self.far_rect]));
        self.draw(encoder, &self.near);
    }

    /// The near and far targets, for the ground's bind group.
    pub(super) fn views(&self) -> [&wgpu::TextureView; 2] {
        [&self.near.view, &self.far.view]
    }

    fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &Target) {
        for level in &target.levels {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ground meshes"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: level,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Nothing over the paint: add nothing, keep all of it.
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &target.bind, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            for &(texture, first, count) in &self.runs {
                let Some(bind) = self.textures.get(texture) else { continue };
                pass.set_bind_group(1, bind, &[]);
                pass.draw(first..first + count, 0..1);
            }
        }
    }
}
